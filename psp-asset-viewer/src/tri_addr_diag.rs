//! RE-278 follow-up measurement rig (RE-280): found a real `Ci4`/CLUT
//! nibble-order bug, not a triangle-interpolation bug.
//!
//! RE-278's `addr_diag_probe` proved fixed-coordinate GE sampling (no
//! screen-space interpolation, plain `Psm8888`) matches the addressing model
//! exactly. RE-279 separately confirmed RE-272/274's real primitive (file
//! 296, node 8) is correctly routed through `meshdraw`'s `SIGNED_CLAMP_UV`
//! float-UV path at pack time. This rig was built to test RE-278's two
//! remaining named candidates at once: **(a)** real per-pixel triangle
//! interpolation of a wide UV delta, and **(b)** the actual `Ci4`
//! (4-bit paletted, CLUT-indexed) bind path, rather than plain `Psm8888`.
//!
//! It found **(b)**, but not the shape expected. [`draw_fixed_probes`] --
//! *non*-interpolated, single-coordinate `Ci4` samples, the one case
//! `addr_diag_probe` never covered because it only bound `Psm8888` -- already
//! mismatches the addressing model on its own, with no interpolation
//! involved at all. The [`draw_quad`] Nearest row's interpolated readback
//! (screen columns 106..296) is not corrupted noise either: it is a clean,
//! periodic saw wave, `0,51,34,85,68,119,102,...` -- consistent with reading
//! the *wrong nibble of the right byte* for every other texel, not with
//! broken interpolation math. `draw_quad`'s `Psm8888` control row (same
//! interpolated geometry, same `baked_index` pattern, no CLUT) reads back as
//! a perfect linear ramp, ruling out interpolation precision entirely.
//!
//! Root cause: `ssb_rom::psp_texture::encode_level`'s `PsmT4` branch (and
//! `pack_indexed`'s straight ROM-copy path, and `pad_edge_repeat_nibbles`)
//! packed two 4-bit texels per byte "high nibble first, matching the N64
//! order" -- but PPSSPP's `PsmT4` texture reader (and, being unable to test
//! the two independently here, plausibly real PSP hardware too, since this
//! project already treats PPSSPP's software rasterizer as the deterministic
//! golden source) reads the *opposite* order: **low nibble is the first
//! (even-indexed) texel, high nibble is the second (odd-indexed) texel.**
//! Confirmed decisively: rebuilding [`TEXTURE`] with nibbles temporarily
//! swapped (`(lo << 4) | hi` instead of the then-shipping `(hi << 4) | lo`)
//! made every one of [`PROBES`]' fixed-sample reads exact (12/12) and turned
//! the interpolated Nearest row into the same clean ramp the `Psm8888`
//! control shows -- see RE-280's evidence record for the full before/after
//! readback table.
//!
//! **Fixed (RE-281).** `psp_texture.rs`'s three nibble-order sites now pack
//! low nibble first, matching the PSP GE. [`TEXTURE`] below packs the same
//! (now-correct) order permanently, so this rig now passes -- a standing
//! regression guard on the real GE's nibble-read behavior, independent of
//! `psp_texture.rs`'s own conversion path.
//!
//! This plausibly explained RE-272 directly: adjacent-texel-pair corruption
//! reads as exactly the "repeating, aliased pattern" RE-272 described, was
//! most visible on sharp high-contrast content (an eye/eyebrow outline) and
//! least visible on smoothly-shaded content (why no other fighter's face
//! triggered a visible complaint before), and was invisible to the
//! regression-capture goldens by construction -- both the real PSP path and
//! PPSSPP applied the identical wrong order, so they still agreed with *each
//! other* even though neither matched the source ROM texture. RE-281 applied
//! the fix, rebuilt the asset pack, and refreshed the affected goldens.

use core::ffi::c_void;

use psp::sys::{
    self, ClearBuffer, ClutPixelFormat, GuPrimitive, GuState, GuTexWrapMode, MipmapLevel,
    TextureColorComponent, TextureEffect, TextureFilter, TexturePixelFormat, VertexType,
};
use psp::Align16;

use ssb_psp_runtime::gu::Gpu;

/// RE-274's `drawn_width` for this exact mirror+clamp axis.
const BAKED_WIDTH: usize = 64;
const CLUT_ENTRIES: usize = 16;

