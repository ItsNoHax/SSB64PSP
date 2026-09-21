//! Deterministic PSP GE texture-sample alignment probe (RE-304).
//!
//! Each tile is a fixed-coordinate sample of a synthetic 4x4 RGBA8888
//! texture (plus a separate 2x2 and pre-baked mirrored 2x2 case). R/G encode the source texel's x/y index as `index * 64`, so a
//! capture can recover both bilinear axes independently.  Columns exercise
//! N64 S10.5 texel centres, half-texel positions, +/-1/32 around boundaries,
//! diagonals, and both sides of the texture boundary.  Rows are:
//! point/no bias, linear/no bias, linear/+0.5 texel, linear/+0.5 repeat,
//! 2x2 linear/+0.5 clamp, and pre-baked 2x2 mirror/+0.5 repeat.

use core::ffi::c_void;

use psp::sys::{
    self, ClearBuffer, GuPrimitive, GuState, GuTexWrapMode, MipmapLevel, TextureColorComponent,
    TextureEffect, TextureFilter, TexturePixelFormat, VertexType,
};
use psp::Align16;

use ssb_psp_runtime::gu::Gpu;

const DIM: usize = 4;
// PSP GE texture-buffer width is encoded in 16-pixel units even when the
// logical texture is smaller.  PPSSPP tolerates `tbw=4`; real hardware does
// not, so keep the synthetic logical 4x4 image in a valid 16-pixel stride.
const STRIDE: usize = 16;
const CELL_W: i16 = 24;
const CELL_H: i16 = 30;
const ROW_Y: [i16; 6] = [4, 44, 84, 124, 164, 204];

// Raw N64 S10.5 coordinates.  These deliberately include axis and diagonal
// boundary probes; the last four cross the 0/4 clamp-or-repeat boundary.
pub const PROBES: [(i16, i16); 20] = [
    (0, 0),
    (16, 0),
    (31, 0),
    (32, 0),
    (33, 0),
    (15, 16),
    (16, 16),
    (17, 16),
    (31, 31),
    (32, 32),
    (33, 33),
    (47, 48),
    (48, 48),
    (49, 48),
    (95, 95),
    (-1, 48),
    (0, 48),
    (127, 48),
    (128, 48),
    (129, 48),
];

#[repr(C)]
#[derive(Clone, Copy)]
struct Vertex {
    u: f32,
    v: f32,
    color: u32,
    x: f32,
    y: f32,
    z: f32,
}

impl Vertex {
    const FORMAT: VertexType = VertexType::from_bits_truncate(
        VertexType::TEXTURE_32BITF.bits()
            | VertexType::COLOR_8888.bits()
            | VertexType::VERTEX_32BITF.bits()
            | VertexType::TRANSFORM_2D.bits(),
    );
}

static mut TEXTURE: Align16<[u8; STRIDE * DIM * 4]> = Align16([0; STRIDE * DIM * 4]);
static mut TEXTURE_2X2: Align16<[u8; STRIDE * 2 * 4]> = Align16([0; STRIDE * 2 * 4]);
static mut TEXTURE_MIRROR: Align16<[u8; STRIDE * 2 * 4]> = Align16([0; STRIDE * 2 * 4]);
static mut READY: bool = false;

unsafe fn ensure_texture() {
    if READY {
        return;
    }
    for y in 0..DIM {
        for x in 0..DIM {
            let o = (y * STRIDE + x) * 4;
            TEXTURE.0[o] = (x * 64) as u8;
            TEXTURE.0[o + 1] = (y * 64) as u8;
            TEXTURE.0[o + 2] = ((x ^ y) * 48) as u8;
            TEXTURE.0[o + 3] = 255;
        }
    }
    for y in 0..2 {
        for x in 0..2 {
            let src = (y * STRIDE + x) * 4;
            let rgba = [TEXTURE.0[src], TEXTURE.0[src + 1], TEXTURE.0[src + 2], 255];
            let o = (y * STRIDE + x) * 4;
            TEXTURE_2X2.0[o..o + 4].copy_from_slice(&rgba);
            for mx in [x, 3 - x] {
                let mo = (y * STRIDE + mx) * 4;
                TEXTURE_MIRROR.0[mo..mo + 4].copy_from_slice(&rgba);
            }
        }
    }
    sys::sceKernelDcacheWritebackRange(
        core::ptr::addr_of!(TEXTURE.0) as *const c_void,
        TEXTURE.0.len() as u32,
    );
    sys::sceKernelDcacheWritebackRange(
        core::ptr::addr_of!(TEXTURE_2X2.0) as *const c_void,
        TEXTURE_2X2.0.len() as u32,
    );
    sys::sceKernelDcacheWritebackRange(
        core::ptr::addr_of!(TEXTURE_MIRROR.0) as *const c_void,
        TEXTURE_MIRROR.0.len() as u32,
    );
    READY = true;
}

