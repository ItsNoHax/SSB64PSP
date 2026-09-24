//! Authored-UV coverage for the build-time filter optimizer. All coordinates
//! remain in the vertex stream's integer S10.5 domain.

#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub(super) struct Coverage {
    pub train: Vec<[i32; 2]>,
    pub validation: Vec<[i32; 2]>,
    /// Present only for a texgen primitive whose generated S10.5 footprint
    /// is bounded (RE-311). `None` for authored UVs and for the full-tile
    /// fallback.
    pub texgen: Option<TexgenMeta>,
}

/// The inclusive S10.5 box every texgen pose can generate for a primitive.
///
/// The RSP normalises `M^T * lookat`, so each basis row is a unit vector
/// whatever the node transform. Cauchy-Schwarz then bounds every per-axis
/// `texgen_dot` by `|n| / 127` for the primitive's longest real normal.
/// Orthogonality of the two rows is *not* assumed: a non-uniform node
/// scale makes them non-orthogonal after normalisation, which can reach the
/// box corners that an orthonormal basis (a disk) never does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct TexgenBound {
    pub min: [i32; 2],
    pub max: [i32; 2],
}

impl TexgenBound {
    pub fn contains(&self, p: [i32; 2]) -> bool {
        (self.min[0]..=self.max[0]).contains(&p[0]) && (self.min[1]..=self.max[1]).contains(&p[1])
    }

    /// The pre-RE-311 full-tile lattice restricted to this box: step
    /// `SAMPLE_STEP_Q5`, phase 0 (training) or 4 (holdout). Every point is
    /// reachable by some pose, so it is the conservative validation set.
    pub fn lattice(&self, phase: i32) -> Vec<[i32; 2]> {
        let step = ssb_rom::filter_compensation::SAMPLE_STEP_Q5;
        let first = |lo: i32| lo + (phase - lo).rem_euclid(step);
        (first(self.min[1])..=self.max[1])
            .step_by(step as usize)
            .flat_map(|t| {
                (first(self.min[0])..=self.max[0])
                    .step_by(step as usize)
                    .map(move |s| [s, t])
            })
            .collect()
    }

    /// Full-box regularization: one S10.5 point per touched texel, at a
    /// sub-texel phase that rotates deterministically between texels so it
    /// does not only ever probe texel centres. One point per texel is 1/16
    /// of the old full-tile training density (`SAMPLE_STEP_Q5` = 8).
    pub fn regularization(&self) -> Vec<[i32; 2]> {
        const PHASES: [[i32; 2]; 4] = [[16, 16], [8, 24], [24, 8], [17, 15]];
        let mut out = Vec::new();
        for y in self.min[1].div_euclid(32)..=self.max[1].div_euclid(32) {
            for x in self.min[0].div_euclid(32)..=self.max[0].div_euclid(32) {
                let ph = PHASES[(x + 2 * y).rem_euclid(4) as usize];
                let p = [x * 32 + ph[0], y * 32 + ph[1]];
                if self.contains(p) {
                    out.push(p);
                } else {
                    // A box narrower than one texel still gets its centre.
                    out.push([
                        p[0].clamp(self.min[0], self.max[0]),
                        p[1].clamp(self.min[1], self.max[1]),
                    ]);
                }
            }
        }
        out.sort_unstable();
        out.dedup();
        out
    }
}

/// What the packer needs about a texgen primitive beyond its sample sets.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct TexgenMeta {
    pub bound: TexgenBound,
    /// `G_TEXTURE_GEN_LINEAR`; reporting only.
    pub linear: bool,
    /// The real-normal holdout. Duplicates `Coverage::validation` in the
    /// normal mode; in `legacy` mode it is kept only for reporting.
    pub holdout: Vec<[i32; 2]>,
    /// `ROMTOOL_TEXGEN_COVERAGE=full-tile`: optimise and gate exactly as
    /// before RE-311 (empty train/validation), but still report against
    /// the real-normal and conservative sets.
    pub legacy: bool,
    /// `ROMTOOL_TEXGEN_COVERAGE=full-tile-per-primitive`: `legacy`, but each
    /// primitive converts (and reports) separately instead of sharing the
    /// first primitive's cache entry, so every real-coverage report line
    /// has a full-tile counterpart on identical sample sets.
    pub per_primitive: bool,
}

const BARYCENTRIC_N: i32 = 8;
const TRAIN_PER_CELL: usize = 8;
const VALIDATE_PER_CELL: usize = 48;
const MAX_TRAIN: usize = 4096;
const MAX_VALIDATION: usize = 16384;

fn evenly_spaced(samples: &[[i32; 2]], limit: usize) -> Vec<[i32; 2]> {
    if samples.len() <= limit {
        return samples.to_vec();
    }
    (0..limit)
        .map(|i| samples[i * samples.len() / limit])
        .collect()
}

fn inside(uv: &[[i32; 2]; 3], p: [i32; 2]) -> bool {
    if uv[0] == uv[1] && uv[1] == uv[2] {
        return p == uv[0];
    }
    let cross =
        |a: [i32; 2], b: [i32; 2]| -> i64 { a[0] as i64 * b[1] as i64 - a[1] as i64 * b[0] as i64 };
    let mut sign = 0;
    for i in 0..3 {
        let a = uv[i];
        let b = uv[(i + 1) % 3];
        let v = cross([b[0] - a[0], b[1] - a[1]], [p[0] - a[0], p[1] - a[1]]);
        if v != 0 {
            let next = v.signum();
            if sign != 0 && sign != next {
                return false;
            }
            sign = next;
        }
    }
    true
}

