//! Stage headers (`MPGroundData`).
//!
//! A stage is spread over several archive files — Dream Land's geometry is in
//! `StagePupupuFile2`/`File3`, its background in `StageDreamLand`, its
//! randomiser weights in `GRPupupuMap` — and one struct in the `GR*Map` file
//! ties them together:
//!
//! ```c
//! struct MPGroundDesc {
//!     DObjDesc *dobjdesc;
//!     AObjEvent32 **anim_joints;
//!     MObjSub ***p_mobjsubs;
//!     AObjEvent32 ***p_matanim_joints;
//! };
//!
//! struct MPGroundData {
//!     MPGroundDesc gr_desc[4];        // four render layers
//!     MPGeometryData *map_geometry;   // collision lines
//!     ...                             // bounds, fog, light angle, BGM
//! };
//! ```
//!
//! `MPGroundDesc` pairs a scene graph with its material table exactly as
//! `FTCommonPart` does for fighters (see [`crate::mobj::PartTables`]) — but
//! with `anim_joints` in between, so the table is at `dobjdesc + 8` rather
//! than `+ 4`. That one word of difference is why every stage layer went
//! unmatched while every fighter matched.
//!
//! The header also carries the camera and map bounds a match needs, so they
//! are read here too rather than rediscovered later.

use alloc::vec::Vec;

use crate::archive::File;

/// `sizeof(MPGroundData)`.
///
/// Confirmed rather than assumed: three `GR*Map` files place the header at
/// `0x14` and the decomp names the next symbol at `0xBC`, and `0xBC - 0x14`
/// is exactly this.
pub const GROUND_DATA_SIZE: u32 = 0xA8;

/// `sizeof(MPGroundDesc)`.
const DESC_SIZE: u32 = 0x10;
const LAYERS: u32 = 4;

// Field offsets within `MPGroundDesc`.
const D_DOBJDESC: u32 = 0x00;
const D_ANIM_JOINTS: u32 = 0x04;
const D_MOBJSUBS: u32 = 0x08;
const D_MATANIM_JOINTS: u32 = 0x0C;

// Field offsets within `MPGroundData`.
const G_MAP_GEOMETRY: u32 = 0x40;
/// `Vec3f light_angle`. Immediately precedes `camera_bound_top`
/// (`0x6C - sizeof(Vec3f)`) — confirmed by that adjacency, not assumed: three
/// `GR*Map` files place `unused` (the `s32` right before it) at `0x5C`, so
/// `0x5C + 4 = 0x60` lines up exactly. `.x`/`.y`, in *degrees*, are read by
/// `ftDisplayLightsDrawReflect` (`refs/ssb-decomp-re/src/ft/ftdisplaylights.c`).
/// `.z` was previously recorded as having no known reader, which turned out
/// to be wrong (RE-131): `gm/gmcamera.c`'s `gmCameraGetAdjustAtAngle` reads
/// it too, folded into the real camera's own eye-direction angle — and
/// measured archive-wide (`romtool stages`), it is stored pre-converted to
/// *radians* already, unlike `.x`/`.y`: the overwhelmingly common value is
/// exactly `-0.17453294` (`-10.0°` to five decimal places), never a
/// degrees-scale number, and `gmCameraGetAdjustAtAngle` itself adds it
/// directly to an already-radians angle with no `F_CLC_DTOR32()` conversion
/// of its own — confirming the source, not just the sink, agrees.
const G_LIGHT_ANGLE: u32 = 0x60;
const G_CAMERA_BOUNDS: u32 = 0x6C;
const G_MAP_BOUNDS: u32 = 0x74;
const G_BGM_ID: u32 = 0x7C;
const G_MAP_NODES: u32 = 0x80;

/// A pointer that has been followed to the file it lands in.
pub type Target = (u32, u32);

/// One of a stage's four render layers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GroundLayer {
    /// Which layer slot this is, 0..4. Kept because empty slots are skipped
    /// and `layer_mask` indexes by slot.
    pub index: u32,
    /// The layer's `DObjDesc` array.
    pub graph: Target,
    /// Its `MObjSub **table[]`, if the layer has one.
    pub mobjsub_table: Option<Target>,
    /// `AObjEvent32 **anim_joints` — one joint-animation script per node, which
    /// is how Whispy sways and the flowers move. Not yet played.
    pub anim_joints: Option<Target>,
    /// `AObjEvent32 ***p_matanim_joints` — one material-animation script per
    /// `MObj`. As for a fighter's costume list (RE-040), frame 0 of this is the
    /// material's *initial* state, and the values baked into `MObjSub` need not
    /// be it.
    pub matanim_joints: Option<Target>,
}

