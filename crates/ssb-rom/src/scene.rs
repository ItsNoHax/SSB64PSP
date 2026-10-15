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
    /// `0x1000` — `nGCMatrixKind50`. Unused by shipped data (RE-063:
    /// 0/3117 nodes archive-wide); flagged `FLAG_BILLBOARD` like `Kind48`
    /// in `pack.rs` anyway, for fidelity with the decomp's case structure.
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
                if f != 0.0
                    && !(f.is_finite() && (MIN_COMPONENT..=MAX_COMPONENT).contains(&f.abs()))
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
    /// the same T*R*S here, which is their rest pose. Each local matrix is
    /// the display's fixed-point one ([`Mat4::from_trs_display`]), so the
    /// baked matrices equal what the runtime composes for an undriven node.
    pub fn world_transforms(&self) -> Vec<Mat4> {
        let mut out: Vec<Mat4> = Vec::with_capacity(self.nodes.len());
        for node in &self.nodes {
            let local =
                Mat4::from_trs_display(node.desc.translate, node.desc.rotate, node.desc.scale);
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
        Self::from_trs_with(t, r, s, sin_cos)
    }

    /// [`Mat4::from_trs`] with `gmCollisionTransformMatrixAll`'s trigonometry,
    /// `lbCommonSin`/`lbCommonCos`: the matrices the original's hit, hurt
    /// and attachment positions come from (RE-468).
    ///
    /// The products associate as the C writes them (`sinx * siny * cosz`
    /// is `(sinx * siny) * cosz`), so every `f32` is the original's.
    pub fn from_trs_collision(t: [f32; 3], r: [f32; 3], s: [f32; 3]) -> Mat4 {
        Self::from_trs_ncs(t, r, s, None).0
    }

    /// `gmCollisionSetMatrixNcs` (`gm/gmcollision.c:82`): the collision
    /// matrix of a fighter part while its motion uses the animation locks
    /// (`is_use_animlocks`, RE-509). `scale_mul` is the parent part's
    /// `vec_scale` (`None` for the root part); each row takes the
    /// accumulated scale `scale_mul * scale`, and each column is divided by
    /// the parent's, so a parent's scale moves its children without
    /// stretching them ("no child scale"). Returns the matrix and the part's
    /// new `vec_scale`.
    ///
    /// The original spins forever on a zero `scale_mul` component
    /// ("zero div x in gcSetMatrixNcs()"); this divides to infinity instead.
    pub fn from_trs_ncs(
        t: [f32; 3],
        r: [f32; 3],
        s: [f32; 3],
        scale_mul: Option<[f32; 3]>,
    ) -> (Mat4, [f32; 3]) {
        let (sinx, cosx) = ssb_engine::math::lb_sin_cos(r[0]);
        let (siny, cosy) = ssb_engine::math::lb_sin_cos(r[1]);
        let (sinz, cosz) = ssb_engine::math::lb_sin_cos(r[2]);
        let nscale = match scale_mul {
            Some(m) => [m[0] * s[0], m[1] * s[1], m[2] * s[2]],
            None => s,
        };
        // `mtx[row][col]`, row-major as the C indexes it.
        let mut m = [
            [cosy * cosz, cosy * sinz, -siny],
            [
                (sinx * siny * cosz) - (cosx * sinz),
                (sinx * siny * sinz) + (cosx * cosz),
                sinx * cosy,
            ],
            [
                (cosx * siny * cosz) + (sinx * sinz),
                (cosx * siny * sinz) - (sinx * cosz),
                cosx * cosy,
            ],
        ];
        for (row, &k) in m.iter_mut().zip(&nscale) {
            if k != 1.0 {
                for e in row.iter_mut() {
                    *e *= k;
                }
            }
        }
        if let Some(mul) = scale_mul {
            for (col, &k) in mul.iter().enumerate() {
                if k != 1.0 {
                    let inv = 1.0 / k;
                    for row in m.iter_mut() {
                        row[col] *= inv;
                    }
                }
            }
        }
        let mat = Mat4([
            m[0][0], m[0][1], m[0][2], 0.0, //
            m[1][0], m[1][1], m[1][2], 0.0, //
            m[2][0], m[2][1], m[2][2], 0.0, //
            t[0], t[1], t[2], 1.0,
        ]);
        (mat, nscale)
    }

    /// `syMatrixF2LFixedW` (`sys/matrix.c:44`): a float matrix as the
    /// display's fixed-point `Mtx`, each 3x3 entry `FTOFIX32`
    /// (`(s32)(x * 65536)`, truncated toward zero). The display draws a
    /// fighter part this way while its `transform_update_mode` is set: a
    /// part frozen by the animation locks shows its frozen
    /// `gmCollisionTransformMatrixAll` matrix (RE-509). The translation
    /// stays as given, as [`Mat4::from_trs_display`]'s does.
    pub fn f2l_fixed_w(&self) -> Mat4 {
        let q = |v: f32| (v * 65536.0) as i32 as f32 * (1.0 / 65536.0);
        let m = &self.0;
        Mat4([
            q(m[0]),
            q(m[1]),
            q(m[2]),
            0.0,
            q(m[4]),
            q(m[5]),
            q(m[6]),
            0.0,
            q(m[8]),
            q(m[9]),
            q(m[10]),
            0.0,
            m[12],
            m[13],
            m[14],
            1.0,
        ])
    }

    /// A fighter's TopN display matrix: its DObj is kind `0x4B`
    /// (`ftManagerMakeFighter`, `lbCommonInitDObj3Transforms`), drawn by
    /// `lbCommonFighterPartsFuncMatrix` (`lb/lbcommon.c:1369`).
    ///
    /// - Queried this frame (`transform_update_mode != 0`):
    ///   `syMatrixF2LFixedW` of the collision local `queried` (rows 0 to 2
    ///   the scaled axes, row 3 the translation).
    /// - Otherwise, outside a flagged motion: `syMatrixTraRotRpyRSca`, or
    ///   `syMatrixTraRotRpyR` when every scale is exactly 1.
    /// - Otherwise, in a flagged motion (`is_use_animlocks`):
    ///   `lbCommonMatrixTraRotScaInv` with `gLBCommonScale`, which
    ///   `ftDisplayMainProcDisplay` resets to 1 before each fighter.
    ///
    /// World units: the caller multiplies in the pack's model scale.
    pub fn fighter_topn_display(
        translate: [f32; 3],
        rotate: [f32; 3],
        scale: [f32; 3],
        flagged: bool,
        queried: Option<[[f32; 3]; 4]>,
    ) -> Mat4 {
        // `syMatrixTraRotRpyR(Sca)`, `lbCommonMatrixTraRotScaInv` and
        // `guMtxF2L` all store the translation as `(s32)(t * 65536.0F)`,
        // truncated toward zero; TopN's is in the original's units, so the
        // words are exact here (RE-511).
        let fixed = |t: f32| {
            let w = t * 65536.0;
            if w.abs() < 2_147_483_648.0 {
                w as i32 as f32 * (1.0 / 65536.0)
            } else {
                t
            }
        };
        if let Some(m) = queried {
            return Mat4([
                m[0][0],
                m[0][1],
                m[0][2],
                0.0,
                m[1][0],
                m[1][1],
                m[1][2],
                0.0,
                m[2][0],
                m[2][1],
                m[2][2],
                0.0,
                fixed(m[3][0]),
                fixed(m[3][1]),
                fixed(m[3][2]),
                1.0,
            ])
            .f2l_fixed_w();
        }
        let translate = translate.map(fixed);
        if flagged {
            Self::from_trs_scale_inv_display(translate, rotate, [1.0; 3], scale)
        } else if scale == [1.0; 3] {
            Self::from_tr_display(translate, rotate)
        } else {
            Self::from_trs_display(translate, rotate, scale)
        }
    }

    /// `lbCommonMatrixTraRotScaInv` (`lb/lbcommon.c:491`): the display
    /// matrix of a fighter part while its motion uses the animation locks
    /// (RE-509). The chain of [`Mat4::from_trs_display`] with each row
    /// times its accumulated `scale` word (`(s32)(scale * 256)`) and each
    /// column times the parent's inverse word
    /// (`(s32)((1 / scale_inv) * 256)`, `>> 8`): column 0 by x, 1 by y,
    /// 2 by z. With every scale 1 it is `syMatrixTraRotRpyR` exactly.
    pub fn from_trs_scale_inv_display(
        t: [f32; 3],
        r: [f32; 3],
        scale_inv: [f32; 3],
        scale: [f32; 3],
    ) -> Mat4 {
        use ssb_engine::math::sy_sin_cos_ushort;
        let (sinx, cosx) = sy_sin_cos_ushort(r[0]);
        let (siny, cosy) = sy_sin_cos_ushort(r[1]);
        let (sinz, cosz) = sy_sin_cos_ushort(r[2]);
        let m = |a: i32, b: i32| a.wrapping_mul(b);
        let word = |v: f32| (v * 256.0) as i32;
        let (kx, ky, kz) = (word(scale[0]), word(scale[1]), word(scale[2]));
        let inv = |v: f32| ((1.0 / v) * 256.0) as i32;
        let (ix, iy, iz) = (inv(scale_inv[0]), inv(scale_inv[1]), inv(scale_inv[2]));
        // `((e * scale_l) >> 8) * inv_l >> 8`.
        let sca = |e: i32, k: i32, i: i32| m(m(e, k) >> 8, i) >> 8;
        let srsp = m(sinx, siny) >> 15;
        let crsp = m(cosx, siny) >> 15;
        let e = [
            sca(m(cosy, cosz) >> 14, kx, ix),
            sca(m(cosy, sinz) >> 14, kx, iy),
            m(m(-siny, kx) >> 7, iz) >> 8,
            sca(
                (m(srsp, cosz) >> 14).wrapping_sub(m(cosx, sinz) >> 14),
                ky,
                ix,
            ),
            sca(
                (m(srsp, sinz) >> 14).wrapping_add(m(cosx, cosz) >> 14),
                ky,
                iy,
            ),
            sca(m(sinx, cosy) >> 14, ky, iz),
            sca(
                (m(crsp, cosz) >> 14).wrapping_add(m(sinx, sinz) >> 14),
                kz,
                ix,
            ),
            sca(
                (m(crsp, sinz) >> 14).wrapping_sub(m(sinx, cosz) >> 14),
                kz,
                iy,
            ),
            sca(m(cosx, cosy) >> 14, kz, iz),
        ];
        let f = |i: usize| e[i] as f32 * (1.0 / 65536.0);
        Mat4([
            f(0),
            f(1),
            f(2),
            0.0,
            f(3),
            f(4),
            f(5),
            0.0,
            f(6),
            f(7),
            f(8),
            0.0,
            t[0],
            t[1],
            t[2],
            1.0,
        ])
    }

    /// `syMatrixTraRotRpyRSca` (`sys/matrix.c:1086`): the fixed-point
    /// display matrix the original builds for a DObj from its translation,
    /// roll-pitch-yaw radians and scale (RE-506).
    ///
    /// Each angle becomes a `gSYSinTable` index truncated toward zero
    /// (`syGetSinCosUShort`, 4096 steps a turn); the 3x3 is an integer chain
    /// of those s1.15 words (`>> 14` to s15.16, three-term products `>> 15`
    /// then `>> 14`, every shift an arithmetic truncation), each row times
    /// its scale as `(s32)(scale * 256)` and `>> 8`. The translation stays
    /// as given (`FTOFIX32` keeps 1/65536 of a unit; callers pass it already
    /// divided by `MODEL_SCALE`).
    ///
    /// `lbCommonFighterPartsFuncMatrix` picks `syMatrixTraRotRpyR` for a
    /// part whose scale is exactly 1: that is this with a scale word of 256,
    /// which the chain reproduces exactly (`(e * 256) >> 8 == e`,
    /// `(-sinp * 256) >> 7 == -sinp * 2`).
    pub fn from_trs_display(t: [f32; 3], r: [f32; 3], s: [f32; 3]) -> Mat4 {
        // `scalex = sx * 256`: an `s32` conversion, truncated toward zero.
        let word = |v: f32| (v * 256.0) as i32;
        Self::rpy_display(t, r, [word(s[0]), word(s[1]), word(s[2])])
    }

    /// `syMatrixTraRotRpyR` (`sys/matrix.c:1023`): [`Mat4::from_trs_display`]
    /// for a DObj kind with no scale (`TraRotRpyR`): its scale is not read.
    pub fn from_tr_display(t: [f32; 3], r: [f32; 3]) -> Mat4 {
        Self::rpy_display(t, r, [256; 3])
    }

    /// The integer chain of `syMatrixTraRotRpyRSca`, `scale` as `s32`
    /// words with 8 fraction bits. Products wrap as the R4300's `mult`/
    /// `mflo` do.
    fn rpy_display(t: [f32; 3], r: [f32; 3], scale: [i32; 3]) -> Mat4 {
        use ssb_engine::math::sy_sin_cos_ushort;
        let (sinr, cosr) = sy_sin_cos_ushort(r[0]);
        let (sinp, cosp) = sy_sin_cos_ushort(r[1]);
        let (siny, cosy) = sy_sin_cos_ushort(r[2]);
        let m = |a: i32, b: i32| a.wrapping_mul(b);
        let sca = |e: i32, k: i32| m(e, k) >> 8;
        let (kx, ky, kz) = (scale[0], scale[1], scale[2]);
        let srsp = m(sinr, sinp) >> 15;
        let crsp = m(cosr, sinp) >> 15;
        let e = [
            // [0, 0] -> [0, 2]
            sca(m(cosp, cosy) >> 14, kx),
            sca(m(cosp, siny) >> 14, kx),
            m(-sinp, kx) >> 7,
            // [1, 0] -> [1, 2]
            sca((m(srsp, cosy) >> 14).wrapping_sub(m(cosr, siny) >> 14), ky),
            sca((m(srsp, siny) >> 14).wrapping_add(m(cosr, cosy) >> 14), ky),
            sca(m(sinr, cosp) >> 14, ky),
            // [2, 0] -> [2, 2]
            sca((m(crsp, cosy) >> 14).wrapping_add(m(sinr, siny) >> 14), kz),
            sca((m(crsp, siny) >> 14).wrapping_sub(m(sinr, cosy) >> 14), kz),
            sca(m(cosr, cosp) >> 14, kz),
        ];
        // s15.16 to float: exact (|e| < 2^24, a power-of-two scale).
        let f = |i: usize| e[i] as f32 * (1.0 / 65536.0);
        Mat4([
            f(0),
            f(1),
            f(2),
            0.0,
            f(3),
            f(4),
            f(5),
            0.0,
            f(6),
            f(7),
            f(8),
            0.0,
            t[0],
            t[1],
            t[2],
            1.0,
        ])
    }

    /// The display matrix's s15.16 words for the 3x3 (`[row][col]` as the
    /// original's `Mtx`), for tests against the ROM's own code.
    #[cfg(test)]
    pub(crate) fn display_words(&self) -> [i32; 9] {
        let w = |i: usize| (self.0[i] * 65536.0) as i32;
        [w(0), w(1), w(2), w(4), w(5), w(6), w(8), w(9), w(10)]
    }

    fn from_trs_with(
        t: [f32; 3],
        r: [f32; 3],
        s: [f32; 3],
        sin_cos: fn(f32) -> (f32, f32),
    ) -> Mat4 {
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

    /// Transforms a point (w = 1).
    pub fn transform_point(&self, p: [f32; 3]) -> [f32; 3] {
        let m = &self.0;
        [
            m[0] * p[0] + m[4] * p[1] + m[8] * p[2] + m[12],
            m[1] * p[0] + m[5] * p[1] + m[9] * p[2] + m[13],
            m[2] * p[0] + m[6] * p[1] + m[10] * p[2] + m[14],
        ]
    }

    /// Inverse of an affine matrix, or `None` when the linear part is singular.
    ///
    /// General 4x4 inversion is not needed: every matrix here is `T * R * S`,
    /// so the bottom row is `(0, 0, 0, 1)` and the inverse is
    /// `inv(linear)` with translation `-inv(linear) * t`. Scales are per-axis
    /// and occasionally non-uniform, so the linear part is inverted properly by
    /// cofactors rather than assumed orthonormal.
    pub fn inverse_affine(&self) -> Option<Mat4> {
        let m = &self.0;
        let a = [
            m[0], m[1], m[2], //
            m[4], m[5], m[6], //
            m[8], m[9], m[10],
        ];

        let c = [
            a[4] * a[8] - a[5] * a[7],
            a[5] * a[6] - a[3] * a[8],
            a[3] * a[7] - a[4] * a[6],
            a[2] * a[7] - a[1] * a[8],
            a[0] * a[8] - a[2] * a[6],
            a[1] * a[6] - a[0] * a[7],
            a[1] * a[5] - a[2] * a[4],
            a[2] * a[3] - a[0] * a[5],
            a[0] * a[4] - a[1] * a[3],
        ];

        let det = a[0] * c[0] + a[3] * c[3] + a[6] * c[6];
        if !det.is_finite() || det.abs() < 1e-20 {
            return None;
        }
        // Out of line: LLVM may divide before the early return (RE-469).
        let inv_det = ssb_engine::math::div_nonzero(1.0, det);

        // `c` is the adjugate in *row*-major order, `out` is column-major, so
        // the copy transposes.
        let mut out = [0f32; 16];
        for row in 0..3 {
            for col in 0..3 {
                out[col * 4 + row] = c[row * 3 + col] * inv_det;
            }
        }

        let t = [m[12], m[13], m[14]];
        for row in 0..3 {
            out[12 + row] = -(out[row] * t[0] + out[4 + row] * t[1] + out[8 + row] * t[2]);
        }
        out[15] = 1.0;
        Some(Mat4(out))
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
pub(crate) fn sin_cos(x: f32) -> (f32, f32) {
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

/// Size of one `DObjDLLink`: `s32 list_id`, `Gfx *dl`.
pub const DL_LINK_SIZE: usize = 8;

/// `ARRAY_COUNT(gSYTaskmanDLHeads)`. Doubles as the `DObjDLLink` terminator,
/// exactly as [`DOBJ_ARRAY_MAX`] does for `DObjDesc`:
///
/// ```c
/// while ((++dl_link)->list_id != ARRAY_COUNT(gSYTaskmanDLHeads))
/// ```
pub const DL_HEAD_COUNT: u32 = 4;

/// One entry of a `DObjDLLink` array: which task display-list head to append
/// to, and what to append.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DlLink {
    pub list_id: u32,
    /// File-relative offset of the display list, or `None` to skip.
    pub dl: Option<u32>,
}

/// Parses a `DObjDLLink` array, if one starts at `at`.
///
/// Returns `None` when the bytes are not a link array — in particular when they
/// are a display list, which is the other thing a node's `dl` field can be.
pub fn parse_dl_links(data: &[u8], intern_slots: &BTreeSet<u32>, at: u32) -> Option<Vec<DlLink>> {
    let mut out = Vec::new();
    let mut off = at as usize;

    loop {
        let raw = data.get(off..off + DL_LINK_SIZE)?;
        let list_id = u32::from_be_bytes([raw[0], raw[1], raw[2], raw[3]]);
        if list_id == DL_HEAD_COUNT {
            // Terminator. A zero-entry array is not a link array, just two
            // words that happen to read as one.
            return (!out.is_empty()).then_some(out);
        }
        if list_id >= DL_HEAD_COUNT {
            return None;
        }

        let dl_raw = u32::from_be_bytes([raw[4], raw[5], raw[6], raw[7]]);
        // Same discriminator as `DObjDesc::dl`, and it is what keeps a real
        // display list from being misread as a link array: a `Gfx` command word
        // small enough to pass as a `list_id` would have to be followed by a
        // word the archive relocated, and `G_VTX`'s own high byte is 0x01 with
        // 24 bits of payload below it, far above 4.
        let dl = match dl_raw {
            0 => None,
            _ if intern_slots.contains(&(off as u32 + 4)) => Some(dl_raw),
            _ => return None,
        };
        out.push(DlLink { list_id, dl });

        off += DL_LINK_SIZE;
        // Real arrays are tiny: 564 of the 589 in the decomp hold a single
        // entry plus the terminator, and the largest holds 64.
        if out.len() > 128 {
            return None;
        }
    }
}

/// Size of a `Gfx *dls[2]` pre/post-matrix pair.
pub const DL_PAIR_SIZE: usize = 8;

/// Parses a `Gfx *dls[2]` pre/post-matrix pair, if one starts at `at`.
///
/// Fighter models use this member. From `ftDisplayMainDrawDefault` case 1
/// (`src/ft/ftdisplaymain.c`):
///
/// ```c
/// dls = dobj->dls;
/// if (dls != NULL && dls[0] != NULL) gSPDisplayList(..., dls[0]);
/// sp58 = gcPrepDObjMatrix(gSYTaskmanDLHeads, dobj);
/// if (dls != NULL && dls[1] != NULL) gSPDisplayList(..., dls[1]);
/// ```
///
/// So `dls[0]` draws in the **parent's** space and `dls[1]` in the node's own —
/// which makes `dls[1]` the one a node places, and `dls[0]` geometry that
/// belongs one level up. Yoshi's array is 19 such pairs back to back, and the
/// decomp labels it exactly that way (`338_YoshiModel.c`: "DObj.dls pre/post-
/// matrix DL pairs @ 0x3308 (19 pairs, 152 bytes)").
///
/// The shape is only two words, so the relocation test carries all the weight:
/// `dls[1]` must be a relocated pointer and `dls[0]` must be NULL or one too.
/// That is what separates a pair from a display list starting with `G_VTX`,
/// whose first word is `0x01xxxxxx` — non-zero and never a relocation target.
/// A `{ NULL, NULL }` pair — a joint that draws nothing — is two zero words and
/// carries no evidence of its own. `BossModel` ships two of them. They are only
/// accepted when a neighbouring pair vouches for them, which is sound because
/// pairs only ever occur as elements of an array.
pub fn parse_dl_pair(
    data: &[u8],
    intern_slots: &BTreeSet<u32>,
    at: u32,
) -> Option<[Option<u32>; 2]> {
    let [pre, post] = read_pair(data, intern_slots, at)?;
    if pre.is_some() || post.is_some() {
        return Some([pre, post]);
    }

    let neighbour = |off: u32| {
        read_pair(data, intern_slots, off).is_some_and(|[a, b]| a.is_some() || b.is_some())
    };
    let vouched = at.checked_sub(DL_PAIR_SIZE as u32).is_some_and(neighbour)
        || neighbour(at + DL_PAIR_SIZE as u32);
    vouched.then_some([pre, post])
}

fn read_pair(data: &[u8], intern_slots: &BTreeSet<u32>, at: u32) -> Option<[Option<u32>; 2]> {
    let raw = data.get(at as usize..at as usize + DL_PAIR_SIZE)?;
    let word = |i: usize| u32::from_be_bytes([raw[i], raw[i + 1], raw[i + 2], raw[i + 3]]);

    let slot = |off: u32, value: u32| -> Option<Option<u32>> {
        match value {
            0 => Some(None),
            _ if intern_slots.contains(&off) => Some(Some(value)),
            _ => None,
        }
    };
    Some([slot(at, word(0))?, slot(at + 4, word(4))?])
}

/// What a node's `dl` field turned out to be. `DObj`'s display-list field is a
/// union — `Gfx*`, `Gfx**`, `DObjDLLink*`, `DObjDistDL*` and more — and
/// **nothing in the data says which**. The discriminator is the `proc_display`
/// callback the GObj was registered with, which lives in game code, not in the
/// archive, so the member has to be recovered structurally.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeDl {
    /// A `DObjDLLink[]` selecting which task display-list head to append to.
    Links(Vec<DlLink>),
    /// A `Gfx *dls[2]` pre/post-matrix pair; see [`parse_dl_pair`].
    Pair { pre: Option<u32>, post: Option<u32> },
    /// A plain `Gfx*` — the field is the display list.
    Direct(u32),
}

impl NodeDl {
    /// The display lists this node draws, **most node-local first**.
    ///
    /// Callers that can only place one mesh per node should take the first:
    /// for a pair that is `dls[1]`, the list drawn under the node's own matrix.
    pub fn lists(&self) -> Vec<u32> {
        match self {
            NodeDl::Links(links) => links.iter().filter_map(|l| l.dl).collect(),
            NodeDl::Pair { pre, post } => post.iter().chain(pre.iter()).copied().collect(),
            NodeDl::Direct(dl) => alloc::vec![*dl],
        }
    }
}

/// Resolves node `dl` fields for one file.
///
/// Holds the file's intern-relocation slots, which every discriminator below
/// consults. Building that set is the expensive part, and a file has hundreds
/// of nodes, so it is built once here rather than per node.
pub struct DlResolver<'a> {
    data: &'a [u8],
    intern_slots: BTreeSet<u32>,
}

impl<'a> DlResolver<'a> {
    pub fn new(file: &'a File) -> Self {
        Self {
            data: &file.data,
            intern_slots: file.intern_relocs.iter().map(|r| r.at).collect(),
        }
    }

    /// Decides which union member `at` points at.
    ///
    /// Order matters: the members are tried most-constrained first, so a shape
    /// that could read as either is claimed by the one with more evidence
    /// behind it. `DObjDLLink` needs a terminator and small `list_id`s; a pair
    /// needs two relocation-backed words; a `Gfx*` needs nothing, so it is the
    /// fallback. Where links and pairs overlap — `{ 0, dl }, { 4, NULL }` reads
    /// as both — they name the same display list anyway.
    pub fn resolve(&self, at: u32) -> NodeDl {
        if let Some(links) = parse_dl_links(self.data, &self.intern_slots, at) {
            return NodeDl::Links(links);
        }
        if let Some([pre, post]) = parse_dl_pair(self.data, &self.intern_slots, at) {
            return NodeDl::Pair { pre, post };
        }
        NodeDl::Direct(at)
    }

    /// Shorthand for `self.resolve(at).lists()`.
    pub fn lists(&self, at: u32) -> Vec<u32> {
        self.resolve(at).lists()
    }
}

/// Resolves a single node's `dl` field. Convenience over [`DlResolver`]; use
/// the resolver directly when walking a whole file.
pub fn resolve_node_lists(file: &File, node_dl: u32) -> Vec<u32> {
    DlResolver::new(file).lists(node_dl)
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
        nodes.push(DObjNode {
            desc: *desc,
            parent,
        });
    }

    Some(SceneGraph { offset, nodes })
}