unsafe fn draw_row(y: i16, linear: bool, bias: f32, repeat: bool) {
    if linear {
        sys::sceGuTexFilter(TextureFilter::Linear, TextureFilter::Linear);
    } else {
        sys::sceGuTexFilter(TextureFilter::Nearest, TextureFilter::Nearest);
    }
    if repeat {
        sys::sceGuTexWrap(GuTexWrapMode::Repeat, GuTexWrapMode::Repeat);
    } else {
        sys::sceGuTexWrap(GuTexWrapMode::Clamp, GuTexWrapMode::Clamp);
    }
    for (i, &(s, t)) in PROBES.iter().enumerate() {
        let x = i as i16 * CELL_W;
        let u = s as f32 / 32.0 + bias;
        let v = t as f32 / 32.0 + bias;
        let verts = [
            Vertex {
                u,
                v,
                color: 0xFFFF_FFFF,
                x: x as f32,
                y: y as f32,
                z: 0.0,
            },
            Vertex {
                u,
                v,
                color: 0xFFFF_FFFF,
                x: (x + CELL_W - 2) as f32,
                y: (y + CELL_H) as f32,
                z: 0.0,
            },
        ];
        // Keep transient vertices in the display-list arena.  Stack memory is
        // coherent in PPSSPP but is cached on real PSP hardware and may be
        // overwritten before the asynchronous GE consumes it.
        let dynamic = sys::sceGuGetMemory(core::mem::size_of_val(&verts) as i32) as *mut Vertex;
        core::ptr::copy_nonoverlapping(verts.as_ptr(), dynamic, verts.len());
        sys::sceGuDrawArray(
            GuPrimitive::Sprites,
            Vertex::FORMAT,
            2,
            core::ptr::null(),
            dynamic as *const c_void,
        );
    }
}

/// Draws the complete probe as a full-frame replacement scene.
pub unsafe fn draw(_gpu: &mut Gpu) {
    ensure_texture();
    sys::sceGuClearColor(0xFFFF_00FF);
    sys::sceGuClear(ClearBuffer::COLOR_BUFFER_BIT | ClearBuffer::DEPTH_BUFFER_BIT);
    sys::sceGuEnable(GuState::Texture2D);
    sys::sceGuDisable(GuState::Lighting);
    sys::sceGuDisable(GuState::DepthTest);
    sys::sceGuDisable(GuState::Blend);
    sys::sceGuDisable(GuState::CullFace);
    sys::sceGuTexMode(TexturePixelFormat::Psm8888, 0, 0, 0);
    sys::sceGuTexImage(
        MipmapLevel::None,
        DIM as i32,
        DIM as i32,
        STRIDE as i32,
        core::ptr::addr_of!(TEXTURE.0) as *const c_void,
    );
    sys::sceGuTexFunc(TextureEffect::Replace, TextureColorComponent::Rgba);
    sys::sceGuTexScale(1.0, 1.0);
    sys::sceGuTexOffset(0.0, 0.0);

    draw_row(ROW_Y[0], false, 0.0, false);
    draw_row(ROW_Y[1], true, 0.0, false);
    draw_row(ROW_Y[2], true, 0.5, false);
    draw_row(ROW_Y[3], true, 0.5, true);

    sys::sceGuTexImage(
        MipmapLevel::None,
        2,
        2,
        STRIDE as i32,
        core::ptr::addr_of!(TEXTURE_2X2.0) as *const c_void,
    );
    draw_row(ROW_Y[4], true, 0.5, false);
    sys::sceGuTexImage(
        MipmapLevel::None,
        4,
        2,
        STRIDE as i32,
        core::ptr::addr_of!(TEXTURE_MIRROR.0) as *const c_void,
    );
    draw_row(ROW_Y[5], true, 0.5, true);
}

pub const fn probe_x(index: usize) -> i16 {
    index as i16 * CELL_W + CELL_W / 2
}
pub const ROW_CENTRES: [i16; 6] = [19, 59, 99, 139, 179, 219];
