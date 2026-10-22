//! ROM facts behind RE-468's fighter fidelity fixes, checked against the
//! unmodified US ROM (`SSB64_ROM`).

use ssb_rom::archive::Archive;

#[path = "common/slot_statuses.rs"]
mod slot_statuses;
use slot_statuses::SLOT_STATUSES;

fn rom() -> Option<Vec<u8>> {
    Some(std::fs::read(ssb_tablegen::test_rom(env!("CARGO_MANIFEST_DIR"))?).unwrap())
}

/// `dLBCommonSinLookup` (`0x800D4CA0`): every one of its 1024 words is the
/// one `ssb_engine::math::lb_sine_sample` returns (generated from the ROM
/// by `ssb-tablegen`), so the collision matrices
/// (`gmCollisionTransformMatrixAll`) read the ROM's own sine. This finds
/// the table by its words rather than its address, so it also checks the
/// generator's address and overlay.
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
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
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn donkey_hand_slap_loop_is_34_frames() {
    if let Some(frames) = donkey_loop_frames("DonkeySpecialLwLoop") {
        assert_eq!(frames, ssb_game::status::DONKEY_SPECIAL_LW_LOOP_FRAMES);
    }
}

/// The Giant Punch charge loop wraps every 12 frames, the period
/// `ftDonkeySpecialNLoopProcUpdate` counts charge levels by.
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
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
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
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

/// `dFTCommonNullStatusDescs`: ROM offset of status 0's `FTStatusDesc`.
const COMMON_STATUS_DESCS: usize = 0x0A_45D8;
/// `nFTCommonStatusSpecialStart`, the first fighter-specific status.
const SPECIAL_START: usize = 220;
/// `nFTCommonStatusLandingAirN`, `LandingAirLw` and `LandingAirNull`.
const LANDING_AIR: std::ops::RangeInclusive<usize> = 214..=218;
const LANDING_AIR_NULL: usize = 219;

/// ROM offsets of each fighter's `dFT<F>SpecialStatusDescs` and
/// `dFT<F>MotionDescs` (`ssb-tablegen` `stat_flags.rs`, `fighters.rs`).
const DESCS: [(&str, usize, usize); 12] = [
    ("Mario", 0x0A_5708, 0x09_2680),
    ("Fox", 0x0A_5A14, 0x09_44B0),
    ("Donkey", 0x0A_57BC, 0x09_5A30),
    ("Samus", 0x0A_5C1C, 0x09_7AD0),
    ("Luigi", 0x0A_5CF8, 0x09_8F10),
    ("Link", 0x0A_5DAC, 0x09_A330),
    ("Yoshi", 0x0A_66F8, 0x09_B810),
    ("Captain", 0x0A_657C, 0x09_CC90),
    ("Kirby", 0x0A_5F00, 0x09_E190),
    ("Pikachu", 0x0A_6810, 0x09_FB00),
    ("Purin", 0x0A_6978, 0x0A_0FB0),
    ("Ness", 0x0A_6AB8, 0x0A_2450),
];

/// Every fighter-specific slot plays the clip of the motion its status's
/// ROM descriptor names (`FTStatusDesc.mflags >> 6`, then the fighter's
/// motion descriptor's `anim_file`), as `ftMainSetStatus` loads it. The old
/// fixed slots played the next motion's clip: Fox's reflector loop played
/// `SpecialLwEnd`, which raised joint 4 and the reflector by ~58 units and
/// reflected PK Thunder three frames early (RE-566).
///
/// A `LandingAirX` whose motion has no clip is never entered:
/// `ftCommonAttackAirProcMap` takes `LandingAirNull` instead, so its slot
/// holds that clip (Ness's `LandingAirLw`).
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn special_slots_play_their_status_descs_motion() {
    let Some(rom) = rom() else {
        return;
    };
    let be16 = |at: usize| i16::from_be_bytes([rom[at], rom[at + 1]]);
    let be32 = |at: usize| u32::from_be_bytes(rom[at..at + 4].try_into().unwrap());
    let status_file = |status_descs: usize, motions: usize, status: usize| {
        let desc = if status < SPECIAL_START {
            COMMON_STATUS_DESCS + status * 0x14
        } else {
            status_descs + (status - SPECIAL_START) * 0x14
        };
        let motion = be16(desc) >> 6;
        assert!(motion >= 0, "status {status} has no motion ({motion})");
        (motion, be32(motions + motion as usize * 12))
    };
    // One row per `Recipe::Special` slot (413), plus Mario's seven for Luigi.
    let slots: std::collections::BTreeSet<_> = SLOT_STATUSES.iter().map(|r| r.0).collect();
    assert_eq!(slots.len(), 413);
    for &(slot_name, name, status) in SLOT_STATUSES.iter() {
        let (_, status_descs, motions) = DESCS.iter().find(|d| d.0 == name).unwrap();
        let (mut motion, mut file) = status_file(*status_descs, *motions, status);
        if file == 0 && LANDING_AIR.contains(&status) {
            (motion, file) = status_file(*status_descs, *motions, LANDING_AIR_NULL);
        }
        let slot = ssb_rom::anim::SLOT_NAMES
            .iter()
            .position(|n| *n == slot_name)
            .unwrap_or_else(|| panic!("{slot_name} is a slot"));
        let fighter = ssb_rom::anim::FIGHTER_ANIMS
            .iter()
            .find(|f| f.name == name)
            .unwrap();
        assert_ne!(file, 0, "{name} {slot_name}: status {status} has a clip");
        assert_eq!(
            u32::from(fighter.files[slot]),
            file,
            "{name} {slot_name}: status {status}, motion {motion}"
        );
    }
}
