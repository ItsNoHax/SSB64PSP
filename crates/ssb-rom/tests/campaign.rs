//! RE-451: authored 1P sprites, one-shot cameras and demo clips from the ROM.
use ssb_rom::{anim, campaign as a, pack::Pack, Archive};

fn fixtures() -> Option<(Vec<u8>, Vec<u8>)> {
    let path = std::env::var_os("SSB64_ROM")?;
    let rom = std::fs::read(path).expect("SSB64_ROM");
    let pack = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak"),
    )
    .expect("rebuilt campaign pack");
    Some((rom, pack))
}

#[test]
fn authored_campaign_sprites_keep_their_rom_dimensions_and_attributes() {
    let Some((rom, bytes)) = fixtures() else {
        return;
    };
    let archive = Archive::open(&rom, ssb_rom::rom::identify(&rom).unwrap().region).unwrap();
    let pack = Pack::open(&bytes).unwrap();
    for group in [
        a::INTRO,
        a::NAMES,
        a::PICTURES,
        a::PLATFORM_PICTURE,
        a::CONTINUE,
        a::CLEAR,
        a::SCORE,
        a::OBJECTIVES,
    ] {
        let file = archive.load(group.file).unwrap();
        for &offset in group.offsets {
            let original = ssb_rom::sprite::decode(&file, offset).unwrap();
            let packed = pack.sprite(group.file, offset).unwrap();
            assert_eq!(
                (packed.width as u32, packed.height as u32),
                (original.image.width, original.image.height),
                "{} {offset:x}",
                group.file
            );
            assert_eq!(packed.attr, original.attr);
            assert!(pack
                .texture_data(&pack.texture(packed.texture).unwrap())
                .is_some());
        }
    }
    for &offset in &a::BONUS_LABELS {
        assert!(pack.sprite(80, offset).is_some());
    }
}

#[test]
fn camera_initial_play_matches_independent_instant_command_values() {
    let Some((rom, bytes)) = fixtures() else {
        return;
    };
    let archive = Archive::open(&rom, ssb_rom::rom::identify(&rom).unwrap().region).unwrap();
    let file = archive.load(a::INTRO.file).unwrap();
    let pack = Pack::open(&bytes).unwrap();
    for (i, &offset) in a::CAMERA_OFFSETS.iter().enumerate() {
        let mut at = offset as usize;
        let mut expected = [0.0, 0.0, 1500.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 30.0];
        loop {
            let word = u32::from_be_bytes(file.data[at..at + 4].try_into().unwrap());
            at += 4;
            let opcode = word >> 25;
            if opcode == 0 {
                break;
            }
            // These are instant sets, without camera interpolation pointers,
            // jumps, followed by a positive wait. Reusing the scalar joint
            // interpreter is valid only while this remains true.
            if opcode == 2 {
                assert!(word & 0x7fff > 0);
                break;
            }
            assert!(matches!(opcode, 10 | 11), "camera {i}: opcode {opcode}");
            assert_eq!(word & 0x7fff, 0, "camera {i}: duration");
            for (track, value) in expected.iter_mut().enumerate() {
                if word & (1 << (15 + track)) != 0 {
                    *value = f32::from_be_bytes(file.data[at..at + 4].try_into().unwrap());
                    at += 4;
                }
            }
        }
        assert_eq!(a::initial_camera(&file.data, offset).unwrap(), expected);
        assert_eq!(a::packed_camera(&pack, i).unwrap(), expected);
        assert!(expected.iter().all(|v| v.is_finite()));
        assert_eq!(expected[8], 0.0, "camera roll {i}");
        assert!(expected[9] > 0.0 && expected[9] < 180.0);
    }
    assert!(a::packed_camera(&pack, a::CAMERA_OFFSETS.len()).is_none());
}

