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

/// One measured N64 frame of `catchslope` (as golden tick `t + 4`):
/// catcher and victim status ids, their TopN `translate` x/y, and the
/// catcher's joints 19-28 (both legs' thighs and knees) `rotate`, as f32
/// bits.
struct Row {
    tick: u64,
    status: [u16; 2],
    pos: [u32; 4],
    legs: [[u32; 3]; 10],
}

/// Mupen64Plus, a Training warp ROM on Kongo Jungle with Mario against a
/// Mario dummy, P1 pad `4:1:a;26:12:x=80;86:1:z+a` (the scene's pad in
/// N64 frames), N64 frames 86-98: `Catch` 86, `CatchPull` 91, `CatchWait`
/// 93, held from 94 (RE-510). Every catcher joint and both positions
/// matched bit for bit through frame 135; the dummy's own idle `Wait`
/// before the catch runs four frames earlier on the warp ROM, which
/// boots into Training with no menu frame (not compared).
#[rustfmt::skip]
const N64: &[Row] = &[
    Row { tick: 90, status: [166, 10], pos: [0xc46bbb08, 0xc2c9fc32, 0xc41d8000, 0xc354d555], legs: [[0xbcb09122, 0xbf283967, 0xbe7a036f], [0x00000000, 0x00000000, 0x3f977a94], [0x00000000, 0xb716feb5, 0xbfd226dd], [0xbf248000, 0xbcb00000, 0xbe9c0000], [0xbfc90fd8, 0x00000000, 0xbfc90fd8], [0x3f2eb6b7, 0x3ebfbee5, 0xbe3fcc6e], [0x00000000, 0x00000000, 0x3fbb1e0c], [0x00000000, 0x3716feb5, 0xbfd2274a], [0x4041ded0, 0xc02cb900, 0x4030a4be], [0x00000000, 0x40490000, 0x00000000]] },
    Row { tick: 91, status: [166, 10], pos: [0xc46bbb08, 0xc2c9fc32, 0xc41d8000, 0xc354d555], legs: [[0x3f1d3f80, 0xbf87af6c, 0xbf88abf9], [0x00000000, 0x00000000, 0x3fe43606], [0x00000000, 0xb716feb5, 0xbfd226dd], [0xbf0c0000, 0xbcafffff, 0xbe9c0000], [0xbfc90fd8, 0x00000000, 0xbfc90fd8], [0x3f66e88f, 0x3e935516, 0xbe836c3c], [0x00000000, 0x00000000, 0x3fb20244], [0x00000000, 0x3716feb5, 0xbfd2274a], [0x40356bda, 0xc020c000, 0x40343097], [0x00000000, 0x40490000, 0x00000000]] },
    Row { tick: 92, status: [166, 10], pos: [0xc46bbb08, 0xc2c9fc32, 0xc41d8000, 0xc354d555], legs: [[0x3e4d3b7c, 0xbf68f1cb, 0xbef36a23], [0x00000000, 0x00000000, 0x3fc9dc72], [0x00000000, 0xb716feb5, 0xbfd226dd], [0xbf3e0000, 0xbcb00000, 0xbe9c0000], [0xbfc90fd8, 0x00000000, 0xbfc90fd8], [0x3f4756e8, 0x3ed91d8d, 0xbea0d8f3], [0x00000000, 0x00000000, 0x3fba4b56], [0x00000000, 0x3716feb5, 0xbfd2274a], [0x40378000, 0xc01bbf00, 0x4034e000], [0x00000000, 0x40490000, 0x00000000]] },
    Row { tick: 93, status: [166, 10], pos: [0xc46bbb08, 0xc2c9fc32, 0xc41d8000, 0xc354d555], legs: [[0xbf36d4b2, 0xbf4a79b8, 0x3e9bfc01], [0x00000000, 0x00000000, 0x3f7c561f], [0x00000000, 0xb716feb5, 0xbfd226dd], [0xbf298000, 0xbcb00000, 0xbe9c0000], [0xbfc90fd8, 0x00000000, 0xbfc90fd8], [0x3ea88581, 0x3efba714, 0xbed5e96c], [0x00000000, 0x00000000, 0x3fbc13ea], [0x00000000, 0x3716feb5, 0xbfd2274a], [0x4048a000, 0xc0202000, 0x40336000], [0x00000000, 0x40490000, 0x00000000]] },
    Row { tick: 94, status: [166, 10], pos: [0xc46bbb08, 0xc2c9fc32, 0xc41d8000, 0xc354d555], legs: [[0xbf923667, 0xbe6f0209, 0x3f2c3fce], [0x00000000, 0x00000000, 0x00000000], [0x00000000, 0xb716feb5, 0xbfd226dd], [0xbf1753ab, 0xbcb00000, 0xbe9c0000], [0xbfc90fd8, 0x00000000, 0xbfc90fd8], [0xbe24fe63, 0x3f08594d, 0xbef3eb32], [0x00000000, 0x00000000, 0x3fa1101b], [0x00000000, 0x3716feb5, 0xbfd2274a], [0x40586f80, 0xc02e8000, 0x403bfa13], [0x00000000, 0x40490000, 0x00000000]] },
    Row { tick: 95, status: [167, 171], pos: [0xc46bbb08, 0xc2c9fc32, 0xc428382f, 0xc345822b], legs: [[0xbf9a1570, 0xbe0e9df8, 0x3f23d312], [0x00000000, 0x00000000, 0x00000000], [0x00000000, 0xb716feb5, 0xbfd226dd], [0xbf098000, 0xbca00000, 0xbe990000], [0xbfc90fd8, 0x00000000, 0xbfc90fd8], [0xbef0c912, 0x3ef26300, 0xbea9cd6d], [0x00000000, 0x00000000, 0x3f801572], [0x00000000, 0x3716feb5, 0xbfd2274a], [0x4064a000, 0xc03e4000, 0x404e6000], [0x00000000, 0x40490000, 0x00000000]] },
    Row { tick: 96, status: [167, 171], pos: [0xc46bbb08, 0xc2c9fc32, 0xc429948c, 0xc3439023], legs: [[0xbf8edddc, 0xbe04cbe8, 0x3f1dc5aa], [0x00000000, 0x00000000, 0x00000000], [0x00000000, 0xb716feb5, 0xbfd226dd], [0xbf080000, 0xbca00000, 0xbe990000], [0xbfc90fd8, 0x00000000, 0xbfc90fd8], [0xbf27cc27, 0x3f01995f, 0xbe70ed85], [0x00000000, 0x00000000, 0x3f67195d], [0x00000000, 0x3716feb5, 0xbfd2274a], [0x4069a000, 0xc04a0000, 0x4052a000], [0x00000000, 0x40490000, 0x00000000]] },
    Row { tick: 97, status: [168, 172], pos: [0xc46bbb08, 0xc2c9fc32, 0xc428cb1e, 0xc344b01b], legs: [[0xbf7ca8ca, 0xbe0a436a, 0x3f18fbb5], [0x00000000, 0x00000000, 0x00000000], [0x00000000, 0xb716feb5, 0xbfd226dd], [0xbf038000, 0xbca00000, 0xbe9a0000], [0xbfc90fd8, 0x00000000, 0xbfc90fd8], [0xbf44b755, 0x3f0fbdeb, 0xbde2e614], [0x00000000, 0x00000000, 0x3f4c5d30], [0x00000000, 0x3716feb5, 0xbfd2274a], [0x406e6000, 0xc052e000, 0x40570000], [0x00000000, 0x40490000, 0x00000000]] },
    Row { tick: 98, status: [168, 172], pos: [0xc46bbb08, 0xc2c9fc32, 0xc428cb1e, 0xc344b01b], legs: [[0xbf7ca8ca, 0xbe0a436a, 0x3f18fbb5], [0x00000000, 0x00000000, 0x00000000], [0x00000000, 0xb716feb5, 0xbfd226dd], [0xbf038000, 0xbca00000, 0xbe9a0000], [0xbfc90fd8, 0x00000000, 0xbfc90fd8], [0xbf44b755, 0x3f0fbdeb, 0xbde2e614], [0x00000000, 0x00000000, 0x3f4c5d30], [0x00000000, 0x3716feb5, 0xbfd2274a], [0x406e6000, 0xc052e000, 0x40570000], [0x00000000, 0x40490000, 0x00000000]] },
    Row { tick: 99, status: [168, 172], pos: [0xc46bbb08, 0xc2c9fc32, 0xc428cb1e, 0xc344b01b], legs: [[0xbf7ca8ca, 0xbe0a436a, 0x3f18fbb5], [0x00000000, 0x00000000, 0x00000000], [0x00000000, 0xb716feb5, 0xbfd226dd], [0xbf038000, 0xbca00000, 0xbe9a0000], [0xbfc90fd8, 0x00000000, 0xbfc90fd8], [0xbf44b755, 0x3f0fbdeb, 0xbde2e614], [0x00000000, 0x00000000, 0x3f4c5d30], [0x00000000, 0x3716feb5, 0xbfd2274a], [0x406e6000, 0xc052e000, 0x40570000], [0x00000000, 0x40490000, 0x00000000]] },
    Row { tick: 100, status: [168, 172], pos: [0xc46bbb08, 0xc2c9fc32, 0xc428cb1e, 0xc344b01b], legs: [[0xbf7ca8ca, 0xbe0a436a, 0x3f18fbb5], [0x00000000, 0x00000000, 0x00000000], [0x00000000, 0xb716feb5, 0xbfd226dd], [0xbf038000, 0xbca00000, 0xbe9a0000], [0xbfc90fd8, 0x00000000, 0xbfc90fd8], [0xbf44b755, 0x3f0fbdeb, 0xbde2e614], [0x00000000, 0x00000000, 0x3f4c5d30], [0x00000000, 0x3716feb5, 0xbfd2274a], [0x406e6000, 0xc052e000, 0x40570000], [0x00000000, 0x40490000, 0x00000000]] },
    Row { tick: 101, status: [168, 172], pos: [0xc46bbb08, 0xc2c9fc32, 0xc428cb1e, 0xc344b01b], legs: [[0xbf7ca8ca, 0xbe0a436a, 0x3f18fbb5], [0x00000000, 0x00000000, 0x00000000], [0x00000000, 0xb716feb5, 0xbfd226dd], [0xbf038000, 0xbca00000, 0xbe9a0000], [0xbfc90fd8, 0x00000000, 0xbfc90fd8], [0xbf44b755, 0x3f0fbdeb, 0xbde2e614], [0x00000000, 0x00000000, 0x3f4c5d30], [0x00000000, 0x3716feb5, 0xbfd2274a], [0x406e6000, 0xc052e000, 0x40570000], [0x00000000, 0x40490000, 0x00000000]] },
    Row { tick: 102, status: [168, 172], pos: [0xc46bbb08, 0xc2c9fc32, 0xc428cb1e, 0xc344b01b], legs: [[0xbf7ca8ca, 0xbe0a436a, 0x3f18fbb5], [0x00000000, 0x00000000, 0x00000000], [0x00000000, 0xb716feb5, 0xbfd226dd], [0xbf038000, 0xbca00000, 0xbe9a0000], [0xbfc90fd8, 0x00000000, 0xbfc90fd8], [0xbf44b755, 0x3f0fbdeb, 0xbde2e614], [0x00000000, 0x00000000, 0x3f4c5d30], [0x00000000, 0x3716feb5, 0xbfd2274a], [0x406e6000, 0xc052e000, 0x40570000], [0x00000000, 0x40490000, 0x00000000]] },
];

