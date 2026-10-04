//! The display effects' lives (`ssb_game::effect::life`, RE-415): the
//! `gcPlayAnimAll` call on which each effect's animation reaches its end and
//! its `proc_update` ejects it, replayed from the ROM's `AObjEvent32`
//! scripts the way `gcPlayAnimAll` runs them.
//!
//! `efManagerMakeEffect` adds the animations with `anim_frame` 0
//! (`AOBJ_ANIM_CHANGED`) and plays them once; each later frame's
//! `proc_update` plays them again and ejects the effect when the `GObj`'s
//! `anim_frame` (the value the last animated `DObj` in tree order wrote) is
//! at or below zero. `efManagerVelAddDestroyAnimEnd` (the small shock) reads
//! its first `MObj`'s clock instead: its script loops (`SetAnim`), which sets
//! the clock to `-anim_wait`, zero at a whole-frame wait.

use ssb_game::effect::{life, DisplayKind};
use ssb_rom::figatree::JointPose;
use ssb_rom::objanim::{joint_scripts, StageJoint};
use ssb_rom::pack::Pack;
use ssb_rom::Archive;

fn rom() -> Option<Vec<u8>> {
    std::fs::read(std::env::var_os("SSB64_ROM")?).ok()
}

fn pack_bytes() -> Option<Vec<u8>> {
    std::env::var_os("SSB64_ROM")?;
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak");
    std::fs::read(path).ok()
}

#[test]
fn scale_x_effects_draw_on_head_one_under_the_effect_links_cld_state() {
    use ssb_rom::pack::flags;
    let Some(bytes) = pack_bytes() else {
        return;
    };
    let p = Pack::open(&bytes).unwrap();
    for key in [(83, 0x7E80), (83, 0x8FA0), (84, 0x1500)] {
        let o = (0..p.object_count())
            .filter_map(|i| p.object(i))
            .find(|o| (o.source_file, o.source_offset) == key)
            .unwrap();
        let m = p.mesh(p.node(o.first_node).unwrap().mesh).unwrap();
        for pr in (0..m.prim_count).map(|i| p.prim(m.first_prim + i).unwrap()) {
            assert_ne!(pr.flags & flags::HEAD1, 0, "{key:?}");
            assert_ne!(pr.flags & flags::ALPHA_BLEND, 0, "{key:?}");
            assert_eq!(
                pr.flags & (flags::DEPTH_TEST | flags::DEPTH_WRITE),
                0,
                "{key:?}"
            );
        }
    }
}

/// The play on which a `DObj` tree's `GObj::anim_frame` first reads at or
/// below zero after creation, counting creation's play as 1.
fn dobj_life(data: &[u8], scripts: &[u32]) -> u16 {
    let mut joints: Vec<StageJoint> = scripts
        .iter()
        .map(|&s| StageJoint::start_changed(s, 0.0))
        .collect();
    let mut poses = vec![
        JointPose {
            rotate: [0.0; 3],
            translate: [0.0; 3],
            scale: [1.0; 3],
        };
        joints.len()
    ];
    let mut frame = 0.0;
    for play in 1..=600u16 {
        for (j, pose) in joints.iter_mut().zip(poses.iter_mut()) {
            j.tick(data, 1.0, pose).unwrap();
            if let Some(f) = j.gobj_frame() {
                frame = f;
            }
        }
        if play > 1 && frame <= 0.0 {
            return play;
        }
    }
    panic!("the animation never ends");
}

