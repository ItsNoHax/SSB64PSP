//! Reading of Super Smash Bros. 64 ROM data.
//!
//! Nothing in this crate embeds copyrighted material. It only describes *how*
//! to interpret a ROM the user already owns; the bytes stay on the user's disk.
//!
//! The crate is `no_std + alloc` so the same decoders can run on the PSP if we
//! ever want runtime loading, even though the intended path is build-time
//! conversion (see `tools/romtool`).

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod anim;
pub mod anim_color;
pub mod archive;
pub mod audio;
pub mod camanim;
pub mod campaign;
pub mod collision;
/// The decomp's small `d*` data arrays, generated at build time from the
/// ROM by `ssb-tablegen` (`d_arrays`; D-048). Each constant is the symbol's
/// name without the `d`, in `UPPER_SNAKE`.
mod d_arrays {
    include!(concat!(env!("OUT_DIR"), "/d_arrays.rs"));
}
pub mod depth_mask;
pub mod dl;
pub mod effect;
pub mod ending;
pub mod explain;
pub mod figatree;
pub mod fighter;
pub mod filter_compensation;
pub mod ground_obj;
pub mod interp;
pub mod loader_plan;
pub mod lod_blend;
pub mod matanim;
pub mod menu_pack;
pub mod mesh;
pub mod mmonster;
pub mod mobj;
pub mod n64_addressing;
pub mod n64_depth;
pub mod n64_filter;
pub mod objanim;
pub mod opening;
pub mod pack;
pub mod particle;
pub mod player_interface;
pub mod psp_texture;
pub mod reloc_link;
pub mod residency;
pub mod rom;
pub mod scan;
pub mod scene;
pub mod scene_deps;
pub mod scene_roots;
pub mod sector;
pub mod skeleton;
pub mod sprite;
#[cfg(feature = "std")]
pub mod sprite_fringe;
#[doc(hidden)]
pub mod sprite_offsets;
pub mod stage;
pub mod stream_pool;
pub mod strict;
pub mod task_head;
pub mod texture;
pub mod title;
pub mod transition;
pub mod vpk0;
pub mod wide_tile;

pub use archive::{Archive, TableEntry};
pub use rom::{Region, RomInfo};

pub mod error;
pub use error::{Error, Result};
pub mod bonus1;
pub mod bonus2;