/// A camera or map extent, in game units.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Bounds {
    pub top: i16,
    pub bottom: i16,
    pub right: i16,
    pub left: i16,
}

impl Bounds {
    /// Whether this reads as a real extent rather than arbitrary bytes.
    ///
    /// A degenerate box would mean the camera could not move, so the game
    /// never ships one; requiring a positive area is a cheap check that no
    /// run of unrelated words passes by accident.
    fn plausible(&self) -> bool {
        self.top > self.bottom && self.right > self.left
    }
}

/// A stage's `MPGroundData` header.
#[derive(Debug, Clone, PartialEq)]
pub struct GroundData {
    /// File holding the header, and its offset within it.
    pub file: u32,
    pub offset: u32,
    /// Non-empty layer slots, in slot order.
    pub layers: Vec<GroundLayer>,
    /// `MPGeometryData`, the collision lines. Not yet decoded.
    pub map_geometry: Option<Target>,
    /// A further `DObjDesc` array, used for stage-specific scenery.
    pub map_nodes: Option<Target>,
    pub camera_bounds: Bounds,
    pub map_bounds: Bounds,
    pub bgm_id: u32,
    /// `light_angle`'s three raw components, in **mixed units** (matches the
    /// ROM's own storage, not normalised here): `.x`/`.y`, in *degrees*, are
    /// the angles `ftDisplayLightsDrawReflect` turns into the key light
    /// every fighter (and, per R0.6/RE-065, this project's baked vertex
    /// shading) is lit by while standing on this stage. `.z`, in *radians*
    /// already (RE-131, measured archive-wide — see [`G_LIGHT_ANGLE`]'s own
    /// doc comment), is the real camera's own eye-direction nudge
    /// (`gmCameraGetAdjustAtAngle`) — a second, unrelated use of the same
    /// three stored floats, not a lighting angle at all despite living in
    /// the same field.
    pub light_angle: [f32; 3],
}

fn read_u32(data: &[u8], at: u32) -> Option<u32> {
    let at = at as usize;
    Some(u32::from_be_bytes(data.get(at..at + 4)?.try_into().ok()?))
}

fn read_i16(data: &[u8], at: u32) -> Option<i16> {
    let at = at as usize;
    Some(i16::from_be_bytes(data.get(at..at + 2)?.try_into().ok()?))
}

fn read_f32(data: &[u8], at: u32) -> Option<f32> {
    let at = at as usize;
    Some(f32::from_bits(u32::from_be_bytes(
        data.get(at..at + 4)?.try_into().ok()?,
    )))
}

fn read_bounds(data: &[u8], at: u32) -> Option<Bounds> {
    Some(Bounds {
        top: read_i16(data, at)?,
        bottom: read_i16(data, at + 2)?,
        right: read_i16(data, at + 4)?,
        left: read_i16(data, at + 6)?,
    })
}

/// Resolves the pointer slot at `at` to the file and offset it targets.
///
/// A stage header points mostly *out* of its own file, so extern relocations
/// carry the load here; intern ones are handled for completeness.
fn target(file: &File, at: u32) -> Option<Target> {
    if let Some(r) = file.extern_relocs.iter().find(|r| r.at == at) {
        return Some((r.target_file as u32, r.target_offset));
    }
    let r = file.intern_relocs.iter().find(|r| r.at == at)?;
    Some((file.id, r.target))
}

/// True when the slot at `at` holds neither a pointer nor NULL — which means
/// whatever is here is not a pointer field, so the candidate is not a header.
fn is_junk(file: &File, at: u32) -> bool {
    target(file, at).is_none() && read_u32(&file.data, at) != Some(0)
}

