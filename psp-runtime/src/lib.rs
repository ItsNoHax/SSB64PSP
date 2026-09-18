//! Shared PSP platform/rendering/runtime library.
//!
//! Joins `ssb-rom` (ROM/pack data) and `ssb-game` (portable gameplay) on real
//! PSP hardware. Consumed as a path dependency by `psp-asset-viewer` and
//! `psp-game`; must never be depended on by `ssb-engine`, `ssb-rom`, or
//! `ssb-game` themselves.
#![no_std]
