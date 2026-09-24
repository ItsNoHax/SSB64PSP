extern crate std;

use std::string::ToString;
use std::vec::Vec;

use super::*;

fn every_viewer_scene() -> Vec<ViewerScene> {
    let mut scenes = std::vec![
        ViewerScene::DreamLand,
        ViewerScene::OpeningRoom,
        ViewerScene::StageSector,
        ViewerScene::CatchSwirl,
        ViewerScene::SaffronGate,
        ViewerScene::MetalTexgen,
        ViewerScene::MetalTexgenRotated,
        ViewerScene::MetalTexgenLinear,
        ViewerScene::MetalTexgenCameraRotated,
        ViewerScene::PeachCastle,
        ViewerScene::LinkCostume1,
        ViewerScene::MarioEntry,
        ViewerScene::BonusPlatform,
        ViewerScene::DepthMask,
    ];
    scenes.extend(Fighter::ALL.map(ViewerScene::Fighter));
    scenes.extend((0..41).map(ViewerScene::Stage));
    scenes
}

#[test]
fn viewer_specs_round_trip() {
    for scene in every_viewer_scene() {
        let spec = scene.to_string();
        assert_eq!(ViewerScene::parse(&spec), Some(scene), "{spec}");
        assert!(spec.len() <= MAX_SPEC_LEN);
    }
}

#[test]
fn game_specs_round_trip() {
    for scene in GameScene::ALL {
        assert_eq!(GameScene::parse(&scene.to_string()), Some(scene));
    }
}

#[test]
fn parses_documented_examples() {
    assert_eq!(ViewerScene::parse("stage 17"), Some(ViewerScene::Stage(17)));
    assert_eq!(
        ViewerScene::parse("fighter fox"),
        Some(ViewerScene::Fighter(Fighter::Fox))
    );
    assert_eq!(ViewerScene::parse("scene3"), Some(ViewerScene::StageSector));
    assert_eq!(
        ViewerScene::parse("depth_mask"),
        Some(ViewerScene::DepthMask)
    );
    assert_eq!(
        ViewerScene::parse("  stage\t5 "),
        Some(ViewerScene::Stage(5))
    );
}

#[test]
fn rejects_malformed_specs() {
    for spec in [
        "",
        "stage",
        "stage -1",
        "stage +3",
        "stage 0x10",
        "stage 99999999999",
        "stage 3 4",
        "scene1",
        "scene11",
        "fighter",
        "fighter bowser",
        "default extra",
        "Stage 3",
    ] {
        assert_eq!(ViewerScene::parse(spec), None, "{spec:?}");
    }
    assert_eq!(GameScene::parse("fireball2"), None);
    assert_eq!(GameScene::parse(""), None);
}

#[test]
fn spec_line_skips_blanks_and_comments() {
    assert_eq!(spec_line("stage 17\n"), Some("stage 17"));
    assert_eq!(
        spec_line("\r\n# note\n  fighter fox \r\n"),
        Some("fighter fox")
    );
    assert_eq!(spec_line("# only a comment\n"), None);
    assert_eq!(spec_line(""), None);
}

#[test]
fn fighter_names_are_unique_feature_suffixes() {
    for (i, a) in Fighter::ALL.iter().enumerate() {
        assert_eq!(Fighter::from_name(a.name()), Some(*a));
        for b in &Fighter::ALL[i + 1..] {
            assert_ne!(a.name(), b.name());
        }
    }
}

#[test]
fn link_costume_scene_is_a_link_fighter_scene() {
    let scene = ViewerScene::LinkCostume1;
    assert_eq!(scene.fighter(), Some(Fighter::Link));
    assert!(scene.object_view() && scene.holds_spin());
}

/// Every `tests/golden/scenes.tsv` row names a scene its crate can parse, and
/// that spec is the scene's canonical spelling.
#[test]
fn manifest_specs_round_trip() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/golden/scenes.tsv");
    let manifest = std::fs::read_to_string(path).expect("read tests/golden/scenes.tsv");
    let mut rows = 0;
    let mut goldens = std::collections::BTreeSet::new();
    for line in manifest.lines() {
        if line.is_empty() || line.starts_with('#') || line.starts_with("golden\t") {
            continue;
        }
        let cols: Vec<&str> = line.split('\t').collect();
        assert_eq!(cols.len(), 5, "{line:?}");
        let (golden, krate, spec, status) = (cols[0], cols[1], cols[2], cols[3]);
        assert!(matches!(status, "pass" | "known-failing"), "{line:?}");
        assert!(goldens.insert(golden), "duplicate golden {golden}");
        let png = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/golden/");
        assert!(
            std::path::Path::new(&std::format!("{png}{golden}.png")).is_file(),
            "{golden}.png missing"
        );
        let canonical = match krate {
            "psp-asset-viewer" => ViewerScene::parse(spec).map(|s| s.to_string()),
            "psp-game" => GameScene::parse(spec).map(|s| s.to_string()),
            other => panic!("unknown crate {other}"),
        };
        assert_eq!(canonical.as_deref(), Some(spec), "{golden}");
        rows += 1;
    }
    let pngs = std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/golden"))
        .unwrap()
        .filter(|e| {
            e.as_ref()
                .unwrap()
                .path()
                .extension()
                .is_some_and(|x| x == "png")
        })
        .count();
    assert_eq!(rows, pngs, "every golden PNG has exactly one manifest row");
}
