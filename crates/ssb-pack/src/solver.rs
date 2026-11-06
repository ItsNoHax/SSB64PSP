//! The RE-312 cross-phase fit `romtool residuals` triages on and the pack
//! applies to the listed exact-use-site overrides
//! ([`crate::residual_fixes::Kind::Direct`]), with the error measurement it
//! scores against: the exact N64 3-point reference vs the verified GE model.

use alloc::collections::BTreeSet;
use alloc::vec;
use alloc::vec::Vec;

use ssb_rom::filter_compensation::{self as fc, AlphaPolicy};
use ssb_rom::n64_filter::{
    bilinear_taps_addressed, sample_3point_addressed, sample_bilinear_addressed, sample_point,
    GeAddressMode,
};
use ssb_rom::texture::Rgba8;

// ---------------------------------------------------------------------------
// Measurement
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Default, Debug)]
pub struct Stat {
    pub n: u64,
    pub sum: u64,
    pub max: u8,
    pub sse: u64,
    pub ge8: u64,
    pub ge16: u64,
    pub ge32: u64,
}

impl Stat {
    pub fn add(&mut self, d: u8, sse: u64) {
        self.n += 1;
        self.sum += d as u64;
        self.max = self.max.max(d);
        self.sse += sse;
        self.ge8 += u64::from(d >= 8);
        self.ge16 += u64::from(d >= 16);
        self.ge32 += u64::from(d >= 32);
    }
    pub fn merge(&mut self, o: &Stat) {
        self.n += o.n;
        self.sum += o.sum;
        self.max = self.max.max(o.max);
        self.sse += o.sse;
        self.ge8 += o.ge8;
        self.ge16 += o.ge16;
        self.ge32 += o.ge32;
    }
    pub fn mean(&self) -> f64 {
        self.sum as f64 / self.n.max(1) as f64
    }
    pub fn pct(&self, c: u64) -> f64 {
        c as f64 * 100.0 / self.n.max(1) as f64
    }
    pub fn p8(&self) -> f64 {
        self.pct(self.ge8)
    }
    pub fn p16(&self) -> f64 {
        self.pct(self.ge16)
    }
    pub fn p32(&self) -> f64 {
        self.pct(self.ge32)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Worst {
    pub s: i32,
    pub t: i32,
    pub d: u8,
    pub sse: u64,
    pub reference: [u8; 4],
    pub psp: [u8; 4],
    pub state: usize,
}

#[derive(Clone, Copy, Default, Debug)]
pub struct Eval {
    /// Policy-scoped raw channels (RGB for opaque, RGBA otherwise): the
    /// pipeline's own `measure_samples_policy` definition.
    pub raw: Stat,
    pub rgb: Stat,
    pub alpha: Stat,
    /// Gameplay-visible error: RGB for opaque; for cutout, RGB where both
    /// pass the real alpha test and 255 where the pass/fail result flips;
    /// for translucent, premultiplied RGB and alpha.
    pub vis: Stat,
    /// `vis` restricted to samples whose alpha-test result agrees (colour
    /// error only); equal to `vis` for opaque and translucent.
    pub color: Stat,
    pub worst: Option<Worst>,
    pub edge_vis_sse: u64,
    pub edge_n: u64,
    pub odd_vis_sse: u64,
    pub odd_n: u64,
    pub cutout_mismatch: u64,
}

impl Eval {
    pub fn merge(&mut self, o: &Eval) {
        self.raw.merge(&o.raw);
        self.rgb.merge(&o.rgb);
        self.alpha.merge(&o.alpha);
        self.vis.merge(&o.vis);
        self.color.merge(&o.color);
        self.edge_vis_sse += o.edge_vis_sse;
        self.edge_n += o.edge_n;
        self.odd_vis_sse += o.odd_vis_sse;
        self.odd_n += o.odd_n;
        self.cutout_mismatch += o.cutout_mismatch;
        if let Some(w) = o.worst {
            if self.worst.is_none_or(|c| better_worst(&w, &c)) {
                self.worst = Some(w);
            }
        }
    }
}

pub fn better_worst(a: &Worst, b: &Worst) -> bool {
    (a.d, a.sse, core::cmp::Reverse((a.state, a.t, a.s)))
        > (b.d, b.sse, core::cmp::Reverse((b.state, b.t, b.s)))
}

pub fn modes(clamp: [bool; 2]) -> (GeAddressMode, GeAddressMode) {
    let m = |c| {
        if c {
            GeAddressMode::Clamp
        } else {
            GeAddressMode::Repeat
        }
    };
    (m(clamp[0]), m(clamp[1]))
}

pub fn passes(policy: AlphaPolicy, a: u8) -> bool {
    match policy {
        AlphaPolicy::Cutout {
            greater_or_equal,
            threshold,
        } => {
            if greater_or_equal {
                a >= threshold
            } else {
                a > threshold
            }
        }
        _ => true,
    }
}

pub fn premul(c: [u8; 4], i: usize) -> i32 {
    (c[i] as u32 * c[3] as u32 / 255) as i32
}

/// Per-sample error decomposition shared by every candidate.
pub fn sample_error(
    policy: AlphaPolicy,
    r: [u8; 4],
    p: [u8; 4],
) -> (u8, u64, u8, u64, u8, u64, u8, u64) {
    let diff = |i: usize| (r[i] as i32 - p[i] as i32).unsigned_abs();
    let rgb_d = (0..3).map(diff).max().unwrap() as u8;
    let rgb_sse: u64 = (0..3).map(|i| (diff(i) * diff(i)) as u64).sum();
    let a_d = diff(3) as u8;
    let a_sse = (diff(3) * diff(3)) as u64;
    let (raw_d, raw_sse) = if policy == AlphaPolicy::Opaque {
        (rgb_d, rgb_sse)
    } else {
        (rgb_d.max(a_d), rgb_sse + a_sse)
    };
    let (vis_d, vis_sse) = match policy {
        AlphaPolicy::Opaque => (rgb_d, rgb_sse),
        AlphaPolicy::Cutout { .. } => match (passes(policy, r[3]), passes(policy, p[3])) {
            (true, true) => (rgb_d, rgb_sse),
            (false, false) => (0, 0),
            _ => (255, 3 * 255 * 255),
        },
        AlphaPolicy::Translucent => {
            let pd = |i: usize| (premul(r, i) - premul(p, i)).unsigned_abs();
            let d = (0..3).map(pd).max().unwrap().max(diff(3)) as u8;
            let sse: u64 = (0..3).map(|i| (pd(i) * pd(i)) as u64).sum::<u64>() + a_sse;
            (d, sse)
        }
    };
    (raw_d, raw_sse, rgb_d, rgb_sse, a_d, a_sse, vis_d, vis_sse)
}

/// Measures `psp(s, t)` against the exact 3-point reference of `reference`.
pub fn evaluate(
    reference: &Rgba8,
    clamp: [bool; 2],
    samples: &[[i32; 2]],
    policy: AlphaPolicy,
    state: usize,
    psp: impl Fn(i32, i32) -> [u8; 4],
) -> Eval {
    let (ms, mt) = modes(clamp);
    let mut e = Eval::default();
    let (w, h) = (reference.width as i32, reference.height as i32);
    for &[s, t] in samples {
        let r = sample_3point_addressed(reference, s, t, ms, mt);
        let p = psp(s, t);
        let (raw_d, raw_sse, rgb_d, rgb_sse, a_d, a_sse, vis_d, vis_sse) =
            sample_error(policy, r, p);
        e.raw.add(raw_d, raw_sse);
        e.rgb.add(rgb_d, rgb_sse);
        e.alpha.add(a_d, a_sse);
        e.vis.add(vis_d, vis_sse);
        if passes(policy, r[3]) != passes(policy, p[3]) {
            e.cutout_mismatch += 1;
        } else {
            e.color.add(vis_d, vis_sse);
        }
        let (x0, y0) = (s.div_euclid(32), t.div_euclid(32));
        if x0 < 0 || x0 + 1 >= w || y0 < 0 || y0 + 1 >= h {
            e.edge_n += 1;
            e.edge_vis_sse += vis_sse;
        }
        if (s | t) & 1 != 0 {
            e.odd_n += 1;
            e.odd_vis_sse += vis_sse;
        }
        let cand = Worst {
            s,
            t,
            d: vis_d,
            sse: vis_sse,
            reference: r,
            psp: p,
            state,
        };
        if e.worst.is_none_or(|c| better_worst(&cand, &c)) {
            e.worst = Some(cand);
        }
    }
    e
}

pub fn bilinear(img: &Rgba8, clamp: [bool; 2]) -> impl Fn(i32, i32) -> [u8; 4] + '_ {
    let (ms, mt) = modes(clamp);
    move |s, t| sample_bilinear_addressed(img, s, t, ms, mt)
}

pub fn nearest(img: &Rgba8, clamp: [bool; 2]) -> impl Fn(i32, i32) -> [u8; 4] + '_ {
    let (ms, mt) = modes(clamp);
    move |s, t| sample_point(img, s, t, ms, mt)
}

