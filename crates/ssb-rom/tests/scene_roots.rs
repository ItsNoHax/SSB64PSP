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

/// The special files Kirby's copy of `kind` reads when it fires, as
/// `(FTKind, file_special index)` per `FTKind` 0..12: each copy's neutral
/// special makes the original's weapon or effect, whose descriptor sits in
/// the copied kind's special file.
/// - Mario, Luigi: `gFTMarioFileSpecial1`, `gFTDataLuigiSpecial1`
///   (`wpmariofireball.c`, `ftkirbycopymariospecialn.c`).
/// - Fox: `gFTDataFoxSpecial1` (`wpfoxblaster.c`).
/// - Samus: `gFTDataSamusSpecial1` (`wpsamuschargeshot.c`).
/// - Link: `gFTDataLinkSpecial1` (`wplinkboomerang.c`).
/// - Yoshi: `gFTDataYoshiSpecial3` (the swallowed foe's egg,
///   `efManagerYoshiEggLayMakeEffect`, `ftcommoncaptureyoshi.c:132`).
/// - Captain Falcon: `gFTDataCaptainSpecial3`
///   (`dEFManagerCaptainFalconPunchEffectDesc`).
/// - Pikachu: `gFTDataPikachuSpecial1`, `gFTDataPikachuSpecial3`
///   (`wppikachuthunderjolt.c`).
/// - Ness: `gFTNessFileSpecial1` (`wpnesspkfire.c`).
/// - Donkey Kong, Kirby, Jigglypuff: none (no weapon or effect).
const COPY_SPECIALS: [&[usize]; 12] = [
    &[0],
    &[0],
    &[],
    &[0],
    &[0],
    &[0],
    &[2],
    &[2],
    &[],
    &[0, 2],
    &[],
    &[0],
];

fn copy_files(kind: usize) -> Vec<u32> {
    let specials = ssb_rom::fighter::FIGHTER_FILES[kind].specials;
    COPY_SPECIALS[kind]
        .iter()
        .map(|&i| specials[i].unwrap_or_else(|| panic!("kind {kind} special {i}")))
        .collect()
}

/// A Kirby's copy reads files only its copied kind loads
/// (`ftManagerSetupFilesPlayablesAll` binds the special heads of kinds
/// already loaded). Outside the Kirby Team a copy's kind swallowed is a
/// fighter of the battle, whose main file's closure holds them; Giant
/// Donkey Kong copies Donkey Kong, who reads none. The Kirby Team copies
/// kinds no player slot loads: the stage's `llKirbySpecial1FileID` load
/// (`sc1pgame.c:2077-2081`, [`r::KIRBY_TEAM`]) holds every one.
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn kirby_copies_read_files_their_scene_holds() {
    let Some(bytes) = pack_bytes() else {
        eprintln!("skipping: SSB64_ROM unset or pack not built");
        return;
    };
    let pack = Pack::open(&bytes).unwrap();
    for swallowed in 0..27usize {
        let copy = usize::from(ssb_game::kirby::COPY[swallowed].0);
        if copy >= 12 {
            continue;
        }
        let held = closure(&pack, &[r::FIGHTER_MAIN[swallowed]]);
        for f in copy_files(copy) {
            assert!(
                held.contains(&f),
                "swallowed {swallowed} (copy {copy}): {f:#x} not in {held:x?}"
            );
        }
    }
    let team = closure(&pack, &r::KIRBY_TEAM);
    for kind in ssb_game::spgame::setup::KIRBY_TEAM_COPY_KINDS {
        for f in copy_files(kind as usize) {
            assert!(team.contains(&f), "{kind:?}: {f:#x} not in {team:x?}");
        }
    }
    // Every copy, not only the team's seven.
    for kind in 0..12 {
        for f in copy_files(kind) {
            assert!(team.contains(&f), "kind {kind}: {f:#x} not in {team:x?}");
        }
    }
}
