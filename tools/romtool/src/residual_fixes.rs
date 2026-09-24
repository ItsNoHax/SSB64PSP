//! RE-312 deployment manifest, transcribed from the RE-313-regenerated
//! `three-point-residuals.json` (pack SHA-256
//! 9c3efae3e52bc20ffc80fc9057f0336fddd7f85ddf85a4735779e5cb223d9a3b). These
//! are exact primitive use sites, not a source-texture-wide override. The
//! report's independent holdout and phase-uniform gates selected them.

use std::collections::BTreeSet;
use std::sync::OnceLock;

use ssb_rom::mesh::TextureRef;

use crate::filter_coverage;
use crate::residuals::CrossFit;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    Phase([i16; 2]),
    Dense,
    /// Section 11 `DIRECT_RGBA_OVERRIDE`/`USE_SITE_TEXTURE_VARIANT`: an
    /// immutable RGBA8888 texture fitted at build time on the site's own
    /// phase-uniform set U1 (`residuals::cross_phase_fit`), the fit whose
    /// disjoint-U2 result justified the fix.
    Direct(CrossFit),
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Case {
    pub variant: u16,
    pub source_file: u32,
    pub source_offset: u32,
    /// Decoded/possibly mirror-extended dimensions in the report. The
    /// converter's `TextureRef` can have a narrower source-tile dimension.
    #[allow(dead_code)] // provenance only; mirror extension can change this width
    pub dimensions: [u16; 2],
    pub kind: Kind,
    pub sites: &'static [(u32, u32, usize)],
}

/// The old pre-RE-313 list is deliberately absent. Each identity and phase
/// below comes from the regenerated JSON's `manual-intervention` rows.
///
/// The report's three remaining dense rows (v6 `52:0x2EE8`, v109
/// `71:0x4830`, v111 `71:0x6840`) were re-tested at their exact sites on
/// 2026-09-24 and are deliberately absent: v6 has no solve, and v109/v111
/// stay RGBA5551 through the promotion gate with only 5.7%/4.1% holdout
/// gain against the shipped texels (bar: RGBA8888 and >= 25%).
pub(super) const CASES: &[Case] = &[
    Case {
        variant: 64,
        source_file: 52,
        source_offset: 0x1FCE8,
        dimensions: [40, 8],
        kind: Kind::Phase([1, 1]),
        sites: &[(52, 0x21B50, 7)],
    },
    Case {
        variant: 121,
        source_file: 73,
        source_offset: 0x820,
        dimensions: [32, 32],
        kind: Kind::Dense,
        sites: &[
            (73, 0xD080, 33),
            (73, 0x5F70, 33),
            (73, 0xD080, 35),
            (73, 0x5F70, 35),
        ],
    },
    Case {
        variant: 378,
        source_file: 107,
        source_offset: 0x848,
        dimensions: [64, 64],
        kind: Kind::Phase([1, 0]),
        sites: &[(107, 0x4758, 2)],
    },
    Case {
        variant: 394,
        source_file: 107,
        source_offset: 0x3248,
        dimensions: [160, 24],
        kind: Kind::Phase([1, 1]),
        sites: &[(107, 0x6A30, 0)],
    },
    Case {
        variant: 395,
        source_file: 107,
        source_offset: 0x3248,
        dimensions: [96, 24],
        kind: Kind::Phase([1, 1]),
        sites: &[(107, 0x6A30, 1)],
    },
    Case {
        variant: 416,
        source_file: 108,
        source_offset: 0x46F0,
        dimensions: [64, 64],
        kind: Kind::Phase([1, 1]),
        sites: &[
            (108, 0x9360, 1),
            (108, 0x9358, 1),
            (108, 0x94C8, 1),
            (108, 0xC480, 0),
            (108, 0xC478, 0),
            (108, 0xC480, 2),
            (108, 0xC480, 4),
            (108, 0xC478, 2),
            (108, 0xC478, 4),
        ],
    },
    Case {
        variant: 439,
        source_file: 109,
        source_offset: 0x3000,
        dimensions: [64, 32],
        kind: Kind::Dense,
        sites: &[(109, 0x7678, 9)],
    },
    Case {
        variant: 440,
        source_file: 109,
        source_offset: 0x3000,
        dimensions: [32, 32],
        kind: Kind::Dense,
        sites: &[(109, 0x7678, 10)],
    },
    Case {
        variant: 564,
        source_file: 113,
        source_offset: 0x2EF0,
        dimensions: [64, 16],
        kind: Kind::Phase([0, 1]),
        sites: &[(113, 0x5020, 14)],
    },
    Case {
        variant: 759,
        source_file: 136,
        source_offset: 0x1010,
        dimensions: [32, 32],
        kind: Kind::Phase([0, 1]),
        sites: &[
            (136, 0x3A60, 2),
            (136, 0x5210, 2),
            (136, 0x42B0, 1),
            (136, 0x5890, 1),
            (136, 0x4AE0, 1),
            (136, 0x5EF0, 1),
        ],
    },
    Case {
        variant: 1607,
        source_file: 356,
        source_offset: 0xA0,
        dimensions: [32, 32],
        kind: Kind::Dense,
        sites: &[(356, 0x4C0, 1), (356, 0x560, 0)],
    },
    // Section 11's single MANUAL_FIX_RECOMMENDED row (report at pack SHA-256
    // 97b3f4d8...5707f216): CI4 cutout(a>0), clamp/clamp, one coverage
    // shared by both entry points of the same list. Cross-phase U2 visible
    // SSE 1708580 -> 898375 (47.4%), alpha-test flips 8 -> 4.
    Case {
        variant: 766,
        source_file: 120,
        source_offset: 0x418,
        dimensions: [40, 8],
        kind: Kind::Direct(CrossFit::Silhouette),
        sites: &[(137, 0x28C8, 1), (137, 0x28D0, 1)],
    },
];