#[cfg(test)]
#[allow(clippy::approx_constant)] // the oracle inputs are exact f32 bit patterns
mod tests {
    use super::*;
    use crate::archive::InternReloc;
    use alloc::vec;

    /// `tools/r4300/matrix_hashes.py`'s input set: `(r, p, y, sx, sy, sz)`.
    fn display_inputs() -> Vec<[f32; 6]> {
        let angles: [f32; 18] = [
            0.0,
            1.570_796_4,
            -1.570_796_4,
            3.141_592_7,
            -3.141_592_7,
            0.1,
            -0.1,
            0.001,
            -0.001,
            0.785_398,
            2.5,
            -2.5,
            6.283_185_5,
            -6.283_185_5,
            12.0,
            -12.0,
            0.001_533_980_8,
            -0.001_533_980_8,
        ];
        let scales: [f32; 9] = [1.0, 0.5, 1.0537, 0.997, 2.0, 1.2, 0.8, 1.5, 0.003];
        let mut out = Vec::new();
        for (i, &r) in angles.iter().enumerate() {
            for (j, &p) in angles.iter().enumerate() {
                let y = angles[(i * 7 + j * 3) % angles.len()];
                let s = scales[(i + j) % scales.len()];
                let t = scales[(i * 5 + j) % scales.len()];
                out.push([r, p, y, s, t, 1.0]);
            }
        }
        for k in (-8192i32..8192).step_by(7) {
            let a = (f64::from(k) * 0.001_533_980_8 + 0.0003) as f32;
            out.push([a, -a * 0.5, a * 2.0, 1.0, 1.0, 1.0]);
        }
        out
    }

