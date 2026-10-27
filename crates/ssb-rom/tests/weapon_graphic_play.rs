//! A made weapon graphic's first counted `gcPlayAnimAll` parses frame 0.
//!
//! `wpManagerMakeWeapon` calls `gcAddAnimAll` and no `gcPlayAnimAll`; the
//! first play is `wpProcessProcWeaponMain`'s on the creation frame, which
//! only consumes `AOBJ_ANIM_CHANGED`. The N64 `cpufx-u` trace (RE-540)
//! logs the Spin Attack weapon's `GObj` frame as 0 on its creation frame
//! and `el - 31` after (el 31 to 62).

use ssb_rom::pack::{AnimDesc, Pack};
use ssb_rom::skeleton::{object_mat_anims, GraphicPlayers};

/// `llLinkMainSpinAttackWeaponAttributes`: file 324's two-node tree.
const SPIN_WEAPON_SOURCE: (u32, u32) = (324, 0x11908);

fn spin_weapon(p: &Pack<'_>) -> ssb_rom::pack::ObjectDesc {
    (0..p.object_count())
        .filter_map(|i| p.object(i))
        .find(|o| (o.source_file, o.source_offset) == SPIN_WEAPON_SOURCE)
        .expect("the Spin Attack weapon object")
}

fn pack_bytes() -> Option<Vec<u8>> {
    ssb_tablegen::test_rom(env!("CARGO_MANIFEST_DIR"))?;
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak");
    std::fs::read(path).ok()
}

#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn the_spin_weapon_graphic_shows_frame_zero_on_its_creation_play() {
    let Some(bytes) = pack_bytes() else { return };
    let p = Pack::open(&bytes).unwrap();
    let object = spin_weapon(&p);
    let anim = p
        .weapon_anim(AnimDesc::WEAPON_ANIM_LINK_SPIN_ATTACK)
        .expect("the Spin Attack weapon animation");
    let script = p.anim_script(&anim);
    assert!(script.is_some());
    let mats: Vec<u32> = object_mat_anims(&p, &object)
        .filter(|&m| m != ssb_rom::pack::TextureDesc::NO_ANIM)
        .collect();
    assert!(!mats.is_empty(), "the band fades through material scripts");

    let mut g = GraphicPlayers::default();
    g.start(&p, Some(&anim), Some(object_mat_anims(&p, &object)));
    for plays in 1..=32u16 {
        g.play(&p, script, true, 1.0);
        let want = f32::from(plays - 1);
        assert_eq!(g.anim.gobj_frame(), want, "transform after {plays} plays");
        for &m in &mats {
            let joint = g.materials.joint_for(m).expect("a started material script");
            assert_eq!(joint.anim_frame(), want, "material {m} after {plays} plays");
        }
    }
}

/// The band's nine `MObj`s each have their own script (the same fade, a
/// different prim colour); every one gets its own slot.
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn the_spin_weapon_graphic_starts_every_material_script() {
    let Some(bytes) = pack_bytes() else { return };
    let p = Pack::open(&bytes).unwrap();
    let object = spin_weapon(&p);
    let mut mats: Vec<u32> = object_mat_anims(&p, &object)
        .filter(|&m| m != ssb_rom::pack::TextureDesc::NO_ANIM)
        .collect();
    mats.sort_unstable();
    mats.dedup();
    let mut g = GraphicPlayers::default();
    g.start(&p, None, Some(object_mat_anims(&p, &object)));
    assert_eq!(mats.len(), 9);
    assert_eq!(g.materials.len(), mats.len());
}
