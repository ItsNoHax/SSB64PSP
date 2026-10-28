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
//! The state machine (closure, read runs, keep states, background-read
//! queue, the [`ssb_engine::memory::AssetArena`] that owns the blocks) is
//! the portable [`ssb_rom::loader_plan::Plan`]; this module is its PSP
//! shell: the pack file's reads, the residency registry, cache-line blocks
//! ([`crate::assets::CacheLineBlocks`]) and the GE safe point.
//!
//! * Streamed files ([`stream_files`]): fighter figatrees, which the N64
//!   reads into each fighter's figatree heap on every status change
//!   (`ftMainSetStatus`, `ftmain.c:4619-4626`). A scene that streams them
//!   (the game names them per scene) holds only the common statuses' ones;
//!   any other is read on its first use into a [`GROUP_BYTES`] slab per
//!   fighter kind and stays there, least recently used out first
//!   ([`ssb_rom::stream_pool`], RE-572 option C, RE-573). A battle makes
//!   its slabs when it starts, as the N64 allocates each fighter's
//!   figatree heap at scene setup (`ftManagerAllocFigatreeHeapKind`,
//!   `scvsbattle.c:199`); the slabs' bookkeeping is sized then too, so a
//!   miss allocates nothing.
//!
//! The background reads run on the sample streamer's thread
//! ([`crate::sample_stream`], priority 0x1E), one [`CHUNK`] after another
//! ([`read_chunk`]), up to [`FRAME_BUDGET`] bytes between two
//! [`safe_point`]s; a frame that reads on the game thread (a miss, a scene's
//! own files) holds them after the chunk in flight until its safe point,
//! so its reads wait for at most that chunk (RE-574). They complete on the
//! game thread: [`safe_point`] sees the read done, registers its files and
//! starts the next, so the registry and the allocator stay single-threaded.
//! A block a read fills is on whole cache lines and written back before its
//! files are registered, so the GE never reads a line the CPU holds.

use alloc::vec::Vec;
use core::sync::atomic::{
    AtomicBool, AtomicI32, AtomicIsize, AtomicPtr, AtomicU8, AtomicUsize, Ordering,
};

use psp::sys;
use ssb_rom::loader_plan::{Io, Plan};
use ssb_rom::pack::Pack;
use ssb_rom::residency::{self, Span};
use ssb_rom::stream_pool::{Entry, StreamPool};

use crate::assets::{AlignedBuf, CacheLineBlocks, LoadError};

pub use ssb_rom::loader_plan::Report;

/// The pack file's reads, the residency registry and the heap, for the
/// [`Plan`].
struct PspIo {
    /// NUL-terminated.
    path: &'static str,
    /// The pack's blob offset in the file.
    blob: i64,
    /// The pack opened for background reads ([`JOB`]), and for the reads
    /// that wait, kept open: opening it costs about 10 ms under PPSSPP
    /// (RE-476). A failed read reopens it once (a descriptor does not
    /// survive standby).
    fd: Option<sys::SceUid>,
    sync_fd: Option<sys::SceUid>,
}

/// The longest single `sceIoRead`/`sceIoReadAsync` (RE-573): the Memory
/// Stick driver serves one request at a time, so a long read would hold the
/// sample streamer's page reads ([`crate::sample_stream`]) behind it. 64 KiB
/// is about 7 ms on a PSP-1000.
const CHUNK: usize = 64 * 1024;

/// The background bytes read between two [`safe_point`]s at most: four
/// chunks, more than a 60 FPS frame has time for (64 KiB is 6-9 ms on the
/// Slim, RE-573); a long frame reads no more than four (RE-574).
pub const FRAME_BUDGET: isize = 4 * CHUNK as isize;

/// [`Job::state`]s.
const IDLE: u8 = 0;
const RUNNING: u8 = 1;
const READ: u8 = 2;
const FAILED: u8 = 3;

/// The background read the streamer thread serves: `len` bytes from the
/// descriptor's position into `dst`, `done` of them read. The game thread
/// writes a job only while it is [`IDLE`] and publishes it with
/// [`RUNNING`]; the streamer thread then owns `done`, `dst`'s bytes and
/// the descriptor until it stores [`READ`] or [`FAILED`].
struct Job {
    fd: AtomicI32,
    dst: AtomicPtr<u8>,
    len: AtomicUsize,
    done: AtomicUsize,
    state: AtomicU8,
    /// Bytes left until the next [`safe_point`] ([`FRAME_BUDGET`]).
    budget: AtomicIsize,
    /// The game thread read this frame: no chunk starts until the
    /// [`safe_point`].
    hold: AtomicBool,
}

