//! Layer A: Smash 64 game logic.
//!
//! Portable, `no_std`, and free of any PSP type. Everything here is derived
//! from the decompilation (`refs/ssb-decomp-re`) — see
//! `docs/architecture.md` for the mapping from the original subsystems to
//! these modules.
//!
//! `PLAN.md` tracks what is ported and what remains.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod appear;
pub mod attack;
pub mod auto_demo;
pub mod backup;
pub mod battle;
pub mod boss;
pub mod camera;
pub mod captain;
pub mod capture_kirby;
pub mod capture_yoshi;
pub mod colanim;
pub mod collision;
pub mod combat;
pub mod computer;
pub mod costume;
pub mod countdown;
pub mod dead;
pub mod dokan;
pub mod effect;
pub mod explain;
pub mod fighter;
pub mod fighter_select;
pub mod fighter_sound;
pub mod fteffect;
pub mod grab;
pub mod ground;
pub mod hazard;
pub mod hud;
pub mod hurtbox;
pub mod item;
pub mod item_sounds;
pub mod item_throw;
pub mod item_use;
#[cfg(test)]
mod item_use_tests;
pub mod key;
pub mod kirby;
pub mod kirby_copy;
pub mod ko;
pub mod link;
mod luigi;
pub mod map;
pub mod menu;
pub mod modelpart;
pub mod monster_weapon;
pub mod motion;
pub mod music;

pub mod ness;
pub mod opening;
pub mod particle;
pub mod pause;
pub mod physics;
pub mod pikachu;
pub mod player_interface;
pub mod players_1p;
pub mod players_1p_bonus;
pub mod players_vs;
pub mod public;
pub mod purin;
pub mod reaction;
pub mod results;
pub mod results_emblem;
pub mod results_layer;
pub mod results_scene;
pub mod rng;
pub mod samus;
#[cfg(test)]
mod setter_tests;
pub mod shadow;
pub mod sound;
mod special_fighters;
pub mod spgame;
pub mod stage;
pub mod stage_select;
pub mod stage_select_layer;
pub mod stale;
pub mod status;
pub mod team;
pub mod thrown;
pub mod training;
pub mod training_layer;
pub mod vs_mode;
pub mod wallpaper;
pub mod weapon;
pub mod wpeffect;
pub mod yoshi;
