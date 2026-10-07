//! Scene loading by archive file (RE-475, D-046).
//!
//! The N64 keeps a few files for the whole run and reads each scene's
//! files into the scene's heap when it starts (`lbRelocLoadFilesListed`,
//! with every extern file they point at), dropping them when the next
//! scene starts. The pack groups its blob by archive file
//! ([`ssb_rom::pack::FileDesc`]); the game holds the tables and the shared
//! region from boot ([`crate::assets::load_pack`]) and this module reads
//! the files:
//!
//! * [`enter`] at a scene's start: the scene's files and their extern
//!   closure. Files the last scene held that this one does not are
//!   dropped at the next [`safe_point`], after the frame that may still
//!   draw them; files both hold stay where they are.
//! * [`persist`]: files every scene keeps.
//! * An offset no loaded file holds ([`ssb_rom::residency`]'s miss hook)
//!   loads the file holding it on demand, counted in [`Report`] and named
//!   in [`demand_files`]: a scene whose list misses a file still draws,
//!   one load later.
//!
//! * [`prefetch`]: the files the next scene will hold, read in the
//!   background (`sceIoReadAsync`) while the current scene runs, as the
//!   N64's cartridge DMA hides behind its scene changes (RE-476). The next
//!   [`enter`] keeps the ones it wants and waits for any read still running;
//!   a scene that turns out to be another one drops them.
//!
//! Adjacent files are read with one `sceIoRead` into one block. A block
//! is freed when none of its files is held, once no display list can read
//! it.
//!
//! The background reads complete on the game thread: [`safe_point`] polls
//! the read in flight, registers its files and starts the next, so the
//! registry and the allocator stay single-threaded. A block a read fills is
//! on whole cache lines and written back before its files are registered,
//! so the GE never reads a line the CPU holds.

use alloc::vec::Vec;

use psp::sys;
use ssb_rom::pack::{FileDesc, Pack};
use ssb_rom::residency::{self, Span};

use crate::assets::{AlignedBuf, LoadError};

/// No block.
const NO_BLOCK: u16 = u16::MAX;
/// The most bytes one read fills: larger runs split.
const MAX_RUN: u32 = 4 << 20;
/// The most bytes one background read fills: a scene that wants none of
/// a prefetch waits for at most one of these (RE-476).
const MAX_BACKGROUND_RUN: u32 = 1 << 20;
/// Free memory a prefetch leaves: the scene's own allocations (a battle's
/// fighters and effect players) come out of the same heap, and `psp`'s
/// allocation-error handler never returns (RE-475).
const PREFETCH_MARGIN: usize = 6 << 20;
/// Demand-loaded files remembered for [`demand_files`].
const DEMAND_LOG: usize = 16;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Keep {
    No,
    Scene,
    /// The last scene's, until [`safe_point`].
    Leaving,
    Always,
    /// Read ahead for the next scene ([`prefetch`]), until its [`enter`].
    Next,
}

#[derive(Clone, Copy)]
struct FileState {
    block: u16,
    keep: Keep,
    /// A background read in [`Loader::jobs`] fills it.
    queued: bool,
}

/// One background read: files `from..=last`, blob `start..start + len`.
struct Job {
    from: usize,
    last: usize,
    start: u32,
    len: u32,
    buf: AlignedBuf,
}

struct Block {
    buf: AlignedBuf,
    /// Files in this block still held.
    live: u32,
}

/// The last [`enter`]'s load, and the demand loads since.
#[derive(Clone, Copy, Default, Debug)]
pub struct Report {
    pub scene: &'static str,
    /// Files the scene holds, loaded now or kept from the last scene.
    pub files: u32,
    /// Bytes those files hold.
    pub bytes: u32,
    /// Files and bytes read for this scene, and the reads it took.
    pub loaded_files: u32,
    pub loaded_bytes: u32,
    pub reads: u32,
    /// Microseconds the reads took.
    pub micros: u32,
    /// Files loaded on demand since, and their bytes.
    pub demand_files: u32,
    pub demand_bytes: u32,
    /// Files and bytes a [`prefetch`] had read before this scene started.
    pub prefetched_files: u32,
    pub prefetched_bytes: u32,
    /// Microseconds [`enter`] waited for background reads still running.
    pub wait_micros: u32,
}