static JOB: Job = Job {
    fd: AtomicI32::new(-1),
    dst: AtomicPtr::new(core::ptr::null_mut()),
    len: AtomicUsize::new(0),
    done: AtomicUsize::new(0),
    state: AtomicU8::new(IDLE),
    budget: AtomicIsize::new(FRAME_BUDGET),
    hold: AtomicBool::new(false),
};

/// The streamer thread's turn at the background read: reads its next
/// chunk unless none is running, the frame holds them or the budget is
/// spent. Whether it read.
pub(crate) fn read_chunk() -> bool {
    if JOB.state.load(Ordering::Acquire) != RUNNING
        || JOB.hold.load(Ordering::Acquire)
        || JOB.budget.load(Ordering::Acquire) <= 0
    {
        return false;
    }
    let len = JOB.len.load(Ordering::Relaxed);
    let done = JOB.done.load(Ordering::Relaxed);
    let n = (len - done).min(CHUNK);
    let fd = sys::SceUid(JOB.fd.load(Ordering::Relaxed));
    // SAFETY: the job's block holds `len` bytes and stays allocated until
    // the game thread sees the job end; the chunk stays inside it.
    let got = unsafe {
        sys::sceIoRead(
            fd,
            JOB.dst.load(Ordering::Relaxed).add(done) as *mut core::ffi::c_void,
            n as u32,
        )
    };
    JOB.budget.fetch_sub(n as isize, Ordering::AcqRel);
    if got as usize != n {
        JOB.state.store(FAILED, Ordering::Release);
    } else if done + n == len {
        JOB.state.store(READ, Ordering::Release);
    } else {
        JOB.done.store(done + n, Ordering::Relaxed);
    }
    true
}

struct Loader {
    plan: Plan<'static, CacheLineBlocks, LoadError>,
    io: PspIo,
    stream: Stream,
}

/// Each streamed group's slab: room for its figatrees beyond the scene's
/// pinned ones (RE-573: one miss a frame fits a frame; a slab of 64 KiB
/// holds 20-30 average figatrees, the largest 13,872 B).
pub const GROUP_BYTES: usize = 64 * 1024;

/// The Memory Stick's sector: a read from a sector boundary is ~0.8 ms
/// faster than one from inside it (RE-573), so a streamed file is read
/// from the boundary before it, into its entry.
const SECTOR: i64 = 512;

/// `Stream::group_of` for a file that is not streamed.
const NO_GROUP: u8 = u8::MAX;

/// One group's slab and what it holds.
struct Group {
    id: u8,
    pool: StreamPool,
    slab: AlignedBuf,
}

/// Streamed files (module docs).
struct Stream {
    /// Per file-table index: its group, or [`NO_GROUP`].
    group_of: Vec<u8>,
    /// The current scene's groups: made at the first [`safe_point`] after
    /// [`enter`] when the scene streams eagerly, else on their first miss.
    groups: Vec<Group>,
    /// The groups [`stream_files`] named, and the most entries each slab
    /// can hold ([`ssb_rom::stream_pool::max_entries`]).
    bounds: Vec<(u8, usize)>,
    /// [`stream_files`]' `eager`, and whether the groups are still to make.
    eager: bool,
    make: bool,
    /// The last scene's, freed at the next [`safe_point`].
    leaving: Vec<Group>,
    /// Scratch for evictions.
    evicted: Vec<Entry>,
    /// Misses since the last [`safe_point`].
    frame_misses: u32,
    stats: StreamStats,
}

