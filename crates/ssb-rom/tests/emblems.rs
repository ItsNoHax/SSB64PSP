//! RE-420: the series emblems (`FTEmblemModels`, file 35) carry their
//! material animations, whose frame `n` lights an emblem in player `n`'s
//! colour (`mnVSResultsMakeEmblem`'s `gcAddMatAnimJointAll(..., color)`).

use ssb_rom::effect::{EMBLEMS, EMBLEM_FILE};
use ssb_rom::pack::{flags, Pack};
use ssb_rom::skeleton::EffectMaterialAnimator;

fn pack_bytes() -> Option<Vec<u8>> {
    std::env::var_os("SSB64_ROM")?;
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak");
    std::fs::read(path).ok()
}

/// Every drawn prim of the emblem's object, as `(flags, mat_anim)`.
fn prims(pack: &Pack<'_>, dobjdesc: u32) -> Vec<(u32, u32)> {
    let object = (0..pack.object_count())
        .filter_map(|i| pack.object(i))
        .find(|o| (o.source_file, o.source_offset) == (EMBLEM_FILE, dobjdesc))
        .unwrap_or_else(|| panic!("emblem {dobjdesc:#x} is packed"));
    let mut out = Vec::new();
    for n in 0..object.node_count {
        let node = pack.node(object.first_node + n).unwrap();
        let Some(mesh) = pack.mesh(node.mesh) else {
            continue;
        };
        for j in 0..mesh.prim_count {
            let p = pack.prim(mesh.first_prim + j).unwrap();
            out.push((p.flags, p.mat_anim));
        }
    }
    out
}

/// The emblem's light colours `(light1, light2)` at `frame`, after the
/// one `gcPlayAnimAll` `mnVSResultsMakeEmblem` runs.
fn lights(pack: &Pack<'_>, dobjdesc: u32, frame: f32) -> Vec<([u8; 4], [u8; 4])> {
    let prims = prims(pack, dobjdesc);
    let mut a = EffectMaterialAnimator::new();
    a.start_at(pack, prims.iter().map(|&(_, m)| m), frame);
    a.tick(pack);
    prims
        .iter()
        .map(|&(_, m)| {
            let c = a.resolved_colors(m).unwrap();
            (c.light1.unwrap(), c.light2.unwrap())
        })
        .collect()
}

#[test]
fn every_emblem_animates_both_light_colours() {
    let Some(bytes) = pack_bytes() else {
        return;
    };
    let pack = Pack::open(&bytes).unwrap();
    for e in EMBLEMS {
        let prims = prims(&pack, e.dobjdesc);
        assert!(!prims.is_empty(), "{e:x?}");
        for (f, m) in prims {
            assert_ne!(m, u32::MAX, "{e:x?}");
            assert_eq!(
                f & (flags::LIT | flags::LIGHT1_ANIM | flags::LIGHT2_ANIM),
                flags::LIT | flags::LIGHT1_ANIM | flags::LIGHT2_ANIM,
                "{e:x?}"
            );
        }
    }
}

/// Kirby's script (file 35 + 0x3EF4) steps `Light1Color` through red,
/// blue, yellow and green one frame apart, over a `(0x26, 0x26, 0x26)`
/// ambient; frame 4 is the CPU's grey over black.
#[test]
fn an_emblems_start_frame_is_its_players_colour() {
    let Some(bytes) = pack_bytes() else {
        return;
    };
    let pack = Pack::open(&bytes).unwrap();
    let kirby = EMBLEMS[usize::from(ssb_rom::effect::EMBLEM_BY_FIGHTER[8])].dobjdesc;
    assert_eq!(kirby, 0x3E68);
    let expect = [
        [0xB3, 0x19, 0x19],
        [0x00, 0x19, 0x99],
        [0xFF, 0xB3, 0x00],
        [0x00, 0x99, 0x19],
    ];
    for (frame, rgb) in expect.iter().enumerate() {
        for (l1, l2) in lights(&pack, kirby, frame as f32) {
            assert_eq!(&l1[..3], rgb, "frame {frame}");
            assert_eq!(&l2[..3], &[0x26; 3], "frame {frame}");
        }
    }
    for (l1, l2) in lights(&pack, kirby, 4.0) {
        assert_eq!(&l1[..3], &[0x66; 3]);
        assert_eq!(&l2[..3], &[0; 3]);
    }
    // Every emblem's frames give the same four player colours.
    for e in EMBLEMS {
        for (frame, rgb) in expect.iter().enumerate() {
            for (l1, _) in lights(&pack, e.dobjdesc, frame as f32) {
                assert_eq!(&l1[..3], rgb, "{e:x?} frame {frame}");
            }
        }
    }
}
