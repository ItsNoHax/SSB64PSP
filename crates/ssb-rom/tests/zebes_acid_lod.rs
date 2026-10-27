//! Planet Zebes's acid (file 157, `llGRZebesMapAcidDObjDesc` 0xB08) is an
//! RDP two-tile blend (RE-321): its `MObjSub` (0x8D8, flags `0x6B`) stages
//! `sprites[texture_id_next]` for tile 1 and writes `lfrac` into
//! `PRIM_LOD_FRAC`, and its list (0x9D8) crossfades the two tiles before
//! cycle 2 multiplies colour and alpha by `SHADE` under
//! `G_RM_AA_ZB_XLU_SURF2`.

use ssb_rom::mesh::{
    convert_sequence, AlphaBlend, InitialMaterial, MatAnimRef, SequenceItem, Source,
};
use ssb_rom::Archive;

const ZEBES: u32 = 157;
const ACID_DL: u32 = 0x9D8;
const ACID_MOBJ_SUB: u32 = 0x8D8;

fn rom() -> Option<Vec<u8>> {
    std::fs::read(ssb_tablegen::test_rom(env!("CARGO_MANIFEST_DIR"))?).ok()
}

#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn the_acid_is_a_two_tile_blend_times_the_shade_alpha() {
    let Some(rom) = rom() else {
        return;
    };
    let info = ssb_rom::rom::identify(&rom).unwrap();
    let archive = Archive::open(&rom, info.region).unwrap();
    let file = archive.load(ZEBES).unwrap();
    let cmds = ssb_rom::dl::decode_list_at(&file.data[ACID_DL as usize..], ACID_DL).unwrap();
    let mobj = ssb_rom::mobj::read_sub(&file, ACID_MOBJ_SUB).expect("MObjSub");
    assert!(mobj.loads_next_block && mobj.prim_lod.is_some());
    let mobjs = [mobj];
    let anims = [Some(MatAnimRef {
        source_file: ZEBES,
        script: 0xDC0,
        source_mobj: ACID_MOBJ_SUB,
    })];
    let items = [SequenceItem {
        cmds: &cmds,
        world: ssb_rom::scene::Mat4::IDENTITY,
        mobjs: &mobjs,
        mat_anims: &anims,
        depth_seed: None,
        stream: 0,
    }];
    let mesh = convert_sequence(&items, Source::of(&file), InitialMaterial::SCENE)
        .remove(0)
        .unwrap();
    assert_eq!(mesh.primitives.len(), 1);
    let prim = &mesh.primitives[0];
    let m = prim.material;
    let lod = m.lod_blend.expect("two-tile blend");
    assert!(lod.shade_alpha);
    // `TEXEL1` is `sprites[0]` through tile 1 (TMEM 0x40, same CI4 shape).
    assert_eq!((lod.next.data_file, lod.next.data_offset), (None, 0x6C0));
    assert!(m.translucent);
    assert_eq!(m.alpha_blend, Some(AlphaBlend::Shade));
    // The edge vertices fade to alpha 0, the rest are 0xDC; all are white.
    let used: std::collections::BTreeSet<u16> = prim.indices.iter().copied().collect();
    let alphas: std::collections::BTreeSet<u8> = used
        .iter()
        .map(|&i| mesh.vertices[usize::from(i)].rgba[3])
        .collect();
    assert_eq!(alphas, [0x00, 0xDC].into_iter().collect());
    assert!(used
        .iter()
        .all(|&i| mesh.vertices[usize::from(i)].rgba[..3] == [0xFF; 3]));
}

/// Each battle's ground restarts the acid's material clock
/// (`grZebesMakeAcid`: `gcAddAnimAll(..., 0.0F)` then `gcPlayAnimAll`,
/// `grzebes.c:95-102`): after `n` updates the acid's `anim_frame` is `n`
/// and its frames match a fresh ground's, whatever ran before.
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn a_new_ground_restarts_the_acid_material_clock() {
    use ssb_rom::pack::Pack;
    use ssb_rom::skeleton::MaterialAnimator;
    if ssb_tablegen::test_rom(env!("CARGO_MANIFEST_DIR")).is_none() {
        return;
    }
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak");
    let Ok(bytes) = std::fs::read(path) else {
        return;
    };
    let pack = Pack::open(&bytes).unwrap();
    let acid = (0..pack.mat_anim_count())
        .find(|&i| {
            pack.mat_anim(i)
                .is_some_and(|a| (a.source_file, a.source_offset) == (ZEBES, ACID_MOBJ_SUB))
        })
        .expect("the acid's MatAnimDesc");
    let frame = |m: &MaterialAnimator| m.joint(acid).unwrap().anim_frame();
    let state = |m: &MaterialAnimator| {
        let lod = m.resolved_lod_blend(&pack, acid);
        (
            m.resolved_texture(&pack, acid),
            lod.map(|l| (l.texture, l.frac)),
        )
    };

    let mut fresh = MaterialAnimator::new();
    fresh.start_stage(&pack);
    assert_eq!(frame(&fresh), 0.0, "the creation call does not advance");
    // A previous battle (and the boot's own start) ran 7 + 1 updates.
    let mut reused = MaterialAnimator::new();
    reused.start(&pack);
    for _ in 0..8 {
        reused.tick(&pack);
    }
    assert_eq!(frame(&reused), 7.0);
    reused.start_stage(&pack);
    let mut flips = 0;
    let mut last = state(&fresh);
    for n in 1..=120u32 {
        fresh.tick(&pack);
        reused.tick(&pack);
        assert_eq!(frame(&fresh), n as f32);
        assert_eq!(frame(&reused), n as f32);
        assert_eq!(state(&reused), state(&fresh), "update {n}");
        if state(&fresh) != last {
            flips += 1;
            last = state(&fresh);
        }
    }
    assert!(flips > 0, "the acid animates within 120 updates");
}
