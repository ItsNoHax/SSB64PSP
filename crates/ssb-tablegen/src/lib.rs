//! Build-time generation of the tables `ssb-game`, `ssb-engine` and `ssb-rom` read from
//! the original game's data: fighter motion scripts, colour-animation
//! scripts, CPU input scripts, status flags, the fighters' joint masks, the
//! fighter animation table and the `gSYSinTable` sine table.
//!
//! The values live in the user's ROM, so they are never committed. The build
//! scripts of `ssb-game`, `ssb-engine` and `ssb-rom` call [`build_script`], which finds the
//! ROM (`SSB64_ROM`, else the one `.z64` in `rom/`), verifies it, reads the
//! tables and writes Rust source into Cargo's `OUT_DIR` for the crates to
//! `include!`. What is committed here is only *where* each table is and how
//! it is laid out (`layout` modules): ROM offsets, record sizes, counts and
//! the identifiers the port gives each entry.
//!
//! Without a ROM the build fails with a message saying how to supply one.
//! `SSB64_STUB_TABLES=1` instead writes empty tables of the same types and
//! sets `cfg(ssb64_stub_tables)`, so CI, which has no ROM, can type-check,
//! lint and run every test that does not need ROM data. A stub build is not
//! a playable game.

extern crate alloc;

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
// runs this crate too, so the name resolves to this crate's `math` instead,
// the host `std` square root that `ssb-engine` uses with `std`.
extern crate self as ssb_engine;
mod math {
    /// `ssb_engine::math::sqrt_exact` with `std`.
    pub fn sqrt_exact(v: f32) -> f32 {
        v.sqrt()
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
mod emit;
mod ending_data;
mod fighters;
mod ground;
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
mod throw_descs;

pub use source::Source;

use std::path::{Path, PathBuf};

/// Each fighter's `<Name>Main` archive file and the offset of its
/// `FTAttributes` in it, as the generator reads them: `(name, file,
/// offset)`. `ssb-rom` checks these against `fighter::FIGHTER_FILES`.
pub fn fighter_main_files() -> Vec<(&'static str, u32, u32)> {
    fighters::FIGHTERS
        .iter()
        .map(|f| (f.name, f.main_file, f.attributes))
        .collect()
}

/// The crate whose tables a build script generates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Crate {
    /// `ssb-game`: motion, colour-animation and CPU scripts, status flags,
    /// joint masks and hidden/texture parts, hurtboxes, Falcon Dive's
    /// capture offsets, the items' `ITAttributes`, the item weapons'
    /// `WPAttributes` and the Smog's scale, Mario's `FTAttributes` physics,
    /// Kirby's copy table and the quakes' and Thunder Shock's lives.
    Game,
    /// `ssb-rom`: the fighter animation table, the offsets the stage
    /// clear's and staff roll's data arrays hold, the stage clear's pictures,
    /// each fighter's main file, the loading-break transitions and the
    /// manager effects' objects and animations.
    Rom,
    /// `ssb-engine`: `gSYSinTable`.
    Engine,
}

/// One generated file: its name in `OUT_DIR` and its Rust source.
pub struct Generated {
    pub name: &'static str,
    pub text: String,
}

/// Generates `krate`'s tables from `rom`, or their stubs when `rom` is
/// `None`.
pub fn generate(krate: Crate, rom: Option<&Source>) -> Result<Vec<Generated>> {
    Ok(match krate {
        Crate::Game => {
            let mut files = vec![
                Generated {
                    name: "motion_scripts.rs",
                    text: motion::generate(rom)?,
                },
                Generated {
                    name: "colanim_scripts.rs",
                    text: colanim::generate(rom)?,
                },
                Generated {
                    name: "computer_scripts.rs",
                    text: computer::generate(rom)?,
                },
                Generated {
                    name: "stat_flags.rs",
                    text: stat_flags::generate(rom)?,
                },
                Generated {
                    name: "ground_tables.rs",
                    text: ground::generate(rom)?,
                },
                Generated {
                    name: "item_sounds.rs",
                    text: item_sounds::generate(rom)?,
                },
                Generated {
                    name: "joint_masks.rs",
                    text: model_parts::generate(rom)?,
                },
                Generated {
                    name: "kirby_attack100.rs",
                    text: kirby_attack100::generate(rom)?,
                },
                Generated {
                    name: "effect_life.rs",
                    text: effect_life::generate(rom)?,
                },
                Generated {
                    name: "hurtboxes.rs",
                    text: hurtbox::generate(rom)?,
                },
                Generated {
                    name: "capture_captain.rs",
                    text: capture_captain::generate(rom)?,
                },
                Generated {
                    name: "item_attributes.rs",
                    text: item_attributes::generate(rom)?,
                },
                Generated {
                    name: "monster_weapons.rs",
                    text: monster_weapons::generate(rom)?,
                },
                Generated {
                    name: "mario_physics.rs",
                    text: physics::generate(rom)?,
                },
                Generated {
                    name: "kirby_copy.rs",
                    text: kirby_copy::generate(rom)?,
                },
                Generated {
                    name: "throw_descs.rs",
                    text: throw_descs::generate(rom)?,
                },
                Generated {
                    name: "special_colls.rs",
                    text: throw_descs::generate_special_colls(rom)?,
                },
            ];
            files.extend(battle_data::generate(rom)?);
            files.extend(ifcommon::generate(rom)?);
            files.extend(menu_data::generate(rom)?);
            files.extend(sc1p_data::generate(rom)?);
            files.extend(ending_data::generate(rom)?);
            files.extend(opening_data::generate(rom)?);
            files.push(d_arrays::generate_game(rom)?);
            files
        }
        Crate::Rom => {
            let mut files = vec![Generated {
                name: "anim_table.rs",
                text: anim_table::generate(rom)?,
            }];
            files.extend(sc1p_data::generate_rom(rom)?);
            files.extend(ending_data::generate_rom(rom)?);
            files.extend(rom_layout::generate_rom(rom)?);
            files.push(d_arrays::generate_rom(rom)?);
            files
        }
        Crate::Engine => vec![
            Generated {
                name: "sin_table.rs",
                text: sin_table::generate(rom)?,
            },
            d_arrays::generate_engine(rom)?,
        ],
    })
}

/// Environment variable naming the ROM (the convention `romtool` and the
/// ROM-backed tests share).
pub const ROM_ENV: &str = "SSB64_ROM";
/// Environment variable that selects empty stub tables when no ROM is
/// available (CI).
pub const STUB_ENV: &str = "SSB64_STUB_TABLES";
/// `cfg` set on a crate built with stub tables.
pub const STUB_CFG: &str = "ssb64_stub_tables";

/// For a crate that only *uses* the generated tables (`romtool`): declares
/// `cfg(ssb64_stub_tables)` and sets it when `SSB64_STUB_TABLES=1`, as the
/// generating crates' build scripts do, so its ROM-table tests are ignored
/// in a stub build too.
pub fn declare_stub_cfg() {
    println!("cargo::rustc-check-cfg=cfg({STUB_CFG})");
    println!("cargo::rerun-if-env-changed={STUB_ENV}");
    if std::env::var_os(STUB_ENV).is_some_and(|v| v == "1") {
        println!("cargo::rustc-cfg={STUB_CFG}");
    }
}

/// Where the ROM is: `SSB64_ROM`, else the single `.z64` file in
/// `<repository>/rom/`. `Ok(None)` when neither exists.
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
pub fn test_rom(manifest_dir: &str) -> Option<PathBuf> {
    locate_rom(&Path::new(manifest_dir).join("../..")).unwrap_or_else(|e| panic!("{e}"))
}

/// The build-script entry point for `krate`, whose manifest directory is
/// `CARGO_MANIFEST_DIR`. Writes the generated files into `OUT_DIR` and
/// prints Cargo's rerun and `cfg` directives. Panics, with instructions,
/// when no ROM is available and stubs were not asked for.
pub fn build_script(krate: Crate) {
    let manifest =
        PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR"));
    // crates/<name> -> the repository root.
    let repo = manifest
        .parent()
        .and_then(Path::parent)
        .expect("crate directory sits two levels below the repository root")
        .to_path_buf();

    println!("cargo::rustc-check-cfg=cfg({STUB_CFG})");
    println!("cargo::rerun-if-env-changed={ROM_ENV}");
    println!("cargo::rerun-if-env-changed={STUB_ENV}");
    println!("cargo::rerun-if-changed={}", repo.join("rom").display());

    let stub = std::env::var_os(STUB_ENV).is_some_and(|v| v == "1");
    let rom = match locate_rom(&repo) {
        Ok(rom) => rom,
        Err(e) => panic!("\n\nssb-tablegen: {e}\n\n"),
    };
    let source = match (rom, stub) {
        (Some(path), false) => {
            println!("cargo::rerun-if-changed={}", path.display());
            match Source::open(&path) {
                Ok(source) => Some(source),
                Err(e) => panic!(
                    "\n\nssb-tablegen: {} is not a usable Super Smash Bros. (USA) ROM: {e}\n\
                     The build reads its tables from a verified US ROM (SHA-1 {}).\n\n",
                    path.display(),
                    rom::Region::Us.sha1()
                ),
            }
        }
        (_, true) => None,
        (None, false) => panic!(
            "\n\nssb-tablegen: no ROM found.\n\
             This crate's tables are generated at build time from your own legally\n\
             obtained Super Smash Bros. (USA) ROM (SHA-1 {}); none are committed.\n\
             Put it in {}/ or set {ROM_ENV}=/path/to/rom.z64, then rebuild.\n\
             (CI only: {STUB_ENV}=1 builds empty stub tables that type-check but\n\
             do not run the game.)\n\n",
            rom::Region::Us.sha1(),
            repo.join("rom").display()
        ),
    };
    if source.is_none() {
        println!("cargo::rustc-cfg={STUB_CFG}");
        println!(
            "cargo::warning={STUB_ENV}=1: building {krate:?} with empty stub tables; this build cannot run the game"
        );
    }
    let files = match generate(krate, source.as_ref()) {
        Ok(files) => files,
        Err(e) => panic!("\n\nssb-tablegen: reading the ROM's tables failed: {e}\n\n"),
    };
    for file in files {
        let path = out.join(file.name);
        // Leave an unchanged file's timestamp alone.
        if std::fs::read_to_string(&path).ok().as_deref() != Some(file.text.as_str()) {
            std::fs::write(&path, file.text)
                .unwrap_or_else(|e| panic!("writing {}: {e}", path.display()));
        }
    }
}