#[repr(C)]
#[derive(Clone, Copy)]
struct TriVertex {
    u: f32,
    v: f32,
    color: u32,
    x: i16,
    y: i16,
    z: i16,
    _pad: i16,
}

impl TriVertex {
    const FORMAT: VertexType = VertexType::from_bits_truncate(
        VertexType::TEXTURE_32BITF.bits()
            | VertexType::COLOR_8888.bits()
            | VertexType::VERTEX_16BIT.bits()
            | VertexType::TRANSFORM_2D.bits(),
    );
}

/// `Ci4`, low nibble first (RE-280/RE-281, `psp_texture`'s corrected
/// convention): texel `2b` in byte `b`'s low nibble, texel `2b+1` in its
/// high nibble.
static mut TEXTURE: Align16<[u8; BAKED_WIDTH / 2]> = Align16([0u8; BAKED_WIDTH / 2]);
static mut CLUT: Align16<[u8; CLUT_ENTRIES * 4]> = Align16([0u8; CLUT_ENTRIES * 4]);
/// Control texture: the exact same `baked_index` pattern, same width, same
/// interpolated geometry -- but plain `Psm8888`, no CLUT indirection and no
/// nibble packing. If this row's readback is a clean ramp while the `Ci4`
/// rows above are not, the divergence is specific to the paletted/nibble
/// path, not to wide-delta interpolation in general.
static mut CONTROL_TEXTURE: Align16<[u8; BAKED_WIDTH * 4]> = Align16([0u8; BAKED_WIDTH * 4]);
static mut READY: bool = false;

/// Synthetic, independently-authored index pattern -- a period-16 sawtooth,
/// *not* a smooth ramp, so the sharpest possible index jump (`15 -> 0`) lands
/// inside the interpolated span at texels 31/32.
const fn baked_index(texel: usize) -> u8 {
    (texel % CLUT_ENTRIES) as u8
}

unsafe fn ensure_built() {
    if READY {
        return;
    }
    for b in 0..BAKED_WIDTH / 2 {
        let lo = baked_index(2 * b);
        let hi = baked_index(2 * b + 1);
        TEXTURE.0[b] = (hi << 4) | lo;
    }
    for i in 0..CLUT_ENTRIES {
        let o = i * 4;
        CLUT.0[o] = (i as u8).wrapping_mul(17);
        CLUT.0[o + 1] = 0;
        CLUT.0[o + 2] = 0;
        CLUT.0[o + 3] = 0xFF;
    }
    for i in 0..BAKED_WIDTH {
        let o = i * 4;
        CONTROL_TEXTURE.0[o] = baked_index(i).wrapping_mul(17);
        CONTROL_TEXTURE.0[o + 1] = 0;
        CONTROL_TEXTURE.0[o + 2] = 0;
        CONTROL_TEXTURE.0[o + 3] = 0xFF;
    }
    sys::sceKernelDcacheWritebackRange(
        core::ptr::addr_of!(TEXTURE.0) as *const c_void,
        TEXTURE.0.len() as u32,
    );
    sys::sceKernelDcacheWritebackRange(
        core::ptr::addr_of!(CLUT.0) as *const c_void,
        CLUT.0.len() as u32,
    );
    sys::sceKernelDcacheWritebackRange(
        core::ptr::addr_of!(CONTROL_TEXTURE.0) as *const c_void,
        CONTROL_TEXTURE.0.len() as u32,
    );
    READY = true;
}

/// Screen-column probes, in raw texel-address units -- RE-274's own measured
/// span (`-31..90`). Predicted red channel: `(clamp(addr, 0, 63) % 16) * 17`
/// (`Clamp` wrap on the S axis, matching `t.wrap & CLAMP_S` for this real
/// primitive). No half-texel probes here (those tested the clamp *boundary*
/// in RE-278) -- this rig's own reason to exist is the interpolated interior,
/// especially the `31 -> 32` index-wraparound step.
const PROBES: [f32; 12] = [
    -31.0, -1.0, 0.0, 5.0, 31.0, 32.0, 45.0, 63.0, 64.0, 70.0, 80.0, 90.0,
];

