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

pub mod audio;
pub mod coord;
pub mod input;
pub mod math;
pub mod renderer;
pub mod timing;

pub use math::{Mat4, Vec2, Vec3};