/// Dense rows the report measures as deployable but the packer's own
/// format/promotion/holdout gates reject at their exact sites (re-tested
/// 2026-09-24). Keyed by source texture and report dimensions. The report
/// keeps the measurement but no longer counts it as deployable.
pub(super) const DENSE_GATE_REJECTED: &[(u32, u32, [u16; 2], &str)] = &[
    (
        52,
        0x2EE8,
        [32, 32],
        "packer gates: no solve at the listed sites",
    ),
    (
        71,
        0x4830,
        [64, 32],
        "packer gates: stays RGBA5551 (no promotion), holdout -5.7% vs shipped",
    ),
    (
        71,
        0x6840,
        [64, 32],
        "packer gates: stays RGBA5551 (no promotion), holdout -4.1% vs shipped",
    ),
];

pub(super) fn dense_gate_rejected(file: u32, offset: u32, dims: [u32; 2]) -> Option<&'static str> {
    DENSE_GATE_REJECTED
        .iter()
        .find(|&&(f, o, d, _)| f == file && o == offset && [d[0] as u32, d[1] as u32] == dims)
        .map(|&(_, _, _, why)| why)
}

/// Section 11 `VISUAL_REVIEW_REQUIRED` rows as report-only candidate
/// overrides. Never packed unless `romtool pack --residual-candidates`
/// names them; the shipped pack carries none of these. Fit modes follow the
/// triage's fix-class fit: the silhouette fit for alpha-constrained cutouts,
/// otherwise the better of the free and alpha-held fits on U2.
pub(super) const REVIEW_CASES: &[Case] = &[
    review(
        397,
        107,
        0x50,
        [272, 256],
        CrossFit::Free,
        &[(107, 0x6A30, 2), (107, 0x6A30, 4)],
    ),
    review(
        1004,
        157,
        0x6C0,
        [384, 384],
        CrossFit::AlphaHeld,
        &[(157, 0x9D8, 0)],
    ),
    review(
        404,
        108,
        0x6D40,
        [192, 64],
        CrossFit::Free,
        &[(108, 0x7E90, 7)],
    ),
    review(
        252,
        86,
        0x4C18,
        [64, 64],
        CrossFit::Silhouette,
        &[(86, 0x5458, 0), (86, 0x5450, 0)],
    ),
    review(
        837,
        121,
        0x30,
        [288, 400],
        CrossFit::Free,
        &[(140, 0x10F0, 3)],
    ),
    review(
        799,
        121,
        0x30,
        [448, 288],
        CrossFit::Free,
        &[(138, 0x1DB8, 9)],
    ),
    review(
        800,
        121,
        0x30,
        [448, 400],
        CrossFit::Free,
        &[(138, 0x1DB8, 10)],
    ),
    review(
        902,
        121,
        0x30,
        [304, 576],
        CrossFit::Free,
        &[(144, 0x34E0, 33), (144, 0x34D8, 33)],
    ),
    review(
        901,
        121,
        0x30,
        [288, 112],
        CrossFit::Free,
        &[(144, 0x34E0, 32), (144, 0x34D8, 32)],
    ),
    review(
        609,
        117,
        0x7A8,
        [384, 192],
        CrossFit::Free,
        &[(117, 0x1708, 1)],
    ),
    review(
        599,
        116,
        0x21C8,
        [384, 384],
        CrossFit::Free,
        &[(116, 0x3AB0, 6)],
    ),
    review(
        76,
        52,
        0x24228,
        [32, 32],
        CrossFit::Silhouette,
        &[(52, 0x24660, 0)],
    ),
];

const fn review(
    variant: u16,
    source_file: u32,
    source_offset: u32,
    dimensions: [u16; 2],
    fit: CrossFit,
    sites: &'static [(u32, u32, usize)],
) -> Case {
    Case {
        variant,
        source_file,
        source_offset,
        dimensions,
        kind: Kind::Direct(fit),
        sites,
    }
}

