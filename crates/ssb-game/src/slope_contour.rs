//! `mpCommonUpdateFighterSlopeContour` (`mp/mpcommon.c:362`), the
//! fighter's `proc_slope`: a two-bone leg fit to the floor under each
//! flagged foot and the `FULL` body tilt (RE-510).
//!
//! Everything here is the original's `f32` arithmetic in its statement
//! order: the collision matrices (`Mtx44f`, row-vector convention), the
//! walk `gmCollisionGetFighterPartsWorldPosition` takes through them, the
//! inverse `gmCollisionSetInvertMatrix` builds and the leg solve
//! `func_ovl2_800EBD08`.

use crate::fighter::FIGHTER_JOINTS;
use ssb_engine::math::{lb_cos, lb_sin, sqrt, sy_arc_cos, sy_arc_sin, sy_arc_tan2, Vec3};

/// A collision `Mtx44f` without its constant last column: rows 0 to 2 are
/// the part's axes, row 3 its translation. A point is the row vector
/// `p * m` (`gmCollisionGetWorldPosition`).
pub type PartMtx = [[f32; 3]; 4];

/// `FTSLOPECONTOUR_FLAG_LFOOT`, `_RFOOT`, `_FULL` (`ftdef.h`).
pub const FLAG_LFOOT: u8 = 1 << 0;
pub const FLAG_RFOOT: u8 = 1 << 1;
pub const FLAG_FULL: u8 = 1 << 2;

/// `PI32` (`macros.h`): 3.14159265358979323846 rounded to f32.
const PI32: f32 = core::f32::consts::PI;

/// `F_CLC_DTOR32(deg)`.
pub fn dtor32(deg: f32) -> f32 {
    (deg * PI32) / 180.0
}

/// Every model part's collision local (`FTParts::unk_dobjtrans_0x10`) and
/// `DObj::translate.x`, in the original's units, as the runtime's skeleton
/// poses them: `gmCollisionTransformMatrixAll`, or
/// `gmCollisionSetMatrixNcs` in a motion with the animation locks, and
/// the frozen rest matrix of a locked part. TopN (joint 0) is built here
/// from the fighter ([`topn_local`]).
#[derive(Debug, Clone)]
pub struct PartLocals {
    pub locals: [PartMtx; FIGHTER_JOINTS],
    pub translate_x: [f32; FIGHTER_JOINTS],
    /// Bit `j`: joint `j` has a local.
    pub present: u64,
}

impl Default for PartLocals {
    fn default() -> Self {
        PartLocals {
            locals: [[[0.0; 3]; 4]; FIGHTER_JOINTS],
            translate_x: [0.0; FIGHTER_JOINTS],
            present: 0,
        }
    }
}

impl PartLocals {
    pub fn set(&mut self, joint: usize, local: PartMtx, translate_x: f32) {
        if joint < FIGHTER_JOINTS {
            self.locals[joint] = local;
            self.translate_x[joint] = translate_x;
            self.present |= 1 << joint;
        }
    }

    pub fn has(&self, joint: usize) -> bool {
        joint < FIGHTER_JOINTS && self.present >> joint & 1 != 0
    }
}

/// What `func_ovl2_800EBD08` wrote for one leg: the thigh's rotation and
/// the knee's `rotate.z`. The runtime puts them on the skeleton's pose,
/// where the next figatree tick overwrites the tracks it animates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LegFit {
    pub thigh: u8,
    pub knee: u8,
    pub thigh_rotate: [f32; 3],
    pub knee_rotate_z: f32,
}

/// `gmCollisionGetWorldPosition` (`gm/gmcollision.c:196`).
pub fn world_position(m: &PartMtx, v: Vec3) -> Vec3 {
    Vec3::new(
        ((m[0][0] * v.x) + (m[1][0] * v.y) + (m[2][0] * v.z)) + m[3][0],
        ((m[0][1] * v.x) + (m[1][1] * v.y) + (m[2][1] * v.z)) + m[3][1],
        ((m[0][2] * v.x) + (m[1][2] * v.y) + (m[2][2] * v.z)) + m[3][2],
    )
}

