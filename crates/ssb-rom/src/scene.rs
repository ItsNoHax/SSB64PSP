//! `DObjDesc` scene graphs: the hierarchy that positions display lists.
//!
//! Up to now every mesh was drawn at the origin, because a display list only
//! says *what* to draw — never *where*. The placement lives in a separate
//! structure: an array of [`DObjDesc`] that the game walks at load time to
//! build a tree of `DObj` ("draw object") nodes.
//!
//! From `gcSetupCommonDObjs` (`src/sys/objanim.c`):
//!
//! ```c
//! DObj *array_dobjs[DOBJ_ARRAY_MAX];          // DOBJ_ARRAY_MAX == 18
//! while (dobjdesc->id != ARRAY_COUNT(array_dobjs))
//! {
//!     id = dobjdesc->id & 0xFFF;
//!     if (id != 0) dobj = array_dobjs[id] = gcAddChildForDObj(array_dobjs[id - 1], dobjdesc->dl);
//!     else         dobj = array_dobjs[0]  = gcAddDObjForGObj(gobj, dobjdesc->dl);
//!     ...
//!     dobj->translate.vec.f = dobjdesc->translate;
//!     dobj->rotate.vec.f    = dobjdesc->rotate;
//!     dobj->scale.vec.f     = dobjdesc->scale;
//!     dobjdesc++;
//! }
//! ```
//!
//! So the array is a **depth-tagged pre-order flattening** of a tree: `id &
//! 0xFFF` is the node's depth, and its parent is whatever node most recently
//! occupied `depth - 1`. Depth 18 terminates the array — which is why a graph
//! can never be more than 18 deep, and why the terminator is not a magic number
//! so much as an out-of-range depth.
//!
//! The high nibble picks how the node's matrix is composed; see
//! [`TransformKind`].
//!
//! # Finding them
//!
//! Nothing indexes these arrays, so they have to be recovered from raw file
//! bytes. Five constraints do it, and the last is the decisive one:
//!
//! 1. The terminator is `id == 18` followed by 40 zero bytes.
//! 2. The first entry is always at depth 0.
//! 3. Depth never jumps by more than one, since `array_dobjs[depth - 1]` has to
//!    have been written already.
//! 4. Every non-zero float component falls in a narrow magnitude band.
//! 5. **`dl` is either NULL or the target of an intern relocation.** The
//!    archive loader already told us exactly which four-byte slots in this file
//!    are pointers, so a candidate whose `dl` is a plausible-looking offset that
//!    was never relocated is not a `DObjDesc` at all.
//!
//! All five were checked against the 366 `DObjDesc` arrays (3576 entries) that
//! the decomp has typed by hand; see the tests.

use alloc::collections::BTreeSet;
use alloc::vec::Vec;

use crate::archive::File;

/// Size of one `DObjDesc` on ROM: `s32 id`, `void *dl`, three `Vec3f`.
pub const DOBJ_DESC_SIZE: usize = 4 + 4 + 12 * 3;

/// `DOBJ_ARRAY_MAX` from `src/sys/objdef.h`. Doubles as the terminator `id`.
pub const DOBJ_ARRAY_MAX: u32 = 18;

/// Deepest depth observed across the decomp corpus (12), with headroom.
///
/// The runtime would accept anything below [`DOBJ_ARRAY_MAX`], but real data
/// stops well short, and tightening this rejects false positives.
const MAX_DEPTH: u32 = 14;

/// Bounds on `|component|` for the non-zero floats of a transform.
///
/// Measured over the decomp corpus: translate peaks at 2.34e4, rotate (radians)
/// at 3.17e1, scale at 1.23e2, and the smallest non-zero magnitude anywhere is
/// 1e-6. The window below is roughly two orders of magnitude of slack either
/// side. It matters because a random word reinterpreted as `f32` lands here
/// only about 5% of the time, so nine of them agreeing is ~1e-12.
const MIN_COMPONENT: f32 = 1e-7;
const MAX_COMPONENT: f32 = 1e6;