struct Loader {
    pack: Pack<'static>,
    /// NUL-terminated.
    path: &'static str,
    files: Vec<FileState>,
    blocks: Vec<Option<Block>>,
    retired: Vec<AlignedBuf>,
    report: Report,
    demand: [u16; DEMAND_LOG],
    demand_len: usize,
    failed: Option<LoadError>,
    /// Background reads, the first in flight once started.
    jobs: alloc::collections::VecDeque<Job>,
    /// The pack opened for [`Loader::jobs`], and for the reads that wait,
    /// kept open: opening it costs about 10 ms under PPSSPP (RE-476). A
    /// failed read reopens it once (a descriptor does not survive standby).
    fd: Option<sys::SceUid>,
    sync_fd: Option<sys::SceUid>,
    in_flight: bool,
}

struct Cell(core::cell::UnsafeCell<Option<Loader>>);
// SAFETY: one thread (the game's) loads and draws; see `residency`.
unsafe impl Sync for Cell {}
static LOADER: Cell = Cell(core::cell::UnsafeCell::new(None));

fn loader() -> Option<&'static mut Loader> {
    // SAFETY: single-threaded (above); no caller holds the reference across
    // another call into this module.
    unsafe { (*LOADER.0.get()).as_mut() }
}

/// Starts serving the pack opened on `resident` (the bytes
/// [`crate::assets::load_pack`] read from `path`): its archive files load
/// from `path` from now on.
///
/// `resident` must never be freed.
pub fn boot(resident: &'static [u8], path: &'static str) -> Result<(), LoadError> {
    let pack = Pack::open(resident).map_err(|_| LoadError::Empty)?;
    let count = pack.file_count() as usize;
    let mut files = Vec::new();
    files
        .try_reserve_exact(count)
        .map_err(|_| LoadError::OutOfMemory)?;
    files.resize(
        count,
        FileState {
            block: NO_BLOCK,
            keep: Keep::No,
            queued: false,
        },
    );
    pack.build_indexes();
    // SAFETY: single-threaded; set before any lookup.
    unsafe {
        *LOADER.0.get() = Some(Loader {
            pack,
            path,
            files,
            blocks: Vec::new(),
            retired: Vec::new(),
            report: Report::default(),
            demand: [0; DEMAND_LOG],
            demand_len: 0,
            failed: None,
            jobs: alloc::collections::VecDeque::new(),
            fd: None,
            sync_fd: None,
            in_flight: false,
        });
        residency::install(resident.as_ptr(), Some(on_miss));
    }
    // Opened now rather than in a scene's first frame (RE-476).
    if let Some(l) = loader() {
        open_fd(&mut l.fd, path);
        open_fd(&mut l.sync_fd, path);
    }
    Ok(())
}

/// The pack [`boot`] opened, for code that reads a scene's data without
/// being handed the pack (the scenes that had packs of their own before
/// RE-475).
pub fn pack() -> Option<&'static Pack<'static>> {
    loader().map(|l| &l.pack)
}

/// Whether [`boot`] ran.
pub fn active() -> bool {
    loader().is_some()
}

/// Holds `roots` and their extern closure for the rest of the run.
pub fn persist(roots: &[u32]) -> Result<(), LoadError> {
    let Some(l) = loader() else { return Ok(()) };
    let want = l.closure(roots);
    for (i, w) in want.iter().enumerate() {
        if *w {
            l.files[i].keep = Keep::Always;
        }
    }
    l.load_missing(&want)?;
    Ok(())
}

