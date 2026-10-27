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
//! The background reads complete on the game thread: [`safe_point`] polls
//! the read in flight, registers its files and starts the next, so the
//! registry and the allocator stay single-threaded. A block a read fills is
//! on whole cache lines and written back before its files are registered,
//! so the GE never reads a line the CPU holds.

use alloc::vec::Vec;

use psp::sys;
use ssb_rom::loader_plan::{Io, Plan};
use ssb_rom::pack::Pack;
use ssb_rom::residency::{self, Span};

use crate::assets::{AlignedBuf, CacheLineBlocks, LoadError};

pub use ssb_rom::loader_plan::Report;

/// The pack file's reads, the residency registry and the heap, for the
/// [`Plan`].
struct PspIo {
    /// NUL-terminated.
    path: &'static str,
    /// The pack's blob offset in the file.
    blob: i64,
    /// The pack opened for background reads, and for the reads that wait,
    /// kept open: opening it costs about 10 ms under PPSSPP (RE-476). A
    /// failed read reopens it once (a descriptor does not survive standby).
    fd: Option<sys::SceUid>,
    sync_fd: Option<sys::SceUid>,
    /// The background read in flight's length.
    pending: u32,
}

struct Loader {
    plan: Plan<'static, CacheLineBlocks, LoadError>,
    io: PspIo,
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
                pending: 0,
            },
        });
        residency::install(resident.as_ptr(), Some(on_miss));
    }
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
    l.plan.enter_end(&mut l.io, scene, within, wait_micros)
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
        l.plan.safe_point(&mut l.io);
        if !crate::gu::GE_LIST_OPEN.load(core::sync::atomic::Ordering::Relaxed) {
            if l.plan.retired() > 0 {
                crate::gu::forget_resident_texture();
            }
            l.plan.collect();
        }
        crate::profile::stop(crate::profile::Span::Files, t);
    }
}

/// Bytes held in blocks, and how many blocks.
pub fn held() -> (usize, usize) {
    loader().map_or((0, 0), |l| l.plan.held())
}

fn on_miss(offset: u32, _len: u32) -> bool {
    let Some(l) = loader() else { return false };
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
        for attempt in 0..2 {
            if attempt == 1 {
                close_fd(&mut self.fd);
            }
            let Some(fd) = open_fd(&mut self.fd, self.path) else {
                return false;
            };
            // SAFETY: the block holds `len` bytes and stays allocated, in
            // the arena, until the read completes.
            let ok = unsafe {
                sys::sceIoLseek(fd, at, sys::IoWhence::Set) == at
                    && sys::sceIoReadAsync(fd, block.as_mut_ptr() as *mut core::ffi::c_void, len)
                        >= 0
            };
            if ok {
                self.pending = len;
                return true;
            }
        }
        false
    }

    fn finish(&mut self, wait: bool) -> Option<bool> {
        let Some(fd) = self.fd else {
            return Some(false);
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
            return None;
        }
        let ok = status >= 0 && res == i64::from(self.pending);
        if !ok {
            // The descriptor is reopened for the next read.
            close_fd(&mut self.fd);
        }
        Some(ok)
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
