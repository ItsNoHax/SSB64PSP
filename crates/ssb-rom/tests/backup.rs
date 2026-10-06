//! `dSCManagerDefaultBackupData` in the ROM against the port's defaults and
//! layout (`ssb_game::backup`).
use ssb_game::backup::{Backup, SIZE};

/// `dSCManagerDefaultBackupData` (0x800A3994) in `sc/scmanager`'s `.data`:
/// that segment's code starts at ROM 0x406D0, VRAM 0x800A1980.
const DEFAULT_BACKUP_ROM: usize = 0x800A_3994 - 0x800A_1980 + 0x406D0;

fn rom_bytes() -> Option<Vec<u8>> {
    std::fs::read(std::env::var_os("SSB64_ROM")?).ok()
}

#[test]
fn the_default_backup_matches_the_rom_byte_for_byte() {
    let Some(bytes) = rom_bytes() else { return };
    let info = ssb_rom::rom::identify(&bytes).unwrap();
    assert_eq!(info.region, ssb_rom::rom::Region::Us);
    let rom = &bytes[DEFAULT_BACKUP_ROM..DEFAULT_BACKUP_ROM + SIZE];
    let port = Backup::default().encode();
    // Every field and its padding; the ROM's checksum field is 0.
    assert_eq!(&port[..SIZE - 4], &rom[..SIZE - 4]);
    assert_eq!(&rom[SIZE - 4..], &[0; 4]);
}
