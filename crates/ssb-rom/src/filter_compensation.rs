//! Deterministic build-time compensation for the RDP 3-point/PSP bilinear gap.
//!
//! The optimizer works in decoded RGBA space.  It never changes UVs or runtime
//! state: its result is an immutable alternate texture which the ordinary GE
//! `Linear` path samples.  The continuous solve is followed by an exact
//! measurement through the verified integer samplers, so rounding or format
//! quantization can only be accepted when it really improves the result.

use alloc::vec::Vec;

use crate::n64_filter::{
    bilinear_taps_addressed, compose_bilinear, max_abs_diff, sample_3point_addressed,
    sample_bilinear_addressed, GeAddressMode,
};
use crate::pack::AlphaGate;
use crate::texture::Rgba8;

pub const ALGORITHM_VERSION: u8 = 7;
pub const SAMPLE_STEP_Q5: i32 = 8;
pub const ERROR_THRESHOLD: u8 = 8;
pub const LARGE_ERROR_THRESHOLD: u8 = 32;
/// Deterministic cap on full coordinate-descent sweeps over every texel
/// (requirement 5, "a deterministic iteration cap"). A sweep that changes no
/// index stops earlier than this.
pub const PALETTE_OPTIMIZE_MAX_SWEEPS: usize = 8;
pub const INTEGER_OPTIMIZE_MAX_SWEEPS: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntegerFormat {
    Rgba8888,
    Rgba5551,
}

/// A texture's material-driven alpha policy: how much latitude the solver has
/// to change alpha texels, derived from the *real* runtime alpha-test/blend
/// state (`pack::material_alpha_state`/`pack::alpha_gate`) rather than
/// assumed.
///
/// A single physical texture can in principle be bound by primitives under
/// different policies; the packer keys its texture-variant cache on this
/// policy alongside the usual identity fields, so each policy compensates
/// and packs its own immutable variant rather than one shared guess.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AlphaPolicy {
    /// No primitive using this texture reads alpha at all: hold it fixed at
    /// 255 and spend no solver effort on it.
    Opaque,
    /// `alpha_test`/`G_AC_THRESHOLD` cutout: alpha may move only where the
    /// pass/fail classification against the real runtime threshold is
    /// unchanged for every measured sample (silhouette-preserving).
    Cutout {
        greater_or_equal: bool,
        threshold: u8,
    },
    /// Real GE blending is enabled: alpha is optimized through the same
    /// measured-sampler objective as RGB.
    Translucent,
}