/// Starts scene `scene`: holds `roots` and their extern closure, drops the
/// last scene's other files, and reads what is missing.
pub fn enter(scene: &'static str, roots: &[u32]) -> Result<Report, LoadError> {
    enter_with(scene, roots, false)
}

/// [`enter`] for a battle the running scene starts (an opening fight,
/// How to Play): a [`prefetch`] for the scene after it carries on.
pub fn enter_within(scene: &'static str, roots: &[u32]) -> Result<Report, LoadError> {
    enter_with(scene, roots, true)
}

fn enter_with(scene: &'static str, roots: &[u32], within: bool) -> Result<Report, LoadError> {
    let Some(l) = loader() else {
        return Ok(Report::default());
    };
    let want = l.closure(roots);
    // A prefetch for another scene: its reads not yet started are
    // dropped; the one in flight, and the ones this scene wants, are
    // waited for. Within a scene, only reads of files it wants are.
    let wait = if within {
        l.jobs.iter().any(|j| (j.from..=j.last).any(|i| want[i]))
    } else {
        l.cancel_unwanted(&want);
        !l.jobs.is_empty()
    };
    let waited = crate::profile::start();
    let started = unsafe { sys::sceKernelGetSystemTimeLow() };
    let had_jobs = wait;
    if wait {
        l.drain();
    }
    let wait_micros = if had_jobs {
        crate::profile::stop(crate::profile::Span::Io, waited);
        unsafe { sys::sceKernelGetSystemTimeLow() }.wrapping_sub(started)
    } else {
        0
    };
    let (mut prefetched_files, mut prefetched_bytes) = (0, 0);
    // The last scene's files stay readable until the frame ends: the
    // frame that starts a scene may still draw the one it leaves.
    for (i, w) in want.iter().enumerate() {
        let held = l.files[i].block != NO_BLOCK;
        let keep = &mut l.files[i].keep;
        if *w && *keep == Keep::Next && held {
            prefetched_files += 1;
            prefetched_bytes += l.pack.file_at(i as u32).map_or(0, |d| d.len);
        }
        if *w && *keep != Keep::Always {
            *keep = Keep::Scene;
        } else if !*w && (*keep == Keep::Scene || (*keep == Keep::Next && !within)) {
            *keep = Keep::Leaving;
        }
    }
    l.report = Report {
        scene,
        prefetched_files,
        prefetched_bytes,
        wait_micros,
        ..Report::default()
    };
    l.demand_len = 0;
    l.load_missing(&want)?;
    for (i, f) in l.files.iter().enumerate() {
        if matches!(f.keep, Keep::Scene | Keep::Always) {
            l.report.files += 1;
            l.report.bytes += l.pack.file_at(i as u32).map_or(0, |d| d.len);
        }
    }
    Ok(l.report)
}

/// Starts reading `roots` and their extern closure in the background,
/// for the scene expected next: the current scene keeps running while
/// [`safe_point`] completes the reads. The next [`enter`] keeps the files
/// it wants and drops the rest. Files already held are not read again.
///
/// Reads stop being queued once fewer than [`PREFETCH_MARGIN`] bytes
/// would be left free; the scene's [`enter`] then reads the rest.
pub fn prefetch(roots: &[u32]) {
    let Some(l) = loader() else { return };
    let want = l.closure(roots);
    for (i, w) in want.iter().enumerate() {
        let keep = &mut l.files[i].keep;
        if *w && matches!(*keep, Keep::No | Keep::Leaving) {
            *keep = Keep::Next;
        }
    }
    l.queue_missing(&want);
    l.pump();
}

/// Whether background reads are queued or running.
pub fn busy() -> bool {
    loader().is_some_and(|l| !l.jobs.is_empty())
}

/// The current scene's load report.
pub fn report() -> Report {
    loader().map(|l| l.report).unwrap_or_default()
}

/// The archive files loaded on demand since the last [`enter`] (the
/// first [`DEMAND_LOG`]).
pub fn demand_files() -> Vec<u16> {
    loader().map_or_else(Vec::new, |l| l.demand[..l.demand_len].to_vec())
}

