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

pub mod attack;
pub mod battle;
pub mod camera;
pub mod captain;
pub mod capture_kirby;
pub mod capture_yoshi;
pub mod collision;
pub mod combat;
pub mod costume;
pub mod dead;
pub mod fighter;
pub mod fighter_select;
pub mod grab;
pub mod ground;
pub mod hazard;
pub mod hurtbox;
pub mod item;
pub mod item_throw;
pub mod kirby;
pub mod kirby_copy;
pub mod link;
mod luigi;
pub mod map;
pub mod motion;
pub mod ness;
pub mod physics;
pub mod pikachu;
pub mod purin;
pub mod reaction;
pub mod rng;
pub mod samus;
pub mod shadow;
pub mod stage;
pub mod stage_select;
pub mod stale;
pub mod status;
pub mod weapon;
pub mod yoshi;
