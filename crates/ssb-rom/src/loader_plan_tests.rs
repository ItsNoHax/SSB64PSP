//! [`Plan`] against the loader it replaced: `psp-runtime`'s `scene_files`
//! before batch 65 (its own closure walk and run-grouping loops, one
//! `AlignedBuf` per run), ported here line for line over the same fake
//! reads and heap. Both are fed the same scene, battle, prefetch, frame
//! and miss sequences (`scene_load`'s, from the L2 simulator) and must
//! agree after every step: held files and their keep states, queued reads,
//! registered spans, how files share blocks and the block sizes, reads,
//! reports, and blocks freed.

use std::cell::Cell;
use std::collections::{BTreeMap, VecDeque};
use std::rc::Rc;

use super::*;
use crate::pack::FileDesc;
use crate::reloc_link::sim_tests::{Event, Roots};

// ---- fakes -----------------------------------------------------------------

/// A heap block on whole 64-byte lines that gives its bytes back to the
/// fake heap when dropped.
struct Buf {
    bytes: Vec<u64>,
    len: usize,
    used: Rc<Cell<usize>>,
}

impl Drop for Buf {
    fn drop(&mut self) {
        self.used.set(self.used.get() - self.bytes.len() * 8);
    }
}

impl Buf {
    fn new(len: usize, heap: &Heap) -> Option<Buf> {
        let size = len.max(1).next_multiple_of(64);
        if heap.used.get() + size > heap.budget {
            return None;
        }
        heap.used.set(heap.used.get() + size);
        Some(Buf {
            bytes: vec![0; size / 8],
            len,
            used: heap.used.clone(),
        })
    }
    fn ptr(&self) -> *mut u8 {
        self.bytes.as_ptr() as *mut u8
    }
}

#[derive(Clone)]
struct Heap {
    budget: usize,
    used: Rc<Cell<usize>>,
}

impl BlockAlloc for Heap {
    type Block = Buf;
    fn alloc(&mut self, len: usize) -> Option<Buf> {
        Buf::new(len, self)
    }
    fn ptr(block: &Buf) -> *mut u8 {
        block.ptr()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct IoErr;

/// How the fake reads behave.
#[derive(Clone, Copy, Debug)]
struct Config {
    budget: usize,
    /// Non-waiting polls before a background read completes.
    delay: u32,
    /// Every `fail_bg`-th background read fails (0: none).
    fail_bg: u32,
    /// Every `fail_start`-th background read cannot start (0: none).
    fail_start: u32,
}

/// Reads, the registry and the heap, recorded.
struct FakeIo {
    cfg: Config,
    heap: Heap,
    /// Registered spans: blob start → (len, address).
    spans: BTreeMap<u32, (u32, usize)>,
    /// Every read: (background, start, len).
    reads: Vec<(bool, u32, u32)>,
    bg_started: u32,
    in_flight: Option<(u32, bool)>,
    flushed: u32,
}

impl FakeIo {
    fn new(cfg: Config) -> Self {
        Self {
            cfg,
            heap: Heap {
                budget: cfg.budget,
                used: Rc::new(Cell::new(0)),
            },
            spans: BTreeMap::new(),
            reads: Vec::new(),
            bg_started: 0,
            in_flight: None,
            flushed: 0,
        }
    }
    fn read(&mut self, start: u32, len: u32) -> Result<u32, IoErr> {
        self.reads.push((false, start, len));
        Ok(len / 1000)
    }
    fn start(&mut self, start: u32, len: u32) -> bool {
        self.bg_started += 1;
        let n = self.bg_started;
        if self.cfg.fail_start != 0 && n.is_multiple_of(self.cfg.fail_start) {
            return false;
        }
        self.reads.push((true, start, len));
        let ok = self.cfg.fail_bg == 0 || !n.is_multiple_of(self.cfg.fail_bg);
        self.in_flight = Some((self.cfg.delay, ok));
        true
    }
    fn finish(&mut self, wait: bool) -> Option<bool> {
        let (left, ok) = self.in_flight.expect("a read is in flight");
        if !wait && left > 0 {
            self.in_flight = Some((left - 1, ok));
            return None;
        }
        self.in_flight = None;
        Some(ok)
    }
    fn insert_run(&mut self, run: &[Span]) {
        for s in run {
            assert!(self
                .spans
                .insert(s.start, (s.len, s.ptr as usize))
                .is_none());
        }
    }
    fn remove(&mut self, starts: &[u32]) {
        for s in starts {
            assert!(self.spans.remove(s).is_some());
        }
    }
    fn max_block(&self) -> usize {
        self.heap.budget - self.heap.used.get()
    }
}

impl Io<Buf> for FakeIo {
    type Error = IoErr;
    const OUT_OF_MEMORY: IoErr = IoErr;
    fn read(&mut self, _block: &Buf, start: u32, len: u32) -> Result<u32, IoErr> {
        FakeIo::read(self, start, len)
    }
    fn start(&mut self, _block: &Buf, start: u32, len: u32) -> bool {
        FakeIo::start(self, start, len)
    }
    fn finish(&mut self, wait: bool) -> Option<bool> {
        FakeIo::finish(self, wait)
    }
    fn flush(&mut self, _block: &Buf) {
        self.flushed += 1;
    }
    fn insert_run(&mut self, run: &[Span]) {
        FakeIo::insert_run(self, run)
    }
    fn remove(&mut self, starts: &[u32]) {
        FakeIo::remove(self, starts)
    }
    fn max_block(&self) -> usize {
        FakeIo::max_block(self)
    }
}

// ---- the replaced loader (`scene_files.rs` at 904b6da1) ---------------------

struct OldJob {
    from: usize,
    last: usize,
    start: u32,
    len: u32,
    buf: Buf,
}

struct OldBlock {
    buf: Buf,
    live: u32,
}

struct Old<'a> {
    pack: &'a Pack<'a>,
    io: FakeIo,
    files: Vec<FileState>,
    blocks: Vec<Option<OldBlock>>,
    retired: Vec<Buf>,
    report: Report,
    demand: Vec<u16>,
    failed: Option<IoErr>,
    jobs: VecDeque<OldJob>,
    in_flight: bool,
}

impl<'a> Old<'a> {
    fn new(pack: &'a Pack<'a>, cfg: Config) -> Self {
        Self {
            pack,
            io: FakeIo::new(cfg),
            files: vec![EMPTY; pack.file_count() as usize],
            blocks: Vec::new(),
            retired: Vec::new(),
            report: Report::default(),
            demand: Vec::new(),
            failed: None,
            jobs: VecDeque::new(),
            in_flight: false,
        }
    }

