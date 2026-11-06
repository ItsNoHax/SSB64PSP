//! Writes the table ids and the section format (`ssb_tablegen::tables`,
//! layout only, no ROM data) into OUT_DIR, and declares
//! `cfg(ssb64_stub_tables)` (`SSB64_STUB_TABLES=1`): the `host-rom`
//! fallback then installs the empty section.

fn main() {
    ssb_tablegen::declare_stub_cfg();
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR"));
    let path = out.join("table_ids.rs");
    let text = ssb_tablegen::tables::ids_rs();
    if std::fs::read_to_string(&path).ok().as_deref() != Some(text.as_str()) {
        std::fs::write(&path, text).unwrap_or_else(|e| panic!("writing {}: {e}", path.display()));
    }
}
