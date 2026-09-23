//! Deterministic build-time compensation for the RDP 3-point/PSP bilinear gap.
//!
//! The optimizer works in decoded RGBA space.  It never changes UVs or runtime
//! state: its result is an immutable alternate texture which the ordinary GE
//! `Linear` path samples.  The continuous solve is followed by an exact
//! measurement through the verified integer samplers, so rounding or format
//! quantization can only be accepted when it really improves the result.

use alloc::vec::Vec;

use crate::n64_filter::{
    max_abs_diff, sample_3point_addressed, sample_bilinear_addressed, GeAddressMode,
};
use crate::pack::AlphaGate;
use crate::texture::Rgba8;

pub const ALGORITHM_VERSION: u8 = 2;
pub const SAMPLE_STEP_Q5: i32 = 8;
pub const ERROR_THRESHOLD: u8 = 8;
pub const LARGE_ERROR_THRESHOLD: u8 = 32;

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
        let channels = if policy == AlphaPolicy::Opaque { 0..3 } else { 0..4 };
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
}