/// Streamed-file counts since boot ([`stream_stats`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StreamStats {
    /// Files read on a miss, and their bytes as read.
    pub misses: u32,
    pub bytes: u32,
    /// Most misses in one frame, the [`residency::epoch`] of the first such
    /// frame, and the scene it was in.
    pub max_frame: u32,
    pub max_epoch: u32,
    pub max_scene: &'static str,
    /// Entries evicted; misses no slab had room for (loaded as the
    /// scene's own file instead).
    pub evicted: u32,
    pub failed: u32,
    /// Slab bytes allocated now, and at most.
    pub slabs: u32,
    pub slabs_peak: u32,
    /// Microseconds spent reading misses, and the longest one.
    pub micros: u32,
    pub max_micros: u32,
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
    let blob = pack.blob_start() as i64;
    let plan = Plan::new(pack, CacheLineBlocks).ok_or(LoadError::OutOfMemory)?;
    plan.pack().build_indexes();
    // SAFETY: single-threaded; set before any lookup.
    unsafe {
        *LOADER.0.get() = Some(Loader {
            plan,
            io: PspIo {
                path,
                blob,
                fd: None,
                sync_fd: None,
            },
            stream: Stream {
                group_of: Vec::new(),
                groups: Vec::new(),
                bounds: Vec::new(),
                eager: false,
                make: false,
                leaving: Vec::new(),
                evicted: Vec::new(),
                frame_misses: 0,
                stats: StreamStats::default(),
            },
        });
        residency::install(resident.as_ptr(), Some(on_miss));
    }
    // The background reads' thread; without it they never start and the
    // scenes read their files as they start.
    let _ = crate::sample_stream::start_reader();
    // Opened now rather than in a scene's first frame (RE-476).
    if let Some(l) = loader() {
        open_fd(&mut l.io.fd, path);
        open_fd(&mut l.io.sync_fd, path);
    }
    Ok(())
}

/// The pack [`boot`] opened, for code that reads a scene's data without
/// being handed the pack (the scenes that had packs of their own before
/// RE-475).
pub fn pack() -> Option<&'static Pack<'static>> {
    loader().map(|l| l.plan.pack())
}

/// Whether [`boot`] ran.
pub fn active() -> bool {
    loader().is_some()
}

/// Holds `roots` and their extern closure for the rest of the run.
pub fn persist(roots: &[u32]) -> Result<(), LoadError> {
    let Some(l) = loader() else { return Ok(()) };
    l.plan.persist(&mut l.io, roots)
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
    let wait = l.plan.enter_begin(roots, within);
    let waited = crate::profile::start();
    let started = unsafe { sys::sceKernelGetSystemTimeLow() };
    if wait {
        l.plan.drain(&mut l.io);
    }
    let wait_micros = if wait {
        crate::profile::stop(crate::profile::Span::Io, waited);
        unsafe { sys::sceKernelGetSystemTimeLow() }.wrapping_sub(started)
    } else {
        0
    };
    let report = l.plan.enter_end(&mut l.io, scene, within, wait_micros);
    // The last scene's groups go at the safe point, as its files do; this
    // scene's are made there, after them.
    let s = &mut l.stream;
    s.leaving.append(&mut s.groups);
    s.make = s.eager;
    report
}

/// Marks the files `table` names (archive file, group) as the next
/// scene's streamed ones, replacing the last scene's: a scene that does
/// not hold one reads it on its first use into its group's slab (module
/// docs). A file named twice keeps its first group; groups are numbered
/// below 255. An empty `table` streams nothing and frees the map.
///
/// `eager`: the groups' slabs are made when the scene starts (at the first
/// [`safe_point`] after [`enter`]), not on their first miss.
pub fn stream_files(table: &[(u32, u8)], eager: bool) {
    let Some(l) = loader() else { return };
    let pack = l.plan.pack();
    let s = &mut l.stream;
    s.bounds.clear();
    s.eager = eager && !table.is_empty();
    if table.is_empty() {
        s.group_of = Vec::new();
        return;
    }
    s.group_of.clear();
    s.group_of.resize(pack.file_count() as usize, NO_GROUP);
    for &(file, group) in table {
        if let Some(i) = pack.file_index(file) {
            let g = &mut s.group_of[i as usize];
            if *g == NO_GROUP {
                *g = group.min(NO_GROUP - 1);
            }
        }
    }
    let mut lens = Vec::new();
    for (i, &group) in s.group_of.iter().enumerate() {
        if group == NO_GROUP || s.bounds.iter().any(|&(g, _)| g == group) {
            continue;
        }
        lens.clear();
        lens.extend(
            s.group_of[i..]
                .iter()
                .zip(i as u32..)
                .filter(|&(&g, _)| g == group)
                .filter_map(|(_, k)| pack.file_at(k).map(|f| f.len)),
        );
        let max = ssb_rom::stream_pool::max_entries(GROUP_BYTES as u32, &lens);
        s.bounds.push((group, max));
    }
}

/// [`StreamStats`] so far.
pub fn stream_stats() -> StreamStats {
    loader().map(|l| l.stream.stats).unwrap_or_default()
}