/// The play on which a material script first loops (`SetAnim` or `Jump`)
/// or ends: one for creation's play, then one a frame of every blocking
/// command's wait. `SetAnim` leaves `anim_frame = -anim_wait`, zero there.
fn mat_loop_play(data: &[u8], script: u32) -> u16 {
    let word = |at: usize| u32::from_be_bytes(data[at..at + 4].try_into().unwrap());
    let mut at = script as usize;
    let mut wait = 0u32;
    for _ in 0..256 {
        let w = word(at);
        let (opcode, flags, payload) = (w >> 25, (w >> 15) & 0x3FF, w & 0x7FFF);
        at += 4;
        let values = |per: u32| (flags.count_ones() * per) as usize * 4;
        match opcode {
            // `End`, `Jump`, `SetAnim`.
            0 | 1 | 14 => return (wait + 1) as u16,
            // `Wait`.
            2 => wait += payload,
            // `SetValBlock`, `SetVal0RateBlock`, `SetValAfterBlock`.
            3 | 8 | 10 => {
                wait += payload;
                at += values(1);
            }
            // `SetValRateBlock`.
            5 => {
                wait += payload;
                at += values(2);
            }
            // `SetValRate`.
            6 => at += values(2),
            // `SetVal`, `SetTargetRate`, `SetVal0Rate`, `SetValAfter`.
            4 | 7 | 9 | 11 => at += values(1),
            other => panic!("material opcode {other} at {at:#x}"),
        }
    }
    panic!("the material script never loops");
}

/// The packed manager effect `key`'s transform scripts, in node order.
fn packed_scripts<'a>(pack: &Pack<'a>, key: (u32, u32)) -> (&'a [u8], Vec<u32>) {
    let slot = ssb_rom::effect::MANAGER_EFFECT_KEYS
        .iter()
        .position(|&k| k == key)
        .unwrap() as u32;
    let anim = pack.effect_anim(slot).unwrap();
    let data = pack.anim_script(&anim).unwrap();
    let mut joints: Vec<_> = (0..anim.joint_count)
        .filter_map(|i| pack.anim_joint(anim.first_joint + i))
        .filter(|j| j.script != ssb_rom::pack::AnimJoint::NO_SCRIPT)
        .map(|j| (j.node, j.script))
        .collect();
    joints.sort_unstable();
    (data, joints.into_iter().map(|(_, s)| s).collect())
}

#[test]
fn display_effect_lives_match_the_rom() {
    let (Some(rom), Some(pack)) = (rom(), pack_bytes()) else {
        return;
    };
    let pack = Pack::open(&pack).unwrap();
    let info = ssb_rom::rom::identify(&rom).unwrap();
    let archive = Archive::open(&rom, info.region).unwrap();

    let mut measured = Vec::new();
    for (name, key, kind) in [
        ("slash", (83, 0x7750), DisplayKind::Slash),
        ("impact wave", (83, 0x7C28), DisplayKind::ImpactWave),
        ("common spark", (83, 0x8FA0), DisplayKind::FlySparks),
        ("metal dust", (83, 0xCAC8), DisplayKind::FlyMDust),
        ("fire spark", (84, 0x2040), DisplayKind::FireSpark),
        ("Poké Ball rays", (85, 0x0628), DisplayKind::MBallRays),
    ] {
        let (data, scripts) = packed_scripts(&pack, key);
        let n = dobj_life(data, &scripts);
        measured.push((name, n, kind.life().unwrap()));
    }
    // The quakes' single `DObj` (`gcAddDObjForGObj(effect_gobj, NULL)`).
    let effects1 = archive.load(83).unwrap();
    for (m, table) in [0xCBC0u32, 0xCC20, 0xCCF0].into_iter().enumerate() {
        let script = joint_scripts(&effects1.data, table, 1)[0].unwrap();
        let n = dobj_life(&effects1.data, &[script]);
        let kind = DisplayKind::Quake { magnitude: m as u8 };
        measured.push(("quake", n, kind.life().unwrap()));
    }
    // RE-416: the packed quakes (their node-less `AnimDesc::EFFECT` slots)
    // replay the archive's scripts.
    for m in 0..3u32 {
        let anim = pack
            .effect_anim(ssb_rom::effect::QUAKE_ANIM_SLOT + m)
            .expect("packed quake");
        let joint = pack.anim_joint(anim.first_joint).unwrap();
        assert_eq!(joint.node, ssb_rom::pack::AnimJoint::NO_NODE);
        let data = pack.anim_script(&anim).unwrap();
        let n = dobj_life(data, &[joint.script]);
        let kind = DisplayKind::Quake { magnitude: m as u8 };
        measured.push(("packed quake", n, kind.life().unwrap()));
    }
    // The small shock's first `MObj`: `llEFCommonEffects2ShockSmallMatAnimJoint`
    // is `DObj -> MObj -> script`.
    let effects2 = archive.load(84).unwrap();
    let at = |o: u32| {
        u32::from_be_bytes(
            effects2.data[o as usize..o as usize + 4]
                .try_into()
                .unwrap(),
        )
    };
    let script = at(at(0x1570));
    let n = mat_loop_play(&effects2.data, script);
    measured.push(("small shock", n, DisplayKind::ShockSmall.life().unwrap()));
    for (name, got, want) in &measured {
        eprintln!("{name}: {got} plays (life {want})");
    }
    for (name, got, want) in measured {
        assert_eq!(got, want, "{name}");
    }
    // The constants the module names are the ones checked.
    assert_eq!(life::QUAKE.len(), 3);
}