/// The catch on the slope against the N64, tick by tick: statuses, both
/// positions and the catcher's slope-fitted legs, exact.
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn catch_slope_legs_and_victim_match_the_n64() {
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
    let pad = |t: u64| {
        let p = script::fighter_pad(scene, t).unwrap();
        (p.buttons, p.stick_x, p.stick_y)
    };
    let first_node = pack.object(fighters[0].object).unwrap().first_node;
    let mut pair = Pair::default();
    let mut failures = Vec::new();
    for tick in 8..=N64.last().unwrap().tick {
        let _ = tick_pair(&pack, &map, &mut fighters, &pad, tick, &mut pair);
        let Some(n64) = N64.iter().find(|r| r.tick == tick) else {
            continue;
        };
        let (m, v) = (&fighters[0].fighter, &fighters[1].fighter);
        let status = [m.status.status.id(), v.status.status.id()];
        let pos = [m.pos.x, m.pos.y, v.pos.x, v.pos.y].map(f32::to_bits);
        let legs: Vec<[u32; 3]> = (19..=28u32)
            .map(|j| {
                let pose = fighters[0].skeleton.node_pose(first_node + j - 4).unwrap();
                pose.rotate.map(f32::to_bits)
            })
            .collect();
        if status != n64.status || pos != n64.pos || legs != n64.legs {
            failures.push(format!(
                "tick {tick}: status {status:?} (N64 {:?}) pos {:?} (N64 {:?}) legs equal {}",
                n64.status,
                pos.map(f32::from_bits),
                n64.pos.map(f32::from_bits),
                legs == n64.legs
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