fn touched(uv: &[[i32; 2]; 3], x: i32, y: i32) -> bool {
    // Clip the UV triangle against the texel cell. The intersection can be a
    // sliver with no lattice point at all; such a cell has no S10.5 sample.
    let mut poly: Vec<[f64; 2]> = uv.iter().map(|p| [p[0] as f64, p[1] as f64]).collect();
    for (axis, edge, keep_greater) in [
        (0, (x * 32) as f64, true),
        (0, ((x + 1) * 32) as f64, false),
        (1, (y * 32) as f64, true),
        (1, ((y + 1) * 32) as f64, false),
    ] {
        let old = core::mem::take(&mut poly);
        if old.is_empty() {
            return false;
        }
        let keep = |p: [f64; 2]| {
            if keep_greater {
                p[axis] >= edge
            } else {
                p[axis] <= edge
            }
        };
        let mut prev = *old.last().unwrap();
        for &cur in &old {
            if keep(cur) != keep(prev) {
                let ratio = (edge - prev[axis]) / (cur[axis] - prev[axis]);
                poly.push([
                    prev[0] + ratio * (cur[0] - prev[0]),
                    prev[1] + ratio * (cur[1] - prev[1]),
                ]);
            }
            if keep(cur) {
                poly.push(cur);
            }
            prev = cur;
        }
    }
    !poly.is_empty()
}

fn add_cell(
    uv: &[[i32; 2]; 3],
    x: i32,
    y: i32,
    train: &mut Vec<[i32; 2]>,
    validation: &mut Vec<[i32; 2]>,
) {
    add_cell_where(|p| inside(uv, p), x, y, train, validation);
}

/// [`add_cell`]'s probe pattern for any footprint predicate.
fn add_cell_where(
    inside: impl Fn([i32; 2]) -> bool,
    x: i32,
    y: i32,
    train: &mut Vec<[i32; 2]>,
    validation: &mut Vec<[i32; 2]>,
) {
    let base = [x * 32, y * 32];
    let mut local_train = Vec::new();
    let mut local_validation = Vec::new();
    let mut push = |s: i32, t: i32, validate: bool| {
        if !(0..32).contains(&s) || !(0..32).contains(&t) {
            return;
        }
        let p = [base[0] + s, base[1] + t];
        if inside(p) {
            let out = if validate {
                &mut local_validation
            } else {
                &mut local_train
            };
            if !out.contains(&p) {
                out.push(p);
            }
        }
    };

    // The largest triangular-versus-bilinear gap lies on the diagonal.
    // Three adjacent S10.5 sums explicitly straddle its branch switch.
    // A clipped/slender triangle may miss all fixed diagonal positions, so
    // also take the middle representable point on each intersected line.
    for sum in [31, 32, 33] {
        let valid: Vec<_> = (0..32)
            .filter(|&s| {
                let t = sum - s;
                (0..32).contains(&t) && inside([base[0] + s, base[1] + t])
            })
            .collect();
        if let Some(&s) = valid.get((valid.len().saturating_sub(1)) / 2) {
            push(s, sum - s, false);
        }
    }
    // Keep a half texel and GE quantization transitions in the capped
    // training set. The two per-axis boundaries rotate deterministically
    // between cells, while validation checks every boundary independently.
    push(16, 16, false);
    let phase = (x.wrapping_mul(5) ^ y.wrapping_mul(11)).rem_euclid(16) * 2;
    for s in [phase, (phase + 8).rem_euclid(32)] {
        push(s, 7, false);
        push(7, s, false);
    }
    for (s, t) in [(0, 0), (31, 0), (0, 31), (31, 31), (16, 0), (0, 16)] {
        push(s, t, false);
    }

    // Independent, denser holdout: different diagonal positions and both
    // sides of every 1/16 GE boundary. Remove any training coordinate below.
    for (s, t) in [(0, 16), (16, 0), (31, 0), (0, 31), (31, 31)] {
        push(s, t, true);
    }
    for s in [3, 11, 19, 27] {
        for sum in [30, 34] {
            push(s, sum - s, true);
        }
    }
    for v in (0..32).step_by(2) {
        push(v, 9, true);
        push(23, v, true);
    }
    for s in [1, 8, 15, 17, 24, 30] {
        for t in [1, 8, 15, 17, 24, 30] {
            push(s, t, true);
        }
    }
    if local_train.is_empty() || local_validation.is_empty() {
        // A long, thin triangle may miss every priority probe in a touched
        // cell. Find its actual integer footprint instead of dropping it.
        for t in 0..32 {
            for s in 0..32 {
                let p = [base[0] + s, base[1] + t];
                if inside(p) {
                    if local_train.is_empty() {
                        local_train.push(p);
                    } else if !local_train.contains(&p) && local_validation.is_empty() {
                        local_validation.push(p);
                    }
                }
            }
        }
    }
    local_validation.retain(|p| !local_train.contains(p));
    train.extend(local_train.into_iter().take(TRAIN_PER_CELL));
    validation.extend(local_validation.into_iter().take(VALIDATE_PER_CELL));
}

pub(super) fn build(triangles: &[[[i32; 2]; 3]]) -> Coverage {
    let mut result = Coverage::default();
    let mut base = Vec::new();
    for uv in triangles {
        // Preserve the original N=8 lattice as broad coverage.
        for a in 0..=BARYCENTRIC_N {
            for b in 0..=BARYCENTRIC_N - a {
                let c = BARYCENTRIC_N - a - b;
                base.push([
                    (uv[0][0] * a + uv[1][0] * b + uv[2][0] * c) / BARYCENTRIC_N,
                    (uv[0][1] * a + uv[1][1] * b + uv[2][1] * c) / BARYCENTRIC_N,
                ]);
            }
        }
        // The vertices are the exact UV extrema and must survive even when
        // a thin triangle has no interior S10.5 coordinate in some cells.
        base.extend(*uv);
        let min_x = uv.iter().map(|p| p[0]).min().unwrap().div_euclid(32);
        let max_x = uv.iter().map(|p| p[0]).max().unwrap().div_euclid(32);
        let min_y = uv.iter().map(|p| p[1]).min().unwrap().div_euclid(32);
        let max_y = uv.iter().map(|p| p[1]).max().unwrap().div_euclid(32);
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                if touched(uv, x, y) {
                    add_cell(uv, x, y, &mut result.train, &mut result.validation);
                }
            }
        }
    }
    result.train.extend(&base);
    result.train.sort_unstable();
    result.train.dedup();
    if result.train.len() > MAX_TRAIN {
        base.sort_unstable();
        base.dedup();
        // The N=8 lattice and UV extrema are immutable broad coverage. A
        // stretched primitive gets a deterministic spread of extra probes
        // within the remaining training budget.
        let extras: Vec<_> = result
            .train
            .iter()
            .copied()
            .filter(|p| base.binary_search(p).is_err())
            .collect();
        let extra_limit = MAX_TRAIN.saturating_sub(base.len());
        base.extend(evenly_spaced(&extras, extra_limit));
        base.sort_unstable();
        base.dedup();
        result.train = base;
    }
    result.validation.sort_unstable();
    result.validation.dedup();
    result
        .validation
        .retain(|p| result.train.binary_search(p).is_err());
    result.validation = evenly_spaced(&result.validation, MAX_VALIDATION);
    result
}

