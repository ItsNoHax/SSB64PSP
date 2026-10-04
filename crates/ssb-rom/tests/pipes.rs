//! Mushroom Kingdom's pipes (`ftcommondokan.c`) against the US ROM: the
//! three pipe points and the two pipe floors the entry check reads.
use ssb_game::dokan;
use ssb_rom::{archive::Archive, collision::LineKind};

#[test]
fn inishie_has_its_pipe_points_and_floors() {
    let Some(path) = std::env::var_os("SSB64_ROM") else {
        return;
    };
    let rom = std::fs::read(path).unwrap();
    let archive = Archive::open(&rom, ssb_rom::rom::identify(&rom).unwrap().region).unwrap();
    // `llGRInishieMapFileID`; its `MPGroundData` sits at 0x14, whose
    // `map_geometry` is the word at 0x40 of it.
    let map = archive.load(0x104).unwrap();
    let geometry = map
        .extern_relocs
        .iter()
        .find(|r| r.at == 0x14 + 0x40)
        .unwrap();
    let file = archive.load(u32::from(geometry.target_file)).unwrap();
    let coll = ssb_rom::collision::read(&file, geometry.target_offset).unwrap();
    let points = |kind| -> Vec<_> {
        coll.map_objects
            .iter()
            .filter(|o| o.kind == kind)
            .map(|o| o.pos)
            .collect()
    };
    let left = points(dokan::MAPOBJ_DOKAN_L);
    let right = points(dokan::MAPOBJ_DOKAN_R);
    assert_eq!(left.len(), 1);
    assert_eq!(right.len(), 1);
    assert_eq!(points(dokan::MAPOBJ_DOKAN_WALL).len(), 1);

    // Each pipe point hangs 60 units over the middle of a floor of its
    // pipe's material: the exit drops the fighter that far onto it.
    for (point, material) in [
        (left[0], dokan::MATERIAL_DOKAN_L),
        (right[0], dokan::MATERIAL_DOKAN_R),
    ] {
        let on = coll.lines_of(LineKind::Floor).any(|l| {
            l.points.windows(2).any(|w| {
                let (a, b) = (w[0], w[1]);
                a.flags & 0xFF == material
                    && a.pos[0].min(b.pos[0]) <= point[0]
                    && a.pos[0].max(b.pos[0]) >= point[0]
                    && i32::from(point[1]) - i32::from(a.pos[1]) == 60
                    && i32::from(a.pos[0]) + i32::from(b.pos[0]) == 2 * i32::from(point[0])
            })
        });
        assert!(on, "{point:?} {material}");
    }
}
