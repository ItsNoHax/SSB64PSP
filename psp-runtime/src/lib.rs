//! Shared PSP platform/rendering/runtime library.
//!
//! Joins `ssb-rom` (ROM/pack data) and `ssb-game` (portable gameplay) on real
//! PSP hardware. Consumed as a path dependency by `psp-asset-viewer` and
//! `psp-game`; must never be depended on by `ssb-engine`, `ssb-rom`, or
//! `ssb-game` themselves.
#![no_std]

extern crate alloc;

pub mod assets;
pub mod audio;
pub mod boss;
pub mod ending;
pub mod gu;
pub mod input;
pub mod memory;
pub mod meshdraw;
pub mod memops;
pub mod movie;
pub mod particles;
pub mod profile;
pub mod savedata;
pub mod scene;
pub mod scene_files;
pub mod thread;
pub mod timing;
pub mod transition;
