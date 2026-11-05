//! A [`SurfaceTable`] answers every public map query as the map walk does,
//! bit for bit, on every stage map in the pack: as packed, with its moving
//! groups displaced, and without topology. Two broken tables on Dream Land
//! are the negative controls.

use ssb_engine::math::{Vec2, Vec3};
use ssb_game::ground::BodyColl;
use ssb_game::map::{self, AirOptions, SurfaceSet, SurfaceTable};
use ssb_game::weapon::{MapSurface, MapSurfaceKind as Kind, SurfaceMotion};

#[path = "common/dream_land.rs"]
mod dream_land;

use dream_land::{dream_land_map, map_groups};

const KINDS: [Kind; 4] = [Kind::Floor, Kind::Ceiling, Kind::RightWall, Kind::LeftWall];

const BODY: BodyColl = BodyColl {
    top: 160.0,
    center: 80.0,
    bottom: 0.0,
    width: 70.0,
};

const PREVIOUS: BodyColl = BodyColl {
    top: 150.0,
    center: 75.0,
    bottom: 0.0,
    width: 60.0,
};

fn open_pack() -> Option<Vec<u8>> {
    ssb_tablegen::test_rom(env!("CARGO_MANIFEST_DIR"))?;
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak");
    std::fs::read(path).ok()
}

/// SplitMix64.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
    fn float(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }
}

