//! Authored-UV coverage for the build-time filter optimizer. All coordinates
//! remain in the vertex stream's integer S10.5 domain.

#[derive(Default, Debug, PartialEq, Eq)]
pub(super) struct Coverage {
    pub train: Vec<[i32; 2]>,
    pub validation: Vec<[i32; 2]>,
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
    let base = [x * 32, y * 32];
    let mut local_train = Vec::new();
    let mut local_validation = Vec::new();
    let mut push = |s: i32, t: i32, validate: bool| {
        if !(0..32).contains(&s) || !(0..32).contains(&t) {
            return;
        }
        let p = [base[0] + s, base[1] + t];
        if inside(uv, p) {
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
                (0..32).contains(&t) && inside(uv, [base[0] + s, base[1] + t])
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
                if inside(uv, p) {
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
}