/// `func_ovl2_800ED490(dst, lhs, rhs)` (`gm/gmcollision.c:208`): a child's
/// world matrix from its parent's (`lhs`) and its local (`rhs`).
pub fn mul(lhs: &PartMtx, rhs: &PartMtx) -> PartMtx {
    let mut dst = [[0.0f32; 3]; 4];
    for i in 0..3 {
        for j in 0..3 {
            dst[i][j] = (lhs[0][j] * rhs[i][0]) + (lhs[1][j] * rhs[i][1]) + (lhs[2][j] * rhs[i][2]);
        }
    }
    for j in 0..3 {
        dst[3][j] = ((lhs[0][j] * rhs[3][0]) + (lhs[1][j] * rhs[3][1]) + (lhs[2][j] * rhs[3][2]))
            + lhs[3][j];
    }
    dst
}

/// `gmCollisionSetInvertMatrix` (`gm/gmcollision.c:228`): the adjugate,
/// its sign flips, then one reciprocal of the determinant. The original
/// spins forever on a zero determinant; this divides to infinity.
pub fn invert(src: &PartMtx) -> PartMtx {
    let s = src;
    let mut d = [[0.0f32; 3]; 4];
    d[0][0] = (s[1][1] * s[2][2]) - (s[1][2] * s[2][1]);
    d[1][0] = (s[1][0] * s[2][2]) - (s[1][2] * s[2][0]);
    d[2][0] = (s[1][0] * s[2][1]) - (s[1][1] * s[2][0]);
    d[3][0] = (s[3][0] * d[0][0]) - (s[3][1] * d[1][0]) + (s[3][2] * d[2][0]);

    d[0][1] = (s[0][1] * s[2][2]) - (s[0][2] * s[2][1]);
    d[1][1] = (s[0][0] * s[2][2]) - (s[0][2] * s[2][0]);
    d[2][1] = (s[0][0] * s[2][1]) - (s[0][1] * s[2][0]);
    d[3][1] = (s[3][0] * d[0][1]) - (s[3][1] * d[1][1]) + (s[3][2] * d[2][1]);

    d[0][2] = (s[0][1] * s[1][2]) - (s[0][2] * s[1][1]);
    d[1][2] = (s[0][0] * s[1][2]) - (s[0][2] * s[1][0]);
    d[2][2] = (s[0][0] * s[1][1]) - (s[0][1] * s[1][0]);
    d[3][2] = (s[3][0] * d[0][2]) - (s[3][1] * d[1][2]) + (s[3][2] * d[2][2]);

    let mut scale = (s[0][0] * d[0][0]) - (s[0][1] * d[1][0]) + (s[0][2] * d[2][0]);

    d[1][0] = -d[1][0];
    d[3][0] = -d[3][0];
    d[0][1] = -d[0][1];
    d[2][1] = -d[2][1];
    d[1][2] = -d[1][2];
    d[3][2] = -d[3][2];

    scale = 1.0 / scale;
    for row in d.iter_mut() {
        for e in row.iter_mut() {
            *e *= scale;
        }
    }
    d
}

/// `gmCollisionTransformMatrixAll` (`gm/gmcollision.c:29`) from a part's
/// translate, rotate and scale.
pub fn transform_matrix(t: Vec3, r: [f32; 3], s: [f32; 3]) -> PartMtx {
    let (sinx, cosx) = (lb_sin(r[0]), lb_cos(r[0]));
    let (siny, cosy) = (lb_sin(r[1]), lb_cos(r[1]));
    let (sinz, cosz) = (lb_sin(r[2]), lb_cos(r[2]));
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
        [t.x, t.y, t.z],
    ];
    for (row, &k) in m.iter_mut().zip(&s) {
        if k != 1.0 {
            for e in row.iter_mut() {
                *e *= k;
            }
        }
    }
    m
}

/// TopN's collision local: `translate` the fighter's position, `rotate`
/// `(rotate.x, lr * 90°, 0)` (the slope tilt, `ftMainSetStatus`'s yaw or
/// a turn's), `scale` `attr->size`. A held fighter's TopN turned to the
/// catcher's hand takes those axes.
pub fn topn_local(f: &crate::fighter::Fighter) -> PartMtx {
    let size = f.attributes.size;
    let held = crate::grab::rotates_topn(f.status.status)
        && f.grab.holder.and_then(|h| h.anchor_transform).is_some();
    if held {
        let a = crate::item_throw::collision_axes(f);
        let row = |v: Vec3| {
            if size != 1.0 {
                [v.x * size, v.y * size, v.z * size]
            } else {
                [v.x, v.y, v.z]
            }
        };
        return [row(a[0]), row(a[1]), row(a[2]), [f.pos.x, f.pos.y, f.pos.z]];
    }
    transform_matrix(f.pos, [f.topn_rotate_x, topn_yaw(f), 0.0], [size; 3])
}

