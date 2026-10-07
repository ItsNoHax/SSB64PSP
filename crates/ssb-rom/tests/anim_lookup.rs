//! RE-469: `Pack::fighter_anim`'s binary search over the built pack agrees
//! with a scan of the animation table in order, for every fighter and slot.

use ssb_rom::pack::Pack;

fn pack_bytes() -> Option<Vec<u8>> {
    std::env::var_os("SSB64_ROM")?;
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak");
    std::fs::read(path).ok()
}

#[test]
fn fighter_anim_matches_a_table_scan() {
    let Some(bytes) = pack_bytes() else {
        eprintln!("skipping: SSB64_ROM unset or pack not built");
        return;
    };
    let pack = Pack::open(&bytes).unwrap();
    let rows: Vec<_> = (0..pack.anim_count())
        .filter_map(|i| pack.anim(i))
        .collect();
    let mut found = 0;
    for fighter in 0..32 {
        for slot in 0..ssb_rom::anim::SLOT_COUNT as u32 {
            let scan = rows
                .iter()
                .find(|a| a.fighter == fighter && a.slot == slot)
                .copied();
            assert_eq!(
                pack.fighter_anim(fighter, slot),
                scan,
                "({fighter}, {slot})"
            );
            found += usize::from(scan.is_some());
        }
    }
    assert!(found > 5000, "{found} fighter clips");
}
