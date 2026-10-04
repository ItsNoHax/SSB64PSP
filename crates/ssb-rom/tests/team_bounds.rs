//! The 1P Game's enemy-team camera and blast bounds
//! (`MPGroundData.camera_bound_team_*`, `map_bound_team_*`) reach the
//! pack. Values from the decompilation's `relocData` headers.

use ssb_rom::pack::{Extent, Pack};

fn pack_bytes() -> Option<Vec<u8>> {
    std::env::var_os("SSB64_ROM")?;
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak");
    std::fs::read(path).ok()
}

fn extent(top: i16, bottom: i16, right: i16, left: i16) -> Extent {
    Extent {
        top,
        bottom,
        right,
        left,
    }
}

#[test]
fn the_campaign_team_bounds_are_packed() {
    let Some(bytes) = pack_bytes() else { return };
    let pack = Pack::open(&bytes).unwrap();
    // (map file, camera team bounds, map team bounds).
    let cases = [
        // `265_GRHyruleMap.c`.
        (
            265,
            extent(6000, -2100, 6000, -6000),
            extent(9000, -5000, 11000, -11000),
        ),
        // `256_GRPupupuSmallMap.c`.
        (
            256,
            extent(4000, -2500, 3500, -3500),
            extent(8500, -5000, 11000, -11000),
        ),
    ];
    for (file, camera, bounds) in cases {
        let i = pack
            .stage_of_file(file)
            .unwrap_or_else(|| panic!("map {file}"));
        let s = pack.stage(i).unwrap();
        assert_eq!(s.team_camera, camera, "map {file}");
        assert_eq!(s.team_bounds, bounds, "map {file}");
    }
}
