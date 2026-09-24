//! Section 11: manual-fix triage of the `hand-edit-candidate` rows.
//!
//! Report-only. Every number here is measured by `analyze`; this module only
//! classifies and ranks. The deciding measurement is the cross-phase bound
//! (`CrossPhase`): a same-resolution texture fitted on the phase-uniform set
//! U1 and scored on the disjoint set U2. The section 5b oracle is fitted on
//! the sparse holdout it is scored on, so it can report a gain that no real
//! frame would see; the cross-phase gain cannot.

use std::fmt::Write as _;

use super::*;

/// Minimum cross-phase SSE reduction for a fix to be worth any bespoke work.
const ACCEPT_BELOW: f64 = 0.25;
/// Minimum cross-phase SSE reduction for a recommended manual fix.
const RECOMMEND_AT: f64 = 0.40;
/// Largest level-0 (resident texture) growth a single bespoke fix may cost.
const MAX_LEVEL0_DELTA: i64 = 64 * 1024;
/// Current cross-phase error must be visible: at least this share of U2
/// samples at >= 16/255, or any alpha-test flip.
const MIN_VISIBLE_PCT16: f64 = 1.0;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum FixClass {
    AlphaEdit,
    UseSiteTextureVariant,
    DirectRgbaOverride,
    PaletteIndexEdit,
    PerPaletteDirectVariant,
    TexgenSpecificVariant,
    AcceptAfterVisualReview,
}

impl FixClass {
    fn name(self) -> &'static str {
        match self {
            FixClass::AlphaEdit => "ALPHA_EDIT",
            FixClass::UseSiteTextureVariant => "USE_SITE_TEXTURE_VARIANT",
            FixClass::DirectRgbaOverride => "DIRECT_RGBA_OVERRIDE",
            FixClass::PaletteIndexEdit => "PALETTE_INDEX_EDIT",
            FixClass::PerPaletteDirectVariant => "PER_PALETTE_DIRECT_VARIANT",
            FixClass::TexgenSpecificVariant => "TEXGEN_SPECIFIC_VARIANT",
            FixClass::AcceptAfterVisualReview => "ACCEPT_AFTER_VISUAL_REVIEW",
        }
    }
    const ALL: [FixClass; 7] = [
        FixClass::AlphaEdit,
        FixClass::UseSiteTextureVariant,
        FixClass::DirectRgbaOverride,
        FixClass::PaletteIndexEdit,
        FixClass::PerPaletteDirectVariant,
        FixClass::TexgenSpecificVariant,
        FixClass::AcceptAfterVisualReview,
    ];
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Bucket {
    Recommended,
    VisualReview,
    Accept,
}

/// Gameplay priority: 1 fighter, 2 normal VS stage, 3 frequent effect,
/// 4 1P/bonus content, 5 menus, movies, unused and demo stages.
fn priority(r: &Row<'_>) -> (u8, &'static str) {
    let name = r.primary_name.as_str();
    const ONE_P_STAGES: [&str; 4] = ["StageMetal", "StageLast", "StageBattlefield", "GRBonus"];
    const NOT_GAMEPLAY: [&str; 2] = ["StagePupupuBeta", "StageExplain"];
    match r.category {
        "fighter" => (1, "fighter"),
        "stage" if NOT_GAMEPLAY.iter().any(|p| name.starts_with(p)) => (5, "unused/demo stage"),
        "stage" if ONE_P_STAGES.iter().any(|p| name.starts_with(p)) => (4, "1P stage"),
        "stage" => (2, "VS stage"),
        "fighter-weapon-effect" | "item-weapon-effect" | "hud" => (3, "effect/item"),
        "fighter-1p-only" | "bonus-stage" => (4, "1P/bonus"),
        _ => (5, "menu/movie"),
    }
}

fn gain(now: u64, after: u64) -> f64 {
    1.0 - ratio(after, now)
}

