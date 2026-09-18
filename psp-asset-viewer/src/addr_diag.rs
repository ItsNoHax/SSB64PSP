//! RE-277 follow-up measurement rig (RE-278).
//!
//! RE-277 swept `n64_addressing`'s hardware reference model against this
//! project's own `psp_lowering_axis` abstraction at every real texel
//! RE-272/274's Mario-face primitive reaches and found zero divergence --
//! but that was a host-side comparison of two *models*, not a check that
//! real GE hardware (or PPSSPP's software rasterizer) actually samples the
//! way either model predicts. This rig closes that gap directly: a synthetic
//! 64-texel-wide texture whose red channel *is* the source texel index it
//! was mirror-baked from (a plain 0..31..0 triangle wave, matching
//! `texture::mirror_extend`'s documented mirror+clamp bake for a 32-texel
//! period doubled to 64 -- RE-274's exact `mask_s=5, mirror_s=true,
//! clamp_s=true, drawn_width=64` axis), sampled at fixed, non-interpolated
//! texture coordinates through a `TRANSFORM_2D` sprite (identity scale/
//! offset, so the UV *is* a raw texel address, the same convention
//! `Gpu::draw_wallpaper_sprite`'s own doc comment establishes) so each
//! probe's readback pixel is exactly one GE texture-unit sample, not a
//! screen-space interpolation this rig would have to model separately.
//!
//! Two rows repeat the same [`PROBES`] under `TextureFilter::Nearest` and
//! `TextureFilter::Linear`: Nearest checks the raw clamp/wrap index
//! selection RE-277 already modeled; Linear additionally exercises the two
//! half-texel probes straddling the drawn rect's far edge (`63.5`/`64.5`),
//! which no prior measurement in this chain has touched -- if the GE blends
//! against a wrapped-around or garbage neighbour right at the clamp boundary
//! instead of holding the edge texel on both sides of it, that would produce
//! exactly RE-272's "repeating, aliased pattern" description without any
//! divergence in the discrete addressing formula RE-277 already cleared.
//!
//! Read the captured screenshot's row-40 and row-150 pixels at each probe's
//! `x` column: the red channel is the sampled source texel index directly
//! (`0..31`), comparable by eye or script against this module's own
//! doc-commented prediction for each probe.

use core::ffi::c_void;

use psp::sys::{
    self, ClearBuffer, GuPrimitive, GuState, GuTexWrapMode, MipmapLevel, TextureColorComponent,
    TextureEffect, TextureFilter, TexturePixelFormat, VertexType,
};
use psp::Align16;

use crate::gu::Gpu;

/// The mirror+clamp bake RE-221/RE-274 describe for a 32-texel period with
/// `mirror=true, clamp=true, drawn=64`: the first period unflipped (`0..31`),
/// the second period reversed (`31..0`) -- independently authored here
/// rather than calling `texture::mirror_extend` itself, so this rig checks
/// GE hardware against a known-correct reference, not against the same
/// function RE-274 already verified byte-correct with a PPM dump.
const BAKED_WIDTH: usize = 64;

#[repr(C)]
#[derive(Clone, Copy)]
struct AddrSpriteVertex {
    u: f32,
    v: f32,
    color: u32,
    x: i16,
    y: i16,
    z: i16,
    _pad: i16,
}

impl AddrSpriteVertex {
    const FORMAT: VertexType = VertexType::from_bits_truncate(
        VertexType::TEXTURE_32BITF.bits()
            | VertexType::COLOR_8888.bits()
            | VertexType::VERTEX_16BIT.bits()
            | VertexType::TRANSFORM_2D.bits(),
    );
}

static mut TEXTURE: Align16<[u8; BAKED_WIDTH * 4]> = Align16([0u8; BAKED_WIDTH * 4]);
static mut TEXTURE_READY: bool = false;

unsafe fn ensure_texture_built() {
    if TEXTURE_READY {
        return;
    }
    for i in 0..BAKED_WIDTH {
        let value = if i < 32 { i as u8 } else { (63 - i) as u8 };
        let o = i * 4;
        TEXTURE.0[o] = value;
        TEXTURE.0[o + 1] = 0;
        TEXTURE.0[o + 2] = 0;
        TEXTURE.0[o + 3] = 0xFF;
    }
    sys::sceKernelDcacheWritebackRange(
        core::ptr::addr_of!(TEXTURE.0) as *const c_void,
        TEXTURE.0.len() as u32,
    );
    TEXTURE_READY = true;
}

