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
//! Adjacent files are read with one `sceIoRead` into one block. A block
//! is freed when none of its files is held, once no display list can read
//! it.

use alloc::vec::Vec;

use psp::sys;
use ssb_rom::pack::{FileDesc, Pack};
use ssb_rom::residency::{self, Span};

use crate::assets::{AlignedBuf, LoadError};

/// No block.
const NO_BLOCK: u16 = u16::MAX;
/// The most bytes one read fills: larger runs split.
const MAX_RUN: u32 = 4 << 20;
/// Demand-loaded files remembered for [`demand_files`].
const DEMAND_LOG: usize = 16;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Keep {
    No,
    Scene,
    /// The last scene's, until [`safe_point`].
    Leaving,
    Always,
}

#[derive(Clone, Copy)]
struct FileState {
    block: u16,
    keep: Keep,
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
        });
        residency::install(resident.as_ptr(), Some(on_miss));
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
    let Some(l) = loader() else {
        return Ok(Report::default());
    };
    let want = l.closure(roots);
    // The last scene's files stay readable until the frame ends: the
    // frame that starts a scene may still draw the one it leaves.
    for (i, w) in want.iter().enumerate() {
        let keep = &mut l.files[i].keep;
        if *w && *keep != Keep::Always {
            *keep = Keep::Scene;
        } else if !*w && *keep == Keep::Scene {
            *keep = Keep::Leaving;
        }
    }
    l.report = Report {
        scene,
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
        for i in 0..l.files.len() {
            if l.files[i].keep == Keep::Leaving {
                l.drop_file(i);
            }
        }
        l.collect_retired();
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
    if l.files[i].block != NO_BLOCK {
        return false;
    }
    let mut want = alloc::vec![false; l.files.len()];
    want[i] = true;
    if l.files[i].keep == Keep::No {
        l.files[i].keep = Keep::Scene;
    }
    let before = (l.report.loaded_files, l.report.loaded_bytes);
    match l.load_missing(&want) {
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

    fn drop_file(&mut self, i: usize) {
        let f = self.files[i];
        self.files[i] = FileState {
            block: NO_BLOCK,
            keep: Keep::No,
        };
        if f.block == NO_BLOCK {
            return;
        }
        if let Some(d) = self.pack.file_at(i as u32) {
            // SAFETY: single-threaded; nothing drawn after this resolves it.
            unsafe { residency::remove(d.blob_start) };
        }
        let b = &mut self.blocks[f.block as usize];
        if let Some(block) = b {
            block.live -= 1;
            if block.live == 0 {
                if let Some(block) = b.take() {
                    self.retired.push(block.buf);
                }
            }
        }
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
        let buf = AlignedBuf::new(len).ok_or(LoadError::OutOfMemory)?;
        let started = unsafe { sys::sceKernelGetSystemTimeLow() };
        let at = self.pack.blob_start() as i64 + i64::from(first.blob_start);
        crate::assets::read_at(self.path, at, buf.as_mut_ptr(), len)?;
        buf.flush_cache();
        self.report.micros += unsafe { sys::sceKernelGetSystemTimeLow() }.wrapping_sub(started);
        self.report.reads += 1;
        let slot = match self.blocks.iter().position(Option::is_none) {
            Some(s) => s,
            None => {
                self.blocks.push(None);
                self.blocks.len() - 1
            }
        };
        let mut live = 0;
        for k in from..=last {
            let Some(d) = self.pack.file_at(k as u32) else {
                continue;
            };
            if d.len == 0 {
                continue;
            }
            // SAFETY: the block stays allocated while any of its files is
            // registered (`drop_file`).
            unsafe {
                residency::insert(Span {
                    start: d.blob_start,
                    len: d.len,
                    ptr: buf.as_ptr().add((d.blob_start - first.blob_start) as usize),
                });
            }
            self.files[k].block = slot as u16;
            live += 1;
            self.report.loaded_files += 1;
            self.report.loaded_bytes += d.len;
        }
        self.blocks[slot] = Some(Block { buf, live });
        Ok(())
    }
}
