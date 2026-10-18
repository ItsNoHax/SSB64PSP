//! The scene lists' per-scene additions against the built pack: the demo
//! clips the selects, the VS results and the 1P continue pose their
//! fighters with, and the stage select's previews (D-046).

use ssb_rom::pack::Pack;
use ssb_rom::scene_roots as r;

fn pack_bytes() -> Option<Vec<u8>> {
    ssb_tablegen::test_rom(env!("CARGO_MANIFEST_DIR"))?;
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak");
    std::fs::read(path).ok()
}

/// `roots` and their extern closure, as `scene_files` loads them.
fn closure(pack: &Pack<'_>, roots: &[u32]) -> Vec<u32> {
    let mut seen: Vec<u32> = Vec::new();
    let mut queue = roots.to_vec();
    while let Some(f) = queue.pop() {
        if seen.contains(&f) {
            continue;
        }
        seen.push(f);
        if let Some(d) = pack.file_index(f).and_then(|i| pack.file_at(i)) {
            queue.extend((0..d.dep_count).filter_map(|k| pack.file_dep(&d, k)));
        }
    }
    seen
}

/// The files PPSSPP captures read on demand before these lists held them:
/// `vsplayers`, `trainingselect(picked)`, `bonusselect`; `vsresults`;
/// `onepcontinue`.
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn clip_lists_hold_the_files_read_on_demand() {
    let Some(bytes) = pack_bytes() else {
        eprintln!("skipping: SSB64_ROM unset or pack not built");
        return;
    };
    let pack = Pack::open(&bytes).unwrap();
    let select = r::clip_files(&pack, 0..12, &r::SELECT_CLIP_SLOTS);
    for f in [0x717, 0x1BC, 0x17D, 0x167, 0x4EB, 0x1A2, 0x1F3] {
        assert!(select.contains(&f), "select clips lack {f:#x}");
    }
    let results = r::clip_files(&pack, 0..12, &r::RESULTS_CLIP_SLOTS);
    for f in [0x169, 0x1A0] {
        assert!(results.contains(&f), "results clips lack {f:#x}");
    }
    let kirby_continue = r::clip_files(&pack, [8], &r::CONTINUE_CLIP_SLOTS);
    assert!(kirby_continue.contains(&0x1A5), "{kirby_continue:x?}");
    // Every playable kind has a clip in each select slot.
    for kind in 0..12 {
        for &slot in &r::SELECT_CLIP_SLOTS {
            assert!(
                pack.fighter_anim(kind, slot as u32).is_some(),
                "kind {kind} slot {slot}"
            );
        }
    }
}

/// Each VS stage's preview files are the pack stage's layer objects' file
/// and its wallpaper sprite's, both in the closure of the map file
/// `mnMapsLoadMapFile` reads.
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn stage_previews_are_their_maps_model_and_wallpaper() {
    let Some(bytes) = pack_bytes() else {
        eprintln!("skipping: SSB64_ROM unset or pack not built");
        return;
    };
    let pack = Pack::open(&bytes).unwrap();
    for gkind in 0..9u8 {
        let map = r::ground_map(u32::from(gkind)).unwrap();
        let held = closure(&pack, &[map]);
        let model = r::STAGE_PREVIEW_MODELS[gkind as usize];
        let wallpaper = r::STAGE_WALLPAPERS[gkind as usize];
        assert!(
            held.contains(&model),
            "gkind {gkind}: {model:#x} not in {held:x?}"
        );
        assert!(
            held.contains(&wallpaper),
            "gkind {gkind}: {wallpaper:#x} not in {held:x?}"
        );
        let stage = (0..pack.stage_count())
            .filter_map(|i| pack.stage(i))
            .find(|s| ssb_rom::stage::vs_ground_kind(s.source_file) == Some(gkind))
            .unwrap();
        for &layer in &stage.layers {
            if let Some(o) = pack.object(layer) {
                assert_eq!(o.source_file, model, "gkind {gkind} layer object");
            }
        }
        assert_eq!(
            pack.stage_wallpaper(gkind).map(|s| s.source_file),
            Some(wallpaper)
        );
    }
}
