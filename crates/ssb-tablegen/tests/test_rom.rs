//! `test_rom` is `locate_rom` from the repository root two levels above the
//! calling crate's manifest directory.

use std::path::Path;

#[test]
fn test_rom_is_locate_rom_from_the_repository_root() {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let expected = ssb_tablegen::locate_rom(&Path::new(manifest).join("../..")).unwrap();
    assert_eq!(ssb_tablegen::test_rom(manifest), expected);
}