/// TopN's `rotate.y`: `lr * 90°` from `ftMainSetStatus` (a status that
/// flips `lr` part-way, Turn's pivot or a roll, keeps it, RE-468), or the
/// yaw a status writes itself: Master Hand's `lr = 0`, an entry's, an item
/// throw's or turn's, a pipe's.
pub fn topn_yaw(f: &crate::fighter::Fighter) -> f32 {
    crate::boss::model_yaw(f)
        .or_else(|| crate::appear::model_yaw(f))
        .or_else(|| crate::item_throw::model_yaw(f))
        .or_else(|| crate::dokan::model_yaw(f))
        .unwrap_or(f.topn_lr * dtor32(90.0))
}

/// TopN's `rotate`, as its display (kind `0x4B`) builds it: a held
/// fighter's from its catcher's hand (`ftCommonCapturePulledRotateScale`),
/// otherwise `(rotate.x, yaw, 0)` with the slope tilt in `rotate.x`.
pub fn topn_rotate(f: &crate::fighter::Fighter) -> [f32; 3] {
    let hand = crate::grab::rotates_topn(f.status.status)
        .then(|| f.grab.holder.and_then(|h| h.anchor_transform))
        .flatten();
    match hand {
        Some(joint) => crate::grab::held_root_rotate(joint),
        None => [f.topn_rotate_x, topn_yaw(f), 0.0],
    }
}

/// `syVectorNorm3D`: a multiply by `1.0F / mag`, not a division.
pub(crate) fn norm3(v: &mut Vec3) -> f32 {
    let mag = sqrt((v.x * v.x) + (v.y * v.y) + (v.z * v.z));
    if mag == 0.0 {
        return 0.0;
    }
    v.x *= 1.0 / mag;
    v.y *= 1.0 / mag;
    v.z *= 1.0 / mag;
    mag
}

/// `syVectorNormCross3D` / `lbCommonCross3D`'s cross product.
pub(crate) fn cross(a: Vec3, b: Vec3) -> Vec3 {
    Vec3::new(
        (a.y * b.z) - (a.z * b.y),
        (a.z * b.x) - (a.x * b.z),
        (a.x * b.y) - (a.y * b.x),
    )
}

fn dot(a: Vec3, b: Vec3) -> f32 {
    (a.x * b.x) + (a.y * b.y) + (a.z * b.z)
}

fn mag3(v: Vec3) -> f32 {
    sqrt((v.x * v.x) + (v.y * v.y) + (v.z * v.z))
}

/// `func_ovl2_800EBA6C`: removes `b`'s direction from `a` in place and
/// returns what is left's squared length.
fn remove_component(a: &mut Vec3, b: Vec3) -> f32 {
    let mut n = b;
    norm3(&mut n);
    let scale = (a.x * n.x) + (n.y * a.y) + (n.z * a.z);
    n.x *= scale;
    n.y *= scale;
    n.z *= scale;
    a.x -= n.x;
    a.y -= n.y;
    a.z -= n.z;
    (a.x * a.x) + (a.y * a.y) + (a.z * a.z)
}

/// `syVectorAngleDiff3D` (`sys/vector.c:104`).
pub(crate) fn angle_diff(a: Vec3, b: Vec3) -> f32 {
    let mag = mag3(a) * mag3(b);
    if mag != 0.0 {
        let cos_diff = (dot(a, b) / mag).clamp(-1.0, 1.0);
        return sy_arc_cos(cos_diff);
    }
    0.0
}

/// `lbCommonSim3D`: the dot product over the sum of the lengths.
fn similarity(a: Vec3, b: Vec3) -> f32 {
    let ma = mag3(a);
    let mb = mag3(b);
    (a.x * b.x + a.y * b.y + a.z * b.z) / (mb + ma)
}