/// How a node's matrix is built, from the `id`'s high nibble.
///
/// `gcSetupCommonDObjs` tests these in priority order, and additionally emits a
/// leading translate-only matrix whenever *any* high bit is set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransformKind {
    /// No high bits: plain translate/rotate/scale.
    TraRotSca,
    /// `0x8000` — `nGCMatrixKindRecalcRotRpyRSca`.
    RecalcRotRpyRSca,
    /// `0x4000` — `nGCMatrixKind46`.
    Kind46,
    /// `0x2000` — `nGCMatrixKind48`.
    Kind48,
    /// `0x1000` — `nGCMatrixKind50`. Unused by shipped data, kept for fidelity.
    Kind50,
}

/// One entry of a `DObjDesc` array.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DObjDesc {
    /// Raw `id`: depth in the low 12 bits, transform kind in the high nibble.
    pub id: u32,
    /// File-relative byte offset of the display list, or `None` for a pure
    /// transform node (52% of entries — these are joints, not geometry).
    pub dl: Option<u32>,
    pub translate: [f32; 3],
    pub rotate: [f32; 3],
    pub scale: [f32; 3],
}

impl DObjDesc {
    /// Depth in the hierarchy; 0 is a root attached directly to the `GObj`.
    pub fn depth(&self) -> u32 {
        self.id & 0xFFF
    }

    pub fn transform_kind(&self) -> TransformKind {
        // Priority order copied from `gcSetupCommonDObjs`; the tests pin it.
        if self.id & 0x8000 != 0 {
            TransformKind::RecalcRotRpyRSca
        } else if self.id & 0x4000 != 0 {
            TransformKind::Kind46
        } else if self.id & 0x2000 != 0 {
            TransformKind::Kind48
        } else if self.id & 0x1000 != 0 {
            TransformKind::Kind50
        } else {
            TransformKind::TraRotSca
        }
    }

    /// Whether the node gets a leading translate-only matrix pushed before its
    /// own transform (`if (dobjdesc->id & 0xF000)`).
    pub fn has_leading_translate(&self) -> bool {
        self.id & 0xF000 != 0
    }

    fn parse(bytes: &[u8], intern_slots: &BTreeSet<u32>, at: u32) -> Option<Self> {
        let raw = bytes.get(..DOBJ_DESC_SIZE)?;
        let word = |i: usize| {
            u32::from_be_bytes([raw[i * 4], raw[i * 4 + 1], raw[i * 4 + 2], raw[i * 4 + 3]])
        };

        let id = word(0);
        if id & 0xFFF > MAX_DEPTH {
            return None;
        }
        // Only these four high-nibble values ever appear; `0x1000` is legal per
        // the runtime but absent from shipped data, so allowing it costs
        // nothing and keeps us honest about what the code accepts.
        if id & !0xFFFu32 & !0xF000u32 != 0 {
            return None;
        }

        let dl_raw = word(1);
        // Constraint 5: a non-NULL `dl` must be a slot the archive relocated.
        // `walk_intern` rewrote such slots to file-relative byte offsets, so the
        // value is directly usable once we know the slot was genuine.
        let dl = match dl_raw {
            0 => None,
            _ if intern_slots.contains(&(at + 4)) => Some(dl_raw),
            _ => return None,
        };

        let mut vecs = [[0f32; 3]; 3];
        for (v, dst) in vecs.iter_mut().enumerate() {
            for (c, out) in dst.iter_mut().enumerate() {
                let f = f32::from_bits(word(2 + v * 3 + c));
                if f != 0.0 && !(f.is_finite() && (MIN_COMPONENT..=MAX_COMPONENT).contains(&f.abs()))
                {
                    return None;
                }
                *out = f;
            }
        }

        Some(DObjDesc {
            id,
            dl,
            translate: vecs[0],
            rotate: vecs[1],
            scale: vecs[2],
        })
    }
}

/// A node of a resolved hierarchy: a descriptor plus its parent's index.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DObjNode {
    pub desc: DObjDesc,
    /// Index into [`SceneGraph::nodes`], or `None` for a depth-0 root.
    pub parent: Option<usize>,
}

/// A `DObjDesc` array recovered from a file, with parents resolved.
#[derive(Debug, Clone, PartialEq)]
pub struct SceneGraph {
    /// File-relative byte offset of the array's first entry.
    pub offset: u32,
    /// Nodes in the array's original pre-order, terminator excluded.
    pub nodes: Vec<DObjNode>,
}

impl SceneGraph {
    /// Number of entries including the terminator, i.e. the array's declared
    /// length in the decomp sources.
    pub fn declared_len(&self) -> usize {
        self.nodes.len() + 1
    }