pub fn full_tile(w: u32, h: u32, phase: i32) -> Vec<[i32; 2]> {
    let step = fc::SAMPLE_STEP_Q5 as usize;
    (phase..h as i32 * 32)
        .step_by(step)
        .flat_map(|t| (phase..w as i32 * 32).step_by(step).map(move |s| [s, t]))
        .collect()
}

// ---------------------------------------------------------------------------
// Report-only least-squares fit
// ---------------------------------------------------------------------------

pub struct Fit {
    pub image: Rgba8,
    pub float_sse: f64,
    pub iterations: usize,
}

pub const FIT_MAX_ITERATIONS: usize = 2000;

/// Box-constrained least squares of the GE's 4-bit-weight bilinear model
/// against the 3-point reference on `samples`, per channel. Preconditioned
/// by each texel's summed tap weight, which bounds the preconditioned
/// Hessian's spectrum by 1 (every sample's weights sum to 1), so a step of
/// 1.5 converges. Starts at `init`; channels outside `free` stay at `init`.
/// Measurement tool only: it is not a pack-time heuristic.
pub fn fit(
    reference: &Rgba8,
    init: &Rgba8,
    clamp: [bool; 2],
    samples: &[[i32; 2]],
    free: &[usize],
) -> Fit {
    let (ms, mt) = modes(clamp);
    let n = (reference.width * reference.height) as usize;
    let taps: Vec<([usize; 4], [f64; 4], [f64; 4])> = samples
        .iter()
        .map(|&[s, t]| {
            let b = bilinear_taps_addressed(reference.width, reference.height, s, t, ms, mt);
            let sf = b.sf as f64 / 16.0;
            let tf = b.tf as f64 / 16.0;
            let w = [
                (1.0 - sf) * (1.0 - tf),
                sf * (1.0 - tf),
                (1.0 - sf) * tf,
                sf * tf,
            ];
            let r = sample_3point_addressed(reference, s, t, ms, mt);
            (b.texel, w, r.map(f64::from))
        })
        .collect();
    let mut rowsum = vec![0.0f64; n];
    for (tx, w, _) in &taps {
        for k in 0..4 {
            rowsum[tx[k]] += w[k];
        }
    }
    let mut values: Vec<f64> = init.pixels.iter().map(|&v| v as f64).collect();
    let mut grad = vec![0.0f64; n * 4];
    let objective = |values: &[f64]| -> f64 {
        let mut sse = 0.0;
        for (tx, w, target) in &taps {
            for &c in free {
                let p: f64 = (0..4).map(|k| values[tx[k] * 4 + c] * w[k]).sum();
                let e = p - target[c];
                sse += e * e;
            }
        }
        sse
    };
    let mut last = objective(&values);
    let mut iterations = 0;
    let mut stalled = 0;
    while iterations < FIT_MAX_ITERATIONS {
        iterations += 1;
        grad.iter_mut().for_each(|g| *g = 0.0);
        for (tx, w, target) in &taps {
            for &c in free {
                let p: f64 = (0..4).map(|k| values[tx[k] * 4 + c] * w[k]).sum();
                let e = p - target[c];
                for k in 0..4 {
                    grad[tx[k] * 4 + c] += e * w[k];
                }
            }
        }
        for i in 0..n {
            if rowsum[i] <= 0.0 {
                continue;
            }
            for &c in free {
                values[i * 4 + c] =
                    (values[i * 4 + c] - 1.5 * grad[i * 4 + c] / rowsum[i]).clamp(0.0, 255.0);
            }
        }
        if iterations % 10 == 0 {
            let now = objective(&values);
            if last - now <= 1e-7 * last.max(1.0) {
                stalled += 1;
                if stalled >= 2 {
                    last = now;
                    break;
                }
            } else {
                stalled = 0;
            }
            last = now;
        }
    }
    let float_sse = objective(&values);
    let mut image = init.clone();
    for i in 0..n {
        for &c in free {
            image.pixels[i * 4 + c] = (values[i * 4 + c] + 0.5) as u8;
        }
    }
    let _ = last;
    Fit {
        image,
        float_sse,
        iterations,
    }
}