/// `func_ovl2_800EBB3C`: the signed angle about `axis` from `b` to `a`
/// once both lose their `axis` component (`800EBA6C` changes both).
fn twist_angle(a: &mut Vec3, b: &mut Vec3, axis: Vec3) -> f32 {
    if remove_component(a, axis) < 0.0004 {
        return 0.0;
    }
    if remove_component(b, axis) < 0.0004 {
        return 0.0;
    }
    let c = cross(*a, *b);
    if similarity(c, axis) < 0.0 {
        -angle_diff(*b, *a)
    } else {
        angle_diff(*b, *a)
    }
}

/// The tail of `func_ovl2_800EBC0C`: from the target in the leg root's
/// space and the thigh's collision local, the twist `800EBD08` turns the
/// thigh by.
pub fn leg_twist(root_local_target: Vec3, thigh_local: &PartMtx) -> f32 {
    let mut sp50 = root_local_target;
    let sp2c = Vec3::new(0.0, 1.0, 0.0);
    let mut sp44 = cross(sp50, sp2c);
    norm3(&mut sp44);
    let mut sp38 = Vec3::new(thigh_local[2][0], thigh_local[2][1], thigh_local[2][2]);
    norm3(&mut sp44);
    norm3(&mut sp38);
    norm3(&mut sp50);
    twist_angle(&mut sp44, &mut sp38, sp50)
}

/// `func_ovl2_800EBD08` (`ft/ftparam.c:2523`): the thigh's rotation and
/// the knee's bend that put the point `rotate` along the knee's x axis at
/// `vec` (leg-root space), the thigh turned by `angle`. `trax` is the
/// knee's `translate.x`.
pub fn leg_ik(vec: Vec3, rotate: f32, angle: f32, trax: f32) -> ([f32; 3], f32) {
    let arg1 = rotate;
    let mut square_xy = (vec.x * vec.x) + (vec.y * vec.y);
    let mut sqrtxyz = sqrt((vec.z * vec.z) + square_xy);
    square_xy = sqrt(square_xy);
    let sin_arg3 = lb_sin(angle);
    let cos_arg3 = lb_cos(angle);
    let square_trax = trax * trax;
    let square_arg1 = arg1 * arg1;
    let normal = ((((sqrtxyz * sqrtxyz) + square_trax) - square_arg1) / ((2.0 * sqrtxyz) * trax))
        .clamp(0.0, 1.0);
    let sp38 = -sqrt(1.0 - (normal * normal));
    if sqrtxyz == 0.0 {
        sqrtxyz = 0.00000001;
    }
    let inverse_xy_3 = normal;
    let inverse_xyz = 1.0 / sqrtxyz;
    if square_xy == 0.0 {
        square_xy = 0.00000001;
    }
    let inverse_xy = 1.0 / square_xy;

    let xmul = vec.x * inverse_xyz;
    let ymul = vec.y * inverse_xyz;
    let zmul = vec.z * inverse_xyz;

    let sp58 = ((((-xmul) * vec.z) * sin_arg3) * inverse_xy) - ((vec.y * cos_arg3) * inverse_xy);
    let zmulnorm = zmul * inverse_xy_3;
    let sp50 = ((((-ymul) * vec.z) * sin_arg3) * inverse_xy) + ((vec.x * cos_arg3) * inverse_xy);
    let imbadatmathhelp = (square_xy * sin_arg3) * inverse_xyz;
    let new_var = imbadatmathhelp * sp38;
    let inverse_xy_2 = zmulnorm + new_var;

    let mut thigh = [0.0f32; 3];
    if inverse_xy_2 == -1.0 || inverse_xy_2 == 1.0 {
        let tx = ((-sp38) * xmul) + (sp58 * normal);
        let ty = ((-sp38) * ymul) + (sp50 * normal);
        if inverse_xy_2 == -1.0 {
            thigh[1] = dtor32(90.0);
            thigh[0] = sy_arc_tan2(tx, ty);
        } else {
            thigh[1] = dtor32(-90.0);
            thigh[0] = sy_arc_tan2(-tx, ty);
        }
        thigh[2] = 0.0;
    } else {
        thigh[1] = sy_arc_sin(-inverse_xy_2);
        thigh[0] = sy_arc_tan2(
            ((-sp38) * zmul) + (imbadatmathhelp * normal),
            (square_xy * cos_arg3) * inverse_xyz,
        );
        thigh[2] = sy_arc_tan2(
            (ymul * normal) + (sp50 * sp38),
            (xmul * normal) + (sp58 * sp38),
        );
    }
    let knee_z =
        sy_arc_cos((((sqrtxyz * sqrtxyz) - square_trax) - square_arg1) / ((2.0 * trax) * arg1));
    (thigh, knee_z)
}