/// Probe coordinates, already relative to the tile origin -- RE-274's own
/// measured span (`-31..80` texels) plus two half-texel probes (`63.5`,
/// `64.5`) straddling the drawn rect's far edge (index 63, the last baked
/// pixel) for the `Linear`-only boundary-blend check this chain has not yet
/// run. Predicted `Nearest` reads (this rig's own independent
/// triangle-wave: `i` for `i<32`, `63-i` for `i>=32`, clamped to `0..=63`
/// first): `-31->0` (clamp low), `-1->0`, `0->0`, `5->5`, `31->31`, `32->31`,
/// `45->18`, `63->0`, `64->0` (clamp high, same held edge as 63), `70->0`,
/// `80->0`, `90->0`. `63.5`/`64.5` have no clean integer prediction under
/// `Nearest` (implementation-defined rounding); under `Linear` all four of
/// `63`, `63.5`, `64`, `64.5` should read the *same* value (0) if the GE
/// correctly holds the far-edge texel on both sides of the clamp boundary --
/// any of them reading something else is the signal this rig exists to
/// catch.
const PROBES: [f32; 14] = [
    -31.0, -1.0, 0.0, 5.0, 31.0, 32.0, 45.0, 63.0, 63.5, 64.0, 64.5, 70.0, 80.0, 90.0,
];

const SPRITE_W: i16 = 30;
const SPRITE_H: i16 = 60;
const GAP: i16 = 4;
const ROW_NEAREST_Y: i16 = 40;
const ROW_LINEAR_Y: i16 = 150;

unsafe fn draw_row(y0: i16, mag: TextureFilter, min: TextureFilter) {
    sys::sceGuTexFilter(mag, min);
    for (i, &texel) in PROBES.iter().enumerate() {
        let x0 = 4 + i as i16 * (SPRITE_W + GAP);
        let verts = [
            AddrSpriteVertex {
                u: texel,
                v: 0.0,
                color: 0xFFFF_FFFF,
                x: x0,
                y: y0,
                z: 0,
                _pad: 0,
            },
            AddrSpriteVertex {
                u: texel,
                v: 0.0,
                color: 0xFFFF_FFFF,
                x: x0 + SPRITE_W,
                y: y0 + SPRITE_H,
                z: 0,
                _pad: 0,
            },
        ];
        sys::sceGuDrawArray(
            GuPrimitive::Sprites,
            AddrSpriteVertex::FORMAT,
            2,
            core::ptr::null(),
            verts.as_ptr() as *const c_void,
        );
    }
}

/// Draws both probe rows as one full-frame replacement scene, the same
/// `texgen_normal_diagnostic_*` shape `normal_diag`/`depth_diag` use.
///
/// # Safety
///
/// Must be called between [`Gpu::begin_frame`] and [`Gpu::end_frame`].
pub unsafe fn draw(_gpu: &mut Gpu) {
    ensure_texture_built();

    sys::sceGuClearColor(0xFFFF_00FF); // opaque magenta: unmistakable if a sprite misses
    sys::sceGuClear(ClearBuffer::COLOR_BUFFER_BIT | ClearBuffer::DEPTH_BUFFER_BIT);

    sys::sceGuEnable(GuState::Texture2D);
    sys::sceGuDisable(GuState::Lighting);
    sys::sceGuDisable(GuState::DepthTest);
    sys::sceGuDisable(GuState::Blend);
    sys::sceGuDisable(GuState::CullFace);

    sys::sceGuTexMode(TexturePixelFormat::Psm8888, 0, 0, 0);
    sys::sceGuTexImage(
        MipmapLevel::None,
        BAKED_WIDTH as i32,
        1,
        BAKED_WIDTH as i32,
        core::ptr::addr_of!(TEXTURE.0) as *const c_void,
    );
    // Matches `t.wrap & CLAMP_S` for RE-274's own axis; T is unused (single
    // texel row) so its wrap mode is inert.
    sys::sceGuTexWrap(GuTexWrapMode::Clamp, GuTexWrapMode::Clamp);
    sys::sceGuTexFunc(TextureEffect::Replace, TextureColorComponent::Rgba);
    // Identity scale/offset: `u`/`v` are raw texel addresses, matching
    // `Gpu::draw_wallpaper_sprite`'s own established convention for
    // `TRANSFORM_2D` through-mode sprites.
    sys::sceGuTexScale(1.0, 1.0);
    sys::sceGuTexOffset(0.0, 0.0);

    draw_row(
        ROW_NEAREST_Y,
        TextureFilter::Nearest,
        TextureFilter::Nearest,
    );
    draw_row(ROW_LINEAR_Y, TextureFilter::Linear, TextureFilter::Linear);
}
