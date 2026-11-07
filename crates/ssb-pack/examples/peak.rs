//! Peak live heap of one pack build (RE-588): a counting global allocator
//! reports live and peak bytes at each `PackLog::phase`, the caller's ROM
//! buffer included (what the PSP installer's heap pays).
//!
//! `SSB64_ROM=<rom> cargo run --release -p ssb-pack --example peak [-- OUT]`
//! writes the pack to `OUT` when given. The on-device build's shape:
//!
//! - `PEAK_PAGED=KIB,PAGES`: the ROM read through a `PagedRom` over the file
//!   (`KIB` KiB pages, at most `PAGES` held) instead of a buffer; any byte
//!   order.
//! - `PEAK_SPILL=PATH`: the blob's compressed pages spilled to `PATH`
//!   (deleted after).

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};
use std::time::Instant;

struct Counting;

static CUR: AtomicUsize = AtomicUsize::new(0);
/// Peak since the last phase.
static PEAK: AtomicUsize = AtomicUsize::new(0);
/// Peak of the whole run.
static TOTAL_PEAK: AtomicUsize = AtomicUsize::new(0);

fn grow(n: usize) {
    let c = CUR.fetch_add(n, Relaxed) + n;
    PEAK.fetch_max(c, Relaxed);
    let old = TOTAL_PEAK.fetch_max(c, Relaxed);
    if c > old && TRACE.load(Relaxed) && c > TRACED.load(Relaxed) + (64 << 10) {
        trace(n, c);
    }
}

/// `PEAK_TRACE=1`: the stack of the allocation that set the run's peak.
static TRACE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static TRACED: AtomicUsize = AtomicUsize::new(0);
static LAST_TRACE: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());

thread_local! {
    static IN_TRACE: Cell<bool> = const { Cell::new(false) };
}

fn trace(n: usize, c: usize) {
    if IN_TRACE.with(|t| t.replace(true)) {
        return;
    }
    let bt = std::backtrace::Backtrace::force_capture().to_string();
    let frames: Vec<&str> = bt
        .lines()
        .filter(|l| l.contains("ssb_") && !l.contains("peak::"))
        .take(12)
        .collect();
    *LAST_TRACE.lock().unwrap() = format!("alloc {n} B at live {c} B:\n{}", frames.join("\n"));
    TRACED.store(CUR.load(Relaxed), Relaxed);
    IN_TRACE.with(|t| t.set(false));
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

const MIB: f64 = 1024.0 * 1024.0;

/// ROM reads and bytes since the last phase (`PEAK_PAGED` only).
static ROM_READS: AtomicUsize = AtomicUsize::new(0);
static ROM_BYTES: AtomicUsize = AtomicUsize::new(0);

/// Counts the reads each phase makes of a [`PagedRom`].
struct Counted<'a, F: ssb_rom::rom_read::RawFile>(&'a ssb_rom::rom_read::PagedRom<F>);

impl<F: ssb_rom::rom_read::RawFile> ssb_rom::rom_read::RomRead for Counted<'_, F> {
    fn len(&self) -> usize {
        ssb_rom::rom_read::RomRead::len(self.0)
    }
    fn read_at(&self, at: usize, out: &mut [u8]) -> ssb_rom::Result<()> {
        ROM_READS.fetch_add(1, Relaxed);
        ROM_BYTES.fetch_add(out.len(), Relaxed);
        self.0.read_at(at, out)
    }
}

struct PhaseLog {
    start: Instant,
    last: Cell<f64>,
}

impl ssb_pack::PackLog for PhaseLog {
    fn line(&self, args: std::fmt::Arguments<'_>) {
        let line = args.to_string();
        if line.contains("blob store") || line.contains("file store") || line.contains("section") {
            eprintln!("{}", line.trim());
        }
    }

    fn phase(&self, name: &str) {
        let (cur, peak) = (CUR.load(Relaxed), PEAK.swap(CUR.load(Relaxed), Relaxed));
        let t = self.start.elapsed().as_secs_f64();
        let (reads, bytes) = (ROM_READS.swap(0, Relaxed), ROM_BYTES.swap(0, Relaxed));
        eprintln!(
            "{name:<22} live {:7.2} MiB  peak {:7.2} MiB  {:6.2} s (+{:.2})  rom {reads} reads {:.1} MB",
            cur as f64 / MIB,
            peak as f64 / MIB,
            t,
            t - self.last.get(),
            bytes as f64 / 1e6
        );
        self.last.set(t);
    }
}

fn main() {
    let rom_path = std::env::var("SSB64_ROM").expect("SSB64_ROM");
    TRACE.store(std::env::var_os("PEAK_TRACE").is_some(), Relaxed);
    let out = std::env::args().nth(1);
    let log = PhaseLog {
        start: Instant::now(),
        last: Cell::new(0.0),
    };
    let paged = std::env::var("PEAK_PAGED").ok().map(|v| {
        let (kib, pages) = v.split_once(',').expect("PEAK_PAGED=KIB,PAGES");
        let file = std::fs::File::open(&rom_path).expect("open ROM");
        let len = file.metadata().expect("ROM size").len() as usize;
        ssb_rom::rom_read::PagedRom::new(
            file,
            len,
            kib.parse::<usize>().expect("KIB") << 10,
            pages.parse().expect("PAGES"),
        )
        .expect("ROM byte order")
    });
    let buffered = match paged {
        Some(_) => Vec::new(),
        None => std::fs::read(&rom_path).expect("read ROM"),
    };
    let counted = paged.as_ref().map(Counted);
    let rom = match &counted {
        Some(counted) => ssb_rom::rom_read::RomRef::Dyn(counted),
        None => ssb_rom::rom_read::RomRef::from(&buffered),
    };
    let spill_path = std::env::var_os("PEAK_SPILL");
    let spill = spill_path.as_ref().map(|p| {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(p)
            .expect("create spill");
        Box::new(ssb_rom::blob_store::FileSpill::new(file))
            as Box<dyn ssb_rom::blob_store::BlobSpill>
    });
    ssb_pack::PackLog::phase(&log, "rom");
    let opts = ssb_pack::PackOptions::default();
    let path = out.unwrap_or_else(|| "/dev/null".into());
    let file = std::fs::File::create(&path).expect("create");
    let mut sink = ssb_rom::pack::IoSink(std::io::BufWriter::with_capacity(64 << 10, file));
    let stats = ssb_pack::build_with(rom, &opts, &log, &ssb_pack::NoObserver, &mut sink, spill)
        .expect("build");
    std::io::Write::flush(&mut sink.0).expect("flush");
    drop((sink, stats));
    if let Some(p) = &spill_path {
        std::fs::remove_file(p).expect("remove spill");
    }
    if let Some(paged) = &paged {
        let s = paged.stats();
        eprintln!(
            "rom {}: {} reads ({} B), {} page misses, {} direct; {} file reads ({} B)",
            paged.order().name(),
            s.reads,
            s.bytes,
            s.misses,
            s.direct,
            s.raw_reads,
            s.raw_bytes
        );
    }
    let len = std::fs::metadata(&path).map_or(0, |m| m.len());
    ssb_pack::PackLog::phase(&log, "done");
    if TRACE.load(Relaxed) {
        eprintln!("peak {}", LAST_TRACE.lock().unwrap());
    }
    eprintln!(
        "whole run peak {:.2} MiB ({} B); pack {len} B; {:.2} s",
        TOTAL_PEAK.load(Relaxed) as f64 / MIB,
        TOTAL_PEAK.load(Relaxed),
        log.start.elapsed().as_secs_f64()
    );
}