/// A load that failed outside [`enter`] (on demand), once.
pub fn take_failure() -> Option<LoadError> {
    loader().and_then(|l| l.failed.take())
}

/// Drops the files the last [`enter`] left and frees their blocks. Call
/// after `Gpu::end_frame`, when the GE has read everything.
pub fn safe_point() {
    if let Some(l) = loader() {
        let t = crate::profile::start();
        l.pump();
        // The registry drops them all in one pass (RE-476).
        let mut starts: Vec<u32> = Vec::new();
        for i in 0..l.files.len() {
            if l.files[i].keep == Keep::Leaving {
                if let Some(start) = l.drop_file(i) {
                    starts.push(start);
                }
            }
        }
        if !starts.is_empty() {
            // SAFETY: single-threaded; nothing drawn after this resolves
            // them. `starts` ascends with the file index.
            unsafe { residency::remove_where(|s| starts.binary_search(&s).is_ok()) };
        }
        l.collect_retired();
        crate::profile::stop(crate::profile::Span::Files, t);
    }
}

/// Bytes held in blocks, and how many blocks.
pub fn held() -> (usize, usize) {
    loader().map_or((0, 0), |l| {
        let live = l.blocks.iter().flatten();
        (
            live.clone().map(|b| b.buf.as_slice().len()).sum(),
            live.count(),
        )
    })
}

fn on_miss(offset: u32, _len: u32) -> bool {
    let Some(l) = loader() else { return false };
    let Some(i) = l.pack.file_at_offset(offset) else {
        return false;
    };
    let i = i as usize;
    if l.files[i].queued {
        let Some(k) = l.jobs.iter().position(|j| (j.from..=j.last).contains(&i)) else {
            return false;
        };
        if k == 0 && l.in_flight {
            // The read in flight holds it: wait for that read rather than
            // read the file twice.
            let t = crate::profile::start();
            while l.files[i].queued && l.complete(true) {}
            crate::profile::stop(crate::profile::Span::Demand, t);
            crate::profile::stop(crate::profile::Span::Io, t);
            return l.files[i].block != NO_BLOCK;
        }
        // A read not yet started: the file is read now, on its own, and
        // the rest of that read queued again behind the others.
        if let Some(job) = l.jobs.remove(k) {
            let mut rest = alloc::vec![false; l.files.len()];
            for f in job.from..=job.last {
                l.files[f].queued = false;
                rest[f] = f != i;
            }
            drop(job);
            let loaded = demand_load(l, i);
            l.queue_missing(&rest);
            return loaded;
        }
        return false;
    }
    if l.files[i].block != NO_BLOCK {
        return false;
    }
    demand_load(l, i)
}

/// Reads file `i`, which a scene needs now.
fn demand_load(l: &mut Loader, i: usize) -> bool {
    let mut want = alloc::vec![false; l.files.len()];
    want[i] = true;
    if l.files[i].keep == Keep::No {
        l.files[i].keep = Keep::Scene;
    }
    let before = (l.report.loaded_files, l.report.loaded_bytes);
    let t = crate::profile::start();
    let loaded = l.load_missing(&want);
    crate::profile::stop(crate::profile::Span::Demand, t);
    match loaded {
        Ok(()) => {
            l.report.demand_files += l.report.loaded_files - before.0;
            l.report.demand_bytes += l.report.loaded_bytes - before.1;
            if l.demand_len < DEMAND_LOG {
                l.demand[l.demand_len] =
                    l.pack.file_at(i as u32).map_or(u16::MAX, |d| d.file as u16);
                l.demand_len += 1;
            }
            true
        }
        Err(e) => {
            l.files[i].keep = Keep::No;
            l.failed = Some(e);
            false
        }
    }
}