    fn display_hash(f: impl Fn(&[f32; 6]) -> Mat4) -> u64 {
        display_inputs()
            .iter()
            .fold(0xcbf2_9ce4_8422_2325, |h, input| {
                f(input).display_words().iter().fold(h, |h, w| {
                    w.to_le_bytes().iter().fold(h, |h, &b| {
                        (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3)
                    })
                })
            })
    }

    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn display_matrix_matches_the_rom() {
        // Hashes of the ROM's own `syMatrixTraRotRpyRSca` (0x8001C588) and
        // `syMatrixTraRotRpyR` (0x8001C21C) over 2665 inputs, run by
        // `tools/r4300/matrix_hashes.py`: every s15.16 word of every 3x3.
        assert_eq!(display_inputs().len(), 2665);
        let sca = display_hash(|i| {
            Mat4::from_trs_display([1.0, 2.0, 3.0], [i[0], i[1], i[2]], [i[3], i[4], i[5]])
        });
        assert_eq!(sca, 0x7aa6_1fcf_d076_712c);
        let rpy = display_hash(|i| Mat4::from_tr_display([1.0, 2.0, 3.0], [i[0], i[1], i[2]]));
        assert_eq!(rpy, 0x9e2b_b5a5_8a59_aa47);
        // The rotation-only builders hash the same and write no
        // translation (the script checks row 3 is (0, 0, 0, 1)):
        // `lbCommonMatrixRotSca` (0x800C82AC, matrix kind 0x45) and
        // `syMatrixRotRpyR` (0x8001BF20). Here they are the same chains
        // with a zero translation.
        let rot_sca = display_hash(|i| {
            Mat4::from_trs_display([0.0; 3], [i[0], i[1], i[2]], [i[3], i[4], i[5]])
        });
        assert_eq!(rot_sca, 0x7aa6_1fcf_d076_712c);
        let rot = display_hash(|i| Mat4::from_tr_display([0.0; 3], [i[0], i[1], i[2]]));
        assert_eq!(rot, 0x9e2b_b5a5_8a59_aa47);
        // A sample the script prints: scale 0.8 truncates to 204/256.
        let m = Mat4::from_trs_display(
            [0.0; 3],
            [-1.570_796_4, -3.141_592_7, -0.001],
            [0.8, 1.2, 1.0],
        );
        assert_eq!(m.display_words(), [-52224, 0, 0, 0, 0, 78592, 0, 65536, 0]);
    }

