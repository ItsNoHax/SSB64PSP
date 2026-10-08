//! Build-time generation of the tables `ssb-game` and `ssb-rom` read from
//! the original game's data: fighter motion scripts, colour-animation
//! scripts, CPU input scripts, status flags and the fighter animation table.
//!
//! The values live in the user's ROM, so they are never committed. The build
//! scripts of `ssb-game` and `ssb-rom` call [`build_script`], which finds the
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

pub use error::{Error, Result};

mod anim_table;
mod colanim;
mod computer;
mod emit;
mod figatree;
mod fighters;
mod ground;
mod motion;
mod source;
mod stat_flags;

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
    /// `ssb-game`: motion, colour-animation and CPU scripts, status flags.
    Game,
    /// `ssb-rom`: the fighter animation table.
    Rom,
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
        Crate::Game => vec![
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
        ],
        Crate::Rom => vec![Generated {
            name: "anim_table.rs",
            text: anim_table::generate(rom)?,
        }],
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
            return Err(format!("{ROM_ENV}={} is not a file", path.display()));
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
