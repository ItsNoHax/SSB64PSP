//! R2.1/T2 measurement rig (RE-226).
//!
//! `PLAN.md`'s T2 needs to know two things about `GU_NORMAL_8BIT` before
//! `meshdraw::apply_texture_mapping` can switch off `NormalizedNormal`:
//! whether the GE actually normalizes the quantized normal before feeding it
//! to the texture-matrix generator (it should not -- the RSP's own texgen
//! never does), and what divisor turns the raw signed byte into the [-1, 1]
//! range the generator matrix expects. `docs/reverse-engineering.md` RE-226
//! records the PPSSPP-source half of that (`GPU/Common/VertexReader.h`'s
//! `ReadNrm` divides every `GE_PROJMAP_NORMAL` axis by 128.0, unconditionally,
//! independent of the `GE_PROJMAP_NORMALIZED_NORMAL` case a few lines above
//! it) -- this module is the other half: driving the real `sceGu` API this
//! project's own renderer calls, through `tools/run-ppsspp-headless.sh`, so
//! the measurement exercises this project's actual code path rather than
//! trusting the PPSSPP source reading alone (`PPSSPP is not physical PSP
//! proof`, `AGENTS.md`).
//!
//! One quad, filling the whole viewport, replaces the entire normal scene
//! (see the `texgen_normal_diagnostic_*` feature block in `psp/Cargo.toml`
//! for exactly which case each build picks). Every vertex shares the same
//! hand-picked `GU_NORMAL_8BIT` value, so the texture-matrix generator's
//! output is constant across the quad; a 256x256 "coordinate ramp" texture
//! (`R(x, y) = x`, `G(x, y) = y`, point-sampled, no filtering) turns whatever
//! texel the GE actually addresses into a screenshot pixel that decodes back
//! to the exact `(s, t)` the hardware computed -- read the top-left output
//! pixel and the case's own predicted values are directly comparable, no
//! calibration or camera math needed.
//!
//! Four real bugs surfaced building this rig, each worth naming since nothing
//! else in this crate had exercised the combination before:
//!
//! 1. **Vertex data must outlive the caller's `sceGuSync`, not just the draw
//!    call.** `sceGumDrawArray` only enqueues a pointer into the display
//!    list; the GE does not read it until `end_frame`'s `sceGuSync`, by which
//!    point a stack-local array's frame would already be popped.
//!    `meshdraw`'s immediate-quad helpers dodge this by taking the array as a
//!    caller-owned `&mut` parameter backed by a local in `run`'s own frame;
//!    this module has no such per-frame caller state, so [`VERTS`] is
//!    `static` instead.
//! 2. **The GE DMAs texture and vertex data and needs 16-byte-aligned
//!    pointers.** `assets.rs`'s own doc comment names this exact hazard for
//!    the asset pack ("unaligned data renders garbage silently rather than
//!    failing") -- `alloc::vec::Vec<u8>` does not guarantee that alignment on
//!    this target, so [`RAMP`] is a plain `Align16`-wrapped static instead of
//!    a heap buffer.
//! 3. **The GE's guard-band clip discards a whole primitive outright once its
//!    unclipped extent is too far outside the frustum, rather than clipping
//!    it down.** A first attempt at "one quad, comfortably bigger than the
//!    frustum at any distance" used a fixed half-extent of 50 and got
//!    silently discarded; the quad below is sized to just barely overfill the
//!    38-degree viewport at its fixed draw distance instead.
//! 4. **The texture-matrix generator's `(s, t)` output is `[0, 1]`-normalized
//!    on this `TRANSFORM_3D` path, not a raw texel address**, contrary to
//!    `meshdraw::draw_wallpaper_sprite`'s own doc comment about identity
//!    scale/offset meaning "raw texel address" for its `TRANSFORM_2D`
//!    through-mode sprite -- that rule is specific to the through-mode path
//!    and does not carry over here. See `draw`'s own `A`/`B` comment for the
//!    plain-`TextureCoords` control draw that pinned this down.

use core::ffi::c_void;

use psp::sys::{
    self, ClearBuffer, GuPrimitive, GuState, GuTexWrapMode, MipmapLevel, ScePspFMatrix4,
    ScePspFVector4, TextureColorComponent, TextureEffect, TextureFilter, TextureMapMode,
    TexturePixelFormat, TextureProjectionMapMode, VertexType,
};
use psp::Align16;

use crate::gu::Gpu;

const RAMP_DIM: usize = 256;

/// Vertex layout for [`draw`]: `GU_NORMAL_8BIT | GU_VERTEX_32BITF`. The 8-bit
/// normal is padded to 4 bytes so the following 32-bit-float position stays
/// naturally aligned -- the same rule the PSP GE's own fixed vertex layout
/// applies to every format combination (confirmed against PPSSPP's decoder:
/// `GPU/Common/VertexDecoderCommon.cpp`'s `posalign[3] == 4` pads whatever
/// preceded a float position up to a 4-byte boundary).
#[repr(C, align(4))]
#[derive(Clone, Copy)]
struct DiagVertex {
    nx: i8,
    ny: i8,
    nz: i8,
    _pad: i8,
    x: f32,
    y: f32,
    z: f32,
}

