//! All US 1P knockback multipliers, including Master Hand and Samus.
#[test]
fn forty_handicap_rows_match_the_us_rom_bit_for_bit() {
    let Some(path) = std::env::var_os("SSB64_ROM") else {
        return;
    };
    let rom = std::fs::read(path).unwrap();
    assert_eq!(
        ssb_rom::rom::identify(&rom).unwrap().region,
        ssb_rom::rom::Region::Us
    );
    // `dFTCommonDataHandicapTable`, RDRAM 0x8012C830, ROM 0xA8030.
    for (i, row) in ssb_game::stale::HANDICAP_TABLE.iter().enumerate() {
        for (j, value) in row.iter().enumerate() {
            let at = 0xA8030 + i * 8 + j * 4;
            let word = u32::from_be_bytes(rom[at..at + 4].try_into().unwrap());
            assert_eq!(value.to_bits(), word, "handicap {} factor {j}", i + 1);
        }
    }
}
