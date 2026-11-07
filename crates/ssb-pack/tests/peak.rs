//! The on-device build's memory bounds (RE-588), by a counting global
//! allocator (`examples/peak.rs` reports the same per phase):
//!
//! - the installer's shape (the ROM paged from its file, the blob spilled
//!   to a scratch file) peaks under 10 MiB of live heap, the page cache
//!   included: a PSP-1000's free memory, less the game;
//! - the host's shape (the ROM in a buffer, nothing spilled) under 40 MiB,
//!   the buffer included.
//!
//! One test: the counter is process-wide.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

struct Counting;

static CUR: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

fn grow(n: usize) {
    PEAK.fetch_max(CUR.fetch_add(n, Relaxed) + n, Relaxed);
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(l) };
        if !p.is_null() {
            grow(l.size());
        }
        p
    }
    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        let p = unsafe { System.alloc_zeroed(l) };
        if !p.is_null() {
            grow(l.size());
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) };
        CUR.fetch_sub(l.size(), Relaxed);
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, new: usize) -> *mut u8 {
        let q = unsafe { System.realloc(p, l, new) };
        if !q.is_null() {
            // A moving realloc holds both blocks for a moment.
            grow(new);
            CUR.fetch_sub(l.size(), Relaxed);
        }
        q
    }
}

#[global_allocator]
static A: Counting = Counting;

/// The bounds: paged and spilled, buffered.
const PAGED_LIMIT: usize = 10 << 20;
const LIMIT: usize = 40 << 20;
/// The installer's ROM cache: 16 pages of 16 KiB (RE-588 item 14).
const ROM_PAGE: usize = 16 << 10;
const ROM_PAGES: usize = 16;

/// Counts the bytes written; keeps none.
struct Discard(usize);

impl ssb_rom::pack::PackSink for Discard {
    fn write(&mut self, bytes: &[u8]) -> Result<(), ssb_rom::pack::SinkError> {
        self.0 += bytes.len();
        Ok(())
    }
}

/// The installer's progress line (`ssb_pack::install::Progress`), fed as
/// the PSP feeds it: never down, steps only inside a phase, 1000 at the
/// end. Allocates nothing (the peak counts every byte).
#[derive(Default)]
struct Track {
    progress: core::cell::Cell<ssb_pack::install::Progress>,
    last: core::cell::Cell<u32>,
    steps: core::cell::Cell<usize>,
    fell: core::cell::Cell<bool>,
}

impl Track {
    fn check(&self) {
        let now = self.progress.get().per_mille();
        if now < self.last.get() {
            self.fell.set(true);
        }
        self.last.set(now);
    }
}

impl ssb_pack::PackLog for Track {
    fn line(&self, _args: core::fmt::Arguments<'_>) {}

    fn phase(&self, name: &str) {
        let mut p = self.progress.get();
        p.phase_ended(name);
        self.progress.set(p);
        self.check();
    }

    fn progress(&self, done: usize, total: usize) {
        let mut p = self.progress.get();
        p.step(done, total);
        self.progress.set(p);
        self.steps.set(self.steps.get() + 1);
        self.check();
    }
}

/// One build's peak live heap above the bytes live before it, and the
/// pack's length.
fn peak_of(
    rom: ssb_rom::rom_read::RomRef<'_>,
    spill: Option<Box<dyn ssb_rom::blob_store::BlobSpill>>,
) -> (usize, usize) {
    let before = CUR.load(Relaxed);
    PEAK.store(before, Relaxed);
    let mut sink = Discard(0);
    let track = Track::default();
    ssb_pack::build_with(
        rom,
        &ssb_pack::PackOptions::default(),
        &track,
        &ssb_pack::NoObserver,
        &mut sink,
        spill,
    )
    .unwrap();
    assert!(!track.fell.get(), "the installer's progress went down");
    assert_eq!(track.progress.get().per_mille(), 1000, "every phase ended");
    assert!(
        track.steps.get() > 100,
        "{} mesh-phase steps",
        track.steps.get()
    );
    // The test harness's own live bytes are not the build's.
    (PEAK.load(Relaxed) - before, sink.0)
}

#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn pack_build_peaks_under_its_bounds() {
    let Some(path) = ssb_tablegen::test_rom(env!("CARGO_MANIFEST_DIR")) else {
        return;
    };
    let mib = |n: usize| n as f64 / (1 << 20) as f64;

    // The installer: the page cache is inside the measured peak.
    let spill_path = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("peak-spill.tmp");
    let spill_file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(&spill_path)
        .unwrap();
    let file = std::fs::File::open(&path).unwrap();
    let len = file.metadata().unwrap().len() as usize;
    let paged = ssb_rom::rom_read::PagedRom::new(file, len, ROM_PAGE, ROM_PAGES).unwrap();
    let spill = Box::new(ssb_rom::blob_store::FileSpill::new(spill_file));
    let (peak, paged_len) = peak_of((&paged).into(), Some(spill));
    std::fs::remove_file(&spill_path).unwrap();
    assert!(paged_len > len, "a pack was written ({paged_len} bytes)");
    assert!(
        peak <= PAGED_LIMIT,
        "paged, spilled pack build peaked at {peak} bytes ({:.2} MiB), over {} MiB",
        mib(peak),
        PAGED_LIMIT >> 20
    );
    drop(paged);

    let rom = std::fs::read(&path).unwrap();
    let (peak, len) = peak_of((&rom).into(), None);
    let peak = peak + rom.len();
    assert_eq!(len, paged_len, "the same pack either way");
    assert!(
        peak <= LIMIT,
        "pack build peaked at {peak} bytes ({:.2} MiB), over {} MiB",
        mib(peak),
        LIMIT >> 20
    );
}