/// One texgen primitive, in exactly the terms the runtime generator reads.
pub(super) struct TexgenPrimitive<'a> {
    /// `G_TEXTURE_GEN_LINEAR` (the `acos` curve) rather than ordinary.
    pub linear: bool,
    /// Each triangle's three real `i8` vertex normals (`MeshVertex::rgba`,
    /// the same bytes `PackedVertex::nx/ny/nz` carries).
    pub normals: &'a [[[i8; 3]; 3]],
    /// `gSPTexture`'s raw `scale_s`/`scale_t`.
    pub scale: [u16; 2],
    /// The render tile's origin, S10.2, applied on clamped axes only.
    pub origin: [u16; 2],
    pub clamp: [bool; 2],
}

/// Samples per reachable texel, for both training and holdout: the old
/// full-tile lattice's density (`SAMPLE_STEP_Q5` = 8 gives 4x4 per texel).
/// Every texel carries four free channels, so a budget near one sample per
/// unknown overfits: measured on Metal Mario's 48x42 body texture, a fixed
/// 8,192-point training set improved its own SSE 36% while the independent
/// real-pose holdout moved only -15%..+2%.
const TEXGEN_SAMPLES_PER_TEXEL: usize = 16;

/// Share of the training budget reserved for [`add_cell`]'s critical
/// probes. They sit at a few fixed sub-texel phases; weighting them more
/// heavily fits those phases at the expense of every other position.
const PROBE_SHARE_PERCENT: usize = 25;

fn texgen_budget(bound: &TexgenBound, floor: usize) -> usize {
    let span = |i: usize| ((bound.max[i] - bound.min[i]) as usize).div_ceil(32).max(1);
    (TEXGEN_SAMPLES_PER_TEXEL * span(0) * span(1)).max(floor)
}

/// Training and holdout pose sets: the 60 rotations of the icosahedral
/// group, each applied to two base orientations. The group is the most
/// uniform finite subgroup of SO(3), so this is a quadrature of *every*
/// orientation rather than a guessed camera. The base quaternions are
/// arbitrary small integers, distinct between the two sets so training and
/// holdout never share a pose. Only `+ - * /` and `sqrt` are used, all
/// correctly rounded in IEEE 754, so the poses are bit-identical on every
/// host.
const TRAIN_BASES: [[i32; 4]; 6] = [
    [1, 2, 3, 4],
    [4, -1, 2, -3],
    [5, 3, -1, 2],
    [2, 5, 4, -1],
    [3, -2, 5, 1],
    [1, 4, -2, 6],
];
const HOLDOUT_BASES: [[i32; 4]; 6] = [
    [2, -3, 1, 5],
    [3, 1, -4, 2],
    [6, 1, 2, -2],
    [1, -5, 3, 3],
    [4, 4, 1, -5],
    [2, 2, -6, 1],
];

fn quat_mul(a: [f64; 4], b: [f64; 4]) -> [f64; 4] {
    [
        a[0] * b[0] - a[1] * b[1] - a[2] * b[2] - a[3] * b[3],
        a[0] * b[1] + a[1] * b[0] + a[2] * b[3] - a[3] * b[2],
        a[0] * b[2] - a[1] * b[3] + a[2] * b[0] + a[3] * b[1],
        a[0] * b[3] + a[1] * b[2] - a[2] * b[1] + a[3] * b[0],
    ]
}

/// The 60 rotations of the icosahedral group as unit quaternions (one of
/// each `±q` pair): the binary icosahedral group's 120 units.
fn icosahedral_rotations() -> Vec<[f64; 4]> {
    let phi = (1.0 + 5f64.sqrt()) / 2.0;
    let mut all = Vec::new();
    for i in 0..4 {
        for sign in [1.0, -1.0] {
            let mut q = [0.0; 4];
            q[i] = sign;
            all.push(q);
        }
    }
    for bits in 0..16 {
        all.push(core::array::from_fn(|i| {
            if bits >> i & 1 == 0 {
                0.5
            } else {
                -0.5
            }
        }));
    }
    const EVEN: [[usize; 4]; 12] = [
        [0, 1, 2, 3],
        [0, 2, 3, 1],
        [0, 3, 1, 2],
        [1, 0, 3, 2],
        [1, 2, 0, 3],
        [1, 3, 2, 0],
        [2, 0, 1, 3],
        [2, 1, 3, 0],
        [2, 3, 0, 1],
        [3, 0, 2, 1],
        [3, 1, 0, 2],
        [3, 2, 1, 0],
    ];
    let base = [0.0, 0.5, phi / 2.0, 0.5 / phi];
    for perm in EVEN {
        for bits in 0..8 {
            let mut q = [0.0; 4];
            for (k, &slot) in perm.iter().enumerate() {
                let sign = if k > 0 && bits >> (k - 1) & 1 != 0 {
                    -1.0
                } else {
                    1.0
                };
                q[slot] = base[k] * sign;
            }
            all.push(q);
        }
    }
    // `q` and `-q` are the same rotation: keep the one whose first nonzero
    // component is positive.
    all.retain(|q| q.iter().find(|&&v| v != 0.0).is_some_and(|&v| v > 0.0));
    all
}

