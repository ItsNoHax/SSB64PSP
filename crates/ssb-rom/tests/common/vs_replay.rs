//! VS stage replays against N64 traces (`~/ppsspp-test/b41/vs_in.py`
//! through `inrows.py`): two pads, the stage's own controller and items.
//!
//! The N64 measurements are embedded in each test ([`Trace`]), as
//! `castle_replays` does: the pad and RNG-seed changes of the whole run,
//! and the checked rows (every tenth tick plus every tick a discrete
//! column changes). The environment variable named by [`replay_env`]
//! optionally lists full per-tick traces (`:` apart) to replay as well;
//! those stay outside git.
//!
//! Per checked row, whitespace-separated: `t`, each fighter
//! `status x y`, then the stage's own columns ([`Row::rest`]). N64 row `t`
//! follows frame `t`; the pads and seed of row `t - 1` act on frame `t`.
//! A full trace's rows also carry `seed in in2` after `t`.

use ssb_game::fighter::{Facing, FighterKind};

use crate::dream_land::{dream_land_map, map_groups, stage_bounds, stage_floors, vs_fighter};
use crate::posed::{tick_pair_pads, NoProps, Pair, Posed};

pub struct Row {
    pub t: u64,
    pub fighters: [(i32, f32, f32); 2],
    /// The stage's columns.
    pub rest: Vec<String>,
}

/// The N64 run's inputs by row: the RNG seed and both pads.
pub struct Inputs {
    pub seed: Vec<u32>,
    pub pad: Vec<[u32; 2]>,
}

/// One embedded N64 run.
pub struct Trace {
    /// `(first row, pad0, pad1)`, each in force until the next entry.
    pub pads: &'static [(u64, u32, u32)],
    /// `(first row, seed)`, likewise.
    pub seeds: &'static [(u64, u32)],
    /// The checked rows, `t` ascending, as `t s0 x0 y0 s1 x1 y1 rest..`.
    pub rows: &'static str,
}

fn fighters(w: &[&str]) -> [(i32, f32, f32); 2] {
    [0, 3].map(|o| {
        (
            w[o].parse().unwrap(),
            w[o + 1].parse().unwrap(),
            w[o + 2].parse().unwrap(),
        )
    })
}

fn parse_embedded(text: &str) -> Vec<Row> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(|l| {
            let w: Vec<&str> = l.split_whitespace().collect();
            Row {
                t: w[0].parse().unwrap(),
                fighters: fighters(&w[1..7]),
                rest: w[7..].iter().map(|s| s.to_string()).collect(),
            }
        })
        .collect()
}

fn expand<T: Copy>(len: usize, changes: impl Iterator<Item = (u64, T)>) -> Vec<T> {
    let mut out = Vec::with_capacity(len);
    let mut changes = changes.peekable();
    let mut cur = changes.next().expect("no first entry").1;
    for t in 0..len as u64 {
        while let Some(&(at, v)) = changes.peek() {
            if at > t {
                break;
            }
            cur = v;
            changes.next();
        }
        out.push(cur);
    }
    out
}

impl Trace {
    fn load(&self) -> (Inputs, Vec<Row>) {
        let rows = parse_embedded(self.rows);
        let len = rows.last().unwrap().t as usize + 1;
        let seed = expand(len, self.seeds.iter().copied());
        let pad = expand(len, self.pads.iter().map(|&(t, a, b)| (t, [a, b])));
        (Inputs { seed, pad }, rows)
    }
}

/// A full per-tick trace: every row carries `t seed in in2`.
fn parse_full(text: &str) -> (Inputs, Vec<Row>) {
    let mut inputs = Inputs {
        seed: Vec::new(),
        pad: Vec::new(),
    };
    let rows = text
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| {
            let w: Vec<&str> = l.split_whitespace().collect();
            let hex = |i: usize| u32::from_str_radix(w[i], 16).unwrap();
            inputs.seed.push(hex(1));
            inputs.pad.push([hex(2), hex(3)]);
            Row {
                t: w[0].parse().unwrap(),
                fighters: fighters(&w[4..10]),
                rest: w[10..].iter().map(|s| s.to_string()).collect(),
            }
        })
        .collect();
    (inputs, rows)
}