pub fn free_channels(policy: AlphaPolicy) -> &'static [usize] {
    if policy == AlphaPolicy::Opaque {
        &[0, 1, 2]
    } else {
        &[0, 1, 2, 3]
    }
}

/// Integer refinement with the pipeline's own GE-exact search.
pub fn refine(
    reference: &Rgba8,
    seed: &Rgba8,
    clamp: [bool; 2],
    samples: &[[i32; 2]],
    policy: AlphaPolicy,
) -> Rgba8 {
    fc::refine_integer(
        reference,
        seed,
        fc::IntegerFormat::Rgba8888,
        clamp[0],
        clamp[1],
        samples,
        policy,
    )
}

/// Cutout silhouette bound: coordinate search over each texel's alpha in
/// {0, threshold, threshold+1, 255, source} minimizing the number of holdout
/// samples whose alpha-test result differs from the 3-point reference, then
/// visible SSE. RGB is unchanged. Only samples whose bilinear footprint
/// includes the texel are recomputed. Report-only.
pub fn silhouette_search(
    reference: &Rgba8,
    mut img: Rgba8,
    clamp: [bool; 2],
    samples: &[[i32; 2]],
    policy: AlphaPolicy,
) -> Rgba8 {
    let AlphaPolicy::Cutout { threshold, .. } = policy else {
        return img;
    };
    let (ms, mt) = modes(clamp);
    let n = (img.width * img.height) as usize;
    let taps: Vec<_> = samples
        .iter()
        .map(|&[s, t]| bilinear_taps_addressed(img.width, img.height, s, t, ms, mt))
        .collect();
    let refs: Vec<[u8; 4]> = samples
        .iter()
        .map(|&[s, t]| sample_3point_addressed(reference, s, t, ms, mt))
        .collect();
    let mut touching: Vec<Vec<u32>> = vec![Vec::new(); n];
    for (k, b) in taps.iter().enumerate() {
        let mut seen = b.texel;
        seen.sort_unstable();
        for (j, &tx) in seen.iter().enumerate() {
            if j == 0 || seen[j - 1] != tx {
                touching[tx].push(k as u32);
            }
        }
    }
    let cost = |img: &Rgba8, list: &[u32]| -> (u64, u64) {
        let mut flips = 0;
        let mut sse = 0;
        for &k in list {
            let b = &taps[k as usize];
            let colors = b.texel.map(|i| img.get(i).map(i32::from));
            let p = ssb_rom::n64_filter::compose_bilinear(colors, b.sf, b.tf);
            let r = refs[k as usize];
            flips += u64::from(passes(policy, r[3]) != passes(policy, p[3]));
            sse += sample_error(policy, r, p).7;
        }
        (flips, sse)
    };
    for _ in 0..4 {
        let mut changed = false;
        for (i, near) in touching.iter().enumerate() {
            if near.is_empty() {
                continue;
            }
            let original = img.pixels[i * 4 + 3];
            let mut best = (cost(&img, near), original);
            for cand in [
                0,
                threshold,
                threshold.saturating_add(1),
                255,
                reference.pixels[i * 4 + 3],
            ] {
                if cand == original {
                    continue;
                }
                img.pixels[i * 4 + 3] = cand;
                let c = cost(&img, near);
                if c < best.0 {
                    best = (c, cand);
                }
            }
            img.pixels[i * 4 + 3] = best.1;
            changed |= best.1 != original;
        }
        if !changed {
            break;
        }
    }
    img
}