    fn fnv_words(h: u64, words: impl IntoIterator<Item = i32>) -> u64 {
        words.into_iter().fold(h, |h, w| {
            w.to_le_bytes().iter().fold(h, |h, &b| {
                (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3)
            })
        })
    }

    /// `INVS` of `tools/r4300/matrix_hashes.py`, three in a row from `n`.
    fn animlock_inv(n: usize) -> [f32; 3] {
        const INVS: [f32; 9] = [1.0, 1.1, 0.5, 1.24, 2.0, 0.91, 1.05, 5.0, 0.039_794_922];
        [INVS[n % 9], INVS[(n + 1) % 9], INVS[(n + 2) % 9]]
    }

    fn float_words(m: &Mat4) -> [i32; 9] {
        let b = |i: usize| m.0[i].to_bits() as i32;
        [b(0), b(1), b(2), b(4), b(5), b(6), b(8), b(9), b(10)]
    }

    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn animlock_matrices_match_the_rom() {
        // RE-509: hashes `tools/r4300/matrix_hashes.py` prints for the ROM's
        // own `lbCommonMatrixTraRotScaInv` (0x800C7DB4),
        // `gmCollisionTransformMatrixAll` (0x800ECDE4, every f32 of the
        // 3x3), `syMatrixF2LFixedW` (0x80019EA0) of those, and
        // `gmCollisionSetMatrixNcs` (0x800ECFF4, the 3x3 and `vec_scale`;
        // no parent every fifth input) over the display inputs.
        let inputs = display_inputs();
        let (mut inv_h, mut all_h, mut f2l_h, mut ncs_h) = (FNV, FNV, FNV, FNV);
        for (n, i) in inputs.iter().enumerate() {
            let (r, s) = ([i[0], i[1], i[2]], [i[3], i[4], i[5]]);
            let inv = animlock_inv(n);
            let m = Mat4::from_trs_scale_inv_display([1.0, 2.0, 3.0], r, inv, s);
            inv_h = fnv_words(inv_h, m.display_words());
            let all = Mat4::from_trs_collision([1.0, 2.0, 3.0], r, s);
            all_h = fnv_words(all_h, float_words(&all));
            f2l_h = fnv_words(f2l_h, all.f2l_fixed_w().display_words());
            let (ncs, scale) =
                Mat4::from_trs_ncs([1.0, 2.0, 3.0], r, s, (n % 5 != 0).then_some(inv));
            ncs_h = fnv_words(ncs_h, float_words(&ncs));
            ncs_h = fnv_words(ncs_h, scale.map(|v| v.to_bits() as i32));
        }
        assert_eq!(inv_h, 0x84b0_dda3_fcfe_e912);
        assert_eq!(all_h, 0x3982_7526_1df6_fe50);
        assert_eq!(f2l_h, 0x48d5_b00e_44b4_0a2d);
        assert_eq!(ncs_h, 0xf758_2515_856b_a73e);
        // A sample the script prints: input 40, scale_inv (2, 0.91, 1.05).
        let i = inputs[40];
        let m = Mat4::from_trs_scale_inv_display(
            [0.0; 3],
            [i[0], i[1], i[2]],
            animlock_inv(40),
            [i[3], i[4], i[5]],
        );
        assert_eq!(m.display_words(), [-26112, 0, 0, 0, 0, 74601, 0, 71936, 0]);
    }

    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn scale_inv_with_unit_scales_is_rpy() {
        for input in display_inputs() {
            let r = [input[0], input[1], input[2]];
            assert_eq!(
                Mat4::from_trs_scale_inv_display([0.0; 3], r, [1.0; 3], [1.0; 3]),
                Mat4::from_tr_display([0.0; 3], r)
            );
        }
    }

    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn fighter_topn_display_kinds() {
        let (t, r) = (
            [-210.0, 1399.25, 0.0],
            [0.0, core::f32::consts::FRAC_PI_2, 0.0],
        );
        // Unflagged: `TraRotRpyRSca` with Mario's 1.12 as the word 286, and
        // `TraRotRpyR` at scale 1.
        let m = Mat4::fighter_topn_display(t, r, [1.12; 3], false, None);
        assert_eq!(m, Mat4::from_trs_display(t, r, [1.12; 3]));
        assert_eq!(m.display_words()[2], -286 * 256);
        assert_eq!(
            Mat4::fighter_topn_display(t, r, [1.0; 3], false, None),
            Mat4::from_tr_display(t, r)
        );
        // Flagged: `TraRotScaInv` against `gLBCommonScale` = 1.
        assert_eq!(
            Mat4::fighter_topn_display(t, r, [1.12; 3], true, None),
            Mat4::from_trs_scale_inv_display(t, r, [1.0; 3], [1.12; 3])
        );
        // Queried: the collision local through `syMatrixF2LFixedW`, flagged
        // or not.
        let local = [[0.1, 0.2, -1.12], [0.0, 1.12, 0.0], [1.12, 0.0, 0.3], t];
        for flagged in [false, true] {
            let q = Mat4::fighter_topn_display([0.0; 3], [0.0; 3], [1.0; 3], flagged, Some(local));
            assert_eq!(
                q.display_words(),
                [6553, 13107, -73400, 0, 73400, 0, 73400, 0, 19660]
            );
            assert_eq!(q.translation(), t);
        }
    }