/// A Mupen64Plus `BUTTONS` word as the host's pad.
pub fn pad(w: u32) -> (u16, i8, i8) {
    use ssb_capture::script::button;
    let mut b = 0;
    for (bit, host) in [
        (1 << 7, button::A),
        (1 << 6, button::B),
        (1 << 5, button::Z),
        (1 << 11, button::C_UP),
        (1 << 13, button::L),
    ] {
        if w & bit != 0 {
            b |= host;
        }
    }
    (b, (w >> 16) as u8 as i8, (w >> 24) as u8 as i8)
}

pub fn open_pack() -> Option<Vec<u8>> {
    ssb_tablegen::test_rom(env!("CARGO_MANIFEST_DIR"))?;
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak"),
    )
    .ok()
}

/// The trace prints fighter positions with two decimals.
pub fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.0051
}

/// One VS trace: Mario against Mario on `gkind`.
pub struct Vs {
    pub gkind: u8,
    /// Each fighter's entry spot, facing, and the row whose Wait shows
    /// `anim_frame` 0 (the entry is not replayed).
    pub spawns: [(f32, f32, Facing, u64); 2],
    /// The first frame `game_status` is past the countdown's Wait; the
    /// pads unlock after that frame's interrupts.
    pub go: u64,
}

/// Replays every tick up to the last of `rows` on the pack's stage
/// `vs.gkind`, driven by `inputs`: the first checked tick that differs,
/// with what differs. `check` compares the stage's columns (it also sees
/// the fighters).
pub fn replay(
    bytes: &[u8],
    vs: &Vs,
    inputs: &Inputs,
    rows: &[Row],
    mut check: impl FnMut(&Row, &Pair, &[Posed]) -> Vec<String>,
) -> Option<(u64, String)> {
    use ssb_game::stage::{MapObject, Stage, StageInit, StageKind};
    let pack = ssb_rom::pack::Pack::open(bytes).unwrap();
    let stage_id = pack
        .stage_of_file(ssb_rom::stage::VS_GROUND_FILES[usize::from(vs.gkind)])
        .unwrap();
    let (stage, mut segments) = stage_floors(&pack, stage_id);
    let stage_groups = ssb_pose::ground::StageGroups::new(&pack, stage_id, &stage);
    // The spawn's floor search runs in world space: a moving group's
    // lines sit at its translation (Yoshi's Island's clouds are group 1-3
    // lines whose group-space points lie under the island).
    let line_group: std::collections::HashMap<u16, u16> = pack
        .stage_lines(&stage)
        .map(|l| (l.id, l.yakumono))
        .collect();
    for (id, seg) in &mut segments {
        let group = line_group[id];
        if group != 0 {
            let t = stage_groups.groups[usize::from(group)].translate;
            let (dx, dy) = (t.x.round() as i16, t.y.round() as i16);
            (seg.x1, seg.y1, seg.x2, seg.y2) = (seg.x1 + dx, seg.y1 + dy, seg.x2 + dx, seg.y2 + dy);
        }
    }
    let map = dream_land_map(&pack, &stage);
    let make = |port: usize| {
        let (x, y, facing, _) = vs.spawns[port];
        let at = (x, y, facing);
        let mut f = vs_fighter(&pack, &stage, FighterKind::Mario, port as u8, at, &segments);
        f.dead.bounds = Some(stage_bounds(&pack, &stage));
        Posed::new(&pack, f)
    };
    let mut fighters: Vec<Posed> = (0..2).map(make).collect();
    let mut pair = Pair {
        groups: Some((stage_groups, map_groups(&pack, &stage))),
        ..Pair::default()
    };
    pair.items.normal_switches.toggles = 0;
    let kind = StageKind::from_gkind(vs.gkind).unwrap();
    let map_objects: Vec<MapObject> = pack
        .stage_points(&stage)
        .map(|p| MapObject {
            kind: p.kind,
            pos: ssb_engine::math::Vec3::new(f32::from(p.x), f32::from(p.y), 0.0),
        })
        .collect();
    let (hazard_attack, hazard_throw) = kind.hazard_descs(stage.hazard);
    let init = StageInit {
        kind,
        map_objects: &map_objects,
        bound_bottom: f32::from(stage.bounds.bottom),
        hazard_attack,
        hazard_throw,
        acid_surface_y: stage.hazard_surface_y,
        bonus3_bumpers: &[],
        player: 0,
    };
    pair.objects = Some(Box::new(ssb_rom::ground_obj::GroundObjects::new(
        &pack,
        stage.source_file,
    )));
    let groups = &mut pair.groups.as_mut().unwrap().0.groups;
    pair.stage = Stage::new(&init, groups, &mut NoProps, &mut *pair.items);
    let pads = |port: usize| {
        move |t: u64| {
            t.checked_sub(1)
                .map_or((0, 0, 0), |p| pad(inputs.pad[p as usize][port]))
        }
    };
    let (pad0, pad1) = (pads(0), pads(1));
    let mut next = rows.iter().peekable();
    for t in 0..=rows.last().unwrap().t {
        // Row 0 follows the battle's first frame, which ran the stage
        // and its items once; the seed is still `syUtilsRandomSeed`'s 1.
        ssb_game::rng::set_seed(
            t.checked_sub(1)
                .map_or(1, |p| inputs.seed[p as usize] as i32),
        );
        pair.started = t >= vs.go;
        pair.locked = t <= vs.go;
        pair.go = t == vs.go;
        tick_pair_pads(&pack, &map, &mut fighters, &pad0, &pad1, t, &mut pair);
        for (port, spawn) in vs.spawns.iter().enumerate() {
            if spawn.3 == t {
                fighters[port] = make(port);
            }
        }
        let Some(row) = next.next_if(|r| r.t == t) else {
            continue;
        };
        let mut bad = Vec::new();
        for (port, &(st, x, y)) in row.fighters.iter().enumerate() {
            if t < vs.spawns[port].3 {
                continue;
            }
            let f = &fighters[port].fighter;
            let id = i32::from(f.status.status.id());
            if id != st || !near(f.pos.x, x) || !near(f.pos.y, y) {
                bad.push(format!(
                    "p{port} host {id} {} {} n64 {st} {x} {y}",
                    f.pos.x, f.pos.y
                ));
            }
        }
        bad.extend(check(row, &pair, &fighters));
        if !bad.is_empty() {
            return Some((t, bad.join("; ")));
        }
    }
    None
}

