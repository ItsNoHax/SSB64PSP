//! A [`SurfaceTable`] answers the map's scans as the map walk does: the
//! same results, bit for bit, on random synthetic maps (every kind,
//! multi-segment lines, shared vertices, moving groups, maps with and
//! without topology). Two broken tables are the negative controls.

use super::*;
use crate::weapon::SurfaceMotion;

const KINDS: [Kind; 4] = [Kind::Floor, Kind::Ceiling, Kind::RightWall, Kind::LeftWall];

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
    fn int(&mut self, lo: i32, hi: i32) -> i32 {
        lo + self.below((hi - lo + 1) as u64) as i32
    }
    fn float(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * (self.next() >> 40) as f32 / (1u64 << 24) as f32
    }
}

/// A random map: polylines of every kind, some sharing a vertex with an
/// earlier line, some on a moving group; the lines' segments interleaved
/// in map order, each line's in its own order.
fn synthetic_map(rng: &mut Rng, topology: bool) -> Vec<MapSurface> {
    let mut lines: Vec<Vec<MapSurface>> = Vec::new();
    let mut vertices = 0u16;
    for line in 0..rng.int(6, 16) as u16 {
        let kind = KINDS[rng.below(4) as usize];
        let segments = rng.int(1, 4) as u16;
        let motion = (rng.below(3) == 0).then(|| SurfaceMotion {
            offset: Vec2::new(rng.float(-60.0, 60.0), rng.float(-60.0, 60.0)),
            speed: Vec3::new(rng.float(-3.0, 3.0), rng.float(-3.0, 3.0), 0.0),
        });
        let mut vertex = if vertices > 0 && rng.below(3) == 0 {
            rng.below(u64::from(vertices)) as u16
        } else {
            vertices += 1;
            vertices - 1
        };
        let (mut x, mut y) = (rng.int(-1500, 1500), rng.int(-800, 1200));
        let mut out = Vec::new();
        for point in 0..segments {
            let (nx, ny) = match kind {
                Kind::Floor => (x + rng.int(20, 300), y + rng.int(-80, 80)),
                Kind::Ceiling => (x - rng.int(20, 300), y + rng.int(-80, 80)),
                Kind::RightWall => (x + rng.int(-60, 60), y - rng.int(20, 300)),
                Kind::LeftWall => (x + rng.int(-60, 60), y + rng.int(20, 300)),
            };
            let next = if point + 1 == segments && vertices > 0 && rng.below(3) == 0 {
                rng.below(u64::from(vertices)) as u16
            } else {
                vertices += 1;
                vertices - 1
            };
            out.push(MapSurface {
                kind,
                segment: Segment {
                    x1: x as i16,
                    y1: y as i16,
                    x2: nx as i16,
                    y2: ny as i16,
                    flags: rng.below(0x400) as u16,
                },
                topology: topology.then_some(SurfaceTopology {
                    line,
                    point,
                    segments,
                    vertex1: vertex,
                    vertex2: next,
                }),
                motion,
            });
            (x, y, vertex) = (nx, ny, next);
        }
        lines.push(out);
    }
    // Interleave: take the next segment of a random line until all are out.
    let mut cursors = vec![0; lines.len()];
    let total: usize = lines.iter().map(Vec::len).sum();
    let mut map = Vec::with_capacity(total);
    while map.len() < total {
        let l = rng.below(lines.len() as u64) as usize;
        if let Some(&s) = lines[l].get(cursors[l]) {
            map.push(s);
            cursors[l] += 1;
        }
    }
    map
}

/// The line IDs worth asking about: every line, and one the map lacks.
fn line_ids(map: &[MapSurface]) -> Vec<u16> {
    let mut ids: Vec<u16> = (0..map.len()).map(|i| line_id(i, map[i])).collect();
    ids.sort_unstable();
    ids.dedup();
    ids.push(999);
    ids
}

/// The queries [`compare`] asks, by their index in its hit counts.
const QUERIES: [&str; 6] = [
    "edge",
    "neighbor",
    "line_ends_y",
    "height",
    "line_common",
    "query_at",
];

