//! The scene loader's state machine (RE-475, D-046, RE-476, RE-571): which
//! archive files a scene holds, the read runs that fill them, the blocks
//! that hold them and when a block is freed. Portable: the platform
//! supplies the reads ([`Io`]) and the memory
//! ([`ssb_engine::memory::BlockAlloc`]); `psp-runtime`'s `scene_files` is
//! the PSP shell around a [`Plan`], and host tests drive one with fakes.
//!
//! * The files a scene holds: [`crate::reloc_link::closure`] of its roots
//!   over the pack's extern IDs.
//! * Read runs: [`crate::reloc_link::place_runs`] (`Order::Ascending`):
//!   files adjacent in the blob, at most [`MAX_RUN`] bytes
//!   ([`MAX_BACKGROUND_RUN`] for a background read), one read and one
//!   [`AssetArena`] block each.
//! * A block lives while any of its files is held: a file one scene keeps
//!   into the next pins its old block. A block whose last file drops is
//!   retired, and freed at the next GE safe point.

use alloc::collections::VecDeque;
use alloc::vec::Vec;

use ssb_engine::memory::{AssetArena, BlockAlloc, BlockId, NO_BLOCK};

use crate::pack::Pack;
use crate::reloc_link::{self, Placement};
use crate::residency::Span;

/// The most bytes one read fills: larger runs split.
pub const MAX_RUN: u32 = 4 << 20;
/// The most bytes one background read fills: a scene that wants none of
/// a prefetch waits for at most one of these (RE-476).
pub const MAX_BACKGROUND_RUN: u32 = 1 << 20;
/// Free memory a prefetch leaves: the scene's own allocations (a battle's
/// fighters and effect players) come out of the same heap, and `psp`'s
/// allocation-error handler never returns (RE-475).
pub const PREFETCH_MARGIN: usize = 6 << 20;
/// Demand-loaded files remembered for [`Plan::demand_files`].
pub const DEMAND_LOG: usize = 16;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Keep {
    No,
    Scene,
    /// The last scene's, until [`Plan::safe_point`].
    Leaving,
    Always,
    /// Read ahead for the next scene ([`Plan::prefetch`]), until its enter.
    Next,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FileState {
    pub block: BlockId,
    pub keep: Keep,
    /// A background read in the queue fills it.
    pub queued: bool,
}

const EMPTY: FileState = FileState {
    block: NO_BLOCK,
    keep: Keep::No,
    queued: false,
};

/// One background read: files `from..=last`, blob `start..start + len`,
/// into `block` (pending until the read completes).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Job {
    pub from: usize,
    pub last: usize,
    pub start: u32,
    pub len: u32,
    pub block: BlockId,
}

/// The last enter's load, and the demand loads since.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
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
    /// Files and bytes a prefetch had read before this scene started.
    pub prefetched_files: u32,
    pub prefetched_bytes: u32,
    /// Microseconds enter waited for background reads still running.
    pub wait_micros: u32,
}

/// The platform's reads, registry and heap, for a [`Plan`] whose blocks
/// are `B`.
pub trait Io<B> {
    type Error: Copy;
    /// The error for a block the heap cannot give.
    const OUT_OF_MEMORY: Self::Error;
    /// Reads blob bytes `start..start + len` into `block`, waiting, and
    /// writes them back from the data cache. Returns the microseconds the
    /// read took.
    fn read(&mut self, block: &B, start: u32, len: u32) -> Result<u32, Self::Error>;
    /// Starts reading blob bytes `start..start + len` into `block` in the
    /// background; `false` when it could not start.
    fn start(&mut self, block: &B, start: u32, len: u32) -> bool;
    /// The background read's end: `None` while it runs (only when `wait`
    /// is `false`), else whether it read every byte.
    fn finish(&mut self, wait: bool) -> Option<bool>;
    /// Writes a block a background read filled back from the data cache.
    fn flush(&mut self, block: &B);
    /// Registers one read run's files (adjacent, ascending).
    fn insert_run(&mut self, run: &[Span]);
    /// Unregisters the files whose blob starts are `starts` (ascending).
    fn remove(&mut self, starts: &[u32]);
    /// The largest block the heap can still give.
    fn max_block(&self) -> usize;
}