    fn closure(&self, roots: &[u32]) -> Vec<bool> {
        let mut want = vec![false; self.files.len()];
        let mut queue: Vec<u32> = roots.to_vec();
        while let Some(file) = queue.pop() {
            let Some(i) = self.pack.file_index(file) else {
                continue;
            };
            if std::mem::replace(&mut want[i as usize], true) {
                continue;
            }
            if let Some(d) = self.pack.file_at(i) {
                queue.extend((0..d.dep_count).filter_map(|k| self.pack.file_dep(&d, k)));
            }
        }
        want
    }

    fn persist(&mut self, roots: &[u32]) -> Result<(), IoErr> {
        let want = self.closure(roots);
        for (i, w) in want.iter().enumerate() {
            if *w {
                self.files[i].keep = Keep::Always;
            }
        }
        self.load_missing(&want)
    }

    fn enter(&mut self, scene: &'static str, roots: &[u32], within: bool) -> Result<Report, IoErr> {
        let want = self.closure(roots);
        let wait = if within {
            self.jobs.iter().any(|j| (j.from..=j.last).any(|i| want[i]))
        } else {
            self.cancel_unwanted(&want);
            !self.jobs.is_empty()
        };
        if wait {
            self.drain();
        }
        let (mut prefetched_files, mut prefetched_bytes) = (0, 0);
        for (i, w) in want.iter().enumerate() {
            let held = self.files[i].block != NO_BLOCK;
            let keep = &mut self.files[i].keep;
            if *w && *keep == Keep::Next && held {
                prefetched_files += 1;
                prefetched_bytes += self.pack.file_at(i as u32).map_or(0, |d| d.len);
            }
            if *w && *keep != Keep::Always {
                *keep = Keep::Scene;
            } else if !*w && (*keep == Keep::Scene || (*keep == Keep::Next && !within)) {
                *keep = Keep::Leaving;
            }
        }
        self.report = Report {
            scene,
            prefetched_files,
            prefetched_bytes,
            wait_micros: 0,
            ..Report::default()
        };
        self.demand.clear();
        self.load_missing(&want)?;
        for (i, f) in self.files.iter().enumerate() {
            if matches!(f.keep, Keep::Scene | Keep::Always) {
                self.report.files += 1;
                self.report.bytes += self.pack.file_at(i as u32).map_or(0, |d| d.len);
            }
        }
        Ok(self.report)
    }

