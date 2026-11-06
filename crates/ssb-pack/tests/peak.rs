//! The on-device build's memory bound (RE-588): one pack build, the ROM
//! buffer included, peaks under 40 MiB of live heap (a PSP-2000's user
//! memory, less the installer). A counting global allocator measures it;
//! `examples/peak.rs` reports the same per phase.

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

/// The bound: 40 MiB.
const LIMIT: usize = 40 << 20;

/// Counts the bytes written; keeps none.
struct Discard(usize);

impl ssb_rom::pack::PackSink for Discard {
    fn write(&mut self, bytes: &[u8]) -> Result<(), ssb_rom::pack::SinkError> {
        self.0 += bytes.len();
        Ok(())
    }
}

#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn pack_build_peaks_under_40_mib() {
    let Some(path) = ssb_tablegen::test_rom(env!("CARGO_MANIFEST_DIR")) else {
        return;
    };
    let rom = std::fs::read(path).unwrap();
    let opts = ssb_pack::PackOptions::default();
    let before = CUR.load(Relaxed) - rom.len();
    PEAK.store(CUR.load(Relaxed), Relaxed);
    let mut sink = Discard(0);
    ssb_pack::build(
        &rom,
        &opts,
        &ssb_pack::NoLog,
        &ssb_pack::NoObserver,
        &mut sink,
    )
    .unwrap();
    // The test harness's own live bytes are not the build's.
    let peak = PEAK.load(Relaxed) - before;
    assert!(sink.0 > rom.len(), "a pack was written ({} bytes)", sink.0);
    assert!(
        peak <= LIMIT,
        "pack build peaked at {peak} bytes ({:.2} MiB), over {} MiB",
        peak as f64 / (1 << 20) as f64,
        LIMIT >> 20
    );
}