/// Object-space `(basis_s, basis_t)` for each pose, through the same chain
/// `DrawState::texgen_object_basis` runs: a world look-at basis (the pose's
/// rotated `IDENTITY_TEXGEN_BASIS`) quantised to the RSP's signed bytes,
/// then `M^T * l`, normalised. Folding the whole camera/node rotation into
/// the pose leaves `M` as identity, which still performs the normalisation.
fn texgen_poses(bases: &[[i32; 4]]) -> Vec<([f32; 3], [f32; 3])> {
    use ssb_engine::math::{quantize_lookat_basis, transform_lookat_basis};
    const IDENTITY: [[f32; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    let group = icosahedral_rotations();
    let mut poses = Vec::with_capacity(group.len() * bases.len());
    for b in bases {
        let len = ((b[0] * b[0] + b[1] * b[1] + b[2] * b[2] + b[3] * b[3]) as f64).sqrt();
        let b = b.map(|v| v as f64 / len);
        for g in &group {
            let [w, x, y, z] = quat_mul(*g, b);
            let right = [
                1.0 - 2.0 * (y * y + z * z),
                2.0 * (x * y + w * z),
                2.0 * (x * z - w * y),
            ];
            let up = [
                2.0 * (x * y - w * z),
                1.0 - 2.0 * (x * x + z * z),
                2.0 * (y * z + w * x),
            ];
            let chain = |v: [f64; 3]| {
                transform_lookat_basis(IDENTITY, quantize_lookat_basis(v.map(|c| c as f32)))
            };
            poses.push((chain(right), chain(up)));
        }
    }
    poses
}

fn texgen_uv(p: &TexgenPrimitive<'_>, n: [i8; 3], basis: ([f32; 3], [f32; 3])) -> [i32; 2] {
    let f = if p.linear {
        ssb_rom::psp_texture::linear_texgen_uv
    } else {
        ssb_rom::psp_texture::regular_texgen_uv
    };
    let (u, v) = f(
        n,
        basis.0,
        basis.1,
        p.scale[0],
        p.scale[1],
        p.origin[0],
        p.origin[1],
        p.clamp[0],
        p.clamp[1],
    );
    [u as i32, v as i32]
}

/// The pose-independent S10.5 box (see [`TexgenBound`]), padded by one unit
/// for the f32 rounding of the dot product.
pub(super) fn texgen_bound(p: &TexgenPrimitive<'_>) -> TexgenBound {
    let longest = p
        .normals
        .iter()
        .flatten()
        .map(|n| n.iter().map(|&c| c as i32 * c as i32).sum::<i32>())
        .max()
        .unwrap_or(0);
    let r = ((longest as f32).sqrt() / 127.0).min(1.0);
    let curve = |d: f32| {
        if p.linear {
            ssb_rom::psp_texture::linear_texgen_curve(d)
        } else {
            ssb_rom::psp_texture::regular_texgen_curve(d)
        }
    };
    let axis = |i: usize| {
        let shift = if p.clamp[i] {
            p.origin[i] as i32 * 8
        } else {
            0
        };
        let at = |d: f32| (curve(d) * p.scale[i] as f32) as i32 - shift;
        (at(-r) - 1, at(r) + 1)
    };
    let (s, t) = (axis(0), axis(1));
    TexgenBound {
        min: [s.0, t.0],
        max: [s.1, t.1],
    }
}

/// A deterministic, cell-stratified subsample. Every texel cell the input
/// touches keeps its lowest-hash point, so the budget never drops a region
/// some pose reaches; the remainder is filled in hash order, which is
/// spatially unbiased. Sorted on return.
fn hashed_subsample(points: Vec<[i32; 2]>, limit: usize) -> Vec<[i32; 2]> {
    if points.len() <= limit {
        let mut points = points;
        points.sort_unstable();
        return points;
    }
    let key = |p: &[i32; 2]| {
        let mut z = (p[0] as u32 as u64) << 32 | p[1] as u32 as u64;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^ (z >> 31)
    };
    let mut ordered: Vec<_> = points.into_iter().map(|p| (key(&p), p)).collect();
    ordered.sort_unstable();
    let mut seen = std::collections::BTreeSet::new();
    let (first, rest): (Vec<_>, Vec<_>) = ordered
        .into_iter()
        .partition(|(_, p)| seen.insert([p[0].div_euclid(32), p[1].div_euclid(32)]));
    let mut out: Vec<_> = first
        .into_iter()
        .chain(rest)
        .map(|(_, p)| p)
        .take(limit)
        .collect();
    out.sort_unstable();
    out
}

/// Every vertex position an orthonormal pose can generate: for a normal
/// of length `|n|`, `(dot_s, dot_t)` fills the disk of radius `|n| / 127`.
/// Sampled on a polar grid of the primitive's longest normal (the smaller
/// disks are inside it), using the rational circle parametrisation so no
/// trigonometry enters the ordinary curve. `phase` staggers the holdout
/// grid between the training grid's rings and spokes.
fn reach_points(p: &TexgenPrimitive<'_>, phase: f32) -> Vec<[i32; 2]> {
    const RINGS: u32 = 48;
    const SPOKES: u32 = 96;
    let longest = p
        .normals
        .iter()
        .flatten()
        .copied()
        .max_by_key(|n| n.iter().map(|&c| c as i32 * c as i32).sum::<i32>());
    let Some(longest) = longest else {
        return Vec::new();
    };
    let length = (longest.iter().map(|&c| c as i32 * c as i32).sum::<i32>() as f32).sqrt() / 127.0;
    let mut out = Vec::new();
    for ring in 0..=RINGS {
        let rho = length * ((ring as f32 + phase) / RINGS as f32).min(1.0);
        for spoke in 0..SPOKES {
            let k = (spoke as f32 + phase) / SPOKES as f32;
            let (c, sn) = ((1.0 - k * k) / (1.0 + k * k), 2.0 * k / (1.0 + k * k));
            for (x, y) in [(c, sn), (-sn, c), (-c, -sn), (sn, -c)] {
                // A basis whose `x` components carry the target dots turns
                // the `+X` unit normal into exactly `(rho x, rho y)`.
                let basis = ([rho * x, 0.0, 0.0], [rho * y, 0.0, 0.0]);
                out.push(texgen_uv(p, [127, 0, 0], basis));
            }
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// The N=8 barycentric lattice and exact vertex UVs of every triangle
/// under every pose, through the exact runtime generator.
fn pose_points(p: &TexgenPrimitive<'_>, bases: &[[i32; 4]]) -> Vec<[i32; 2]> {
    let mut out = Vec::new();
    for pose in texgen_poses(bases) {
        for tri in p.normals {
            let uv = tri.map(|n| texgen_uv(p, n, pose));
            out.extend(uv);
            for a in 0..=BARYCENTRIC_N {
                for b in 0..=BARYCENTRIC_N - a {
                    let c = BARYCENTRIC_N - a - b;
                    out.push([
                        (uv[0][0] * a + uv[1][0] * b + uv[2][0] * c) / BARYCENTRIC_N,
                        (uv[0][1] * a + uv[1][1] * b + uv[2][1] * c) / BARYCENTRIC_N,
                    ]);
                }
            }
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// Real-normal texgen coverage (RE-311).
///
/// Broad coverage is every triangle's actual normals run through
/// [`TRAIN_BASES`]' poses and the exact runtime generator (the N=8
/// barycentric lattice and vertices of each generated UV triangle), plus
/// [`reach_points`]. Every texel cell that reaches then gets [`add_cell`]'s
/// critical probe pattern (diagonal branch switch, half texel, GE 1/16
/// boundaries, seams), restricted to the pose-independent [`TexgenBound`]:
/// the union of hundreds of poses covers a reached cell far more densely
/// than any one authored triangle, so per-triangle clipping would only
/// multiply cost. The holdout is the same construction through the
/// disjoint [`HOLDOUT_BASES`] poses, the staggered reach grid, and the
/// probe pattern's own holdout half.
///
/// Budget: [`texgen_budget`]. Critical probes take priority for up to
/// [`PROBE_SHARE_PERCENT`] of it (cell-stratified); broad real-pose points
/// fill the rest.
pub(super) fn build_texgen(p: &TexgenPrimitive<'_>) -> Coverage {
    let bound = texgen_bound(p);
    let mut broad = pose_points(p, &TRAIN_BASES);
    broad.extend(reach_points(p, 0.0));
    broad.sort_unstable();
    broad.dedup();
    let cells: std::collections::BTreeSet<[i32; 2]> = broad
        .iter()
        .map(|q| [q[0].div_euclid(32), q[1].div_euclid(32)])
        .collect();
    let mut probes = Vec::new();
    let mut probe_holdout = Vec::new();
    for &[x, y] in &cells {
        add_cell_where(|q| bound.contains(q), x, y, &mut probes, &mut probe_holdout);
    }
    probes.sort_unstable();
    probes.dedup();
    let train_budget = texgen_budget(&bound, MAX_TRAIN);
    let probes = hashed_subsample(probes, train_budget * PROBE_SHARE_PERCENT / 100);
    broad.retain(|q| probes.binary_search(q).is_err());
    let mut train = hashed_subsample(broad, train_budget - probes.len());
    train.extend(probes);
    train.sort_unstable();

    let mut validation = pose_points(p, &HOLDOUT_BASES);
    validation.extend(reach_points(p, 0.5));
    validation.extend(probe_holdout);
    validation.sort_unstable();
    validation.dedup();
    validation.retain(|q| train.binary_search(q).is_err());
    let validation = hashed_subsample(validation, texgen_budget(&bound, MAX_VALIDATION));
    debug_assert!(train.iter().chain(&validation).all(|&q| bound.contains(q)));
    Coverage {
        texgen: Some(TexgenMeta {
            bound,
            linear: p.linear,
            holdout: validation.clone(),
            legacy: false,
            per_primitive: false,
        }),
        train,
        validation,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ssb_rom::{n64_filter, texture::Rgba8};

    #[test]
    fn saddle_diagonal_worst_case_is_missed_by_sparse_lattice() {
        let uv = [[27, 29], [12, 1], [9, 5]];
        let coverage = build(&[uv]);
        let mut img = Rgba8::new(2, 2);
        for (i, c) in [0, 255, 255, 0].into_iter().enumerate() {
            img.put(i, [c, c, c, 255]);
        }
        let error = |p: [i32; 2]| {
            n64_filter::sample_3point(&img, p[0], p[1])[0]
                .abs_diff(n64_filter::sample_bilinear(&img, p[0], p[1])[0])
        };
        let sparse: Vec<_> = (0..=8)
            .flat_map(|a| {
                (0..=8 - a).map(move |b| {
                    let c = 8 - a - b;
                    [
                        (uv[0][0] * a + uv[1][0] * b + uv[2][0] * c) / 8,
                        (uv[0][1] * a + uv[1][1] * b + uv[2][1] * c) / 8,
                    ]
                })
            })
            .collect();
        let sparse_max = sparse.into_iter().map(error).max().unwrap();
        let actual_max = (0..32)
            .flat_map(|s| (0..32).map(move |t| [s, t]))
            .filter(|&p| inside(&uv, p))
            .map(error)
            .max()
            .unwrap();
        assert!(sparse_max < actual_max);
        assert_eq!(actual_max, 128);
        assert_eq!(
            coverage.train.iter().copied().map(error).max(),
            Some(actual_max)
        );
        assert!(coverage.train.contains(&[17, 15]));
    }

    #[test]
    fn clamp_repeat_and_mirror_seams_have_both_sides() {
        // Width two: Q5 64 is a repeat or prebaked mirror seam; Q5 0 is
        // also the near clamp edge. The same authored triangle crosses both.
        let c = build(&[[[-4, -4], [72, -4], [-4, 40]]]);
        for x in [-1, 0, 31, 32, 63, 64] {
            assert!(c.train.iter().any(|p| p[0] == x), "missing seam/edge {x}");
        }
        assert!(c.validation.iter().any(|p| p[0] == 65));
        assert!(c.train.iter().all(|p| !c.validation.contains(p)));
    }

    #[test]
    fn prebaked_mirror_pair_and_ge_boundaries_are_covered() {
        let mut original = Rgba8::new(2, 2);
        for (i, c) in [20, 240, 80, 160].into_iter().enumerate() {
            original.put(i, [c, c, c, 255]);
        }
        let mirrored = ssb_rom::texture::mirror_extend(&original, true, false, false, false, 2, 2);
        assert_eq!(mirrored.width, 4);
        let c = build(&[[[0, 0], [128, 0], [0, 32]], [[128, 0], [128, 32], [0, 32]]]);
        for seam in [64, 128] {
            assert!(c.train.iter().any(|p| p[0] == seam));
            assert!(c.train.iter().any(|p| p[0] == seam - 1));
            assert!(c.validation.iter().any(|p| p[0] == seam - 2));
        }
        for frac in (0..32).step_by(2) {
            assert!(c.validation.iter().any(|p| p[0].rem_euclid(32) == frac));
        }
    }

    #[test]
    fn tiny_textures_keep_critical_samples_and_independent_validation() {
        for side in [2, 4] {
            let edge = side * 32;
            let c = build(&[[[0, 0], [edge, 0], [0, edge]]]);
            assert!(c.train.contains(&[16, 16]));
            assert!(c
                .train
                .iter()
                .any(|p| p[0].rem_euclid(32) + p[1].rem_euclid(32) == 31));
            assert!(c
                .train
                .iter()
                .any(|p| p[0].rem_euclid(32) + p[1].rem_euclid(32) == 33));
            assert!(c.validation.len() > c.train.len());
            assert!(c.train.iter().all(|p| !c.validation.contains(p)));
        }
    }

    #[test]
    fn large_stretched_triangle_is_bounded_and_deterministic() {
        let triangles = [[[0, 0], [2048, 0], [0, 512]]];
        let c = build(&triangles);
        assert_eq!(c, build(&triangles));
        assert!(c.train.contains(&[2048, 0]));
        for a in 0..=BARYCENTRIC_N {
            for b in 0..=BARYCENTRIC_N - a {
                let p = [
                    2048 * b / BARYCENTRIC_N,
                    512 * (BARYCENTRIC_N - a - b) / BARYCENTRIC_N,
                ];
                assert!(c.train.contains(&p));
            }
        }
        assert!(c.train.len() <= MAX_TRAIN);
        assert!(c.validation.len() <= MAX_VALIDATION);
        assert!(c.train.iter().all(|p| !c.validation.contains(p)));
    }

    #[test]
    fn zero_area_uv_triangle_keeps_only_its_authored_point() {
        let c = build(&[[[7, 11], [7, 11], [7, 11]]]);
        assert_eq!(c.train, vec![[7, 11]]);
        assert!(c.validation.is_empty());
    }

    fn octahedron_normals() -> Vec<[[i8; 3]; 3]> {
        // Eight faces of an octahedron: every triangle's normals are three
        // orthogonal unit axes, the widest footprint a real mesh produces.
        let mut out = Vec::new();
        for sx in [127i8, -127] {
            for sy in [127i8, -127] {
                for sz in [127i8, -127] {
                    out.push([[sx, 0, 0], [0, sy, 0], [0, 0, sz]]);
                }
            }
        }
        out
    }

    fn metal_like(linear: bool, normals: &[[[i8; 3]; 3]]) -> TexgenPrimitive<'_> {
        // The dominant real metal-fighter state (`romtool texgen`): a 32x32
        // wrapped tile under `gSPTexture(0x07C0, 0x07C0)`.
        TexgenPrimitive {
            linear,
            normals,
            scale: [0x07C0, 0x07C0],
            origin: [0, 0],
            clamp: [false, false],
        }
    }

    #[test]
    fn icosahedral_group_has_sixty_distinct_unit_rotations() {
        let g = icosahedral_rotations();
        assert_eq!(g.len(), 60);
        for q in &g {
            let n: f64 = q.iter().map(|v| v * v).sum();
            assert!((n - 1.0).abs() < 1e-12);
        }
        // Closed under composition, up to sign.
        for a in &g {
            for b in &g {
                let c = quat_mul(*a, *b);
                assert!(g.iter().any(|q| {
                    (0..4).all(|i| (q[i] - c[i]).abs() < 1e-9)
                        || (0..4).all(|i| (q[i] + c[i]).abs() < 1e-9)
                }));
            }
        }
    }

    #[test]
    fn known_normals_reproduce_the_runtime_generator_through_every_pose() {
        // The +X normal under the identity pose: dot_s = 127/127, dot_t = 0.
        let p = metal_like(false, &[]);
        let identity = ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
        assert_eq!(texgen_uv(&p, [127, 0, 0], identity), [992, 496]);
        assert_eq!(texgen_uv(&p, [-127, 0, 0], identity), [0, 496]);
        assert_eq!(texgen_uv(&p, [0, 0, 127], identity), [496, 496]);
        // Every pose's basis is unit length and orthogonal to within the
        // look-at byte quantisation.
        for (s, t) in texgen_poses(&TRAIN_BASES)
            .into_iter()
            .chain(texgen_poses(&HOLDOUT_BASES))
        {
            let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
            assert!((dot(s, s) - 1.0).abs() < 1e-5);
            assert!((dot(t, t) - 1.0).abs() < 1e-5);
            assert!(dot(s, t).abs() < 0.03);
        }
    }

    #[test]
    fn ordinary_texgen_coverage_follows_real_normals_and_stays_bounded() {
        let normals = octahedron_normals();
        let p = metal_like(false, &normals);
        let c = build_texgen(&p);
        let bound = c.texgen.as_ref().unwrap().bound;
        assert_eq!(
            bound,
            TexgenBound {
                min: [-1, -1],
                max: [993, 993]
            }
        );
        assert!(c.train.len() <= texgen_budget(&bound, MAX_TRAIN) && !c.train.is_empty());
        assert!(
            c.validation.len() <= texgen_budget(&bound, MAX_VALIDATION) && !c.validation.is_empty()
        );
        assert!(c
            .train
            .iter()
            .chain(&c.validation)
            .all(|&q| bound.contains(q)));
        assert!(c
            .train
            .iter()
            .all(|q| c.validation.binary_search(q).is_err()));
        // Orthonormal poses put a unit normal on a disk: the tile corners are
        // never generated even though the conservative box includes them.
        let centre = 496.0;
        assert!(c.train.iter().all(|q| {
            let (ds, dt) = (q[0] as f64 - centre, q[1] as f64 - centre);
            // A reached rim cell's probe pattern may extend one cell
            // diagonal (45 S10.5 units) past the disk, never further.
            (ds * ds + dt * dt).sqrt() <= centre + 46.0
        }));
    }

    #[test]
    fn short_normals_shrink_the_bound() {
        let half = [[[63i8, 0, 0], [0, 63, 0], [0, 0, 63]]];
        let bound = texgen_bound(&metal_like(false, &half));
        assert!(bound.min[0] > 200 && bound.max[0] < 800, "{bound:?}");
    }

    #[test]
    fn linear_texgen_uses_the_exact_cpu_generator() {
        let normals = octahedron_normals();
        let p = metal_like(true, &normals);
        for pose in texgen_poses(&TRAIN_BASES).into_iter().take(8) {
            let n = [90, -30, 80];
            let (u, v) = ssb_rom::psp_texture::linear_texgen_uv(
                n, pose.0, pose.1, 0x07C0, 0x07C0, 0, 0, false, false,
            );
            assert_eq!(texgen_uv(&p, n, pose), [u as i32, v as i32]);
        }
        let c = build_texgen(&p);
        let bound = c.texgen.as_ref().unwrap().bound;
        assert!(c
            .train
            .iter()
            .chain(&c.validation)
            .all(|&q| bound.contains(q)));
        // Linear and ordinary curves share endpoints but not the interior.
        assert_ne!(c.train, build_texgen(&metal_like(false, &normals)).train);
    }

    #[test]
    fn clamped_origin_and_small_scale_bound_only_the_reachable_texels() {
        // The real 159x79 clamped stage tile under `gSPTexture(0x0400,
        // 0x0200)` with origin (12, 0): 16x8 reachable texels, not the tile.
        let normals = octahedron_normals();
        let p = TexgenPrimitive {
            linear: false,
            normals: &normals,
            scale: [0x0400, 0x0200],
            origin: [12, 0],
            clamp: [true, true],
        };
        let c = build_texgen(&p);
        let bound = c.texgen.as_ref().unwrap().bound;
        assert_eq!(
            bound,
            TexgenBound {
                min: [-97, -1],
                max: [417, 257]
            }
        );
        assert!(c
            .train
            .iter()
            .chain(&c.validation)
            .all(|&q| bound.contains(q)));
    }

    #[test]
    fn unseen_camera_poses_stay_inside_the_bound_and_near_training_cells() {
        // Camera/look-at variation outside the quadrature: arbitrary
        // (including non-orthogonal, i.e. non-uniformly scaled node) bases.
        let normals = octahedron_normals();
        let p = metal_like(false, &normals);
        let c = build_texgen(&p);
        let bound = c.texgen.as_ref().unwrap().bound;
        let cells: std::collections::BTreeSet<_> = c
            .train
            .iter()
            .map(|q| [q[0].div_euclid(32), q[1].div_euclid(32)])
            .collect();
        let mut state = 0x2545F4914F6CDD1Du64;
        let mut next = || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state >> 11) as f32 / (1u64 << 53) as f32 * 2.0 - 1.0
        };
        let mut covered = 0;
        let mut total = 0;
        for orthogonal in [true, false] {
            for _ in 0..256 {
                let norm = |v: [f32; 3]| {
                    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
                    v.map(|c| c / l)
                };
                let s = norm([next(), next(), next()]);
                let mut t = norm([next(), next(), next()]);
                if orthogonal {
                    let d = s[0] * t[0] + s[1] * t[1] + s[2] * t[2];
                    t = norm([t[0] - d * s[0], t[1] - d * s[1], t[2] - d * s[2]]);
                }
                for tri in &normals {
                    for n in tri {
                        let q = texgen_uv(&p, *n, (s, t));
                        assert!(bound.contains(q), "{q:?} outside {bound:?}");
                        if orthogonal {
                            total += 1;
                            covered +=
                                cells.contains(&[q[0].div_euclid(32), q[1].div_euclid(32)]) as i32;
                        }
                    }
                }
            }
        }
        assert!(covered * 100 >= total * 99, "{covered}/{total}");
    }

    #[test]
    fn conservative_lattice_and_regularization_cover_the_whole_box() {
        let bound = TexgenBound {
            min: [-1, -1],
            max: [993, 993],
        };
        let lattice = bound.lattice(4);
        assert!(lattice
            .iter()
            .all(|&q| bound.contains(q) && q[0].rem_euclid(8) == 4));
        assert_eq!(lattice.len(), 124 * 124);
        let reg = bound.regularization();
        assert!(reg.iter().all(|&q| bound.contains(q)));
        assert_eq!(reg.len(), 33 * 33);
        // Regularization is a sixteenth of the old training density.
        assert_eq!(bound.lattice(0).len() / reg.len(), 14);
        let tiny = TexgenBound {
            min: [3, 3],
            max: [5, 5],
        };
        assert_eq!(tiny.regularization(), vec![[5, 5]]);
    }

    #[test]
    fn texgen_coverage_hash_is_pinned() {
        // Only IEEE-exact operations feed ordinary texgen, so this hash is
        // host-independent. It changes only if the poses or builder do.
        let normals = octahedron_normals();
        let c = build_texgen(&metal_like(false, &normals));
        assert_eq!(c, build_texgen(&metal_like(false, &normals)));
        let hash = |v: &[[i32; 2]]| {
            v.iter().flatten().fold(0xcbf29ce484222325u64, |h, &x| {
                (h ^ x as u64).wrapping_mul(0x100000001b3)
            })
        };
        assert_eq!(
            (hash(&c.train), hash(&c.validation)),
            (13610423240582775210, 7960285149008070151)
        );
    }

    /// Real metal-fighter and metal-stage texgen primitives: Metal Mario
    /// (file 300), polygon Fox (303) and the Metal stage's linear-texgen
    /// geometry (117), at list offsets `romtool texgen` reports.
    #[test]
    fn real_rom_metal_fighter_and_stage_texgen_coverage() {
        let Some(path) = std::env::var_os("SSB64_ROM") else {
            return;
        };
        let data = std::fs::read(path).unwrap();
        let info = ssb_rom::rom::identify(&data).unwrap();
        let archive = ssb_rom::archive::Archive::open(&data, info.region).unwrap();
        let mut seen = std::collections::BTreeMap::new();
        for (id, lists) in [
            (300u32, &[0xC60usize, 0xDD0, 0xEE0, 0x1130][..]),
            (303, &[0x1BF8, 0x1D40, 0x1E80][..]),
            (117, &[0x1708, 0x2950, 0x3368, 0x3B50][..]),
        ] {
            let file = archive.load(id).unwrap();
            for &at in lists {
                let cmds = ssb_rom::dl::decode_list_at(&file.data[at..], at as u32).unwrap();
                let Ok(mesh) = ssb_rom::mesh::convert(&cmds, ssb_rom::mesh::Source::of(&file))
                else {
                    continue;
                };
                for prim in &mesh.primitives {
                    let Some(scale) = prim.material.texgen_scale else {
                        continue;
                    };
                    if prim.material.mat_anim.is_some() {
                        continue;
                    }
                    // The Metal stage's texels live behind a segmented
                    // pointer a standalone list cannot resolve (RE-037); its
                    // tile state (`texture_shape`) is still the real one.
                    let Some(t) = prim.material.texture.or(prim.material.texture_shape) else {
                        continue;
                    };
                    let mut material_prim = prim.clone();
                    material_prim.material.texture = Some(t);
                    let prim = &material_prim;
                    let c = crate::texgen_filter_coverage(&mesh, prim, scale);
                    assert_eq!(c, crate::texgen_filter_coverage(&mesh, prim, scale));
                    let meta = c.texgen.as_ref().expect("bounded texgen coverage");
                    assert!(!c.train.is_empty() && !c.validation.is_empty());
                    assert!(c
                        .train
                        .iter()
                        .chain(&c.validation)
                        .all(|&q| meta.bound.contains(q)));
                    // The reachable box never exceeds one generated period
                    // (`gSPTexture` scale / 2) plus the clamp origin shift.
                    for axis in 0..2 {
                        let span = meta.bound.max[axis] - meta.bound.min[axis];
                        assert!(span <= [scale.0, scale.1][axis] as i32 / 2 + 2);
                    }
                    if id == 117 {
                        // 0x0400/0x0200 on the 159x79 tile: 16x8 texels.
                        assert!(meta.bound.max[0] - meta.bound.min[0] <= 16 * 32 + 2);
                        assert!(meta.bound.max[1] - meta.bound.min[1] <= 8 * 32 + 2);
                    }
                    // Camera/look-at variation: 128 orthonormal poses outside
                    // the quadrature, through the real normals.
                    let mut state = 0x9E3779B97F4A7C15u64 ^ at as u64;
                    let mut next = || {
                        state ^= state << 13;
                        state ^= state >> 7;
                        state ^= state << 17;
                        (state >> 11) as f32 / (1u64 << 53) as f32 * 2.0 - 1.0
                    };
                    let cells: std::collections::BTreeSet<_> = c
                        .train
                        .iter()
                        .map(|q| [q[0].div_euclid(32), q[1].div_euclid(32)])
                        .collect();
                    let input_normals: Vec<[i8; 3]> = prim
                        .indices
                        .iter()
                        .map(|&i| {
                            let v = mesh.vertices[i as usize].rgba;
                            [v[0] as i8, v[1] as i8, v[2] as i8]
                        })
                        .collect();
                    let tp = TexgenPrimitive {
                        linear: prim.material.texture_gen == ssb_rom::mesh::TextureGen::Linear,
                        normals: &[],
                        scale: [scale.0, scale.1],
                        origin: [t.origin_s, t.origin_t],
                        clamp: [t.clamp_s, t.clamp_t],
                    };
                    let (mut hit, mut total) = (0, 0);
                    for _ in 0..128 {
                        let norm = |v: [f32; 3]| {
                            let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
                            v.map(|c| c / l)
                        };
                        let s = norm([next(), next(), next()]);
                        let t = norm([next(), next(), next()]);
                        let d = s[0] * t[0] + s[1] * t[1] + s[2] * t[2];
                        let t = norm([t[0] - d * s[0], t[1] - d * s[1], t[2] - d * s[2]]);
                        for &n in &input_normals {
                            let q = texgen_uv(&tp, n, (s, t));
                            assert!(meta.bound.contains(q), "file {id} {at:#x}: {q:?}");
                            total += 1;
                            hit += cells.contains(&[q[0].div_euclid(32), q[1].div_euclid(32)])
                                as usize;
                        }
                    }
                    assert!(hit * 100 >= total * 99, "file {id} {at:#x}: {hit}/{total}");
                    *seen.entry((id, tp.linear)).or_insert(0) += 1;
                }
            }
        }
        assert!(seen.contains_key(&(300, false)), "{seen:?}");
        assert!(seen.contains_key(&(303, false)), "{seen:?}");
        assert!(seen.contains_key(&(117, true)), "{seen:?}");
    }
}
