//! The asset pack built from ROM bytes: geometry, textures, animations,
//! sprites, audio and the `TABLES` section, in the layout the PSP runtime
//! reads (`ssb_rom::pack`).
//!
//! Portable: `no_std + alloc`, no file I/O, no PSP code. `romtool pack` is a
//! wrapper (options, file output, summary); the PSP installer runs the same
//! build on the device (ROM-free release, RE-588, D-046, D-048).
//!
//! - Input: the ROM's bytes and [`PackOptions`].
//! - Progress and census lines: [`PackLog`].
//! - The `romtool residuals` audit's records: [`PackObserver`].
//! - Single-threaded: the per-build state lives in one [`PackCtx`].

#![no_std]
// Census tables and per-build records use ad-hoc tuples on purpose.
#![allow(clippy::type_complexity)]

extern crate alloc;
#[cfg(any(feature = "std", test))]
extern crate std;

// The modules' shared prelude: each module imports `crate::*`.
#[allow(unused_imports)]
use alloc::{
    borrow::ToOwned,
    boxed::Box,
    collections::{BTreeMap, BTreeSet},
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};
use core::cell::{Cell, RefCell};
use core::fmt;
use ssb_rom::rom_read::RomRef;

mod build;
#[cfg(test)]
mod convert_tests;
mod effect_assets;
pub mod filter_coverage;
mod graph_seeds;
pub mod install;
mod loaded;
mod mat_anims;
mod menus;
mod mesh_pack;
pub mod observe;
pub mod residual_fixes;
mod rom_bridge;
pub mod solver;
mod texture_convert;

pub use build::*;
pub use effect_assets::*;
pub use graph_seeds::*;
pub use loaded::*;
pub use mat_anims::*;
pub use menus::*;
pub use mesh_pack::*;
pub use observe::{Animated, Conversion, NoObserver, PackObserver, UseSite};
pub use texture_convert::*;

/// A failed build: the message says what the ROM or the packer lacked.
pub type PackError = Box<dyn core::error::Error>;

/// `Ok` or a [`PackError`].
pub type PackResult<T = ()> = Result<T, PackError>;

/// Texgen filter coverage (RE-311). Anything but [`TexgenCoverage::Exact`]
/// is a measurement mode (`ROMTOOL_TEXGEN_COVERAGE`): the shipped pack uses
/// the exact sets.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TexgenCoverage {
    /// The real per-pose sets.
    #[default]
    Exact,
    /// `full-tile`: the pre-RE-311 optimisation and gate; reproduces the
    /// pre-RE-311 pack byte for byte.
    FullTile,
    /// `full-tile-per-primitive`.
    FullTilePerPrimitive,
}

/// How to build the pack. [`Default`] is the shipped pack.
#[derive(Clone, Debug)]
pub struct PackOptions {
    /// Pack only this archive file's meshes (development).
    pub only_file: Option<u32>,
    /// Swizzle textures for the GE (`--no-swizzle` bisects texture bugs:
    /// swizzling is a pure reordering).
    pub swizzle: bool,
    /// Report-only (RE-318): draw nothing for primitives the wide-tile
    /// lowering produced.
    pub hide_wide_tiles: bool,
    /// Report-only A/B packs: also apply these section 11 review candidates
    /// ([`residual_fixes::REVIEW_CASES`] variants).
    pub residual_candidates: Vec<u16>,
    /// Report-only: zero the alpha of every packed direct override (cutout
    /// sites) to capture the frame without them.
    pub residual_hide_probe: bool,
    /// Texgen filter coverage mode.
    pub texgen_coverage: TexgenCoverage,
    /// Treat every texture as compensated (tests of the solve itself).
    pub list_all: bool,
}

impl Default for PackOptions {
    fn default() -> Self {
        Self {
            only_file: None,
            swizzle: true,
            hide_wide_tiles: false,
            residual_candidates: Vec::new(),
            residual_hide_probe: false,
            texgen_coverage: TexgenCoverage::Exact,
            list_all: false,
        }
    }
}

impl PackOptions {
    /// Sets [`PackOptions::residual_candidates`], refusing a variant that is
    /// not a review candidate.
    pub fn set_residual_candidates(
        &mut self,
        variants: Vec<u16>,
    ) -> Result<(), alloc::string::String> {
        for v in &variants {
            if !residual_fixes::REVIEW_CASES.iter().any(|c| c.variant == *v) {
                return Err(alloc::format!("v{v} is not a review candidate"));
            }
        }
        self.residual_candidates = variants;
        Ok(())
    }
}