/// Finds every `MPGroundData` header in a file.
///
/// `is_graph(file, offset)` reports whether a recovered `DObjDesc` array
/// starts there. At least one layer has to name one: that is the anchor, and
/// it comes from data the header itself does not contain.
pub fn find_ground_data(file: &File, is_graph: impl Fn(u32, u32) -> bool) -> Vec<GroundData> {
    // Only offsets where some layer's `dobjdesc` could sit need trying, and a
    // relocation is the only thing that slot can hold.
    let mut candidates: Vec<u32> = file
        .extern_relocs
        .iter()
        .map(|r| r.at)
        .chain(file.intern_relocs.iter().map(|r| r.at))
        .flat_map(|at| (0..LAYERS).filter_map(move |i| at.checked_sub(i * DESC_SIZE)))
        .collect();
    candidates.sort_unstable();
    candidates.dedup();

    let mut out: Vec<GroundData> = Vec::new();
    for base in candidates {
        let Some(header) = read_ground_data(file, base, &is_graph) else {
            continue;
        };
        // Headers do not overlap; the first accepted one wins its span. Four
        // layers means four aligned offsets would otherwise each re-detect the
        // same header shifted by a descriptor.
        if out
            .last()
            .is_some_and(|p| base < p.offset + GROUND_DATA_SIZE)
        {
            continue;
        }
        out.push(header);
    }
    out
}