/// Phase-uniform set U1 over every texel cell `eval` touches: 16 samples
/// per cell whose sub-texel phases cycle through all 32 residues across
/// cells. U2 is the same set shifted by (4, 4)/32 texel.
pub fn phase_uniform_set(eval: &[[i32; 2]]) -> Vec<[i32; 2]> {
    let cells: BTreeSet<(i32, i32)> = eval
        .iter()
        .map(|&[s, t]| (t.div_euclid(32), s.div_euclid(32)))
        .collect();
    let mut v = Vec::with_capacity(cells.len() * 16);
    for (ty, tx) in cells {
        let o = (tx * 5 + ty * 3).rem_euclid(8);
        for j in 0..4 {
            for i in 0..4 {
                v.push([tx * 32 + 1 + 8 * i + o, ty * 32 + 1 + 8 * j + (o * 3) % 8]);
            }
        }
    }
    v
}

/// The cross-phase fits section 11 triages on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CrossFit {
    /// RGBA8888, alpha free unless opaque.
    Free,
    /// RGB fitted, alpha held at the source.
    AlphaHeld,
    /// `AlphaHeld` plus the flip-minimizing cutout alpha search.
    Silhouette,
}

/// Same-resolution RGBA8888 fit on U1 with GE-exact integer refinement:
/// exactly the image the report's cross-phase bound scores on U2. RE-312
/// also calls it at pack time for the listed exact-use-site overrides.
pub fn cross_phase_fit(
    reference: &Rgba8,
    clamp: [bool; 2],
    policy: AlphaPolicy,
    u1: &[[i32; 2]],
    mode: CrossFit,
) -> Rgba8 {
    match mode {
        CrossFit::Free => {
            let refine_policy = if policy == AlphaPolicy::Opaque {
                AlphaPolicy::Opaque
            } else {
                AlphaPolicy::Translucent
            };
            let o = fit(reference, reference, clamp, u1, free_channels(policy));
            refine(reference, &o.image, clamp, u1, refine_policy)
        }
        CrossFit::AlphaHeld => {
            let a = fit(reference, reference, clamp, u1, &[0, 1, 2]);
            refine(reference, &a.image, clamp, u1, policy)
        }
        CrossFit::Silhouette => {
            let held = cross_phase_fit(reference, clamp, policy, u1, CrossFit::AlphaHeld);
            silhouette_search(reference, held, clamp, u1, policy)
        }
    }
}
