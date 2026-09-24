//! Report-only residual census for the N64 3-point / PSP bilinear gap.
//!
//! `romtool residuals` rebuilds the pack with a recorder switched on. The
//! recorder captures, for every packed mesh texture variant, the decoded N64
//! source texels, the texels the final variant actually packs, and every
//! primitive that binds the variant together with that primitive's real
//! filter coverage. The pack bytes are not influenced: every hook only clones
//! values the converter already computed, and the command checks the rebuilt
//! pack against the committed build's hash.
//!
//! The analysis then measures, on the *independent* holdout coverage (never
//! the optimizer's training samples), the exact N64 3-point reference against
//! the verified GE model for the current variant and for a fixed set of
//! alternatives. It does not select or apply any of them.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::sync::atomic::{AtomicUsize, Ordering};

use ssb_rom::filter_compensation::{self as fc, AlphaPolicy};
use ssb_rom::n64_filter::{
    bilinear_taps_addressed, sample_3point_addressed, sample_bilinear_addressed, sample_point,
    GeAddressMode,
};
use ssb_rom::psp_texture::{self as psp, Psm, PspTexture};
use ssb_rom::texture::Rgba8;

use crate::filter_coverage::Coverage;

// ---------------------------------------------------------------------------
// Recorder
// ---------------------------------------------------------------------------

/// A material-animation index field that one variant shares across palettes.
pub(super) struct Animated {
    pub source_indices: Vec<u8>,
    pub final_indices: Vec<u8>,
    pub states: Vec<Vec<u32>>,
}

/// What one `convert_texture` call packed.
pub(super) struct Conversion {
    pub texture: ssb_rom::mesh::TextureRef,
    /// File the display list came from; resolves `texture.data_file == None`.
    pub home: u32,
    /// Decoded, mirror-extended N64 texels: the 3-point reference input.
    pub source: Rgba8,
    /// Level-0 texels of the packed variant (static palette for indexed).
    pub final_level0: Rgba8,
    pub source_psm: Psm,
    pub final_psm: Psm,
    pub palette: Vec<u32>,
    pub animated: Option<Animated>,
    pub method: &'static str,
    /// Whether the compensator was run for this conversion at all.
    pub attempted: bool,
    pub policy: AlphaPolicy,
}

pub(super) struct Variant {
    pub conversion: Conversion,
    pub packed: PspTexture,
    pub clamp: [bool; 2],
}

/// One primitive that binds a packed variant.
pub(super) struct UseSite {
    pub variant: u32,
    pub file: u32,
    pub dl: u32,
    pub prim: usize,
    pub sprite_slot: Option<usize>,
    pub policy: AlphaPolicy,
    /// `Some(linear)` for `G_TEXTURE_GEN`.
    pub texgen: Option<bool>,
    pub mat_anim: bool,
    pub triangles: usize,
    pub coverage: Coverage,
}

#[derive(Default)]
pub(super) struct Recorder {
    pending: Option<Conversion>,
    variants: BTreeMap<u32, Variant>,
    sites: Vec<UseSite>,
}

thread_local! {
    static STATE: RefCell<Option<Recorder>> = const { RefCell::new(None) };
}

pub(super) fn enable() {
    STATE.with(|s| *s.borrow_mut() = Some(Recorder::default()));
}

pub(super) fn enabled() -> bool {
    STATE.with(|s| s.borrow().is_some())
}

fn take() -> Option<Recorder> {
    STATE.with(|s| s.borrow_mut().take())
}

pub(super) fn record_conversion(c: Conversion) {
    STATE.with(|s| {
        if let Some(r) = s.borrow_mut().as_mut() {
            r.pending = Some(c);
        }
    });
}

/// The pending conversion's method, whether the compensator ran, and its
/// natural and final formats; clears it. Tests only.
#[cfg(test)]
pub(super) fn take_pending() -> Option<(&'static str, bool, Psm, Psm)> {
    STATE.with(|s| {
        s.borrow_mut()
            .as_mut()
            .and_then(|r| r.pending.take())
            .map(|c| (c.method, c.attempted, c.source_psm, c.final_psm))
    })
}

/// The conversion was deduplicated into an existing variant, or failed.
pub(super) fn discard_conversion() {
    STATE.with(|s| {
        if let Some(r) = s.borrow_mut().as_mut() {
            r.pending = None;
        }
    });
}

pub(super) fn record_variant(index: u32, packed: &PspTexture, clamp_s: bool, clamp_t: bool) {
    STATE.with(|s| {
        if let Some(r) = s.borrow_mut().as_mut() {
            if let Some(conversion) = r.pending.take() {
                r.variants.insert(
                    index,
                    Variant {
                        conversion,
                        packed: packed.clone(),
                        clamp: [clamp_s, clamp_t],
                    },
                );
            }
        }
    });
}

pub(super) fn record_site(site: UseSite) {
    STATE.with(|s| {
        if let Some(r) = s.borrow_mut().as_mut() {
            r.sites.push(site);
        }
    });
}

// ---------------------------------------------------------------------------
// Measurement
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Default, Debug)]
struct Stat {
    n: u64,
    sum: u64,
    max: u8,
    sse: u64,
    ge8: u64,
    ge16: u64,
    ge32: u64,
}

impl Stat {
    fn add(&mut self, d: u8, sse: u64) {
        self.n += 1;
        self.sum += d as u64;
        self.max = self.max.max(d);
        self.sse += sse;
        self.ge8 += u64::from(d >= 8);
        self.ge16 += u64::from(d >= 16);
        self.ge32 += u64::from(d >= 32);
    }
    fn merge(&mut self, o: &Stat) {
        self.n += o.n;
        self.sum += o.sum;
        self.max = self.max.max(o.max);
        self.sse += o.sse;
        self.ge8 += o.ge8;
        self.ge16 += o.ge16;
        self.ge32 += o.ge32;
    }
    fn mean(&self) -> f64 {
        self.sum as f64 / self.n.max(1) as f64
    }
    fn pct(&self, c: u64) -> f64 {
        c as f64 * 100.0 / self.n.max(1) as f64
    }
    fn p8(&self) -> f64 {
        self.pct(self.ge8)
    }
    fn p16(&self) -> f64 {
        self.pct(self.ge16)
    }
    fn p32(&self) -> f64 {
        self.pct(self.ge32)
    }
}

#[derive(Clone, Copy, Debug)]
struct Worst {
    s: i32,
    t: i32,
    d: u8,
    sse: u64,
    reference: [u8; 4],
    psp: [u8; 4],
    state: usize,
}

#[derive(Clone, Copy, Default, Debug)]
struct Eval {
    /// Policy-scoped raw channels (RGB for opaque, RGBA otherwise): the
    /// pipeline's own `measure_samples_policy` definition.
    raw: Stat,
    rgb: Stat,
    alpha: Stat,
    /// Gameplay-visible error: RGB for opaque; for cutout, RGB where both
    /// pass the real alpha test and 255 where the pass/fail result flips;
    /// for translucent, premultiplied RGB and alpha.
    vis: Stat,
    /// `vis` restricted to samples whose alpha-test result agrees (colour
    /// error only); equal to `vis` for opaque and translucent.
    color: Stat,
    worst: Option<Worst>,
    edge_vis_sse: u64,
    edge_n: u64,
    odd_vis_sse: u64,
    odd_n: u64,
    cutout_mismatch: u64,
}

