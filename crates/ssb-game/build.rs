//! Writes this crate's table layout (names, ids, counts; no ROM data) into
//! OUT_DIR (`ssb_tablegen::build_script`) and declares
//! `cfg(ssb64_stub_tables)` (`SSB64_STUB_TABLES=1`): the tests that need
//! the ROM's tables are then ignored. Reads no ROM; the values are in the
//! pack's `TABLES` section (`ssb-tables`).

fn main() {
    ssb_tablegen::build_script(ssb_tablegen::Crate::Game);
}