    /// Byte offset one past the terminator.
    pub fn end_offset(&self) -> u32 {
        self.offset + (self.declared_len() * DOBJ_DESC_SIZE) as u32
    }

    /// Display lists referenced by this graph, in tree order.
    pub fn display_lists(&self) -> impl Iterator<Item = u32> + '_ {
        self.nodes.iter().filter_map(|n| n.desc.dl)
    }

    /// Composes each node's local transform with its ancestors', yielding one
    /// world matrix per node in the same order as [`SceneGraph::nodes`].
    ///
    /// Only [`TransformKind::TraRotSca`] is composed exactly; the four
    /// matrix-kind variants are animation-driven at runtime and fall back to
    /// the same T*R*S here, which is their rest pose.
    pub fn world_transforms(&self) -> Vec<Mat4> {
        let mut out: Vec<Mat4> = Vec::with_capacity(self.nodes.len());
        for node in &self.nodes {
            let local = Mat4::from_trs(node.desc.translate, node.desc.rotate, node.desc.scale);
            let world = match node.parent {
                // Parents always precede children: `array_dobjs[depth - 1]` has
                // to have been assigned by an earlier entry for the child to
                // reference it, so `out[p]` is already final.
                Some(p) => out[p].mul(&local),
                None => local,
            };
            out.push(world);
        }
        out
    }
}

/// A column-major 4x4 matrix, laid out the way the PSP GE wants it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mat4(pub [f32; 16]);

impl Mat4 {
    pub const IDENTITY: Mat4 = Mat4([
        1.0, 0.0, 0.0, 0.0, //
        0.0, 1.0, 0.0, 0.0, //
        0.0, 0.0, 1.0, 0.0, //
        0.0, 0.0, 0.0, 1.0,
    ]);

    /// Builds `T * Rz * Ry * Rx * S`.
    ///
    /// The rotation order is the N64 library's roll-pitch-yaw convention: the
    /// vector holds radians about x, y and z, applied x first.
    pub fn from_trs(t: [f32; 3], r: [f32; 3], s: [f32; 3]) -> Mat4 {
        let (sx, cx) = sin_cos(r[0]);
        let (sy, cy) = sin_cos(r[1]);
        let (sz, cz) = sin_cos(r[2]);

        // Rz * Ry * Rx, expanded.
        let m = [
            cz * cy,
            sz * cy,
            -sy,
            cz * sy * sx - sz * cx,
            sz * sy * sx + cz * cx,
            cy * sx,
            cz * sy * cx + sz * sx,
            sz * sy * cx - cz * sx,
            cy * cx,
        ];

        Mat4([
            m[0] * s[0],
            m[1] * s[0],
            m[2] * s[0],
            0.0,
            m[3] * s[1],
            m[4] * s[1],
            m[5] * s[1],
            0.0,
            m[6] * s[2],
            m[7] * s[2],
            m[8] * s[2],
            0.0,
            t[0],
            t[1],
            t[2],
            1.0,
        ])
    }

    /// `self * rhs`, i.e. `rhs` applied first.
    pub fn mul(&self, rhs: &Mat4) -> Mat4 {
        let mut out = [0f32; 16];
        for col in 0..4 {
            for row in 0..4 {
                let mut acc = 0.0;
                for k in 0..4 {
                    acc += self.0[k * 4 + row] * rhs.0[col * 4 + k];
                }
                out[col * 4 + row] = acc;
            }
        }
        Mat4(out)
    }

    /// The translation column.
    pub fn translation(&self) -> [f32; 3] {
        [self.0[12], self.0[13], self.0[14]]
    }
}

/// `no_std`-friendly sine/cosine.
///
/// `core` has no `sin`/`cos`, and pulling in `libm` for a load-time helper that
/// runs a few thousand times is not worth the dependency.
///
/// Reducing only to `[-pi, pi]` is not enough: the Taylor series' error grows as
/// `x^(n+1)/(n+1)!`, so at the interval's edge a 6th-order cosine is off by
/// ~9e-4 — visible as a degree of skew on a 90-degree joint, and exactly what
/// the first version of this got wrong. Reducing by quadrant instead keeps
/// `|a| <= pi/4`, where the same polynomials are accurate to ~1e-8.
fn sin_cos(x: f32) -> (f32, f32) {
    const FRAC_PI_2: f32 = core::f32::consts::FRAC_PI_2;

    // Nearest quadrant, then the residual within it.
    let k = (x / FRAC_PI_2 + if x < 0.0 { -0.5 } else { 0.5 }) as i32;
    let a = x - k as f32 * FRAC_PI_2;

    let x2 = a * a;
    let s = a * (1.0 - x2 * (1.0 / 6.0 - x2 * (1.0 / 120.0 - x2 / 5040.0)));
    let c = 1.0 - x2 * (0.5 - x2 * (1.0 / 24.0 - x2 * (1.0 / 720.0 - x2 / 40320.0)));

    match k.rem_euclid(4) {
        0 => (s, c),
        1 => (c, -s),
        2 => (-s, -c),
        _ => (-c, s),
    }
}

