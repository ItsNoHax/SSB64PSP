//! Declares `cfg(ssb64_stub_tables)` (`SSB64_STUB_TABLES=1`): the tests
//! that need the ROM's tables are ignored in a stub build (CI). The tables
//! themselves are read at run time from the pack's `TABLES` section
//! (`ssb-tables`).

fn main() {
    ssb_tablegen::declare_stub_cfg();
}
