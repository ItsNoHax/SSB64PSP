//! Material-animation script windows (`VERSION` 149): every script the run
//! time can start a joint on plays from its packed window exactly as from
//! the ROM file, and every restart the run time asks the pack for resolves
//! as `mobj_script` on the ROM file.
//!
//! Two readings: the pack build's walk ([`ssb_rom::matanim::script_window`])
//! chose the words; [`MaterialJoint`] on the ROM file is the reference.

use std::collections::BTreeMap;

use ssb_rom::matanim::{mobj_script, MaterialJoint, TICK_TRACK_COUNT};
use ssb_rom::pack::{AnimDesc, ObjectDesc, Pack, TextureDesc};
use ssb_rom::skeleton::EffectMaterialAnimator;
use ssb_rom::{rom, Archive};

/// Ticks per root: past every packed script's first loop.
const TICKS: usize = 1200;

/// Upper bound on the windows' total bytes: 99,868 B of script words
/// measured (batch 188), plus their root and restart tables. The whole
/// files were 1,347,328 B.
const WINDOW_BYTES_MAX: usize = 112 * 1024;

fn inputs() -> Option<(Vec<u8>, Vec<u8>)> {
    let path = ssb_tablegen::test_rom(env!("CARGO_MANIFEST_DIR"))?;
    let rom = std::fs::read(path).ok()?;
    let pack = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak"),
    )
    .ok()?;
    Some((rom, pack))
}

/// One desc per window (by blob offset), with its source file.
fn windows(pack: &Pack<'_>) -> BTreeMap<u32, (u32, u32)> {
    let mut out = BTreeMap::new();
    for i in 0..pack.mat_anim_count() {
        let d = pack.mat_anim(i).unwrap();
        out.entry(d.file_offset).or_insert((i, d.source_file));
    }
    out
}

/// Plays `source` on the ROM file and `packed` on the window side by side.
fn same_play(file: &[u8], source: u32, window: &[u8], packed: u32) -> Result<(), String> {
    let mut whole = MaterialJoint::start(source, 0.0);
    let mut win = MaterialJoint::start(packed, 0.0);
    for tick in 0..TICKS {
        let (a, b) = (whole.tick(file, 1.0), win.tick(window, 1.0));
        if a.is_ok() != b.is_ok() {
            return Err(format!("tick {tick}: file {a:?}, window {b:?}"));
        }
        for t in 0..TICK_TRACK_COUNT {
            let (x, y) = (whole.track_value(t), win.track_value(t));
            if x.map(f32::to_bits) != y.map(f32::to_bits) {
                return Err(format!("tick {tick} track {t}: file {x:?}, window {y:?}"));
            }
        }
        if (whole.ended(), whole.looped()) != (win.ended(), win.looped()) {
            return Err(format!("tick {tick}: end or loop differs"));
        }
        if a.is_err() || whole.ended() {
            break;
        }
    }
    Ok(())
}

#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn every_window_root_plays_as_its_rom_file() {
    let Some((rom_bytes, pack_bytes)) = inputs() else {
        return;
    };
    let info = rom::identify(&rom_bytes).unwrap();
    let archive = Archive::open(&rom_bytes, info.region).unwrap();
    let pack = Pack::open(&pack_bytes).unwrap();
    let windows = windows(&pack);
    assert!(!windows.is_empty());
    let (mut roots, mut total, mut whole) = (0, 0, 0);
    let mut bad = Vec::new();
    for &(i, f) in windows.values() {
        let file = archive.load(f).unwrap();
        let window = pack.mat_anim_file_at(i).unwrap();
        assert!(
            window.len() < file.data.len(),
            "file {f}: window not smaller"
        );
        total += window.len();
        whole += file.data.len();
        for (source, packed) in pack.mat_anim_roots(i) {
            roots += 1;
            if let Err(e) = same_play(&file.data, source, window, packed) {
                bad.push(format!("file {f} root {source:#x}: {e}"));
            }
        }
    }
    // Every desc's own script is a root of its window.
    for i in 0..pack.mat_anim_count() {
        let d = pack.mat_anim(i).unwrap();
        assert!(
            pack.mat_anim_roots(i).any(|(_, packed)| packed == d.script),
            "mat anim {i}: script {:#x} is no root",
            d.script
        );
    }
    eprintln!(
        "{} windows, {roots} roots, {total} bytes for {whole} bytes of files",
        windows.len()
    );
    assert!(bad.is_empty(), "{}", bad.join("\n"));
    assert!(
        total <= WINDOW_BYTES_MAX,
        "windows {total} bytes, over {WINDOW_BYTES_MAX}"
    );
}

