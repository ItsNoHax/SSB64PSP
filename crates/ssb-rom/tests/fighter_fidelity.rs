//! ROM facts behind RE-468's fighter fidelity fixes, checked against the
//! unmodified US ROM (`SSB64_ROM`).

use ssb_rom::archive::Archive;

fn rom() -> Option<Vec<u8>> {
    Some(std::fs::read(std::env::var_os("SSB64_ROM")?).unwrap())
}

/// `dLBCommonSinLookup` (`0x800D4CA0`): every one of its 1024 words is the
/// one `ssb_engine::math::lb_sine_sample` builds, so the collision matrices
/// (`gmCollisionTransformMatrixAll`) read the ROM's own sine.
#[test]
fn lbcommon_sine_table_matches_the_rom() {
    let Some(rom) = rom() else {
        return;
    };
    let words: Vec<[u8; 4]> = (0..1024u16)
        .map(|i| ssb_engine::math::lb_sine_sample(i).to_bits().to_be_bytes())
        .collect();
    // Entry 0 is 0.0; look for entries 1.. 16 and then compare the rest.
    let needle: Vec<u8> = words[1..17].iter().flatten().copied().collect();
    let at = rom
        .windows(needle.len())
        .position(|w| w == needle.as_slice())
        .expect("the sine table's opening words are in the ROM")
        - 4;
    for (i, word) in words.iter().enumerate() {
        assert_eq!(&rom[at + i * 4..at + i * 4 + 4], word, "entry {i}");
    }
}

fn donkey_loop_frames(slot_name: &str) -> Option<f32> {
    let rom = rom()?;
    let archive = Archive::open(&rom, ssb_rom::rom::identify(&rom).unwrap().region).unwrap();
    let slot = ssb_rom::anim::SLOT_NAMES
        .iter()
        .position(|n| *n == slot_name)
        .unwrap();
    let donkey = ssb_rom::anim::FIGHTER_ANIMS
        .iter()
        .find(|f| f.name == "Donkey")
        .unwrap();
    let file = archive.load(u32::from(donkey.files[slot])).unwrap();
    Some(ssb_rom::anim::loop_frames(&file).map(f32::from).unwrap())
}

/// The Hand Slap loop's figatree jumps back after 34 frames, the period
/// `ftDonkeySpecialLwLoopProcUpdate` checks (the N64 shows anim frames 0 to
/// 33, then 0 again).
#[test]
fn donkey_hand_slap_loop_is_34_frames() {
    if let Some(frames) = donkey_loop_frames("DonkeySpecialLwLoop") {
        assert_eq!(frames, ssb_game::status::DONKEY_SPECIAL_LW_LOOP_FRAMES);
    }
}

/// The Giant Punch charge loop wraps every 12 frames, the period
/// `ftDonkeySpecialNLoopProcUpdate` counts charge levels by.
#[test]
fn donkey_giant_punch_loop_is_12_frames() {
    if let Some(frames) = donkey_loop_frames("DonkeySpecialNLoop") {
        assert_eq!(frames, ssb_game::status::DONKEY_SPECIAL_N_LOOP_FRAMES);
    }
}

/// `ssb_game::modelpart`'s hidden parts from joint 4 on are each
/// fighter's `FTAttributes::hiddenparts`, with whether the high-detail
/// descriptor carries a display list: Samus's beam strands (24, 25) and
/// Yoshi's tongue (9) are the only ones that draw.
#[test]
fn the_hidden_parts_are_the_roms() {
    let Some(rom) = rom() else {
        return;
    };
    let archive = Archive::open(&rom, ssb_rom::rom::identify(&rom).unwrap().region).unwrap();
    for k in (0..27).filter(|&k| k != 12) {
        let kind = ssb_game::fighter::FighterKind::from_ordinal(k).unwrap();
        let entry = *ssb_rom::fighter::FIGHTER_FILES
            .iter()
            .find(|e| e.kind == kind as u8)
            .unwrap();
        let main = archive.load(entry.file).unwrap();
        let common = ssb_rom::fighter::common_parts(&main, entry)[0].unwrap();
        let model = archive.load(common.model_file).unwrap();
        let graph = ssb_rom::scene::find_scene_graphs(&model)
            .into_iter()
            .find(|g| g.offset == common.graph)
            .unwrap();
        let mut parts = Vec::new();
        for i in 0..16u32 {
            let Some(h) = ssb_rom::fighter::hidden_part(&main, entry, i) else {
                break;
            };
            if (4..37).contains(&h.root_joint_id) {
                let dl = graph.nodes[(h.root_joint_id - 4) as usize]
                    .desc
                    .dl
                    .is_some();
                parts.push((i as u8, h.root_joint_id as u8, dl));
            }
        }
        assert_eq!(
            ssb_game::modelpart::hidden_part_joints(kind),
            parts.as_slice(),
            "{kind:?}"
        );
    }
}
