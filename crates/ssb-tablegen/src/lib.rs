//! The tables `ssb-game`, `ssb-engine` and `ssb-rom` read from the original
//! game's data (motion, colour-animation and CPU scripts, status flags,
//! joint masks, the fighter animation table, the decomp's `d*` arrays, ...):
//! where each is in the ROM, how it is laid out, and how to read it.
//!
//! - Values: [`tables_section`] reads them from the user's ROM into the
//!   pack's `TABLES` section (`romtool pack` on a PC, `ssb-pack` on the
//!   PSP); `ssb-tables` reads the section at run time. No binary holds them
//!   (D-048).
//! - Layout: the build scripts of `ssb-game` and `ssb-rom` call
//!   [`build_script`], which writes Rust naming each table's entries and
//!   ids into Cargo's `OUT_DIR` for the crates to `include!`. It reads no
//!   ROM: the workspace builds without one.
//! - `SSB64_STUB_TABLES=1` sets `cfg(ssb64_stub_tables)` ([`declare_stub_cfg`]):
//!   host tests that need the ROM's tables are ignored, and `ssb-tables`'
//!   `host-rom` fallback installs the empty section (every table zeros)
//!   instead of reading the ROM. For CI, which has no ROM; never for a
//!   game build.
//!
//! Without the default `std` feature only the `TABLES` section's reader is
//! built ([`tables_section`], [`Source`]): the portable packer
//! (`ssb-pack`) builds the section on the device.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::vec::Vec;

// `ssb-rom`'s ROM reader, compiled in rather than depended on: `ssb-rom`'s
// own build script runs this crate, so it cannot be a dependency of it.
#[path = "../../ssb-rom/src/archive.rs"]
#[allow(dead_code)]
pub mod archive;
#[path = "../../ssb-rom/src/error.rs"]
pub mod error;
#[path = "../../ssb-rom/src/rom.rs"]
#[allow(dead_code)]
pub mod rom;
#[path = "../../ssb-rom/src/vpk0.rs"]
pub mod vpk0;
// `ssb-rom`'s sprite manifests, for the tables that name a sprite by its
// file offset (`ifcommon`).
#[path = "../../ssb-rom/src/sprite_offsets.rs"]
#[allow(dead_code)]
pub mod sprite_offsets;
// `ssb-rom`'s animation player, for a table gameplay keeps as the values a
// fixed `AObjEvent32` script produces (`monster_weapons`).
#[path = "../../ssb-rom/src/figatree.rs"]
#[allow(dead_code)]
pub mod figatree;
// `ssb-rom`'s figatree binding order, for the joint whose script decides a
// fighter animation's end (`motion`'s `END_BLOCKS`).
#[path = "../../ssb-rom/src/figatree_order.rs"]
pub mod figatree_order;
#[path = "../../ssb-rom/src/interp.rs"]
#[allow(dead_code)]
pub mod interp;
#[path = "../../ssb-rom/src/objanim.rs"]
#[allow(dead_code)]
pub mod objanim;
// `ssb-rom`'s material animation player, for the texture id a fighter's
// costume script gives its texture part (`model_parts`).
#[path = "../../ssb-rom/src/matanim.rs"]
#[allow(dead_code)]
pub mod matanim;

// `interp` calls `ssb_engine::math::sqrt_exact`. `ssb-engine`'s build script
// runs this crate too, so the name resolves to this crate's `math` instead:
// the host `std` square root that `ssb-engine` uses with `std`, else
// `libm`'s, which rounds correctly too (the same bits as the PSP's
// `sqrt.s`).
extern crate self as ssb_engine;
mod math {
    // Host tests: the `no_std` (PSP) square root on request, and a count
    // of the calls (`tables::tests`).
    #[cfg(all(test, feature = "std"))]
    std::thread_local! {
        pub static LIBM: core::cell::Cell<bool> = const { core::cell::Cell::new(false) };
        pub static CALLS: core::cell::Cell<u64> = const { core::cell::Cell::new(0) };
    }

    /// `ssb_engine::math::sqrt_exact`.
    pub fn sqrt_exact(v: f32) -> f32 {
        #[cfg(all(test, feature = "std"))]
        {
            CALLS.with(|c| c.set(c.get() + 1));
            if LIBM.with(|l| l.get()) {
                return libm::sqrtf(v);
            }
        }
        #[cfg(feature = "std")]
        {
            v.sqrt()
        }
        #[cfg(not(feature = "std"))]
        {
            libm::sqrtf(v)
        }
    }
}

pub use error::{Error, Result};

mod anim_length;
mod anim_table;
mod battle_data;
mod capture_captain;
mod colanim;
mod computer;
mod d_arrays;
mod effect_life;
mod ending_data;
mod fighters;
mod ground;
mod ground_effect;
mod hurtbox;
mod ifcommon;
mod item_attributes;
mod item_sounds;
mod kirby_attack100;
mod kirby_copy;
mod menu_data;
mod model_parts;
mod monster_weapons;
mod motion;
mod opening_data;
mod physics;
mod rom_layout;
mod sc1p_data;
mod sin_table;
// Every code overlay is listed; the generators use some of them.
#[allow(dead_code)]
mod source;
mod stat_flags;
pub mod tables;
mod throw_descs;

pub use source::Source;

/// The pack's `TABLES` section read from `rom`; the empty section (every
/// table zeros, the stub) for `None` ([`tables`]).
pub fn tables_section(rom: Option<&Source>) -> Result<Vec<u8>> {
    tables::section(rom)
}