/// The parts' collision matrices as `proc_slope` reads them: the locals,
/// and the world matrices (`mtx_translate`, `unk_dobjtrans_0x5`) and
/// inverses (`unk_dobjtrans_0x9C`, `unk_dobjtrans_0x7`) it caches.
pub(crate) struct PartWalk<'a> {
    pub locals: &'a PartLocals,
    pub topn: PartMtx,
    pub parents: &'a [u8; FIGHTER_JOINTS],
    pub flagged: bool,
    world: [Option<PartMtx>; FIGHTER_JOINTS],
    inverse: [Option<PartMtx>; FIGHTER_JOINTS],
}

impl<'a> PartWalk<'a> {
    pub fn new(
        locals: &'a PartLocals,
        topn: PartMtx,
        parents: &'a [u8; FIGHTER_JOINTS],
        flagged: bool,
    ) -> Self {
        PartWalk {
            locals,
            topn,
            parents,
            flagged,
            world: [None; FIGHTER_JOINTS],
            inverse: [None; FIGHTER_JOINTS],
        }
    }

    fn parent(&self, j: usize) -> Option<usize> {
        let p = self.parents[j];
        (j != 0 && usize::from(p) < FIGHTER_JOINTS).then_some(usize::from(p))
    }

    fn local(&self, j: usize) -> PartMtx {
        if j == 0 {
            self.topn
        } else {
            self.locals.locals[j]
        }
    }

    /// `func_ovl2_800EDBA4`: the world matrices from the nearest cached
    /// ancestor (or TopN) down to `j`.
    pub fn world(&mut self, j: usize) -> PartMtx {
        let mut chain = [0usize; FIGHTER_JOINTS];
        let mut n = 0;
        let mut cur = j;
        let mut parent_world = loop {
            if let Some(w) = self.world[cur] {
                break w;
            }
            match self.parent(cur) {
                None => {
                    let w = self.local(cur);
                    self.world[cur] = Some(w);
                    break w;
                }
                Some(p) => {
                    chain[n] = cur;
                    n += 1;
                    cur = p;
                }
            }
        };
        for &c in chain[..n].iter().rev() {
            let w = mul(&parent_world, &self.local(c));
            self.world[c] = Some(w);
            parent_world = w;
        }
        parent_world
    }

    /// `gmCollisionGetFighterPartsWorldPosition`: outside a flagged motion
    /// each part's local in turn until a part with a world matrix.
    pub fn point(&mut self, j: usize, v: Vec3) -> Vec3 {
        if self.flagged {
            let w = self.world(j);
            return world_position(&w, v);
        }
        let mut v = v;
        let mut cur = Some(j);
        while let Some(c) = cur {
            if let Some(w) = self.world[c] {
                return world_position(&w, v);
            }
            v = world_position(&self.local(c), v);
            cur = self.parent(c);
        }
        v
    }

    /// `func_ovl2_800EE018`: a world point into `j`'s space.
    pub fn world_to_local(&mut self, j: usize, v: Vec3) -> Vec3 {
        let inv = match self.inverse[j] {
            Some(m) => m,
            None => {
                let w = self.world(j);
                let m = invert(&w);
                self.inverse[j] = Some(m);
                m
            }
        };
        world_position(&inv, v)
    }

    /// `ftParamsUpdateFighterPartsTransformAll(j)`: `j`'s subtree drops
    /// its cached world matrices and inverses.
    pub fn reset_subtree(&mut self, j: usize) {
        for k in 0..FIGHTER_JOINTS {
            let mut c = k;
            let mut inside = false;
            for _ in 0..FIGHTER_JOINTS {
                if c == j {
                    inside = true;
                    break;
                }
                match self.parent(c) {
                    Some(p) => c = p,
                    None => break,
                }
            }
            if inside {
                self.world[k] = None;
                self.inverse[k] = None;
            }
        }
    }
}