#[test]
fn playable_fighters_have_intro_and_continue_figatrees() {
    let Some((rom, bytes)) = fixtures() else {
        return;
    };
    let archive = Archive::open(&rom, ssb_rom::rom::identify(&rom).unwrap().region).unwrap();
    let pack = Pack::open(&bytes).unwrap();
    for fighter in 0..12 {
        for slot in anim::SLOT_FIGURE_DROPPED..=anim::SLOT_INTRO_R {
            assert!(!anim::is_anim_joint_slot(slot));
            // Falcon, Jigglypuff and Ness have null opponent-card rows.
            // The campaign has no opponent cards for these fighters.
            if matches!(fighter, 7 | 10 | 11) && slot == anim::SLOT_INTRO_R {
                assert!(pack.fighter_anim(fighter, slot as u32).is_none());
                continue;
            }
            let clip = pack
                .fighter_anim(fighter, slot as u32)
                .unwrap_or_else(|| panic!("fighter {fighter} slot {slot}"));
            assert!(clip.joint_count > 0);
            let file = archive
                .load(anim::FIGHTER_ANIMS[fighter as usize].files[slot] as u32)
                .unwrap();
            assert_eq!(pack.anim_script(&clip).unwrap(), file.data);
        }
    }
}

#[test]
fn frozen_yoshi_cards_use_distinct_authored_frames_without_advancing() {
    let Some((_, bytes)) = fixtures() else { return };
    let pack = Pack::open(&bytes).unwrap();
    let clip = pack.fighter_anim(6, anim::SLOT_INTRO_R as u32).unwrap();
    let script = pack.anim_script(&clip).unwrap();
    let object = pack
        .object(ssb_rom::scene_deps::fighter_object(&pack, 6).unwrap())
        .unwrap();
    let mut cards = Vec::new();
    for frame in 0..18 {
        let mut skeleton = ssb_rom::skeleton::Skeleton::new();
        skeleton.start(&pack, &clip, frame as f32, 0.0);
        skeleton
            .tick_scaled(script, pack.fighter_translate_scales(6), object.first_node)
            .unwrap();
        let mut pose = [ssb_rom::scene::Mat4::IDENTITY; ssb_rom::skeleton::MAX_NODES];
        let count = skeleton.compose(&pack, &object, &mut pose);
        let before = pose[..count].to_vec();
        for _ in 0..20 {
            skeleton
                .tick_scaled(script, pack.fighter_translate_scales(6), object.first_node)
                .unwrap();
        }
        skeleton.compose(&pack, &object, &mut pose);
        assert_eq!(before, pose[..count], "card {frame}");
        cards.push(before);
    }
    assert!(cards.windows(2).all(|c| c[0] != c[1]));
}

#[test]
fn continue_drop_moves_from_authored_first_sample_to_the_floor() {
    let Some((_, bytes)) = fixtures() else { return };
    let pack = Pack::open(&bytes).unwrap();
    for kind in 0..12 {
        let clip = pack
            .fighter_anim(kind, anim::SLOT_FIGURE_DROPPED as u32)
            .unwrap();
        let script = pack.anim_script(&clip).unwrap();
        let object = pack
            .object(ssb_rom::scene_deps::fighter_object(&pack, kind).unwrap())
            .unwrap();
        let mut skeleton = ssb_rom::skeleton::Skeleton::new();
        skeleton.start(&pack, &clip, 0.0, 1.0);
        skeleton
            .tick_scaled(
                script,
                if kind == 4 {
                    None
                } else {
                    pack.fighter_translate_scales(kind)
                },
                object.first_node,
            )
            .unwrap();
        let first = skeleton.pose(0).unwrap().translate;
        // ROM samples at frame zero: Mario/Luigi/Ness share the dropped
        // figure; Link's translation differs from the other eight fighters.
        let expected = match kind {
            0 | 4 | 11 => [40.25, 1949.5, -70.25],
            5 => [51.0, 1952.5, -101.5],
            _ => [51.0, 1955.25, -86.25],
        };
        assert_eq!(first, expected, "kind {kind}");
        for _ in 0..135 {
            skeleton
                .tick_scaled(
                    script,
                    pack.fighter_translate_scales(kind),
                    object.first_node,
                )
                .unwrap();
        }
        assert!(anim::LEADING_RUNTIME_JOINT[kind as usize][anim::SLOT_FIGURE_DROPPED]);
        let last = skeleton.pose(0).unwrap().translate;
        assert_eq!(last, [0.0; 3], "kind {kind}");
        let rest_y = match kind {
            0 | 4 | 11 => 120.5,
            5 => 117.5,
            _ => 114.75,
        };
        assert_eq!(
            2070.0 + last[1] - first[1],
            rest_y,
            "integrated drop {kind}"
        );
    }
}