    const FNV: u64 = 0xcbf2_9ce4_8422_2325;

    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn display_matrix_identities() {
        // No rotation, scale 1: the identity, exactly.
        let m = Mat4::from_trs_display([0.5, -2.0, 3.0], [0.0; 3], [1.0; 3]);
        assert_eq!(m, Mat4::from_trs([0.5, -2.0, 3.0], [0.0; 3], [1.0; 3]));
        // Scale 1 is `syMatrixTraRotRpyR` bit for bit.
        for input in display_inputs() {
            let r = [input[0], input[1], input[2]];
            assert_eq!(
                Mat4::from_trs_display([0.0; 3], r, [1.0; 3]),
                Mat4::from_tr_display([0.0; 3], r)
            );
        }
        // An angle truncates toward zero to a 4096th of a turn: just short
        // of one step is no rotation, either sign.
        let step = core::f32::consts::TAU / 4096.0;
        for a in [step * 0.99, -step * 0.99] {
            let m = Mat4::from_tr_display([0.0; 3], [0.0, 0.0, a]);
            assert_eq!(m.display_words(), [65536, 0, 0, 0, 65536, 0, 0, 0, 65536]);
        }
        // Scale 1.0537 is 269/256.
        let m = Mat4::from_trs_display([0.0; 3], [0.0; 3], [1.0537; 3]);
        assert_eq!(m.0[0], 269.0 / 256.0);
        // Close to the exact matrix everywhere.
        for input in display_inputs() {
            let (r, s) = (
                [input[0], input[1], input[2]],
                [input[3], input[4], input[5]],
            );
            let a = Mat4::from_trs_display([0.0; 3], r, s);
            let b = Mat4::from_trs([0.0; 3], r, s);
            for k in 0..12 {
                assert!(
                    (a.0[k] - b.0[k]).abs() < 8e-3,
                    "{input:?} {k}: {} {}",
                    a.0[k],
                    b.0[k]
                );
            }
        }
    }

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
        let slots = vec![
            64 + DOBJ_DESC_SIZE as u32 + 4,
            64 + 2 * DOBJ_DESC_SIZE as u32 + 4,
        ];
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
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
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

