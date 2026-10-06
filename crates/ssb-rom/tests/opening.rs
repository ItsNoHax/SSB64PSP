//! The opening movie's ROM data (`ssb_rom::opening`, `ssb_rom::camanim`)
//! against the decomp's `mv/mvopening/*.c`.

use ssb_rom::camanim;
use ssb_rom::opening as op;

fn archive() -> Option<(Vec<u8>, ssb_rom::Region)> {
    let path = std::env::var_os("SSB64_ROM")?;
    let data = std::fs::read(path).unwrap();
    let info = ssb_rom::rom::identify(&data).unwrap();
    Some((data, info.region))
}

#[test]
fn every_camera_bakes_to_its_end() {
    let Some((data, region)) = archive() else {
        return;
    };
    let archive = ssb_rom::archive::Archive::open(&data, region).unwrap();
    let cams = op::ROOM_CAMERAS.iter().chain(&op::COMMON_CAMERAS).chain(&[
        op::RUN_CAMERA,
        op::CLIFF_CAMERA,
        op::YAMABUKI_CAMERA,
        op::JUNGLE_CAMERA,
        op::YOSTER_CAMERA,
        op::SECTOR_CAMERA,
        op::STANDOFF_CAMERA,
        op::CLASH_FIGHTERS_CAMERA,
        op::CLASH_WALLPAPER_CAMERA,
    ]);
    for cam in cams {
        let file = archive.load(cam.file).unwrap();
        let frames = camanim::bake(&file.data, cam.offset, cam.init, 8192).unwrap();
        eprintln!(
            "camera {:#x}+{:#x}: {} plays, first {:?}, last {:?}",
            cam.file,
            cam.offset,
            frames.len(),
            frames.first(),
            frames.last()
        );
        assert!(!frames.is_empty() && frames.len() < 8192);
    }
}

/// RE-467: the standoff's lightning (`llMVOpeningStandoffLightningDObjDesc`)
/// has a material on six of its thirteen nodes, each with a texture-swap
/// script whose `MObjSub.sprites` array leaves entry 1 NULL. The scripts
/// select only 0, 2, 3 and 4, so romtool fills the hole
/// (`sparse_opening_sprites`) rather than declining the animation.
#[test]
fn the_standoff_lightning_swaps_textures_from_a_sparse_table() {
    let Some((data, region)) = archive() else {
        return;
    };
    let archive = ssb_rom::archive::Archive::open(&data, region).unwrap();
    let f = archive.load(op::file::STANDOFF).unwrap();
    let graphs = ssb_rom::scene::find_scene_graphs(&f);
    let g = graphs
        .iter()
        .find(|g| g.offset == op::standoff::LIGHTNING)
        .unwrap();
    let table =
        ssb_rom::mobj::read_table(&f, op::standoff::LIGHTNING_MOBJSUB, g.nodes.len()).unwrap();
    let chains: Vec<usize> = table.nodes.iter().map(|n| n.len()).collect();
    assert_eq!(chains, [0, 0, 0, 1, 1, 0, 0, 1, 1, 0, 0, 1, 1]);
    let scripts = ssb_rom::matanim::resolve_scripts(
        &f,
        op::standoff::LIGHTNING_MAT_ANIM_JOINT,
        table.nodes.len(),
        |n| table.nodes[n].len(),
    );
    assert_eq!(scripts.iter().flatten().flatten().count(), 6);
    let sub = table.nodes[3][0].at;
    assert!(ssb_rom::mobj::read_sprites(&f, sub, 5).is_none());
    let present: Vec<bool> = (0..5)
        .map(|i| ssb_rom::mobj::read_sprite_at(&f, sub, i).is_some())
        .collect();
    assert_eq!(present, [true, false, true, true, true]);
}