/// Starts reading `roots` and their extern closure in the background,
/// for the scene expected next: the current scene keeps running while
/// [`safe_point`] completes the reads. The next [`enter`] keeps the files
/// it wants and drops the rest. Files already held are not read again.
///
/// Reads stop being queued once fewer than
/// [`ssb_rom::loader_plan::PREFETCH_MARGIN`] bytes would be left free; the
/// scene's [`enter`] then reads the rest.
pub fn prefetch(roots: &[u32]) {
    let Some(l) = loader() else { return };
    l.plan.prefetch(&mut l.io, roots);
}

/// Whether background reads are queued or running.
pub fn busy() -> bool {
    loader().is_some_and(|l| l.plan.busy())
}

/// The current scene's load report.
pub fn report() -> Report {
    loader().map(|l| l.plan.report()).unwrap_or_default()
}

/// The archive files loaded on demand since the last [`enter`] (the
/// first [`ssb_rom::loader_plan::DEMAND_LOG`]).
pub fn demand_files() -> Vec<u16> {
    loader().map_or_else(Vec::new, |l| l.plan.demand_files().to_vec())
}

/// A load that failed outside [`enter`] (on demand), once.
pub fn take_failure() -> Option<LoadError> {
    loader().and_then(|l| l.plan.take_failure())
}

/// Drops the files the last [`enter`] left and frees their blocks. Call
/// after `Gpu::end_frame`, when the GE has read everything.
pub fn safe_point() {
    if let Some(l) = loader() {
        let t = crate::profile::start();
        // The next frame's background reads.
        JOB.hold.store(false, Ordering::Release);
        JOB.budget.store(FRAME_BUDGET, Ordering::Release);
        if JOB.state.load(Ordering::Acquire) == RUNNING {
            crate::sample_stream::wake();
        }
        l.plan.safe_point(&mut l.io);
        stream_safe_point(l);
        if !crate::gu::GE_LIST_OPEN.load(core::sync::atomic::Ordering::Relaxed) {
            if l.plan.retired() > 0 {
                crate::gu::forget_resident_texture();
            }
            l.plan.collect();
        }
        make_groups(&mut l.stream);
        crate::profile::stop(crate::profile::Span::Files, t);
    }
}

/// Bytes held in blocks, and how many blocks.
pub fn held() -> (usize, usize) {
    loader().map_or((0, 0), |l| l.plan.held())
}

fn on_miss(offset: u32, _len: u32) -> bool {
    let Some(l) = loader() else { return false };
    if let Some(loaded) = stream_miss(l, offset) {
        return loaded;
    }
    let t = crate::profile::start();
    let miss = l.plan.miss(&mut l.io, offset);
    if miss.loaded || miss.waited {
        crate::profile::stop(crate::profile::Span::Demand, t);
    }
    if miss.waited {
        crate::profile::stop(crate::profile::Span::Io, t);
    }
    miss.loaded
}

impl PspIo {
    /// Reads `len` bytes at pack offset `at` into `dst`, waiting, in
    /// [`CHUNK`]-byte reads.
    fn read_sync(&mut self, at: i64, dst: *mut u8, len: usize) -> Result<(), LoadError> {
        // The background reads wait for the safe point (module docs).
        JOB.hold.store(true, Ordering::Release);
        for attempt in 0..2 {
            if attempt == 1 {
                close_fd(&mut self.sync_fd);
            }
            let Some(fd) = open_fd(&mut self.sync_fd, self.path) else {
                return Err(LoadError::NotFound);
            };
            // SAFETY: `dst` holds `len` bytes; each read stays inside it.
            let ok = unsafe { sys::sceIoLseek(fd, at, sys::IoWhence::Set) == at }
                && (0..len).step_by(CHUNK).all(|done| {
                    let n = (len - done).min(CHUNK);
                    unsafe {
                        sys::sceIoRead(fd, dst.add(done) as *mut core::ffi::c_void, n as u32)
                            as usize
                            == n
                    }
                });
            if ok {
                return Ok(());
            }
        }
        Err(LoadError::ShortRead)
    }
}

impl Io<AlignedBuf> for PspIo {
    type Error = LoadError;
    const OUT_OF_MEMORY: LoadError = LoadError::OutOfMemory;

    fn read(&mut self, block: &AlignedBuf, start: u32, len: u32) -> Result<u32, LoadError> {
        let started = unsafe { sys::sceKernelGetSystemTimeLow() };
        self.read_sync(
            self.blob + i64::from(start),
            block.as_mut_ptr(),
            len as usize,
        )?;
        block.flush_cache();
        crate::profile::stop(crate::profile::Span::Io, started);
        Ok(unsafe { sys::sceKernelGetSystemTimeLow() }.wrapping_sub(started))
    }