const LEFT_TEXEL: f32 = -31.0;
const RIGHT_TEXEL: f32 = 90.0;
const PX_PER_TEXEL: f32 = 3.0;
const X0: i16 = 10;
const QUAD_H: i16 = 40;
const ROW_NEAREST_Y: i16 = 20;
const ROW_LINEAR_Y: i16 = 100;
const ROW_CONTROL_Y: i16 = 180;
const ROW_FIXED_CI4_Y: i16 = 230;

const fn screen_x(texel: f32) -> i16 {
    (X0 as f32 + (texel - LEFT_TEXEL) * PX_PER_TEXEL) as i16
}

unsafe fn draw_quad(y0: i16, mag: TextureFilter, min: TextureFilter) {
    sys::sceGuTexFilter(mag, min);
    let x0 = screen_x(LEFT_TEXEL);
    let x1 = screen_x(RIGHT_TEXEL);
    let verts = [
        TriVertex {
            u: LEFT_TEXEL,
            v: 0.0,
            color: 0xFFFF_FFFF,
            x: x0,
            y: y0,
            z: 0,
            _pad: 0,
        },
        TriVertex {
            u: RIGHT_TEXEL,
            v: 0.0,
            color: 0xFFFF_FFFF,
            x: x1,
            y: y0,
            z: 0,
            _pad: 0,
        },
        TriVertex {
            u: RIGHT_TEXEL,
            v: 0.0,
            color: 0xFFFF_FFFF,
            x: x1,
            y: y0 + QUAD_H,
            z: 0,
            _pad: 0,
        },
        TriVertex {
            u: LEFT_TEXEL,
            v: 0.0,
            color: 0xFFFF_FFFF,
            x: x0,
            y: y0,
            z: 0,
            _pad: 0,
        },
        TriVertex {
            u: RIGHT_TEXEL,
            v: 0.0,
            color: 0xFFFF_FFFF,
            x: x1,
            y: y0 + QUAD_H,
            z: 0,
            _pad: 0,
        },
        TriVertex {
            u: LEFT_TEXEL,
            v: 0.0,
            color: 0xFFFF_FFFF,
            x: x0,
            y: y0 + QUAD_H,
            z: 0,
            _pad: 0,
        },
    ];
    sys::sceGuDrawArray(
        GuPrimitive::Triangles,
        TriVertex::FORMAT,
        6,
        core::ptr::null(),
        verts.as_ptr() as *const c_void,
    );
}

/// One fixed, non-interpolated sample per [`PROBES`] entry (both corners
/// share the same `u`, `addr_diag`'s own established convention) through
/// whatever texture/CLUT is currently bound. Distinguishes "`Ci4` addressing
/// is broken in general" from "`Ci4` interpolation across a wide per-triangle
/// delta is broken" -- the fixed probes in RE-278 never bound a `Ci4`
/// texture, only `Psm8888`.
unsafe fn draw_fixed_probes(y0: i16, mag: TextureFilter, min: TextureFilter) {
    sys::sceGuTexFilter(mag, min);
    for (i, &texel) in PROBES.iter().enumerate() {
        let x0 = 4 + i as i16 * 30;
        let verts = [
            TriVertex {
                u: texel,
                v: 0.0,
                color: 0xFFFF_FFFF,
                x: x0,
                y: y0,
                z: 0,
                _pad: 0,
            },
            TriVertex {
                u: texel,
                v: 0.0,
                color: 0xFFFF_FFFF,
                x: x0 + 26,
                y: y0,
                z: 0,
                _pad: 0,
            },
            TriVertex {
                u: texel,
                v: 0.0,
                color: 0xFFFF_FFFF,
                x: x0 + 26,
                y: y0 + 20,
                z: 0,
                _pad: 0,
            },
            TriVertex {
                u: texel,
                v: 0.0,
                color: 0xFFFF_FFFF,
                x: x0,
                y: y0,
                z: 0,
                _pad: 0,
            },
            TriVertex {
                u: texel,
                v: 0.0,
                color: 0xFFFF_FFFF,
                x: x0 + 26,
                y: y0 + 20,
                z: 0,
                _pad: 0,
            },
            TriVertex {
                u: texel,
                v: 0.0,
                color: 0xFFFF_FFFF,
                x: x0,
                y: y0 + 20,
                z: 0,
                _pad: 0,
            },
        ];
        sys::sceGuDrawArray(
            GuPrimitive::Triangles,
            TriVertex::FORMAT,
            6,
            core::ptr::null(),
            verts.as_ptr() as *const c_void,
        );
    }
}