impl Loader {
    /// `roots` and every file they reach through their extern IDs, by
    /// file-table index.
    fn closure(&self, roots: &[u32]) -> Vec<bool> {
        let mut want = alloc::vec![false; self.files.len()];
        let mut queue: Vec<u32> = roots.to_vec();
        while let Some(file) = queue.pop() {
            let Some(i) = self.pack.file_index(file) else {
                continue;
            };
            if core::mem::replace(&mut want[i as usize], true) {
                continue;
            }
            if let Some(d) = self.pack.file_at(i) {
                queue.extend((0..d.dep_count).filter_map(|k| self.pack.file_dep(&d, k)));
            }
        }
        want
    }

    /// Forgets file `i` and releases its share of its block. Returns its
    /// blob start, whose span the caller unregisters.
    fn drop_file(&mut self, i: usize) -> Option<u32> {
        let f = self.files[i];
        self.files[i] = FileState {
            block: NO_BLOCK,
            keep: Keep::No,
            queued: false,
        };
        if f.block == NO_BLOCK {
            return None;
        }
        let start = self.pack.file_at(i as u32).map(|d| d.blob_start);
        let b = &mut self.blocks[f.block as usize];
        if let Some(block) = b {
            block.live -= 1;
            if block.live == 0 {
                if let Some(block) = b.take() {
                    self.retired.push(block.buf);
                }
            }
        }
        start
    }

    fn collect_retired(&mut self) {
        if !crate::gu::GE_LIST_OPEN.load(core::sync::atomic::Ordering::Relaxed) {
            self.retired.clear();
        }
    }

    /// Reads every wanted file not yet loaded, adjacent ones together.
    fn load_missing(&mut self, want: &[bool]) -> Result<(), LoadError> {
        let mut i = 0;
        while i < self.files.len() {
            let missing = |i: usize, s: &Self| {
                want[i]
                    && s.files[i].block == NO_BLOCK
                    && s.pack.file_at(i as u32).is_some_and(|d| d.len > 0)
            };
            if !missing(i, self) {
                i += 1;
                continue;
            }
            let first = self.pack.file_at(i as u32).unwrap_or_default();
            let mut end = first.blob_start + first.len;
            let mut last = i;
            let mut j = i + 1;
            while j < self.files.len() {
                let Some(d) = self.pack.file_at(j as u32) else {
                    break;
                };
                if d.len == 0 {
                    j += 1;
                    continue;
                }
                if !missing(j, self)
                    || d.blob_start != end.next_multiple_of(16)
                    || d.blob_start + d.len - first.blob_start > MAX_RUN
                {
                    break;
                }
                end = d.blob_start + d.len;
                last = j;
                j += 1;
            }
            self.read_run(first, i, last, end)?;
            i = last + 1;
        }
        Ok(())
    }

    /// Reads files `first..=last` (blob `first.blob_start..end`) into one
    /// block and registers each.
    fn read_run(
        &mut self,
        first: FileDesc,
        from: usize,
        last: usize,
        end: u32,
    ) -> Result<(), LoadError> {
        let len = (end - first.blob_start) as usize;
        let buf = AlignedBuf::cache_lines(len).ok_or(LoadError::OutOfMemory)?;
        let started = unsafe { sys::sceKernelGetSystemTimeLow() };
        let at = self.pack.blob_start() as i64 + i64::from(first.blob_start);
        self.read_sync(at, buf.as_mut_ptr(), len)?;
        buf.flush_cache();
        crate::profile::stop(crate::profile::Span::Io, started);
        self.report.micros += unsafe { sys::sceKernelGetSystemTimeLow() }.wrapping_sub(started);
        self.report.reads += 1;
        self.register(buf, first.blob_start, from, last, true);
        Ok(())
    }