    /// Serialises a `DObjDLLink` entry.
    fn link(list_id: u32, dl: u32) -> Vec<u8> {
        let mut out = list_id.to_be_bytes().to_vec();
        out.extend_from_slice(&dl.to_be_bytes());
        out
    }

    #[test]
    fn resolves_a_dobjdllink_array_to_its_display_lists() {
        // The exact shape of dStageSectorFile2_gap_0x3EE0_sub_0x558:
        //   { 0, DL_0x3EE0 }, { 4, NULL }
        let mut data = alloc::vec![0u8; 32];
        data.extend(link(0, 0x3EE0));
        data.extend(link(DL_HEAD_COUNT, 0));
        let file = file_with(data, &[36]); // the `dl` word at 32 + 4

        assert_eq!(resolve_node_lists(&file, 32), [0x3EE0]);
    }

    #[test]
    fn a_link_entry_with_a_null_list_is_skipped_not_terminating() {
        // `if (dl_link->dl != NULL)` skips the entry; only list_id == 4 ends
        // the walk. Reading NULL as a terminator would silently drop the
        // entries after it.
        let mut data = alloc::vec![0u8; 16];
        data.extend(link(0, 0));
        data.extend(link(1, 0xAA0));
        data.extend(link(DL_HEAD_COUNT, 0));
        let file = file_with(data, &[16 + 8 + 4]);

        assert_eq!(resolve_node_lists(&file, 16), [0xAA0]);
    }