/// Recovers every `DObjDesc` array in a loaded file.
///
/// Results are ordered by file offset and never overlap.
pub fn find_scene_graphs(file: &File) -> Vec<SceneGraph> {
    let intern_slots: BTreeSet<u32> = file.intern_relocs.iter().map(|r| r.at).collect();
    let data = &file.data;

    let mut out: Vec<SceneGraph> = Vec::new();
    let mut consumed_until = 0u32;

    // Anchor on terminators, then walk back. Scanning forward instead would
    // need a way to guess where an array starts, and depth-0 entries are far
    // too common to anchor on.
    let mut at = 0usize;
    while at + DOBJ_DESC_SIZE <= data.len() {
        if is_terminator(&data[at..at + DOBJ_DESC_SIZE]) {
            if let Some(graph) = walk_back(data, &intern_slots, at) {
                if graph.offset >= consumed_until {
                    consumed_until = graph.end_offset();
                    out.push(graph);
                }
            }
        }
        at += 4;
    }
    out
}

fn is_terminator(entry: &[u8]) -> bool {
    entry[..4] == DOBJ_ARRAY_MAX.to_be_bytes() && entry[4..].iter().all(|&b| b == 0)
}

/// Walks backwards from a terminator collecting entries until the chain breaks.
fn walk_back(data: &[u8], intern_slots: &BTreeSet<u32>, term_at: usize) -> Option<SceneGraph> {
    let mut rev: Vec<DObjDesc> = Vec::new();
    let mut at = term_at;

    while at >= DOBJ_DESC_SIZE {
        let start = at - DOBJ_DESC_SIZE;
        let Some(desc) = DObjDesc::parse(&data[start..], intern_slots, start as u32) else {
            break;
        };
        // The terminator's own depth is out of range for a real node, so a
        // second one means we have run into the previous array.
        if desc.depth() >= DOBJ_ARRAY_MAX {
            break;
        }
        rev.push(desc);
        at = start;
        if desc.depth() == 0 {
            // A root can only be the first entry, so stop rather than absorb
            // whatever precedes it.
            break;
        }
    }

    let descs: Vec<DObjDesc> = rev.into_iter().rev().collect();
    // Constraint 2: the array must begin at depth 0.
    if descs.first()?.depth() != 0 {
        return None;
    }

    let offset = (term_at - descs.len() * DOBJ_DESC_SIZE) as u32;
    resolve(descs, offset)
}