/// Replays each embedded trace, then each `:`-separated full trace in
/// `var` if set; `None` when the pack is missing (a ROM-less run checks
/// nothing, as the other ROM tests).
pub fn replay_env(
    var: &str,
    vs: &Vs,
    traces: &[&Trace],
    mut check: impl FnMut(&Row, &Pair, &[Posed]) -> Vec<String>,
) -> Option<()> {
    let bytes = open_pack()?;
    for (i, trace) in traces.iter().enumerate() {
        let (inputs, rows) = trace.load();
        assert!(rows.len() > 100, "embedded trace {i} too short");
        assert!(rows.last().unwrap().t > 600, "embedded trace {i} too short");
        if let Some((t, what)) = replay(&bytes, vs, &inputs, &rows, &mut check) {
            panic!("embedded trace {i}: first divergence at tick {t}: {what}");
        }
    }
    for path in std::env::var_os(var).iter().flat_map(std::env::split_paths) {
        let (inputs, rows) = parse_full(&std::fs::read_to_string(&path).unwrap());
        assert!(rows.len() > 600, "{}: too short", path.display());
        if let Some((t, what)) = replay(&bytes, vs, &inputs, &rows, &mut check) {
            panic!("{}: first divergence at tick {t}: {what}", path.display());
        }
    }
    Some(())
}