    /// Registers files `from..=last`, read into `buf` from blob offset
    /// `start`, as one block. `count` adds them to the scene's report.
    fn register(&mut self, buf: AlignedBuf, start: u32, from: usize, last: usize, count: bool) {
        let slot = match self.blocks.iter().position(Option::is_none) {
            Some(s) => s,
            None => {
                self.blocks.push(None);
                self.blocks.len() - 1
            }
        };
        let mut live = 0;
        let mut run: Vec<Span> = Vec::with_capacity(last + 1 - from);
        for k in from..=last {
            let Some(d) = self.pack.file_at(k as u32) else {
                continue;
            };
            if d.len == 0 {
                continue;
            }
            run.push(Span {
                start: d.blob_start,
                len: d.len,
                // SAFETY: inside the block, which holds the run's bytes.
                ptr: unsafe { buf.as_ptr().add((d.blob_start - start) as usize) },
            });
            self.files[k].block = slot as u16;
            self.files[k].queued = false;
            live += 1;
            if count {
                self.report.loaded_files += 1;
                self.report.loaded_bytes += d.len;
            }
        }
        // SAFETY: the block stays allocated while any of its files is
        // registered (`drop_file`); one read's files are adjacent.
        unsafe { residency::insert_run(&run) };
        self.blocks[slot] = Some(Block { buf, live });
    }

    /// Queues background reads of every wanted file neither held nor
    /// queued, adjacent ones together, while memory allows.
    fn queue_missing(&mut self, want: &[bool]) {
        let mut i = 0;
        while i < self.files.len() {
            let missing = |i: usize, s: &Self| {
                want[i]
                    && s.files[i].block == NO_BLOCK
                    && !s.files[i].queued
                    && s.pack.file_at(i as u32).is_some_and(|d| d.len > 0)
            };
            if !missing(i, self) {
                i += 1;
                continue;
            }
            let first = self.pack.file_at(i as u32).unwrap_or_default();
            let mut end = first.blob_start + first.len;
            let mut last = i;
            let mut j = i + 1;
            while j < self.files.len() {
                let Some(d) = self.pack.file_at(j as u32) else {
                    break;
                };
                if d.len == 0 {
                    j += 1;
                    continue;
                }
                if !missing(j, self)
                    || d.blob_start != end.next_multiple_of(16)
                    || d.blob_start + d.len - first.blob_start > MAX_BACKGROUND_RUN
                {
                    break;
                }
                end = d.blob_start + d.len;
                last = j;
                j += 1;
            }
            let len = end - first.blob_start;
            if crate::memory::max_block() < len as usize + PREFETCH_MARGIN {
                return;
            }
            let Some(buf) = AlignedBuf::cache_lines(len as usize) else {
                return;
            };
            for k in i..=last {
                self.files[k].queued = self.pack.file_at(k as u32).is_some_and(|d| d.len > 0);
            }
            self.jobs.push_back(Job {
                from: i,
                last,
                start: first.blob_start,
                len,
                buf,
            });
            i = last + 1;
        }
    }

    /// Drops the queued reads not yet started that hold no file `want`
    /// names.
    fn cancel_unwanted(&mut self, want: &[bool]) {
        let skip = usize::from(self.in_flight);
        let mut k = skip;
        while k < self.jobs.len() {
            let j = &self.jobs[k];
            if (j.from..=j.last).any(|i| want[i]) {
                k += 1;
                continue;
            }
            if let Some(job) = self.jobs.remove(k) {
                for i in job.from..=job.last {
                    self.files[i].queued = false;
                    if self.files[i].keep == Keep::Next && self.files[i].block == NO_BLOCK {
                        self.files[i].keep = Keep::No;
                    }
                }
            }
        }
    }

    /// Completes the read in flight if it has finished, and starts the
    /// next; never waits.
    fn pump(&mut self) {
        loop {
            if !self.in_flight && !self.start_next() {
                return;
            }
            if !self.complete(false) {
                return;
            }
        }
    }

    /// Waits for every queued read.
    fn drain(&mut self) {
        while self.in_flight || self.start_next() {
            self.complete(true);
        }
    }