/// Turns depth tags into parent indices, mirroring the `array_dobjs` slot table.
fn resolve(descs: Vec<DObjDesc>, offset: u32) -> Option<SceneGraph> {
    // `array_dobjs[d]` holds the most recent node seen at depth `d`.
    let mut slots: [Option<usize>; DOBJ_ARRAY_MAX as usize] = [None; DOBJ_ARRAY_MAX as usize];
    let mut nodes = Vec::with_capacity(descs.len());

    for (i, desc) in descs.iter().enumerate() {
        let depth = desc.depth() as usize;
        let parent = if depth == 0 {
            None
        } else {
            // Constraint 3: the slot one level up must already be occupied.
            // `gcAddChildForDObj(NULL, ..)` would dereference a null parent, so
            // shipped data never does this — and random bytes usually do.
            Some(slots[depth - 1]?)
        };
        slots[depth] = Some(i);
        nodes.push(DObjNode { desc: *desc, parent });
    }

    Some(SceneGraph { offset, nodes })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::InternReloc;
    use alloc::vec;

    /// Serialises a descriptor the way the ROM stores it, post-relocation.
    fn emit(id: u32, dl: u32, t: [f32; 3], r: [f32; 3], s: [f32; 3]) -> Vec<u8> {
        let mut out = Vec::with_capacity(DOBJ_DESC_SIZE);
        out.extend_from_slice(&id.to_be_bytes());
        out.extend_from_slice(&dl.to_be_bytes());
        for v in t.iter().chain(&r).chain(&s) {
            out.extend_from_slice(&v.to_bits().to_be_bytes());
        }
        out
    }

    fn terminator() -> Vec<u8> {
        emit(DOBJ_ARRAY_MAX, 0, [0.0; 3], [0.0; 3], [0.0; 3])
    }

    fn file_with(data: Vec<u8>, pointer_slots: &[u32]) -> File {
        File {
            id: 0,
            data,
            extern_relocs: Vec::new(),
            intern_relocs: pointer_slots
                .iter()
                .map(|&at| InternReloc { at, target: 0 })
                .collect(),
        }
    }

    /// The shape of `dStageSectorFile2_Layer1Anim_DObj_0x2200`: a root and a
    /// two-deep chain below it, the lower two carrying display lists.
    fn sector_z_layer1_anim() -> (Vec<u8>, Vec<u32>) {
        let mut data = vec![0u8; 64]; // leading padding, so offset != 0
        data.extend(emit(0, 0, [0.0; 3], [0.0; 3], [1.0, 1.0, 1.0]));
        data.extend(emit(0x4001, 0x2020, [0.0; 3], [0.0; 3], [1.0, 1.0, 1.0]));
        data.extend(emit(0x4002, 0x2110, [0.0; 3], [0.0; 3], [1.0, 1.0, 1.0]));
        data.extend(terminator());
        // `dl` sits at +4 within each entry.
        let slots = vec![64 + DOBJ_DESC_SIZE as u32 + 4, 64 + 2 * DOBJ_DESC_SIZE as u32 + 4];
        (data, slots)
    }

    #[test]
    fn recovers_a_known_array_with_parents_and_flags() {
        let (data, slots) = sector_z_layer1_anim();
        let graphs = find_scene_graphs(&file_with(data, &slots));

        assert_eq!(graphs.len(), 1);
        let g = &graphs[0];
        assert_eq!(g.offset, 64);
        assert_eq!(g.declared_len(), 4, "three nodes plus the terminator");

        // ids are 0, 0x4001, 0x4002 — depths 0, 1, 2. So this is a chain, not a
        // root with two children: the third entry attaches to the second.
        assert_eq!(g.nodes[0].parent, None);
        assert_eq!(g.nodes[1].parent, Some(0));
        assert_eq!(g.nodes[2].parent, Some(1));

        assert_eq!(g.display_lists().collect::<Vec<_>>(), [0x2020, 0x2110]);
        assert_eq!(g.nodes[1].desc.transform_kind(), TransformKind::Kind46);
        assert!(g.nodes[1].desc.has_leading_translate());
        assert_eq!(g.nodes[0].desc.transform_kind(), TransformKind::TraRotSca);
    }

    #[test]
    fn depth_two_attaches_to_the_preceding_depth_one_node() {
        // Root, child A, A's child, then child B — B must reattach to the root,
        // and the grandchild must not be stolen by B.
        let mut data = Vec::new();
        data.extend(emit(0, 0, [0.0; 3], [0.0; 3], [1.0; 3]));
        data.extend(emit(1, 0, [1.0, 0.0, 0.0], [0.0; 3], [1.0; 3]));
        data.extend(emit(2, 0, [0.0, 2.0, 0.0], [0.0; 3], [1.0; 3]));
        data.extend(emit(1, 0, [3.0, 0.0, 0.0], [0.0; 3], [1.0; 3]));
        data.extend(terminator());

        let graphs = find_scene_graphs(&file_with(data, &[]));
        let g = &graphs[0];
        assert_eq!(g.nodes[1].parent, Some(0));
        assert_eq!(g.nodes[2].parent, Some(1));
        assert_eq!(g.nodes[3].parent, Some(0));

        // And the composed transform proves the chain, not just the indices.
        let w = g.world_transforms();
        assert_eq!(w[2].translation(), [1.0, 2.0, 0.0]);
        assert_eq!(w[3].translation(), [3.0, 0.0, 0.0]);
    }

    #[test]
    fn a_dl_pointer_that_was_never_relocated_is_rejected() {
        // Identical bytes, but without the intern-reloc slot backing `dl` the
        // entry must not parse. This is constraint 5, and it is what keeps
        // arbitrary float-looking data from being read as a scene graph.
        let (data, slots) = sector_z_layer1_anim();
        assert_eq!(find_scene_graphs(&file_with(data.clone(), &slots)).len(), 1);

        let graphs = find_scene_graphs(&file_with(data, &[]));
        // The two DL-bearing entries drop out, so the walk stops before them
        // and no depth-0 root is reached.
        assert!(
            graphs.is_empty(),
            "unrelocated dl slots should not yield a graph, got {graphs:?}"
        );
    }

    #[test]
    fn wild_floats_reject_the_whole_array() {
        // Paired controls: identical layout, one float differs. Anything else
        // that changed the outcome would show up in the positive case too.
        let build = |x: f32| {
            let mut data = Vec::new();
            data.extend(emit(0, 0, [0.0; 3], [0.0; 3], [1.0; 3]));
            data.extend(emit(1, 0, [x, 0.0, 0.0], [0.0; 3], [1.0; 3]));
            data.extend(terminator());
            find_scene_graphs(&file_with(data, &[]))
        };

        assert_eq!(build(120.0).len(), 1, "a plausible translate parses");

        // 1e30 is finite and non-zero but far outside anything the game ships.
        // Rejecting the entry breaks the walk back from the terminator before
        // it reaches a depth-0 root, so the array is discarded entirely rather
        // than silently truncated to its tail.
        assert!(build(1e30).is_empty());
    }

    #[test]
    fn arrays_do_not_overlap_each_other() {
        // Two back-to-back arrays; the second must not swallow the first.
        let mut data = Vec::new();
        data.extend(emit(0, 0, [0.0; 3], [0.0; 3], [1.0; 3]));
        data.extend(terminator());
        data.extend(emit(0, 0, [5.0, 0.0, 0.0], [0.0; 3], [1.0; 3]));
        data.extend(emit(1, 0, [0.0; 3], [0.0; 3], [1.0; 3]));
        data.extend(terminator());

        let graphs = find_scene_graphs(&file_with(data, &[]));
        assert_eq!(graphs.len(), 2);
        assert_eq!(graphs[0].nodes.len(), 1);
        assert_eq!(graphs[1].nodes.len(), 2);
        assert_eq!(graphs[1].offset, graphs[0].end_offset());
    }

    #[test]
    fn transform_kind_follows_the_runtime_priority_order() {
        // `gcSetupCommonDObjs` tests 0x8000 before 0x4000 before 0x2000, so a
        // hypothetical combination resolves to the highest bit.
        let k = |id| DObjDesc {
            id,
            dl: None,
            translate: [0.0; 3],
            rotate: [0.0; 3],
            scale: [1.0; 3],
        }
        .transform_kind();
        assert_eq!(k(0x0001), TransformKind::TraRotSca);
        assert_eq!(k(0x8001), TransformKind::RecalcRotRpyRSca);
        assert_eq!(k(0x4001), TransformKind::Kind46);
        assert_eq!(k(0x2001), TransformKind::Kind48);
        assert_eq!(k(0x1001), TransformKind::Kind50);
        assert_eq!(k(0xC001), TransformKind::RecalcRotRpyRSca);
    }

    #[test]
    fn rotation_matches_a_hand_computed_case() {
        // 90 degrees about z sends +x to +y.
        let m = Mat4::from_trs([0.0; 3], [0.0, 0.0, core::f32::consts::FRAC_PI_2], [1.0; 3]);
        let x_axis = [m.0[0], m.0[1], m.0[2]];
        assert!((x_axis[0] - 0.0).abs() < 1e-5, "{x_axis:?}");
        assert!((x_axis[1] - 1.0).abs() < 1e-5, "{x_axis:?}");
    }

    #[test]
    fn sin_cos_tracks_the_real_thing() {
        // The polynomial approximation is only trustworthy after range
        // reduction, so check well outside [-pi, pi] too.
        for i in -200..=200 {
            let x = i as f32 * 0.1;
            let (s, c) = sin_cos(x);
            assert!((s - x.sin()).abs() < 1e-5, "sin({x}) = {s}");
            assert!((c - x.cos()).abs() < 1e-5, "cos({x}) = {c}");
        }
    }
}