/// (Queries that differ, each query's answers that found something)
/// between the map walk and `table` over many random queries of `map`.
fn compare(rng: &mut Rng, map: &[MapSurface], table: &SurfaceTable<'_>) -> (usize, [usize; 6]) {
    let walk = || map.iter().copied();
    let (mut differ, mut found) = (0, [0; 6]);
    let mut check = |query: usize, a: String, b: String, some: bool| {
        differ += usize::from(a != b);
        found[query] += usize::from(some);
    };
    let ids = line_ids(map);
    for kind in KINDS {
        for &line in &ids {
            for high in [false, true] {
                let a = edge(&walk, kind, line, high);
                let b = edge(table, kind, line, high);
                check(0, format!("{a:?}"), format!("{b:?}"), a.is_some());
                let a = neighbor(&walk, kind, line, high);
                let b = neighbor(table, kind, line, high);
                check(1, format!("{a:?}"), format!("{b:?}"), a.is_some());
            }
            let a = line_ends_y(&walk, kind, line);
            let b = line_ends_y(table, kind, line);
            check(2, format!("{a:?}"), format!("{b:?}"), a.is_some());
            for _ in 0..8 {
                let x = rng.float(-2000.0, 2000.0);
                let a = height(&walk, kind, line, x);
                let b = height(table, kind, line, x);
                check(3, format!("{a:?}"), format!("{b:?}"), a.is_some());
                let pos = Vec3::new(x, rng.float(-1000.0, 1500.0), 0.0);
                let a = line_common(&walk, kind, line, pos);
                let b = line_common(table, kind, line, pos);
                check(4, format!("{a:?}"), format!("{b:?}"), a.is_some());
            }
        }
        for _ in 0..400 {
            let from = Vec2::new(rng.float(-2000.0, 2000.0), rng.float(-1000.0, 1500.0));
            let to = Vec2::new(
                from.x + rng.float(-400.0, 400.0),
                from.y + rng.float(-400.0, 400.0),
            );
            for moving in [false, true] {
                let a = query_at(&walk, kind, from, to, moving);
                let b = query_at(table, kind, from, to, moving);
                check(5, format!("{a:?}"), format!("{b:?}"), a.is_some());
            }
        }
    }
    (differ, found)
}

#[test]
fn surface_table_answers_as_the_map_walk() {
    let mut rng = Rng(0x5eed);
    let mut found = [0; 6];
    for round in 0..200 {
        let map = synthetic_map(&mut rng, round % 4 != 3);
        let table = SurfaceTable::new(&map).unwrap();
        for kind in KINDS {
            let walked: Vec<usize> = (0..map.len()).filter(|&i| map[i].kind == kind).collect();
            let indexed: Vec<usize> = table.of_kind(kind).map(|(i, _)| i).collect();
            assert_eq!(indexed, walked, "round {round}: {kind:?}'s entries");
        }
        let (differ, hits) = compare(&mut rng, &map, &table);
        assert_eq!(differ, 0, "round {round}: {differ} queries differ");
        for (f, h) in found.iter_mut().zip(hits) {
            *f += h;
        }
    }
    eprintln!(
        "hits per query: {:?}",
        QUERIES.iter().zip(found).collect::<Vec<_>>()
    );
    // Every query finds lines, not only empty space.
    for (query, found) in QUERIES.into_iter().zip(found) {
        assert!(found > 1000, "only {found} {query} queries found a line");
    }
}

#[test]
fn broken_surface_tables_answer_otherwise() {
    let mut rng = Rng(0xbad);
    let (mut dropped, mut reversed) = (0, 0);
    for round in 0..50 {
        let map = synthetic_map(&mut rng, round % 4 != 3);
        if let Some(other) = map.iter().position(|s| s.kind != Kind::Floor) {
            let mut table = SurfaceTable::new(&map).unwrap();
            if let Some(first) = table.kind_indices_mut(Kind::Floor).first_mut() {
                *first = other as u8;
                dropped += compare(&mut rng, &map, &table).0;
            }
        }
        let mut table = SurfaceTable::new(&map).unwrap();
        for kind in KINDS {
            table.kind_indices_mut(kind).reverse();
        }
        reversed += compare(&mut rng, &map, &table).0;
    }
    eprintln!("differing answers: {dropped} (floor dropped), {reversed} (kinds reversed)");
    assert!(dropped > 0, "a table missing a floor answers as the walk");
    assert!(reversed > 0, "a table in reverse order answers as the walk");
}

#[test]
fn surface_table_refuses_a_map_past_capacity() {
    let s = MapSurface {
        kind: Kind::Floor,
        segment: Segment {
            x1: 0,
            y1: 0,
            x2: 1,
            y2: 0,
            flags: 0,
        },
        topology: None,
        motion: None,
    };
    assert!(SurfaceTable::new(&[s; SurfaceTable::CAPACITY]).is_some());
    assert!(SurfaceTable::new(&[s; SurfaceTable::CAPACITY + 1]).is_none());
}
