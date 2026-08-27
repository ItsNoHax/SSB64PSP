//! Stage collision geometry (`MPGeometryData`).
//!
//! Smash's collision is 2D polylines, not triangles. A stage's
//! [`crate::stage::GroundData`] points at one `MPGeometryData`, which reaches
//! the lines through two levels of indirection:
//!
//! ```c
//! line_info[yakumono].line_data[kind] = { group_id, line_count };
//! // line ids group_id .. group_id + line_count
//! vertex_links[line_id] = { vertex1, vertex2 };
//! pos = vertex_data[ vertex_id[vertex1 + k] ].pos;   // k < vertex2
//! ```
//!
//! `MPVertexLinks`'s field names are misleading and cost me an hour: `vertex2`
//! is a **count**, not a second vertex. `mpCollisionCheckFloor` walks
//! `for (v = vertex1; v < vertex1 + vertex2 - 1; v++)` joining consecutive
//! points, so one "line" is a polyline of `vertex2` points. Dream Land's line
//! 3 has `{9, 2}` and resolves to `(-2318, 0) .. (2318, 0)` — the main
//! platform, symmetric about the origin, which is exactly right and would not
//! have come out that way under the other reading.
//!
//! Array lengths are not stored anywhere, but they do not need to be: the
//! line count is the largest `group_id + line_count` over every kind, the
//! `vertex_id` length the largest `vertex1 + vertex2` over every line, and the
//! vertex count one past the largest index those name. Each level bounds the
//! next, so nothing is guessed from adjacency.
//!
//! Not extracted: `MPVertexInfo`, which the collision code indexes by line id
//! for early rejection. It is absent from `MPGeometryData` because the runtime
//! derives it on load — it is an acceleration structure, not source data.

use alloc::vec::Vec;

use crate::archive::File;

/// `sizeof(MPGeometryData)`.
pub const GEOMETRY_SIZE: u32 = 0x1C;

/// `sizeof(MPVertexData)`: `Vec2h pos` then `u16 vertex_flags`.
const VERTEX_SIZE: u32 = 6;
/// `sizeof(MPVertexLinks)`.
const LINK_SIZE: u32 = 4;
/// `sizeof(MPLineInfo)`: `u16 yakumono_id` then `MPLineData line_data[4]`.
/// Everything in it is `u16`, so the stride is 18 and not padded to 20.
const LINE_INFO_SIZE: u32 = 2 + 4 * 4;
/// `sizeof(MPMapObjData)`: `u16 mapobj_kind` then `Vec2h pos`.
const MAPOBJ_SIZE: u32 = 6;

// Field offsets within `MPGeometryData`.
const F_YAKUMONO_COUNT: u32 = 0x00;
const F_VERTEX_DATA: u32 = 0x04;
const F_VERTEX_ID: u32 = 0x08;
const F_VERTEX_LINKS: u32 = 0x0C;
const F_LINE_INFO: u32 = 0x10;
const F_MAPOBJ_COUNT: u32 = 0x14;
const F_MAPOBJS: u32 = 0x18;

/// A stage never has anything like this many; the cap only stops a misread
/// header from allocating wildly.
const MAX_YAKUMONO: u32 = 64;
const MAX_LINES: u32 = 4096;

/// Which side of a surface a line collides against.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LineKind {
    Floor,
    Ceiling,
    RightWall,
    LeftWall,
}

impl LineKind {
    pub const ALL: [LineKind; 4] = [
        LineKind::Floor,
        LineKind::Ceiling,
        LineKind::RightWall,
        LineKind::LeftWall,
    ];
}

/// One collision vertex.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CollisionVertex {
    /// Position in game units. Collision is planar; there is no z.
    pub pos: [i16; 2],
    /// Upper byte is surface flags (drop-through, cliff), lower byte the
    /// material that sets friction.
    pub flags: u16,
}

/// One collision polyline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollisionLine {
    /// Which movable group ("yakumono") owns this line. Group transforms are
    /// applied at run time, so the points here are in the group's own space.
    pub yakumono: u16,
    pub kind: LineKind,
    /// Its id in the flat line array, as `stand_line_id` reports it.
    pub id: u16,
    pub points: Vec<CollisionVertex>,
}

/// A point of interest: player spawns, item drops, and so on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapObject {
    /// `MPMapObjKind`. 0..=3 are the four players' start positions.
    pub kind: u16,
    pub pos: [i16; 2],
}

/// A stage's decoded collision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollisionMap {
    pub lines: Vec<CollisionLine>,
    pub map_objects: Vec<MapObject>,
}

impl CollisionMap {
    pub fn lines_of(&self, kind: LineKind) -> impl Iterator<Item = &CollisionLine> {
        self.lines.iter().filter(move |l| l.kind == kind)
    }
}

fn read_u16(data: &[u8], at: u32) -> Option<u16> {
    let at = at as usize;
    Some(u16::from_be_bytes(data.get(at..at + 2)?.try_into().ok()?))
}

fn read_i16(data: &[u8], at: u32) -> Option<i16> {
    read_u16(data, at).map(|v| v as i16)
}

