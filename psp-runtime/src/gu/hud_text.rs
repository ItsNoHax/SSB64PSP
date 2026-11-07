//! Debug text drawn by the GE: 8x8 glyphs from an atlas, one sprite each,
//! queued by [`Gpu::debug_text`] and drawn by [`Gpu::end_frame`] before the
//! list closes. Replaces `sceGuDebugPrint`/`sceGuDebugFlush`, a CPU write
//! into the draw buffer that real PSPs faulted on (RE-202) and PPSSPP's
//! hardware backends never showed (D-029).
//!
//! Font: [`FONT`], rust-psp's sceGu debug font (`psp/src/sys/debugfont.bin`,
//! b1542af2), MIT, Copyright (c) 2020 Marko Mijalkovic: the font
//! `sceGuDebugFlush` drew, so the HUD keeps its pixels.

use core::ffi::c_void;
use core::sync::atomic::{AtomicBool, Ordering};

use psp::sys::{self, GuPrimitive, GuState, TexturePixelFormat};

use super::{draw_array, list_memory, Gpu, SpriteVertex};

/// Glyphs for ASCII 0x20..=0x7F: one `u64` each, the font file's 8 bytes
/// little-endian; pixel (col, row) is bit `row * 8 + col`.
#[rustfmt::skip]
const FONT: [u64; 96] = [
    0x0000000000000000, 0x0010000010101010, 0x0000000000282828, 0x0028287C287C2828,
    0x00103C5038147810, 0x0060640810204C0C, 0x0058245408141408, 0x0000000000102040,
    0x0040201010102040, 0x0004081010100804, 0x0010543810385410, 0x000010107C101000,
    0x0810100000000000, 0x000000007C000000, 0x0018180000000000, 0x0000040810204000,
    0x0038444C54644438, 0x007C101010141810, 0x007C041820404438, 0x0038444030404438,
    0x0020207C24283020, 0x00384440403C047C, 0x003844443C040830, 0x001010101020447C,
    0x0038444438444438, 0x0018204078444438, 0x0000100000100000, 0x0810100000100000,
    0x006030180C183060, 0x0000007C007C0000, 0x000C18306030180C, 0x0010001020404438,
    0x0038047454744438, 0x0044447C44442810, 0x003C48483C48483C, 0x0030480404044830,
    0x001C28484848281C, 0x007C04043C04047C, 0x000404043C04047C, 0x0038444474044438,
    0x004444447C444444, 0x0038101010101038, 0x0018242020202070, 0x004424140C142444,
    0x007C040404040404, 0x0044444454546C44, 0x00446464544C4C44, 0x0038444444444438,
    0x000404043C44443C, 0x0058245444444438, 0x004424143C44443C, 0x0038444038044438,
    0x001010101010107C, 0x0038444444444444, 0x0010282844444444, 0x00446C5454444444,
    0x0044442810284444, 0x0010101038444444, 0x007C04081020407C, 0x0070101010101070,
    0x0000402010080400, 0x001C10101010101C, 0x0000000000442810, 0x007C000000000000,
    0x0000000000000000, 0x0078447840380000, 0x00344C444C340404, 0x0038440444380000,
    0x0058644464584040, 0x0038047C44380000, 0x001010107C105020, 0x3840586464580000,
    0x004444444C340404, 0x0038101010180010, 0x0C12101010180010, 0x0024140C14240404,
    0x0038101010101018, 0x00545454542C0000, 0x004444444C340000, 0x0038444444380000,
    0x0404344C4C340000, 0x4040586464580000, 0x000404044C340000, 0x003C403C04780000,
    0x00304808083C0808, 0x0058242424240000, 0x0010284444440000, 0x0028545454440000,
    0x0044281028440000, 0x3840586444440000, 0x007C0810207C0000, 0x0030080804080830,
    0x0010101010101010, 0x000C10102010100C, 0x000000000000007C, 0x0000000000000000,
];

/// Atlas side in texels: 16 x 6 glyphs of 8 x 8.
const ATLAS: usize = 128;
const GLYPH: i16 = 8;

/// RGBA4444 atlas: opaque white where a glyph bit is set, clear elsewhere.
/// 64-byte aligned, as every buffer the GE reads must be.
#[repr(C, align(64))]
struct Atlas([u16; ATLAS * ATLAS]);
static mut ATLAS_TEXELS: Atlas = Atlas([0; ATLAS * ATLAS]);
static ATLAS_BUILT: AtomicBool = AtomicBool::new(false);

/// Queued blocks: origin, ABGR colour, and their bytes in [`Queue::text`].
const BLOCKS: usize = 16;
const TEXT: usize = 2048;

struct Queue {
    blocks: [(i16, i16, u32, u16, u16); BLOCKS],
    count: usize,
    text: [u8; TEXT],
    len: usize,
}

impl Queue {
    const fn empty() -> Self {
        Queue {
            blocks: [(0, 0, 0, 0, 0); BLOCKS],
            count: 0,
            text: [0; TEXT],
            len: 0,
        }
    }
}

static mut QUEUE: Queue = Queue::empty();

impl core::fmt::Write for Queue {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        // Past capacity is dropped: truncated diagnostics beat a panic.
        let room = TEXT - self.len;
        let n = s.len().min(room);
        self.text[self.len..self.len + n].copy_from_slice(&s.as_bytes()[..n]);
        self.len += n;
        Ok(())
    }
}