impl Eval {
    fn merge(&mut self, o: &Eval) {
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

fn better_worst(a: &Worst, b: &Worst) -> bool {
    (a.d, a.sse, std::cmp::Reverse((a.state, a.t, a.s)))
        > (b.d, b.sse, std::cmp::Reverse((b.state, b.t, b.s)))
}

fn modes(clamp: [bool; 2]) -> (GeAddressMode, GeAddressMode) {
    let m = |c| {
        if c {
            GeAddressMode::Clamp
        } else {
            GeAddressMode::Repeat
        }
    };
    (m(clamp[0]), m(clamp[1]))
}

fn passes(policy: AlphaPolicy, a: u8) -> bool {
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

fn premul(c: [u8; 4], i: usize) -> i32 {
    (c[i] as u32 * c[3] as u32 / 255) as i32
}

/// Per-sample error decomposition shared by every candidate.
fn sample_error(
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
fn evaluate(
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

fn bilinear(img: &Rgba8, clamp: [bool; 2]) -> impl Fn(i32, i32) -> [u8; 4] + '_ {
    let (ms, mt) = modes(clamp);
    move |s, t| sample_bilinear_addressed(img, s, t, ms, mt)
}

fn nearest(img: &Rgba8, clamp: [bool; 2]) -> impl Fn(i32, i32) -> [u8; 4] + '_ {
    let (ms, mt) = modes(clamp);
    move |s, t| sample_point(img, s, t, ms, mt)
}

fn full_tile(w: u32, h: u32, phase: i32) -> Vec<[i32; 2]> {
    let step = fc::SAMPLE_STEP_Q5 as usize;
    (phase..h as i32 * 32)
        .step_by(step)
        .flat_map(|t| (phase..w as i32 * 32).step_by(step).map(move |s| [s, t]))
        .collect()
}

// ---------------------------------------------------------------------------
// Report-only least-squares fit
// ---------------------------------------------------------------------------

struct Fit {
    image: Rgba8,
    float_sse: f64,
    iterations: usize,
}

const FIT_MAX_ITERATIONS: usize = 2000;

/// Box-constrained least squares of the GE's 4-bit-weight bilinear model
/// against the 3-point reference on `samples`, per channel. Preconditioned
/// by each texel's summed tap weight, which bounds the preconditioned
/// Hessian's spectrum by 1 (every sample's weights sum to 1), so a step of
/// 1.5 converges. Starts at `init`; channels outside `free` stay at `init`.
/// Measurement tool only: it is not a pack-time heuristic.
fn fit(
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

fn free_channels(policy: AlphaPolicy) -> &'static [usize] {
    if policy == AlphaPolicy::Opaque {
        &[0, 1, 2]
    } else {
        &[0, 1, 2, 3]
    }
}

/// Integer refinement with the pipeline's own GE-exact search.
fn refine(
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
fn silhouette_search(
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
        for i in 0..n {
            if touching[i].is_empty() {
                continue;
            }
            let original = img.pixels[i * 4 + 3];
            let mut best = (cost(&img, &touching[i]), original);
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
                let c = cost(&img, &touching[i]);
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

// ---------------------------------------------------------------------------
// Pack decode and GE padded-addressing check
// ---------------------------------------------------------------------------

/// Level 0 of a packed variant, including padding, as the GE addresses it.
fn decode_packed(t: &PspTexture, palette: &[u32]) -> Option<(Rgba8, Vec<u8>)> {
    let stride = t.stride as usize;
    let ph = psp::pad_to_power_of_two(t.height) as usize;
    let bits = t.format.bits();
    let stride_bytes = (stride * bits).div_ceil(8);
    let raw = t.data.get(..stride_bytes * ph)?;
    let bytes = if t.swizzled {
        psp::unswizzle(raw, stride_bytes, ph)
    } else {
        raw.to_vec()
    };
    let mut img = Rgba8::new(stride as u32, ph as u32);
    let mut indices = Vec::new();
    let abgr = |v: u32| [v as u8, (v >> 8) as u8, (v >> 16) as u8, (v >> 24) as u8];
    for y in 0..ph {
        for x in 0..stride {
            let px = match t.format {
                Psm::Psm8888 => {
                    let o = y * stride_bytes + x * 4;
                    abgr(u32::from_le_bytes(bytes[o..o + 4].try_into().unwrap()))
                }
                Psm::Psm5551 => {
                    let o = y * stride_bytes + x * 2;
                    let w = u16::from_le_bytes(bytes[o..o + 2].try_into().unwrap());
                    let e = |v: u16| ((v << 3) | (v >> 2)) as u8;
                    [
                        e(w & 0x1f),
                        e((w >> 5) & 0x1f),
                        e((w >> 10) & 0x1f),
                        if w & 0x8000 != 0 { 255 } else { 0 },
                    ]
                }
                Psm::PsmT4 | Psm::PsmT8 => {
                    let i = if t.format == Psm::PsmT4 {
                        let b = bytes[y * stride_bytes + x / 2];
                        if x & 1 == 0 {
                            b & 0x0f
                        } else {
                            b >> 4
                        }
                    } else {
                        bytes[y * stride_bytes + x]
                    };
                    indices.push(i);
                    abgr(*palette.get(i as usize).unwrap_or(&0))
                }
                _ => return None,
            };
            img.put(y * stride + x, px);
        }
    }
    Some((img, indices))
}

fn crop(img: &Rgba8, w: u32, h: u32) -> Rgba8 {
    let mut out = Rgba8::new(w, h);
    for y in 0..h {
        for x in 0..w {
            out.put((y * w + x) as usize, img.get((y * img.width + x) as usize));
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Per-variant analysis
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum CoverageKind {
    Authored,
    TexgenReal,
    TexgenFullTile,
    FullTile,
}

impl CoverageKind {
    fn name(self) -> &'static str {
        match self {
            CoverageKind::Authored => "authored-uv",
            CoverageKind::TexgenReal => "texgen-real-normal",
            CoverageKind::TexgenFullTile => "texgen-full-tile-fallback",
            CoverageKind::FullTile => "full-tile",
        }
    }
}

/// One distinct (coverage) use of a variant, with the primitives that share it.
struct SiteGroup {
    kind: CoverageKind,
    texgen_linear: Option<bool>,
    policy: AlphaPolicy,
    prims: Vec<(u32, u32, usize, Option<usize>)>,
    mat_anim: bool,
    triangles: usize,
    train: Vec<[i32; 2]>,
    eval: Vec<[i32; 2]>,
}

fn site_sets(site: &UseSite, w: u32, h: u32) -> (CoverageKind, Vec<[i32; 2]>, Vec<[i32; 2]>) {
    let c = &site.coverage;
    match (&c.texgen, site.texgen) {
        (Some(meta), _) if !meta.legacy => {
            let mut train = c.train.clone();
            train.extend(meta.bound.regularization());
            let mut eval = c.validation.clone();
            eval.extend(meta.bound.lattice(4));
            (CoverageKind::TexgenReal, sorted(train), sorted(eval))
        }
        _ if c.train.is_empty() => {
            let kind = if site.texgen.is_some() {
                CoverageKind::TexgenFullTile
            } else {
                CoverageKind::FullTile
            };
            (kind, full_tile(w, h, 0), full_tile(w, h, 4))
        }
        _ => {
            let eval = if c.validation.is_empty() {
                full_tile(w, h, 4)
            } else {
                c.validation.clone()
            };
            (
                CoverageKind::Authored,
                sorted(c.train.clone()),
                sorted(eval),
            )
        }
    }
}

fn sorted(mut v: Vec<[i32; 2]>) -> Vec<[i32; 2]> {
    v.sort_unstable_by_key(|p| (p[1], p[0]));
    v.dedup();
    v
}

#[derive(Clone, Copy, Debug)]
struct Candidate {
    eval: Eval,
    /// Level-0 bytes (texels + CLUT) this candidate would occupy.
    level0_bytes: u64,
    /// Whole pack entry bytes with the pipeline's mip chain.
    pack_bytes: u64,
    float_sse: Option<f64>,
    iterations: Option<usize>,
}

struct Analysis {
    index: u32,
    width: u32,
    height: u32,
    states: usize,
    groups: Vec<SiteGroup>,
    eval_samples: usize,
    train_samples: usize,
    current: Candidate,
    uncompensated: Candidate,
    nearest: Candidate,
    rgba_practical: Candidate,
    rgba_oracle: Candidate,
    /// Oracle with the variant's real alpha constraint: alpha held exactly
    /// (for every non-opaque policy) -- isolates what alpha freedom buys.
    rgba_alpha_held_oracle: Option<Candidate>,
    /// Cutout only: alpha-held oracle colours plus a flip-minimizing alpha
    /// search on the holdout (bounds what any alpha edit can do for the
    /// silhouette).
    rgba_silhouette_oracle: Option<Candidate>,
    indexed: Option<Candidate>,
    /// Per-palette-state direct colour (animated only), practical.
    per_state_direct: Option<Candidate>,
    /// Per-use-site oracles, summed over the distinct sites, and the union
    /// oracle measured on the same per-site sets.
    per_site_oracle_vis_sse: Option<(u64, u64)>,
    per_site_current: Vec<Eval>,
    uv_phase: Option<([i32; 2], Eval)>,
    /// Phase-uniform set over every holdout cell: current, uncompensated,
    /// and the holdout's best UV shift.
    uniform: (Eval, Eval, Option<Eval>),
    /// Texel cells touched by training or holdout coverage.
    cells: usize,
    /// Practical candidate trained on the training set plus 4 phase-varied
    /// samples in every touched cell (still disjoint from the holdout
    /// points); measured on the holdout.
    rgba_dense: Candidate,
    pack_level0_match: bool,
    /// Texels whose packed level-0 value differs from the recorded final
    /// texels, and the first one: (x, y, recorded, packed).
    level0_diff: (u64, Option<(u32, u32, [u8; 4], [u8; 4])>),
    padded_mismatch: u64,
    current_level0_bytes: u64,
    current_pack_bytes: u64,
    practical_image: Rgba8,
}

impl Analysis {
    /// The oracle with the lowest colour SSE (ties: visible SSE, then order).
    fn best_oracle(&self) -> &Candidate {
        [
            Some(&self.rgba_oracle),
            self.rgba_alpha_held_oracle.as_ref(),
            self.rgba_silhouette_oracle.as_ref(),
        ]
        .into_iter()
        .flatten()
        .min_by_key(|c| (c.eval.color.sse, c.eval.vis.sse))
        .unwrap()
    }
}

fn level0_bytes(w: u32, h: u32, psm: Psm, palette: usize) -> u64 {
    let stride = psp::pad_to_power_of_two(w) as u64;
    let ph = psp::pad_to_power_of_two(h) as u64;
    (stride * psm.bits() as u64).div_ceil(8) * ph + palette as u64 * 4
}

fn direct_pack_bytes(img: &Rgba8) -> u64 {
    psp::pack_mipped(img, Psm::Psm8888, &[], true).data.len() as u64
}

fn candidate(eval: Eval, level0: u64, pack: u64) -> Candidate {
    Candidate {
        eval,
        level0_bytes: level0,
        pack_bytes: pack,
        float_sse: None,
        iterations: None,
    }
}

/// State-expanded reference and current images.
fn state_images(v: &Variant) -> Vec<(Rgba8, Rgba8)> {
    let c = &v.conversion;
    match &c.animated {
        Some(a) => a
            .states
            .iter()
            .map(|p| {
                (
                    fc::indices_to_rgba(c.source.width, c.source.height, &a.source_indices, p),
                    fc::indices_to_rgba(c.source.width, c.source.height, &a.final_indices, p),
                )
            })
            .collect(),
        None => vec![(c.source.clone(), c.final_level0.clone())],
    }
}

fn eval_states(
    images: &[(Rgba8, Rgba8)],
    clamp: [bool; 2],
    samples: &[[i32; 2]],
    policy: AlphaPolicy,
    pick: impl Fn(usize, &Rgba8, &Rgba8) -> Rgba8,
    sampler: fn(&Rgba8, [bool; 2], i32, i32) -> [u8; 4],
) -> Eval {
    let mut e = Eval::default();
    for (k, (r, cur)) in images.iter().enumerate() {
        let img = pick(k, r, cur);
        e.merge(&evaluate(r, clamp, samples, policy, k, |s, t| {
            sampler(&img, clamp, s, t)
        }));
    }
    e
}

fn sample_linear(img: &Rgba8, clamp: [bool; 2], s: i32, t: i32) -> [u8; 4] {
    bilinear(img, clamp)(s, t)
}

fn sample_nearest(img: &Rgba8, clamp: [bool; 2], s: i32, t: i32) -> [u8; 4] {
    nearest(img, clamp)(s, t)
}

fn analyze(index: u32, v: &Variant, sites: &[&UseSite], refine_integer: bool) -> Analysis {
    let c = &v.conversion;
    let (w, h) = (c.source.width, c.source.height);
    let clamp = v.clamp;
    let policy = c.policy;

    // Distinct coverage groups; identical coverage from several primitives
    // (costumes, repeated lists) is measured once.
    let mut groups: BTreeMap<(Vec<[i32; 2]>, Vec<[i32; 2]>, AlphaPolicy), SiteGroup> =
        BTreeMap::new();
    for site in sites {
        let (kind, train, eval) = site_sets(site, w, h);
        let g = groups
            .entry((train.clone(), eval.clone(), site.policy))
            .or_insert_with(|| SiteGroup {
                kind,
                texgen_linear: site.texgen,
                policy: site.policy,
                prims: Vec::new(),
                mat_anim: false,
                triangles: 0,
                train,
                eval,
            });
        g.prims
            .push((site.file, site.dl, site.prim, site.sprite_slot));
        g.mat_anim |= site.mat_anim;
        g.triangles += site.triangles;
    }
    let mut groups: Vec<SiteGroup> = groups.into_values().collect();
    groups.sort_by(|a, b| a.prims.cmp(&b.prims));
    let train = sorted(
        groups
            .iter()
            .flat_map(|g| g.train.iter().copied())
            .collect(),
    );
    let eval = sorted(groups.iter().flat_map(|g| g.eval.iter().copied()).collect());

    let images = state_images(v);
    let states = images.len();

    // Final pack truth: level 0 decoded from the packed bytes must be the
    // recorded final texels, and GE addressing at the padded size must agree
    // with the logical-size model the compensator optimizes.
    let mut level0_diff = (0u64, None);
    let (pack_level0_match, padded_mismatch) = match decode_packed(&v.packed, &v.packed.palette) {
        Some((padded, indices)) => {
            let logical = crop(&padded, w, h);
            if c.animated.is_none() {
                for i in 0..(w * h) as usize {
                    let (a, b) = (c.final_level0.get(i), logical.get(i));
                    if a != b {
                        level0_diff.0 += 1;
                        if level0_diff.1.is_none() {
                            level0_diff.1 = Some((i as u32 % w, i as u32 / w, a, b));
                        }
                    }
                }
            }
            let matched = match &c.animated {
                Some(a) => {
                    let stride = v.packed.stride as usize;
                    (0..h as usize).all(|y| {
                        (0..w as usize)
                            .all(|x| indices[y * stride + x] == a.final_indices[y * w as usize + x])
                    })
                }
                None => logical.pixels == c.final_level0.pixels,
            };
            let (ms, mt) = modes(clamp);
            let model = &logical;
            let mismatch = eval
                .iter()
                .filter(|&&[s, t]| {
                    sample_bilinear_addressed(model, s, t, ms, mt)
                        != sample_bilinear_addressed(&padded, s, t, ms, mt)
                })
                .count() as u64;
            (matched, mismatch)
        }
        None => (false, u64::MAX),
    };

    let cur_level0 = level0_bytes(w, h, v.packed.format, v.packed.palette.len());
    let cur_pack = v.packed.data.len() as u64 + v.packed.palette.len() as u64 * 4;
    let src_level0 = level0_bytes(
        w,
        h,
        c.source_psm,
        if c.source_psm.is_paletted() {
            c.palette.len()
        } else {
            0
        },
    );
    // Natural-format cost of the unmodified source (what an uncompensated
    // or nearest-filtered variant would pack).
    let src_pack = if c.source_psm.is_paletted() && !c.palette.is_empty() {
        psp::pack_mipped(&c.source, c.source_psm, &c.palette, true)
            .data
            .len() as u64
            + c.palette.len() as u64 * 4
    } else if !c.source_psm.is_paletted() {
        psp::pack_mipped(&c.source, c.source_psm, &[], true)
            .data
            .len() as u64
    } else {
        direct_pack_bytes(&c.source)
    };
    let direct_level0 = level0_bytes(w, h, Psm::Psm8888, 0);

    let current = candidate(
        eval_states(
            &images,
            clamp,
            &eval,
            policy,
            |_, _, cur| cur.clone(),
            sample_linear,
        ),
        cur_level0,
        cur_pack,
    );
    let uncompensated = candidate(
        eval_states(
            &images,
            clamp,
            &eval,
            policy,
            |_, r, _| r.clone(),
            sample_linear,
        ),
        src_level0,
        src_pack,
    );
    let nearest_c = candidate(
        eval_states(
            &images,
            clamp,
            &eval,
            policy,
            |_, r, _| r.clone(),
            sample_nearest,
        ),
        src_level0,
        src_pack,
    );

    // Unconstrained same-resolution RGBA8888: practical (fit on training
    // coverage, measured on the holdout) and oracle (fit on the holdout
    // itself: the representability bound over the exercised samples).
    let free = free_channels(policy);
    let refine_policy = if policy == AlphaPolicy::Opaque {
        AlphaPolicy::Opaque
    } else {
        AlphaPolicy::Translucent
    };
    let mut practical_eval = Eval::default();
    let mut oracle_eval = Eval::default();
    let mut practical_pack = 0u64;
    let mut oracle_float = 0.0;
    let mut oracle_iter = 0usize;
    let mut practical_image = c.source.clone();
    let mut seed_image = c.source.clone();
    let mut oracle_fits: Vec<Rgba8> = Vec::new();
    let solvable = w >= 2 && h >= 2 && !train.is_empty();
    for (k, (r, _)) in images.iter().enumerate() {
        // Practical: the pipeline's own 48-iteration continuous solve
        // (ungated) and its GE-exact integer refinement, under the variant's
        // real alpha policy -- what the packer would ship if the
        // admission/holdout/promotion gates and the palette were removed.
        let seed = if solvable {
            fc::solve_samples_ungated(r, clamp[0], clamp[1], &train, policy)
        } else {
            r.clone()
        };
        let p_img = if refine_integer && solvable {
            refine(r, &seed, clamp, &train, policy)
        } else {
            seed.clone()
        };
        practical_eval.merge(&evaluate(
            r,
            clamp,
            &eval,
            policy,
            k,
            bilinear(&p_img, clamp),
        ));
        practical_pack += direct_pack_bytes(&p_img);
        // Oracle: converged fit on the holdout samples themselves.
        let o = fit(r, r, clamp, &eval, free);
        oracle_fits.push(o.image.clone());
        let o_img = if refine_integer {
            refine(r, &o.image, clamp, &eval, refine_policy)
        } else {
            o.image
        };
        oracle_eval.merge(&evaluate(
            r,
            clamp,
            &eval,
            policy,
            k,
            bilinear(&o_img, clamp),
        ));
        oracle_float += o.float_sse;
        oracle_iter = oracle_iter.max(o.iterations);
        if k == 0 {
            practical_image = p_img;
            seed_image = seed;
        }
    }
    let direct_pack_one = direct_pack_bytes(&practical_image);
    let rgba_practical = Candidate {
        eval: practical_eval,
        level0_bytes: direct_level0 * states as u64,
        pack_bytes: practical_pack,
        float_sse: None,
        iterations: None,
    };
    let rgba_oracle = Candidate {
        eval: oracle_eval,
        level0_bytes: direct_level0 * states as u64,
        pack_bytes: direct_pack_one * states as u64,
        float_sse: Some(oracle_float),
        iterations: Some(oracle_iter),
    };

    // Alpha-constrained oracles (non-opaque only): alpha held exactly, and
    // for cutout the alpha-held colours plus a flip-minimizing alpha search.
    let (rgba_alpha_held_oracle, rgba_silhouette_oracle) = if policy == AlphaPolicy::Opaque {
        (None, None)
    } else {
        let mut held = Eval::default();
        let mut sil = Eval::default();
        let mut held_float = 0.0;
        for (k, (r, _)) in images.iter().enumerate() {
            let a = fit(r, r, clamp, &eval, &[0, 1, 2]);
            let a_img = if refine_integer {
                refine(r, &a.image, clamp, &eval, policy)
            } else {
                a.image
            };
            held.merge(&evaluate(
                r,
                clamp,
                &eval,
                policy,
                k,
                bilinear(&a_img, clamp),
            ));
            held_float += a.float_sse;
            if matches!(policy, AlphaPolicy::Cutout { .. }) {
                let s_img = silhouette_search(r, a_img, clamp, &eval, policy);
                sil.merge(&evaluate(
                    r,
                    clamp,
                    &eval,
                    policy,
                    k,
                    bilinear(&s_img, clamp),
                ));
            }
        }
        let held_c = Candidate {
            eval: held,
            level0_bytes: direct_level0 * states as u64,
            pack_bytes: direct_pack_one * states as u64,
            float_sse: Some(held_float),
            iterations: None,
        };
        let sil_c = matches!(policy, AlphaPolicy::Cutout { .. }).then(|| Candidate {
            eval: sil,
            level0_bytes: direct_level0 * states as u64,
            pack_bytes: direct_pack_one * states as u64,
            float_sse: None,
            iterations: None,
        });
        (Some(held_c), sil_c)
    };

    // Filter-aware indexed candidate on the pipeline's own optimizers,
    // trained on the training coverage and measured on the holdout.
    let max_index = if c.source_psm == Psm::PsmT4 {
        16
    } else {
        c.palette.len()
    };
    let indexed = if !c.source_psm.is_paletted() || c.palette.is_empty() {
        None
    } else if let Some(a) = &c.animated {
        let limit = max_index.min(a.states.iter().map(Vec::len).min().unwrap_or(0));
        (limit > 0 && a.source_indices.iter().all(|&i| (i as usize) < limit)).then(|| {
            let opt = fc::optimize_animated_indices(
                &a.source_indices,
                w,
                h,
                &a.states,
                limit,
                clamp[0],
                clamp[1],
                &train,
            );
            let mut e = Eval::default();
            for (k, (r, _)) in images.iter().enumerate() {
                let img = fc::indices_to_rgba(w, h, &opt, &a.states[k]);
                e.merge(&evaluate(r, clamp, &eval, policy, k, bilinear(&img, clamp)));
            }
            candidate(e, src_level0, src_pack)
        })
    } else {
        let seed = seed_image.clone();
        let opt = fc::optimize_palette_indices(
            &c.source, &seed, &c.palette, max_index, clamp[0], clamp[1], &train,
        );
        let nearest_q = fc::quantize_to_palette(&seed, &c.palette);
        let e_opt = evaluate(&c.source, clamp, &eval, policy, 0, bilinear(&opt, clamp));
        let e_near = evaluate(
            &c.source,
            clamp,
            &eval,
            policy,
            0,
            bilinear(&nearest_q, clamp),
        );
        let best = if e_opt.vis.sse <= e_near.vis.sse {
            e_opt
        } else {
            e_near
        };
        Some(candidate(best, src_level0, src_pack))
    };
    let per_state_direct = c.animated.as_ref().map(|_| rgba_practical);

    // Use-site-specific oracles.
    let per_site_current: Vec<Eval> = groups
        .iter()
        .map(|g| {
            eval_states(
                &images,
                clamp,
                &g.eval,
                g.policy,
                |_, _, cur| cur.clone(),
                sample_linear,
            )
        })
        .collect();
    let per_site_oracle_vis_sse = (groups.len() >= 2).then(|| {
        let mut own = 0u64;
        let mut shared = 0u64;
        for g in &groups {
            for (k, (r, _)) in images.iter().enumerate() {
                let f = fit(r, r, clamp, &g.eval, free);
                own += evaluate(r, clamp, &g.eval, policy, k, bilinear(&f.image, clamp))
                    .vis
                    .sse;
                let union_img = &oracle_fits[k];
                shared += evaluate(r, clamp, &g.eval, policy, k, bilinear(union_img, clamp))
                    .vis
                    .sse;
            }
        }
        (own, shared)
    });

    // Uniform UV phase shift of the current texture, +/-2/32 texel per axis.
    let uv_phase = if current.eval.vis.ge8 > 0 {
        let mut best: Option<([i32; 2], Eval)> = None;
        for dt in -2..=2 {
            for ds in -2..=2 {
                if ds == 0 && dt == 0 {
                    continue;
                }
                let mut e = Eval::default();
                for (k, (r, cur)) in images.iter().enumerate() {
                    let f = bilinear(cur, clamp);
                    e.merge(&evaluate(r, clamp, &eval, policy, k, |s, t| {
                        f(s + ds, t + dt)
                    }));
                }
                if best.as_ref().is_none_or(|b| e.vis.sse < b.1.vis.sse) {
                    best = Some(([ds, dt], e));
                }
            }
        }
        best
    } else {
        None
    };

    // Phase-uniform check: 16 samples per touched cell whose sub-texel
    // phases cycle through all 32 residues across cells, free of the
    // boundary-probe emphasis in the holdout.
    let uniform_set: Vec<[i32; 2]> = {
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
    };
    let uniform = {
        let cur_u = eval_states(
            &images,
            clamp,
            &uniform_set,
            policy,
            |_, _, cur| cur.clone(),
            sample_linear,
        );
        let unc_u = eval_states(
            &images,
            clamp,
            &uniform_set,
            policy,
            |_, r, _| r.clone(),
            sample_linear,
        );
        let shift_u = uv_phase.as_ref().map(|([ds, dt], _)| {
            let mut e = Eval::default();
            for (k, (r, cur)) in images.iter().enumerate() {
                let f = bilinear(cur, clamp);
                e.merge(&evaluate(r, clamp, &uniform_set, policy, k, |s, t| {
                    f(s + ds, t + dt)
                }));
            }
            e
        });
        (cur_u, unc_u, shift_u)
    };
    let cell_set: BTreeSet<(i32, i32)> = eval
        .iter()
        .chain(train.iter())
        .map(|&[s, t]| (t.div_euclid(32), s.div_euclid(32)))
        .collect();
    let cells = cell_set.len();
    let eval_points: BTreeSet<[i32; 2]> = eval.iter().copied().collect();
    let mut dense = train.clone();
    for &(ty, tx) in &cell_set {
        let o = (tx * 7 + ty * 11).rem_euclid(16);
        for (i, j) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            let p = [tx * 32 + 16 * i + o, ty * 32 + 16 * j + (o * 5 + 3) % 16];
            if !eval_points.contains(&p) {
                dense.push(p);
            }
        }
    }
    let dense = sorted(dense);
    let rgba_dense = {
        let mut e = Eval::default();
        let mut pack = 0u64;
        for (k, (r, _)) in images.iter().enumerate() {
            let img = if w >= 2 && h >= 2 && !dense.is_empty() {
                let seed = fc::solve_samples_ungated(r, clamp[0], clamp[1], &dense, policy);
                if refine_integer {
                    refine(r, &seed, clamp, &dense, policy)
                } else {
                    seed
                }
            } else {
                r.clone()
            };
            e.merge(&evaluate(r, clamp, &eval, policy, k, bilinear(&img, clamp)));
            pack += direct_pack_bytes(&img);
        }
        Candidate {
            eval: e,
            level0_bytes: direct_level0 * states as u64,
            pack_bytes: pack,
            float_sse: None,
            iterations: None,
        }
    };

    Analysis {
        index,
        width: w,
        height: h,
        states,
        eval_samples: eval.len(),
        train_samples: train.len(),
        groups,
        current,
        uncompensated,
        nearest: nearest_c,
        rgba_practical,
        rgba_oracle,
        rgba_alpha_held_oracle,
        rgba_silhouette_oracle,
        indexed,
        per_state_direct,
        per_site_oracle_vis_sse,
        per_site_current,
        uv_phase,
        uniform,
        cells,
        rgba_dense,
        pack_level0_match,
        level0_diff,
        padded_mismatch,
        current_level0_bytes: cur_level0,
        current_pack_bytes: cur_pack,
        practical_image,
    }
}

// ---------------------------------------------------------------------------
// Classification
// ---------------------------------------------------------------------------

/// Materially bad: the pipeline's own admission threshold (RE-219/RE-305:
/// at least 1% of samples >= 8/255 with a maximum >= 16/255), or at least
/// 0.1% of samples >= 32/255, on the gameplay-visible error.
fn material(s: &Stat) -> bool {
    (s.p8() >= 1.0 && s.max >= 16) || s.p32() >= 0.1
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Tier {
    None,
    Minor,
    Material,
}

fn tier(s: &Stat) -> Tier {
    if s.max < 8 {
        Tier::None
    } else if material(s) {
        Tier::Material
    } else {
        Tier::Minor
    }
}

const CAUSES: [&str; 11] = [
    "PALETTE_CONSTRAINT",
    "ALPHA_CONSTRAINT",
    "ANIMATED_PALETTE_CONFLICT",
    "CONFLICTING_USE_SITES",
    "BILINEAR_SURFACE_LIMIT",
    "TEXGEN_COVERAGE",
    "ADDRESSING_OR_EDGE",
    "FORMAT_QUANTIZATION",
    "SAMPLING_ALIGNMENT",
    "LIKELY_IMPLEMENTATION_BUG",
    "OTHER",
];

struct Classified {
    causes: Vec<(&'static str, String)>,
}

fn ratio(a: u64, b: u64) -> f64 {
    a as f64 / b.max(1) as f64
}

fn classify(a: &Analysis, v: &Variant) -> Classified {
    let c = &v.conversion;
    let cur = &a.current.eval.vis;
    let mut causes: Vec<(&'static str, String)> = Vec::new();

    // Implementation checks first: they invalidate everything else.
    if !a.pack_level0_match {
        causes.push((
            "LIKELY_IMPLEMENTATION_BUG",
            match a.level0_diff {
                (n, Some((x, y, rec, packed))) => {
                    let t = &c.texture;
                    let full = match t.size {
                        ssb_rom::texture::BitSize::Bits4 => 16,
                        _ => 256,
                    };
                    let hint = if t.tlut.enabled() && t.format != ssb_rom::texture::Format::Ci {
                        format!(
                            "{} texture drawn with G_MDSFT_TEXTLUT {:?} and a {}-entry TLUT: its texels are TLUT indices (RE-313), and the packed palette does not reproduce that lookup",
                            fmt_name(t.format, t.size),
                            t.tlut,
                            t.palette_entries
                        )
                    } else if t.tlut.enabled() && (t.palette_entries as usize) < full {
                        format!(
                            "{} texture with a {}-entry TLUT: an index past the loaded entries reads stale TMEM (RE-313); check `STALE_TLUT_ENTRIES` and the `stale-tlut-unresolved` log",
                            fmt_name(t.format, t.size),
                            t.palette_entries
                        )
                    } else {
                        "cause not classified".into()
                    };
                    format!(
                        "packed level 0 differs from the recorded final texels ({n} of {} texels; first at ({x},{y}): recorded {} packed {}); {hint}",
                        a.width * a.height,
                        rgba(rec),
                        rgba(packed)
                    )
                }
                _ => "packed level 0 differs from the recorded final texels (index field)".into(),
            },
        ));
    }
    if a.padded_mismatch > 0 {
        causes.push((
            "LIKELY_IMPLEMENTATION_BUG",
            format!(
                "{} holdout samples differ between logical-size and padded GE addressing",
                a.padded_mismatch
            ),
        ));
    }
    let comp = c.method != "no-solve"
        && c.method != "no-promotion"
        && c.method != "holdout-rejected"
        && c.method != "not-attempted:direct-format"
        && c.method != "animated-rejected"
        && c.method != "animated-incomplete-table";
    let unc = &a.uncompensated.eval.vis;
    if comp && (cur.sse > unc.sse || cur.ge32 > unc.ge32) {
        causes.push((
            "LIKELY_IMPLEMENTATION_BUG",
            format!(
                "accepted compensation regresses the independent holdout (visible SSE {} -> {}, >=32 {} -> {})",
                unc.sse, cur.sse, unc.ge32, cur.ge32
            ),
        ));
    }
    if !c.attempted && a.rgba_practical.eval.vis.sse * 4 <= cur.sse * 3 {
        causes.push((
            "LIKELY_IMPLEMENTATION_BUG",
            format!(
                "compensation never attempted for source format {:?} ({}); RGBA8888 fit would cut holdout visible SSE {:.0}%",
                c.source_psm,
                if c.source_psm.is_paletted() {
                    "the RE-283 direct-colour bypass routes it to convert_texture's non-paletted branch, which never runs the compensator"
                } else {
                    "convert_texture only compensates paletted formats; its Psm5551 branch is unreachable"
                },
                100.0 - 100.0 * ratio(a.rgba_practical.eval.vis.sse, cur.sse)
            ),
        ));
    }

    let oracle = &a.rgba_oracle.eval.vis;
    let practical = &a.rgba_practical.eval.vis;
    // Proof, not inference: every oracle is fitted on the holdout samples
    // themselves, so no same-resolution texture does materially better on
    // them. The limit holds when the best oracle keeps at least half of the
    // current colour error (samples whose alpha-test result agrees) and is
    // still material or reaches 16/255. The cutout silhouette is judged
    // separately below.
    let best = a.best_oracle();
    let cur_color = &a.current.eval.color;
    let best_color = &best.eval.color;
    if best_color.sse * 2 >= cur_color.sse && (material(best_color) || best_color.max >= 16) {
        causes.push((
            "BILINEAR_SURFACE_LIMIT",
            format!(
                "best same-resolution RGBA8888 oracle fitted on the holdout itself keeps {:.0}% of the current colour SSE: {:.2}% >=8, {:.2}% >=32, max {} (continuous LB SSE {:.0}, {} iterations)",
                100.0 * ratio(best_color.sse, cur_color.sse),
                best_color.p8(),
                best_color.p32(),
                best_color.max,
                a.rgba_oracle.float_sse.unwrap_or(0.0),
                a.rgba_oracle.iterations.unwrap_or(0)
            ),
        ));
    }
    if let (AlphaPolicy::Cutout { .. }, Some(sil)) = (c.policy, &a.rgba_silhouette_oracle) {
        let flips = a.current.eval.cutout_mismatch;
        let flip_share = ratio(cur.sse - cur_color.sse, cur.sse);
        if flips > 0 && flip_share >= 0.5 {
            let left = sil.eval.cutout_mismatch;
            if left * 4 <= flips * 3 {
                causes.push((
                    "ALPHA_CONSTRAINT",
                    format!(
                        "alpha-test flips carry {:.0}% of visible SSE; a flip-minimizing alpha search removes {:.0}% of them ({} -> {}), but the pipeline reverts cutout alpha whenever any flip remains",
                        flip_share * 100.0,
                        100.0 - 100.0 * ratio(left, flips),
                        flips,
                        left
                    ),
                ));
            } else {
                causes.push((
                    "BILINEAR_SURFACE_LIMIT",
                    format!(
                        "alpha-test flips carry {:.0}% of visible SSE and a flip-minimizing alpha search keeps {:.0}% of them ({} -> {})",
                        flip_share * 100.0,
                        100.0 * ratio(left, flips),
                        flips,
                        left
                    ),
                ));
            }
        }
    }
    if c.source_psm.is_paletted() && c.animated.is_none() && c.final_psm.is_paletted() {
        if let Some(ix) = &a.indexed {
            if ix.eval.vis.sse * 4 >= practical.sse * 5 && practical.sse * 4 <= cur.sse * 3 {
                causes.push((
                    "PALETTE_CONSTRAINT",
                    format!(
                        "best filter-aware index field {} vs unconstrained RGBA8888 {} holdout visible SSE",
                        ix.eval.vis.sse, practical.sse
                    ),
                ));
            }
        }
    }
    if c.animated.is_some() {
        if let (Some(ix), Some(direct)) = (&a.indexed, &a.per_state_direct) {
            if direct.eval.vis.sse * 4 <= ix.eval.vis.sse * 3 {
                causes.push((
                    "ANIMATED_PALETTE_CONFLICT",
                    format!(
                        "joint index field over {} palettes {} vs per-palette direct colour {} holdout visible SSE",
                        a.states, ix.eval.vis.sse, direct.eval.vis.sse
                    ),
                ));
            }
        }
    }
    if c.policy == AlphaPolicy::Translucent {
        let alpha_share = ratio(a.current.eval.alpha.sse, a.current.eval.raw.sse);
        let held = a
            .rgba_alpha_held_oracle
            .as_ref()
            .map_or(0, |h| h.eval.vis.sse);
        if held * 4 >= oracle.sse * 5 && held > oracle.sse + 1000 {
            causes.push((
                "ALPHA_CONSTRAINT",
                format!(
                    "alpha-held oracle visible SSE {} vs free-alpha oracle {} (alpha share of current raw SSE {:.0}%)",
                    held,
                    oracle.sse,
                    alpha_share * 100.0
                ),
            ));
        } else if alpha_share >= 0.5 && a.current.eval.alpha.max >= 16 {
            causes.push((
                "ALPHA_CONSTRAINT",
                format!(
                    "alpha carries {:.0}% of current raw SSE (alpha max {})",
                    alpha_share * 100.0,
                    a.current.eval.alpha.max
                ),
            ));
        }
    }
    if let Some((own, shared)) = a.per_site_oracle_vis_sse {
        if shared * 4 >= own * 5 && shared > own + 1000 {
            causes.push((
                "CONFLICTING_USE_SITES",
                format!(
                    "{} distinct coverages: per-site oracles {} vs one shared oracle {} visible SSE",
                    a.groups.len(),
                    own,
                    shared
                ),
            ));
        }
    }
    let texgen = a
        .groups
        .iter()
        .any(|g| g.kind == CoverageKind::TexgenReal || g.kind == CoverageKind::TexgenFullTile);
    if texgen {
        let fallback = a
            .groups
            .iter()
            .any(|g| g.kind == CoverageKind::TexgenFullTile);
        let gap = practical.sse * 2 >= oracle.sse * 3 && practical.sse > oracle.sse + 1000;
        if fallback || gap {
            causes.push((
                "TEXGEN_COVERAGE",
                format!(
                    "{}; practical RGBA (pose-trained) {} vs holdout oracle {} visible SSE",
                    if fallback {
                        "material-animated texgen uses full-tile coverage"
                    } else {
                        "real-normal pose coverage"
                    },
                    practical.sse,
                    oracle.sse
                ),
            ));
        }
    }
    let edge_share = ratio(a.current.eval.edge_vis_sse, cur.sse);
    let edge_sample_share = ratio(a.current.eval.edge_n, cur.n);
    if cur.sse > 0 && edge_share >= 0.5 && edge_share >= 2.0 * edge_sample_share {
        causes.push((
            "ADDRESSING_OR_EDGE",
            format!(
                "{:.0}% of visible SSE from the {:.0}% of samples whose footprint clamps or wraps",
                edge_share * 100.0,
                edge_sample_share * 100.0
            ),
        ));
    }
    if let (Some((d, e)), Some(u)) = (&a.uv_phase, &a.uniform.2) {
        let uc = &a.uniform.0.vis;
        if e.vis.sse * 5 <= cur.sse * 4 && u.vis.sse * 5 <= uc.sse * 4 {
            causes.push((
                "SAMPLING_ALIGNMENT",
                format!(
                    "a uniform ({}, {})/32-texel UV shift alone cuts visible SSE {:.0}% on the holdout ({} -> {}, alpha-test flips {} -> {}) and {:.0}% on the phase-uniform set",
                    d[0],
                    d[1],
                    100.0 - 100.0 * ratio(e.vis.sse, cur.sse),
                    cur.sse,
                    e.vis.sse,
                    a.current.eval.cutout_mismatch,
                    e.cutout_mismatch,
                    100.0 - 100.0 * ratio(u.vis.sse, uc.sse)
                ),
            ));
        }
    }
    let odd_share = ratio(a.current.eval.odd_vis_sse, cur.sse);
    let odd_sample_share = ratio(a.current.eval.odd_n, cur.n);
    if cur.sse > 0
        && odd_share >= 0.5
        && odd_share >= 1.5 * odd_sample_share
        && !causes.iter().any(|(k, _)| *k == "SAMPLING_ALIGNMENT")
    {
        causes.push((
            "SAMPLING_ALIGNMENT",
            format!(
                "{:.0}% of visible SSE from odd-1/32 coordinates ({:.0}% of samples) that the GE's 4-bit weights truncate",
                odd_share * 100.0,
                odd_sample_share * 100.0
            ),
        ));
    }
    if let Some(fl) = a.rgba_oracle.float_sse {
        let integer = a.rgba_oracle.eval.raw.sse as f64;
        if integer > 1.5 * fl && integer - fl > 5000.0 && !material(oracle) {
            causes.push((
                "FORMAT_QUANTIZATION",
                format!("oracle integer SSE {:.0} vs continuous {:.0}: GE truncation/8-bit rounding dominates", integer, fl),
            ));
        }
    }
    if c.final_psm == Psm::Psm5551 {
        causes.push(("FORMAT_QUANTIZATION", "5551 final format".into()));
    }
    if causes.is_empty() {
        let dense = &a.rgba_dense.eval.vis;
        let why = if dense.sse * 4 <= cur.sse * 3 && practical.sse * 4 > cur.sse * 3 {
            format!(
                "training density: {} training samples over {} touched cells ({:.2}/cell); the pipeline solve trained on 4 extra phase-varied samples per cell cuts holdout visible SSE {:.0}% ({} -> {}), the shipped training set only {:.0}%; holdout oracle {}",
                a.train_samples,
                a.cells,
                a.train_samples as f64 / a.cells.max(1) as f64,
                100.0 - 100.0 * ratio(dense.sse, cur.sse),
                cur.sse,
                dense.sse,
                100.0 - 100.0 * ratio(practical.sse, cur.sse),
                oracle.sse
            )
        } else {
            format!(
                "no class matched: practical {} / dense-training {} / oracle {} vs current {} visible SSE; the gates ({}) kept this variant",
                practical.sse, dense.sse, oracle.sse, cur.sse, c.method
            )
        };
        causes.push(("OTHER", why));
    }
    Classified { causes }
}

// ---------------------------------------------------------------------------
// Naming and severity
// ---------------------------------------------------------------------------

struct Names(BTreeMap<u32, String>);

impl Names {
    fn load() -> Self {
        let mut map = BTreeMap::new();
        if let Ok(text) = std::fs::read_to_string("refs/ssb-decomp-re/include/reloc_data.us.h") {
            for line in text.lines() {
                let Some(rest) = line.strip_prefix("extern int ll") else {
                    continue;
                };
                let Some((name, tail)) = rest.split_once("FileID;") else {
                    continue;
                };
                let Some(hex) = tail.trim().strip_prefix("// 0x") else {
                    continue;
                };
                if let Ok(id) = u32::from_str_radix(hex.trim(), 16) {
                    map.insert(id, name.to_string());
                }
            }
        }
        Names(map)
    }
    fn get(&self, id: u32) -> String {
        self.0
            .get(&id)
            .cloned()
            .unwrap_or_else(|| format!("file{id}"))
    }
}

/// Gameplay-visibility category and weight of a relocData file. Weights
/// are a fixed, documented ordering, not a perceptual model: fighters and
/// the playable stages are on screen for a whole match; 1P-only fighters,
/// items/weapons/effects are intermittent; menus/movies are not gameplay.
fn category(name: &str) -> (&'static str, f64) {
    const FIGHTERS: [&str; 13] = [
        "Mario", "Fox", "Donkey", "Samus", "Luigi", "Link", "Kirby", "Purin", "Captain", "Ness",
        "Yoshi", "Pikachu", "Boss",
    ];
    let model = name.ends_with("Model") || name.ends_with("MainMotion") || name.ends_with("Main");
    if model
        && (name.starts_with("MMario")
            || name.starts_with('N') && FIGHTERS.iter().any(|f| name[1..].starts_with(f))
            || name.starts_with("Boss"))
    {
        return ("fighter-1p-only", 0.8);
    }
    if FIGHTERS.iter().any(|f| name.starts_with(f)) {
        if name.ends_with("Model") {
            return ("fighter", 1.0);
        }
        return ("fighter-weapon-effect", 0.7);
    }
    if name.starts_with("NCommon") {
        return ("fighter-1p-only", 0.8);
    }
    if name.starts_with("GRBonus") || name.contains("Bonus") {
        return ("bonus-stage", 0.6);
    }
    if name.starts_with("Stage") || name.starts_with("GR") {
        return ("stage", 0.9);
    }
    if name.starts_with("IT")
        || name.starts_with("EF")
        || name.starts_with("FT")
        || name.starts_with("WP")
    {
        return ("item-weapon-effect", 0.7);
    }
    if name.starts_with("IF") {
        return ("hud", 0.6);
    }
    ("menu-movie-ui", 0.5)
}

fn severity(s: &Stat, weight: f64) -> f64 {
    weight * (4.0 * s.p32() + 2.0 * s.p16() + s.p8() + s.mean())
}

fn policy_name(p: AlphaPolicy) -> String {
    match p {
        AlphaPolicy::Opaque => "opaque".into(),
        AlphaPolicy::Cutout {
            greater_or_equal,
            threshold,
        } => format!(
            "cutout(a{}{})",
            if greater_or_equal { ">=" } else { ">" },
            threshold
        ),
        AlphaPolicy::Translucent => "translucent".into(),
    }
}

fn fmt_name(f: ssb_rom::texture::Format, s: ssb_rom::texture::BitSize) -> String {
    let bits = match s {
        ssb_rom::texture::BitSize::Bits4 => 4,
        ssb_rom::texture::BitSize::Bits8 => 8,
        ssb_rom::texture::BitSize::Bits16 => 16,
        ssb_rom::texture::BitSize::Bits32 => 32,
    };
    format!("{f:?}{bits}").to_uppercase()
}

fn psm_name(p: Psm) -> &'static str {
    match p {
        Psm::Psm5650 => "5650",
        Psm::Psm5551 => "5551",
        Psm::Psm4444 => "4444",
        Psm::Psm8888 => "8888",
        Psm::PsmT4 => "T4",
        Psm::PsmT8 => "T8",
    }
}

fn addressing(t: &ssb_rom::mesh::TextureRef) -> String {
    let axis = |mirror: bool, clamp: bool| match (mirror, clamp) {
        (true, true) => "mirror+clamp",
        (true, false) => "mirror",
        (false, true) => "clamp",
        (false, false) => "repeat",
    };
    format!(
        "s={} t={}",
        axis(t.mirror_s, t.clamp_s),
        axis(t.mirror_t, t.clamp_t)
    )
}

// ---------------------------------------------------------------------------
// Interventions
// ---------------------------------------------------------------------------

struct Intervention {
    name: &'static str,
    /// Measured with a candidate the packer or runtime could ship as is
    /// (not an oracle fitted on the holdout).
    deployable: bool,
    after_tier: Option<Tier>,
    /// Holdout visible SSE after the intervention, when measurable.
    after_sse: Option<u64>,
    note: String,
    vram_delta: i64,
    pack_delta: i64,
    runtime: &'static str,
    complexity: &'static str,
}

fn interventions(a: &Analysis, v: &Variant, cl: &Classified) -> Vec<Intervention> {
    let c = &v.conversion;
    let has = |k: &str| cl.causes.iter().any(|(n, _)| *n == k);
    let cur = a.current.eval.vis.sse;
    let mut out = vec![Intervention {
        name: "accept residual",
        deployable: false,
        after_tier: None,
        after_sse: Some(cur),
        note: "no change".into(),
        vram_delta: 0,
        pack_delta: 0,
        runtime: "none",
        complexity: "none",
    }];
    if c.final_psm != Psm::Psm8888 || c.animated.is_some() {
        let p = &a.rgba_practical;
        out.push(Intervention {
            name: if c.animated.is_some() { "per-palette direct-color variant" } else { "promote to RGBA8888" },
            deployable: true,
            after_tier: Some(tier(&p.eval.vis)),
            after_sse: Some(p.eval.vis.sse),
            note: format!(
                "pipeline continuous solve (ungated) + GE-exact refinement on training coverage; holdout max {} / >=32 {:.2}%{}",
                p.eval.vis.max,
                p.eval.vis.p32(),
                if c.animated.is_some() { format!("; {} direct variants selected per palette state at runtime (new runtime path)", a.states) } else { String::new() }
            ),
            vram_delta: p.level0_bytes as i64 - a.current_level0_bytes as i64,
            pack_delta: p.pack_bytes as i64 - a.current_pack_bytes as i64,
            runtime: if c.animated.is_some() { "texture swap per palette change instead of CLUT reload; 32-bit texel fetch" } else { "32-bit texel fetch (x8 bandwidth vs T4, x4 vs T8)" },
            complexity: if c.animated.is_some() { "medium: runtime must map PaletteID to a texture index" } else { "low: build-time override list" },
        });
    } else {
        out.push(Intervention {
            name: "re-fit RGBA8888 (already direct)",
            deployable: true,
            after_tier: Some(tier(&a.rgba_practical.eval.vis)),
            after_sse: Some(a.rgba_practical.eval.vis.sse),
            note: "pipeline continuous solve (ungated) + GE-exact refinement".into(),
            vram_delta: 0,
            pack_delta: 0,
            runtime: "none",
            complexity: "low",
        });
    }
    out.push(Intervention {
        name: "denser-training RGBA8888 variant",
        deployable: true,
        after_tier: Some(tier(&a.rgba_dense.eval.vis)),
        after_sse: Some(a.rgba_dense.eval.vis.sse),
        note: format!(
            "pipeline solve on training + 4 phase-varied samples per touched cell ({} cells, {} training samples now)",
            a.cells, a.train_samples
        ),
        vram_delta: a.rgba_dense.level0_bytes as i64 - a.current_level0_bytes as i64,
        pack_delta: a.rgba_dense.pack_bytes as i64 - a.current_pack_bytes as i64,
        runtime: if c.final_psm == Psm::Psm8888 { "none" } else { "32-bit texel fetch" },
        complexity: "low-medium: coverage budget per variant; build time grows with texel count",
    });
    if let (Some(ix), true) = (&a.indexed, c.final_psm.is_paletted()) {
        out.push(Intervention {
            name: "re-optimize index field (same palette)",
            deployable: true,
            after_tier: Some(tier(&ix.eval.vis)),
            after_sse: Some(ix.eval.vis.sse),
            note: if c.animated.is_some() {
                "optimize_animated_indices on training coverage".into()
            } else {
                "optimize_palette_indices on training coverage, seeded by the ungated solve".into()
            },
            vram_delta: 0,
            pack_delta: 0,
            runtime: "none",
            complexity: "low: gate override only",
        });
    }
    if a.groups.len() >= 2 {
        if let Some((own, shared)) = a.per_site_oracle_vis_sse {
            let extra = (a.groups.len() as i64 - 1)
                * level0_bytes(a.width, a.height, Psm::Psm8888, 0) as i64;
            out.push(Intervention {
                name: "per-use-site compensated variant",
                deployable: false,
                after_tier: None,
                after_sse: None,
                note: format!(
                    "oracle bound: {} per-site vs {} shared visible SSE over {} coverages",
                    own,
                    shared,
                    a.groups.len()
                ),
                vram_delta: extra,
                pack_delta: extra * 4 / 3,
                runtime: "none (texture index per primitive already exists)",
                complexity: "low-medium: cache key already per coverage; relax dedupe",
            });
        }
    }
    if c.policy != AlphaPolicy::Opaque && has("ALPHA_CONSTRAINT") {
        let bound = a.rgba_silhouette_oracle.as_ref().unwrap_or(&a.rgba_oracle);
        out.push(Intervention {
            name: "alpha-only hand compensation",
            deployable: false,
            after_tier: Some(tier(&bound.eval.vis)),
            after_sse: Some(bound.eval.vis.sse),
            note: format!(
                "bound: {} on the holdout (alpha-test flips {} -> {}); alpha-held oracle {}",
                if a.rgba_silhouette_oracle.is_some() {
                    "flip-minimizing alpha search"
                } else {
                    "free-alpha oracle"
                },
                a.current.eval.cutout_mismatch,
                bound.eval.cutout_mismatch,
                a.rgba_alpha_held_oracle
                    .as_ref()
                    .map_or(0, |h| h.eval.vis.sse)
            ),
            vram_delta: 0,
            pack_delta: 0,
            runtime: "none",
            complexity: "medium: manual per-texel alpha edits must keep the cutout silhouette",
        });
    }
    out.push(Intervention {
        name: "per-material GU_NEAREST",
        deployable: true,
        after_tier: Some(tier(&a.nearest.eval.vis)),
        after_sse: Some(a.nearest.eval.vis.sse),
        note: "point sampling without the +0.5 texel linear bias".into(),
        vram_delta: a.uncompensated.level0_bytes as i64 - a.current_level0_bytes as i64,
        pack_delta: a.uncompensated.pack_bytes as i64 - a.current_pack_bytes as i64,
        runtime: "one sceGuTexFilter + texture offset change per material",
        complexity: "low-medium: per-material flag and bias switch in meshdraw",
    });
    if let Some((d, e)) = &a.uv_phase {
        out.push(Intervention {
            name: "tiny UV phase adjustment",
            // Deployable only when the phase-uniform set confirms the
            // holdout's gain (>= 20%), since the holdout over-samples the
            // 1/32 boundary phases a shift trades on.
            deployable: a
                .uniform
                .2
                .as_ref()
                .is_some_and(|u| u.vis.sse * 5 <= a.uniform.0.vis.sse * 4),
            after_tier: Some(tier(&e.vis)),
            after_sse: Some(e.vis.sse),
            note: format!(
                "best uniform shift ({}, {})/32 texel; phase-uniform set {} -> {}",
                d[0],
                d[1],
                a.uniform.0.vis.sse,
                a.uniform.2.as_ref().map_or(0, |u| u.vis.sse)
            ),
            vram_delta: 0,
            pack_delta: 0,
            runtime: "per-material sceGuTexOffset",
            complexity: "low, but a global shift trades error elsewhere",
        });
    }
    out.push(Intervention {
        name: "selective geometry subdivision",
        deployable: false,
        after_tier: None,
        after_sse: Some(cur),
        note: "does not change the per-pixel filter formula; UVs are already interpolated per pixel on both machines".into(),
        vram_delta: 0,
        pack_delta: 0,
        runtime: "more vertices",
        complexity: "high",
    });
    if has("LIKELY_IMPLEMENTATION_BUG") {
        out.push(Intervention {
            name: "investigate likely renderer/asset bug",
            deployable: false,
            after_tier: None,
            after_sse: None,
            note: cl
                .causes
                .iter()
                .filter(|(k, _)| *k == "LIKELY_IMPLEMENTATION_BUG")
                .map(|(_, r)| r.as_str())
                .collect::<Vec<_>>()
                .join("; "),
            vram_delta: 0,
            pack_delta: 0,
            runtime: "n/a",
            complexity: "investigation",
        });
    }
    out
}

/// Best deployable (non-oracle) alternative that halves the holdout visible
/// SSE, or cuts it by >= 25% and leaves the material tier.
fn deployable_best(a: &Analysis, v: &Variant, cl: &Classified) -> Option<(Intervention, Tier)> {
    let now = a.current.eval.vis.sse;
    interventions(a, v, cl)
        .into_iter()
        .filter(|i| i.deployable && i.after_sse.is_some())
        .map(|i| {
            let t = i.after_tier.unwrap_or(Tier::Material);
            (i, t)
        })
        .filter(|(i, t)| {
            let after = i.after_sse.unwrap();
            after * 2 <= now || (after * 4 <= now * 3 && *t != Tier::Material)
        })
        .min_by_key(|(i, _)| (i.after_sse.unwrap(), i.vram_delta))
}

/// Oracle-level bound for a hand edit: the best holdout-fitted oracle (or
/// cutout silhouette search) at least halves the visible SSE.
fn oracle_bound(a: &Analysis) -> u64 {
    [
        Some(&a.rgba_oracle),
        a.rgba_alpha_held_oracle.as_ref(),
        a.rgba_silhouette_oracle.as_ref(),
    ]
    .into_iter()
    .flatten()
    .map(|c| c.eval.vis.sse)
    .min()
    .unwrap()
}

fn disposition(a: &Analysis, v: &Variant, cl: &Classified, t: Tier) -> &'static str {
    if t == Tier::None {
        "clean"
    } else if cl
        .causes
        .iter()
        .any(|(k, _)| *k == "LIKELY_IMPLEMENTATION_BUG")
    {
        "investigate-bug"
    } else if t == Tier::Minor {
        "minor"
    } else if deployable_best(a, v, cl).is_some() {
        "manual-intervention"
    } else if oracle_bound(a) * 2 <= a.current.eval.vis.sse {
        "hand-edit-candidate"
    } else {
        "accept"
    }
}

// ---------------------------------------------------------------------------
// PNG
// ---------------------------------------------------------------------------

fn crc32(data: &[u8]) -> u32 {
    let mut table = [0u32; 256];
    for (n, e) in table.iter_mut().enumerate() {
        let mut c = n as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 {
                0xEDB88320 ^ (c >> 1)
            } else {
                c >> 1
            };
        }
        *e = c;
    }
    let mut c = 0xFFFF_FFFFu32;
    for &b in data {
        c = table[((c ^ b as u32) & 0xff) as usize] ^ (c >> 8);
    }
    c ^ 0xFFFF_FFFF
}

/// Deterministic RGB PNG with stored (uncompressed) deflate blocks.
fn png(w: u32, h: u32, rgb: &[u8]) -> Vec<u8> {
    let mut raw = Vec::with_capacity((w as usize * 3 + 1) * h as usize);
    for y in 0..h as usize {
        raw.push(0);
        raw.extend_from_slice(&rgb[y * w as usize * 3..(y + 1) * w as usize * 3]);
    }
    let mut z = vec![0x78, 0x01];
    let chunks: Vec<&[u8]> = raw.chunks(65535).collect();
    for (i, ch) in chunks.iter().enumerate() {
        z.push(u8::from(i + 1 == chunks.len()));
        let len = ch.len() as u16;
        z.extend_from_slice(&len.to_le_bytes());
        z.extend_from_slice(&(!len).to_le_bytes());
        z.extend_from_slice(ch);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &x in &raw {
        a = (a + x as u32) % 65521;
        b = (b + a) % 65521;
    }
    z.extend_from_slice(&((b << 16) | a).to_be_bytes());
    let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    let mut chunk = |kind: &[u8], data: &[u8]| {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let mut body = kind.to_vec();
        body.extend_from_slice(data);
        out.extend_from_slice(&body);
        out.extend_from_slice(&crc32(&body).to_be_bytes());
    };
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    chunk(b"IHDR", &ihdr);
    chunk(b"IDAT", &z);
    chunk(b"IEND", &[]);
    out
}

/// Composites one sample for display under the variant's alpha policy.
fn display(policy: AlphaPolicy, c: [u8; 4], x: u32, y: u32) -> [u8; 3] {
    let checker = if ((x / 8) + (y / 8)) % 2 == 0 {
        96u32
    } else {
        160
    };
    match policy {
        AlphaPolicy::Opaque => [c[0], c[1], c[2]],
        AlphaPolicy::Cutout { .. } => {
            if passes(policy, c[3]) {
                [c[0], c[1], c[2]]
            } else {
                [checker as u8; 3]
            }
        }
        AlphaPolicy::Translucent => {
            let a = c[3] as u32;
            [0, 1, 2].map(|i| ((c[i] as u32 * a + checker * (255 - a)) / 255) as u8)
        }
    }
}

fn heat(d: u8) -> [u8; 3] {
    // 0 black, 8 blue, 16 magenta, 32 red, 64 yellow, >=128 white.
    let stops: [(u8, [u8; 3]); 6] = [
        (0, [0, 0, 0]),
        (8, [0, 0, 200]),
        (16, [180, 0, 180]),
        (32, [230, 0, 0]),
        (64, [255, 220, 0]),
        (128, [255, 255, 255]),
    ];
    for win in stops.windows(2) {
        let ((d0, c0), (d1, c1)) = (win[0], win[1]);
        if d <= d1 {
            let f = (d - d0) as u32;
            let span = (d1 - d0) as u32;
            return [0, 1, 2]
                .map(|i| ((c0[i] as u32 * (span - f) + c1[i] as u32 * f) / span) as u8);
        }
    }
    [255, 255, 255]
}

const PANELS: [&str; 6] = [
    "reference",
    "current",
    "diff",
    "nearest",
    "rgba",
    "coverage",
];

/// Writes diagnostic crops around the worst holdout sample.
fn write_images(
    dir: &std::path::Path,
    a: &Analysis,
    v: &Variant,
    eval: &[[i32; 2]],
) -> std::io::Result<Vec<String>> {
    std::fs::create_dir_all(dir)?;
    let c = &v.conversion;
    let clamp = v.clamp;
    let policy = c.policy;
    let images = state_images(v);
    let worst = a
        .current
        .eval
        .worst
        .expect("measured variant has a worst sample");
    let (reference, current) = &images[worst.state];
    let (w, h) = (a.width as i32, a.height as i32);
    let span_x = w.min(16);
    let span_y = h.min(16);
    let x0 = (worst.s.div_euclid(32) - span_x / 2).clamp(0, (w - span_x).max(0));
    let y0 = (worst.t.div_euclid(32) - span_y / 2).clamp(0, (h - span_y).max(0));
    const SUB: i32 = 8;
    const ZOOM: i32 = 2;
    let (pw, ph) = ((span_x * SUB * ZOOM) as u32, (span_y * SUB * ZOOM) as u32);
    let (ms, mt) = modes(clamp);
    let rgba_img = if c.animated.is_some() && worst.state != 0 {
        fit(reference, reference, clamp, eval, free_channels(policy)).image
    } else {
        a.practical_image.clone()
    };
    let covered: BTreeSet<(i32, i32)> = eval
        .iter()
        .filter(|&&[s, t]| {
            let (sx, sy) = (s - x0 * 32, t - y0 * 32);
            (0..span_x * 32).contains(&sx) && (0..span_y * 32).contains(&sy)
        })
        .map(|&[s, t]| ((s - x0 * 32) * SUB / 32, (t - y0 * 32) * SUB / 32))
        .collect();
    let mut panels: Vec<Vec<u8>> = vec![Vec::with_capacity((pw * ph * 3) as usize); PANELS.len()];
    for py in 0..ph as i32 {
        for px in 0..pw as i32 {
            let (gx, gy) = (px / ZOOM, py / ZOOM);
            let s = x0 * 32 + gx * (32 / SUB) + (32 / SUB) / 2;
            let t = y0 * 32 + gy * (32 / SUB) + (32 / SUB) / 2;
            let r = sample_3point_addressed(reference, s, t, ms, mt);
            let p = sample_bilinear_addressed(current, s, t, ms, mt);
            let n = sample_point(reference, s, t, ms, mt);
            let q = sample_bilinear_addressed(&rgba_img, s, t, ms, mt);
            let (_, _, _, _, _, _, vd, _) = sample_error(policy, r, p);
            let (ux, uy) = (px as u32, py as u32);
            panels[0].extend_from_slice(&display(policy, r, ux, uy));
            panels[1].extend_from_slice(&display(policy, p, ux, uy));
            panels[2].extend_from_slice(&heat(vd));
            panels[3].extend_from_slice(&display(policy, n, ux, uy));
            panels[4].extend_from_slice(&display(policy, q, ux, uy));
            let mut m = if (gx % SUB == 0) || (gy % SUB == 0) {
                [40, 40, 40]
            } else {
                [0, 0, 0]
            };
            if covered.contains(&(gx, gy)) {
                m = [0, 220, 0];
            }
            if (s - worst.s).abs() < 32 / SUB && (t - worst.t).abs() < 32 / SUB {
                m = [255, 0, 0];
            }
            panels[5].extend_from_slice(&m);
        }
    }
    let mut paths = Vec::new();
    for (name, rgb) in PANELS.iter().zip(&panels) {
        let path = dir.join(format!("{name}.png"));
        std::fs::write(&path, png(pw, ph, rgb))?;
        paths.push(path.display().to_string());
    }
    // Side-by-side strip with a 4px separator.
    let gap = 4u32;
    let sw = pw * PANELS.len() as u32 + gap * (PANELS.len() as u32 - 1);
    let mut strip = vec![255u8; (sw * ph * 3) as usize];
    for (k, rgb) in panels.iter().enumerate() {
        let ox = k as u32 * (pw + gap);
        for y in 0..ph {
            let src = &rgb[(y * pw * 3) as usize..((y + 1) * pw * 3) as usize];
            let d = ((y * sw + ox) * 3) as usize;
            strip[d..d + src.len()].copy_from_slice(src);
        }
    }
    let path = dir.join("strip.png");
    std::fs::write(&path, png(sw, ph, &strip))?;
    paths.push(path.display().to_string());
    let _ = (x0, y0);
    Ok(paths)
}

// ---------------------------------------------------------------------------
// Output
// ---------------------------------------------------------------------------

fn json_str(s: &str) -> String {
    let mut o = String::from("\"");
    for ch in s.chars() {
        match ch {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            c if (c as u32) < 0x20 => {
                let _ = write!(o, "\\u{:04x}", c as u32);
            }
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

fn f4(x: f64) -> String {
    format!("{x:.4}")
}

fn stat_json(s: &Stat) -> String {
    format!(
        "{{\"samples\":{},\"mean\":{},\"max\":{},\"sse\":{},\"ge8\":{},\"ge16\":{},\"ge32\":{},\"pct_ge8\":{},\"pct_ge16\":{},\"pct_ge32\":{}}}",
        s.n,
        f4(s.mean()),
        s.max,
        s.sse,
        s.ge8,
        s.ge16,
        s.ge32,
        f4(s.p8()),
        f4(s.p16()),
        f4(s.p32())
    )
}

fn eval_json(e: &Eval) -> String {
    let worst = e.worst.map_or("null".to_string(), |w| {
        format!(
            "{{\"s_q5\":{},\"t_q5\":{},\"s_texel\":{},\"t_texel\":{},\"state\":{},\"visible_error\":{},\"n64_rgba\":[{},{},{},{}],\"psp_rgba\":[{},{},{},{}]}}",
            w.s,
            w.t,
            f4(w.s as f64 / 32.0),
            f4(w.t as f64 / 32.0),
            w.state,
            w.d,
            w.reference[0],
            w.reference[1],
            w.reference[2],
            w.reference[3],
            w.psp[0],
            w.psp[1],
            w.psp[2],
            w.psp[3]
        )
    });
    format!(
        "{{\"max_channel\":{},\"rgb\":{},\"alpha\":{},\"visible\":{},\"colour\":{},\"worst\":{},\"edge_samples\":{},\"edge_visible_sse\":{},\"odd_phase_samples\":{},\"odd_phase_visible_sse\":{},\"cutout_mismatch\":{}}}",
        stat_json(&e.raw),
        stat_json(&e.rgb),
        stat_json(&e.alpha),
        stat_json(&e.vis),
        stat_json(&e.color),
        worst,
        e.edge_n,
        e.edge_vis_sse,
        e.odd_n,
        e.odd_vis_sse,
        e.cutout_mismatch
    )
}

fn cand_json(c: &Candidate) -> String {
    format!(
        "{{\"eval\":{},\"level0_bytes\":{},\"pack_bytes\":{},\"float_sse\":{},\"iterations\":{}}}",
        eval_json(&c.eval),
        c.level0_bytes,
        c.pack_bytes,
        c.float_sse.map_or("null".into(), |f| format!("{f:.1}")),
        c.iterations.map_or("null".into(), |i| i.to_string())
    )
}

struct Row<'a> {
    a: &'a Analysis,
    v: &'a Variant,
    cl: Classified,
    tier: Tier,
    disposition: &'static str,
    severity: f64,
    category: &'static str,
    weight: f64,
    primary_name: String,
    images: Vec<String>,
}

fn source_file(v: &Variant) -> u32 {
    v.conversion
        .texture
        .data_file
        .map_or(v.conversion.home, u32::from)
}

fn palette_identity(v: &Variant) -> String {
    let t = &v.conversion.texture;
    match t.palette_offset {
        Some(off) => format!(
            "file {} 0x{:X} bank {} ({} entries)",
            t.palette_file.map_or(v.conversion.home, u32::from),
            off,
            t.palette,
            t.palette_entries
        ),
        None if v.conversion.source_psm.is_paletted() => "generated intensity ramp".into(),
        None => "none".into(),
    }
}

/// Why the final format differs from the natural PSP format, if it does.
fn promotion_kind(v: &Variant) -> &'static str {
    let c = &v.conversion;
    if c.final_psm == c.source_psm {
        "none"
    } else if c.method == "promoted-rgba8888" {
        "compensation-promotion"
    } else if c.source_psm.is_paletted() {
        "re283-direct-colour-bypass"
    } else {
        "direct-format-expansion"
    }
}

fn mapping(a: &Analysis) -> String {
    let kinds: BTreeSet<&str> = a.groups.iter().map(|g| g.kind.name()).collect();
    let mut s = kinds.into_iter().collect::<Vec<_>>().join("+");
    if a.groups.iter().any(|g| g.texgen_linear == Some(true)) {
        s.push_str(" (linear)");
    }
    s
}

fn use_sites_text(a: &Analysis, names: &Names, limit: usize) -> String {
    let mut prims: Vec<(u32, u32, usize, Option<usize>)> = a
        .groups
        .iter()
        .flat_map(|g| g.prims.iter().copied())
        .collect();
    prims.sort();
    prims.dedup();
    let total = prims.len();
    let mut parts: Vec<String> = prims
        .iter()
        .take(limit)
        .map(|&(f, dl, p, slot)| {
            format!(
                "{} {}:0x{:X}#{}{}",
                names.get(f),
                f,
                dl,
                p,
                slot.map_or(String::new(), |s| format!(" sprite{s}"))
            )
        })
        .collect();
    if total > limit {
        parts.push(format!("+{} more", total - limit));
    }
    parts.join("; ")
}

fn row_json(r: &Row<'_>, names: &Names) -> String {
    let a = r.a;
    let v = r.v;
    let c = &v.conversion;
    let t = &c.texture;
    let mut s = String::new();
    let _ = write!(
        s,
        "{{\"variant\":{},\"source_file\":{},\"source_file_name\":{},\"texture_offset\":\"0x{:X}\",\"home_file\":{},\"width\":{},\"height\":{},\"source_format\":\"{}\",\"source_psm\":\"{}\",\"final_psm\":\"{}\",\"format_promoted\":{},\"promotion_kind\":\"{}\",\"palette\":{},\"animated\":{},\"palette_states\":{},\"alpha_mode\":\"{}\",\"addressing\":\"{}\",\"mapping\":\"{}\",\"compensation_method\":\"{}\",\"compensation_attempted\":{},\"level0_bytes\":{},\"pack_bytes\":{},\"pack_level0_match\":{},\"pack_level0_differing_texels\":{},\"padded_addressing_mismatch\":{},\"train_samples\":{},\"eval_samples\":{},\"touched_cells\":{},\"tier\":\"{:?}\",\"disposition\":\"{}\",\"category\":\"{}\",\"visibility_weight\":{},\"severity\":{},\"primary_use\":{},",
        a.index,
        source_file(v),
        json_str(&names.get(source_file(v))),
        t.data_offset,
        c.home,
        a.width,
        a.height,
        fmt_name(t.format, t.size),
        psm_name(c.source_psm),
        psm_name(c.final_psm),
        c.final_psm != c.source_psm,
        promotion_kind(v),
        json_str(&palette_identity(v)),
        c.animated.is_some(),
        a.states,
        policy_name(c.policy),
        addressing(t),
        mapping(a),
        c.method,
        c.attempted,
        a.current_level0_bytes,
        a.current_pack_bytes,
        a.pack_level0_match,
        a.level0_diff.0,
        a.padded_mismatch,
        a.train_samples,
        a.eval_samples,
        a.cells,
        r.tier,
        r.disposition,
        r.category,
        f4(r.weight),
        f4(r.severity),
        json_str(&r.primary_name),
    );
    s.push_str("\"use_sites\":[");
    for (i, (g, e)) in a.groups.iter().zip(&a.per_site_current).enumerate() {
        if i > 0 {
            s.push(',');
        }
        let prims: Vec<String> = g
            .prims
            .iter()
            .map(|&(f, dl, p, slot)| {
                format!(
                    "{{\"file\":{},\"file_name\":{},\"display_list\":\"0x{:X}\",\"primitive\":{},\"sprite_slot\":{}}}",
                    f,
                    json_str(&names.get(f)),
                    dl,
                    p,
                    slot.map_or("null".into(), |x| x.to_string())
                )
            })
            .collect();
        let _ = write!(
            s,
            "{{\"coverage\":\"{}\",\"alpha_mode\":\"{}\",\"material_animation\":{},\"triangles\":{},\"train_samples\":{},\"eval_samples\":{},\"current_visible\":{},\"primitives\":[{}]}}",
            g.kind.name(),
            policy_name(g.policy),
            g.mat_anim,
            g.triangles,
            g.train.len(),
            g.eval.len(),
            stat_json(&e.vis),
            prims.join(",")
        );
    }
    s.push_str("],\"alternatives\":{");
    let mut alts: Vec<(&str, String)> = vec![
        ("uncompensated_linear", cand_json(&a.uncompensated)),
        ("current_linear", cand_json(&a.current)),
        ("nearest", cand_json(&a.nearest)),
        ("rgba8888_practical", cand_json(&a.rgba_practical)),
        ("rgba8888_dense_training", cand_json(&a.rgba_dense)),
        ("rgba8888_oracle", cand_json(&a.rgba_oracle)),
    ];
    if let Some(x) = &a.rgba_alpha_held_oracle {
        alts.push(("rgba8888_alpha_held_oracle", cand_json(x)));
    }
    if let Some(x) = &a.rgba_silhouette_oracle {
        alts.push(("rgba8888_silhouette_oracle", cand_json(x)));
    }
    if let Some(x) = &a.indexed {
        alts.push(("indexed_filter_aware", cand_json(x)));
    }
    if let Some(x) = &a.per_state_direct {
        alts.push(("per_palette_direct", cand_json(x)));
    }
    if let Some((own, shared)) = a.per_site_oracle_vis_sse {
        alts.push((
            "per_use_site_oracle",
            format!("{{\"per_site_visible_sse\":{own},\"shared_visible_sse\":{shared}}}"),
        ));
    }
    if let Some((d, e)) = &a.uv_phase {
        alts.push((
            "uv_phase_shift",
            format!(
                "{{\"shift_q5\":[{},{}],\"eval\":{}}}",
                d[0],
                d[1],
                eval_json(e)
            ),
        ));
    }
    alts.push((
        "phase_uniform_set",
        format!(
            "{{\"current_linear\":{},\"uncompensated_linear\":{},\"uv_phase_shift\":{}}}",
            eval_json(&a.uniform.0),
            eval_json(&a.uniform.1),
            a.uniform.2.as_ref().map_or("null".into(), eval_json)
        ),
    ));
    s.push_str(
        &alts
            .iter()
            .map(|(k, v)| format!("\"{k}\":{v}"))
            .collect::<Vec<_>>()
            .join(","),
    );
    s.push_str("},\"causes\":[");
    s.push_str(
        &r.cl
            .causes
            .iter()
            .map(|(k, why)| format!("{{\"class\":\"{k}\",\"evidence\":{}}}", json_str(why)))
            .collect::<Vec<_>>()
            .join(","),
    );
    s.push_str("],\"interventions\":[");
    s.push_str(
        &interventions(a, v, &r.cl)
            .iter()
            .map(|i| {
                format!(
                    "{{\"name\":\"{}\",\"visible_sse_after\":{},\"reduction_pct\":{},\"note\":{},\"vram_delta\":{},\"pack_delta\":{},\"runtime\":{},\"complexity\":{}}}",
                    i.name,
                    i.after_sse.map_or("null".into(), |x| x.to_string()),
                    i.after_sse.map_or("null".into(), |x| f4(100.0 - 100.0 * ratio(x, a.current.eval.vis.sse))),
                    json_str(&i.note),
                    i.vram_delta,
                    i.pack_delta,
                    json_str(i.runtime),
                    json_str(i.complexity)
                )
            })
            .collect::<Vec<_>>()
            .join(","),
    );
    s.push_str("],\"images\":[");
    s.push_str(
        &r.images
            .iter()
            .map(|p| json_str(p))
            .collect::<Vec<_>>()
            .join(","),
    );
    s.push_str("]}");
    s
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

pub(super) struct Options {
    pub json: std::path::PathBuf,
    pub markdown: std::path::PathBuf,
    pub images: std::path::PathBuf,
    pub top_images: usize,
    pub refine: bool,
    pub threads: usize,
    pub pack_path: std::path::PathBuf,
    pub pack_sha256: String,
    pub rom_path: String,
}

pub(super) fn run(opts: &Options) -> Result<(), Box<dyn std::error::Error>> {
    let rec = take().ok_or("residual recorder was not enabled")?;
    let names = Names::load();

    let mut by_variant: BTreeMap<u32, Vec<&UseSite>> = BTreeMap::new();
    for site in &rec.sites {
        by_variant.entry(site.variant).or_default().push(site);
    }
    let unrecorded: Vec<u32> = by_variant
        .keys()
        .copied()
        .filter(|k| !rec.variants.contains_key(k))
        .collect();
    let orphans = rec
        .variants
        .keys()
        .filter(|k| !by_variant.contains_key(k))
        .count();
    let work: Vec<(u32, &Variant, Vec<&UseSite>)> = rec
        .variants
        .iter()
        .filter_map(|(&i, v)| by_variant.get(&i).map(|s| (i, v, s.clone())))
        .collect();
    eprintln!(
        "residuals: {} variants recorded, {} with use sites, {} sites, {} site variants without a recorded conversion, {} orphan variants",
        rec.variants.len(),
        work.len(),
        rec.sites.len(),
        unrecorded.len(),
        orphans
    );

    // Largest first for load balance; results are keyed by variant index.
    let mut order: Vec<usize> = (0..work.len()).collect();
    order.sort_by_key(|&k| {
        let (_, v, s) = &work[k];
        std::cmp::Reverse(
            (v.conversion.source.width * v.conversion.source.height) as usize * s.len(),
        )
    });
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let results: Vec<std::sync::Mutex<Option<Analysis>>> = (0..work.len())
        .map(|_| std::sync::Mutex::new(None))
        .collect();
    std::thread::scope(|scope| {
        for _ in 0..opts.threads.max(1) {
            scope.spawn(|| loop {
                let k = next.fetch_add(1, Ordering::Relaxed);
                if k >= order.len() {
                    break;
                }
                let (i, v, s) = &work[order[k]];
                let a = analyze(*i, v, s, opts.refine);
                *results[order[k]].lock().unwrap() = Some(a);
                let n = done.fetch_add(1, Ordering::Relaxed) + 1;
                if n % 100 == 0 {
                    eprintln!("residuals: {n}/{} variants analysed", order.len());
                }
            });
        }
    });
    let analyses: Vec<Analysis> = results
        .into_iter()
        .map(|m| m.into_inner().unwrap().unwrap())
        .collect();

    let mut rows: Vec<Row<'_>> = analyses
        .iter()
        .zip(&work)
        .map(|(a, (_, v, sites))| {
            let cl = classify(a, v);
            // The visibility weight is that of the most visible use site.
            let (category, weight, primary) = sites
                .iter()
                .map(|s| {
                    let n = names.get(s.file);
                    let (c, w) = category(&n);
                    (
                        c,
                        w,
                        format!("{n} ({}) dl 0x{:X} prim {}", s.file, s.dl, s.prim),
                    )
                })
                .max_by(|x, y| x.1.partial_cmp(&y.1).unwrap().then_with(|| y.2.cmp(&x.2)))
                .unwrap();
            let t = tier(&a.current.eval.vis);
            Row {
                a,
                v,
                severity: severity(&a.current.eval.vis, weight),
                disposition: disposition(a, v, &cl, t),
                cl,
                tier: t,
                category,
                weight,
                primary_name: primary,
                images: Vec::new(),
            }
        })
        .collect();

    // Ranking: severity, then visible SSE, then variant index.
    let mut ranked: Vec<usize> = (0..rows.len())
        .filter(|&k| rows[k].tier == Tier::Material)
        .collect();
    ranked.sort_by(|&x, &y| {
        rows[y]
            .severity
            .partial_cmp(&rows[x].severity)
            .unwrap()
            .then(
                rows[y]
                    .a
                    .current
                    .eval
                    .vis
                    .sse
                    .cmp(&rows[x].a.current.eval.vis.sse),
            )
            .then(rows[x].a.index.cmp(&rows[y].a.index))
    });

    // Diagnostics for the top cases plus every suspected bug.
    let mut image_set: Vec<usize> = ranked.iter().copied().take(opts.top_images).collect();
    for &k in &ranked {
        if rows[k]
            .cl
            .causes
            .iter()
            .any(|(c, _)| *c == "LIKELY_IMPLEMENTATION_BUG")
            && !image_set.contains(&k)
            && image_set.len() < opts.top_images + 20
        {
            image_set.push(k);
        }
    }
    if opts.images.exists() {
        std::fs::remove_dir_all(&opts.images)?;
    }
    for (rank, &k) in ranked.iter().enumerate() {
        if !image_set.contains(&k) {
            continue;
        }
        let r = &rows[k];
        let dir = opts.images.join(format!(
            "{:03}-v{}-f{}-0x{:X}",
            rank + 1,
            r.a.index,
            source_file(r.v),
            r.v.conversion.texture.data_offset
        ));
        let eval = sorted(
            r.a.groups
                .iter()
                .flat_map(|g| g.eval.iter().copied())
                .collect(),
        );
        let paths = write_images(&dir, r.a, r.v, &eval)?;
        rows[k].images = paths;
    }

    // JSON, one record per variant in pack order.
    let mut json = String::new();
    let _ = write!(
        json,
        "{{\"schema\":1,\"rom\":{},\"pack_sha256\":{},\"thresholds\":{{\"material\":\"(pct_ge8>=1 && max>=16) || pct_ge32>=0.1 on the visible error\",\"none\":\"visible max < 8\"}},\"severity\":\"weight * (4*pct_ge32 + 2*pct_ge16 + pct_ge8 + mean)\",\"variants\":[\n",
        json_str(&opts.rom_path),
        json_str(&opts.pack_sha256)
    );
    let mut sorted_rows: Vec<usize> = (0..rows.len()).collect();
    sorted_rows.sort_by_key(|&k| rows[k].a.index);
    for (n, &k) in sorted_rows.iter().enumerate() {
        if n > 0 {
            json.push_str(",\n");
        }
        json.push_str(&row_json(&rows[k], &names));
    }
    json.push_str("\n]}\n");
    if let Some(p) = opts.json.parent() {
        std::fs::create_dir_all(p)?;
    }
    std::fs::write(&opts.json, json)?;

    let md = markdown(
        &rows,
        &ranked,
        &names,
        opts,
        rec.sites.len(),
        orphans,
        &unrecorded,
    );
    if let Some(p) = opts.markdown.parent() {
        std::fs::create_dir_all(p)?;
    }
    std::fs::write(&opts.markdown, md)?;
    eprintln!(
        "residuals: wrote {}, {}, {}",
        opts.json.display(),
        opts.markdown.display(),
        opts.images.display()
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Markdown
// ---------------------------------------------------------------------------

/// Signed reduction from `before` to `after` (negative: worse).
fn reduction(before: u64, after: u64) -> String {
    format!("{:.1}%", 100.0 - 100.0 * ratio(after, before))
}

fn short_causes(cl: &Classified) -> String {
    let set: BTreeSet<&str> = cl.causes.iter().map(|(k, _)| *k).collect();
    CAUSES
        .iter()
        .filter(|k| set.contains(*k))
        .copied()
        .collect::<Vec<_>>()
        .join(", ")
}

fn rgba(c: [u8; 4]) -> String {
    format!("({},{},{},{})", c[0], c[1], c[2], c[3])
}

fn ident(r: &Row<'_>) -> String {
    format!(
        "v{} `{}:0x{:X}`",
        r.a.index,
        source_file(r.v),
        r.v.conversion.texture.data_offset
    )
}

fn stat_cells(s: &Stat) -> String {
    format!(
        "{:.3} | {} | {:.2} | {:.2} | {:.3} | {}",
        s.mean(),
        s.max,
        s.p8(),
        s.p16(),
        s.p32(),
        s.sse
    )
}

fn markdown(
    rows: &[Row<'_>],
    ranked: &[usize],
    names: &Names,
    opts: &Options,
    sites: usize,
    orphans: usize,
    unrecorded: &[u32],
) -> String {
    let mut m = String::new();
    let total = rows.len();
    let none = rows.iter().filter(|r| r.tier == Tier::None).count();
    let minor = rows.iter().filter(|r| r.tier == Tier::Minor).count();
    let mat = ranked.len();
    let mut cause_count: BTreeMap<&str, usize> = BTreeMap::new();
    for &k in ranked {
        let set: BTreeSet<&str> = rows[k].cl.causes.iter().map(|(c, _)| *c).collect();
        for c in set {
            *cause_count.entry(c).or_default() += 1;
        }
    }
    let bug_rows: Vec<usize> = (0..rows.len())
        .filter(|&k| {
            rows[k]
                .cl
                .causes
                .iter()
                .any(|(c, _)| *c == "LIKELY_IMPLEMENTATION_BUG")
                && rows[k].tier != Tier::None
        })
        .collect();
    let sum = |f: &dyn Fn(&Row<'_>) -> u64| rows.iter().map(f).sum::<u64>();

    let _ = writeln!(m, "# N64 3-point filtering: final residual report\n");
    let _ = writeln!(
        m,
        "Generated by `romtool residuals` (report-only; see [Reproduction](#9-exact-reproduction-commands)). \
Pack SHA-256 `{}`. Every number below is measured on each variant's *independent* holdout coverage, \
never the optimizer's training samples. Do not edit by hand: rerun the command.\n",
        opts.pack_sha256
    );

    // 1
    let _ = writeln!(m, "## 1. Executive summary\n");
    let _ = writeln!(
        m,
        "- Mesh texture variants measured: **{total}** ({sites} primitive use sites)."
    );
    let _ = writeln!(
        m,
        "- No meaningful residual (visible max < 8/255): **{none}**."
    );
    let _ = writeln!(
        m,
        "- Minor residual (some samples >= 8/255, below the material threshold): **{minor}**."
    );
    let _ = writeln!(m, "- Materially bad: **{mat}**.");
    let limit = cause_count
        .get("BILINEAR_SURFACE_LIMIT")
        .copied()
        .unwrap_or(0);
    let _ = writeln!(
        m,
        "- Of the materially bad variants, **{limit}** are `BILINEAR_SURFACE_LIMIT`: the best same-resolution RGBA8888 oracle, fitted on the holdout samples themselves, keeps at least half of the current colour error and is still material (or reaches 16/255), or a flip-minimizing alpha search keeps most cutout silhouette flips. Proven per variant, not inferred from compensation failing."
    );
    let _ = writeln!(
        m,
        "- Variants with a suspected implementation bug (any tier above none): **{}**.",
        bug_rows.len()
    );
    let not_attempted = rows.iter().filter(|r| !r.v.conversion.attempted).count();
    let not_attempted_mat = ranked
        .iter()
        .filter(|&&k| !rows[k].v.conversion.attempted)
        .count();
    let _ = writeln!(
        m,
        "- Variants the compensator never attempted (non-paletted source formats and the RE-283 direct-colour bypass): **{not_attempted}**, of which **{not_attempted_mat}** are materially bad.\n"
    );
    let mut disp: BTreeMap<&str, usize> = BTreeMap::new();
    for r in rows {
        *disp.entry(r.disposition).or_default() += 1;
    }
    let _ = writeln!(
        m,
        "Disposition of every variant (JSON field `disposition`):\n"
    );
    let _ = writeln!(m, "| Disposition | Variants | Meaning |\n|---|---:|---|");
    for (d, meaning) in [
        ("clean", "visible max < 8/255"),
        (
            "minor",
            "some samples >= 8/255, below the material threshold",
        ),
        (
            "investigate-bug",
            "a measured implementation inconsistency (section 7); any tier above clean",
        ),
        (
            "manual-intervention",
            "materially bad; a deployable measured alternative meets the bar (section 5a)",
        ),
        (
            "hand-edit-candidate",
            "materially bad; only an oracle-level texture halves the error (section 5b)",
        ),
        (
            "accept",
            "materially bad; nothing measured halves it (section 6)",
        ),
    ] {
        let _ = writeln!(
            m,
            "| `{d}` | {} | {meaning} |",
            disp.get(d).copied().unwrap_or(0)
        );
    }
    m.push('\n');
    let _ = writeln!(
        m,
        "Cause classes over materially bad variants (a variant can carry several):\n"
    );
    let _ = writeln!(m, "| Class | Variants |\n|---|---:|");
    for c in CAUSES {
        let _ = writeln!(
            m,
            "| `{c}` | {} |",
            cause_count.get(c).copied().unwrap_or(0)
        );
    }
    m.push('\n');

    // 2
    let _ = writeln!(m, "## 2. Archive-wide final metrics\n");
    let _ = writeln!(
        m,
        "Summed over all {total} variants and their holdout samples. `visible` is the gameplay-visible error: RGB for opaque; for cutout, RGB where both samples pass the real alpha test and 255 where pass/fail flips; for translucent, premultiplied RGB plus alpha. `max-channel` is the pipeline's policy-scoped raw error (RGB for opaque, RGBA otherwise).\n"
    );
    let _ = writeln!(
        m,
        "| Alternative | Metric | Mean | Max | %>=8 | %>=16 | %>=32 | SSE |\n|---|---|---:|---:|---:|---:|---:|---:|"
    );
    let alts: [(&str, fn(&Analysis) -> Option<&Candidate>); 5] = [
        ("uncompensated `GU_LINEAR`", |a| Some(&a.uncompensated)),
        ("current compensated `GU_LINEAR`", |a| Some(&a.current)),
        ("`GU_NEAREST` (no bias)", |a| Some(&a.nearest)),
        ("RGBA8888 practical", |a| Some(&a.rgba_practical)),
        ("RGBA8888 oracle", |a| Some(&a.rgba_oracle)),
    ];
    for (name, get) in alts {
        for (metric, pick) in [
            ("visible", (|e: &Eval| e.vis) as fn(&Eval) -> Stat),
            ("colour (alpha-test agrees)", |e: &Eval| e.color),
            ("max-channel", |e: &Eval| e.raw),
            ("RGB", |e: &Eval| e.rgb),
            ("alpha", |e: &Eval| e.alpha),
        ] {
            let mut s = Stat::default();
            for r in rows {
                if let Some(c) = get(r.a) {
                    s.merge(&pick(&c.eval));
                }
            }
            let _ = writeln!(m, "| {name} | {metric} | {} |", stat_cells(&s));
        }
    }
    let _ = writeln!(
        m,
        "\nPhase-uniform set (16 samples per holdout cell, all 32 sub-texel residues, no boundary-probe emphasis), visible error:\n"
    );
    let _ = writeln!(
        m,
        "| Alternative | Mean | Max | %>=8 | %>=16 | %>=32 | SSE |\n|---|---:|---:|---:|---:|---:|---:|"
    );
    for (name, pick) in [
        (
            "uncompensated `GU_LINEAR`",
            (|a: &Analysis| Some(a.uniform.1)) as fn(&Analysis) -> Option<Eval>,
        ),
        ("current `GU_LINEAR`", |a: &Analysis| Some(a.uniform.0)),
        (
            "current + best UV shift (where measured)",
            |a: &Analysis| Some(a.uniform.2.unwrap_or(a.uniform.0)),
        ),
    ] {
        let mut st = Stat::default();
        for r in rows {
            if let Some(e) = pick(r.a) {
                st.merge(&e.vis);
            }
        }
        let _ = writeln!(m, "| {name} | {} |", stat_cells(&st));
    }
    let lvl0 = sum(&|r| r.a.current_level0_bytes);
    let pk = sum(&|r| r.a.current_pack_bytes);
    let _ = writeln!(
        m,
        "\nCurrent variants occupy {lvl0} bytes of level 0 (texels + CLUT, the only level the runtime binds) and {pk} bytes of pack texel/CLUT data including the unused mip chain."
    );
    let mut methods: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    for r in rows {
        let e = methods.entry(r.v.conversion.method).or_default();
        e.0 += 1;
        e.1 += usize::from(r.tier == Tier::Material);
    }
    let _ = writeln!(
        m,
        "\n| Compensation method | Variants | Materially bad |\n|---|---:|---:|"
    );
    for (k, (n, b)) in &methods {
        let _ = writeln!(m, "| `{k}` | {n} | {b} |");
    }
    let mut fmts: BTreeMap<(String, &str, &str), (usize, usize)> = BTreeMap::new();
    for r in rows {
        let c = &r.v.conversion;
        let e = fmts
            .entry((
                fmt_name(c.texture.format, c.texture.size),
                psm_name(c.source_psm),
                psm_name(c.final_psm),
            ))
            .or_default();
        e.0 += 1;
        e.1 += usize::from(r.tier == Tier::Material);
    }
    let _ = writeln!(
        m,
        "\n| N64 format | Natural PSP | Final PSP | Variants | Materially bad |\n|---|---|---|---:|---:|"
    );
    for ((f, a, b), (n, bad)) in &fmts {
        let _ = writeln!(m, "| {f} | {a} | {b} | {n} | {bad} |");
    }
    let mut cats: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    for r in rows {
        let e = cats.entry(r.category).or_default();
        e.0 += 1;
        e.1 += usize::from(r.tier == Tier::Material);
    }
    let _ = writeln!(
        m,
        "\n| Most visible use category | Variants | Materially bad |\n|---|---:|---:|"
    );
    for (k, (n, b)) in &cats {
        let _ = writeln!(m, "| {k} | {n} | {b} |");
    }
    let mut pols: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    for r in rows {
        let key = match r.v.conversion.policy {
            AlphaPolicy::Opaque => "opaque",
            AlphaPolicy::Cutout { .. } => "cutout",
            AlphaPolicy::Translucent => "translucent",
        };
        let e = pols.entry(key.into()).or_default();
        e.0 += 1;
        e.1 += usize::from(r.tier == Tier::Material);
    }
    let _ = writeln!(
        m,
        "\n| Alpha mode | Variants | Materially bad |\n|---|---:|---:|"
    );
    for (k, (n, b)) in &pols {
        let _ = writeln!(m, "| {k} | {n} | {b} |");
    }
    let _ = writeln!(
        m,
        "\nThreshold sensitivity (current variants, visible error). The report's material tier is the first row; stricter rows are for triage only:\n"
    );
    let _ = writeln!(m, "| Criterion | Variants |\n|---|---:|");
    let crit: [(&str, fn(&Stat) -> bool); 6] = [
        (
            "material: (%>=8 >= 1 and max >= 16) or %>=32 >= 0.1",
            material,
        ),
        ("%>=32 >= 0.1", |s| s.p32() >= 0.1),
        ("%>=32 >= 1", |s| s.p32() >= 1.0),
        ("%>=32 >= 5", |s| s.p32() >= 5.0),
        ("%>=16 >= 10", |s| s.p16() >= 10.0),
        ("mean >= 8", |s| s.mean() >= 8.0),
    ];
    for (name, f) in crit {
        let _ = writeln!(
            m,
            "| {name} | {} |",
            rows.iter().filter(|r| f(&r.a.current.eval.vis)).count()
        );
    }
    let cut_rows: Vec<&Row<'_>> = rows
        .iter()
        .filter(|r| matches!(r.v.conversion.policy, AlphaPolicy::Cutout { .. }))
        .collect();
    let flips: u64 = cut_rows
        .iter()
        .map(|r| r.a.current.eval.cutout_mismatch)
        .sum();
    let unc_flips: u64 = cut_rows
        .iter()
        .map(|r| r.a.uncompensated.eval.cutout_mismatch)
        .sum();
    let sil_flips: u64 = cut_rows
        .iter()
        .filter_map(|r| r.a.rgba_silhouette_oracle.as_ref())
        .map(|c| c.eval.cutout_mismatch)
        .sum();
    let cut_samples: u64 = cut_rows.iter().map(|r| r.a.current.eval.vis.n).sum();
    let with_flip = cut_rows
        .iter()
        .filter(|r| r.a.uncompensated.eval.cutout_mismatch > 0)
        .count();
    let _ = writeln!(
        m,
        "\nCutout silhouette: {} of {} cutout variants already flip the alpha-test result on some holdout sample when uncompensated, which makes the pipeline's cutout rule (revert alpha on any flip) freeze their alpha. Flips: uncompensated {unc_flips}, current {flips}, flip-minimizing alpha search {sil_flips}, of {cut_samples} cutout holdout samples.",
        with_flip,
        cut_rows.len()
    );
    let _ = writeln!(
        m,
        "\nRecorder integrity: {orphans} packed variants had no recorded use site; {} use-site variants had no recorded conversion{}.\n",
        unrecorded.len(),
        if unrecorded.is_empty() {
            String::new()
        } else {
            format!(" (pack indices {:?})", unrecorded)
        }
    );

    // 3
    let _ = writeln!(m, "## 3. Top residuals ranked by severity\n");
    let _ = writeln!(
        m,
        "Severity = visibility weight x (4 x %>=32 + 2 x %>=16 + %>=8 + mean), on the visible error of the current variant. Weight is the most visible use: fighter 1.0, stage 0.9, 1P-only fighter 0.8, weapon/item/effect 0.7, bonus stage/HUD 0.6, menu/movie 0.5. The full table of all {mat} materially bad variants follows the detailed cards.\n"
    );
    let detailed = 40.min(ranked.len());
    for (rank, &k) in ranked.iter().take(detailed).enumerate() {
        card(&mut m, rank + 1, &rows[k], names);
    }
    let _ = writeln!(m, "### All materially bad variants\n");
    let _ = writeln!(
        m,
        "| # | Variant | Size | Fmt | Alpha | Map | Method | Sev | Mean | Max | %>=8 | %>=16 | %>=32 | SSE | Uncomp SSE | Nearest SSE | RGBA prac SSE | RGBA oracle SSE | Indexed SSE | Causes |\n|---:|---|---|---|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|"
    );
    for (rank, &k) in ranked.iter().enumerate() {
        let r = &rows[k];
        let a = r.a;
        let c = &r.v.conversion;
        let s = &a.current.eval.vis;
        let _ = writeln!(
            m,
            "| {} | {} | {}x{} | {}>{} | {} | {} | `{}` | {:.2} | {:.3} | {} | {:.2} | {:.2} | {:.3} | {} | {} | {} | {} | {} | {} | {} |",
            rank + 1,
            ident(r),
            a.width,
            a.height,
            fmt_name(c.texture.format, c.texture.size),
            psm_name(c.final_psm),
            policy_name(c.policy),
            mapping(a),
            c.method,
            r.severity,
            s.mean(),
            s.max,
            s.p8(),
            s.p16(),
            s.p32(),
            s.sse,
            a.uncompensated.eval.vis.sse,
            a.nearest.eval.vis.sse,
            a.rgba_practical.eval.vis.sse,
            a.rgba_oracle.eval.vis.sse,
            a.indexed
                .as_ref()
                .map_or("-".into(), |x| x.eval.vis.sse.to_string()),
            short_causes(&r.cl)
        );
    }
    m.push('\n');

    // 4
    let _ = writeln!(m, "## 4. Residuals grouped by cause class\n");
    for cause in CAUSES {
        let members: Vec<usize> = ranked
            .iter()
            .copied()
            .filter(|&k| rows[k].cl.causes.iter().any(|(c, _)| *c == cause))
            .collect();
        let _ = writeln!(m, "### `{cause}` ({})\n", members.len());
        if members.is_empty() {
            let _ = writeln!(m, "None.\n");
            continue;
        }
        let vis: u64 = members
            .iter()
            .map(|&k| rows[k].a.current.eval.vis.sse)
            .sum();
        let prac: u64 = members
            .iter()
            .map(|&k| rows[k].a.rgba_practical.eval.vis.sse)
            .sum();
        let orac: u64 = members
            .iter()
            .map(|&k| rows[k].a.rgba_oracle.eval.vis.sse)
            .sum();
        let _ = writeln!(
            m,
            "Summed holdout visible SSE: current {vis}, RGBA practical {prac}, RGBA oracle {orac}.\n"
        );
        let _ = writeln!(m, "| Rank | Variant | Use | Evidence |\n|---:|---|---|---|");
        const CLASS_ROWS: usize = 25;
        for &k in members.iter().take(CLASS_ROWS) {
            let rank = ranked.iter().position(|&x| x == k).unwrap() + 1;
            let why: Vec<&str> = rows[k]
                .cl
                .causes
                .iter()
                .filter(|(c, _)| *c == cause)
                .map(|(_, w)| w.as_str())
                .collect();
            let _ = writeln!(
                m,
                "| {rank} | {} | {} | {} |",
                ident(&rows[k]),
                rows[k].primary_name,
                why.join("; ")
            );
        }
        if members.len() > CLASS_ROWS {
            let _ = writeln!(
                m,
                "\n{} more, in rank order in section 3's full table; per-variant evidence for all of them is in the JSON (`causes[].class == \"{cause}\"`).",
                members.len() - CLASS_ROWS
            );
        }
        m.push('\n');
    }

    // 5 / 6
    let with = |d: &str| -> Vec<usize> {
        ranked
            .iter()
            .copied()
            .filter(|&k| rows[k].disposition == d)
            .collect()
    };
    let fixable = with("manual-intervention");
    let hand = with("hand-edit-candidate");
    let accept = with("accept");
    let _ = writeln!(m, "## 5. Cases likely requiring manual intervention\n");
    let _ = writeln!(
        m,
        "### 5a. A deployable alternative is measured ({})\n\nMaterially bad variants where a deployable measured alternative (not an oracle) halves the holdout visible SSE, or cuts it by >= 25% and leaves the material tier. A UV phase shift counts only when the phase-uniform set confirms a >= 20% gain.\n",
        fixable.len()
    );
    let _ = writeln!(
        m,
        "| Rank | Variant | Use | Best alternative | SSE now | SSE after | Reduction | Tier after | Level-0 delta | Pack delta | Causes |\n|---:|---|---|---|---:|---:|---:|---|---:|---:|---|"
    );
    for &k in &fixable {
        let r = &rows[k];
        let (best, after_tier) = deployable_best(r.a, r.v, &r.cl).unwrap();
        let rank = ranked.iter().position(|&x| x == k).unwrap() + 1;
        let now = r.a.current.eval.vis.sse;
        let after = best.after_sse.unwrap();
        let _ = writeln!(
            m,
            "| {rank} | {} | {} | {} | {now} | {after} | {} | {:?} | {} | {} | {} |",
            ident(r),
            r.primary_name,
            best.name,
            reduction(now, after),
            after_tier,
            best.vram_delta,
            best.pack_delta,
            short_causes(&r.cl)
        );
    }
    let _ = writeln!(
        m,
        "\n### 5b. Hand-edit candidates: only an oracle-level texture reaches it ({})\n\nNo deployable candidate meets the bar, but the best holdout-fitted oracle (RGBA, alpha-held, or cutout silhouette search) at least halves the visible SSE, so a hand-authored or use-site-specific texture could. These oracles are fitted on the samples they are scored on: treat the figure as an upper bound on the gain. Top 40 by severity; the rest have `disposition == \"hand-edit-candidate\"` in the JSON.\n",
        hand.len()
    );
    let _ = writeln!(
        m,
        "| Rank | Variant | Use | SSE now | Oracle bound | Bound reduction | Alpha-test flips now -> bound | Causes |\n|---:|---|---|---:|---:|---:|---|---|"
    );
    for &k in hand.iter().take(40) {
        let r = &rows[k];
        let rank = ranked.iter().position(|&x| x == k).unwrap() + 1;
        let now = r.a.current.eval.vis.sse;
        let b = oracle_bound(r.a);
        let flips = r.a.rgba_silhouette_oracle.as_ref().map_or("-".into(), |x| {
            format!(
                "{} -> {}",
                r.a.current.eval.cutout_mismatch, x.eval.cutout_mismatch
            )
        });
        let _ = writeln!(
            m,
            "| {rank} | {} | {} | {now} | {b} | {} | {flips} | {} |",
            ident(r),
            r.primary_name,
            reduction(now, b),
            short_causes(&r.cl)
        );
    }
    m.push('\n');
    let _ = writeln!(m, "## 6. Cases safe to accept\n");
    let _ = writeln!(
        m,
        "{} materially bad variants have neither a deployable alternative nor an oracle that halves the error: no same-resolution texture edit buys back a material share, so the residual is accepted as the cost of `GU_LINEAR`. Also safe: {minor} minor-tier and {none} clean variants. Breakdown by cause combination, then the 25 highest-severity accepted variants; every accepted variant has `disposition == \"accept\"` in the JSON.\n",
        accept.len()
    );
    let mut combos: BTreeMap<String, (usize, u64, u64)> = BTreeMap::new();
    for &k in &accept {
        let e = combos.entry(short_causes(&rows[k].cl)).or_default();
        e.0 += 1;
        e.1 += rows[k].a.current.eval.vis.sse;
        e.2 += oracle_bound(rows[k].a);
    }
    let mut combos: Vec<_> = combos.into_iter().collect();
    combos.sort_by(|a, b| b.1 .0.cmp(&a.1 .0).then(a.0.cmp(&b.0)));
    let _ = writeln!(
        m,
        "| Causes | Variants | Visible SSE | Oracle bound | Bound reduction |\n|---|---:|---:|---:|---:|"
    );
    for (k, (n, now, b)) in &combos {
        let _ = writeln!(m, "| {k} | {n} | {now} | {b} | {} |", reduction(*now, *b));
    }
    let _ = writeln!(
        m,
        "\n| Rank | Variant | Use | Visible SSE | Oracle bound | Causes |\n|---:|---|---|---:|---:|---|"
    );
    for &k in accept.iter().take(25) {
        let r = &rows[k];
        let rank = ranked.iter().position(|&x| x == k).unwrap() + 1;
        let _ = writeln!(
            m,
            "| {rank} | {} | {} | {} | {} | {} |",
            ident(r),
            r.primary_name,
            r.a.current.eval.vis.sse,
            oracle_bound(r.a),
            short_causes(&r.cl)
        );
    }
    m.push('\n');

    // 7
    let _ = writeln!(
        m,
        "## 7. Suspected implementation bugs requiring investigation\n"
    );
    let _ = writeln!(
        m,
        "All tiers except clean. Each reason is a measured inconsistency, not a guess.\n"
    );
    let mut reasons: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for &k in &bug_rows {
        for (c, why) in &rows[k].cl.causes {
            if *c == "LIKELY_IMPLEMENTATION_BUG" {
                let key = why
                    .split(" (")
                    .next()
                    .unwrap_or(why)
                    .split(';')
                    .next()
                    .unwrap_or(why);
                let key = if key.starts_with("compensation never attempted") {
                    "compensation never attempted for a non-paletted source format".to_string()
                } else if key.contains("holdout samples differ") {
                    "logical-size vs padded GE addressing disagree".to_string()
                } else {
                    key.to_string()
                };
                reasons.entry(key).or_default().push(k);
            }
        }
    }
    for (why, ks) in &reasons {
        let _ = writeln!(m, "### {why} ({} variants)\n", ks.len());
        let _ = writeln!(m, "| Variant | Tier | Use | Detail |\n|---|---|---|---|");
        let mut ks = ks.clone();
        ks.sort_by(|&x, &y| {
            rows[y]
                .severity
                .partial_cmp(&rows[x].severity)
                .unwrap()
                .then(rows[x].a.index.cmp(&rows[y].a.index))
        });
        ks.dedup();
        for k in ks {
            let r = &rows[k];
            let detail: Vec<&str> =
                r.cl.causes
                    .iter()
                    .filter(|(c, _)| *c == "LIKELY_IMPLEMENTATION_BUG")
                    .map(|(_, w)| w.as_str())
                    .collect();
            let _ = writeln!(
                m,
                "| {} | {:?} | {} | {} |",
                ident(r),
                r.tier,
                r.primary_name,
                detail.join("; ")
            );
        }
        m.push('\n');
    }

    // 8
    let _ = writeln!(m, "## 8. Memory/runtime trade-off table\n");
    let _ = writeln!(
        m,
        "Aggregates over the materially bad variants whose intervention applies. Level-0 delta is resident texel+CLUT bytes the runtime binds; pack delta includes the mip chain the packer emits. Reduction is the summed holdout visible SSE.\n"
    );
    let _ = writeln!(
        m,
        "`Blanket` applies the intervention to every materially bad variant it applies to. `Selective` applies it only where it lowers that variant's holdout visible SSE; its memory deltas count only those variants. Oracle-based rows (alpha hand edits) are upper bounds.\n"
    );
    let _ = writeln!(
        m,
        "| Intervention | Applies | Blanket SSE after | Blanket reduction | Blanket level-0 delta | Blanket pack delta | Selective variants | Selective reduction | Selective level-0 delta | Selective pack delta | Runtime cost | Complexity/risk |\n|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|---|"
    );
    #[derive(Default)]
    struct Agg {
        n: usize,
        before: u64,
        after: u64,
        vram: i64,
        pack: i64,
        sel_n: usize,
        sel_after: u64,
        sel_vram: i64,
        sel_pack: i64,
        runtime: &'static str,
        complexity: &'static str,
    }
    let mut agg: BTreeMap<&str, Agg> = BTreeMap::new();
    let total_before: u64 = ranked.iter().map(|&k| rows[k].a.current.eval.vis.sse).sum();
    for &k in ranked {
        let r = &rows[k];
        let now = r.a.current.eval.vis.sse;
        for i in interventions(r.a, r.v, &r.cl) {
            let Some(after) = i.after_sse else { continue };
            let e = agg.entry(i.name).or_default();
            e.runtime = i.runtime;
            e.complexity = i.complexity;
            e.n += 1;
            e.before += now;
            e.after += after;
            e.vram += i.vram_delta;
            e.pack += i.pack_delta;
            if after < now {
                e.sel_n += 1;
                e.sel_after += after;
                e.sel_vram += i.vram_delta;
                e.sel_pack += i.pack_delta;
            } else {
                e.sel_after += now;
            }
        }
    }
    for (name, e) in &agg {
        let _ = writeln!(
            m,
            "| {name} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            e.n,
            e.after,
            reduction(e.before, e.after),
            e.vram,
            e.pack,
            e.sel_n,
            reduction(e.before, e.sel_after),
            e.sel_vram,
            e.sel_pack,
            e.runtime,
            e.complexity
        );
    }
    let _ = writeln!(
        m,
        "\nSummed current holdout visible SSE over all materially bad variants: {total_before}."
    );
    let (u_now, u_shift) = ranked.iter().fold((0u64, 0u64), |(x, y), &k| {
        let u = &rows[k].a.uniform;
        (
            x + u.0.vis.sse,
            y + u.2.as_ref().map_or(u.0.vis.sse, |e| e.vis.sse),
        )
    });
    let _ = writeln!(
        m,
        "\nThe UV-phase row is scored on the holdout, which over-samples the 1/32 boundary phases a shift trades on. On the phase-uniform set the same per-variant shifts change the summed visible SSE by {} ({u_now} -> {u_shift}); only the {} variants that also pass there are counted as deployable in section 5a.",
        reduction(u_now, u_shift),
        ranked.iter().filter(|&&k| rows[k].cl.causes.iter().any(|(c, _)| *c == "SAMPLING_ALIGNMENT")).count()
    );
    m.push('\n');

    // 9
    let _ = writeln!(m, "## 9. Exact reproduction commands\n");
    let _ = writeln!(m, "```bash\ncargo build --release -p romtool");
    let _ = writeln!(
        m,
        "cargo run --release -p romtool -- residuals \"{}\" --top-images {} --threads {}\n```\n",
        opts.rom_path, opts.top_images, opts.threads
    );
    let _ = writeln!(
        m,
        "The command rebuilds the pack to `{}` with the report-only recorder enabled and refuses to report if its SHA-256 differs from `assets/generated/ssb64.pak`. Outputs: this file, `{}` (one record per variant, pack order), and diagnostics under `{}/` (gitignored: derived from the ROM). Output is deterministic; the thread count only changes scheduling. {} integer refinement of the report-only RGBA candidates.\n",
        opts.pack_path.display(),
        opts.json.display(),
        opts.images.display(),
        if opts.refine {
            "With"
        } else {
            "Without (`--no-refine`)"
        }
    );
    let _ = writeln!(m, "Method notes:\n");
    let _ = writeln!(
        m,
        "- Holdout (evaluation) coverage per use site: authored UVs use the cell-local RE-310 holdout; real-normal texgen uses the disjoint-pose RE-311 holdout plus the conservative box lattice; the full-tile fallback uses the phase-4 lattice. Training coverage is never used for measurement."
    );
    let _ = writeln!(
        m,
        "- Reference: `n64_filter::sample_3point_addressed`; PSP: `sample_bilinear_addressed` (RE-304 GE model, +0.5 texel, 4-bit weights); nearest: `sample_point` (no bias)."
    );
    let _ = writeln!(m, "- RGBA8888 practical: the pipeline's own 48-iteration continuous solve without its admission/acceptance gates (`filter_compensation::solve_samples_ungated`, a pack-neutral refactor) on the union training coverage, then the pipeline's `refine_integer`, under the variant's real alpha policy. This is what the packer would ship with the palette, promotion and holdout gates removed.
- RGBA8888 oracle: a report-only box-constrained least-squares fit of the GE 4-bit-weight bilinear model, iterated to convergence (<= 2000 iterations) *on the holdout itself*, then `refine_integer` on the same set; alpha free unless opaque. Because it is fitted on the samples it is scored on, it bounds what any same-resolution texture achieves there (up to solver convergence; the continuous lower bound is reported as `float_sse`). The alpha-held oracle repeats it with alpha fixed; the silhouette oracle (cutout only) adds a flip-minimizing per-texel alpha search.
- Classification uses the best oracle's colour error (samples whose alpha-test result agrees) for `BILINEAR_SURFACE_LIMIT`; cutout alpha-test flips are judged separately against the silhouette oracle.
- Tiny UV phase: the current texture sampled at a uniform (ds, dt)/32-texel offset, ds, dt in -2..2, best visible SSE. It needs a per-material `sceGuTexOffset`.
- Holdout sets include RE-310's targeted boundary probes (diagonal switch, half texel, GE 1/16 boundaries, seams), which over-represent the phases where 3-point and bilinear differ most. Absolute percentages are therefore pessimistic relative to uniformly distributed screen pixels; the ranking and the alternative comparisons use the same sets and are unaffected.");
    let _ = writeln!(
        m,
        "- Indexed candidate: the pipeline's `optimize_palette_indices` (static) or `optimize_animated_indices` (animated) on the training coverage, measured on the holdout."
    );
    let _ = writeln!(
        m,
        "- Pack truth: each variant's level 0 is decoded from the packed bytes (unswizzled) and compared with the recorded final texels; the padded, power-of-two image is also sampled with GE addressing and compared with the logical-size model on every holdout sample."
    );
    let _ = writeln!(
        m,
        "- Not measured: LBParticle bank frames and the fighter shadow texture use a separate converter with no primitive coverage and no compensation; runtime-generated framebuffer textures carry no texels.\n"
    );

    // 10
    let _ = writeln!(m, "## 10. Recommended next investigation order\n");
    let class_n = |c: &str| {
        ranked
            .iter()
            .filter(|&&k| rows[k].cl.causes.iter().any(|(x, _)| *x == c))
            .count()
    };
    let mut n = 0;
    let mut item = |m: &mut String, text: String| {
        n += 1;
        let _ = writeln!(m, "{n}. {text}");
    };
    for (why, ks) in &reasons {
        let material_n = ks
            .iter()
            .filter(|&&k| rows[k].tier == Tier::Material)
            .count();
        item(
            &mut m,
            format!(
                "Suspected bug \"{why}\" ({} variants, {material_n} materially bad; section 7). It changes the reference or the packed texels themselves, so it can move every other number for those variants.",
                ks.len()
            ),
        );
    }
    let never: usize = rows
        .iter()
        .filter(|r| !r.v.conversion.attempted && r.tier == Tier::Material)
        .count();
    item(
        &mut m,
        format!(
            "Decide whether direct-colour sources (RGBA16/RGBA32/IA and the RE-283 bypass) should enter the compensator: {never} materially bad variants never reached it; their per-variant gains are in the `rgba8888_practical`/`rgba8888_dense_training` JSON fields."
        ),
    );
    item(
        &mut m,
        format!(
            "Section 5a, in rank order ({} variants): deployable, measured, mostly free in memory.",
            fixable.len()
        ),
    );
    item(
        &mut m,
        format!(
            "`SAMPLING_ALIGNMENT` ({} variants): confirm on hardware that a per-material (ds, dt)/32 texture offset reproduces the host gain; the effect concentrates on threshold-0 cutouts, where the GE's 4-bit weights drop the small alpha the RDP's 5-bit weights keep.",
            class_n("SAMPLING_ALIGNMENT")
        ),
    );
    item(
        &mut m,
        format!(
            "`ALPHA_CONSTRAINT` cutouts ({} variants): the cutout rule freezes alpha whenever any flip exists; the silhouette search bounds what an alpha-only edit can recover (section 2 flip totals).",
            class_n("ALPHA_CONSTRAINT")
        ),
    );
    item(
        &mut m,
        format!(
            "Training density ({} of the {} `OTHER` variants): the shipped training coverage cannot reach an error the pipeline's own solve reaches with 4 more phase-varied samples per touched cell; the remaining `OTHER` variants matched no class and are listed with their measurements in section 4.",
            ranked
                .iter()
                .filter(|&&k| rows[k].cl.causes.iter().any(|(c, w)| *c == "OTHER" && w.starts_with("training density")))
                .count(),
            class_n("OTHER")
        ),
    );
    item(
        &mut m,
        format!(
            "`CONFLICTING_USE_SITES` ({}), `PALETTE_CONSTRAINT` ({}), `ANIMATED_PALETTE_CONFLICT` ({}), `TEXGEN_COVERAGE` ({}): per-variant decisions with the memory costs in the cards.",
            class_n("CONFLICTING_USE_SITES"),
            class_n("PALETTE_CONSTRAINT"),
            class_n("ANIMATED_PALETTE_CONFLICT"),
            class_n("TEXGEN_COVERAGE")
        ),
    );
    item(
        &mut m,
        format!(
            "Section 5b hand-edit candidates ({}): only after the above, since their bound comes from oracles.",
            hand.len()
        ),
    );
    item(
        &mut m,
        format!(
            "Accept section 6 ({} variants) unless a runtime filter change (not a texture change) is on the table.",
            accept.len()
        ),
    );
    let _ = writeln!(
        m,
        "\nRe-run this report after each step; it is deterministic and refuses to run on a pack that differs from the shipped one.\n"
    );
    m
}

fn card(m: &mut String, rank: usize, r: &Row<'_>, names: &Names) {
    let a = r.a;
    let v = r.v;
    let c = &v.conversion;
    let t = &c.texture;
    let _ = writeln!(
        m,
        "### {rank}. {} - {} ({:.2})\n",
        ident(r),
        r.primary_name,
        r.severity
    );
    let _ = writeln!(m, "| Field | Value |\n|---|---|");
    let _ = writeln!(
        m,
        "| Source | file {} `{}` offset 0x{:X} (display-list home file {}) |",
        source_file(v),
        names.get(source_file(v)),
        t.data_offset,
        c.home
    );
    let _ = writeln!(m, "| Pack variant | texture index {} |", a.index);
    let _ = writeln!(m, "| Used by | {} |", use_sites_text(a, names, 6));
    let _ = writeln!(
        m,
        "| Dimensions | {}x{} (tile {}x{}, source row {}) |",
        a.width, a.height, t.width, t.height, t.source_width
    );
    let _ = writeln!(
        m,
        "| Format | N64 {} -> PSP {} (natural {}), promotion: {} |",
        fmt_name(t.format, t.size),
        psm_name(c.final_psm),
        psm_name(c.source_psm),
        promotion_kind(v)
    );
    let _ = writeln!(m, "| Palette | {} |", palette_identity(v));
    let _ = writeln!(
        m,
        "| Animated | {} |",
        if c.animated.is_some() {
            format!("yes, {} reachable palette states", a.states)
        } else {
            "no".into()
        }
    );
    let _ = writeln!(m, "| Alpha mode | {} |", policy_name(c.policy));
    let _ = writeln!(m, "| Addressing | {} |", addressing(t));
    let _ = writeln!(
        m,
        "| Mapping | {} ({} coverages, {} holdout samples) |",
        mapping(a),
        a.groups.len(),
        a.eval_samples
    );
    let _ = writeln!(m, "| Compensation | `{}` |", c.method);
    let _ = writeln!(
        m,
        "| Cost | level 0 {} B, pack {} B |",
        a.current_level0_bytes, a.current_pack_bytes
    );
    if let Some(w) = a.current.eval.worst {
        let _ = writeln!(
            m,
            "| Worst sample | S10.5 ({}, {}) = texel ({:.3}, {:.3}), state {}: N64 {} vs PSP {} |",
            w.s,
            w.t,
            w.s as f64 / 32.0,
            w.t as f64 / 32.0,
            w.state,
            rgba(w.reference),
            rgba(w.psp)
        );
    }
    let _ = writeln!(m, "| Causes | {} |", short_causes(&r.cl));
    m.push('\n');
    let _ = writeln!(
        m,
        "| Alternative | Mean | Max | %>=8 | %>=16 | %>=32 | Visible SSE | RGB SSE | Alpha SSE | Alpha max | Level 0 B |\n|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|"
    );
    let mut line = |name: &str, x: &Candidate| {
        let e = &x.eval;
        let _ = writeln!(
            m,
            "| {name} | {:.3} | {} | {:.2} | {:.2} | {:.3} | {} | {} | {} | {} | {} |",
            e.vis.mean(),
            e.vis.max,
            e.vis.p8(),
            e.vis.p16(),
            e.vis.p32(),
            e.vis.sse,
            e.rgb.sse,
            e.alpha.sse,
            e.alpha.max,
            x.level0_bytes
        );
    };
    line("uncompensated `GU_LINEAR`", &a.uncompensated);
    line("current `GU_LINEAR`", &a.current);
    line("`GU_NEAREST`", &a.nearest);
    line("RGBA8888 practical", &a.rgba_practical);
    line("RGBA8888 dense-training practical", &a.rgba_dense);
    line("RGBA8888 oracle", &a.rgba_oracle);
    if let Some(x) = &a.rgba_alpha_held_oracle {
        line("RGBA8888 alpha-held oracle", x);
    }
    if let Some(x) = &a.rgba_silhouette_oracle {
        line("RGBA8888 silhouette oracle (cutout)", x);
    }
    if let Some(x) = &a.indexed {
        line(
            if c.animated.is_some() {
                "joint indexed (animated)"
            } else {
                "indexed filter-aware"
            },
            x,
        );
    }
    if let Some((own, shared)) = a.per_site_oracle_vis_sse {
        let _ = writeln!(
            m,
            "\nPer-use-site oracle: {own} visible SSE summed over {} coverages vs {shared} for one shared oracle.",
            a.groups.len()
        );
    }
    if let Some((d, e)) = &a.uv_phase {
        let _ = writeln!(
            m,
            "\nBest uniform UV phase shift ({}, {})/32 texel: holdout visible SSE {} (max {}); phase-uniform set {} -> {}.",
            d[0],
            d[1],
            e.vis.sse,
            e.vis.max,
            a.uniform.0.vis.sse,
            a.uniform.2.as_ref().map_or(0, |u| u.vis.sse)
        );
    }
    m.push('\n');
    for (k, why) in &r.cl.causes {
        let _ = writeln!(m, "- `{k}`: {why}");
    }
    m.push('\n');
    let _ = writeln!(
        m,
        "| Intervention | SSE after | Reduction | Level-0 delta | Pack delta | Runtime | Complexity/risk | Note |\n|---|---:|---:|---:|---:|---|---|---|"
    );
    for i in interventions(a, v, &r.cl) {
        let _ = writeln!(
            m,
            "| {} | {} | {} | {} | {} | {} | {} | {} |",
            i.name,
            i.after_sse.map_or("-".into(), |x| x.to_string()),
            i.after_sse.map_or("-".into(), |x| format!(
                "{:.1}%",
                100.0 - 100.0 * ratio(x, a.current.eval.vis.sse)
            )),
            i.vram_delta,
            i.pack_delta,
            i.runtime,
            i.complexity,
            i.note
        );
    }
    if !r.images.is_empty() {
        let _ = writeln!(
            m,
            "\nDiagnostics (16x16-texel crop around the worst sample, 8 samples/texel; panels: {}): `{}`",
            PANELS.join(", "),
            r.images.last().unwrap()
        );
    }
    m.push('\n');
}

/// SHA-256 of the rebuilt pack, recorded in the report header.
pub(super) fn sha256_hex(data: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut msg = data.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&((data.len() as u64) * 8).to_be_bytes());
    for block in msg.chunks(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes(block[i * 4..i * 4 + 4].try_into().unwrap());
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let mut v = h;
        for i in 0..64 {
            let s1 = v[4].rotate_right(6) ^ v[4].rotate_right(11) ^ v[4].rotate_right(25);
            let ch = (v[4] & v[5]) ^ (!v[4] & v[6]);
            let t1 = v[7]
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = v[0].rotate_right(2) ^ v[0].rotate_right(13) ^ v[0].rotate_right(22);
            let maj = (v[0] & v[1]) ^ (v[0] & v[2]) ^ (v[1] & v[2]);
            let t2 = s0.wrapping_add(maj);
            v = [
                t1.wrapping_add(t2),
                v[0],
                v[1],
                v[2],
                v[3].wrapping_add(t1),
                v[4],
                v[5],
                v[6],
            ];
        }
        for i in 0..8 {
            h[i] = h[i].wrapping_add(v[i]);
        }
    }
    h.iter().map(|x| format!("{x:08x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_known_vectors() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn png_is_deterministic_and_well_formed() {
        let rgb: Vec<u8> = (0..4 * 3 * 3).map(|i| i as u8).collect();
        let a = png(4, 3, &rgb);
        assert_eq!(a, png(4, 3, &rgb));
        assert_eq!(&a[..8], &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn oracle_fit_never_loses_to_the_source_on_its_own_samples() {
        let mut img = Rgba8::new(4, 4);
        for i in 0..16 {
            img.put(
                i,
                [
                    (i * 37 % 256) as u8,
                    (i * 91 % 256) as u8,
                    (i * 13 % 256) as u8,
                    255,
                ],
            );
        }
        let samples = full_tile(4, 4, 4);
        let f = fit(&img, &img, [false, false], &samples, &[0, 1, 2]);
        let before = evaluate(
            &img,
            [false, false],
            &samples,
            AlphaPolicy::Opaque,
            0,
            bilinear(&img, [false, false]),
        );
        let after = evaluate(
            &img,
            [false, false],
            &samples,
            AlphaPolicy::Opaque,
            0,
            bilinear(&f.image, [false, false]),
        );
        assert!(after.vis.sse < before.vis.sse);
    }

    #[test]
    fn packed_level0_decodes_back() {
        let mut img = Rgba8::new(8, 8);
        for i in 0..64 {
            img.put(i, [i as u8 * 3, 255 - i as u8, (i * 7) as u8, 255]);
        }
        let t = psp::pack_mipped(&img, Psm::Psm8888, &[], true);
        let (padded, _) = decode_packed(&t, &[]).unwrap();
        assert_eq!(crop(&padded, 8, 8).pixels, img.pixels);
    }

    #[test]
    fn cutout_flip_is_fully_visible() {
        let p = AlphaPolicy::Cutout {
            greater_or_equal: true,
            threshold: 128,
        };
        let (_, _, _, _, _, _, d, sse) = sample_error(p, [10, 10, 10, 200], [10, 10, 10, 100]);
        assert_eq!((d, sse), (255, 3 * 255 * 255));
        let (_, _, _, _, _, _, d, _) = sample_error(p, [10, 10, 10, 20], [90, 10, 10, 100]);
        assert_eq!(d, 0);
    }
}