/// What a residency miss did ([`Plan::miss`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Miss {
    /// The file is now loaded.
    pub loaded: bool,
    /// It waited for the background read in flight.
    pub waited: bool,
}

/// The loader's state over one pack. See the module docs.
pub struct Plan<'p, A: BlockAlloc, E> {
    pack: Pack<'p>,
    files: Vec<FileState>,
    arena: AssetArena<A>,
    /// Reusable scratch: the closure (`want`) and its read runs.
    place: Placement,
    ids: Vec<u32>,
    spans: Vec<Span>,
    starts: Vec<u32>,
    report: Report,
    demand: [u16; DEMAND_LOG],
    demand_len: usize,
    failed: Option<E>,
    /// Background reads, the first in flight once started.
    jobs: VecDeque<Job>,
    in_flight: bool,
    /// Some file may be [`Keep::Leaving`]: set by
    /// [`enter_end`](Self::enter_end), the only place that marks one,
    /// cleared by the [`safe_point`](Self::safe_point) that drops them.
    leaving: bool,
}

impl<'p, A: BlockAlloc, E: Copy> Plan<'p, A, E> {
    /// A plan over `pack` with no file loaded, or `None` when the file
    /// table does not fit in memory.
    pub fn new(pack: Pack<'p>, alloc: A) -> Option<Self> {
        let count = pack.file_count() as usize;
        let mut files = Vec::new();
        files.try_reserve_exact(count).ok()?;
        files.resize(count, EMPTY);
        Some(Self {
            pack,
            files,
            arena: AssetArena::new(alloc),
            place: Placement::default(),
            ids: Vec::new(),
            spans: Vec::new(),
            starts: Vec::new(),
            report: Report::default(),
            demand: [0; DEMAND_LOG],
            demand_len: 0,
            failed: None,
            jobs: VecDeque::new(),
            in_flight: false,
            leaving: false,
        })
    }

