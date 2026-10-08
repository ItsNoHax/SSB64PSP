//! Writes every generated table into a directory, for inspection:
//!
//!     cargo run -p ssb-tablegen -- [--rom PATH] [--stub] OUT_DIR
//!
//! The ROM defaults to `SSB64_ROM`, else the one `.z64` in `rom/`. The build
//! scripts of `ssb-game` and `ssb-rom` produce the same files in their
//! `OUT_DIR`; this never writes into the source tree unless told to.

use std::path::{Path, PathBuf};

use ssb_tablegen::{generate, locate_rom, Crate, Source};

fn main() {
    let mut rom: Option<PathBuf> = None;
    let mut stub = false;
    let mut out: Option<PathBuf> = None;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--rom" => rom = args.next().map(PathBuf::from),
            "--stub" => stub = true,
            _ if out.is_none() => out = Some(PathBuf::from(a)),
            _ => usage(),
        }
    }
    let Some(out) = out else { usage() };
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let source = if stub {
        None
    } else {
        let path = rom
            .or_else(|| locate_rom(&repo).unwrap_or_else(|e| fail(&e)))
            .unwrap_or_else(|| fail("no ROM: pass --rom, set SSB64_ROM or put it in rom/"));
        Some(Source::open(&path).unwrap_or_else(|e| fail(&e)))
    };
    std::fs::create_dir_all(&out).unwrap_or_else(|e| fail(&e.to_string()));
    for krate in [Crate::Game, Crate::Rom] {
        for file in generate(krate, source.as_ref()).unwrap_or_else(|e| fail(&e.to_string())) {
            let path = out.join(file.name);
            std::fs::write(&path, file.text).unwrap_or_else(|e| fail(&e.to_string()));
            println!("wrote {}", path.display());
        }
    }
}

fn usage() -> ! {
    fail("usage: ssb-tablegen [--rom PATH] [--stub] OUT_DIR")
}

fn fail(msg: &str) -> ! {
    eprintln!("ssb-tablegen: {msg}");
    std::process::exit(2)
}