fn build_atlas() {
    if ATLAS_BUILT.load(Ordering::Relaxed) {
        return;
    }
    // SAFETY: the frame loop's thread is the only user; the GE reads the
    // atlas only after the write-back below.
    let texels = unsafe { &mut (*core::ptr::addr_of_mut!(ATLAS_TEXELS)).0 };
    for (c, glyph) in FONT.iter().enumerate() {
        let (cx, cy) = ((c % 16) * 8, (c / 16) * 8);
        for row in 0..8 {
            let bits = (glyph >> (8 * row)) as u8;
            for col in 0..8 {
                let on = bits & (1 << col) != 0;
                texels[(cy + row) * ATLAS + cx + col] = if on { 0xFFFF } else { 0 };
            }
        }
    }
    unsafe {
        sys::sceKernelDcacheWritebackRange(
            texels.as_ptr() as *const c_void,
            core::mem::size_of_val(texels) as u32,
        );
    }
    ATLAS_BUILT.store(true, Ordering::Relaxed);
}

impl Gpu {
    /// Queues debug text at screen pixel `(x, y)` for this frame; `\n`
    /// starts a new line under `x`. `color` is packed ABGR. Any number of
    /// calls per frame, up to 16 blocks and 2 KiB of text; the rest is
    /// dropped. Drawn by [`Gpu::end_frame`] over everything else, outside
    /// the scissor.
    pub fn debug_text(&mut self, x: i32, y: i32, color: u32, args: core::fmt::Arguments<'_>) {
        // SAFETY: the frame loop's thread is the only user.
        let q = unsafe { &mut *core::ptr::addr_of_mut!(QUEUE) };
        if q.count == BLOCKS {
            return;
        }
        let start = q.len;
        let _ = core::fmt::Write::write_fmt(q, args);
        q.blocks[q.count] = (
            x as i16,
            y as i16,
            color,
            start as u16,
            (q.len - start) as u16,
        );
        q.count += 1;
    }

    /// Draws and clears the queued text. Between `begin_frame` and the
    /// list's `sceGuFinish`.
    pub(super) fn draw_debug_text(&mut self) {
        // SAFETY: the frame loop's thread is the only user.
        let q = unsafe { &mut *core::ptr::addr_of_mut!(QUEUE) };
        if q.count == 0 {
            return;
        }
        build_atlas();
        let glyphs = q.text[..q.len].iter().filter(|&&b| b > b' ').count();
        if glyphs > 0 {
            unsafe {
                let verts = list_memory(glyphs * 2 * core::mem::size_of::<SpriteVertex>())
                    as *mut SpriteVertex;
                let mut n = 0;
                for &(x0, y0, color, start, len) in &q.blocks[..q.count] {
                    let (mut x, mut y) = (x0, y0);
                    for &b in &q.text[start as usize..(start + len) as usize] {
                        if b == b'\n' {
                            x = x0;
                            y += GLYPH;
                            continue;
                        }
                        // sceGuDebugFlush skipped a glyph not wholly on screen.
                        let shown = (0..480 - 7).contains(&x) && (0..272 - 7).contains(&y);
                        if b > b' ' && b < 0x80 && shown {
                            let u = f32::from((b - b' ') % 16 * 8);
                            let v = f32::from((b - b' ') / 16 * 8);
                            let corner = |u, v, x, y| SpriteVertex {
                                u,
                                v,
                                color,
                                x,
                                y,
                                z: 0,
                                _pad: 0,
                            };
                            verts.add(n).write(corner(u, v, x, y));
                            verts
                                .add(n + 1)
                                .write(corner(u + 8.0, v + 8.0, x + GLYPH, y + GLYPH));
                            n += 2;
                        }
                        x += GLYPH;
                    }
                }
                sys::sceGuScissor(0, 0, 480, 272);
                sys::sceGuDisable(GuState::DepthTest);
                sys::sceGuDisable(GuState::CullFace);
                sys::sceGuDisable(GuState::Lighting);
                sys::sceGuDisable(GuState::AlphaTest);
                sys::sceGuDisable(GuState::Fog);
                sys::sceGuEnable(GuState::Texture2D);
                sys::sceGuEnable(GuState::Blend);
                sys::sceGuBlendFunc(
                    sys::BlendOp::Add,
                    sys::BlendFactor::SrcAlpha,
                    sys::BlendFactor::OneMinusSrcAlpha,
                    0,
                    0,
                );
                sys::sceGuTexFlush();
                sys::sceGuTexMode(TexturePixelFormat::Psm4444, 0, 0, 0);
                sys::sceGuTexImage(
                    sys::MipmapLevel::None,
                    ATLAS as i32,
                    ATLAS as i32,
                    ATLAS as i32,
                    core::ptr::addr_of!(ATLAS_TEXELS) as *const c_void,
                );
                sys::sceGuTexFilter(sys::TextureFilter::Nearest, sys::TextureFilter::Nearest);
                sys::sceGuTexWrap(sys::GuTexWrapMode::Clamp, sys::GuTexWrapMode::Clamp);
                sys::sceGuTexFunc(
                    sys::TextureEffect::Modulate,
                    sys::TextureColorComponent::Rgba,
                );
                draw_array(
                    GuPrimitive::Sprites,
                    SpriteVertex::FORMAT,
                    n as i32,
                    core::ptr::null(),
                    verts as *const c_void,
                );
            }
        }
        q.count = 0;
        q.len = 0;
    }
}