struct Triage<'r, 'a> {
    r: &'r Row<'a>,
    rank: usize,
    class: FixClass,
    /// The fix class the evidence points at before the transfer check.
    nominal: FixClass,
    bucket: Bucket,
    why: String,
    /// Cross-phase (U2) current and expected visible SSE for `nominal`.
    u2_now: u64,
    u2_after: u64,
    u2_flips: (u64, u64),
    holdout_now: u64,
    holdout_bound: u64,
    level0_delta: i64,
    pack_delta: i64,
    priority: (u8, &'static str),
    other_kind: Option<&'static str>,
}

fn has(r: &Row<'_>, class: &str) -> bool {
    r.cl.causes.iter().any(|(c, _)| *c == class)
}

fn threshold(p: AlphaPolicy) -> Option<String> {
    match p {
        AlphaPolicy::Cutout {
            greater_or_equal,
            threshold,
        } => Some(format!(
            "alpha {} {}",
            if greater_or_equal { ">=" } else { ">" },
            threshold
        )),
        _ => None,
    }
}

fn palette_capacity(r: &Row<'_>) -> Option<usize> {
    let c = &r.v.conversion;
    match c.final_psm {
        Psm::PsmT4 => Some(16),
        Psm::PsmT8 => Some(256),
        _ => None,
    }
}

fn direct_level0(r: &Row<'_>) -> u64 {
    level0_bytes(r.a.width, r.a.height, Psm::Psm8888, 0)
}

fn direct_pack_one(r: &Row<'_>) -> u64 {
    r.a.rgba_oracle.pack_bytes / r.a.states.max(1) as u64
}

fn triage<'r, 'a>(r: &'r Row<'a>, rank: usize) -> Triage<'r, 'a> {
    let a = r.a;
    let c = &r.v.conversion;
    let x = a
        .uniform_transfer
        .cross
        .as_ref()
        .expect("every hand-edit candidate has a cross-phase bound");
    let now = x.current.vis.sse;
    let flips_now = x.current.cutout_mismatch;
    let g = |e: &Eval| gain(now, e.vis.sse);
    let rgba_g = g(&x.rgba);
    let held_g = x.alpha_held.as_ref().map(g);
    let sil_g = x.silhouette.as_ref().map(g);
    let ao_g = x.alpha_only.as_ref().map(|(e, _)| g(e));
    let indexed_g = a
        .indexed
        .as_ref()
        .map(|i| gain(a.current.eval.vis.sse, i.eval.vis.sse));
    let cutout = matches!(c.policy, AlphaPolicy::Cutout { .. });
    let states = a.states as u64;
    let cur_l0 = a.current_level0_bytes as i64;
    let cur_pack = a.current_pack_bytes as i64;
    let direct_l0 = direct_level0(r) as i64;
    let direct_pack = direct_pack_one(r) as i64;
    let texgen = a.groups.iter().any(|g| {
        matches!(
            g.kind,
            CoverageKind::TexgenReal | CoverageKind::TexgenFullTile
        )
    });

    // The best free/held RGBA fit, and its U2 result.
    let best_rgba = [Some((rgba_g, &x.rgba)), held_g.zip(x.alpha_held.as_ref())]
        .into_iter()
        .flatten()
        .max_by(|p, q| p.0.total_cmp(&q.0))
        .unwrap();

    let (nominal, after, flips_after, l0, pack, mut why) = if texgen {
        (
            FixClass::TexgenSpecificVariant,
            best_rgba.1,
            best_rgba.1.cutout_mismatch,
            direct_l0 * states as i64 - cur_l0,
            direct_pack * states as i64 - cur_pack,
            "texgen coverage: one texture serves every pose".to_string(),
        )
    } else if cutout && has(r, "ALPHA_CONSTRAINT") {
        let (ao_eval, colours) = x.alpha_only.as_ref().unwrap();
        let (sil_eval, sil) = (x.silhouette.as_ref().unwrap(), sil_g.unwrap());
        let ao = ao_g.unwrap();
        if ao >= 0.8 * sil.max(rgba_g) && ao > 0.0 {
            // Alpha-only keeps RGB byte-identical. Paletted storage survives
            // when the edited texture still fits the CLUT.
            let fits = palette_capacity(r).is_none_or(|cap| *colours <= cap);
            let (l0, pack) = if fits {
                (0, 0)
            } else {
                (
                    direct_l0 * states as i64 - cur_l0,
                    direct_pack * states as i64 - cur_pack,
                )
            };
            (
                FixClass::AlphaEdit,
                ao_eval,
                ao_eval.cutout_mismatch,
                l0,
                pack,
                format!(
                    "alpha-only search from the shipped texels (RGB untouched) reaches {:.0}% of the silhouette fit; needs {} distinct RGBA values{}",
                    100.0 * ao / sil.max(rgba_g).max(1e-9),
                    colours,
                    palette_capacity(r).map_or(String::new(), |cap| format!(
                        " vs CLUT capacity {cap} ({})",
                        if fits { "fits: index/CLUT edit" } else { "exceeds: direct RGBA" }
                    ))
                ),
            )
        } else {
            (
                FixClass::DirectRgbaOverride,
                sil_eval,
                sil_eval.cutout_mismatch,
                direct_l0 * states as i64 - cur_l0,
                direct_pack * states as i64 - cur_pack,
                format!(
                    "alpha-only reaches {:.0}% vs silhouette with refitted RGB {:.0}%: RGB must change too",
                    100.0 * ao,
                    100.0 * sil
                ),
            )
        }
    } else if has(r, "CONFLICTING_USE_SITES") && a.groups.len() >= 2 {
        let n = a.groups.len() as i64;
        (
            FixClass::UseSiteTextureVariant,
            best_rgba.1,
            best_rgba.1.cutout_mismatch,
            direct_l0 * n * states as i64 - cur_l0,
            direct_pack * n * states as i64 - cur_pack,
            format!("{} distinct use-site coverages", a.groups.len()),
        )
    } else if states > 1 {
        if indexed_g.is_some_and(|ig| ig >= ACCEPT_BELOW) {
            (
                FixClass::PaletteIndexEdit,
                best_rgba.1,
                best_rgba.1.cutout_mismatch,
                0,
                0,
                format!(
                    "one index field over {} palette states already cuts holdout SSE {:.0}%",
                    states,
                    100.0 * indexed_g.unwrap()
                ),
            )
        } else {
            (
                FixClass::PerPaletteDirectVariant,
                best_rgba.1,
                best_rgba.1.cutout_mismatch,
                direct_l0 * states as i64 - cur_l0,
                direct_pack * states as i64 - cur_pack,
                format!(
                    "{} palette states; index-only optimum {}",
                    states,
                    indexed_g.map_or("unavailable".into(), |ig| format!("{:.0}%", 100.0 * ig))
                ),
            )
        }
    } else if c.final_psm.is_paletted() && indexed_g.is_some_and(|ig| ig >= ACCEPT_BELOW) {
        (
            FixClass::PaletteIndexEdit,
            best_rgba.1,
            best_rgba.1.cutout_mismatch,
            0,
            0,
            format!(
                "index-only optimum already cuts holdout SSE {:.0}%",
                100.0 * indexed_g.unwrap()
            ),
        )
    } else {
        let promoted = c.final_psm == Psm::Psm8888;
        (
            FixClass::DirectRgbaOverride,
            best_rgba.1,
            best_rgba.1.cutout_mismatch,
            if promoted { 0 } else { direct_l0 - cur_l0 },
            if promoted { 0 } else { direct_pack - cur_pack },
            format!(
                "RGBA8888 fit{}",
                indexed_g.map_or(String::new(), |ig| format!(
                    "; index-only optimum {:.0}%",
                    100.0 * ig
                ))
            ),
        )
    };

    let expected = gain(now, after.vis.sse);
    let other_kind = has(r, "OTHER").then(|| {
        let own_shared = a.per_site_oracle_vis_sse;
        if expected < ACCEPT_BELOW {
            "true bilinear surface limitation"
        } else if own_shared.is_some_and(|(o, s)| o * 4 <= s * 3) {
            "use-site conflict"
        } else if held_g.is_some_and(|h| h + 0.1 < rgba_g) {
            "alpha-limited"
        } else if c.final_psm.is_paletted() && indexed_g.is_none_or(|ig| ig < 0.1) {
            "quantization-limited"
        } else if gain(a.uniform.0.vis.sse, a.uniform_transfer.dense.vis.sse) >= 0.5 * expected {
            "coverage-limited"
        } else {
            "quantization-limited"
        }
    });

    let visible = x.current.vis.p16() >= MIN_VISIBLE_PCT16 || flips_now > 0;
    let pr = priority(r);
    let (class, bucket) = if expected < ACCEPT_BELOW {
        why = format!(
            "{why}; cross-phase gain {:.0}% < {:.0}%: the holdout oracle's {:.0}% does not transfer",
            100.0 * expected,
            100.0 * ACCEPT_BELOW,
            100.0 * gain(a.current.eval.vis.sse, oracle_bound(a))
        );
        (FixClass::AcceptAfterVisualReview, Bucket::Accept)
    } else if !visible {
        why = format!(
            "{why}; current cross-phase error is below visibility ({:.2}% >=16, no flips)",
            x.current.vis.p16()
        );
        (FixClass::AcceptAfterVisualReview, Bucket::Accept)
    } else if expected >= RECOMMEND_AT && l0 <= MAX_LEVEL0_DELTA && pr.0 <= 4 {
        (nominal, Bucket::Recommended)
    } else {
        if expected < RECOMMEND_AT {
            why = format!(
                "{why}; cross-phase gain {:.0}% below {:.0}%",
                100.0 * expected,
                100.0 * RECOMMEND_AT
            );
        }
        if l0 > MAX_LEVEL0_DELTA {
            why = format!("{why}; level-0 cost {l0} B above {MAX_LEVEL0_DELTA} B");
        }
        if pr.0 > 4 {
            why = format!("{why}; {} only", pr.1);
        }
        (nominal, Bucket::VisualReview)
    };

    Triage {
        r,
        rank,
        class,
        nominal,
        bucket,
        why,
        u2_now: now,
        u2_after: after.vis.sse,
        u2_flips: (flips_now, flips_after),
        holdout_now: a.current.eval.vis.sse,
        holdout_bound: oracle_bound(a),
        level0_delta: l0,
        pack_delta: pack,
        priority: pr,
        other_kind,
    }
}

fn runtime_cost(c: FixClass, r: &Row<'_>, l0: i64) -> &'static str {
    match c {
        FixClass::AlphaEdit if l0 == 0 => "none (same format and CLUT size)",
        FixClass::PaletteIndexEdit => "none",
        FixClass::UseSiteTextureVariant => {
            "one extra texture bind per split use site; 32-bit texel fetch"
        }
        FixClass::PerPaletteDirectVariant => {
            "texture swap per palette change instead of CLUT reload"
        }
        _ if r.v.conversion.final_psm == Psm::Psm8888 => "none (already RGBA8888)",
        _ => "32-bit texel fetch",
    }
}

fn approach(t: &Triage<'_, '_>) -> String {
    let r = t.r;
    let a = r.a;
    let c = &r.v.conversion;
    let sites = a
        .groups
        .iter()
        .flat_map(|g| g.prims.iter())
        .map(|&(f, dl, p, _)| format!("{f}:0x{dl:X}#{p}"))
        .collect::<Vec<_>>()
        .join(", ");
    match t.nominal {
        FixClass::AlphaEdit => format!(
            "offline flip-minimizing alpha search from the shipped texels on a dense phase-uniform set ({}; candidates 0/thr/thr+1/255/source); keep RGB and {}; add as an immutable exact-use-site variant at {sites} through the residual-fix manifest; gate on a disjoint phase set",
            threshold(c.policy).unwrap_or_default(),
            if t.level0_delta == 0 { "the index format, re-indexing the CLUT" } else { "store RGBA8888" }
        ),
        FixClass::DirectRgbaOverride => format!(
            "offline RGBA8888 fit on a dense phase-uniform set (U1-style, 16/cell), GE-exact integer refinement, gate on a disjoint phase set; immutable exact-use-site variant at {sites}"
        ),
        FixClass::UseSiteTextureVariant => format!(
            "split into {} immutable per-coverage textures (one fit per site group: {sites}); no shared texels change",
            a.groups.len()
        ),
        FixClass::PaletteIndexEdit => format!(
            "re-optimize the index field on a dense phase-uniform set over all {} palette state(s); CLUT unchanged; variant at {sites}",
            a.states
        ),
        FixClass::PerPaletteDirectVariant => format!(
            "{} direct RGBA8888 textures, one per palette state; runtime maps the material's palette index to a texture index at {sites}",
            a.states
        ),
        FixClass::TexgenSpecificVariant => {
            format!("pose-specific variant; no single texture covers every pose at {sites}")
        }
        FixClass::AcceptAfterVisualReview => "none".into(),
    }
}

fn crop(r: &Row<'_>) -> String {
    r.images
        .first()
        .and_then(|p| std::path::Path::new(p).parent())
        .map_or("-".into(), |p| format!("`{}/`", p.display()))
}

fn pct(now: u64, after: u64) -> String {
    format!("{:.1}%", 100.0 * gain(now, after))
}

pub(super) fn section(m: &mut String, rows: &[Row<'_>], ranked: &[usize], names: &Names) {
    let all: Vec<Triage<'_, '_>> = ranked
        .iter()
        .enumerate()
        .filter(|(_, &k)| rows[k].disposition == "hand-edit-candidate")
        .map(|(rank, &k)| triage(&rows[k], rank + 1))
        .collect();
    let deployable = ranked
        .iter()
        .filter(|&&k| rows[k].disposition == "manual-intervention")
        .count();

    let _ = writeln!(m, "## 11. Manual-fix triage of hand-edit candidates\n");
    let _ = writeln!(
        m,
        "Only `disposition == \"hand-edit-candidate\"` rows ({}). Section 5b's oracle is fitted on the sparse holdout it is scored on (typically 1-2 samples per touched texel cell), so its bound mixes real gain with overfit. Every decision below uses the **cross-phase bound** instead: the same same-resolution fit made on the phase-uniform set U1 (16 samples per holdout cell, all 32 sub-texel residues) and scored on U2, the same cells shifted by (4, 4)/32 texel, disjoint from U1. U2 approximates uniformly distributed screen pixels; a gain that survives there is a gain a real frame shows. JSON: `alternatives.cross_phase`, plus `phase_uniform_set.*` for the holdout-fitted images scored on U1.\n",
        all.len()
    );
    let _ = writeln!(
        m,
        "Buckets: **ACCEPT** when the cross-phase gain is below {:.0}% or the current U2 error is not visible (< {:.0}% of samples >= 16/255 and no alpha-test flip); **MANUAL_FIX_RECOMMENDED** when the gain is at least {:.0}%, level-0 growth is at most {} KiB and the asset is gameplay content (priority 1-4); **VISUAL_REVIEW_REQUIRED** otherwise. Priority: 1 fighter, 2 VS stage, 3 effect/item, 4 1P/bonus, 5 menu/movie/unused.\n",
        100.0 * ACCEPT_BELOW,
        MIN_VISIBLE_PCT16,
        100.0 * RECOMMEND_AT,
        MAX_LEVEL0_DELTA / 1024
    );

    // Summary
    let holdout_now: u64 = all.iter().map(|t| t.holdout_now).sum();
    let holdout_bound: u64 = all.iter().map(|t| t.holdout_bound).sum();
    let u2_now: u64 = all.iter().map(|t| t.u2_now).sum();
    // A fit that is worse than the shipped texture would not be shipped.
    let u2_after: u64 = all.iter().map(|t| t.u2_after.min(t.u2_now)).sum();
    let _ = writeln!(
        m,
        "Over all {} candidates: holdout SSE {holdout_now} -> oracle bound {holdout_bound} ({}), but cross-phase SSE only {u2_now} -> {u2_after} ({}) with each row's fix-class fit (kept only where it beats the shipped texture). {} of {} rows keep at least {:.0}% on U2; {} keep at least {:.0}%.\n",
        all.len(),
        pct(holdout_now, holdout_bound),
        pct(u2_now, u2_after),
        all.iter().filter(|t| gain(t.u2_now, t.u2_after) >= ACCEPT_BELOW).count(),
        all.len(),
        100.0 * ACCEPT_BELOW,
        all.iter().filter(|t| gain(t.u2_now, t.u2_after) >= RECOMMEND_AT).count(),
        100.0 * RECOMMEND_AT
    );
    let _ = writeln!(m, "| Fix class | Candidates | Recommended | Visual review | Accept |\n|---|---:|---:|---:|---:|");
    for class in FixClass::ALL {
        let of: Vec<_> = all.iter().filter(|t| t.class == class).collect();
        if of.is_empty() {
            continue;
        }
        let n = |b: Bucket| of.iter().filter(|t| t.bucket == b).count();
        let _ = writeln!(
            m,
            "| `{}` | {} | {} | {} | {} |",
            class.name(),
            of.len(),
            n(Bucket::Recommended),
            n(Bucket::VisualReview),
            n(Bucket::Accept)
        );
    }
    let nominal_accepted: BTreeMap<&str, usize> = all
        .iter()
        .filter(|t| t.class == FixClass::AcceptAfterVisualReview)
        .fold(BTreeMap::new(), |mut acc, t| {
            *acc.entry(t.nominal.name()).or_default() += 1;
            acc
        });
    let _ = writeln!(
        m,
        "\n`ACCEPT_AFTER_VISUAL_REVIEW` rows by the class the evidence would otherwise point at: {}.\n",
        nominal_accepted
            .iter()
            .map(|(k, v)| format!("`{k}` {v}"))
            .collect::<Vec<_>>()
            .join(", ")
    );

    // 11.1 Alpha-constrained cutouts
    let alpha: Vec<_> = all
        .iter()
        .filter(|t| {
            matches!(t.r.v.conversion.policy, AlphaPolicy::Cutout { .. })
                && has(t.r, "ALPHA_CONSTRAINT")
        })
        .collect();
    let _ = writeln!(
        m,
        "### 11.1 Alpha-constrained cutouts ({})\n\nFlips are alpha-test disagreements with the 3-point reference on U2. `Oracle` is the silhouette fit (alpha search over refitted RGB); `alpha-only` starts from the shipped texels and changes only alpha, so RGB stays byte-identical. Recoverable = share of current flips the oracle removes. Distinct = RGBA values the alpha-only result needs (CLUT capacity 16 for T4, 256 for T8).\n",
        alpha.len()
    );
    let _ = writeln!(
        m,
        "| Rank | Variant | Use | Threshold | Flips now | Oracle flips | Recoverable | Alpha-only flips | Alpha-only SSE gain | Oracle SSE gain | Alpha-only enough | RGB untouched | Distinct / CLUT | Class |\n|---:|---|---|---|---:|---:|---:|---:|---:|---:|---|---|---|---|"
    );
    for t in &alpha {
        let x = t.r.a.uniform_transfer.cross.as_ref().unwrap();
        let sil = x.silhouette.as_ref().unwrap();
        let (ao, colours) = x.alpha_only.as_ref().unwrap();
        let now = x.current.vis.sse;
        let flips = x.current.cutout_mismatch;
        let ao_g = gain(now, ao.vis.sse);
        let best = gain(now, sil.vis.sse).max(gain(now, x.rgba.vis.sse));
        let enough = ao_g >= 0.8 * best && ao_g > 0.0;
        let _ = writeln!(
            m,
            "| {} | {} | {} | {} | {flips} | {} | {} | {} | {} | {} | {} | {} | {colours} / {} | `{}` |",
            t.rank,
            ident(t.r),
            t.r.primary_name,
            threshold(t.r.v.conversion.policy).unwrap(),
            sil.cutout_mismatch,
            if flips == 0 { "-".into() } else { pct(flips, sil.cutout_mismatch) },
            ao.cutout_mismatch,
            pct(now, ao.vis.sse),
            pct(now, sil.vis.sse),
            if enough { "yes" } else { "no" },
            if enough { "yes" } else { "no: RGB refit needed" },
            palette_capacity(t.r).map_or("direct".into(), |c| c.to_string()),
            t.class.name()
        );
    }

    // 11.2 Conflicting use sites
    let conflict: Vec<_> = all
        .iter()
        .filter(|t| has(t.r, "CONFLICTING_USE_SITES"))
        .collect();
    let _ = writeln!(
        m,
        "\n### 11.2 Conflicting use sites ({})\n\nEach distinct coverage scored on its own holdout: current, its own oracle, and the shared (union) oracle. The split estimate is the sum of own oracles; the cross-phase column is the shared fit's U2 result (the split's U2 result is at least as good). Cost counts one RGBA8888 texture per site group.\n",
        conflict.len()
    );
    let _ = writeln!(
        m,
        "| Rank | Variant | Site group | Current | Own oracle | Shared oracle |\n|---:|---|---|---:|---:|---:|"
    );
    for t in &conflict {
        let a = t.r.a;
        for (i, (g, e)) in a.groups.iter().zip(&a.per_site_current).enumerate() {
            let (own, shared) = a.per_site_oracle_split.get(i).copied().unwrap_or((0, 0));
            let prims = g
                .prims
                .iter()
                .map(|&(f, dl, p, _)| format!("{f}:0x{dl:X}#{p}"))
                .collect::<Vec<_>>()
                .join(", ");
            let _ = writeln!(
                m,
                "| {} | {} | {prims} | {} | {own} | {shared} |",
                t.rank,
                ident(t.r),
                e.vis.sse
            );
        }
    }
    let _ = writeln!(
        m,
        "\n| Rank | Variant | Groups | Split (own oracles) | Shared oracle | Cross-phase now -> after | Level-0 delta | Pack delta | Class |\n|---:|---|---:|---:|---:|---|---:|---:|---|"
    );
    for t in &conflict {
        let (own, shared) = t.r.a.per_site_oracle_vis_sse.unwrap_or((0, 0));
        let _ = writeln!(
            m,
            "| {} | {} | {} | {own} | {shared} | {} -> {} ({}) | {} | {} | `{}` |",
            t.rank,
            ident(t.r),
            t.r.a.groups.len(),
            t.u2_now,
            t.u2_after,
            pct(t.u2_now, t.u2_after),
            t.level0_delta,
            t.pack_delta,
            t.class.name()
        );
    }

    // 11.3 Palette and animated-palette cases
    let palette: Vec<_> = all
        .iter()
        .filter(|t| {
            t.r.a.states > 1
                || has(t.r, "PALETTE_CONSTRAINT")
                || has(t.r, "ANIMATED_PALETTE_CONFLICT")
        })
        .collect();
    let _ = writeln!(
        m,
        "\n### 11.3 Palette and animated-palette cases ({})\n\nPer-state holdout SSE identifies the palette states that carry the error. Index-only = the pipeline's filter-aware index optimizer (trained, scored on the holdout). Direct fallback = one RGBA8888 texture per state.\n",
        palette.len()
    );
    let _ = writeln!(
        m,
        "| Rank | Variant | States | Worst states (share of current SSE) | Index-only gain | Cross-phase RGBA gain | Index-only enough | Direct fallback level-0 / pack | Class |\n|---:|---|---:|---|---:|---:|---|---:|---|"
    );
    for t in &palette {
        let a = t.r.a;
        let total: u64 = a.per_state_vis_sse.iter().map(|s| s.0).sum();
        let mut by: Vec<(usize, u64)> = a
            .per_state_vis_sse
            .iter()
            .map(|s| s.0)
            .enumerate()
            .collect();
        by.sort_by(|p, q| q.1.cmp(&p.1).then(p.0.cmp(&q.0)));
        let worst = if by.is_empty() {
            "1 state".into()
        } else {
            by.iter()
                .take(3)
                .map(|(k, s)| format!("#{k} {:.0}%", 100.0 * ratio(*s, total)))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let ig = a
            .indexed
            .as_ref()
            .map(|i| gain(a.current.eval.vis.sse, i.eval.vis.sse));
        let x = a.uniform_transfer.cross.as_ref().unwrap();
        let rg = gain(x.current.vis.sse, x.rgba.vis.sse);
        let states = a.states as i64;
        let _ = writeln!(
            m,
            "| {} | {} | {} | {worst} | {} | {:.1}% | {} | {} / {} | `{}` |",
            t.rank,
            ident(t.r),
            a.states,
            ig.map_or("-".into(), |g| format!("{:.1}%", 100.0 * g)),
            100.0 * rg,
            if ig.is_some_and(|g| g >= ACCEPT_BELOW) {
                "yes"
            } else {
                "no"
            },
            direct_level0(t.r) as i64 * states - a.current_level0_bytes as i64,
            direct_pack_one(t.r) as i64 * states - a.current_pack_bytes as i64,
            t.class.name()
        );
    }

    // 11.4 OTHER
    let other: Vec<_> = all.iter().filter(|t| t.other_kind.is_some()).collect();
    let _ = writeln!(
        m,
        "\n### 11.4 `OTHER` cases ({})\n\nThese matched no cause class: the pipeline's gates rejected every candidate and no single constraint explains the gap between current and the holdout oracle. The cross-phase fit decides what the gap really is: below {:.0}% the holdout oracle was fitting sparse probe phases (true bilinear surface limitation); otherwise the first matching explanation of use-site conflict (own oracles <= 75% of shared), alpha-limited (alpha-held fit >= 10 points worse), quantization-limited (paletted and index-only < 10%), coverage-limited (dense training reaches half the gain on U1).\n",
        other.len(),
        100.0 * ACCEPT_BELOW
    );
    let mut kinds: BTreeMap<&str, (usize, u64, u64)> = BTreeMap::new();
    for t in &other {
        let e = kinds.entry(t.other_kind.unwrap()).or_default();
        e.0 += 1;
        e.1 += t.u2_now;
        e.2 += t.u2_after.min(t.u2_now);
    }
    let _ = writeln!(m, "| Explanation | Variants | Cross-phase SSE now | Best fit | Gain |\n|---|---:|---:|---:|---:|");
    for (k, (n, now, after)) in &kinds {
        let _ = writeln!(m, "| {k} | {n} | {now} | {after} | {} |", pct(*now, *after));
    }
    let _ = writeln!(
        m,
        "\n| Rank | Variant | Use | Holdout now -> oracle | Dense (U1) gain | Cross-phase gain | Explanation |\n|---:|---|---|---|---:|---:|---|"
    );
    for t in &other {
        let a = t.r.a;
        let _ = writeln!(
            m,
            "| {} | {} | {} | {} -> {} ({}) | {} | {} | {} |",
            t.rank,
            ident(t.r),
            t.r.primary_name,
            t.holdout_now,
            t.holdout_bound,
            pct(t.holdout_now, t.holdout_bound),
            pct(a.uniform.0.vis.sse, a.uniform_transfer.dense.vis.sse),
            pct(t.u2_now, t.u2_after),
            t.other_kind.unwrap()
        );
    }

    // 11.5 Texgen (all rows, not only candidates)
    let texgen: Vec<&Row<'_>> = ranked
        .iter()
        .map(|&k| &rows[k])
        .filter(|r| has(r, "TEXGEN_COVERAGE"))
        .collect();
    let _ = writeln!(
        m,
        "\n### 11.5 Texgen cases ({})\n\nNo texgen variant is a hand-edit candidate. Real-normal coverage = the practical solve trained on RE-311's pose samples; conservative coverage is RE-311's box lattice, which is part of the holdout below; material-animated texgen uses the full-tile fallback for both. Oracle = fitted on that holdout; U1 = the same fitted texture on the phase-uniform set.\n",
        texgen.len()
    );
    let _ = writeln!(
        m,
        "| Variant | Use | Coverage | Current | Practical (real-normal/full-tile training) | Dense training | Holdout oracle | Oracle gain | Oracle on U1 | Texture-only change helps materially |\n|---|---|---|---:|---:|---:|---:|---:|---:|---|"
    );
    for r in &texgen {
        let a = r.a;
        let now = a.current.eval.vis.sse;
        let og = gain(now, a.rgba_oracle.eval.vis.sse);
        let _ = writeln!(
            m,
            "| {} | {} | {} | {now} | {} | {} | {} | {:.1}% | {} | {} |",
            ident(r),
            r.primary_name,
            mapping(a),
            a.rgba_practical.eval.vis.sse,
            a.rgba_dense.eval.vis.sse,
            a.rgba_oracle.eval.vis.sse,
            100.0 * og,
            pct(a.uniform.0.vis.sse, a.uniform_transfer.rgba_oracle.vis.sse),
            if og >= 0.5 {
                "possible"
            } else {
                "no: even the holdout-fitted oracle keeps most of the error"
            }
        );
    }

    // Shortlists
    fn order<'t, 'r, 'a>(mut v: Vec<&'t Triage<'r, 'a>>) -> Vec<&'t Triage<'r, 'a>> {
        v.sort_by(|p, q| {
            p.priority
                .0
                .cmp(&q.priority.0)
                .then(
                    (q.u2_now.saturating_sub(q.u2_after)).cmp(&p.u2_now.saturating_sub(p.u2_after)),
                )
                .then(p.r.a.index.cmp(&q.r.a.index))
        });
        v
    }
    let rec = order(
        all.iter()
            .filter(|t| t.bucket == Bucket::Recommended)
            .collect(),
    );
    let review = order(
        all.iter()
            .filter(|t| t.bucket == Bucket::VisualReview)
            .collect(),
    );
    let accept = order(all.iter().filter(|t| t.bucket == Bucket::Accept).collect());

    let _ = writeln!(m, "\n## MANUAL_FIX_RECOMMENDED ({})\n", rec.len());
    if rec.is_empty() {
        let _ = writeln!(m, "None: no candidate meets every criterion.\n");
    } else {
        let _ = writeln!(
            m,
            "Implementation order: gameplay priority, then absolute cross-phase SSE removed. Expected SSE is the cross-phase (U2) result of the fix class's fit; `current SSE` is the same U2 set.\n"
        );
        let _ = writeln!(
            m,
            "| # | Variant | File | Use | Priority | Format | Fix | Current SSE | Expected SSE | Reduction | Flips before -> after | Level-0 delta | Pack delta | Runtime | Approach | Crop |\n|---:|---|---|---|---|---|---|---:|---:|---:|---|---:|---:|---|---|---|"
        );
        for (i, t) in rec.iter().enumerate() {
            let c = &t.r.v.conversion;
            let _ = writeln!(
                m,
                "| {} | {} | {} | {} | {} | {} -> {} | `{}` | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
                i + 1,
                ident(t.r),
                names.get(source_file(t.r.v)),
                use_sites_text(t.r.a, names, 4),
                t.priority.1,
                fmt_name(c.texture.format, c.texture.size),
                psm_name(c.final_psm),
                t.class.name(),
                t.u2_now,
                t.u2_after,
                pct(t.u2_now, t.u2_after),
                if matches!(c.policy, AlphaPolicy::Cutout { .. }) {
                    format!("{} -> {}", t.u2_flips.0, t.u2_flips.1)
                } else {
                    "-".into()
                },
                t.level0_delta,
                t.pack_delta,
                runtime_cost(t.class, t.r, t.level0_delta),
                approach(t),
                crop(t.r)
            );
        }
    }

    let _ = writeln!(
        m,
        "\n## VISUAL_REVIEW_REQUIRED ({})\n\nLarge holdout bound and a cross-phase gain of at least {:.0}%, but short of the recommendation bar (gain, cost or visibility): check the crop at PSP resolution before any work.\n",
        review.len(),
        100.0 * ACCEPT_BELOW
    );
    let _ = writeln!(
        m,
        "| Variant | Use | Priority | Fix | Holdout now -> bound | Cross-phase now -> after | Level-0 delta | Why not recommended | Crop |\n|---|---|---|---|---|---|---:|---|---|"
    );
    for t in &review {
        let _ = writeln!(
            m,
            "| {} | {} | {} | `{}` | {} -> {} ({}) | {} -> {} ({}) | {} | {} | {} |",
            ident(t.r),
            t.r.primary_name,
            t.priority.1,
            t.class.name(),
            t.holdout_now,
            t.holdout_bound,
            pct(t.holdout_now, t.holdout_bound),
            t.u2_now,
            t.u2_after,
            pct(t.u2_now, t.u2_after),
            t.level0_delta,
            t.why,
            crop(t.r)
        );
    }

    let _ = writeln!(
        m,
        "\n## ACCEPT ({})\n\nEven a bespoke same-resolution texture does not buy enough at uniformly distributed phases. By priority and explanation:\n",
        accept.len()
    );
    let mut groups: BTreeMap<(u8, &str, &str), (usize, u64, u64, u64, u64)> = BTreeMap::new();
    for t in &accept {
        let e = groups
            .entry((t.priority.0, t.priority.1, t.nominal.name()))
            .or_default();
        e.0 += 1;
        e.1 += t.holdout_now;
        e.2 += t.holdout_bound;
        e.3 += t.u2_now;
        e.4 += t.u2_after.min(t.u2_now);
    }
    let _ = writeln!(
        m,
        "| Priority | Evidence pointed at | Variants | Holdout now -> bound | Cross-phase now -> fit (if better) |\n|---|---|---:|---|---|"
    );
    for ((_, p, class), (n, hn, hb, un, ua)) in &groups {
        let _ = writeln!(
            m,
            "| {p} | `{class}` | {n} | {hn} -> {hb} ({}) | {un} -> {ua} ({}) |",
            pct(*hn, *hb),
            pct(*un, *ua)
        );
    }
    let _ = writeln!(
        m,
        "\nPer-row reason for every accepted candidate:\n\n| Variant | Use | Reason |\n|---|---|---|"
    );
    for t in &accept {
        let _ = writeln!(m, "| {} | {} | {} |", ident(t.r), t.r.primary_name, t.why);
    }

    let _ = writeln!(m, "\n## Triage totals\n");
    let _ = writeln!(
        m,
        "- Deployable automatic cases remaining (section 5a): **{deployable}**"
    );
    let _ = writeln!(m, "- Hand-edit candidates: **{}**", all.len());
    let _ = writeln!(m, "- MANUAL_FIX_RECOMMENDED: **{}**", rec.len());
    let _ = writeln!(m, "- VISUAL_REVIEW_REQUIRED: **{}**", review.len());
    let _ = writeln!(m, "- ACCEPT: **{}**\n", accept.len());
    let _ = writeln!(
        m,
        "Top 20 manual fixes in implementation order (recommended first, then visual review):\n"
    );
    for (i, t) in rec.iter().chain(review.iter()).take(20).enumerate() {
        let _ = writeln!(
            m,
            "{}. {} {} - `{}`, cross-phase {} ({}){}",
            i + 1,
            ident(t.r),
            t.r.primary_name,
            t.class.name(),
            pct(t.u2_now, t.u2_after),
            t.priority.1,
            if t.bucket == Bucket::VisualReview {
                ", after visual review"
            } else {
                ""
            }
        );
    }
    let _ = writeln!(m);
}