    fn prefetch(&mut self, roots: &[u32]) {
        let want = self.closure(roots);
        for (i, w) in want.iter().enumerate() {
            let keep = &mut self.files[i].keep;
            if *w && matches!(*keep, Keep::No | Keep::Leaving) {
                *keep = Keep::Next;
            }
        }
        self.queue_missing(&want);
        self.pump();
    }

    /// `safe_point`, the GE idle.
    fn safe_point(&mut self) -> usize {
        self.pump();
        let mut starts: Vec<u32> = Vec::new();
        for i in 0..self.files.len() {
            if self.files[i].keep == Keep::Leaving {
                if let Some(start) = self.drop_file(i) {
                    starts.push(start);
                }
            }
        }
        if !starts.is_empty() {
            self.io.remove(&starts);
        }
        let n = self.retired.len();
        self.retired.clear();
        n
    }

    fn held(&self) -> (usize, usize) {
        let live = self.blocks.iter().flatten();
        (live.clone().map(|b| b.buf.len).sum(), live.count())
    }

    fn on_miss(&mut self, offset: u32) -> bool {
        let Some(i) = self.pack.file_at_offset(offset) else {
            return false;
        };
        let i = i as usize;
        if self.files[i].queued {
            let Some(k) = self
                .jobs
                .iter()
                .position(|j| (j.from..=j.last).contains(&i))
            else {
                return false;
            };
            if k == 0 && self.in_flight {
                while self.files[i].queued && self.complete(true) {}
                return self.files[i].block != NO_BLOCK;
            }
            if let Some(job) = self.jobs.remove(k) {
                let mut rest = vec![false; self.files.len()];
                // As the replaced loader wrote it.
                #[allow(clippy::needless_range_loop)]
                for f in job.from..=job.last {
                    self.files[f].queued = false;
                    rest[f] = f != i;
                }
                drop(job);
                let loaded = self.demand_load(i);
                self.queue_missing(&rest);
                return loaded;
            }
            return false;
        }
        if self.files[i].block != NO_BLOCK {
            return false;
        }
        self.demand_load(i)
    }

