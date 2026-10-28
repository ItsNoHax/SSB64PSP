//! Shared PSP platform/rendering/runtime library.
//!
//! Joins `ssb-rom` (ROM/pack data) and `ssb-game` (portable gameplay) on real
//! PSP hardware. Consumed as a path dependency by `psp-asset-viewer` and
//! `psp-game`; must never be depended on by `ssb-engine`, `ssb-rom`, or
//! `ssb-game` themselves.
#![no_std]
// `psp::vfpu_asm!` expands to `asm!` for MIPS (`meshdraw`'s model matrix).
#![feature(asm_experimental_arch)]

extern crate alloc;

pub mod arena;
pub mod assets;
pub mod audio;
pub mod boss;
pub mod ending;
#[cfg(all(feature = "fcr31_probe", target_os = "psp"))]
pub mod fcr31_probe;
pub mod fpu;
pub mod frame_timer;
pub mod gu;
pub mod heap;
#[cfg(all(feature = "profile_hw", target_os = "psp"))]
pub mod hwcount;
pub mod input;
pub mod memory;
pub mod meshdraw;
pub mod movie;
pub mod particles;
pub mod profile;
pub mod savedata;
pub mod scene;
pub mod sample_stream;
pub mod scene_files;
#[cfg(feature = "sqrt_selftest")]
pub mod sqrt_selftest;
pub mod stack_poison;
#[cfg(feature = "texture_probe")]
pub mod texprobe;
pub mod thread;
pub mod timing;
pub mod transition;
#[cfg(feature = "trig_selftest")]
pub mod trig_selftest;
