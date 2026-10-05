//! Master Hand's ROM-backed constants: his `AnimJoint` clip lengths, the
//! attack-choice bytes `ftBossWaitDecideStatusComputer` reads and his
//! animation slots.

use ssb_game::boss;
use ssb_rom::anim::{
    is_anim_joint_slot, BOSS_SLOTS, FIGHTER_ANIMS, SLOT_BOSS_DEFAULT, SLOT_COUNT, SLOT_NAMES,
};
use ssb_rom::{rom, Archive};

const BOSS: usize = 12;

fn rom_bytes() -> Option<Vec<u8>> {
    let path = std::env::var_os("SSB64_ROM")?;
    std::fs::read(path).ok()
}

/// The tick every joint stream of a 32-bit clip ends on, less one, as
/// `anim::decode_fighter` reads the entry clips (RE-401).
fn anim_joint_frames(archive: &Archive<'_>, file: u32) -> f32 {
    let f = archive.load(file).unwrap();
    let len = ssb_rom::anim::joint_table_len(&f.data).unwrap();
    let ends: Vec<u32> = ssb_rom::objanim::joint_scripts(&f.data, 0, len)
        .into_iter()
        .flatten()
        .map(|at| ssb_rom::objanim::script_end_tick(&f.data, at, 4096).unwrap())
        .collect();
    assert!(
        ends.iter().all(|&e| e == ends[0]),
        "file {file}: uneven ends {ends:?}"
    );
    (ends[0] - 1) as f32
}

#[test]
fn the_slots_follow_the_boss_motion_order() {
    assert_eq!(boss::SLOT_DEFAULT, SLOT_BOSS_DEFAULT);
    assert_eq!(SLOT_BOSS_DEFAULT + BOSS_SLOTS, SLOT_COUNT);
    assert_eq!(SLOT_NAMES[SLOT_BOSS_DEFAULT + 17], "BossDrill");
    let row = &FIGHTER_ANIMS[BOSS];
    assert_eq!(row.name, "Boss");
    // `dFTBossMotionDescs`: Default 2098 .. Appear 2131, with SlamStart
    // (2124) for Okupunch1 and DyingStart's file 2130 for DeadRight.
    assert_eq!(row.files[SLOT_BOSS_DEFAULT], 2098);
    assert_eq!(row.files[SLOT_BOSS_DEFAULT + 22], 2124);
    assert_eq!(row.files[SLOT_COUNT - 1], 2131);
    for other in FIGHTER_ANIMS.iter().filter(|a| a.name != "Boss") {
        assert!(other.files[SLOT_BOSS_DEFAULT..].iter().all(|&f| f == 0));
    }
    let joint: Vec<usize> = (0..BOSS_SLOTS)
        .filter(|&i| is_anim_joint_slot(SLOT_BOSS_DEFAULT + i))
        .collect();
    assert_eq!(joint, [3, 4, 5, 17, 23, 24, 25, 29]);
}

#[test]
fn the_anim_joint_lengths_match_the_rom() {
    let Some(bytes) = rom_bytes() else { return };
    let info = rom::identify(&bytes).unwrap();
    let archive = Archive::open(&bytes, info.region).unwrap();
    for (file, frames) in [
        (2101, boss::LAUNCH_FRAMES),
        (2102, boss::FLY_FRAMES),
        (2103, boss::LANDING_FRAMES),
        (2115, boss::DRILL_FRAMES),
        (2125, boss::PUNCH3_FRAMES),
        (2126, boss::PUNCH_END_FRAMES),
        (2127, boss::SLAM_FRAMES),
        (2131, boss::APPEAR_FRAMES),
    ] {
        assert_eq!(anim_joint_frames(&archive, file), frames, "file {file}");
    }
}

/// `ftbosswait.o`'s `.data` at VRAM 0x80188DC0 lies in `ovl3`, whose
/// segment starts at ROM 0x0AC540 for VRAM 0x80131B00
/// (`smashbrothers.us.yaml`); the next 16 bytes are `ftkirbyspecialn.o`'s.
#[test]
fn the_attack_choice_bytes_match_the_rom() {
    let Some(bytes) = rom_bytes() else { return };
    let at = 0x0AC540 + (0x8018_8DC0 - 0x8013_1B00);
    assert_eq!(&bytes[at..at + boss::WAIT_DATA.len()], &boss::WAIT_DATA);
}

fn last_file(bytes: &[u8]) -> ssb_rom::archive::File {
    let info = rom::identify(bytes).unwrap();
    let archive = Archive::open(bytes, info.region).unwrap();
    archive.load(ssb_game::spgame::boss::EFFECT_FILE).unwrap()
}

/// `gcPlayAnimAll`'s `gobj->anim_frame` after each play of one script at
/// `speed`, the make's play first.
fn gobj_frames(data: &[u8], script: u32, speed: f32, plays: usize) -> Vec<Option<f32>> {
    let mut j = ssb_rom::objanim::StageJoint::start_changed(script, 0.0);
    let mut pose = ssb_rom::figatree::JointPose::default();
    (0..plays)
        .map(|_| {
            j.tick(data, speed, &mut pose).unwrap();
            j.gobj_frame()
        })
        .collect()
}

