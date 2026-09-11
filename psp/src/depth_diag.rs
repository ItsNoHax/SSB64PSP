//! `R2.2`/C3 measurement rig (RE-251).
//!
//! `meshdraw::apply_material` now toggles `sceGuDepthMask` per primitive from
//! the pack's `DEPTH_WRITE` bit (RE-244 through RE-250 built the data;
//! RE-251 wires it). Every ROM-sourced golden scene this project already
//! captures either carries no depth-test-without-write content in its frozen
//! camera window, or carries it without anything drawn afterward at an
//! overlapping screen position -- so the pixel a write-disabled surface
//! *doesn't* write is never actually read back by a later depth test in any
//! of them, and a regression here would pass every existing golden
//! unnoticed. This is a synthetic, non-ROM scene built specifically to read
//! that pixel back, the same `texgen_normal_diagnostic_*` shape
//! `normal_diag.rs` uses for a different GE contract this crate has no other
//! way to exercise.
//!
//! Three quads, drawn in this order, all overlapping the same screen region
//! at decreasing on-screen size so each is fully visible against the last:
//!
//! 1. **A**, opaque red, farthest (`z = -10`). Depth test on, write on --
//!    establishes a far depth-buffer value across the whole overlap region.
//! 2. **B**, translucent green (alpha 0.5), nearest (`z = -6`), blended.
//!    Depth test on, write **off** -- passes against A (nearer) and blends,
//!    but must not advance the depth buffer.
//! 3. **C**, opaque blue, *between* A and B in depth (`z = -8`), smallest.
//!    Depth test on, write on again -- the ON->OFF->ON switch.
//!
//! C is farther than B, so if B had wrongly written depth, C's test against
//! B's near value would fail and C would stay hidden behind B's blend. If
//! `sceGuDepthMask` correctly kept B from writing, the buffer still holds
//! A's far value when C tests, C passes, and its opaque blue shows through
//! *in front of* B on screen -- despite being submitted after B and lying
//! behind it in depth. That inversion is the whole point of a depth-test-
//! without-write surface (real-time translucency compositing, RE-244's
//! `ZMODE_XLU`) and is the one observable difference a broken
//! `sceGuDepthMask` wire would change: C would vanish, leaving only B's
//! blended green visible over the whole inner region instead of a solid
//! blue disc nested inside it.

use psp::sys::{self, BlendFactor, BlendOp, ClearBuffer, GuState};
use psp::Align16;

use crate::gu::{Gpu, GuVertex};

/// One quad's two triangles, `Align16` per the GE's DMA alignment
/// requirement (`normal_diag.rs`'s own doc comment names this hazard).
static mut QUAD: Align16<[GuVertex; 6]> = Align16([GuVertex::new(0.0, 0.0, 0.0, 0.0, 0.0, 0); 6]);

unsafe fn quad(half_extent: f32, z: f32, color: u32) -> &'static [GuVertex; 6] {
    QUAD.0 = [
        GuVertex::new(-half_extent, -half_extent, z, 0.0, 0.0, color),
        GuVertex::new(half_extent, -half_extent, z, 0.0, 0.0, color),
        GuVertex::new(half_extent, half_extent, z, 0.0, 0.0, color),
        GuVertex::new(-half_extent, -half_extent, z, 0.0, 0.0, color),
        GuVertex::new(half_extent, half_extent, z, 0.0, 0.0, color),
        GuVertex::new(-half_extent, half_extent, z, 0.0, 0.0, color),
    ];
    &*core::ptr::addr_of!(QUAD.0)
}

/// Draws the three-quad depth-mask proof described in the module doc.
///
/// # Safety
///
/// Must be called between [`Gpu::begin_frame`] and [`Gpu::end_frame`], with
/// the perspective and view matrices already set -- reuses whatever camera
/// the caller established, same convention as `normal_diag::draw`.
pub unsafe fn draw(gpu: &mut Gpu, _aspect: f32) {
    sys::sceGuClearColor(0xFF20_2020);
    sys::sceGuClear(ClearBuffer::COLOR_BUFFER_BIT | ClearBuffer::DEPTH_BUFFER_BIT);

    sys::sceGuDisable(GuState::Lighting);
    sys::sceGuDisable(GuState::CullFace);
    sys::sceGuEnable(GuState::DepthTest);
    gpu.model_transform([0.0, 0.0, 0.0], [0.0, 0.0, 0.0], 1.0);

    // A: opaque red, far, write on.
    sys::sceGuDisable(GuState::Blend);
    sys::sceGuDepthMask(0);
    gpu.draw_triangles(quad(4.0, -10.0, 0xFF00_00FF));

    // B: translucent green, near, write OFF.
    sys::sceGuEnable(GuState::Blend);
    sys::sceGuBlendFunc(
        BlendOp::Add,
        BlendFactor::SrcAlpha,
        BlendFactor::OneMinusSrcAlpha,
        0,
        0,
    );
    sys::sceGuDepthMask(1);
    gpu.draw_triangles(quad(1.6, -6.0, 0x8000_FF00));

    // C: opaque blue, between A and B in depth, write back ON -- the
    // ON->OFF->ON switch. Must appear as a solid disc nested inside B's
    // blended region if `sceGuDepthMask` left A's far depth untouched by B.
    sys::sceGuDisable(GuState::Blend);
    sys::sceGuDepthMask(0);
    gpu.draw_triangles(quad(0.8, -8.0, 0xFFFF_0000));
}