/// RE-417: the Egg Lay egg's animations. The packed Throw table (the
/// `EFDesc`'s own) ends on `EGG_THROW_PLAYS`, and the packed Wait and Break
/// tables (`dEFManagerYoshiEggLayAnimJoints`) replay the archive's: Break
/// ends on its tenth play after the set (`EGG_BREAK_FRAMES`), and Wait loops.
#[test]
fn egg_lay_animations_match_the_rom() {
    use ssb_game::capture_yoshi::{EGG_BREAK_FRAMES, EGG_THROW_PLAYS};
    let (Some(rom), Some(pack)) = (rom(), pack_bytes()) else {
        return;
    };
    let pack = Pack::open(&pack).unwrap();
    let info = ssb_rom::rom::identify(&rom).unwrap();
    let archive = Archive::open(&rom, info.region).unwrap();
    let (data, scripts) = packed_scripts(&pack, ssb_rom::effect::YOSHI_EGG_LAY_KEY);
    assert_eq!(scripts.len(), 2);
    let throw = dobj_life(data, &scripts);
    eprintln!("egg throw: {throw} plays");
    assert_eq!(throw, EGG_THROW_PLAYS);

    let file = archive.load(ssb_rom::effect::YOSHI_EGG_LAY_KEY.0).unwrap();
    let object = (0..pack.object_count())
        .filter_map(|i| pack.object(i))
        .find(|o| (o.source_file, o.source_offset) == ssb_rom::effect::YOSHI_EGG_LAY_KEY)
        .unwrap();
    for (i, &table) in ssb_rom::effect::YOSHI_EGG_LAY_ANIM_JOINTS
        .iter()
        .enumerate()
    {
        let archived: Vec<u32> = joint_scripts(&file.data, table, 2)
            .into_iter()
            .flatten()
            .collect();
        let anim = pack
            .effect_anim(ssb_rom::effect::YOSHI_EGG_LAY_ANIM_SLOT + i as u32)
            .expect("packed egg animation");
        let mut joints: Vec<_> = (0..anim.joint_count)
            .filter_map(|j| pack.anim_joint(anim.first_joint + j))
            .map(|j| (j.node - object.first_node, j.script))
            .collect();
        joints.sort_unstable();
        assert_eq!(joints.iter().map(|j| j.0).collect::<Vec<_>>(), vec![0, 1]);
        let packed: Vec<u32> = joints.iter().map(|j| j.1).collect();
        let data = pack.anim_script(&anim).unwrap();
        // Same bytes at the packed offsets as at the archive's.
        for (p, a) in packed.iter().zip(&archived) {
            assert_eq!(
                data[*p as usize..*p as usize + 16],
                file.data[*a as usize..*a as usize + 16]
            );
        }
        if i == 1 {
            // Set on a live effect, so its first play is the set's: one
            // fewer than `dobj_life`'s count, which includes a make.
            let n = dobj_life(data, &packed) - 1;
            eprintln!("egg break: {n} plays");
            assert_eq!(n, u16::from(EGG_BREAK_FRAMES));
        }
    }
}
