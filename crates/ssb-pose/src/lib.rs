//! The fighter tick both hosts run: the skeleton's clip, the joint samples
//! gameplay reads, and the weapon placements those samples feed.
//!
//! `ssb-game` must not know the pack and `ssb-rom` must not know gameplay,
//! so the code that joins a [`ssb_game::fighter::Fighter`] to its
//! [`ssb_rom::skeleton::Skeleton`] lives here, depending on both. The PSP
//! runtime's `FighterScene` and the host replays (`ssb-rom`'s
//! `tests/common/posed.rs`) run this one copy (D-052). Platform concerns
//! (profiler spans, stack poisoning, the overlay's airborne count, the
//! camera, display) stay with the PSP runtime: they reach the physics half
//! through [`PhysicsHooks`].
//!
//! No PSP code; `no_std` without the `std` feature, like its dependencies.

#![cfg_attr(not(feature = "std"), no_std)]

pub mod joints;
pub mod posed;
pub mod quake;
pub mod skeleton_tick;

pub use joints::{part_locals_into, sample_demo_joints, set_joint_parents, set_posed_joints};
pub use posed::{PhysicsHooks, PhysicsSpan, PosedFighter, Snapshot};
pub use quake::quake_translate;
pub use skeleton_tick::{fighter_skeleton, tick_skeleton_animation};