    fn start(&mut self, block: &AlignedBuf, start: u32, len: u32) -> bool {
        let at = self.blob + i64::from(start);
        if !crate::sample_stream::running() || JOB.state.load(Ordering::Acquire) != IDLE {
            return false;
        }
        for attempt in 0..2 {
            if attempt == 1 {
                close_fd(&mut self.fd);
            }
            let Some(fd) = open_fd(&mut self.fd, self.path) else {
                return false;
            };
            // SAFETY: an open descriptor with no read in flight.
            if unsafe { sys::sceIoLseek(fd, at, sys::IoWhence::Set) } == at {
                JOB.fd.store(fd.0, Ordering::Relaxed);
                JOB.dst.store(block.as_mut_ptr(), Ordering::Relaxed);
                JOB.len.store(len as usize, Ordering::Relaxed);
                JOB.done.store(0, Ordering::Relaxed);
                JOB.state.store(RUNNING, Ordering::Release);
                crate::sample_stream::wake();
                return true;
            }
        }
        false
    }

    /// The background read's end: `None` while it runs; with `wait`, lets
    /// it read past the frame's hold and budget and waits for it.
    fn finish(&mut self, wait: bool) -> Option<bool> {
        let mut woke = false;
        loop {
            match JOB.state.load(Ordering::Acquire) {
                RUNNING if crate::sample_stream::running() => {
                    if !wait {
                        return None;
                    }
                    if !woke {
                        woke = true;
                        JOB.hold.store(false, Ordering::Release);
                        JOB.budget.store(isize::MAX, Ordering::Release);
                        crate::sample_stream::wake();
                    }
                    // SAFETY: a plain sleep; the streamer thread outranks
                    // this one.
                    unsafe { sys::sceKernelDelayThread(200) };
                }
                READ => {
                    JOB.state.store(IDLE, Ordering::Release);
                    return Some(true);
                }
                _ => {
                    // The descriptor is reopened for the next read.
                    JOB.state.store(IDLE, Ordering::Release);
                    close_fd(&mut self.fd);
                    return Some(false);
                }
            }
        }
    }

    fn flush(&mut self, block: &AlignedBuf) {
        block.flush_cache();
    }

    fn insert_run(&mut self, run: &[Span]) {
        // SAFETY: single-threaded; the plan keeps the block allocated while
        // any of its files is registered.
        unsafe { residency::insert_run(run) };
    }

    fn remove(&mut self, starts: &[u32]) {
        // SAFETY: single-threaded; nothing drawn after this resolves them.
        // `starts` ascends with the file index.
        unsafe { residency::remove_where(|s| starts.binary_search(&s).is_ok()) };
    }

