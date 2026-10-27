//! `EffectMaterialAnimator` keeps [`MAX_EFFECT_MAT_ANIMS`] slots and drops
//! any script past them, so the cap must cover every object that can reach
//! it. Its fill paths:
//! - start path: [`EffectMaterialAnimator::start`] / `restart(mat_anim, ..)`,
//!   one slot per distinct `mat_anim` an object's primitives name
//!   (`object_mat_anims`: effect, weapon and item graphics, boss restart,
//!   movie, ending, menus, results, bonus2). Callers find their object by
//!   source key, so every pack object counts except the stage map layers.
//! - stage path: `ground_obj::restart_materials`, one slot per distinct
//!   `MObj` a controller's `GROUND_MAT` tables name on their object.
//!
//! The stage map layers (`StageDesc::layers`) are drawn with the `Vec`-backed
//! `MaterialAnimator` (`meshdraw::draw_stage_animated`), never this player.

use std::collections::{BTreeMap, BTreeSet};

use ssb_rom::ground_obj::MAT_ANIMS;
use ssb_rom::pack::{AnimDesc, AnimJoint, ObjectDesc, Pack, StageDesc, TextureDesc};
use ssb_rom::skeleton::{object_mat_anims, EffectMaterialAnimator, MAX_EFFECT_MAT_ANIMS};

fn pack_bytes() -> Option<Vec<u8>> {
    ssb_tablegen::test_rom(env!("CARGO_MANIFEST_DIR"))?;
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak");
    std::fs::read(path).ok()
}

fn distinct_scripts(p: &Pack<'_>, o: &ObjectDesc) -> usize {
    object_mat_anims(p, o)
        .filter(|&m| m != TextureDesc::NO_ANIM)
        .collect::<BTreeSet<_>>()
        .len()
}

/// Every object some stage draws as a render layer.
fn map_layers(p: &Pack<'_>) -> BTreeSet<u32> {
    (0..p.stage_count())
        .filter_map(|i| p.stage(i))
        .flat_map(|s| s.layers)
        .filter(|&l| l != StageDesc::NO_LAYER)
        .collect()
}

/// `(distinct scripts, label)`, largest first.
type Fills = Vec<(usize, String)>;

/// Start path: every object outside the map layers. Stage path: every
/// `GROUND_MAT` target.
fn effect_fills(p: &Pack<'_>) -> Fills {
    let layers = map_layers(p);
    let mut out = Fills::new();
    for i in (0..p.object_count()).filter(|i| !layers.contains(i)) {
        let Some(o) = p.object(i) else { continue };
        let label = format!("object {i} ({}+{:#x})", o.source_file, o.source_offset);
        out.push((distinct_scripts(p, &o), label));
    }
    let mut stage: BTreeMap<u8, BTreeSet<u32>> = BTreeMap::new();
    for a in (0..p.anim_count()).filter_map(|i| p.anim(i)) {
        if a.fighter != AnimDesc::GROUND_MAT {
            continue;
        }
        let Some(asset) = MAT_ANIMS.get(a.slot as usize) else {
            continue;
        };
        let keys = stage.entry(asset.object).or_default();
        for j in (0..a.joint_count).filter_map(|j| p.anim_joint(a.first_joint + j)) {
            if j.node != AnimJoint::NO_NODE && j.script != AnimJoint::NO_SCRIPT {
                keys.insert(j.node);
            }
        }
    }
    for (object, keys) in stage {
        out.push((keys.len(), format!("stage object {object} (GROUND_MAT)")));
    }
    out.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    out
}

#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn every_effect_material_fill_fits_the_cap() {
    let Some(bytes) = pack_bytes() else { return };
    let p = Pack::open(&bytes).unwrap();
    let fills = effect_fills(&p);
    for (n, label) in fills.iter().take(4) {
        eprintln!("{n:3} {label}");
    }
    let over: Vec<_> = fills
        .iter()
        .filter(|(n, _)| *n > MAX_EFFECT_MAT_ANIMS)
        .collect();
    assert!(
        over.is_empty(),
        "over MAX_EFFECT_MAT_ANIMS ({MAX_EFFECT_MAT_ANIMS}): {over:?}"
    );
}

/// Only map layers, which play `MaterialAnimator`, have more scripts than
/// the cap.
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn only_map_layers_exceed_the_cap() {
    let Some(bytes) = pack_bytes() else { return };
    let p = Pack::open(&bytes).unwrap();
    let layers = map_layers(&p);
    let over: Vec<_> = (0..p.object_count())
        .filter_map(|i| p.object(i).map(|o| (i, o)))
        .filter(|(_, o)| distinct_scripts(&p, o) > MAX_EFFECT_MAT_ANIMS)
        .collect();
    for (i, o) in &over {
        let n = distinct_scripts(&p, o);
        eprintln!(
            "{n:3} object {i} ({}+{:#x})",
            o.source_file, o.source_offset
        );
        assert!(layers.contains(i), "object {i} is not a map layer");
    }
}

/// The busiest start-path object starts every one of its scripts.
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn the_busiest_effect_object_starts_every_script() {
    let Some(bytes) = pack_bytes() else { return };
    let p = Pack::open(&bytes).unwrap();
    let layers = map_layers(&p);
    let (i, o, want) = (0..p.object_count())
        .filter(|i| !layers.contains(i))
        .filter_map(|i| p.object(i).map(|o| (i, o)))
        .map(|(i, o)| (i, o, distinct_scripts(&p, &o)))
        .max_by_key(|&(_, _, n)| n)
        .unwrap();
    let mut a = EffectMaterialAnimator::new();
    a.start(&p, object_mat_anims(&p, &o));
    assert_eq!(a.len(), want, "object {i}");
}
