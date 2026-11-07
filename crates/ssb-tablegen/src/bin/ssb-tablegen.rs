//! Writes every generated layout file into a directory, for inspection:
//!
//!     cargo run -p ssb-tablegen -- OUT_DIR
//!
//! The files the build scripts of `ssb-game`, `ssb-rom` and `ssb-tables`
//! write into their `OUT_DIR`: layout only, no ROM is read. The tables'
//! values are in the pack's `TABLES` section (`romtool pack`). This never
//! writes into the source tree unless told to.

use std::path::PathBuf;

use ssb_tablegen::{generate, Crate, Generated};

fn main() {
    let mut args = std::env::args().skip(1);
    let (Some(out), None) = (args.next(), args.next()) else {
        fail("usage: ssb-tablegen OUT_DIR")
    };
    let out = PathBuf::from(out);
    std::fs::create_dir_all(&out).unwrap_or_else(|e| fail(&e.to_string()));
    let ids = Generated {
        name: "table_ids.rs",
        text: ssb_tablegen::tables::ids_rs(),
    };
    let files = generate(Crate::Game)
        .into_iter()
        .chain(generate(Crate::Rom))
        .chain([ids]);
    for file in files {
        let path = out.join(file.name);
        std::fs::write(&path, file.text).unwrap_or_else(|e| fail(&e.to_string()));
        println!("wrote {}", path.display());
    }
}

fn fail(msg: &str) -> ! {
    eprintln!("ssb-tablegen: {msg}");
    std::process::exit(2)
}