/// Draws both filter rows as one full-frame replacement scene.
///
/// # Safety
///
/// Must be called between [`Gpu::begin_frame`] and [`Gpu::end_frame`].
pub unsafe fn draw(_gpu: &mut Gpu) {
    ensure_built();

    sys::sceGuClearColor(0xFFFF_00FF); // opaque magenta: unmistakable if a triangle misses
    sys::sceGuClear(ClearBuffer::COLOR_BUFFER_BIT | ClearBuffer::DEPTH_BUFFER_BIT);

    sys::sceGuEnable(GuState::Texture2D);
    sys::sceGuDisable(GuState::Lighting);
    sys::sceGuDisable(GuState::DepthTest);
    sys::sceGuDisable(GuState::Blend);
    sys::sceGuDisable(GuState::CullFace);

    // Same CLUT setup `bind_texture` uses for a real `Ci4` primitive: 0x0F
    // mask (4-bit index width), `Psm8888` CLUT entries, blocks of 8.
    sys::sceGuClutMode(ClutPixelFormat::Psm8888, 0, 0x0F, 0);
    let blocks = (CLUT_ENTRIES as i32 + 7) / 8;
    sys::sceGuClutLoad(blocks, core::ptr::addr_of!(CLUT.0) as *const c_void);

    sys::sceGuTexMode(TexturePixelFormat::PsmT4, 0, 0, 0);
    sys::sceGuTexImage(
        MipmapLevel::None,
        BAKED_WIDTH as i32,
        1,
        BAKED_WIDTH as i32,
        core::ptr::addr_of!(TEXTURE.0) as *const c_void,
    );
    // Matches `t.wrap & CLAMP_S` for RE-274's own axis.
    sys::sceGuTexWrap(GuTexWrapMode::Clamp, GuTexWrapMode::Clamp);
    sys::sceGuTexFunc(TextureEffect::Replace, TextureColorComponent::Rgba);
    // Identity scale/offset: `u`/`v` are raw texel addresses, matching
    // `addr_diag`'s own established convention for `TRANSFORM_2D`
    // through-mode primitives.
    sys::sceGuTexScale(1.0, 1.0);
    sys::sceGuTexOffset(0.0, 0.0);

    draw_fixed_probes(
        ROW_FIXED_CI4_Y,
        TextureFilter::Nearest,
        TextureFilter::Nearest,
    );

    draw_quad(
        ROW_NEAREST_Y,
        TextureFilter::Nearest,
        TextureFilter::Nearest,
    );
    draw_quad(ROW_LINEAR_Y, TextureFilter::Linear, TextureFilter::Linear);

    // Control row: same `baked_index` pattern, same interpolated geometry,
    // plain `Psm8888` -- no CLUT, no nibble packing. Isolates whether any
    // divergence above is specific to the paletted/nibble path.
    sys::sceGuTexMode(TexturePixelFormat::Psm8888, 0, 0, 0);
    sys::sceGuTexImage(
        MipmapLevel::None,
        BAKED_WIDTH as i32,
        1,
        BAKED_WIDTH as i32,
        core::ptr::addr_of!(CONTROL_TEXTURE.0) as *const c_void,
    );
    sys::sceGuTexWrap(GuTexWrapMode::Clamp, GuTexWrapMode::Clamp);
    draw_quad(
        ROW_CONTROL_Y,
        TextureFilter::Nearest,
        TextureFilter::Nearest,
    );
}

/// Exposed for the evidence record's own readback script to recompute
/// predictions without duplicating the formula by hand.
#[allow(dead_code)]
pub fn predicted_red(texel: f32) -> u8 {
    let clamped = texel.clamp(0.0, (BAKED_WIDTH - 1) as f32);
    baked_index(clamped as usize).wrapping_mul(17)
}

#[allow(dead_code)]
pub const fn probe_screen_x(texel: f32) -> i16 {
    screen_x(texel)
}

#[allow(dead_code)]
pub const PROBE_LIST: [f32; 12] = PROBES;