/// The ground effects' and boss wallpaper's restart queries, made as the
/// run time makes them (psp-game's ground-effect draw, `psp_runtime::boss`),
/// against `mobj_script` on the ROM file; and each restart replays through
/// [`EffectMaterialAnimator`] as the ROM file's script.
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn every_runtime_restart_resolves_and_plays_as_mobj_script() {
    let Some((rom_bytes, pack_bytes)) = inputs() else {
        return;
    };
    let info = rom::identify(&rom_bytes).unwrap();
    let archive = Archive::open(&rom_bytes, info.region).unwrap();
    let pack = Pack::open(&pack_bytes).unwrap();
    let object = |key: (u32, u32)| -> Option<ObjectDesc> {
        (0..pack.object_count())
            .filter_map(|i| pack.object(i))
            .find(|o| (o.source_file, o.source_offset) == key)
    };
    let mut queries = Vec::new();
    {
        use ssb_game::ground_effect as g;
        for k in 0..g::DESC_COUNT {
            let Some(file) = g::desc_file(k).filter(|_| g::O_MATANIM_JOINT[k] != 0) else {
                continue;
            };
            if let Some(o) = object((file, g::O_DOBJSETUP[k])) {
                queries.push((
                    format!("ground {k}"),
                    o,
                    g::O_MATANIM_JOINT[k],
                    g::O_MOBJSUB[k],
                ));
            }
        }
    }
    for (w, row) in ssb_game::spgame::boss::EFFECT_ASSETS.iter().enumerate() {
        for (e, asset) in row.iter().enumerate() {
            let Some(a) = asset else { continue };
            let (Some(table), Some(subs)) = (a.matanim_joint, a.mobjsub) else {
                continue;
            };
            let o = object((ssb_game::spgame::boss::EFFECT_FILE, a.graph))
                .unwrap_or_else(|| panic!("boss w{w} e{e}: not packed"));
            queries.push((format!("boss w{w} e{e}"), o, table, subs));
        }
    }
    let (mut resolved, mut misses) = (0, 0);
    let mut keys = std::collections::BTreeSet::new();
    let mut bad = Vec::new();
    for (what, o, table, subs) in &queries {
        for n in 0..o.node_count {
            let Some(mesh) = pack.node(o.first_node + n).and_then(|d| pack.mesh(d.mesh)) else {
                continue;
            };
            for j in 0..mesh.prim_count {
                let prim = pack.prim(mesh.first_prim + j).unwrap();
                if prim.mat_anim == TextureDesc::NO_ANIM {
                    continue;
                }
                let d = pack.mat_anim(prim.mat_anim).unwrap();
                let file = archive.load(d.source_file).unwrap();
                let want = mobj_script(&file.data, *table, *subs, n as usize, d.source_offset);
                let got = pack.mat_anim_restart(prim.mat_anim, *table, *subs, n as usize);
                if got != want {
                    bad.push(format!("{what} node {n}: pack {got:x?}, ROM {want:x?}"));
                    continue;
                }
                let Some(script) = want else {
                    misses += 1;
                    continue;
                };
                resolved += 1;
                keys.insert((d.source_file, *table, *subs, n, d.source_offset));
                // The run time's player, restarted on the source offset.
                let mut m = EffectMaterialAnimator::new();
                m.start(&pack, std::iter::once(prim.mat_anim));
                m.restart(prim.mat_anim, script);
                let mut r = MaterialJoint::start(script, 0.0);
                for tick in 0..TICKS / 4 {
                    m.tick(&pack);
                    let ok = r.tick(&file.data, 1.0).is_ok();
                    let live = m.joint_for(prim.mat_anim).unwrap();
                    let same = (0..TICK_TRACK_COUNT).all(|t| {
                        live.track_value(t).map(f32::to_bits) == r.track_value(t).map(f32::to_bits)
                    });
                    if !same {
                        bad.push(format!("{what} node {n} tick {tick}: tracks differ"));
                        break;
                    }
                    if !ok || r.ended() {
                        break;
                    }
                }
            }
        }
    }
    eprintln!(
        "{} effects, {resolved} restarts resolved, {misses} None",
        queries.len()
    );
    assert!(resolved > 0);
    assert!(bad.is_empty(), "{}", bad.join("\n"));
    // No row the run time never asks for.
    let rows: usize = windows(&pack)
        .values()
        .map(|&(i, _)| pack.mat_anim_restarts(i).count())
        .sum();
    assert_eq!(rows, keys.len(), "restart rows");
}

/// The other scripts the run time restarts a joint on by file offset: the
/// effect constants and the ground material tables.
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn every_constant_and_ground_material_script_is_a_root() {
    let Some((_, pack_bytes)) = inputs() else {
        return;
    };
    let pack = Pack::open(&pack_bytes).unwrap();
    let constants = [
        (
            ssb_rom::effect::PIKACHU_THUNDER_SHOCK_KEY.0,
            ssb_rom::effect::PIKACHU_THUNDER_SHOCK_MAT_SCRIPTS.to_vec(),
        ),
        (84, ssb_rom::effect::DEAD_EXPLODE_MAT_SCRIPTS.concat()),
    ];
    for (file, scripts) in constants {
        let descs: Vec<u32> = (0..pack.mat_anim_count())
            .filter(|&i| pack.mat_anim(i).unwrap().source_file == file)
            .collect();
        assert!(!descs.is_empty(), "file {file}: no mat anim");
        for i in descs {
            for &s in &scripts {
                assert!(
                    pack.mat_anim_script(i, s).is_some(),
                    "file {file} script {s:#x}"
                );
            }
        }
    }
    let mut joints = 0;
    for a in (0..pack.anim_count()).filter_map(|i| pack.anim(i)) {
        if a.fighter != AnimDesc::GROUND_MAT {
            continue;
        }
        for j in (0..a.joint_count).filter_map(|j| pack.anim_joint(a.first_joint + j)) {
            joints += 1;
            assert!(
                pack.mat_anim_script(j.node, j.script).is_some(),
                "ground material {}: mat anim {} script {:#x}",
                a.slot,
                j.node,
                j.script
            );
        }
    }
    assert!(joints > 0);
}
