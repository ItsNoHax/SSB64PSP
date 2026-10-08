//! Generates this crate's ROM-derived tables into OUT_DIR from the user's
//! own ROM (`crates/ssb-tablegen`). Fails with instructions when there is no
//! ROM; `SSB64_STUB_TABLES=1` builds empty stubs instead (CI).

fn main() {
    ssb_tablegen::build_script(ssb_tablegen::Crate::Game);
}
