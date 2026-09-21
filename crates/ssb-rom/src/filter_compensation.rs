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
use crate::texture::Rgba8;

pub const ALGORITHM_VERSION: u8 = 1;
pub const SAMPLE_STEP_Q5: i32 = 8;
pub const ERROR_THRESHOLD: u8 = 8;
pub const LARGE_ERROR_THRESHOLD: u8 = 32;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Metrics {
    pub mean: f64,
    pub max: u8,
    pub above_8: u64,
    pub above_32: u64,
    pub samples: u64,
    pub squared_error: u64,
}

impl Metrics {
    pub fn percent_above_8(self) -> f64 {
        self.above_8 as f64 * 100.0 / self.samples.max(1) as f64
    }
    pub fn percent_above_32(self) -> f64 {
        self.above_32 as f64 * 100.0 / self.samples.max(1) as f64
    }
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
    for &[s, t] in coverage {
        let a = sample_3point_addressed(original, s, t, ms, mt);
        let b = sample_bilinear_addressed(candidate, s, t, ms, mt);
        let d = max_abs_diff(a, b);
        sum += d as u64;
        max = max.max(d);
        above_8 += u64::from(d >= ERROR_THRESHOLD);
        above_32 += u64::from(d >= LARGE_ERROR_THRESHOLD);
        for c in 0..3 {
            let e = a[c] as i32 - b[c] as i32;
            squared_error += (e * e) as u64;
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
    }
}

/// Solves a least-squares RGBA image with deterministic projected gradient
/// descent.  The gradient uses the GE's measured 4-bit bilinear weights; each
/// iteration is projected to [0,255], and the final integer image is accepted
/// only after exact sampler measurement.
pub fn solve(original: &Rgba8, clamp_s: bool, clamp_t: bool) -> Option<Candidate> {
    let samples: Vec<[i32; 2]> = (0..original.height as i32 * 32)
        .step_by(SAMPLE_STEP_Q5 as usize)
        .flat_map(|t| {
            (0..original.width as i32 * 32)
                .step_by(SAMPLE_STEP_Q5 as usize)
                .map(move |s| [s, t])
        })
        .collect();
    solve_samples(original, clamp_s, clamp_t, &samples)
}

pub fn solve_samples(
    original: &Rgba8,
    clamp_s: bool,
    clamp_t: bool,
    coverage: &[[i32; 2]],
) -> Option<Candidate> {
    if original.width < 2 || original.height < 2 {
        return None;
    }
    if coverage.is_empty() {
        return None;
    }
    let baseline = measure_samples(original, original, clamp_s, clamp_t, coverage);
    // RE-219 measured 5.73% archive-wide at >=8/255. Requiring at least 1%
    // avoids spending pack space on already-good textures and single probes.
    if baseline.percent_above_8() < 1.0 || baseline.max < 16 {
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
        for i in 0..n {
            for c in 0..3 {
                values[i * 4 + c] = (values[i * 4 + c] - 0.72 * grad[i * 4 + c] / norm[i].max(1.0))
                    .clamp(0.0, 255.0);
            }
        }
    }
    let mut rgba = Rgba8::new(original.width, original.height);
    for i in 0..n {
        rgba.put(
            i,
            [
                (values[i * 4] + 0.5) as u8,
                (values[i * 4 + 1] + 0.5) as u8,
                (values[i * 4 + 2] + 0.5) as u8,
                original.pixels[i * 4 + 3],
            ],
        );
    }
    let compensated = measure_samples(original, &rgba, clamp_s, clamp_t, coverage);
    // Material means at least 5% SSE reduction, fewer >=8 errors, and no
    // meaningful max-error regression (8/255 is the evidence threshold).
    (compensated.squared_error * 100 <= baseline.squared_error * 95
        && compensated.above_8 < baseline.above_8
        && compensated.max <= baseline.max.saturating_add(ERROR_THRESHOLD))
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
        let c = solve_samples(&img, false, false, &coverage).unwrap();
        assert!(c.compensated.squared_error < c.baseline.squared_error);
    }
    #[test]
    fn flat_texture_is_left_unchanged() {
        let mut img = Rgba8::new(4, 4);
        for i in 0..16 {
            img.put(i, [12, 34, 56, 78]);
        }
        assert!(solve(&img, true, true).is_none());
    }

    #[test]
    fn alpha_texels_and_palette_alpha_are_preserved() {
        let mut img = Rgba8::new(2, 2);
        for (i, c) in [
            [0, 0, 0, 0],
            [255, 20, 10, 255],
            [240, 10, 20, 255],
            [0, 0, 0, 0],
        ]
        .into_iter()
        .enumerate()
        {
            img.put(i, c)
        }
        let coverage = [[16, 16], [15, 16], [16, 15], [17, 16], [16, 17]];
        if let Some(c) = solve_samples(&img, false, false, &coverage) {
            assert_eq!(
                (0..4).map(|i| c.rgba.get(i)[3]).collect::<Vec<_>>(),
                vec![0, 255, 255, 0]
            );
        }
        let palette = [0x00000000, 0xff0a14ff];
        let q = quantize_to_palette(&img, &palette);
        assert!(q.pixels.chunks_exact(4).all(|p| p[3] == 0 || p[3] == 255));
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
}