    fn demand_load(&mut self, i: usize) -> bool {
        let mut want = vec![false; self.files.len()];
        want[i] = true;
        if self.files[i].keep == Keep::No {
            self.files[i].keep = Keep::Scene;
        }
        let before = (self.report.loaded_files, self.report.loaded_bytes);
        match self.load_missing(&want) {
            Ok(()) => {
                self.report.demand_files += self.report.loaded_files - before.0;
                self.report.demand_bytes += self.report.loaded_bytes - before.1;
                if self.demand.len() < DEMAND_LOG {
                    self.demand.push(
                        self.pack
                            .file_at(i as u32)
                            .map_or(u16::MAX, |d| d.file as u16),
                    );
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

    fn drop_file(&mut self, i: usize) -> Option<u32> {
        let f = self.files[i];
        self.files[i] = EMPTY;
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

    fn load_missing(&mut self, want: &[bool]) -> Result<(), IoErr> {
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

    fn read_run(
        &mut self,
        first: FileDesc,
        from: usize,
        last: usize,
        end: u32,
    ) -> Result<(), IoErr> {
        let len = (end - first.blob_start) as usize;
        let buf = Buf::new(len, &self.io.heap).ok_or(IoErr)?;
        let micros = self.io.read(first.blob_start, len as u32)?;
        self.report.micros += micros;
        self.report.reads += 1;
        self.register(buf, first.blob_start, from, last, true);
        Ok(())
    }

    fn register(&mut self, buf: Buf, start: u32, from: usize, last: usize, count: bool) {
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
                ptr: buf.ptr().wrapping_add((d.blob_start - start) as usize),
            });
            self.files[k].block = slot as u16;
            self.files[k].queued = false;
            live += 1;
            if count {
                self.report.loaded_files += 1;
                self.report.loaded_bytes += d.len;
            }
        }
        self.io.insert_run(&run);
        self.blocks[slot] = Some(OldBlock { buf, live });
    }

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
            if self.io.max_block() < len as usize + PREFETCH_MARGIN {
                return;
            }
            let Some(buf) = Buf::new(len as usize, &self.io.heap) else {
                return;
            };
            for k in i..=last {
                self.files[k].queued = self.pack.file_at(k as u32).is_some_and(|d| d.len > 0);
            }
            self.jobs.push_back(OldJob {
                from: i,
                last,
                start: first.blob_start,
                len,
                buf,
            });
            i = last + 1;
        }
    }

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

    fn drain(&mut self) {
        while self.in_flight || self.start_next() {
            self.complete(true);
        }
    }

    fn start_next(&mut self) -> bool {
        while let Some(job) = self.jobs.front() {
            let (start, len) = (job.start, job.len);
            if self.io.start(start, len) {
                self.in_flight = true;
                return true;
            }
            self.abandon_front();
        }
        false
    }

    fn complete(&mut self, wait: bool) -> bool {
        if !self.in_flight {
            return false;
        }
        let Some(ok) = self.io.finish(wait) else {
            return false;
        };
        self.in_flight = false;
        let Some(job) = self.jobs.pop_front() else {
            return true;
        };
        if !ok {
            self.release_job_files(&job);
        } else {
            self.io.flushed += 1;
            self.register(job.buf, job.start, job.from, job.last, false);
        }
        true
    }

    fn release_job_files(&mut self, job: &OldJob) {
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

// ---- the new loader, in `scene_files`' call shape --------------------------

struct Planned<'a> {
    plan: Plan<'a, Heap, IoErr>,
    io: FakeIo,
}

impl<'a> Planned<'a> {
    fn new(pack: Pack<'a>, cfg: Config) -> Self {
        let io = FakeIo::new(cfg);
        let plan = Plan::new(pack, io.heap.clone()).unwrap();
        Self { plan, io }
    }
    fn enter(&mut self, scene: &'static str, roots: &[u32], within: bool) -> Result<Report, IoErr> {
        if self.plan.enter_begin(roots, within) {
            self.plan.drain(&mut self.io);
        }
        self.plan.enter_end(&mut self.io, scene, within, 0)
    }
    fn safe_point(&mut self) -> usize {
        self.plan.safe_point(&mut self.io);
        self.plan.collect()
    }
}

// ---- comparison ------------------------------------------------------------

/// A loader's observable state: files, queued reads, registered spans with
/// each one's (block length, offset in block), and how files share blocks.
#[derive(Debug, PartialEq, Eq)]
struct Snapshot {
    files: Vec<(Keep, bool, bool)>,
    jobs: Vec<(usize, usize, u32, u32)>,
    spans: Vec<(u32, u32, usize, usize)>,
    groups: Vec<Vec<u32>>,
    held: (usize, usize),
    in_flight: bool,
}

fn snapshot(
    files: &[FileState],
    jobs: Vec<(usize, usize, u32, u32)>,
    spans: &BTreeMap<u32, (u32, usize)>,
    blocks: &[(usize, usize)],
    held: (usize, usize),
    in_flight: bool,
) -> Snapshot {
    let mut by_block: BTreeMap<usize, Vec<u32>> = BTreeMap::new();
    let spans = spans
        .iter()
        .map(|(&start, &(len, ptr))| {
            let &(base, blen) = blocks
                .iter()
                .find(|&&(base, blen)| ptr >= base && ptr + len as usize <= base + blen)
                .expect("every span lies in a live block");
            by_block.entry(base).or_default().push(start);
            (start, len, blen, ptr - base)
        })
        .collect();
    let mut groups: Vec<Vec<u32>> = by_block.into_values().collect();
    groups.sort();
    Snapshot {
        files: files
            .iter()
            .map(|f| (f.keep, f.block != NO_BLOCK, f.queued))
            .collect(),
        jobs,
        spans,
        groups,
        held,
        in_flight,
    }
}

fn old_snapshot(o: &Old<'_>) -> Snapshot {
    let blocks: Vec<(usize, usize)> = o
        .blocks
        .iter()
        .flatten()
        .map(|b| (b.buf.ptr() as usize, b.buf.len))
        .collect();
    let jobs = o
        .jobs
        .iter()
        .map(|j| (j.from, j.last, j.start, j.len))
        .collect();
    snapshot(&o.files, jobs, &o.io.spans, &blocks, o.held(), o.in_flight)
}

fn new_snapshot(n: &Planned<'_>) -> Snapshot {
    let arena = n.plan.arena();
    let blocks: Vec<(usize, usize)> = (0..NO_BLOCK)
        .filter_map(|id| Some((arena.ptr(id)? as usize, arena.len(id)?)))
        .collect();
    let jobs = n
        .plan
        .jobs()
        .iter()
        .map(|j| (j.from, j.last, j.start, j.len))
        .collect();
    snapshot(
        n.plan.files(),
        jobs,
        &n.io.spans,
        &blocks,
        n.plan.held(),
        n.plan.in_flight(),
    )
}

/// One step both loaders take.
enum Step {
    Persist(Vec<u32>),
    Enter(&'static str, Vec<u32>, bool),
    Prefetch(Vec<u32>),
    Frame,
    /// A residency miss on the first queued file (the read in flight, or
    /// one not started).
    MissQueued,
    /// A residency miss on the `n`-th file nobody holds.
    MissCold(usize),
}

/// `scene_load`'s calls for `events` (the composition `Sim::run` copies),
/// with misses mixed in.
fn steps(events: &[Event]) -> Vec<Step> {
    let mut out = vec![Step::Persist(vec![0])];
    let mut base: Vec<u32> = Vec::new();
    let mut base_in_battle = false;
    for (k, e) in events.iter().enumerate() {
        match e {
            Event::Scene(name, roots) => {
                base = roots.clone();
                base_in_battle = matches!(*name, "opening" | "explain" | "auto-demo");
                out.push(Step::Enter(name, roots.clone(), false));
            }
            Event::Battle(roots) => {
                let within = base_in_battle;
                if !within {
                    base.clear();
                }
                let mut all = base.clone();
                all.extend_from_slice(roots);
                out.push(Step::Enter("battle", all, within));
            }
            Event::SetBase(roots) => {
                base = roots.clone();
                base_in_battle = true;
            }
            Event::Maps(own, preview) => {
                base = own.clone();
                base_in_battle = false;
                let mut all = own.clone();
                all.extend_from_slice(preview);
                out.push(Step::Enter("maps", all, false));
            }
            Event::Preview(roots) => {
                let mut all = base.clone();
                all.extend_from_slice(roots);
                out.push(Step::Enter("maps-preview", all, true));
            }
            Event::Prefetch(roots) => {
                out.push(Step::Prefetch(roots.clone()));
                if k % 3 == 0 {
                    out.push(Step::MissQueued);
                }
            }
            Event::Frame => {
                out.push(Step::Frame);
                if k % 5 == 0 {
                    out.push(Step::MissCold(k * 7919));
                }
            }
        }
    }
    out
}

fn run_both(bytes: &[u8], pack: &Pack<'_>, steps: &[Step], cfg: Config) -> (u32, u32) {
    let mut old = Old::new(pack, cfg);
    let mut new = Planned::new(Pack::open(bytes).unwrap(), cfg);
    let (mut errors, mut misses) = (0, 0);
    for (k, step) in steps.iter().enumerate() {
        let what = match step {
            Step::Persist(roots) => {
                let a = old.persist(roots);
                let b = new.plan.persist(&mut new.io, roots);
                assert_eq!(a, b, "step {k}: persist");
                "persist"
            }
            Step::Enter(name, roots, within) => {
                let a = old.enter(name, roots, *within);
                let b = new.enter(name, roots, *within);
                errors += u32::from(a.is_err());
                assert_eq!(a, b, "step {k}: enter {name}");
                name
            }
            Step::Prefetch(roots) => {
                old.prefetch(roots);
                new.plan.prefetch(&mut new.io, roots);
                "prefetch"
            }
            Step::Frame => {
                let a = old.safe_point();
                let b = new.safe_point();
                assert_eq!(a, b, "step {k}: blocks freed");
                "frame"
            }
            Step::MissQueued | Step::MissCold(_) => {
                let file = match step {
                    Step::MissQueued => old.files.iter().position(|f| f.queued),
                    Step::MissCold(n) => {
                        let cold: Vec<usize> = (0..old.files.len())
                            .filter(|&i| {
                                old.files[i].block == NO_BLOCK
                                    && !old.files[i].queued
                                    && pack.file_at(i as u32).is_some_and(|d| d.len > 0)
                            })
                            .collect();
                        (!cold.is_empty()).then(|| cold[n % cold.len()])
                    }
                    _ => unreachable!(),
                };
                if let Some(i) = file {
                    let d = pack.file_at(i as u32).unwrap();
                    let offset = d.blob_start + d.len / 2;
                    let a = old.on_miss(offset);
                    let b = new.plan.miss(&mut new.io, offset).loaded;
                    misses += 1;
                    assert_eq!(a, b, "step {k}: miss on file index {i}");
                    assert_eq!(old.failed, new.plan.take_failure(), "step {k}: failure");
                    old.failed = None;
                }
                "miss"
            }
        };
        assert_eq!(old.report, new.plan.report(), "step {k} ({what}): report");
        assert_eq!(old.demand, new.plan.demand_files(), "step {k}: demand log");
        assert_eq!(old.io.reads, new.io.reads, "step {k} ({what}): reads");
        assert_eq!(old.io.flushed, new.io.flushed, "step {k}: flushes");
        assert_eq!(
            old.io.heap.used.get(),
            new.io.heap.used.get(),
            "step {k} ({what}): heap bytes"
        );
        assert_eq!(old_snapshot(&old), new_snapshot(&new), "step {k} ({what})");
    }
    (errors, misses)
}

fn real_pack() -> Option<Vec<u8>> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak"),
    )
    .ok()
}

/// The new plan equals the replaced loader over `scene_load`'s whole
/// attract loop and menu sequences, with reads that complete at once or
/// late, fail or fail to start, and a heap roomy or tight enough to stop
/// prefetches and fail loads.
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn plan_equals_the_replaced_loader() {
    let Some(bytes) = real_pack() else { return };
    let pack = Pack::open(&bytes).unwrap();
    let roots = Roots::new(&pack);
    let mut events = roots.attract();
    events.extend(roots.attract());
    events.extend(roots.menus());
    let steps = steps(&events);
    let configs = [
        (64 << 20, 0, 0, 0),
        (64 << 20, 2, 0, 0),
        (64 << 20, 1_000_000, 0, 0),
        (64 << 20, 1, 7, 11),
        (24 << 20, 1, 0, 0),
        (14 << 20, 0, 5, 0),
        (8 << 20, 0, 0, 0),
        (10 << 20, 1, 3, 0),
    ];
    let mut any_error = false;
    for (budget, delay, fail_bg, fail_start) in configs {
        let cfg = Config {
            budget,
            delay,
            fail_bg,
            fail_start,
        };
        let (errors, misses) = run_both(&bytes, &pack, &steps, cfg);
        println!(
            "{cfg:?}: {} steps, {errors} failed enters, {misses} misses",
            steps.len()
        );
        assert!(misses > 0, "{cfg:?}: the misses were exercised");
        any_error |= errors > 0;
    }
    assert!(any_error, "the tight heap exercised a failed load");
}

/// The plan's arena: held bytes match the blocks' files, kept files pin
/// an earlier scene's block, and every block is freed once nothing holds
/// it.
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn plan_blocks_follow_their_files() {
    let Some(bytes) = real_pack() else { return };
    let pack = Pack::open(&bytes).unwrap();
    let roots = Roots::new(&pack);
    let cfg = Config {
        budget: 64 << 20,
        delay: 0,
        fail_bg: 0,
        fail_start: 0,
    };
    let mut new = Planned::new(Pack::open(&bytes).unwrap(), cfg);
    let mut pinned = 0;
    for step in steps(&roots.attract()) {
        match step {
            // A persisted file never leaves; the equivalence test covers it.
            Step::Persist(_) => {}
            Step::Enter(name, r, within) => {
                new.enter(name, &r, within).unwrap();
                pinned = pinned.max(new.plan.arena().pinned().1);
            }
            Step::Prefetch(r) => new.plan.prefetch(&mut new.io, &r),
            Step::Frame => {
                new.safe_point();
            }
            _ => {}
        }
        let arena = new.plan.arena();
        // Every committed block's live count is the number of held files
        // in it; the heap holds exactly the arena's blocks.
        let mut live = vec![0u32; usize::from(NO_BLOCK)];
        for f in new.plan.files() {
            if f.block != NO_BLOCK {
                live[usize::from(f.block)] += 1;
            }
        }
        for id in 0..NO_BLOCK {
            if let Some(l) = arena.live(id) {
                let pending = new.plan.jobs().iter().any(|j| j.block == id);
                assert_eq!(l, if pending { 0 } else { live[usize::from(id)] });
            }
        }
        assert!(arena.high_water() >= arena.live_bytes());
    }
    assert!(
        pinned > 0,
        "files kept across scenes pinned an earlier block"
    );
    // Leaving everything frees every block.
    new.enter("empty", &[], false).unwrap();
    new.safe_point();
    new.plan.drain(&mut new.io);
    new.safe_point();
    let arena = new.plan.arena();
    assert_eq!(new.plan.held(), (0, 0));
    assert_eq!(
        (arena.live_bytes(), arena.retired()),
        (0, 0),
        "nothing held, nothing allocated"
    );
    assert_eq!(new.io.heap.used.get(), 0);
    assert!(new.io.spans.is_empty());
}

/// The stage select as the N64 runs it (`mnMapsMakePreview`, RE-572): one
/// preview held at a time, read when the cursor lands; the last one's
/// files stay through the frame that may still draw them and leave at its
/// safe point, while the battle files read ahead carry on.
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn a_stage_select_holds_the_cursors_preview_alone() {
    let Some(bytes) = real_pack() else { return };
    let pack = Pack::open(&bytes).unwrap();
    let cfg = Config {
        budget: 64 << 20,
        delay: 1_000_000,
        fail_bg: 0,
        fail_start: 0,
    };
    let mut new = Planned::new(Pack::open(&bytes).unwrap(), cfg);
    let files = |roots: &[u32]| {
        let (mut want, mut ids) = (Vec::new(), Vec::new());
        crate::reloc_link::closure(&pack, roots, &mut want, &mut ids).unwrap();
        (0..want.len()).filter(|&i| want[i]).collect::<Vec<usize>>()
    };
    let held = |new: &Planned<'_>, set: &[usize]| {
        set.iter().all(|&i| {
            new.plan.files()[i].block != NO_BLOCK && new.plan.files()[i].keep == Keep::Scene
        })
    };
    let own = crate::scene_roots::MAPS.to_vec();
    let preview = |g: u8| Roots::preview(false, g);
    let with = |g: u8| {
        let mut v = own.clone();
        v.extend(preview(g));
        v
    };
    // Only the files neither the scene nor the other preview holds.
    let only = |g: u8, other: u8| {
        let (mine, theirs) = (files(&with(g)), files(&with(other)));
        mine.into_iter()
            .filter(|i| !theirs.contains(i))
            .collect::<Vec<usize>>()
    };
    new.enter("maps", &with(0), false).unwrap();
    // A background read that never completes: the battle's fighters.
    new.plan
        .prefetch(&mut new.io, &[crate::scene_roots::FIGHTER_MAIN[0]]);
    let next: Vec<usize> = (0..new.plan.files().len())
        .filter(|&i| new.plan.files()[i].keep == Keep::Next)
        .collect();
    assert!(!next.is_empty());
    new.safe_point();
    let castle = only(0, 6);
    assert!(held(&new, &files(&with(0))));
    let (one, _) = new.plan.held();

