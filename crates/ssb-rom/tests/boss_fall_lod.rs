//! Master Hand's Fall wallpaper (file 114, graph 0xA188,
//! `SC1PGameBossWallpaper1ProcDisplay`: `G_CYC_2CYCLE`, `G_RM_PASS,
//! G_RM_AA_XLU_SURF2`) crossfades two tiles (RE-321) whose static windows
//! differ: tile 0 at `uls`/`ult` (256,128), tile 1 at (256,0), both clamped
//! (list 0x9D38, `MObjSub` 0x9B60). The packed UVs carry tile 0's origin
//! removed, so the second pass re-bases by the difference.

use ssb_rom::dl::Cmd;
use ssb_rom::mesh::{convert_sequence, InitialMaterial, MatAnimRef, SequenceItem, Source};
use ssb_rom::Archive;

const BOSS: u32 = 114;
const FALL_DL: u32 = 0x9D38;
const FALL_MOBJ_SUB: u32 = 0x9B60;
const FALL_SCRIPT: u32 = 0xB3E8;

fn rom() -> Option<Vec<u8>> {
    std::fs::read(ssb_tablegen::test_rom(env!("CARGO_MANIFEST_DIR"))?).ok()
}

#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn the_fall_pairs_tiles_whose_origins_differ() {
    let Some(rom) = rom() else {
        return;
    };
    let info = ssb_rom::rom::identify(&rom).unwrap();
    let archive = Archive::open(&rom, info.region).unwrap();
    let file = archive.load(BOSS).unwrap();
    let mut cmds = ssb_rom::dl::decode_list_at(&file.data[FALL_DL as usize..], FALL_DL).unwrap();
    // The display proc's state before the tree's lists.
    cmds.splice(
        0..0,
        [
            Cmd::SetOtherModeH {
                shift: 20,
                len: 2,
                data: 0x0010_0000,
            },
            Cmd::SetOtherModeL {
                shift: 3,
                len: 29,
                data: 0x0C18_41C8,
            },
            Cmd::SetPrimColor {
                m: 0,
                l: 0,
                rgba: [0, 0, 0, 0xFF],
            },
        ],
    );
    let mobj = ssb_rom::mobj::read_sub(&file, FALL_MOBJ_SUB).expect("MObjSub");
    assert!(mobj.loads_next_block && mobj.prim_lod.is_some());
    let mobjs = [mobj];
    let anims = [Some(MatAnimRef {
        source_file: BOSS,
        script: FALL_SCRIPT,
        source_mobj: FALL_MOBJ_SUB,
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
    let m = mesh.primitives[0].material;
    let t0 = m.texture.expect("TEXEL0");
    let lod = m.lod_blend.expect("two-tile blend");
    assert_eq!((t0.origin_s, t0.origin_t), (256, 128));
    assert_eq!((lod.next.origin_s, lod.next.origin_t), (256, 0));
    assert!(t0.clamp_s && t0.clamp_t && lod.next.clamp_s && lod.next.clamp_t);
    assert_eq!(lod.origin_delta, [0, 128]);
    assert_eq!(lod.tile1_params, [64, 160, 64]);
    assert!(m.translucent);
}