const VERTEX_FORMAT: VertexType = VertexType::from_bits_truncate(
    VertexType::NORMAL_8BIT.bits()
        | VertexType::VERTEX_32BITF.bits()
        | VertexType::TRANSFORM_3D.bits(),
);

/// See module doc point 1: owned here, not local to [`draw`], so it stays
/// live across the caller's `sceGuSync`. `Align16` per point 2.
static mut VERTS: Align16<[DiagVertex; 6]> = Align16(
    [DiagVertex {
        nx: 0,
        ny: 0,
        nz: 0,
        _pad: 0,
        x: 0.0,
        y: 0.0,
        z: 0.0,
    }; 6],
);

/// The 256x256 `Psm8888` coordinate-ramp texture (`R(x, y) = x`,
/// `G(x, y) = y`), filled once on first [`draw`]. `Align16` per module doc
/// point 2; a plain `alloc::vec::Vec<u8>` does not carry that guarantee here.
static mut RAMP: Align16<[u8; RAMP_DIM * RAMP_DIM * 4]> = Align16([0u8; RAMP_DIM * RAMP_DIM * 4]);
static mut RAMP_READY: bool = false;

/// One measurement case: the raw signed-byte normal to submit, and which
/// `sceGuTexProjMapMode` to generate texture coordinates from it.
struct Case {
    normal: [i8; 3],
    /// `false` selects the previously-shipped `NormalizedNormal`; `true`
    /// selects the raw, unnormalized `Normal` this task is evaluating.
    raw: bool,
}

const CASES: [Case; 7] = [
    Case {
        normal: [127, 0, 0],
        raw: false,
    },
    Case {
        normal: [127, 0, 0],
        raw: true,
    },
    Case {
        normal: [64, 0, 0],
        raw: false,
    },
    Case {
        normal: [64, 0, 0],
        raw: true,
    },
    Case {
        normal: [-128, 0, 0],
        raw: true,
    },
    Case {
        normal: [90, 90, 0],
        raw: true,
    },
    Case {
        normal: [73, -41, 99],
        raw: true,
    },
];

const fn active_case() -> usize {
    if cfg!(feature = "texgen_normal_diagnostic_0") {
        0
    } else if cfg!(feature = "texgen_normal_diagnostic_1") {
        1
    } else if cfg!(feature = "texgen_normal_diagnostic_2") {
        2
    } else if cfg!(feature = "texgen_normal_diagnostic_3") {
        3
    } else if cfg!(feature = "texgen_normal_diagnostic_4") {
        4
    } else if cfg!(feature = "texgen_normal_diagnostic_5") {
        5
    } else {
        6
    }
}

/// Fills [`RAMP`] and flushes it from the CPU data cache: the GE reads
/// system memory by DMA and never sees the CPU's write-back cache, the same
/// hazard `assets::PackBuf::flush_cache`'s own doc comment names for the
/// asset pack.
unsafe fn ensure_ramp_built() {
    if RAMP_READY {
        return;
    }
    for y in 0..RAMP_DIM {
        for x in 0..RAMP_DIM {
            let i = (y * RAMP_DIM + x) * 4;
            RAMP.0[i] = x as u8;
            RAMP.0[i + 1] = y as u8;
            RAMP.0[i + 2] = 0;
            RAMP.0[i + 3] = 0xFF;
        }
    }
    sys::sceKernelDcacheWritebackRange(
        core::ptr::addr_of!(RAMP.0) as *const c_void,
        RAMP.0.len() as u32,
    );
    RAMP_READY = true;
}