    let report = new.enter("maps-preview", &with(6), true).unwrap();
    assert!(report.loaded_files > 0 && report.reads > 0);
    assert!(
        new.plan.in_flight() && !new.plan.jobs().is_empty(),
        "the read ahead is not waited for"
    );
    // The frame that reads it may still draw the last preview.
    assert!(castle
        .iter()
        .all(|&i| new.plan.files()[i].keep == Keep::Leaving));
    assert!(castle
        .iter()
        .all(|&i| new.plan.files()[i].block != NO_BLOCK));
    new.safe_point();
    assert!(castle
        .iter()
        .all(|&i| new.plan.files()[i].block == NO_BLOCK));
    assert!(held(&new, &files(&with(6))));
    assert!(next.iter().all(|&i| new.plan.files()[i].keep == Keep::Next));

    // The random slot reads nothing and holds no preview.
    let report = new.enter("maps-preview", &with(0xDE), true).unwrap();
    assert_eq!((report.loaded_files, report.reads), (0, 0));
    new.safe_point();
    assert!(only(6, 0xDE)
        .iter()
        .all(|&i| new.plan.files()[i].block == NO_BLOCK));
    let (none, _) = new.plan.held();
    assert!(none < one);

    // Back on Peach's Castle: what the select held at its start.
    new.enter("maps-preview", &with(0), true).unwrap();
    new.safe_point();
    assert_eq!(new.plan.held().0, one);
    assert!(next.iter().all(|&i| new.plan.files()[i].keep == Keep::Next));
}