    #[test]
    fn a_display_list_is_not_mistaken_for_a_link_array() {
        // A node's `dl` may point straight at a Gfx list, and that must fall
        // through to "use this offset". G_VTX's command word is 0x01xxxxxx,
        // far above a valid list_id, so it cannot pass as one.
        let mut data = alloc::vec![0u8; 16];
        data.extend_from_slice(&0x0100_1010u32.to_be_bytes());
        data.extend_from_slice(&0x0000_0040u32.to_be_bytes());
        data.extend_from_slice(&0xDF00_0000u32.to_be_bytes()); // G_ENDDL
        data.extend_from_slice(&0u32.to_be_bytes());
        let file = file_with(data, &[20]);

        assert_eq!(resolve_node_lists(&file, 16), [16]);
    }

    #[test]
    fn an_unrelocated_link_pointer_is_rejected() {
        // Same bytes, no intern reloc backing the `dl` word: this is what stops
        // arbitrary `{small int, plausible offset}` pairs reading as links.
        let mut data = alloc::vec![0u8; 16];
        data.extend(link(0, 0x3EE0));
        data.extend(link(DL_HEAD_COUNT, 0));

        assert_eq!(
            resolve_node_lists(&file_with(data.clone(), &[20]), 16),
            [0x3EE0]
        );
        assert_eq!(resolve_node_lists(&file_with(data, &[]), 16), [16]);
    }