/// Draws the active [`CASES`] entry as one full-viewport quad.
///
/// # Safety
///
/// Must be called between [`Gpu::begin_frame`] and [`Gpu::end_frame`], with
/// the perspective and view matrices already set (this reuses whatever
/// camera the caller established, only replacing the model matrix, texture
/// state and geometry).
pub unsafe fn draw(gpu: &mut Gpu, aspect: f32) {
    ensure_ramp_built();
    let case = &CASES[active_case()];

    sys::sceGuClearColor(0xFFFF_00FF); // opaque magenta: unmistakable if the quad misses the viewport
    sys::sceGuClear(ClearBuffer::COLOR_BUFFER_BIT | ClearBuffer::DEPTH_BUFFER_BIT);

    sys::sceGuEnable(GuState::Texture2D);
    sys::sceGuDisable(GuState::Lighting);
    sys::sceGuDisable(GuState::DepthTest);
    sys::sceGuDisable(GuState::Blend);
    sys::sceGuDisable(GuState::CullFace);

    sys::sceGuTexMode(TexturePixelFormat::Psm8888, 0, 0, 0);
    sys::sceGuTexImage(
        MipmapLevel::None,
        RAMP_DIM as i32,
        RAMP_DIM as i32,
        RAMP_DIM as i32,
        core::ptr::addr_of!(RAMP.0) as *const c_void,
    );
    // Point sampling: the payload is a per-texel address readout, not an
    // image, so any blending between texels would corrupt the reading.
    sys::sceGuTexFilter(TextureFilter::Nearest, TextureFilter::Nearest);
    sys::sceGuTexWrap(GuTexWrapMode::Clamp, GuTexWrapMode::Clamp);
    sys::sceGuTexFunc(TextureEffect::Replace, TextureColorComponent::Rgba);
    // Neutralise whatever scale/offset the object-view's own texgen draws
    // left set from earlier in this same frame (`apply_texture_mapping`
    // always sets both explicitly for exactly this reason -- residual state
    // otherwise silently rescales this rig's own texture-matrix output).
    sys::sceGuTexScale(1.0, 1.0);
    sys::sceGuTexOffset(0.0, 0.0);

    // `a`/`b` map the generator's [-1, 1]-ish output to a *normalized*
    // texture coordinate roughly in the middle third of the ramp --
    // comfortably clear of both edges for every case above, including
    // [90,90,0]'s magnitude-1.27 direction and the un-normalized
    // [73,-41,99] whose largest single axis is 99/128. Confirmed empirically
    // (not assumed): a same-pipeline plain-`TextureCoords` control draw with
    // vertex UV `(0.5, 0.25)` against this same texture read back texel
    // `(128, 64)` exactly -- `[0, 1]`-normalized, `* textureDimension` at
    // sample time, the ordinary GL-style convention. A first attempt used
    // `A = 90, B = 128` on the theory that identity scale/offset means "raw
    // texel address" (true for `meshdraw::draw_wallpaper_sprite`'s
    // `TRANSFORM_2D` through-mode sprite, per that function's own doc
    // comment) and got a uniform out-of-range clamp on every case -- that
    // rule does not carry over to this `TRANSFORM_3D` generator path.
    const A: f32 = 90.0 / RAMP_DIM as f32;
    const B: f32 = 128.0 / RAMP_DIM as f32;
    // Row convention: `result = source (row vector) * M`, `M`'s named
    // fields are its *rows* -- confirmed against `meshdraw::
    // apply_texture_mapping`'s own already-working texgen matrix, whose
    // `column(i)` closure builds exactly this shape (row `i` holds what
    // source axis `i` contributes to `(s, t)`).
    sys::sceGuSetMatrix(
        sys::MatrixMode::Texture,
        &ScePspFMatrix4 {
            x: ScePspFVector4 {
                x: A,
                y: 0.0,
                z: 0.0,
                w: 0.0,
            },
            y: ScePspFVector4 {
                x: 0.0,
                y: A,
                z: 0.0,
                w: 0.0,
            },
            z: ScePspFVector4 {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                w: 0.0,
            },
            w: ScePspFVector4 {
                x: B,
                y: B,
                z: 1.0,
                w: 1.0,
            },
        },
    );
    sys::sceGuTexMapMode(TextureMapMode::TextureMatrix, 0, 0);
    sys::sceGuTexProjMapMode(if case.raw {
        TextureProjectionMapMode::Normal
    } else {
        TextureProjectionMapMode::NormalizedNormal
    });

    // One quad, just barely overfilling the 38-degree-vertical viewport at
    // distance `d` (see module doc point 3 for why "just barely" and not
    // "wildly"). `TAN_HALF_FOVY` is `tan(19 deg)`, precomputed: this
    // `no_std` crate has no `libm`, so no runtime `tan` is available to
    // derive it from the 38 degree FOV `set_perspective` already uses.
    const TAN_HALF_FOVY: f32 = 0.344_327_6;
    const D: f32 = 8.0;
    let half_h = D * TAN_HALF_FOVY * 1.5;
    let half_w = half_h * aspect;
    let quad = [
        (-half_w, -half_h),
        (half_w, -half_h),
        (half_w, half_h),
        (-half_w, -half_h),
        (half_w, half_h),
        (-half_w, half_h),
    ];
    for (i, (x, y)) in quad.into_iter().enumerate() {
        VERTS.0[i] = DiagVertex {
            nx: case.normal[0],
            ny: case.normal[1],
            nz: case.normal[2],
            _pad: 0,
            x,
            y,
            z: -D,
        };
    }

    gpu.model_transform([0.0, 0.0, 0.0], [0.0, 0.0, 0.0], 1.0);
    sys::sceGumDrawArray(
        GuPrimitive::Triangles,
        VERTEX_FORMAT,
        6,
        core::ptr::null(),
        core::ptr::addr_of!(VERTS.0) as *const c_void,
    );
}