#[cfg(feature = "std")]
use std::path::{Path, PathBuf};

/// Each fighter's `<Name>Main` archive file and the offset of its
/// `FTAttributes` in it, as the generator reads them: `(name, file,
/// offset)`. `ssb-rom` checks these against `fighter::FIGHTER_FILES`.
#[cfg(feature = "std")]
pub fn fighter_main_files() -> Vec<(&'static str, u32, u32)> {
    fighters::FIGHTERS
        .iter()
        .map(|f| (f.name, f.main_file, f.attributes))
        .collect()
}

/// The crate whose layout files a build script writes.
#[cfg(feature = "std")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Crate {
    /// `ssb-game`: the colour-animation and CPU script names and ids, and
    /// its `d*` array views.
    Game,
    /// `ssb-rom`: the fighter animation table's slots, the names and counts
    /// of the tables it reads from code, and its `d*` array views.
    Rom,
}

/// One generated file: its name in `OUT_DIR` and its Rust source.
#[cfg(feature = "std")]
pub struct Generated {
    pub name: &'static str,
    pub text: String,
}

/// `krate`'s layout files: names, ids, counts and offsets into the pack's
/// `TABLES` section. Committed layout only; no ROM is read.
#[cfg(feature = "std")]
pub fn generate(krate: Crate) -> Vec<Generated> {
    match krate {
        Crate::Game => vec![
            Generated {
                name: "colanim_layout.rs",
                text: colanim::layout_rs(),
            },
            Generated {
                name: "computer_layout.rs",
                text: computer::layout_rs(),
            },
            d_arrays::generate_game(),
        ],
        Crate::Rom => {
            let mut files = vec![Generated {
                name: "anim_layout.rs",
                text: anim_table::layout_rs(),
            }];
            files.extend(rom_layout::generate_rom());
            files.push(d_arrays::generate_rom());
            files
        }
    }
}

/// Environment variable naming the ROM (the convention `romtool` and the
/// ROM-backed tests share).
pub const ROM_ENV: &str = "SSB64_ROM";
/// Environment variable that selects the stub (empty) `TABLES` section for
/// host tests when no ROM is available (CI).
pub const STUB_ENV: &str = "SSB64_STUB_TABLES";
/// `cfg` [`declare_stub_cfg`] sets under `SSB64_STUB_TABLES=1`.
pub const STUB_CFG: &str = "ssb64_stub_tables";

/// Declares `cfg(ssb64_stub_tables)` and sets it when `SSB64_STUB_TABLES=1`:
/// the crate's tests that need the ROM's tables are then ignored. Every
/// crate whose tests read the tables calls it from its build script.
#[cfg(feature = "std")]
pub fn declare_stub_cfg() {
    println!("cargo::rustc-check-cfg=cfg({STUB_CFG})");
    println!("cargo::rerun-if-env-changed={STUB_ENV}");
    if std::env::var_os(STUB_ENV).is_some_and(|v| v == "1") {
        println!("cargo::rustc-cfg={STUB_CFG}");
    }
}

/// Where the ROM is: `SSB64_ROM`, else the single `.z64` file in
/// `<repository>/rom/`. `Ok(None)` when neither exists.
#[cfg(feature = "std")]
pub fn locate_rom(repo: &Path) -> std::result::Result<Option<PathBuf>, String> {
    if let Some(path) = std::env::var_os(ROM_ENV).filter(|p| !p.is_empty()) {
        let path = PathBuf::from(path);
        if !path.is_file() {
            let hint = if path.is_relative() {
                " (use an absolute path)"
            } else {
                ""
            };
            return Err(format!("{ROM_ENV}={} is not a file{hint}", path.display()));
        }
        return Ok(Some(path));
    }
    let dir = repo.join("rom");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Ok(None);
    };
    let mut found: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("z64")))
        .collect();
    found.sort();
    match found.len() {
        0 => Ok(None),
        1 => Ok(found.pop()),
        _ => Err(format!(
            "{} holds {} .z64 files; set {ROM_ENV} to the one to use",
            dir.display(),
            found.len()
        )),
    }
}

/// The ROM for a test or tool of the crate at `manifest_dir`
/// (`env!("CARGO_MANIFEST_DIR")`, two levels below the repository root):
/// [`locate_rom`] from that root. `None` when there is none (CI); panics
/// when `SSB64_ROM` names no file or `rom/` holds more than one ROM.
#[cfg(feature = "std")]
pub fn test_rom(manifest_dir: &str) -> Option<PathBuf> {
    locate_rom(&Path::new(manifest_dir).join("../..")).unwrap_or_else(|e| panic!("{e}"))
}

/// The build-script entry point for `krate`: writes its layout files
/// ([`generate`]) into `OUT_DIR` and declares the stub `cfg`
/// ([`declare_stub_cfg`]). Reads no ROM.
#[cfg(feature = "std")]
pub fn build_script(krate: Crate) {
    declare_stub_cfg();
    let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR"));
    for file in generate(krate) {
        let path = out.join(file.name);
        // Leave an unchanged file's timestamp alone.
        if std::fs::read_to_string(&path).ok().as_deref() != Some(file.text.as_str()) {
            std::fs::write(&path, file.text)
                .unwrap_or_else(|e| panic!("writing {}: {e}", path.display()));
        }
    }
}