/// Every answer of the walk and the table side by side, by query.
#[derive(Default)]
struct Tally {
    /// (query, walk's answer, table's answer) for each differing answer.
    differ: Vec<(&'static str, String, String)>,
    /// Answers that found something, by query.
    found: std::collections::BTreeMap<&'static str, usize>,
}

impl Tally {
    fn check<T: core::fmt::Debug>(&mut self, query: &'static str, a: T, b: T, some: bool) {
        let (a, b) = (format!("{a:?}"), format!("{b:?}"));
        if a != b {
            self.differ.push((query, a, b));
        }
        *self.found.entry(query).or_default() += usize::from(some);
    }
}

/// Asks the walk and `table` the same random queries of `map`.
fn compare(rng: &mut Rng, map: &[MapSurface], table: &SurfaceTable<'_>, tally: &mut Tally) {
    let walk = || map.iter().copied();
    let (mut lo, mut hi) = (Vec2::new(f32::MAX, f32::MAX), Vec2::new(f32::MIN, f32::MIN));
    for s in map {
        let [x1, y1, x2, y2] = s.coords();
        lo = Vec2::new(lo.x.min(x1).min(x2), lo.y.min(y1).min(y2));
        hi = Vec2::new(hi.x.max(x1).max(x2), hi.y.max(y1).max(y2));
    }
    let mut ids: Vec<u16> = map
        .iter()
        .enumerate()
        .map(|(i, s)| s.topology.map_or(i as u16, |t| t.line))
        .collect();
    ids.sort_unstable();
    ids.dedup();
    ids.push(999);
    let point = |rng: &mut Rng| {
        Vec3::new(
            rng.float(lo.x - 300.0, hi.x + 300.0),
            rng.float(lo.y - 300.0, hi.y + 600.0),
            0.0,
        )
    };
    let near = |rng: &mut Rng, p: Vec3, r: f32| {
        Vec3::new(p.x + rng.float(-r, r), p.y + rng.float(-r, r), 0.0)
    };
    let flat = |p: Vec3| Vec2::new(p.x, p.y);

    for &line in &ids {
        for kind in KINDS {
            for high in [false, true] {
                let (a, b) = (
                    map::line_edge(&walk, kind, line, high),
                    map::line_edge(table, kind, line, high),
                );
                tally.check("line_edge", a, b, a.is_some());
                let (a, b) = (
                    map::line_neighbor(&walk, kind, line, high),
                    map::line_neighbor(table, kind, line, high),
                );
                tally.check("line_neighbor", a, b, a.is_some());
            }
        }
        for right in [false, true] {
            let (a, b) = (
                map::floor_edge(&walk, line, right),
                map::floor_edge(table, line, right),
            );
            tally.check("floor_edge", a, b, a.is_some());
            let (a, b) = (
                map::floor_edge_under(&walk, line, right),
                map::floor_edge_under(table, line, right),
            );
            tally.check("floor_edge_under", a, b, a.is_some());
        }
        let (a, b) = (map::line_exists(&walk, line), map::line_exists(table, line));
        tally.check("line_exists", a, b, a);
        let (a, b) = (map::line_speed(walk, line), map::line_speed(table, line));
        tally.check("line_speed", a, b, true);
        for facing in [-1.0, 1.0] {
            let (a, b) = (
                map::cliff_corner(walk, line, facing),
                map::cliff_corner(table, line, facing),
            );
            tally.check("cliff_corner", a, b, a.is_some());
        }
        for _ in 0..16 {
            let p = point(rng);
            for kind in KINDS {
                let (a, b) = (
                    map::line_common(&walk, kind, line, p),
                    map::line_common(table, kind, line, p),
                );
                tally.check("line_common", a, b, a.is_some());
            }
            let (a, b) = (
                map::floor_distance(&walk, line, p.x, p.y),
                map::floor_distance(table, line, p.x, p.y),
            );
            tally.check("floor_distance", a, b, a.is_some());
            let (a, b) = (
                map::floor_point(walk, line, p.x),
                map::floor_point(table, line, p.x),
            );
            tally.check("floor_point", a, b, a.is_some());
            let to = near(rng, p, 40.0);
            let stop_edge = rng.below(2) == 0;
            let (a, b) = (
                map::move_ground(&BODY, p, to, line, stop_edge, walk),
                map::move_ground(&BODY, p, to, line, stop_edge, table),
            );
            tally.check("move_ground", a, b, true);
        }
    }

    for _ in 0..600 {
        let from = point(rng);
        let to = near(rng, from, 300.0);
        let (a, b) = (
            map::floor_crossing(&walk, flat(from), flat(to)),
            map::floor_crossing(table, flat(from), flat(to)),
        );
        tally.check("floor_crossing", a, b, a.is_some());
        for kind in KINDS {
            let (a, b) = (
                map::line_crossing_same(&walk, kind, flat(from), flat(to)),
                map::line_crossing_same(table, kind, flat(from), flat(to)),
            );
            tally.check("line_crossing_same", a, b, a.is_some());
            let (a, b) = (
                map::line_crossing(&walk, kind, flat(from), flat(to)),
                map::line_crossing(table, kind, flat(from), flat(to)),
            );
            tally.check("line_crossing", a, b, a.is_some());
        }
        for kind in [Kind::RightWall, Kind::LeftWall] {
            let (a, b) = (
                map::wall_crossing(&walk, kind, flat(from), flat(to)),
                map::wall_crossing(table, kind, flat(from), flat(to)),
            );
            tally.check("wall_crossing", a, b, a.is_some());
        }
        let (a, b) = (
            map::project_floor_line(&walk, from),
            map::project_floor_line(table, from),
        );
        tally.check("project_floor_line", a, b, a.is_some());
        let (a, b) = (
            map::project_floor(&walk, from),
            map::project_floor(table, from),
        );
        tally.check("project_floor", a, b, a.is_some());
        let (a, b) = (
            map::project_floor_below(&walk, from),
            map::project_floor_below(table, from),
        );
        tally.check("project_floor_below", a, b, a.is_some());
        let (a, b) = (
            map::project_rwall_dist(&walk, from),
            map::project_rwall_dist(table, from),
        );
        tally.check("project_rwall_dist", a, b, a.is_some());

        let to = near(rng, from, 60.0);
        let options = AirOptions::default();
        let (a, b) = (
            map::move_air(&BODY, from, to, options, walk),
            map::move_air(&BODY, from, to, options, table),
        );
        tally.check("move_air", a, b, true);
        let (a, b) = (
            map::run_default_collision_project(&BODY, &PREVIOUS, from, to, walk),
            map::run_default_collision_project(&BODY, &PREVIOUS, from, to, table),
        );
        tally.check("run_default_collision_project", a, b, true);
        let (a, b) = (
            map::move_air_check_coll_end(&BODY, from, to, walk),
            map::move_air_check_coll_end(&BODY, from, to, table),
        );
        tally.check("move_air_check_coll_end", a, b, true);
        let (a, b) = (
            map::move_damage(&BODY, from, to, 0, false, None, walk),
            map::move_damage(&BODY, from, to, 0, false, None, table),
        );
        tally.check("move_damage", a, b, true);
        let facing = if rng.below(2) == 0 { -1.0 } else { 1.0 };
        let reach = Vec2::new(rng.float(100.0, 400.0), rng.float(100.0, 400.0));
        let (a, b) = (
            map::cliff(walk, facing, 0, reach, from, to),
            map::cliff(table, facing, 0, reach, from, to),
        );
        tally.check("cliff", a, b, a.is_some());
    }
}

/// `map` with each moving group displaced as a moving yakumono would be,
/// and every other group static.
fn displaced(rng: &mut Rng, map: &[MapSurface], groups: &[u16]) -> Vec<MapSurface> {
    let mut motion = std::collections::BTreeMap::new();
    map.iter()
        .zip(groups)
        .map(|(&s, &g)| MapSurface {
            motion: *motion.entry(g).or_insert_with(|| {
                (g != 0 && rng.below(4) != 0).then(|| SurfaceMotion {
                    offset: Vec2::new(rng.float(-200.0, 200.0), rng.float(-200.0, 200.0)),
                    speed: Vec3::new(rng.float(-4.0, 4.0), rng.float(-4.0, 4.0), 0.0),
                })
            }),
            ..s
        })
        .collect()
}

#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn surface_table_answers_as_the_map_walk_on_every_stage() {
    let Some(bytes) = open_pack() else {
        return;
    };
    let pack = ssb_rom::pack::Pack::open(&bytes).unwrap();
    let mut rng = Rng(0x7ab1e);
    let mut tally = Tally::default();
    let mut maps = 0;
    for index in 0..pack.stage_count() {
        let stage = pack.stage(index).unwrap();
        let packed = dream_land_map(&pack, &stage);
        if packed.is_empty() {
            continue;
        }
        let groups = map_groups(&pack, &stage);
        assert_eq!(groups.len(), packed.len());
        let moving = displaced(&mut rng, &packed, &groups);
        let bare: Vec<MapSurface> = packed
            .iter()
            .map(|&s| MapSurface {
                topology: None,
                ..s
            })
            .collect();
        for map in [&packed, &moving, &bare] {
            let table = SurfaceTable::new(map).expect("a stage map fits the table");
            for kind in KINDS {
                let walked: Vec<usize> = (0..map.len()).filter(|&i| map[i].kind == kind).collect();
                let indexed: Vec<usize> = table.of_kind(kind).map(|(i, _)| i).collect();
                assert_eq!(indexed, walked, "stage {index}: {kind:?}'s entries");
            }
            compare(&mut rng, map, &table, &mut tally);
            maps += 1;
        }
    }
    eprintln!(
        "{maps} maps; answers that found something: {:?}",
        tally.found
    );
    assert!(
        tally.differ.is_empty(),
        "{} answers differ, first {:?}",
        tally.differ.len(),
        tally.differ.first()
    );
    for (query, found) in &tally.found {
        assert!(*found > 0, "no {query} query found anything");
    }
}

#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn broken_surface_tables_answer_otherwise_on_dream_land() {
    let Some(bytes) = open_pack() else {
        return;
    };
    let pack = ssb_rom::pack::Pack::open(&bytes).unwrap();
    let map = dream_land_map(&pack, &pack.stage(0).unwrap());
    let other = map.iter().position(|s| s.kind != Kind::Floor).unwrap() as u8;
    let mut rng = Rng(0xbad);

    let mut dropped = SurfaceTable::new(&map).unwrap();
    dropped.kind_indices_mut(Kind::Floor)[0] = other;
    let mut tally = Tally::default();
    compare(&mut rng, &map, &dropped, &mut tally);
    let dropped = tally.differ.len();

    let mut reversed = SurfaceTable::new(&map).unwrap();
    for kind in KINDS {
        reversed.kind_indices_mut(kind).reverse();
    }
    let mut tally = Tally::default();
    compare(&mut rng, &map, &reversed, &mut tally);
    let reversed = tally.differ.len();

    eprintln!("differing answers: {dropped} (floor dropped), {reversed} (kinds reversed)");
    assert!(dropped > 0, "a table missing a floor answers as the walk");
    assert!(reversed > 0, "a table in reverse order answers as the walk");
}
