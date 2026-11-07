//! The paged ROM reader (RE-588) against the whole image: every archive
//! file and the audio section read the same through a `PagedRom` in each
//! cartridge byte order as through the `.z64` slice.

use ssb_rom::archive::Archive;
use ssb_rom::rom::{self, ByteOrder};
use ssb_rom::rom_read::{PagedRom, RomRef};
use ssb_rom::{audio, Region};

fn rom() -> Option<Vec<u8>> {
    Some(std::fs::read(ssb_tablegen::test_rom(env!("CARGO_MANIFEST_DIR"))?).unwrap())
}

/// `z64` in `order`: each swap is its own inverse.
fn swapped(z64: &[u8], order: ByteOrder) -> Vec<u8> {
    let mut out = z64.to_vec();
    order.to_z64(&mut out);
    out
}

#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn every_archive_file_reads_the_same_paged() {
    let Some(z64) = rom() else {
        return;
    };
    let slice = Archive::open(&z64, Region::Us).unwrap();
    let want: Vec<_> = (0..slice.len() as u32)
        .map(|id| {
            slice
                .load(id)
                .map(|f| (f.data, f.extern_relocs, f.intern_relocs))
        })
        .collect();
    assert!(want.iter().all(Result::is_ok), "every file loads");
    for order in [ByteOrder::Z64, ByteOrder::V64, ByteOrder::N64] {
        let image = swapped(&z64, order);
        let paged = PagedRom::new(&image[..], image.len(), 16 * 1024, 16).unwrap();
        assert_eq!(paged.order(), order);
        assert_eq!(
            rom::identify_reader(RomRef::from(&paged)).unwrap().region,
            Region::Us
        );
        let archive = Archive::open(&paged, Region::Us).unwrap();
        for (id, want) in want.iter().enumerate() {
            let got = archive
                .load(id as u32)
                .map(|f| (f.data, f.extern_relocs, f.intern_relocs));
            assert_eq!(&got, want, "file {id} in {}", order.name());
        }
    }
}

/// `Archive::load` decodes each VPK0 stream from its exact on-ROM range
/// (what one paged read holds); the decoder stops at the same byte as over
/// the rest of the ROM, the range the loader gave it before (RE-588).
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn vpk0_streams_end_inside_their_range() {
    let Some(z64) = rom() else {
        return;
    };
    let archive = Archive::open(&z64, Region::Us).unwrap();
    let mut compressed = 0;
    for (id, entry) in archive.entries()[..archive.len()].iter().enumerate() {
        if !entry.compressed {
            continue;
        }
        compressed += 1;
        let at = archive.data_base() + entry.data_offset as usize;
        let exact = ssb_rom::vpk0::decompress(&z64[at..at + entry.rom_size()]);
        let open = ssb_rom::vpk0::decompress(&z64[at..]);
        assert_eq!(exact, open, "file {id}");
        assert!(exact.is_ok(), "file {id}");
    }
    assert!(compressed > 0, "no compressed files");
}

#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn streamed_audio_section_matches_the_built_one() {
    let Some(z64) = rom() else {
        return;
    };
    let built = audio::build_section(&z64).unwrap();
    assert_eq!(audio::section_len(RomRef::from(&z64)).unwrap(), built.len());
    let mut streamed = Vec::new();
    let n = audio::write_section(RomRef::from(&z64), &mut streamed).unwrap();
    assert_eq!(n, built.len());
    assert!(streamed == built, "slice-streamed section differs");

    let image = swapped(&z64, ByteOrder::N64);
    let paged = PagedRom::new(&image[..], image.len(), 16 * 1024, 8).unwrap();
    let mut streamed = Vec::new();
    audio::write_section(RomRef::from(&paged), &mut streamed).unwrap();
    assert!(streamed == built, "paged-streamed section differs");
}
