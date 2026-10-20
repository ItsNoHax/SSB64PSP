//! [`PosedFighter::place_played_spawn`] plays a replaced status's motion
//! aside, re-places the weapon from it and puts back everything the aside
//! play wrote (D-052).

extern crate std;

use std::format;
use std::string::String;
use std::vec::Vec;

use ssb_engine::input::ControllerState;
use ssb_engine::math::Vec3;
use ssb_game::fighter::{Fighter, FighterKind};
use ssb_game::spawn_pose::played_status;
use ssb_game::weapon::MapSurface;

use super::PosedFighter;

fn open_pack() -> Option<Vec<u8>> {
    ssb_tablegen::test_rom(env!("CARGO_MANIFEST_DIR"))?;
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak"),
    )
    .ok()
}

/// Pikachu's air Thunder, ticked until `SpecialAirLwStart` has made the
/// Thunder and its anim end set the Loop over it
/// (ftpikachuspeciallw.c:21-58): returned right after that interrupt, the
/// Start's play still to be placed from.
fn thunder_replaced(pack: &ssb_rom::pack::Pack<'_>) -> PosedFighter {
    let mut f = Fighter::new(FighterKind::Pikachu, 0, 3);
    f.attributes.size = pack.fighter(FighterKind::Pikachu as u32).unwrap().size;
    f.pikachu.map_bound_top = Some(5000.0);
    ssb_game::status::set_fall(&mut f);
    ssb_game::pikachu::set_special_lw(&mut f);
    let mut p = PosedFighter::new(pack, f);
    let surfaces = core::iter::empty::<MapSurface>;
    for _ in 0..80 {
        p.before_interrupt(Some(5000.0), ControllerState::default(), false, false);
        p.fighter.tick_interrupt(&surfaces);
        if played_status(&p.fighter).is_some() {
            return p;
        }
        p.after_interrupt(pack);
        p.tick_physics(pack, &surfaces);
    }
    panic!("no Thunder made over a replaced status");
}

/// The fighter as `Debug` text with its weapon's placement left out.
fn without_spawn(f: &Fighter) -> String {
    let mut f = f.clone();
    f.weapon_spawn = None;
    f.weapon_spawn_pose = None;
    format!("{f:?}")
}

/// Everything but the weapon's placement comes back as it was.
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn place_played_spawn_moves_only_the_weapon() {
    let Some(bytes) = open_pack() else {
        return;
    };
    let pack = ssb_rom::pack::Pack::open(&bytes).unwrap();
    let mut p = thunder_replaced(&pack);
    let pose_before = format!("{:?}", p.snapshot());
    let fighter_before = without_spawn(&p.fighter);
    let spawn_before = p.fighter.weapon_spawn.expect("the Thunder is queued");
    assert!(
        p.fighter.weapon_spawn_pose.is_some(),
        "its placement is pending"
    );

    p.place_played_spawn(&pack);

    assert!(
        p.fighter.weapon_spawn_pose.is_none(),
        "one placement per make"
    );
    let spawn_after = p.fighter.weapon_spawn.expect("the Thunder stays queued");
    assert_ne!(
        spawn_after.position, spawn_before.position,
        "placed from the aside play"
    );
    assert_eq!(format!("{:?}", p.snapshot()), pose_before);
    assert_eq!(without_spawn(&p.fighter), fighter_before);
}

/// Every [`super::Snapshot`] field the aside play writes is put back:
/// set to values that play would not leave, each survives it. The
/// skeleton and the joints are [`place_played_spawn_moves_only_the_weapon`]'s.
/// `shield_yrotn` only a shield's pose writes, so this play cannot move it.
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn place_played_spawn_restores_what_the_aside_play_writes() {
    let Some(bytes) = open_pack() else {
        return;
    };
    let pack = ssb_rom::pack::Pack::open(&bytes).unwrap();
    let mut p = thunder_replaced(&pack);
    let played = played_status(&p.fighter).unwrap();

    // Values the aside play does not leave.
    p.started = None;
    p.clip_restarts_seen = played.clip_restarts.wrapping_add(1);
    p.root_motion_before_tick = None;
    p.fighter.is_use_animlocks = !p.skeleton.locks.flagged;
    p.fighter.joint_parents = [0xEE; ssb_game::fighter::FIGHTER_JOINTS];
    p.fighter.grab.thrown_part4 = Some(Vec3::new(1.0, 2.0, 3.0));
    let sentinel = |p: &PosedFighter| {
        (
            p.started,
            p.clip_restarts_seen,
            p.root_motion_before_tick,
            p.fighter.is_use_animlocks,
            p.fighter.joint_parents,
            p.fighter.grab.thrown_part4,
        )
    };
    let set = sentinel(&p);

    // The aside play alone moves every one of them.
    let saved = p.snapshot();
    let fighter = p.fighter.clone();
    p.play_aside(&pack, played);
    let aside = sentinel(&p);
    assert_ne!(aside.0, set.0, "started");
    assert_ne!(aside.1, set.1, "clip_restarts_seen");
    assert_ne!(aside.2, set.2, "root_motion_before_tick");
    assert_ne!(aside.3, set.3, "is_use_animlocks");
    assert_ne!(aside.4, set.4, "joint_parents");
    assert_ne!(aside.5, set.5, "thrown_part4");
    p.restore(saved);
    p.fighter = fighter;
    assert_eq!(sentinel(&p), set);

    p.place_played_spawn(&pack);

    assert_eq!(sentinel(&p), set);
}
