//! Every sprite `ssb_game::players_vs::layer` draws is one `romtool pack`
//! converts (RE-411).

use ssb_game::players_vs::layer;

fn packed(file: u32, offset: u32) -> bool {
    ssb_rom::sprite::FILES
        .iter()
        .any(|f| f.file == file && f.offsets.contains(&offset))
}

#[test]
fn every_vs_select_sprite_is_packed() {
    for &(file, offset) in layer::SPRITES {
        assert!(packed(file, offset), "{file}+{offset:#x}");
    }
    for &(file, list) in layer::SPRITE_LISTS {
        for &offset in list.iter().filter(|&&o| o != 0) {
            assert!(packed(file, offset), "{file}+{offset:#x}");
        }
    }
    let (file, offset, luts) = layer::GATE_CARD;
    assert_eq!(
        (file, offset, usize::from(luts)),
        (
            ssb_rom::sprite::PLAYERS_COMMON.file,
            ssb_rom::sprite::GATE_CARD,
            ssb_rom::sprite::GATE_LUTS.len()
        )
    );
}

/// And every sprite `ssb_game::fighter_select::layer` adds for the
/// Training select.
#[test]
fn every_training_select_sprite_is_packed() {
    use ssb_game::fighter_select::layer as training;
    for &(file, offset) in training::SPRITES {
        assert!(packed(file, offset), "{file}+{offset:#x}");
    }
    let (file, offset, luts) = training::GATE_CARD;
    assert_eq!(
        (file, offset, usize::from(luts)),
        (
            ssb_rom::sprite::PLAYERS_1P_MODE_FILE,
            ssb_rom::sprite::TRAINING_GATE_CARD,
            ssb_rom::sprite::TRAINING_GATE_LUTS.len()
        )
    );
}
