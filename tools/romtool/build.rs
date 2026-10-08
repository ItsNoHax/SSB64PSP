//! `romtool`'s tests read the tables `ssb-rom` generates from the ROM; in a
//! `SSB64_STUB_TABLES=1` build (CI) they are ignored like the crates' own.

fn main() {
    ssb_tablegen::declare_stub_cfg();
}