/// Where the build's lines go: one call per line.
pub trait PackLog {
    /// A progress or census line (`romtool pack`'s stdout).
    fn line(&self, args: fmt::Arguments<'_>);

    /// A diagnostic line (`romtool pack`'s stderr).
    fn diag(&self, args: fmt::Arguments<'_>) {
        self.line(args);
    }

    /// The build reached phase `name` (memory measurement: a counting
    /// allocator reports its live and peak bytes here). Prints nothing by
    /// default.
    fn phase(&self, _name: &str) {}

    /// Progress inside the running phase: `done` of `total` steps (the
    /// meshes phase, about 55% of a PSP build, reports one step per archive
    /// file: the installer's progress line, RE-588). Does nothing by
    /// default.
    fn progress(&self, _done: usize, _total: usize) {}
}

/// The log that drops every line.
pub struct NoLog;

impl PackLog for NoLog {
    fn line(&self, _args: fmt::Arguments<'_>) {}
}

/// One line to the build's [`PackLog`].
#[macro_export]
macro_rules! pack_log {
    ($ctx:expr, $($arg:tt)*) => {
        $ctx.log.line(format_args!($($arg)*))
    };
}

/// One diagnostic line to the build's [`PackLog`].
#[macro_export]
macro_rules! pack_diag {
    ($ctx:expr, $($arg:tt)*) => {
        $ctx.log.diag(format_args!($($arg)*))
    };
}

/// One classified two-tile blend primitive and what the packer did with it.
#[derive(Clone, Debug)]
pub struct LodBlendSite {
    pub file: u32,
    pub dl: u32,
    pub prim: usize,
    pub triangles: usize,
    /// `Ok(next textures added)` or the reason the primitive keeps its
    /// single-texture path.
    pub outcome: Result<usize, &'static str>,
}

/// RE-426 pipeline counts: texture-part primitives recorded, sprites
/// converted, ids whose sprite pointer did not resolve, and primitives the
/// wide-tile lowering split (left on their own texture).
#[derive(Clone, Copy, Debug, Default)]
pub struct TexturePartStats {
    pub prims: usize,
    pub sprites: usize,
    pub unresolved: usize,
    pub lowered: usize,
}

/// RE-527 guard: primitives whose texture the converter rewrites
/// (`TextureRef::rgb_lerp`, `rgb_unused`) and the material animations that
/// swap them at runtime. Image swaps (`convert_mat_anim_sprite`, texture
/// parts) convert through the same `TextureRef` and so are rewritten too;
/// `convert_mat_anim_palette` is not, so a palette swap on such a
/// primitive would draw the raw palette and fails the pack.
#[derive(Clone, Copy, Debug, Default)]
pub struct RgbLerpStats {
    pub lerped: usize,
    pub lerped_sprite_anim: usize,
    pub lerped_palette_anim: usize,
    pub unused_palette_anim: usize,
}

/// One build's options, sinks and census state, passed by reference to
/// every converter that reads them.
pub struct PackCtx<'a> {
    pub opts: &'a PackOptions,
    pub log: &'a dyn PackLog,
    pub observer: &'a dyn PackObserver,
    /// RE-318 census: source primitives lowered, triangles split,
    /// primitives emitted.
    pub wide_tiles: Cell<(usize, usize, usize)>,
    /// RE-321 census: every primitive the converter classified as a
    /// two-tile blend, with the outcome.
    pub lod_blend_sites: RefCell<Vec<LodBlendSite>>,
    pub texture_parts: Cell<TexturePartStats>,
    pub rgb_lerp: Cell<RgbLerpStats>,
    /// Primitives that named a sprite-table script before any primitive
    /// drawing its image did, and so carry no `mat_anim` (RE-326).
    pub non_owner_first: Cell<usize>,
}

impl<'a> PackCtx<'a> {
    pub fn new(
        opts: &'a PackOptions,
        log: &'a dyn PackLog,
        observer: &'a dyn PackObserver,
    ) -> Self {
        Self {
            opts,
            log,
            observer,
            wide_tiles: Cell::new((0, 0, 0)),
            lod_blend_sites: RefCell::new(Vec::new()),
            texture_parts: Cell::new(TexturePartStats::default()),
            rgb_lerp: Cell::new(RgbLerpStats::default()),
            non_owner_first: Cell::new(0),
        }
    }

    /// Updates the texture-part census.
    pub fn texture_part(&self, f: impl FnOnce(&mut TexturePartStats)) {
        let mut s = self.texture_parts.get();
        f(&mut s);
        self.texture_parts.set(s);
    }
}
