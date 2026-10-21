//! `catchslope` replayed with both fighters posed (RE-510): Mario dashes onto
//! Kongo Jungle's left slope and grabs the dummy. `CatchWait` keeps the
//! catch's slope contour (`FTSTATUS_PRESERVE_SLOPECONTOUR`) and stops the
//! clock, so the slope-fitted legs hold still until capture.

use ssb_capture::script;
use ssb_capture::GameScene;
use ssb_game::fighter::FighterKind;
use ssb_game::status::{AnyStatus, Status};
use ssb_rom::figatree::JointPose;

#[path = "common/posed.rs"]
mod posed;
use posed::{tick_pair, Pair, Posed};

#[path = "common/dream_land.rs"]
mod dream_land;
use dream_land::{dream_land_map, stage_floors, training_fighter};

/// First tick of `CatchWait` after its entry pose (host replay: `Catch` 90,
/// `CatchPull` 95, `CatchWait` 97).
const HOLD_FROM: u64 = 98;

fn open_pack() -> Option<Vec<u8>> {
    ssb_tablegen::test_rom(env!("CARGO_MANIFEST_DIR"))?;
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak");
    std::fs::read(path).ok()
}

#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn catch_wait_holds_the_slope_legs() {
    let Some(bytes) = open_pack() else {
        return;
    };
    let pack = ssb_rom::pack::Pack::open(&bytes).unwrap();
    const JUNGLE: u32 = 6;
    let (stage, segments) = stage_floors(&pack, JUNGLE);
    let map = dream_land_map(&pack, &stage);
    let mut fighters: Vec<Posed> = [(FighterKind::Mario, 0), (FighterKind::Mario, 1)]
        .into_iter()
        .map(|(kind, port)| {
            Posed::new(
                &pack,
                training_fighter(&pack, &stage, kind, port, &segments),
            )
        })
        .collect();
    let scene = GameScene::CatchSlope;
    let capture = script::capture_tick(scene).unwrap();
    let pad = |t: u64| {
        let p = script::fighter_pad(scene, t).unwrap();
        (p.buttons, p.stick_x, p.stick_y)
    };
    let poses = |m: &Posed| -> Vec<Option<JointPose>> {
        (0..m.skeleton.joint_count())
            .map(|j| m.skeleton.pose(j).copied())
            .collect()
    };
    let mut pair = Pair::default();
    let mut held: Option<Vec<Option<JointPose>>> = None;
    for tick in 8..=capture {
        let _ = tick_pair(&pack, &map, &mut fighters, &pad, tick, &mut pair);
        let m = &fighters[0];
        if tick < HOLD_FROM {
            continue;
        }
        assert_eq!(
            m.fighter.status.status,
            AnyStatus::Common(Status::CatchWait),
            "tick {tick}"
        );
        assert_ne!(
            m.fighter.slope_contour, 0,
            "tick {tick}: CatchWait keeps the contour"
        );
        let now = poses(m);
        match &held {
            None => held = Some(now),
            Some(first) => assert_eq!(first, &now, "tick {tick}: CatchWait legs moved"),
        }
    }
    let m = &fighters[0];
    assert!(
        (m.fighter.pos.x - -942.92).abs() < 0.05,
        "x {}",
        m.fighter.pos.x
    );
    assert!(
        (m.fighter.pos.y - -100.99).abs() < 0.05,
        "y {}",
        m.fighter.pos.y
    );
    assert_eq!(
        fighters[1].fighter.status.status,
        AnyStatus::Common(Status::CaptureWait)
    );
}