    #[test]
    fn resolves_a_pre_post_matrix_pair() {
        // The exact shape of dYoshiModel_Joint_0x3148_post_post_post[0..2]:
        // { NULL, DL } then { pre_DL, DL }. Neither reads as a link array --
        // the second pair's first word is a pointer, far above list_id 4 --
        // and the post-matrix list must come first, since that is the one the
        // node itself places.
        let mut data = alloc::vec![0u8; 16];
        data.extend_from_slice(&0u32.to_be_bytes());
        data.extend_from_slice(&0x2050u32.to_be_bytes());
        data.extend_from_slice(&0x31C0u32.to_be_bytes());
        data.extend_from_slice(&0x2248u32.to_be_bytes());
        let file = file_with(data, &[20, 24, 28]);

        let r = DlResolver::new(&file);
        assert_eq!(
            r.resolve(16),
            NodeDl::Pair {
                pre: None,
                post: Some(0x2050)
            }
        );
        assert_eq!(
            r.resolve(24),
            NodeDl::Pair {
                pre: Some(0x31C0),
                post: Some(0x2248)
            }
        );
        assert_eq!(r.lists(24), [0x2248, 0x31C0]);
    }

    #[test]
    fn an_empty_pair_needs_a_neighbour_to_vouch_for_it() {
        // BossModel's array holds a { NULL, NULL } joint. Read as a `Gfx*` the
        // two zero words decode as G_SPNOOP and the walk runs on into whatever
        // follows -- which is how that node ended up reporting
        // VertexDataOutOfBounds against an unrelocated extern pointer.
        let mut data = alloc::vec![0u8; 16];
        data.extend_from_slice(&[0u8; 8]); // { NULL, NULL } at 16
        data.extend_from_slice(&0u32.to_be_bytes());
        data.extend_from_slice(&0x1478u32.to_be_bytes()); // a real pair at 24

        assert!(matches!(
            DlResolver::new(&file_with(data.clone(), &[28])).resolve(16),
            NodeDl::Pair {
                pre: None,
                post: None
            }
        ));

        // Alone, the same two zero words are just two zero words.
        assert_eq!(
            DlResolver::new(&file_with(alloc::vec![0u8; 24], &[])).resolve(16),
            NodeDl::Direct(16)
        );
    }

    #[test]
    fn an_unrelocated_pair_is_rejected() {
        // Constraint 5 again, and it is the whole discriminator here: a pair is
        // only two words, so without the relocation evidence any display list
        // beginning with a zero word would read as one.
        let mut data = alloc::vec![0u8; 16];
        data.extend_from_slice(&0u32.to_be_bytes());
        data.extend_from_slice(&0x2050u32.to_be_bytes());

        assert!(matches!(
            DlResolver::new(&file_with(data.clone(), &[20])).resolve(16),
            NodeDl::Pair { .. }
        ));
        assert_eq!(
            DlResolver::new(&file_with(data, &[])).resolve(16),
            NodeDl::Direct(16)
        );
    }

    #[test]
    fn affine_inverse_undoes_a_scaled_rotated_translation() {
        // Non-uniform scale on purpose: an orthonormal-transpose shortcut would
        // pass a rotation-only test and be wrong here.
        let m = Mat4::from_trs([12.0, -3.0, 40.0], [0.4, -1.1, 2.0], [2.0, 0.5, 3.0]);
        let inv = m.inverse_affine().expect("non-singular");

        for p in [[0.0; 3], [1.0, 2.0, 3.0], [-100.0, 40.0, 7.5]] {
            let round = inv.transform_point(m.transform_point(p));
            for k in 0..3 {
                assert!((round[k] - p[k]).abs() < 1e-3, "{round:?} vs {p:?}");
            }
        }
    }

    #[test]
    fn a_singular_matrix_has_no_inverse() {
        // A zero scale on one axis: the game ships these to hide a joint, and
        // inverting one anyway would produce infinities in every vertex that
        // borrowed from it.
        let m = Mat4::from_trs([0.0; 3], [0.0; 3], [1.0, 0.0, 1.0]);
        assert!(m.inverse_affine().is_none());
    }

    #[test]
    fn transform_kind_follows_the_runtime_priority_order() {
        // `gcSetupCommonDObjs` tests 0x8000 before 0x4000 before 0x2000, so a
        // hypothetical combination resolves to the highest bit.
        let k = |id| {
            DObjDesc {
                id,
                dl: None,
                translate: [0.0; 3],
                rotate: [0.0; 3],
                scale: [1.0; 3],
            }
            .transform_kind()
        };
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