    /// Starts the first queued read; `false` when none is queued or it
    /// could not start (its files then load when a scene wants them).
    fn start_next(&mut self) -> bool {
        while let Some(job) = self.jobs.front() {
            let at = self.pack.blob_start() as i64 + i64::from(job.start);
            let (dst, len) = (job.buf.as_mut_ptr(), job.len);
            for attempt in 0..2 {
                if attempt == 1 {
                    close_fd(&mut self.fd);
                }
                let Some(fd) = open_fd(&mut self.fd, self.path) else {
                    break;
                };
                // SAFETY: the job's buffer holds `len` bytes and stays in
                // the queue, unmoved on the heap, until the read completes.
                let ok = unsafe {
                    sys::sceIoLseek(fd, at, sys::IoWhence::Set) == at
                        && sys::sceIoReadAsync(fd, dst as *mut core::ffi::c_void, len) >= 0
                };
                if ok {
                    self.in_flight = true;
                    return true;
                }
            }
            self.abandon_front();
        }
        false
    }

    /// Reads `len` bytes at pack offset `at` into `dst`, waiting.
    fn read_sync(&mut self, at: i64, dst: *mut u8, len: usize) -> Result<(), LoadError> {
        for attempt in 0..2 {
            if attempt == 1 {
                close_fd(&mut self.sync_fd);
            }
            let Some(fd) = open_fd(&mut self.sync_fd, self.path) else {
                return Err(LoadError::NotFound);
            };
            // SAFETY: `dst` holds `len` bytes.
            let ok = unsafe {
                sys::sceIoLseek(fd, at, sys::IoWhence::Set) == at
                    && sys::sceIoRead(fd, dst as *mut core::ffi::c_void, len as u32) as usize == len
            };
            if ok {
                return Ok(());
            }
        }
        Err(LoadError::ShortRead)
    }

    /// Finishes the read in flight: registers its files, or gives them up
    /// if the read failed. `wait` blocks until it ends; otherwise returns
    /// `false` while it runs.
    fn complete(&mut self, wait: bool) -> bool {
        let Some(fd) = self.fd.filter(|_| self.in_flight) else {
            return false;
        };
        let mut res: i64 = 0;
        // SAFETY: `fd` has the read in flight.
        let status = unsafe {
            if wait {
                sys::sceIoWaitAsync(fd, &mut res)
            } else {
                sys::sceIoPollAsync(fd, &mut res)
            }
        };
        if status == 1 {
            return false;
        }
        self.in_flight = false;
        let Some(job) = self.jobs.pop_front() else {
            return true;
        };
        if status < 0 || res != i64::from(job.len) {
            // Its files load when a scene wants them; the descriptor is
            // reopened for the next read.
            self.release_job_files(&job);
            close_fd(&mut self.fd);
        } else {
            job.buf.flush_cache();
            self.register(job.buf, job.start, job.from, job.last, false);
        }
        true
    }

    fn release_job_files(&mut self, job: &Job) {
        for i in job.from..=job.last {
            self.files[i].queued = false;
            if self.files[i].keep == Keep::Next {
                self.files[i].keep = Keep::No;
            }
        }
    }

    fn abandon_front(&mut self) {
        if let Some(job) = self.jobs.pop_front() {
            self.release_job_files(&job);
        }
    }
}

/// `fd`, opening the NUL-terminated `path` into it if it is closed.
fn open_fd(fd: &mut Option<sys::SceUid>, path: &str) -> Option<sys::SceUid> {
    if fd.is_none() {
        // SAFETY: `path` is NUL-terminated.
        let f = unsafe { sys::sceIoOpen(path.as_ptr(), sys::IoOpenFlags::RD_ONLY, 0o777) };
        if f.0 >= 0 {
            *fd = Some(f);
        }
    }
    *fd
}

/// Closes `fd`, which has no read in flight.
fn close_fd(fd: &mut Option<sys::SceUid>) {
    if let Some(f) = fd.take() {
        // SAFETY: the caller has no read in flight on it.
        unsafe { sys::sceIoClose(f) };
    }
}