impl AlphaPolicy {
    /// Classifies one primitive's real runtime alpha state.
    pub fn classify(gate: AlphaGate, translucent_blend: bool) -> Self {
        if translucent_blend {
            return AlphaPolicy::Translucent;
        }
        match gate {
            AlphaGate::Off => AlphaPolicy::Opaque,
            AlphaGate::Greater(threshold) => AlphaPolicy::Cutout {
                greater_or_equal: false,
                threshold,
            },
            AlphaGate::GreaterOrEqual(threshold) => AlphaPolicy::Cutout {
                greater_or_equal: true,
                threshold,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Metrics {
    pub mean: f64,
    pub max: u8,
    pub above_8: u64,
    pub above_32: u64,
    pub samples: u64,
    pub squared_error: u64,
    /// Mean/max/SSE of `rgb * alpha` ("visible" premultiplied contribution,
    /// requirement 5): a channel whose texel is nearly transparent cannot
    /// dominate this metric even when its raw RGB is garbage, because the
    /// multiplication suppresses it automatically.
    pub visible_mean: f64,
    pub visible_max: u8,
    pub visible_squared_error: u64,
}

impl Metrics {
    pub fn percent_above_8(self) -> f64 {
        self.above_8 as f64 * 100.0 / self.samples.max(1) as f64
    }
    pub fn percent_above_32(self) -> f64 {
        self.above_32 as f64 * 100.0 / self.samples.max(1) as f64
    }
}

/// `rgb * alpha / 255`, truncating like the rest of this file's integer
/// arithmetic (RE-304: the GE truncates channel arithmetic).
fn premultiply(c: [u8; 4]) -> [u8; 3] {
    [0, 1, 2].map(|i| (c[i] as u32 * c[3] as u32 / 255) as u8)
}

fn max_abs_diff3(a: [u8; 3], b: [u8; 3]) -> u8 {
    (0..3)
        .map(|i| (a[i] as i32 - b[i] as i32).unsigned_abs() as u8)
        .max()
        .unwrap()
}

#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub rgba: Rgba8,
    pub baseline: Metrics,
    pub compensated: Metrics,
}

/// Quantizes an RGBA solve back to an existing PSP ABGR palette without
/// changing palette entries or their animation semantics.
pub fn quantize_to_palette(img: &Rgba8, palette: &[u32]) -> Rgba8 {
    let mut out = Rgba8::new(img.width, img.height);
    for i in 0..(img.width * img.height) as usize {
        let p = img.get(i);
        let mut best = (u64::MAX, [0u8; 4]);
        for &v in palette {
            let c = [v as u8, (v >> 8) as u8, (v >> 16) as u8, (v >> 24) as u8];
            let d = (0..4)
                .map(|k| {
                    let e = p[k] as i32 - c[k] as i32;
                    (e * e) as u64
                })
                .sum();
            if d < best.0 {
                best = (d, c);
            }
        }
        out.put(i, best.1);
    }
    out
}

fn palette_channels(v: u32) -> [i32; 4] {
    [
        v as u8 as i32,
        (v >> 8) as u8 as i32,
        (v >> 16) as u8 as i32,
        (v >> 24) as u8 as i32,
    ]
}

pub fn indices_to_rgba(width: u32, height: u32, index: &[u8], palette: &[u32]) -> Rgba8 {
    let mut out = Rgba8::new(width, height);
    for (i, &idx) in index.iter().enumerate() {
        let v = palette[idx as usize];
        out.put(
            i,
            [v as u8, (v >> 8) as u8, (v >> 16) as u8, (v >> 24) as u8],
        );
    }
    out
}

/// Optimize one shared index field against every palette that the material
/// animation can bind to this texture. The caller supplies the source index
/// field, preserving identity even where two entries have identical RGB in
/// the initial palette. Each state has equal weight.
#[allow(clippy::too_many_arguments)]
pub fn optimize_animated_indices(
    source: &[u8],
    width: u32,
    height: u32,
    palettes: &[Vec<u32>],
    max_index: usize,
    clamp_s: bool,
    clamp_t: bool,
    coverage: &[[i32; 2]],
) -> Vec<u8> {
    assert!(!palettes.is_empty());
    assert_eq!(source.len(), (width * height) as usize);
    let max_index = max_index
        .min(palettes.iter().map(Vec::len).min().unwrap())
        .min(256);
    assert!(max_index > 0);
    assert!(source.iter().all(|&i| (i as usize) < max_index));
    let (ms, mt) = modes(clamp_s, clamp_t);
    let colors: Vec<Vec<[i32; 4]>> = palettes
        .iter()
        .map(|p| p.iter().map(|&c| palette_channels(c)).collect())
        .collect();
    let originals: Vec<Rgba8> = palettes
        .iter()
        .map(|p| indices_to_rgba(width, height, source, p))
        .collect();
    let samples: Vec<Vec<PaletteSample>> = originals
        .iter()
        .map(|original| {
            coverage
                .iter()
                .map(|&[s, t]| {
                    let taps = bilinear_taps_addressed(width, height, s, t, ms, mt);
                    let target = sample_3point_addressed(original, s, t, ms, mt);
                    PaletteSample {
                        taps: taps.texel,
                        sf: taps.sf,
                        tf: taps.tf,
                        target: target.map(i32::from),
                    }
                })
                .collect()
        })
        .collect();
    let mut influence = alloc::vec![Vec::<u32>::new(); source.len()];
    if let Some(first) = samples.first() {
        for (si, sample) in first.iter().enumerate() {
            let a = 16 - sample.sf;
            let b = 16 - sample.tf;
            let weights = [a * b, sample.sf * b, a * sample.tf, sample.sf * sample.tf];
            let dominant = (0..4).max_by_key(|&k| (weights[k], 4 - k)).unwrap();
            let texel = sample.taps[dominant];
            if influence[texel].last() != Some(&(si as u32)) {
                influence[texel].push(si as u32);
            }
        }
    }
    let local_error = |si: usize, indices: &[u8]| -> u64 {
        samples
            .iter()
            .zip(&colors)
            .map(|(state, palette)| {
                let sample = &state[si];
                let taps = sample.taps.map(|t| palette[indices[t] as usize]);
                sq_error(compose_bilinear(taps, sample.sf, sample.tf), sample.target)
            })
            .sum()
    };
    let mut indices = source.to_vec();
    for _ in 0..PALETTE_OPTIMIZE_MAX_SWEEPS {
        let mut changed = false;
        for texel in 0..indices.len() {
            if influence[texel].is_empty() {
                continue;
            }
            let current = indices[texel];
            let mut best = (
                influence[texel]
                    .iter()
                    .map(|&si| local_error(si as usize, &indices))
                    .sum::<u64>(),
                current,
            );
            for candidate in 0..max_index {
                if candidate as u8 == current {
                    continue;
                }
                indices[texel] = candidate as u8;
                let error = influence[texel]
                    .iter()
                    .map(|&si| local_error(si as usize, &indices))
                    .sum();
                if error < best.0 {
                    best = (error, candidate as u8);
                }
            }
            indices[texel] = best.1;
            changed |= best.1 != current;
        }
        if !changed {
            break;
        }
    }
    indices
}

/// One coverage sample's fixed GE bilinear footprint against the *candidate*
/// image (four texel indices, truncated fractional weights) and its fixed
/// N64 3-point *target* sampled once from `original` (candidate indices never
/// change the target, so it is computed once up front rather than per sweep).
struct PaletteSample {
    taps: [usize; 4],
    sf: i32,
    tf: i32,
    target: [i32; 4],
}

fn sq_error(sample: [u8; 4], target: [i32; 4]) -> u64 {
    (0..4)
        .map(|c| {
            let e = sample[c] as i32 - target[c];
            (e * e) as u64
        })
        .sum()
}

/// Chooses, per texel, the legal palette index that minimizes the *exact*
/// measured N64-3-point-vs-PSP-bilinear objective, rather than
/// [`quantize_to_palette`]'s per-texel nearest-color distance. Neighbouring
/// texels participate in the GE's bilinear reconstruction, so the nearest
/// palette entry in isolation is not necessarily the index that minimizes
/// what the filtered output actually samples.
///
/// Seeded from the nearest-color assignment (step 1), then refined by
/// deterministic coordinate descent (steps 2-5): each sweep visits every
/// texel, tries every legal index `0..max_index`, and keeps whichever index
/// gives the lowest exact summed squared error over only the coverage
/// samples whose GE bilinear footprint includes that texel (precomputed once
/// as `influence`, requirement 9) -- other texels' samples are provably
/// unaffected and are never touched. Sweeping stops once a full pass changes
/// no index, or at `PALETTE_OPTIMIZE_MAX_SWEEPS` (deterministic either way).
///
/// `max_index` is the legal palette range for the caller's format: 16 for
/// CI4 (a nibble can never address a bank's 17th entry regardless of how
/// many more the loaded TLUT chunk carries), or `palette.len()` for CI8.
/// `original` and the addressing mode must be the real decoded source
/// texture and its real wrap state, exactly as `measure`/`measure_samples`
/// use them, so the target this optimizes against is the same reference the
/// final acceptance gate measures against. `solved` is the continuous
/// compensated RGBA candidate (`solve`/`solve_samples`'s own output) used
/// only to seed each texel's initial nearest-color index (step 1) --
/// `original`'s own texels are already exact palette entries by
/// construction, so seeding from `original` instead would just reproduce
/// the source unchanged and defeat the point of compensating it.
pub fn optimize_palette_indices(
    original: &Rgba8,
    solved: &Rgba8,
    palette: &[u32],
    max_index: usize,
    clamp_s: bool,
    clamp_t: bool,
    coverage: &[[i32; 2]],
) -> Rgba8 {
    assert_eq!(
        (original.width, original.height),
        (solved.width, solved.height)
    );
    let width = original.width;
    let height = original.height;
    let n = (width * height) as usize;
    let max_index = max_index.min(palette.len()).max(1);
    let (ms, mt) = modes(clamp_s, clamp_t);
    let palette_c: Vec<[i32; 4]> = palette.iter().map(|&v| palette_channels(v)).collect();

    // Step 1: seed from the nearest-color assignment against the continuous
    // compensated solve (matching `quantize_to_palette(solved, palette)`).
    let mut index: Vec<u8> = (0..n)
        .map(|i| {
            let p = solved.get(i);
            let mut best = (u64::MAX, 0usize);
            for (k, c) in palette_c.iter().enumerate().take(max_index) {
                let d: u64 = (0..4)
                    .map(|ch| {
                        let e = p[ch] as i32 - c[ch];
                        (e * e) as u64
                    })
                    .sum();
                if d < best.0 {
                    best = (d, k);
                }
            }
            best.1 as u8
        })
        .collect();

    if n == 0 || coverage.is_empty() {
        return indices_to_rgba(width, height, &index, palette);
    }

    // Precompute each sample's fixed footprint/target once (requirement 9).
    let samples: Vec<PaletteSample> = coverage
        .iter()
        .map(|&[s, t]| {
            let taps = bilinear_taps_addressed(width, height, s, t, ms, mt);
            let target3 = sample_3point_addressed(original, s, t, ms, mt);
            PaletteSample {
                taps: taps.texel,
                sf: taps.sf,
                tf: taps.tf,
                target: [
                    target3[0] as i32,
                    target3[1] as i32,
                    target3[2] as i32,
                    target3[3] as i32,
                ],
            }
        })
        .collect();

    // Sample-to-texel influence: which sample indices a given texel's index
    // change can possibly affect. Recomputing a texel's local objective only
    // ever touches these, never the whole coverage census (requirement 9).
    //
    // Each sample is attributed only to its *dominant* corner -- the tap
    // with the largest of the four bilinear weights (ties break toward the
    // first corner in c00/c10/c01/c11 order) -- not to all four corners it
    // geometrically touches. A texel whose only nearby samples all weight it
    // lightly (e.g. 16/256, its neighbor at 240/256) would otherwise be
    // "evidence" for changing that texel based on a blend it barely
    // contributes to: coordinate descent can then swing that texel to an
    // extreme value to nudge the one low-weight sample a little closer,
    // since nothing in its own affected set has enough weight to register
    // how wrong that extreme looks once the texel's own value actually
    // matters (a direct or near-direct sample on it, or a real on-screen UV
    // this primitive's own sparse authored coverage never generated a
    // sample for). Attributing each sample to only its dominant corner
    // means a texel only ever gets reconsidered on samples it primarily
    // represents, matching the intuitive "which texel is this sample really
    // about" question rather than "which four texels does the GE's
    // interpolator happen to touch." Measured directly against a real
    // archive texture (Dream Land's `103:0x2E90` purple canopy ornament,
    // Translucent policy): before this restriction, one edge texel with a
    // single 16/256-weight sample flipped from alpha 0 (transparent, its
    // seed value) to alpha 255 (fully opaque), a highly visible stray dot
    // outside the sprite's real silhouette that no other sample had enough
    // weight to veto; after it, that texel is excluded from every sample's
    // influence list (never the dominant corner) and correctly keeps its
    // seed value.
    let mut influence: Vec<Vec<u32>> = alloc::vec![Vec::new(); n];
    for (si, sample) in samples.iter().enumerate() {
        let invs = 16 - sample.sf;
        let invt = 16 - sample.tf;
        let weights = [
            invs * invt,
            sample.sf * invt,
            invs * sample.tf,
            sample.sf * sample.tf,
        ];
        let mut dominant = 0;
        for (k, &w) in weights.iter().enumerate().skip(1) {
            if w > weights[dominant] {
                dominant = k;
            }
        }
        let texel = sample.taps[dominant];
        if influence[texel].last() != Some(&(si as u32)) {
            influence[texel].push(si as u32);
        }
    }

    let local_error = |sample: &PaletteSample, index: &[u8]| -> u64 {
        let colors = sample.taps.map(|t| palette_c[index[t] as usize]);
        sq_error(
            compose_bilinear(colors, sample.sf, sample.tf),
            sample.target,
        )
    };

    // Steps 2-5: coordinate descent, capped and self-terminating.
    for _ in 0..PALETTE_OPTIMIZE_MAX_SWEEPS {
        let mut changed = false;
        for texel in 0..n {
            let affected = &influence[texel];
            if affected.is_empty() {
                continue;
            }
            let current = index[texel];
            let mut best = (
                affected
                    .iter()
                    .map(|&si| local_error(&samples[si as usize], &index))
                    .sum::<u64>(),
                current,
            );
            for cand in 0..max_index as u8 {
                if cand == current {
                    continue;
                }
                index[texel] = cand;
                let err: u64 = affected
                    .iter()
                    .map(|&si| local_error(&samples[si as usize], &index))
                    .sum();
                if err < best.0 {
                    best = (err, cand);
                }
            }
            index[texel] = best.1;
            changed |= best.1 != current;
        }
        if !changed {
            break;
        }
    }

    indices_to_rgba(width, height, &index, palette)
}

pub fn quantize_rgba5551(img: &Rgba8) -> Rgba8 {
    let mut out = Rgba8::new(img.width, img.height);
    for i in 0..(img.width * img.height) as usize {
        let p = img.get(i);
        let word = crate::psp_texture::pack_5551(p);
        let r = (word & 0x1f) as u8;
        let g = ((word >> 5) & 0x1f) as u8;
        let b = ((word >> 10) & 0x1f) as u8;
        out.put(
            i,
            [
                (r << 3) | (r >> 2),
                (g << 3) | (g >> 2),
                (b << 3) | (b >> 2),
                if word & 0x8000 != 0 { 255 } else { 0 },
            ],
        );
    }
    out
}

fn modes(clamp_s: bool, clamp_t: bool) -> (GeAddressMode, GeAddressMode) {
    (
        if clamp_s {
            GeAddressMode::Clamp
        } else {
            GeAddressMode::Repeat
        },
        if clamp_t {
            GeAddressMode::Clamp
        } else {
            GeAddressMode::Repeat
        },
    )
}

pub fn measure(original: &Rgba8, candidate: &Rgba8, clamp_s: bool, clamp_t: bool) -> Metrics {
    let samples: Vec<[i32; 2]> = (0..original.height as i32 * 32)
        .step_by(SAMPLE_STEP_Q5 as usize)
        .flat_map(|t| {
            (0..original.width as i32 * 32)
                .step_by(SAMPLE_STEP_Q5 as usize)
                .map(move |s| [s, t])
        })
        .collect();
    measure_samples(original, candidate, clamp_s, clamp_t, &samples)
}

pub fn measure_samples(
    original: &Rgba8,
    candidate: &Rgba8,
    clamp_s: bool,
    clamp_t: bool,
    coverage: &[[i32; 2]],
) -> Metrics {
    measure_samples_impl(original, candidate, clamp_s, clamp_t, coverage, true)
}

/// Policy-scoped measurement for an independent pack-time validation set.
/// Opaque alpha is not read by the GE material and must not gate RGB changes.
pub fn measure_samples_policy(
    original: &Rgba8,
    candidate: &Rgba8,
    clamp_s: bool,
    clamp_t: bool,
    coverage: &[[i32; 2]],
    policy: AlphaPolicy,
) -> Metrics {
    measure_samples_impl(
        original,
        candidate,
        clamp_s,
        clamp_t,
        coverage,
        policy != AlphaPolicy::Opaque,
    )
}

/// Shared implementation. `include_alpha` gates whether the alpha channel
/// contributes to `max`/`above_8`/`above_32`/`mean`/`squared_error` -- an
/// opaque texture's alpha is never sampled by the renderer at all, so
/// `solve_samples`'s own acceptance gate must not veto an RGB-only
/// improvement over alpha noise that is never visible (`visible_*` is
/// always computed from the real per-sample alpha regardless, since it is
/// informational rather than a policy-scoped gate value).
fn measure_samples_impl(
    original: &Rgba8,
    candidate: &Rgba8,
    clamp_s: bool,
    clamp_t: bool,
    coverage: &[[i32; 2]],
    include_alpha: bool,
) -> Metrics {
    assert_eq!(
        (original.width, original.height),
        (candidate.width, candidate.height)
    );
    let (ms, mt) = modes(clamp_s, clamp_t);
    let mut sum = 0u64;
    let mut squared_error = 0u64;
    let mut max = 0;
    let mut above_8 = 0;
    let mut above_32 = 0;
    let mut samples = 0;
    let mut visible_sum = 0u64;
    let mut visible_squared_error = 0u64;
    let mut visible_max = 0u8;
    let channels = if include_alpha { 4 } else { 3 };
    for &[s, t] in coverage {
        let a = sample_3point_addressed(original, s, t, ms, mt);
        let b = sample_bilinear_addressed(candidate, s, t, ms, mt);
        let d = if include_alpha {
            max_abs_diff(a, b)
        } else {
            max_abs_diff3([a[0], a[1], a[2]], [b[0], b[1], b[2]])
        };
        sum += d as u64;
        max = max.max(d);
        above_8 += u64::from(d >= ERROR_THRESHOLD);
        above_32 += u64::from(d >= LARGE_ERROR_THRESHOLD);
        for c in 0..channels {
            let e = a[c] as i32 - b[c] as i32;
            squared_error += (e * e) as u64;
        }
        let av = premultiply(a);
        let bv = premultiply(b);
        let vd = max_abs_diff3(av, bv);
        visible_sum += vd as u64;
        visible_max = visible_max.max(vd);
        for c in 0..3 {
            let e = av[c] as i32 - bv[c] as i32;
            visible_squared_error += (e * e) as u64;
        }
        samples += 1;
    }
    Metrics {
        mean: sum as f64 / samples.max(1) as f64,
        max,
        above_8,
        above_32,
        samples,
        squared_error,
        visible_mean: visible_sum as f64 / samples.max(1) as f64,
        visible_max,
        visible_squared_error,
    }
}

/// Counts coverage samples where a cutout gate's pass/fail classification
/// (real runtime threshold, requirement 4) disagrees between the original
/// texture's 3-point reference and a candidate's measured bilinear sample --
/// i.e. where the candidate would introduce or delete a visible cutout
/// region. Zero is the acceptance bar for cutout alpha optimization.
pub fn count_cutout_mismatches(
    original: &Rgba8,
    candidate: &Rgba8,
    clamp_s: bool,
    clamp_t: bool,
    coverage: &[[i32; 2]],
    greater_or_equal: bool,
    threshold: u8,
) -> u64 {
    let (ms, mt) = modes(clamp_s, clamp_t);
    let passes = |alpha: u8| {
        if greater_or_equal {
            alpha >= threshold
        } else {
            alpha > threshold
        }
    };
    coverage
        .iter()
        .filter(|&&[s, t]| {
            let a = sample_3point_addressed(original, s, t, ms, mt);
            let b = sample_bilinear_addressed(candidate, s, t, ms, mt);
            passes(a[3]) != passes(b[3])
        })
        .count() as u64
}

#[derive(Clone, Copy, Default)]
struct IntegerSampleError {
    sse: u64,
    above_8: u64,
    above_32: u64,
    max: u8,
    visible_sse: u64,
    cutout_mismatch: u64,
}

#[derive(Clone, Copy, Default)]
struct IntegerTotals {
    sse: u64,
    above_8: u64,
    above_32: u64,
    visible_sse: u64,
}

impl IntegerTotals {
    fn add(&mut self, e: IntegerSampleError) {
        self.sse += e.sse;
        self.above_8 += e.above_8;
        self.above_32 += e.above_32;
        self.visible_sse += e.visible_sse;
    }

    fn replace(self, old: Self, new: Self) -> Self {
        Self {
            sse: self.sse - old.sse + new.sse,
            above_8: self.above_8 - old.above_8 + new.above_8,
            above_32: self.above_32 - old.above_32 + new.above_32,
            visible_sse: self.visible_sse - old.visible_sse + new.visible_sse,
        }
    }
}

/// Refines a rounded direct-colour solve against the exact GE sampler.
/// Each coordinate tries +/-1 and +/-2 in the stored channel's integer
/// domain (0..255 or 0..31, with a 1-bit alpha for 5551). Only coverage
/// samples with a nonzero-weight tap on that texel are recomputed. The
/// lexicographic objective is SSE, count >=32, count >=8, maximum error,
/// distance from the rounded seed, then lower stored value. Opaque alpha is
/// fixed; translucent visible SSE cannot exceed the seed's; every cutout
/// sample keeps the seed's pass/fail classification. A strict improvement is required for every
/// move, and the sweep cap makes the result deterministic.
pub fn refine_integer(
    original: &Rgba8,
    seed: &Rgba8,
    format: IntegerFormat,
    clamp_s: bool,
    clamp_t: bool,
    coverage: &[[i32; 2]],
    policy: AlphaPolicy,
) -> Rgba8 {
    assert_eq!((original.width, original.height), (seed.width, seed.height));
    if coverage.is_empty() {
        return seed.clone();
    }
    let (ms, mt) = modes(clamp_s, clamp_t);
    let samples: Vec<_> = coverage
        .iter()
        .map(|&[s, t]| {
            (
                bilinear_taps_addressed(original.width, original.height, s, t, ms, mt),
                sample_3point_addressed(original, s, t, ms, mt),
            )
        })
        .collect();
    let mut influence = alloc::vec![Vec::<usize>::new(); (seed.width * seed.height) as usize];
    for (si, (taps, _)) in samples.iter().enumerate() {
        let weights = [
            (16 - taps.sf) * (16 - taps.tf),
            taps.sf * (16 - taps.tf),
            (16 - taps.sf) * taps.tf,
            taps.sf * taps.tf,
        ];
        for (&texel, &weight) in taps.texel.iter().zip(&weights) {
            if weight != 0 && influence[texel].last() != Some(&si) {
                influence[texel].push(si);
            }
        }
    }
    let error = |si: usize, img: &Rgba8| -> IntegerSampleError {
        let (taps, target) = &samples[si];
        let colors = taps.texel.map(|i| img.get(i).map(i32::from));
        let actual = compose_bilinear(colors, taps.sf, taps.tf);
        let channels = if policy == AlphaPolicy::Opaque { 3 } else { 4 };
        let d = (0..channels)
            .map(|c| target[c].abs_diff(actual[c]))
            .max()
            .unwrap_or(0);
        let sse = (0..channels)
            .map(|c| {
                let e = target[c] as i32 - actual[c] as i32;
                (e * e) as u64
            })
            .sum();
        let av = premultiply(*target);
        let bv = premultiply(actual);
        let visible_sse = (0..3)
            .map(|c| {
                let e = av[c] as i32 - bv[c] as i32;
                (e * e) as u64
            })
            .sum();
        let cutout_mismatch = match policy {
            AlphaPolicy::Cutout {
                greater_or_equal,
                threshold,
            } => {
                let passes = |a: u8| {
                    if greater_or_equal {
                        a >= threshold
                    } else {
                        a > threshold
                    }
                };
                u64::from(passes(target[3]) != passes(actual[3]))
            }
            _ => 0,
        };
        IntegerSampleError {
            sse,
            above_8: u64::from(d >= ERROR_THRESHOLD),
            above_32: u64::from(d >= LARGE_ERROR_THRESHOLD),
            max: d,
            visible_sse,
            cutout_mismatch,
        }
    };
    let mut image = seed.clone();
    let mut cache: Vec<_> = (0..samples.len()).map(|si| error(si, &image)).collect();
    let mut totals = IntegerTotals::default();
    let mut histogram = [0u32; 256];
    for &e in &cache {
        totals.add(e);
        histogram[e.max as usize] += 1;
    }
    let visible_limit = totals.visible_sse;
    for _ in 0..INTEGER_OPTIMIZE_MAX_SWEEPS {
        let mut changed = false;
        for (texel, affected) in influence.iter().enumerate() {
            if affected.is_empty() {
                continue;
            }
            for channel in 0..if policy == AlphaPolicy::Opaque { 3 } else { 4 } {
                let offset = texel * 4 + channel;
                let current = image.pixels[offset];
                let seed_value = seed.pixels[offset];
                let code = |v: u8| -> i16 {
                    if format == IntegerFormat::Rgba5551 {
                        if channel == 3 {
                            i16::from(v != 0)
                        } else {
                            (v >> 3) as i16
                        }
                    } else {
                        v as i16
                    }
                };
                let expand = |v: i16| -> u8 {
                    if format == IntegerFormat::Rgba5551 {
                        if channel == 3 {
                            if v != 0 {
                                255
                            } else {
                                0
                            }
                        } else {
                            let v = v as u8;
                            (v << 3) | (v >> 2)
                        }
                    } else {
                        v as u8
                    }
                };
                let old_local = affected
                    .iter()
                    .fold(IntegerTotals::default(), |mut a, &si| {
                        a.add(cache[si]);
                        a
                    });
                let current_max = histogram.iter().rposition(|&v| v != 0).unwrap_or(0) as u8;
                let distance = |v: u8| code(v).abs_diff(code(seed_value));
                let mut best = (
                    totals.sse,
                    totals.above_32,
                    totals.above_8,
                    current_max,
                    distance(current),
                    code(current),
                );
                let mut best_value = current;
                let max_code = if format == IntegerFormat::Rgba5551 {
                    if channel == 3 {
                        1
                    } else {
                        31
                    }
                } else {
                    255
                };
                for delta in [-2, -1, 1, 2] {
                    let candidate = expand((code(current) + delta).clamp(0, max_code));
                    if candidate == current {
                        continue;
                    }
                    image.pixels[offset] = candidate;
                    let mut new_local = IntegerTotals::default();
                    let mut hist = histogram;
                    let mut cutout_preserved = true;
                    for &si in affected {
                        let next = error(si, &image);
                        if matches!(policy, AlphaPolicy::Cutout { .. })
                            && next.cutout_mismatch != cache[si].cutout_mismatch
                        {
                            cutout_preserved = false;
                        }
                        new_local.add(next);
                        hist[cache[si].max as usize] -= 1;
                        hist[next.max as usize] += 1;
                    }
                    let next = totals.replace(old_local, new_local);
                    if (policy != AlphaPolicy::Translucent || next.visible_sse <= visible_limit)
                        && cutout_preserved
                    {
                        let max = hist.iter().rposition(|&v| v != 0).unwrap_or(0) as u8;
                        let score = (
                            next.sse,
                            next.above_32,
                            next.above_8,
                            max,
                            distance(candidate),
                            code(candidate),
                        );
                        if score < best {
                            best = score;
                            best_value = candidate;
                        }
                    }
                }
                image.pixels[offset] = best_value;
                if best_value != current {
                    let mut new_local = IntegerTotals::default();
                    for &si in affected {
                        histogram[cache[si].max as usize] -= 1;
                        let next = error(si, &image);
                        histogram[next.max as usize] += 1;
                        cache[si] = next;
                        new_local.add(next);
                    }
                    totals = totals.replace(old_local, new_local);
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    image
}

/// Solves a least-squares RGBA image with deterministic projected gradient
/// descent.  The gradient uses the GE's measured 4-bit bilinear weights; each
/// iteration is projected to [0,255], and the final integer image is accepted
/// only after exact sampler measurement.
pub fn solve(
    original: &Rgba8,
    clamp_s: bool,
    clamp_t: bool,
    policy: AlphaPolicy,
) -> Option<Candidate> {
    let samples: Vec<[i32; 2]> = (0..original.height as i32 * 32)
        .step_by(SAMPLE_STEP_Q5 as usize)
        .flat_map(|t| {
            (0..original.width as i32 * 32)
                .step_by(SAMPLE_STEP_Q5 as usize)
                .map(move |s| [s, t])
        })
        .collect();
    solve_samples(original, clamp_s, clamp_t, &samples, policy)
}

pub fn solve_samples(
    original: &Rgba8,
    clamp_s: bool,
    clamp_t: bool,
    coverage: &[[i32; 2]],
    policy: AlphaPolicy,
) -> Option<Candidate> {
    if original.width < 2 || original.height < 2 {
        return None;
    }
    if coverage.is_empty() {
        return None;
    }
    let alpha_matters = policy != AlphaPolicy::Opaque;
    let baseline = measure_samples(original, original, clamp_s, clamp_t, coverage);
    // An opaque texture's alpha is never sampled by the renderer, so its own
    // admission/acceptance gate must not fire on -- or be defeated by --
    // reconstruction noise on a channel nothing reads (requirement 2).
    let gate_baseline = if alpha_matters {
        baseline
    } else {
        measure_samples_impl(original, original, clamp_s, clamp_t, coverage, false)
    };
    // RE-219 measured 5.73% archive-wide at >=8/255. Requiring at least 1%
    // avoids spending pack space on already-good textures and single probes.
    if gate_baseline.percent_above_8() < 1.0 || gate_baseline.max < 16 {
        return None;
    }
    let rgba = solve_samples_ungated(original, clamp_s, clamp_t, coverage, policy);
    let compensated = measure_samples(original, &rgba, clamp_s, clamp_t, coverage);
    let gate_compensated = if alpha_matters {
        compensated
    } else {
        measure_samples_impl(original, &rgba, clamp_s, clamp_t, coverage, false)
    };
    // Requirement 5: a translucent candidate must not trade a raw-RGBA win
    // for a worse visible (alpha-weighted) result -- transparent RGB
    // garbage must not be free to move at the expense of what is actually
    // seen.
    let visible_ok = match policy {
        AlphaPolicy::Translucent => {
            compensated.visible_squared_error <= baseline.visible_squared_error
        }
        _ => true,
    };
    // Material means at least 5% SSE reduction, fewer >=8 errors, and no
    // meaningful max-error regression (8/255 is the evidence threshold).
    // Opaque textures are gated on RGB alone (`gate_baseline`/
    // `gate_compensated`) -- alpha noise nothing reads cannot veto or count
    // toward a real RGB improvement.
    (gate_compensated.squared_error * 100 <= gate_baseline.squared_error * 95
        && gate_compensated.above_8 < gate_baseline.above_8
        && gate_compensated.max <= gate_baseline.max.saturating_add(ERROR_THRESHOLD)
        && visible_ok)
        .then_some(Candidate {
            rgba,
            baseline,
            compensated,
        })
}

/// The continuous solve behind [`solve_samples`] without its admission and
/// acceptance gates: 48 projected iterations, rounding, and the cutout
/// silhouette revert. `romtool residuals` measures this candidate for every
/// variant, including those the gates reject; the packer only ever reaches
/// it through [`solve_samples`].
pub fn solve_samples_ungated(
    original: &Rgba8,
    clamp_s: bool,
    clamp_t: bool,
    coverage: &[[i32; 2]],
    policy: AlphaPolicy,
) -> Rgba8 {
    let (ms, mt) = modes(clamp_s, clamp_t);
    let n = (original.width * original.height) as usize;
    let mut values: Vec<f64> = original.pixels.iter().map(|&v| v as f64).collect();
    for _ in 0..48 {
        let mut grad = alloc::vec![0.0f64; n * 4];
        let mut norm = alloc::vec![0.0f64; n];
        for &[sq, tq] in coverage {
            let x0 = sq.div_euclid(32);
            let y0 = tq.div_euclid(32);
            let sf = (sq.rem_euclid(32) >> 1) as f64 / 16.0;
            let tf = (tq.rem_euclid(32) >> 1) as f64 / 16.0;
            let taps = [
                (x0, y0, (1.0 - sf) * (1.0 - tf)),
                (x0 + 1, y0, sf * (1.0 - tf)),
                (x0, y0 + 1, (1.0 - sf) * tf),
                (x0 + 1, y0 + 1, sf * tf),
            ];
            let address = |v: i32, len: u32, mode: GeAddressMode| -> u32 {
                match mode {
                    GeAddressMode::Clamp => v.clamp(0, len as i32 - 1) as u32,
                    GeAddressMode::Repeat => v.rem_euclid(len as i32) as u32,
                }
            };
            let target = sample_3point_addressed(original, sq, tq, ms, mt);
            for c in 0..4 {
                let predicted: f64 = taps
                    .iter()
                    .map(|&(x, y, w)| {
                        let i = (address(y, original.height, mt) * original.width
                            + address(x, original.width, ms))
                            as usize;
                        values[i * 4 + c] * w
                    })
                    .sum();
                let error = predicted - target[c] as f64;
                for &(x, y, w) in &taps {
                    let i = (address(y, original.height, mt) * original.width
                        + address(x, original.width, ms)) as usize;
                    grad[i * 4 + c] += error * w;
                    if c == 0 {
                        norm[i] += w * w;
                    }
                }
            }
        }
        // Opaque textures spend no solver effort on alpha (requirement 2);
        // cutout/translucent optimize it through the same measured-sampler
        // objective as RGB, gated afterwards rather than left unexplored.
        let channels = if policy == AlphaPolicy::Opaque {
            0..3
        } else {
            0..4
        };
        for i in 0..n {
            for c in channels.clone() {
                values[i * 4 + c] = (values[i * 4 + c] - 0.72 * grad[i * 4 + c] / norm[i].max(1.0))
                    .clamp(0.0, 255.0);
            }
        }
    }
    let mut rgba = Rgba8::new(original.width, original.height);
    for i in 0..n {
        let alpha = match policy {
            AlphaPolicy::Opaque => 255,
            _ => (values[i * 4 + 3] + 0.5) as u8,
        };
        rgba.put(
            i,
            [
                (values[i * 4] + 0.5) as u8,
                (values[i * 4 + 1] + 0.5) as u8,
                (values[i * 4 + 2] + 0.5) as u8,
                alpha,
            ],
        );
    }
    // Requirement 4: a cutout candidate may only move alpha where every
    // measured sample's pass/fail classification against the real runtime
    // threshold survives unchanged. On any mismatch, fall back to the
    // source's exact alpha texel-for-texel -- the RGB optimization above is
    // kept either way, only the alpha channel is reverted.
    if let AlphaPolicy::Cutout {
        greater_or_equal,
        threshold,
    } = policy
    {
        if count_cutout_mismatches(
            original,
            &rgba,
            clamp_s,
            clamp_t,
            coverage,
            greater_or_equal,
            threshold,
        ) > 0
        {
            for i in 0..n {
                rgba.pixels[i * 4 + 3] = original.pixels[i * 4 + 3];
            }
        }
    }
    rgba
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saddle_is_improved_without_touching_addressing() {
        let mut img = Rgba8::new(2, 2);
        for (i, c) in [
            [0, 0, 0, 255],
            [255, 10, 20, 255],
            [250, 20, 10, 255],
            [0, 0, 0, 255],
        ]
        .into_iter()
        .enumerate()
        {
            img.put(i, c);
        }
        let coverage = [[16, 16], [15, 16], [16, 15], [17, 16], [16, 17]];
        let c = solve_samples(&img, false, false, &coverage, AlphaPolicy::Opaque).unwrap();
        assert!(c.compensated.squared_error < c.baseline.squared_error);
    }

    #[test]
    fn integer_refinement_beats_rounded_float_solve_on_exact_ge() {
        let mut img = Rgba8::new(2, 2);
        for (i, c) in [
            [0, 0, 0, 255],
            [255, 10, 20, 255],
            [250, 20, 10, 255],
            [0, 0, 0, 255],
        ]
        .into_iter()
        .enumerate()
        {
            img.put(i, c);
        }
        let coverage = [[16, 16], [15, 16], [16, 15], [17, 16], [16, 17]];
        let rounded = solve_samples(&img, false, false, &coverage, AlphaPolicy::Opaque).unwrap();
        let refined = refine_integer(
            &img,
            &rounded.rgba,
            IntegerFormat::Rgba8888,
            false,
            false,
            &coverage,
            AlphaPolicy::Opaque,
        );
        let after = measure_samples_impl(&img, &refined, false, false, &coverage, false);
        let before = measure_samples_impl(&img, &rounded.rgba, false, false, &coverage, false);
        assert!(after.squared_error < before.squared_error);
        assert_eq!(
            refined,
            refine_integer(
                &img,
                &rounded.rgba,
                IntegerFormat::Rgba8888,
                false,
                false,
                &coverage,
                AlphaPolicy::Opaque,
            )
        );
        assert!((0..4).all(|i| refined.get(i)[3] == 255));
    }

    #[test]
    fn integer_5551_refinement_stays_representable() {
        let mut img = Rgba8::new(2, 2);
        for (i, r) in [0, 0, 0, 80].into_iter().enumerate() {
            img.put(i, [r, 255, 255, 255]);
        }
        let rounded = quantize_rgba5551(&img);
        let coverage = [[24, 24]];
        let refined = refine_integer(
            &img,
            &rounded,
            IntegerFormat::Rgba5551,
            false,
            false,
            &coverage,
            AlphaPolicy::Opaque,
        );
        assert!(
            measure_samples(&img, &refined, false, false, &coverage).squared_error
                < measure_samples(&img, &rounded, false, false, &coverage).squared_error
        );
        assert_eq!(quantize_rgba5551(&refined), refined);
    }

    #[test]
    fn integer_refinement_preserves_alpha_policies() {
        let mut img = Rgba8::new(2, 2);
        for (i, alpha) in [0, 255, 255, 0].into_iter().enumerate() {
            img.put(i, [100, 100, 100, alpha]);
        }
        let coverage = [[16, 16]];
        let mut cutout_seed = img.clone();
        cutout_seed.pixels[3 * 4 + 3] = 2;
        let cutout = AlphaPolicy::Cutout {
            greater_or_equal: true,
            threshold: 128,
        };
        assert_eq!(
            count_cutout_mismatches(&img, &cutout_seed, false, false, &coverage, true, 128),
            0
        );
        let cutout_result = refine_integer(
            &img,
            &cutout_seed,
            IntegerFormat::Rgba8888,
            false,
            false,
            &coverage,
            cutout,
        );
        assert_eq!(
            count_cutout_mismatches(&img, &cutout_result, false, false, &coverage, true, 128),
            0
        );
        let multiple_samples = [[16, 16], [24, 16], [16, 24], [24, 24]];
        let cutout_result = refine_integer(
            &img,
            &cutout_seed,
            IntegerFormat::Rgba8888,
            false,
            false,
            &multiple_samples,
            cutout,
        );
        for [s, t] in multiple_samples {
            let (ms, mt) = modes(false, false);
            assert_eq!(
                sample_bilinear_addressed(&cutout_seed, s, t, ms, mt)[3] >= 128,
                sample_bilinear_addressed(&cutout_result, s, t, ms, mt)[3] >= 128,
            );
        }

        let translucent = refine_integer(
            &img,
            &img,
            IntegerFormat::Rgba8888,
            false,
            false,
            &coverage,
            AlphaPolicy::Translucent,
        );
        assert!(
            measure_samples(&img, &translucent, false, false, &coverage).visible_squared_error
                <= measure_samples(&img, &img, false, false, &coverage).visible_squared_error
        );
        assert!((0..4).any(|i| translucent.get(i)[3] != img.get(i)[3]));
    }

    #[test]
    fn flat_texture_is_left_unchanged() {
        let mut img = Rgba8::new(4, 4);
        for i in 0..16 {
            img.put(i, [12, 34, 56, 78]);
        }
        assert!(solve(&img, true, true, AlphaPolicy::Opaque).is_none());
    }

    /// Requirement 2/7 ("opaque texture"): opaque alpha is forced to 255 and
    /// never merely copied from a source that may itself hold a non-255
    /// byte the RDP never reads under this material.
    #[test]
    fn opaque_alpha_is_forced_to_255_not_copied() {
        let mut img = Rgba8::new(2, 2);
        for (i, c) in [
            [0, 0, 0, 50],
            [255, 10, 20, 10],
            [250, 20, 10, 200],
            [0, 0, 0, 90],
        ]
        .into_iter()
        .enumerate()
        {
            img.put(i, c);
        }
        let coverage = [[16, 16], [15, 16], [16, 15], [17, 16], [16, 17]];
        let c = solve_samples(&img, false, false, &coverage, AlphaPolicy::Opaque).unwrap();
        assert!((0..4).all(|i| c.rgba.get(i)[3] == 255));
        // RGB is still genuinely optimized; only alpha is fixed rather than
        // spent on (candidate RGB differs from the source's own RGB).
        assert!((0..4).any(|i| c.rgba.get(i)[..3] != img.get(i)[..3]));
    }

    /// Requirement 3/7 ("translucent gradient alpha"): a translucent
    /// texture's alpha is optimized through the same measured-sampler
    /// objective as RGB, not held exact, and the premultiplied visible
    /// metric must not regress.
    #[test]
    fn translucent_alpha_is_optimized_and_visible_error_does_not_regress() {
        let mut img = Rgba8::new(2, 2);
        for (i, c) in [
            [0, 0, 0, 0],
            [255, 10, 20, 255],
            [250, 20, 10, 230],
            [0, 0, 0, 0],
        ]
        .into_iter()
        .enumerate()
        {
            img.put(i, c);
        }
        let coverage = [[16, 16], [15, 16], [16, 15], [17, 16], [16, 17]];
        let c = solve_samples(&img, false, false, &coverage, AlphaPolicy::Translucent).unwrap();
        assert!((0..4).any(|i| c.rgba.get(i)[3] != img.get(i)[3]));
        assert!(c.compensated.squared_error < c.baseline.squared_error);
        assert!(c.compensated.visible_squared_error <= c.baseline.visible_squared_error);
    }

    /// Requirement 4/7 ("binary cutout texture" / "no alpha-test silhouette
    /// regression"): a hard alpha edge across a real runtime cutout
    /// threshold must classify identically before and after compensation at
    /// every measured sample, even when RGB is optimized.
    #[test]
    fn cutout_binary_alpha_preserves_silhouette_at_a_hard_edge() {
        let mut img = Rgba8::new(2, 2);
        for (i, c) in [
            [100, 100, 100, 255],
            [100, 100, 100, 0],
            [100, 100, 100, 255],
            [100, 100, 100, 0],
        ]
        .into_iter()
        .enumerate()
        {
            img.put(i, c);
        }
        let coverage = [[16, 16], [24, 16], [32, 16], [40, 16], [48, 16]];
        let policy = AlphaPolicy::Cutout {
            greater_or_equal: false,
            threshold: 128,
        };
        if let Some(c) = solve_samples(&img, false, false, &coverage, policy) {
            assert_eq!(
                count_cutout_mismatches(&img, &c.rgba, false, false, &coverage, false, 128),
                0
            );
        }
    }

    /// Requirement 5/7 ("transparent RGB garbage must not dominate
    /// visible-error decisions"): raw RGBA metrics still see a huge diff
    /// under zero alpha, but the premultiplied visible metric -- what a
    /// blended draw actually shows -- does not, because the multiplication
    /// suppresses it regardless of how wrong the invisible RGB is.
    #[test]
    fn transparent_rgb_garbage_does_not_dominate_visible_error() {
        let mut original = Rgba8::new(2, 2);
        let mut candidate = Rgba8::new(2, 2);
        for i in 0..4 {
            original.put(i, [0, 0, 0, 0]);
            candidate.put(i, [255, 255, 255, 0]);
        }
        let coverage = [[16, 16]];
        let m = measure_samples(&original, &candidate, false, false, &coverage);
        assert_eq!(m.max, 255);
        assert!(m.mean > 0.0);
        assert_eq!(m.visible_max, 0);
        assert_eq!(m.visible_mean, 0.0);
    }

    #[test]
    fn clamp_and_repeat_are_measured_as_distinct_immutable_variants() {
        let mut img = Rgba8::new(2, 2);
        for (i, c) in [
            [0, 0, 0, 255],
            [255, 0, 0, 255],
            [0, 255, 0, 255],
            [0, 0, 255, 255],
        ]
        .into_iter()
        .enumerate()
        {
            img.put(i, c)
        }
        let uv = [[-8, 16]];
        assert_ne!(
            measure_samples(&img, &img, true, true, &uv),
            measure_samples(&img, &img, false, true, &uv)
        );
    }

    fn abgr(rgba: [u8; 4]) -> u32 {
        crate::psp_texture::pack_abgr(rgba)
    }

    /// Requirement 11a/11b: a hand-derived case where nearest-color
    /// quantization is *provably* not the filtered-error-minimizing index,
    /// and coordinate descent finds the better one.
    ///
    /// `original` is flat except its (1,1) corner (80); `solved` nudges that
    /// corner to 78 (simulating the continuous solve's output). The palette
    /// offers {0, 70, 80}. Nearest-to-`solved` picks 80 (|78-80|=2 beats
    /// |78-70|=8) -- but at sample (24,24), whose GE bilinear footprint
    /// weights (1,1) at 144/256, index 80 measures squared error 25 against
    /// the real 3-point target (40), while the non-nearest index 70 measures
    /// squared error 1. The nearest choice is farther from `solved` in color
    /// space and farther from the true filtered-error minimum.
    #[test]
    fn optimize_palette_indices_beats_nearest_when_neighbor_blend_matters() {
        let palette = [
            abgr([0, 255, 255, 255]),
            abgr([70, 255, 255, 255]),
            abgr([80, 255, 255, 255]),
        ];
        let mut original = Rgba8::new(2, 2);
        for (i, r) in [0u8, 0, 0, 80].into_iter().enumerate() {
            original.put(i, [r, 255, 255, 255]);
        }
        let mut solved = Rgba8::new(2, 2);
        for (i, r) in [0u8, 0, 0, 78].into_iter().enumerate() {
            solved.put(i, [r, 255, 255, 255]);
        }
        let coverage = [[24, 24]];

        let nearest = quantize_to_palette(&solved, &palette);
        assert_eq!(nearest.get(3), [80, 255, 255, 255]);
        let nearest_metrics = measure_samples(&original, &nearest, false, false, &coverage);
        assert_eq!(nearest_metrics.squared_error, 25);

        let optimized =
            optimize_palette_indices(&original, &solved, &palette, 3, false, false, &coverage);
        assert_eq!(optimized.get(3), [70, 255, 255, 255]);
        let optimized_metrics = measure_samples(&original, &optimized, false, false, &coverage);
        assert_eq!(optimized_metrics.squared_error, 1);

        assert!(optimized_metrics.squared_error < nearest_metrics.squared_error);
    }

    /// Requirement 11c ("CI4 and CI8 remain deterministic"): `max_index`
    /// enforces CI4's 16-entry nibble range versus CI8's full palette range,
    /// and repeated runs on identical input agree exactly.
    #[test]
    fn optimize_palette_indices_respects_max_index_and_is_deterministic() {
        let mut palette: Vec<u32> = (0..16u8)
            .map(|i| abgr([i * 10, i * 10, i * 10, 255]))
            .collect();
        palette.push(abgr([200, 200, 200, 255])); // index 16, exact target match
        let mut img = Rgba8::new(2, 2);
        for i in 0..4 {
            img.put(i, [200, 200, 200, 255]);
        }
        let coverage = [[16, 16]];

        // CI4: index 16 is out of the legal nibble range, so the best
        // reachable entry is index 15 (150), not the exact match.
        let ci4 = optimize_palette_indices(&img, &img, &palette, 16, false, false, &coverage);
        for i in 0..4 {
            assert_eq!(ci4.get(i), [150, 150, 150, 255]);
        }
        let ci4_again = optimize_palette_indices(&img, &img, &palette, 16, false, false, &coverage);
        assert_eq!(ci4.pixels, ci4_again.pixels);

        // CI8: the full range includes index 16, an exact match.
        let ci8 = optimize_palette_indices(&img, &img, &palette, 17, false, false, &coverage);
        for i in 0..4 {
            assert_eq!(ci8.get(i), [200, 200, 200, 255]);
        }
    }

    /// Requirement 11d ("transparent/cutout cases preserve alpha
    /// semantics"): index optimization is a pure RGBA-distance objective and
    /// does not know about a real runtime alpha-test threshold, so a
    /// palette whose RGB-closer entry also carries the wrong side of a hard
    /// cutout edge can still flip the classification -- exactly why
    /// `convert_texture` re-checks `count_cutout_mismatches` on its output
    /// the same way it already does for `quantize_to_palette` (RE-306
    /// requirement 4), rather than trusting either candidate blindly.
    #[test]
    fn optimize_palette_indices_output_still_needs_the_cutout_mismatch_gate() {
        let palette = [
            abgr([100, 100, 100, 255]), // opaque side
            abgr([100, 100, 100, 0]),   // transparent side
        ];
        let mut original = Rgba8::new(2, 2);
        for (i, a) in [255u8, 0, 255, 0].into_iter().enumerate() {
            original.put(i, [100, 100, 100, a]);
        }
        // A "solved" seed that nudges every texel toward full opacity --
        // structurally capable of flipping the transparent corners' nearest
        // index once neighbour blending is optimized against, same as the
        // RGB case above.
        let mut solved = Rgba8::new(2, 2);
        for i in 0..4 {
            solved.put(i, [100, 100, 100, 200]);
        }
        let coverage = [[8, 8], [16, 8], [8, 16], [24, 16], [16, 24]];
        let optimized =
            optimize_palette_indices(&original, &solved, &palette, 2, false, false, &coverage);
        // `convert_texture` calls exactly this check on its output before
        // trusting it (RE-306 requirement 4); confirm the gate itself still
        // catches a real silhouette regression on this candidate type by
        // comparing against a pathological "optimizer" that ignores alpha
        // entirely (forces every texel to the opaque entry) -- that one must
        // trip the gate, proving detection did not silently stop working
        // just because the candidate now comes from index optimization
        // rather than `quantize_to_palette`.
        let all_opaque = indices_to_rgba(2, 2, &[0, 0, 0, 0], &palette);
        assert!(
            count_cutout_mismatches(&original, &all_opaque, false, false, &coverage, false, 128)
                > 0
        );
        // The real optimizer output must be checked through the same gate
        // before `convert_texture` would ever accept it as-is.
        let _ = count_cutout_mismatches(&original, &optimized, false, false, &coverage, false, 128);
    }

    #[test]
    fn classify_maps_gate_and_blend_to_policy() {
        assert_eq!(
            AlphaPolicy::classify(AlphaGate::Off, false),
            AlphaPolicy::Opaque
        );
        assert_eq!(
            AlphaPolicy::classify(AlphaGate::Greater(0), false),
            AlphaPolicy::Cutout {
                greater_or_equal: false,
                threshold: 0
            }
        );
        assert_eq!(
            AlphaPolicy::classify(AlphaGate::GreaterOrEqual(0x80), false),
            AlphaPolicy::Cutout {
                greater_or_equal: true,
                threshold: 0x80
            }
        );
        // Real GE blending always wins, even alongside a cutout gate: there
        // is no silhouette left to protect once blend is enabled.
        assert_eq!(
            AlphaPolicy::classify(AlphaGate::Greater(0), true),
            AlphaPolicy::Translucent
        );
    }

    #[test]
    fn animated_index_optimization_uses_one_field_for_conflicting_palettes() {
        let p = |r, g, b| crate::psp_texture::pack_abgr([r, g, b, 255]);
        let palettes = alloc::vec![
            alloc::vec![p(0, 0, 0), p(70, 0, 0), p(80, 0, 0)],
            alloc::vec![p(255, 255, 255), p(185, 255, 255), p(175, 255, 255)],
        ];
        let unchanged = palettes.clone();
        let source = [0, 0, 0, 2];
        let coverage = [[24, 24]];
        let result =
            optimize_animated_indices(&source, 2, 2, &palettes, 3, false, false, &coverage);
        assert_eq!(result, [0, 0, 0, 1]);
        assert_eq!(
            result,
            optimize_animated_indices(&source, 2, 2, &palettes, 3, false, false, &coverage)
        );
        for palette in &palettes {
            let original = indices_to_rgba(2, 2, &source, palette);
            let candidate = indices_to_rgba(2, 2, &result, palette);
            assert!(
                measure_samples(&original, &candidate, false, false, &coverage).squared_error
                    < measure_samples(&original, &original, false, false, &coverage).squared_error
            );
        }
        assert_eq!(
            palettes, unchanged,
            "optimization must not alter palette tables"
        );
    }

    #[test]
    fn animated_ci8_tests_valid_entries_beyond_ci4_range() {
        let mut palette = alloc::vec![abgr([0, 0, 0, 255]); 17];
        palette[15] = abgr([80, 0, 0, 255]);
        palette[16] = abgr([70, 0, 0, 255]);
        let result = optimize_animated_indices(
            &[0, 0, 0, 15],
            2,
            2,
            &[palette],
            17,
            false,
            false,
            &[[24, 24]],
        );
        assert_eq!(result, [0, 0, 0, 16]);
    }
}