/// Reads a header at a known offset, or `None` if the bytes there are not one.
pub fn read_ground_data(
    file: &File,
    base: u32,
    is_graph: impl Fn(u32, u32) -> bool,
) -> Option<GroundData> {
    if base.checked_add(GROUND_DATA_SIZE)? as usize > file.data.len() {
        return None;
    }

    let mut layers = Vec::new();
    for index in 0..LAYERS {
        let desc = base + index * DESC_SIZE;
        // Every word in a descriptor is a pointer, so a non-zero non-pointer
        // anywhere in the block rules the candidate out.
        if (0..4).any(|w| is_junk(file, desc + w * 4)) {
            return None;
        }
        let Some(graph) = target(file, desc + D_DOBJDESC) else {
            continue;
        };
        if !is_graph(graph.0, graph.1) {
            return None;
        }
        layers.push(GroundLayer {
            index,
            graph,
            mobjsub_table: target(file, desc + D_MOBJSUBS),
            anim_joints: target(file, desc + D_ANIM_JOINTS),
            matanim_joints: target(file, desc + D_MATANIM_JOINTS),
        });
    }
    if layers.is_empty() {
        return None;
    }

    let camera_bounds = read_bounds(&file.data, base + G_CAMERA_BOUNDS)?;
    let map_bounds = read_bounds(&file.data, base + G_MAP_BOUNDS)?;
    if !camera_bounds.plausible() || !map_bounds.plausible() {
        return None;
    }
    let light_angle = [
        read_f32(&file.data, base + G_LIGHT_ANGLE)?,
        read_f32(&file.data, base + G_LIGHT_ANGLE + 4)?,
        read_f32(&file.data, base + G_LIGHT_ANGLE + 8)?,
    ];

    Some(GroundData {
        file: file.id,
        offset: base,
        layers,
        map_geometry: target(file, base + G_MAP_GEOMETRY),
        map_nodes: target(file, base + G_MAP_NODES),
        camera_bounds,
        map_bounds,
        bgm_id: read_u32(&file.data, base + G_BGM_ID)?,
        light_angle,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::{ExternReloc, InternReloc};
    use alloc::vec;

    const GRAPH_FILE: u16 = 104;

    /// A header at `base` with two layers, the second carrying a material
    /// table, laid out the way `GRPupupuMap` lays one out.
    fn fixture(base: u32) -> File {
        let mut data = vec![0u8; (base + GROUND_DATA_SIZE + 0x10) as usize];
        let mut externs = Vec::new();
        let mut put = |at: u32, target_offset: u32| {
            data[at as usize..at as usize + 4].copy_from_slice(&target_offset.to_be_bytes());
            externs.push(ExternReloc {
                at,
                target_file: GRAPH_FILE,
                target_offset,
            });
        };
        put(base + D_DOBJDESC, 0x1008);
        put(base + DESC_SIZE + D_DOBJDESC, 0x2450);
        put(base + DESC_SIZE + D_MOBJSUBS, 0x1F50);
        put(base + DESC_SIZE + D_MATANIM_JOINTS, 0x2530);
        put(base + G_MAP_GEOMETRY, 0x1F34);
        put(base + G_MAP_NODES, 0x10F0);

        let mut bounds = |at: u32, v: [i16; 4]| {
            for (i, x) in v.iter().enumerate() {
                let at = at as usize + i * 2;
                data[at..at + 2].copy_from_slice(&x.to_be_bytes());
            }
        };
        bounds(base + G_CAMERA_BOUNDS, [4000, -2000, 3900, -3900]);
        bounds(base + G_MAP_BOUNDS, [8300, -3500, 9000, -9000]);

        let angle_x = (base + G_LIGHT_ANGLE) as usize;
        data[angle_x..angle_x + 4].copy_from_slice(&30.0f32.to_be_bytes());
        let angle_y = (base + G_LIGHT_ANGLE + 4) as usize;
        data[angle_y..angle_y + 4].copy_from_slice(&(-40.0f32).to_be_bytes());
        let angle_z = (base + G_LIGHT_ANGLE + 8) as usize;
        data[angle_z..angle_z + 4].copy_from_slice(&5.0f32.to_be_bytes());

        File {
            id: 255,
            data,
            extern_relocs: externs,
            intern_relocs: Vec::new(),
        }
    }

    fn graphs(f: u32, o: u32) -> bool {
        f == GRAPH_FILE as u32 && matches!(o, 0x1008 | 0x2450)
    }

    #[test]
    fn reads_a_stage_header_and_its_layers() {
        let found = find_ground_data(&fixture(0x14), graphs);
        assert_eq!(found.len(), 1);
        let h = &found[0];
        assert_eq!(h.offset, 0x14);
        assert_eq!(h.layers.len(), 2);
        assert_eq!(h.layers[0].index, 0);
        assert_eq!(h.layers[0].graph, (104, 0x1008));
        assert_eq!(h.layers[0].mobjsub_table, None);
        // The table sits at `dobjdesc + 8`, past `anim_joints`. Reading it at
        // +4 the way an `FTCommonPart` is read would find nothing here.
        assert_eq!(h.layers[1].mobjsub_table, Some((104, 0x1F50)));
        // The two animation pointers sit at +4 and +0xC, either side of the
        // material table. Nothing plays them yet, but reading them is what says
        // *which* layers move — 40 of 100 carry joint animation (RE-048).
        assert_eq!(h.layers[1].anim_joints, None);
        assert_eq!(h.layers[1].matanim_joints, Some((104, 0x2530)));
        assert_eq!(h.map_geometry, Some((104, 0x1F34)));
        assert_eq!(h.camera_bounds.top, 4000);
        assert_eq!(h.map_bounds.left, -9000);
        assert_eq!(h.light_angle, [30.0, -40.0, 5.0]);
    }

    #[test]
    fn a_header_is_reported_once_not_once_per_layer() {
        // Each of the four descriptor offsets generates a candidate base, and
        // three of them are wrong by a multiple of 0x10.
        assert_eq!(find_ground_data(&fixture(0x14), graphs).len(), 1);
    }

    #[test]
    fn a_layer_pointing_somewhere_that_is_not_a_graph_is_rejected() {
        assert!(find_ground_data(&fixture(0x14), |_, _| false).is_empty());
    }

    #[test]
    fn a_degenerate_bounding_box_is_rejected() {
        let mut file = fixture(0x14);
        // camera_bound_top = camera_bound_bottom: no room for the camera to
        // move, which no shipped stage has.
        let at = (0x14 + G_CAMERA_BOUNDS) as usize;
        file.data[at..at + 2].copy_from_slice(&(-2000i16).to_be_bytes());
        assert!(find_ground_data(&file, graphs).is_empty());
    }

    #[test]
    fn a_descriptor_word_that_is_not_a_pointer_rules_the_candidate_out() {
        let mut file = fixture(0x14);
        // `anim_joints` holding a non-zero, unrelocated word means this block
        // is not four `MPGroundDesc`s.
        let at = (0x14 + 0x04) as usize;
        file.data[at..at + 4].copy_from_slice(&0xDEAD_BEEFu32.to_be_bytes());
        assert!(find_ground_data(&file, graphs).is_empty());
    }

    #[test]
    fn an_intern_pointer_resolves_to_the_same_file() {
        let mut file = fixture(0x14);
        file.extern_relocs.retain(|r| r.at != 0x14 + G_MAP_GEOMETRY);
        file.intern_relocs.push(InternReloc {
            at: 0x14 + G_MAP_GEOMETRY,
            target: 0x90,
        });
        let found = find_ground_data(&file, graphs);
        assert_eq!(found[0].map_geometry, Some((255, 0x90)));
    }
}