#[test]
fn the_wallpaper_effects_loop_and_end_as_the_controller_reads() {
    let Some(bytes) = rom_bytes() else { return };
    let f = last_file(&bytes);
    let graphs = ssb_rom::scene::find_scene_graphs(&f);
    let nodes = |graph: u32| {
        graphs
            .iter()
            .find(|g| g.offset == graph)
            .unwrap()
            .nodes
            .len()
    };
    let comets = ssb_game::spgame::boss::EFFECT_ASSETS[0][0].unwrap();
    let scripts: Vec<u32> =
        ssb_rom::objanim::joint_scripts(&f.data, comets.anim_joint.unwrap(), nodes(comets.graph))
            .into_iter()
            .flatten()
            .collect();
    assert_eq!(scripts.len(), 3);
    for &script in &scripts {
        // At speed 1 the clock reads 1, 2, ... 148, then 0: the loop the
        // comets restart on.
        let frames = gobj_frames(&f.data, script, 1.0, 300);
        let loop_at = boss_wallpaper_loop(&frames);
        assert_eq!(loop_at, ssb_game::spgame::boss::COMET_LOOP_FRAMES as usize);
    }
    let closing = ssb_game::spgame::boss::EFFECT_ASSETS[3][1].unwrap();
    let scripts: Vec<u32> =
        ssb_rom::objanim::joint_scripts(&f.data, closing.anim_joint.unwrap(), nodes(closing.graph))
            .into_iter()
            .flatten()
            .collect();
    for &script in &scripts {
        let mut j = ssb_rom::objanim::StageJoint::start_changed(script, 0.0);
        let mut pose = ssb_rom::figatree::JointPose::default();
        let mut plays = 0;
        while !j.ended() {
            j.tick(&f.data, 1.0, &mut pose).unwrap();
            plays += 1;
            assert!(plays < 1000);
        }
        assert_eq!(plays, ssb_game::spgame::boss::CLOSING_FRAMES);
    }
}

/// The play on which the clock first returns to zero or below.
fn boss_wallpaper_loop(frames: &[Option<f32>]) -> usize {
    (1..frames.len())
        .find(|&i| frames[i].is_some_and(|f| f <= 0.0))
        .unwrap()
}

/// `sc1PGameWaitStageBossUpdate`'s and `sc1PGameBossDefeatInterfaceProcSet`'s
/// camera animations (file 114 + 0x6010 and + 0x6450): plain eye and look-at
/// tracks without a path, 601 and 541 plays long.
#[test]
fn the_boss_camera_animations_end_on_their_own() {
    let Some(bytes) = rom_bytes() else { return };
    let f = last_file(&bytes);
    for (offset, plays) in [
        (ssb_rom::campaign::BOSS_INTRO_CAMERA, 601),
        (ssb_rom::campaign::BOSS_DEFEAT_CAMERA, 541),
    ] {
        let frames = ssb_rom::campaign::camera_frames(&f.data, offset).unwrap();
        assert_eq!(frames.len(), plays, "camera {offset:#x}");
    }
    for offset in [
        ssb_rom::campaign::BOSS_INTRO_CAMERA,
        ssb_rom::campaign::BOSS_DEFEAT_CAMERA,
    ] {
        let mut j = ssb_rom::objanim::StageJoint::start_changed(offset, 0.0);
        let mut pose = ssb_rom::figatree::JointPose::default();
        while !j.ended() {
            j.tick(&f.data, 1.0, &mut pose).unwrap();
            assert!(j.interp().is_none());
        }
    }
    let intro =
        ssb_rom::campaign::camera_frames(&f.data, ssb_rom::campaign::BOSS_INTRO_CAMERA).unwrap();
    assert_eq!(intro[0], [-600.0, 300.0, -2550.0, -750.0, 300.0, 0.0, 25.0]);
}

fn pack_bytes() -> Option<Vec<u8>> {
    std::env::var_os("SSB64_ROM")?;
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak");
    std::fs::read(path).ok()
}

/// Every Master Hand motion is packed over his model, the `AnimJoint`
/// clips at their ROM lengths, and both camera animations are baked.
#[test]
fn the_pack_carries_master_hands_motions_and_cameras() {
    let Some(bytes) = pack_bytes() else { return };
    let pack = ssb_rom::pack::Pack::open(&bytes).unwrap();
    let joint_frames = [
        (3, boss::LAUNCH_FRAMES),
        (4, boss::FLY_FRAMES),
        (5, boss::LANDING_FRAMES),
        (17, boss::DRILL_FRAMES),
        (23, boss::PUNCH3_FRAMES),
        (24, boss::PUNCH_END_FRAMES),
        (25, boss::SLAM_FRAMES),
        (29, boss::APPEAR_FRAMES),
    ];
    for i in 0..BOSS_SLOTS {
        let slot = (SLOT_BOSS_DEFAULT + i) as u32;
        let anim = pack
            .fighter_anim(BOSS as u32, slot)
            .unwrap_or_else(|| panic!("{} missing", SLOT_NAMES[slot as usize]));
        assert_eq!(
            anim.source_file,
            u32::from(FIGHTER_ANIMS[BOSS].files[slot as usize])
        );
        if let Some(&(_, frames)) = joint_frames.iter().find(|(j, _)| *j == i) {
            assert_eq!(anim.frames as f32, frames, "{}", SLOT_NAMES[slot as usize]);
        }
    }
    for (slot, plays) in [
        (ssb_rom::campaign::BOSS_INTRO_CAMERA_SLOT, 601),
        (ssb_rom::campaign::BOSS_DEFEAT_CAMERA_SLOT, 541),
    ] {
        assert_eq!(
            ssb_rom::campaign::packed_camera_frames(&pack, slot)
                .unwrap()
                .count(),
            plays
        );
    }
    assert!((0..pack.object_count())
        .filter_map(|i| pack.object(i))
        .any(|o| o.source_file == 344 && o.source_offset == 0x2CB8));
}