/// `gmCollisionGetFighterPartsWorldPosition`'s walk outside a flagged
/// motion, with no part's world matrix cached, stopped below TopN: `v`
/// through part `j`'s collision local and each ancestor's in turn, into
/// TopN's space.
pub fn point_below_topn(
    locals: &PartLocals,
    parents: &[u8; FIGHTER_JOINTS],
    j: usize,
    v: Vec3,
) -> Vec3 {
    let mut v = v;
    let mut cur = j;
    for _ in 0..FIGHTER_JOINTS {
        if cur == 0 || cur >= FIGHTER_JOINTS {
            break;
        }
        v = world_position(&locals.locals[cur], v);
        cur = usize::from(parents[cur]);
    }
    v
}

/// `1.0F / sqrtf(SQUARE(r[0]) + SQUARE(r[1]) + SQUARE(r[2]))`, or 0 for a
/// zero row, as `func_ovl0_800C9A38` and `func_ovl2_800EDA0C` take a
/// row's scale out.
pub fn inverse_row_scale(r: [f32; 3]) -> f32 {
    let scale = sqrt(((r[0] * r[0]) + (r[1] * r[1])) + (r[2] * r[2]));
    if scale != 0.0 {
        1.0 / scale
    } else {
        scale
    }
}

/// `func_ovl0_800C9A38` (`lb/lbcommon.c:1477`, the US build of
/// `ftCommonCapturePulledRotateScale`): part `j`'s world matrix with the
/// scale taken out of its axes. In a flagged motion, or for a part without
/// a parent, that is `func_ovl2_800EDBA4`'s `mtx_translate` with each axis
/// row normalised; otherwise the part's own collision local, its rows
/// normalised, its columns divided by the parent's axis lengths, times the
/// parent's `mtx_translate` (`func_ovl2_800ED490`). The translation row is
/// the part's world origin.
pub fn unscaled_world(
    locals: &PartLocals,
    topn: PartMtx,
    parents: &[u8; FIGHTER_JOINTS],
    flagged: bool,
    j: usize,
) -> PartMtx {
    let mut walk = PartWalk::new(locals, topn, parents, flagged);
    let parent = walk.parent(j);
    match parent {
        Some(p) if !flagged => {
            let mut f = walk.local(j);
            for row in f.iter_mut().take(3) {
                let s = inverse_row_scale(*row);
                for e in row.iter_mut() {
                    *e *= s;
                }
            }
            let pw = walk.world(p);
            for (k, prow) in pw.iter().take(3).enumerate() {
                let s = inverse_row_scale(*prow);
                for row in f.iter_mut().take(3) {
                    row[k] *= s;
                }
            }
            mul(&pw, &f)
        }
        _ => {
            let mut m = walk.world(j);
            for row in m.iter_mut().take(3) {
                let s = inverse_row_scale(*row);
                for e in row.iter_mut() {
                    *e *= s;
                }
            }
            m
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invert_undoes_a_transform() {
        let m = transform_matrix(
            Vec3::new(10.0, -4.0, 2.5),
            [0.3, -1.1, 0.7],
            [1.2, 1.2, 1.2],
        );
        let inv = invert(&m);
        let p = Vec3::new(3.0, 5.0, -7.0);
        let back = world_position(&inv, world_position(&m, p));
        assert!((back.x - p.x).abs() < 1e-4, "{back:?}");
        assert!((back.y - p.y).abs() < 1e-4, "{back:?}");
        assert!((back.z - p.z).abs() < 1e-4, "{back:?}");
    }

    #[test]
    fn leg_ik_reaches_the_target() {
        // Thigh 50 long (the knee's translate.x), shin 60 (`rotate`).
        let (trax, shin) = (50.0f32, 60.0f32);
        let target = Vec3::new(70.0, 20.0, 10.0);
        let (thigh, knee_z) = leg_ik(target, shin, 0.0, trax);
        let thigh_m = transform_matrix(Vec3::new(0.0, 0.0, 0.0), thigh, [1.0; 3]);
        let knee_m = transform_matrix(Vec3::new(trax, 0.0, 0.0), [0.0, 0.0, knee_z], [1.0; 3]);
        let foot = world_position(&mul(&thigh_m, &knee_m), Vec3::new(shin, 0.0, 0.0));
        assert!((foot.x - target.x).abs() < 0.5, "{foot:?}");
        assert!((foot.y - target.y).abs() < 0.5, "{foot:?}");
        assert!((foot.z - target.z).abs() < 0.5, "{foot:?}");
    }
}
