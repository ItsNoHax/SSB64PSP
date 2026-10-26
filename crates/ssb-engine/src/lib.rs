//! Layer B: the boundary between game logic and hardware.
//!
//! Everything here is `no_std` and free of PSP types. Game code (Layer A) talks
//! to these traits; `psp/ssb64-psp` (Layer C) implements them with `sceGu`,
//! `sceCtrl`, `sceAudio` and friends. A host build can implement them too,
//! which is what makes headless physics tests possible.
//!
//! The rule this layer exists to enforce: **gameplay never mentions the PSP,
//! and never performs a coordinate conversion.** Simulation runs in the
//! original game's coordinate system end to end; the renderer converts on the
//! way out (see [`coord`]).

#![cfg_attr(not(feature = "std"), no_std)]
// The audio DSP uses Allegrex instructions through inline assembly on the
// PSP (`audio::dsp`); MIPS inline assembly is still unstable (the PSP build
// is nightly).
#![cfg_attr(target_arch = "mips", feature(asm_experimental_arch))]

// The audio system allocates its pools once at boot (D-049).
extern crate alloc;

pub mod array_vec;
pub mod audio;
pub mod coord;
/// The decomp's small `d*` data arrays, generated at build time from the
/// ROM by `ssb-tablegen` (`d_arrays`; D-048). Each constant is the symbol's
/// name without the `d`, in `UPPER_SNAKE`. The sine table's words are
/// ROM data, not approximations of the standard constants.
#[allow(clippy::approx_constant)]
mod d_arrays {
    include!(concat!(env!("OUT_DIR"), "/d_arrays.rs"));
}
#[cfg(test)]
mod fpu_traps;
pub mod guard_clip;
pub mod heap;
pub mod input;
pub mod math;
pub mod memory;
pub mod rdp_noise;
pub mod renderer;
pub mod rsp_light;
pub mod timing;

pub use math::{Mat4, Vec2, Vec3};
