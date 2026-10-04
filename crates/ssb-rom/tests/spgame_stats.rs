//! Every authored US stat flag, independent of the source-header generator.
#[test]
fn all_497_status_flags_match_the_us_rom() {
    let Some(path) = std::env::var_os("SSB64_ROM") else {
        return;
    };
    let rom = std::fs::read(path).unwrap();
    assert_eq!(
        ssb_rom::rom::identify(&rom).unwrap().region,
        ssb_rom::rom::Region::Us
    );
    use ssb_game::fighter::FighterKind as F;
    // FTStatusDesc is 20 bytes: mflags and sflags are each two bytes,
    // followed by four callbacks. RDRAM - 0x80084800 maps this overlay.
    let tables = [
        (F::Mario, 0, 220, 0xA45D8),
        (F::Mario, 220, 9, 0xA5708),
        (F::Fox, 220, 26, 0xA5A14),
        (F::Donkey, 220, 30, 0xA57BC),
        (F::Samus, 220, 11, 0xA5C1C),
        (F::Luigi, 220, 9, 0xA5CF8),
        (F::Link, 220, 17, 0xA5DAC),
        (F::Yoshi, 220, 14, 0xA66F8),
        (F::Captain, 220, 19, 0xA657C),
        (F::Kirby, 220, 83, 0xA5F00),
        (F::Pikachu, 220, 18, 0xA6810),
        (F::Purin, 220, 16, 0xA6978),
        (F::Ness, 220, 25, 0xA6AB8),
    ];
    let mut checked = 0;
    for (kind, start, count, offset) in tables {
        for i in 0..count {
            let at = offset + i as usize * 20 + 2;
            let expected = u16::from_be_bytes(rom[at..at + 2].try_into().unwrap());
            let flags = ssb_game::spgame::live::status_flags_by_id(kind, start + i);
            assert_eq!(flags.0, expected, "{kind:?} status {}", start + i);
            // Every source ID fits the bonus counter's enum.
            assert!((flags.id() as usize) < ssb_game::spgame::bonus::ATTACK_COUNT);
            checked += 1;
        }
    }
    assert_eq!(checked, 497);
}
