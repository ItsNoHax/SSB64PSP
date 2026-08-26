//! Layer A: Smash 64 game logic.
//!
//! Portable, `no_std`, and free of any PSP type. Everything here is derived
//! from the decompilation (`refs/ssb-decomp-re`) — see
//! `docs/ssb-architecture.md` for the mapping from the original subsystems to
//! these modules.
//!
//! Status: the physics and fighter-state scaffolding below is in place and
//! tested; per-character logic and the match loop are the next milestone
//! (M4). `docs/porting-status.md` tracks what is real versus stubbed.

#![cfg_attr(not(feature = "std"), no_std)]

pub mod fighter;
pub mod physics;