/// Follows the pointer slot at `at` within this file.
///
/// Collision arrays always live alongside the `MPGeometryData` that names
/// them, so an intern relocation is the only valid form; requiring one keeps a
/// misidentified header from reading arbitrary bytes as vertices.
fn intern(file: &File, at: u32) -> Option<u32> {
    file.intern_relocs
        .iter()
        .find(|r| r.at == at)
        .map(|r| r.target)
}

/// Reads the `MPGeometryData` at `at`.
pub fn read(file: &File, at: u32) -> Option<CollisionMap> {
    let data = &file.data;
    let yakumono_count = read_u16(data, at + F_YAKUMONO_COUNT)? as u32;
    if yakumono_count == 0 || yakumono_count > MAX_YAKUMONO {
        return None;
    }
    let vertex_data = intern(file, at + F_VERTEX_DATA)?;
    let vertex_id = intern(file, at + F_VERTEX_ID)?;
    let vertex_links = intern(file, at + F_VERTEX_LINKS)?;
    let line_info = intern(file, at + F_LINE_INFO)?;
    let mapobj_count = read_u16(data, at + F_MAPOBJ_COUNT)? as u32;

    // Pass 1: the line groups, which bound the flat line array.
    let mut groups = Vec::new();
    let mut line_count = 0u32;
    for i in 0..yakumono_count {
        let info = line_info + i * LINE_INFO_SIZE;
        let yakumono = read_u16(data, info)?;
        for (k, kind) in LineKind::ALL.into_iter().enumerate() {
            let entry = info + 2 + k as u32 * 4;
            let first = read_u16(data, entry)? as u32;
            let count = read_u16(data, entry + 2)? as u32;
            let end = first.checked_add(count)?;
            if end > MAX_LINES {
                return None;
            }
            line_count = line_count.max(end);
            if count != 0 {
                groups.push((yakumono, kind, first, count));
            }
        }
    }
    if line_count == 0 {
        return None;
    }

    // Pass 2: each line's slice of `vertex_id`, bounding that array in turn.
    let mut spans = Vec::with_capacity(line_count as usize);
    let mut id_count = 0u32;
    for line in 0..line_count {
        let link = vertex_links + line * LINK_SIZE;
        let first = read_u16(data, link)? as u32;
        let count = read_u16(data, link + 2)? as u32;
        // A polyline needs two points to be a segment; the collision walk
        // would do nothing with fewer.
        if count < 2 {
            return None;
        }
        let end = first.checked_add(count)?;
        id_count = id_count.max(end);
        spans.push((first, count));
    }

    // Pass 3: the vertices those ids name.
    let ids: Vec<u16> = (0..id_count)
        .map(|i| read_u16(data, vertex_id + i * 2))
        .collect::<Option<_>>()?;
    let vertex_count = ids.iter().copied().max()? as u32 + 1;
    let vertices: Vec<CollisionVertex> = (0..vertex_count)
        .map(|i| {
            let v = vertex_data + i * VERTEX_SIZE;
            Some(CollisionVertex {
                pos: [read_i16(data, v)?, read_i16(data, v + 2)?],
                flags: read_u16(data, v + 4)?,
            })
        })
        .collect::<Option<_>>()?;

    let mut lines = Vec::new();
    for (yakumono, kind, first, count) in groups {
        for id in first..first + count {
            let (start, points) = spans[id as usize];
            lines.push(CollisionLine {
                yakumono,
                kind,
                id: id as u16,
                points: (start..start + points)
                    .map(|i| vertices.get(ids[i as usize] as usize).copied())
                    .collect::<Option<_>>()?,
            });
        }
    }

    let map_objects = match intern(file, at + F_MAPOBJS) {
        Some(mapobjs) => (0..mapobj_count)
            .map(|i| {
                let o = mapobjs + i * MAPOBJ_SIZE;
                Some(MapObject {
                    kind: read_u16(data, o)?,
                    pos: [read_i16(data, o + 2)?, read_i16(data, o + 4)?],
                })
            })
            .collect::<Option<_>>()?,
        None => Vec::new(),
    };

    Some(CollisionMap { lines, map_objects })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::InternReloc;
    use alloc::vec;

    const GEOM: u32 = 0x100;
    const VPOS: u32 = 0x200;
    const VIDS: u32 = 0x300;
    const LINKS: u32 = 0x400;
    const INFO: u32 = 0x500;
    const OBJS: u32 = 0x600;

    /// Dream Land's shape in miniature: one yakumono, two floor lines, one
    /// ceiling line, and a player spawn.
    fn fixture() -> File {
        let mut data = vec![0u8; 0x800];
        let mut relocs = Vec::new();
        let u16_at = |data: &mut Vec<u8>, at: u32, v: u16| {
            data[at as usize..at as usize + 2].copy_from_slice(&v.to_be_bytes());
        };

        u16_at(&mut data, GEOM + F_YAKUMONO_COUNT, 1);
        u16_at(&mut data, GEOM + F_MAPOBJ_COUNT, 1);
        for (slot, target) in [
            (F_VERTEX_DATA, VPOS),
            (F_VERTEX_ID, VIDS),
            (F_VERTEX_LINKS, LINKS),
            (F_LINE_INFO, INFO),
            (F_MAPOBJS, OBJS),
        ] {
            data[(GEOM + slot) as usize..(GEOM + slot) as usize + 4]
                .copy_from_slice(&target.to_be_bytes());
            relocs.push(InternReloc {
                at: GEOM + slot,
                target,
            });
        }

        // Four vertices: a ground span and a raised platform.
        for (i, (x, y, flags)) in [
            (-2318i16, 0i16, 0x8000u16),
            (2318, 0, 0),
            (-1396, 904, 0x4000),
            (-951, 904, 0x4000),
        ]
        .into_iter()
        .enumerate()
        {
            let v = VPOS + i as u32 * VERTEX_SIZE;
            data[v as usize..v as usize + 2].copy_from_slice(&x.to_be_bytes());
            data[(v + 2) as usize..(v + 2) as usize + 2].copy_from_slice(&y.to_be_bytes());
            u16_at(&mut data, v + 4, flags);
        }
        // Indirection: line 0 uses vertices 0,1; line 1 uses 2,3; line 2
        // reuses 1,0 as a ceiling.
        for (i, id) in [0u16, 1, 2, 3, 1, 0].into_iter().enumerate() {
            u16_at(&mut data, VIDS + i as u32 * 2, id);
        }
        for (i, (first, count)) in [(0u16, 2u16), (2, 2), (4, 2)].into_iter().enumerate() {
            let l = LINKS + i as u32 * LINK_SIZE;
            u16_at(&mut data, l, first);
            u16_at(&mut data, l + 2, count);
        }
        // yakumono 1: floor lines 0..2, ceiling line 2, no walls.
        u16_at(&mut data, INFO, 1);
        for (k, (first, count)) in [(0u16, 2u16), (2, 1), (0, 0), (0, 0)]
            .into_iter()
            .enumerate()
        {
            u16_at(&mut data, INFO + 2 + k as u32 * 4, first);
            u16_at(&mut data, INFO + 4 + k as u32 * 4, count);
        }
        u16_at(&mut data, OBJS, 0); // player 1 start
        u16_at(&mut data, OBJS + 2, (-500i16) as u16);
        u16_at(&mut data, OBJS + 4, 900);

        File {
            id: 104,
            data,
            extern_relocs: Vec::new(),
            intern_relocs: relocs,
        }
    }

    #[test]
    fn decodes_lines_through_both_indirections() {
        let map = read(&fixture(), GEOM).expect("collision");
        assert_eq!(map.lines.len(), 3);

        let floors: Vec<&CollisionLine> = map.lines_of(LineKind::Floor).collect();
        assert_eq!(floors.len(), 2);
        // `vertex2` is a count, so line 0 is the two-point span 0..2 rather
        // than the single segment from vertex 0 to vertex 2.
        assert_eq!(floors[0].points.len(), 2);
        assert_eq!(floors[0].points[0].pos, [-2318, 0]);
        assert_eq!(floors[0].points[1].pos, [2318, 0]);
        assert_eq!(floors[0].points[0].flags, 0x8000);
        assert_eq!(floors[1].points[0].pos, [-1396, 904]);

        let ceilings: Vec<&CollisionLine> = map.lines_of(LineKind::Ceiling).collect();
        assert_eq!(ceilings.len(), 1);
        assert_eq!(ceilings[0].id, 2);
        assert_eq!(ceilings[0].yakumono, 1);
    }

    #[test]
    fn reads_map_objects() {
        let map = read(&fixture(), GEOM).expect("collision");
        assert_eq!(map.map_objects.len(), 1);
        assert_eq!(map.map_objects[0].kind, 0);
        assert_eq!(map.map_objects[0].pos, [-500, 900]);
    }

    #[test]
    fn array_lengths_come_from_the_data_not_from_adjacency() {
        // Nothing stores how many vertices there are. The largest id any line
        // names is 3, so reading a fifth vertex would mean reading past the
        // data the file actually holds for this map.
        let map = read(&fixture(), GEOM).expect("collision");
        let named: usize = map.lines.iter().map(|l| l.points.len()).sum();
        assert_eq!(named, 6, "three two-point lines");
    }

    #[test]
    fn an_unrelocated_array_pointer_is_rejected() {
        let mut file = fixture();
        file.intern_relocs.retain(|r| r.at != GEOM + F_VERTEX_DATA);
        assert_eq!(read(&file, GEOM), None);
    }

    #[test]
    fn a_line_with_fewer_than_two_points_is_rejected() {
        let mut file = fixture();
        let at = (LINKS + 2) as usize;
        file.data[at..at + 2].copy_from_slice(&1u16.to_be_bytes());
        assert_eq!(read(&file, GEOM), None);
    }

    #[test]
    fn a_zero_yakumono_count_is_not_a_geometry_header() {
        let mut file = fixture();
        let at = (GEOM + F_YAKUMONO_COUNT) as usize;
        file.data[at..at + 2].copy_from_slice(&0u16.to_be_bytes());
        assert_eq!(read(&file, GEOM), None);
    }
}