    fn max_block(&self) -> usize {
        crate::memory::max_block()
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

/// The frame's end for the streamed files: the last scene's groups go
/// (no lookup of this frame is used after it), the frame's misses are
/// counted and the registry's clock moves on.
fn stream_safe_point(l: &mut Loader) {
    let s = &mut l.stream;
    for g in s.leaving.drain(..) {
        s.evicted.clear();
        let mut g = g;
        g.pool.drain_into(&mut s.evicted);
        unregister(&l.plan, &s.evicted);
        s.stats.slabs -= GROUP_BYTES as u32;
        drop(g.slab);
    }
    if s.frame_misses > s.stats.max_frame {
        s.stats.max_frame = s.frame_misses;
        s.stats.max_epoch = residency::epoch();
        s.stats.max_scene = l.plan.report().scene;
    }
    s.frame_misses = 0;
    residency::advance();
}

/// Unregisters the spans of `entries` (file-table indices) that are still
/// the stream's: a file the plan has since loaded has its own span.
fn unregister(plan: &Plan<'static, CacheLineBlocks, LoadError>, entries: &[Entry]) {
    for e in entries {
        let i = e.key as usize;
        if plan.files()[i].block != ssb_engine::memory::NO_BLOCK {
            continue;
        }
        if let Some(f) = plan.pack().file_at(e.key) {
            // SAFETY: single-threaded; the entry's bytes are not read after
            // this frame's lookups (they were not looked up this frame, or
            // this is the safe point).
            unsafe { residency::remove(f.blob_start) };
        }
    }
}

/// An eager scene's groups, at its first safe point: after the last
/// scene's groups and files are freed, so that the two scenes' never
/// coexist.
fn make_groups(s: &mut Stream) {
    if core::mem::take(&mut s.make) {
        for k in 0..s.bounds.len() {
            let id = s.bounds[k].0;
            if !s.groups.iter().any(|g| g.id == id) && make_group(s, id).is_none() {
                break;
            }
        }
    }
}

/// Makes group `id`'s slab, with room reserved in the stream's lists and
/// the registry for every entry it can hold (`bounds`). Its index in
/// `groups`; `None` when the slab does not fit.
fn make_group(s: &mut Stream, id: u8) -> Option<usize> {
    let slab = AlignedBuf::cache_lines(GROUP_BYTES)?;
    let bound = |id: u8| {
        s.bounds
            .iter()
            .find(|&&(g, _)| g == id)
            .map_or(0, |&(_, max)| max)
    };
    let max = bound(id);
    // Every group's entries, those of the groups made before included (the
    // registry may already hold some of them).
    let spans = max + s.groups.iter().map(|g| bound(g.id)).sum::<usize>();
    s.stats.slabs += GROUP_BYTES as u32;
    s.stats.slabs_peak = s.stats.slabs_peak.max(s.stats.slabs);
    s.groups
        .reserve_exact(s.bounds.len().max(s.groups.len() + 1) - s.groups.len());
    s.evicted.reserve(max);
    // SAFETY: single-threaded (module docs).
    unsafe { residency::reserve(spans) };
    s.groups.push(Group {
        id,
        pool: StreamPool::with_entries(GROUP_BYTES as u32, max),
        slab,
    });
    Some(s.groups.len() - 1)
}

/// A miss on a streamed file: reads it into its group's slab, evicting the
/// least recently used entries not read this frame. `None` when `offset`
/// is not in a streamed file or no slab has room: the plan loads it.
fn stream_miss(l: &mut Loader, offset: u32) -> Option<bool> {
    let pack = l.plan.pack();
    let i = pack.file_at_offset(offset)?;
    let group = *l.stream.group_of.get(i as usize)?;
    let f = l.plan.files()[i as usize];
    if group == NO_GROUP || f.block != ssb_engine::memory::NO_BLOCK || f.queued {
        return None;
    }
    let file = pack.file_at(i)?;
    let s = &mut l.stream;
    let now = residency::epoch();
    let k = match s.groups.iter().position(|g| g.id == group) {
        Some(k) => k,
        None => make_group(s, group)?,
    };
    let at = l.io.blob + i64::from(file.blob_start);
    let pad = (at % SECTOR) as u32;
    let g = &mut s.groups[k];
    // Still in the slab: a span the plan replaced and then dropped.
    let entry = match g.pool.get(i) {
        Some(e) => e,
        None => {
            s.evicted.clear();
            let placed = g.pool.place(
                i,
                pad + file.len,
                now,
                |e| {
                    pack.file_at(e.key)
                        .and_then(|f| residency::last_used(f.blob_start))
                        .unwrap_or(now.wrapping_sub(1))
                },
                &mut s.evicted,
            );
            unregister(&l.plan, &s.evicted);
            s.stats.evicted += s.evicted.len() as u32;
            let Some(e) = placed else {
                s.stats.failed += 1;
                return None;
            };
            let t = crate::profile::start();
            let started = unsafe { sys::sceKernelGetSystemTimeLow() };
            // SAFETY: the entry holds `pad + len` bytes of the slab.
            let dst = unsafe { g.slab.as_mut_ptr().add(e.at as usize) };
            if l.io
                .read_sync(at - i64::from(pad), dst, e.len as usize)
                .is_err()
            {
                g.pool.remove(i);
                return Some(false);
            }
            let us = unsafe { sys::sceKernelGetSystemTimeLow() }.wrapping_sub(started);
            crate::profile::stop(crate::profile::Span::Demand, t);
            crate::profile::stop(crate::profile::Span::Io, t);
            s.stats.misses += 1;
            s.stats.bytes += e.len;
            s.stats.micros = s.stats.micros.wrapping_add(us);
            s.stats.max_micros = s.stats.max_micros.max(us);
            s.frame_misses += 1;
            e
        }
    };
    // SAFETY: single-threaded; the slab lives until the group leaves, which
    // unregisters its spans first.
    unsafe {
        residency::insert(Span {
            start: file.blob_start,
            len: file.len,
            ptr: g.slab.as_ptr().add((entry.at + pad) as usize),
        })
    };
    Some(true)
}
