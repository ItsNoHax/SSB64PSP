//! `scan::ROOT_DL_OFFSETS` is exactly what the scan finds (RE-588): the
//! packer reads the table, so a scan change must regenerate it.
use ssb_rom::archive::Archive;
use ssb_rom::scan::{find_root_display_lists, root_display_lists, ROOT_DL_OFFSETS};

fn rom() -> Option<Vec<u8>> {
    ssb_tablegen::test_rom(env!("CARGO_MANIFEST_DIR")).map(|p| std::fs::read(p).unwrap())
}

#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn table_equals_the_scan() {
    let Some(rom) = rom() else {
        return;
    };
    let archive = Archive::open(&rom, ssb_rom::rom::identify(&rom).unwrap().region).unwrap();
    let mut files = 0;
    for id in 0..archive.len() as u32 {
        let Ok(file) = archive.load(id) else {
            continue;
        };
        let scan = find_root_display_lists(&file);
        let table = root_display_lists(&file);
        let key = |v: &[ssb_rom::scan::FoundDl]| -> Vec<(u32, Vec<ssb_rom::dl::Cmd>)> {
            v.iter().map(|d| (d.offset, d.commands.clone())).collect()
        };
        assert_eq!(key(&scan), key(&table), "file {id}");
        files += usize::from(!scan.is_empty());
    }
    assert_eq!(files, ROOT_DL_OFFSETS.len());
}

#[test]
fn table_is_sorted() {
    assert!(ROOT_DL_OFFSETS.windows(2).all(|w| w[0].0 < w[1].0));
    assert!(ROOT_DL_OFFSETS
        .iter()
        .all(|(_, o)| !o.is_empty() && o.windows(2).all(|w| w[0] < w[1])));
}