static CANDIDATES: OnceLock<Vec<u16>> = OnceLock::new();

/// Report-only: pack the named `REVIEW_CASES` variants too (A/B captures).
pub(super) fn enable_candidates(variants: Vec<u16>) -> Result<(), String> {
    for v in &variants {
        if !REVIEW_CASES.iter().any(|c| c.variant == *v) {
            return Err(format!("v{v} is not a review candidate"));
        }
    }
    CANDIDATES
        .set(variants)
        .map_err(|_| "review candidates already set".to_string())
}

static HIDE_PROBE: OnceLock<bool> = OnceLock::new();

/// Report-only silhouette probe: every packed `Kind::Direct` cutout texel
/// gets alpha 0, so a capture shows the frame without those sites. Frame
/// pixels that differ from this probe are pixels the site's texture drew.
pub(super) fn enable_hide_probe() {
    let _ = HIDE_PROBE.set(true);
}

pub(super) fn hide_probe() -> bool {
    HIDE_PROBE.get().copied().unwrap_or(false)
}

pub(super) fn find(file: u32, dl: u32, prim: usize, tex: &TextureRef) -> Option<&'static Case> {
    let source = tex.data_file.map_or(file, u32::from);
    let enabled = CANDIDATES.get().map_or(&[][..], Vec::as_slice);
    CASES
        .iter()
        .chain(REVIEW_CASES.iter().filter(|c| enabled.contains(&c.variant)))
        .find(|c| {
            c.source_file == source
                && c.source_offset == tex.data_offset
                && c.sites.contains(&(file, dl, prim))
        })
}

/// RE-312's measured method: four phase-varied samples in every cell touched
/// by training or independent validation. Validation points are excluded,
/// then the set is sorted/deduplicated in the report's `(t, s)` order.
pub(super) fn dense_coverage(base: &filter_coverage::Coverage) -> filter_coverage::Coverage {
    let cells: BTreeSet<(i32, i32)> = base
        .train
        .iter()
        .chain(&base.validation)
        .map(|&[s, t]| (t.div_euclid(32), s.div_euclid(32)))
        .collect();
    let validation: BTreeSet<[i32; 2]> = base.validation.iter().copied().collect();
    let mut train = base.train.clone();
    for (ty, tx) in cells {
        let o = (tx * 7 + ty * 11).rem_euclid(16);
        for (i, j) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            let p = [tx * 32 + 16 * i + o, ty * 32 + 16 * j + (o * 5 + 3) % 16];
            if !validation.contains(&p) {
                train.push(p);
            }
        }
    }
    train.sort_unstable_by_key(|p| (p[1], p[0]));
    train.dedup();
    filter_coverage::Coverage {
        train,
        validation: base.validation.clone(),
        texgen: base.texgen.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dense_training_is_deterministic_and_disjoint_from_holdout() {
        let base = filter_coverage::Coverage {
            train: vec![[0, 0], [32, 0]],
            validation: vec![[1, 1], [33, 1], [47, 16]],
            texgen: None,
        };
        let a = dense_coverage(&base);
        let b = dense_coverage(&base);
        assert_eq!(a.train, b.train);
        assert_eq!(a.validation, base.validation);
        assert!(a.train.len() > base.train.len());
        assert!(a.train.iter().all(|p| !a.validation.contains(p)));
        assert!(a
            .train
            .windows(2)
            .all(|w| (w[0][1], w[0][0]) < (w[1][1], w[1][0])));
    }

    #[test]
    fn gate_rejected_dense_rows_are_not_deployed() {
        for &(file, offset, dims, _) in DENSE_GATE_REJECTED {
            assert!(!CASES
                .iter()
                .any(|c| c.source_file == file && c.source_offset == offset));
            assert!(dense_gate_rejected(file, offset, [dims[0] as u32, dims[1] as u32]).is_some());
            assert!(
                dense_gate_rejected(file, offset, [dims[0] as u32 + 1, dims[1] as u32]).is_none()
            );
        }
    }

    #[test]
    fn review_candidates_are_off_unless_named() {
        assert!(CANDIDATES.get().is_none());
        assert!(REVIEW_CASES
            .iter()
            .all(|c| !CASES.iter().any(|d| d.variant == c.variant)));
        assert!(REVIEW_CASES
            .iter()
            .all(|c| matches!(c.kind, Kind::Direct(_))));
    }

    #[test]
    fn manifest_sites_are_unique_and_phases_are_bounded() {
        let mut sites = BTreeSet::new();
        for case in CASES.iter().chain(REVIEW_CASES) {
            assert!(case.dimensions[0] > 0 && case.dimensions[1] > 0);
            for site in case.sites {
                assert!(sites.insert(*site), "duplicate residual fix site {site:?}");
            }
            if let Kind::Phase([s, t]) = case.kind {
                assert!((-2..=2).contains(&s) && (-2..=2).contains(&t));
                assert!(s != 0 || t != 0);
            }
        }
    }
}