    pub fn pack(&self) -> &Pack<'p> {
        &self.pack
    }

    pub fn arena(&self) -> &AssetArena<A> {
        &self.arena
    }

    pub fn files(&self) -> &[FileState] {
        &self.files
    }

    pub fn jobs(&self) -> &VecDeque<Job> {
        &self.jobs
    }

    pub fn in_flight(&self) -> bool {
        self.in_flight
    }

    pub fn report(&self) -> Report {
        self.report
    }

    /// The archive files loaded on demand since the last enter.
    pub fn demand_files(&self) -> &[u16] {
        &self.demand[..self.demand_len]
    }

    /// A load that failed outside an enter (on demand), once.
    pub fn take_failure(&mut self) -> Option<E> {
        self.failed.take()
    }

    /// Whether background reads are queued or running.
    pub fn busy(&self) -> bool {
        !self.jobs.is_empty()
    }

    /// Bytes held in blocks with registered files, and how many blocks.
    pub fn held(&self) -> (usize, usize) {
        self.arena.committed()
    }

    /// `roots` and their extern closure into `place.want`.
    fn closure(&mut self, roots: &[u32]) {
        // The pack treats an ID it lacks as an empty file: no error.
        let _ = reloc_link::closure(&self.pack, roots, &mut self.place.want, &mut self.ids);
    }

    /// `place.want` = file `i` alone, or the files `rest` marks.
    fn want_only(&mut self, files: impl Iterator<Item = usize>) {
        self.place.want.clear();
        self.place.want.resize(self.files.len(), false);
        for i in files {
            self.place.want[i] = true;
        }
    }

    /// Holds `roots` and their extern closure for the rest of the run.
    pub fn persist<I: Io<A::Block, Error = E>>(
        &mut self,
        io: &mut I,
        roots: &[u32],
    ) -> Result<(), E> {
        self.closure(roots);
        for i in 0..self.files.len() {
            if self.place.want[i] {
                self.files[i].keep = Keep::Always;
            }
        }
        self.load_missing(io)
    }

    /// An enter's first step: takes `roots`' closure and, for a scene that
    /// is not `within` the running one, drops the queued reads it does not
    /// want. Returns whether background reads must be waited for
    /// ([`drain`](Self::drain)) before [`enter_end`](Self::enter_end).
    pub fn enter_begin(&mut self, roots: &[u32], within: bool) -> bool {
        self.closure(roots);
        // A prefetch for another scene: its reads not yet started are
        // dropped; the one in flight, and the ones this scene wants, are
        // waited for. Within a scene, only reads of files it wants are.
        if within {
            let want = &self.place.want;
            self.jobs.iter().any(|j| (j.from..=j.last).any(|i| want[i]))
        } else {
            self.cancel_unwanted();
            !self.jobs.is_empty()
        }
    }

    /// An enter's last step, after [`enter_begin`](Self::enter_begin) (and
    /// [`drain`](Self::drain) when it asked): the last scene's other files
    /// leave at the next [`safe_point`](Self::safe_point), the missing ones
    /// are read.
    pub fn enter_end<I: Io<A::Block, Error = E>>(
        &mut self,
        io: &mut I,
        scene: &'static str,
        within: bool,
        wait_micros: u32,
    ) -> Result<Report, E> {
        let (mut prefetched_files, mut prefetched_bytes) = (0, 0);
        // The last scene's files stay readable until the frame ends: the
        // frame that starts a scene may still draw the one it leaves.
        for i in 0..self.files.len() {
            let w = self.place.want[i];
            let held = self.files[i].block != NO_BLOCK;
            let keep = &mut self.files[i].keep;
            if w && *keep == Keep::Next && held {
                prefetched_files += 1;
                prefetched_bytes += self.pack.file_at(i as u32).map_or(0, |d| d.len);
            }
            if w && *keep != Keep::Always {
                *keep = Keep::Scene;
            } else if !w && (*keep == Keep::Scene || (*keep == Keep::Next && !within)) {
                *keep = Keep::Leaving;
                self.leaving = true;
            }
        }
        self.report = Report {
            scene,
            prefetched_files,
            prefetched_bytes,
            wait_micros,
            ..Report::default()
        };
        self.demand_len = 0;
        self.arena.begin_scene();
        self.load_missing(io)?;
        for (i, f) in self.files.iter().enumerate() {
            if matches!(f.keep, Keep::Scene | Keep::Always) {
                self.report.files += 1;
                self.report.bytes += self.pack.file_at(i as u32).map_or(0, |d| d.len);
            }
        }
        Ok(self.report)
    }

    /// Starts reading `roots` and their extern closure in the background,
    /// for the scene expected next. Files held or queued are not read
    /// again; reads stop being queued once fewer than [`PREFETCH_MARGIN`]
    /// bytes would be left free.
    pub fn prefetch<I: Io<A::Block, Error = E>>(&mut self, io: &mut I, roots: &[u32]) {
        self.closure(roots);
        for i in 0..self.files.len() {
            let keep = &mut self.files[i].keep;
            if self.place.want[i] && matches!(*keep, Keep::No | Keep::Leaving) {
                *keep = Keep::Next;
            }
        }
        self.queue_missing(io);
        self.pump(io);
    }

    /// Completes background reads that have finished, starts the next, and
    /// drops the files the last enter left (their blocks retire). The
    /// caller frees retired blocks ([`collect`](Self::collect)) once the GE
    /// has read everything.
    pub fn safe_point<I: Io<A::Block, Error = E>>(&mut self, io: &mut I) {
        self.pump(io);
        // Most frames leave nothing: skip the scan of every file.
        if !self.leaving {
            return;
        }
        self.leaving = false;
        // The registry drops them all in one pass (RE-476).
        self.starts.clear();
        for i in 0..self.files.len() {
            if self.files[i].keep == Keep::Leaving {
                if let Some(start) = self.drop_file(i) {
                    self.starts.push(start);
                }
            }
        }
        if !self.starts.is_empty() {
            io.remove(&self.starts);
        }
    }

    /// Blocks retired and not yet freed.
    pub fn retired(&self) -> usize {
        self.arena.retired()
    }

    /// Frees the retired blocks: no display list may still read them.
    pub fn collect(&mut self) -> usize {
        self.arena.collect()
    }

    /// A residency miss at blob `offset`: loads the file holding it now.
    pub fn miss<I: Io<A::Block, Error = E>>(&mut self, io: &mut I, offset: u32) -> Miss {
        let Some(i) = self.pack.file_at_offset(offset) else {
            return Miss::default();
        };
        let i = i as usize;
        if self.files[i].queued {
            let Some(k) = self
                .jobs
                .iter()
                .position(|j| (j.from..=j.last).contains(&i))
            else {
                return Miss::default();
            };
            if k == 0 && self.in_flight {
                // The read in flight holds it: wait for that read rather
                // than read the file twice.
                while self.files[i].queued && self.complete(io, true) {}
                return Miss {
                    loaded: self.files[i].block != NO_BLOCK,
                    waited: true,
                };
            }
            // A read not yet started: the file is read now, on its own,
            // and the rest of that read queued again behind the others.
            if let Some(job) = self.jobs.remove(k) {
                for f in job.from..=job.last {
                    self.files[f].queued = false;
                }
                self.arena.free(job.block);
                let loaded = self.demand_load(io, i);
                self.want_only((job.from..=job.last).filter(|&f| f != i));
                self.queue_missing(io);
                return Miss {
                    loaded,
                    waited: false,
                };
            }
            return Miss::default();
        }
        if self.files[i].block != NO_BLOCK {
            return Miss::default();
        }
        Miss {
            loaded: self.demand_load(io, i),
            waited: false,
        }
    }

    /// Reads file `i`, which a scene needs now.
    fn demand_load<I: Io<A::Block, Error = E>>(&mut self, io: &mut I, i: usize) -> bool {
        self.want_only(core::iter::once(i));
        if self.files[i].keep == Keep::No {
            self.files[i].keep = Keep::Scene;
        }
        let before = (self.report.loaded_files, self.report.loaded_bytes);
        match self.load_missing(io) {
            Ok(()) => {
                self.report.demand_files += self.report.loaded_files - before.0;
                self.report.demand_bytes += self.report.loaded_bytes - before.1;
                if self.demand_len < DEMAND_LOG {
                    self.demand[self.demand_len] = self
                        .pack
                        .file_at(i as u32)
                        .map_or(u16::MAX, |d| d.file as u16);
                    self.demand_len += 1;
                }
                true
            }
            Err(e) => {
                self.files[i].keep = Keep::No;
                self.failed = Some(e);
                false
            }
        }
    }

    /// Forgets file `i` and releases its share of its block. Returns its
    /// blob start, whose span the caller unregisters.
    fn drop_file(&mut self, i: usize) -> Option<u32> {
        let f = self.files[i];
        self.files[i] = EMPTY;
        if f.block == NO_BLOCK {
            return None;
        }
        let start = self.pack.file_at(i as u32).map(|d| d.blob_start);
        self.arena.release(f.block);
        start
    }

    /// Reads every file in `place.want` not yet loaded, one read per run.
    fn load_missing<I: Io<A::Block, Error = E>>(&mut self, io: &mut I) -> Result<(), E> {
        let files = &self.files;
        let _ = reloc_link::place_runs(
            &self.pack,
            |i| files[i as usize].block != NO_BLOCK,
            MAX_RUN,
            &mut self.place,
        );
        for k in 0..self.place.runs.len() {
            let r = self.place.runs[k];
            let block = self.arena.alloc(r.len as usize).ok_or(I::OUT_OF_MEMORY)?;
            let Some(buf) = self.arena.block(block) else {
                return Err(I::OUT_OF_MEMORY);
            };
            match io.read(buf, r.start, r.len) {
                Ok(micros) => {
                    self.report.micros += micros;
                    self.report.reads += 1;
                    self.register(io, block, r.start, r.first as usize, r.last as usize, true);
                }
                Err(e) => {
                    self.arena.free(block);
                    return Err(e);
                }
            }
        }
        Ok(())
    }

    /// Registers files `from..=last`, read into `block` from blob offset
    /// `start`. `count` adds them to the scene's report.
    fn register<I: Io<A::Block, Error = E>>(
        &mut self,
        io: &mut I,
        block: BlockId,
        start: u32,
        from: usize,
        last: usize,
        count: bool,
    ) {
        let base = self.arena.ptr(block).unwrap_or(core::ptr::null_mut());
        let mut live = 0;
        self.spans.clear();
        for k in from..=last {
            let Some(d) = self.pack.file_at(k as u32) else {
                continue;
            };
            if d.len == 0 {
                continue;
            }
            self.spans.push(Span {
                start: d.blob_start,
                len: d.len,
                ptr: base.wrapping_add((d.blob_start - start) as usize),
            });
            self.files[k].block = block;
            self.files[k].queued = false;
            live += 1;
            if count {
                self.report.loaded_files += 1;
                self.report.loaded_bytes += d.len;
            }
        }
        // The block stays allocated while any of its files is registered
        // (`drop_file`); one read's files are adjacent.
        io.insert_run(&self.spans);
        self.arena.commit(block, live);
    }

    /// Queues background reads of every file in `place.want` neither held
    /// nor queued, one per run, while memory allows.
    fn queue_missing<I: Io<A::Block, Error = E>>(&mut self, io: &mut I) {
        let files = &self.files;
        let _ = reloc_link::place_runs(
            &self.pack,
            |i| files[i as usize].block != NO_BLOCK || files[i as usize].queued,
            MAX_BACKGROUND_RUN,
            &mut self.place,
        );
        for k in 0..self.place.runs.len() {
            let r = self.place.runs[k];
            if io.max_block() < r.len as usize + PREFETCH_MARGIN {
                return;
            }
            let Some(block) = self.arena.alloc(r.len as usize) else {
                return;
            };
            for f in r.first..=r.last {
                self.files[f as usize].queued = self.pack.file_at(f).is_some_and(|d| d.len > 0);
            }
            self.jobs.push_back(Job {
                from: r.first as usize,
                last: r.last as usize,
                start: r.start,
                len: r.len,
                block,
            });
        }
    }

    /// Drops the queued reads not yet started that hold no file
    /// `place.want` names.
    fn cancel_unwanted(&mut self) {
        let mut k = usize::from(self.in_flight);
        while k < self.jobs.len() {
            let j = self.jobs[k];
            if (j.from..=j.last).any(|i| self.place.want[i]) {
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
                self.arena.free(job.block);
            }
        }
    }

    /// Completes the read in flight if it has finished, and starts the
    /// next; never waits.
    pub fn pump<I: Io<A::Block, Error = E>>(&mut self, io: &mut I) {
        loop {
            if !self.in_flight && !self.start_next(io) {
                return;
            }
            if !self.complete(io, false) {
                return;
            }
        }
    }

    /// Waits for every queued read.
    pub fn drain<I: Io<A::Block, Error = E>>(&mut self, io: &mut I) {
        while self.in_flight || self.start_next(io) {
            self.complete(io, true);
        }
    }

    /// Starts the first queued read; `false` when none is queued or it
    /// could not start (its files then load when a scene wants them).
    fn start_next<I: Io<A::Block, Error = E>>(&mut self, io: &mut I) -> bool {
        while let Some(&job) = self.jobs.front() {
            if let Some(buf) = self.arena.block(job.block) {
                if io.start(buf, job.start, job.len) {
                    self.in_flight = true;
                    return true;
                }
            }
            self.abandon_front();
        }
        false
    }

    /// Finishes the read in flight: registers its files, or gives them up
    /// if the read failed. `wait` blocks until it ends; otherwise returns
    /// `false` while it runs.
    fn complete<I: Io<A::Block, Error = E>>(&mut self, io: &mut I, wait: bool) -> bool {
        if !self.in_flight {
            return false;
        }
        let Some(ok) = io.finish(wait) else {
            return false;
        };
        self.in_flight = false;
        let Some(job) = self.jobs.pop_front() else {
            return true;
        };
        if !ok {
            // Its files load when a scene wants them.
            self.release_job(&job);
        } else {
            if let Some(buf) = self.arena.block(job.block) {
                io.flush(buf);
            }
            self.register(io, job.block, job.start, job.from, job.last, false);
        }
        true
    }

    fn release_job(&mut self, job: &Job) {
        for i in job.from..=job.last {
            self.files[i].queued = false;
            if self.files[i].keep == Keep::Next {
                self.files[i].keep = Keep::No;
            }
        }
        self.arena.free(job.block);
    }

    fn abandon_front(&mut self) {
        if let Some(job) = self.jobs.pop_front() {
            self.release_job(&job);
        }
    }
}

#[cfg(test)]
#[path = "loader_plan_tests.rs"]
mod tests;
