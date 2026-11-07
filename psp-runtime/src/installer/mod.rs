//! The first-launch installer (PLAN.md; RE-588, D-048): the release ships
//! without a pack, so when the game's boot finds no valid `ssb64.pak`
//! (missing, another version, cut short: [`crate::assets::pack_status`])
//! it LoadExecs `INSTALL.PBP` (`psp-installer`, [`crate::launch`]), which
//! runs this: it builds the pack from the user's ROM with
//! `ssb_pack::build_with` (ROM read in pages, blob spilled to a file:
//! RE-588's PSP-1000 working set), then returns to the XMB. The game's
//! code is not loaded meanwhile; the next launch of the game boots it.
//!
//! Files, in the output root (the bad pack's search root, else the ROM's):
//! `ssb64.pak.tmp` (the pack being written, renamed to `ssb64.pak` once
//! checked) and `ssb64.spill.tmp` (the blob's spill). Both are removed on
//! failure and at the next start. `install.log` beside the EBOOT: one
//! `start` line per run, then the phases with times and memory.
//!
//! Threads: the main thread builds; a ticker (priority 0x18, above the
//! main thread's 32) keeps the PSP awake, owns the HOME exit callback and
//! redraws the progress screen once a second from atomics.

pub mod io;
mod screen;

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::UnsafeCell;
use core::ffi::c_void;
use core::fmt;
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicUsize, Ordering};

use psp::sys;
use ssb_pack::install::{self, InstallError, Progress, RomPick, Space, PHASES};
use ssb_rom::pack::PackState;

use crate::assets::{self, PackFile, PACK_NAME, SEARCH_ROOTS};
use crate::{heap, memory, thread, timing};
use io::{log, now, CPath};
use screen::{Clock, Frame};

/// The ROM's page cache: 16 pages of 16 KiB, the fewest file reads per
/// byte held of the host's sweep (RE-588 item 14).
const ROM_PAGE: usize = 16 * 1024;
const ROM_PAGES: usize = 16;
/// The pack while it is written, and the blob's spill.
const TMP_PACK: &str = "ssb64.pak.tmp";
const TMP_SPILL: &str = "ssb64.spill.tmp";
/// The ticker's period and priority.
const TICK_US: u32 = 1_000_000;
const TICK_PRIORITY: i32 = 0x18;
/// The progress bar's width in columns.
const BAR: usize = 64;
const NO_ROOT: usize = usize::MAX;

/// `sceKernelGetSystemTimeLow` at [`run`]'s start.
static START: AtomicU32 = AtomicU32::new(0);
/// Elapsed microseconds at the build's start and at each phase's end.
static BUILD_START: AtomicU32 = AtomicU32::new(0);
static PHASE_END: [AtomicU32; PHASES.len()] = [const { AtomicU32::new(0) }; PHASES.len()];
/// Elapsed microseconds at the last logged phase.
static LAST_PHASE: AtomicU32 = AtomicU32::new(0);
/// Phases ended, and the running phase's steps (`PackLog::progress`).
static ENDED: AtomicUsize = AtomicUsize::new(0);
static STEP_DONE: AtomicUsize = AtomicUsize::new(0);
static STEP_TOTAL: AtomicUsize = AtomicUsize::new(0);
/// The output root (index into `SEARCH_ROOTS`), for the cleanup.
static OUT_ROOT: AtomicUsize = AtomicUsize::new(NO_ROOT);
/// The ticker draws the progress screen while set.
static BUILDING: AtomicBool = AtomicBool::new(false);
/// Held by whoever prints to the debug console (its buffer is one static).
static DRAW_LOCK: AtomicBool = AtomicBool::new(false);
/// The ROM's path, for the screen: written before `BUILDING` is set.
static ROM_TEXT: Text = Text(UnsafeCell::new([0; 96]), AtomicUsize::new(0));

/// A short text written once by the main thread before the ticker reads
/// it (ordered by `BUILDING`'s release store).
struct Text(UnsafeCell<[u8; 96]>, AtomicUsize);

// SAFETY: written only before `BUILDING` is set, read only after (see
// above).
unsafe impl Sync for Text {}

impl Text {
    fn set(&self, s: &str) {
        let mut n = s.len().min(96);
        while !s.is_char_boundary(n) {
            n -= 1;
        }
        // SAFETY: as the `Sync` impl: no reader yet.
        unsafe { (&mut *self.0.get())[..n].copy_from_slice(&s.as_bytes()[..n]) };
        self.1.store(n, Ordering::Release);
    }

    fn get(&self) -> &str {
        let n = self.1.load(Ordering::Acquire);
        // SAFETY: as the `Sync` impl: no writer any more.
        let bytes = unsafe { &(&*self.0.get())[..n] };
        core::str::from_utf8(bytes).unwrap_or("?")
    }
}

/// Microseconds since [`run`]'s start.
fn elapsed() -> u32 {
    now().wrapping_sub(START.load(Ordering::Relaxed))
}

fn tick() {
    // SAFETY: no preconditions; resets the auto-sleep timer (RE-481).
    unsafe { sys::scePowerTick(sys::PowerTick::All) };
}

/// Builds the pack, shows the result and exits to the XMB on a button
/// press. `status`: what the boot's gate found (`None`: no pack).
pub fn run(status: Option<PackFile>) -> ! {
    START.store(now(), Ordering::Relaxed);
    spawn_ticker();
    memory::on_fatal(on_fatal);
    let clock = timing::full_speed();
    // SAFETY: no preconditions.
    unsafe {
        sys::sceCtrlSetSamplingCycle(0);
        sys::sceCtrlSetSamplingMode(sys::CtrlMode::Digital);
    }
    log(format_args!(
        "start pack={status:?} clock_rc={clock} free={} max={} stack_free={}",
        memory::free(),
        memory::max_block(),
        thread::stack_free_bytes(),
    ));
    message(&["Looking for the ROM..."]);
    let result = match psp::catch_unwind(|| install(status)) {
        Ok(result) => result,
        Err(payload) => {
            let msg = payload
                .downcast_ref::<String>()
                .map(String::as_str)
                .or_else(|| payload.downcast_ref::<&str>().copied())
                .unwrap_or("(no message)");
            log(format_args!("panic t={} {msg}", elapsed()));
            cleanup();
            Err(InstallError::Panic(String::from(msg)))
        }
    };
    match result {
        Ok(root) => done(root),
        Err(e) => failed(&e),
    }
}

/// `root` (index into `SEARCH_ROOTS`) joined with `name`.
fn path(root: usize, name: &str) -> CPath {
    CPath::new(&[SEARCH_ROOTS[root], name]).unwrap_or(CPath::empty())
}

/// Closes the build's files and removes the temporary ones.
fn cleanup() {
    io::close_all();
    let root = OUT_ROOT.load(Ordering::Relaxed);
    if root < SEARCH_ROOTS.len() {
        io::remove(&path(root, TMP_PACK));
        io::remove(&path(root, TMP_SPILL));
    }
}

/// The install; the output root on success.
fn install(status: Option<PackFile>) -> Result<usize, InstallError> {
    // A stopped run's leftovers (HOME, power off).
    for root in 0..SEARCH_ROOTS.len() {
        for name in [TMP_PACK, TMP_SPILL] {
            let r = io::remove(&path(root, name));
            if r >= 0 {
                log(format_args!("removed {}{name}", SEARCH_ROOTS[root]));
            }
        }
    }
    let (rom_root, rom_name) = find_rom()?;
    let out = status.map_or(rom_root, |p| p.root);
    OUT_ROOT.store(out, Ordering::Relaxed);
    let old_pack = path(out, PACK_NAME);
    let old_len = status.map_or(0, |p| p.len);
    match io::free_bytes(SEARCH_ROOTS[out]) {
        Ok(free) => {
            let plan = install::plan_space(free, old_len);
            log(format_args!(
                "space free={free} old_pack={old_len} plan={plan:?}"
            ));
            match plan {
                Space::Enough => {}
                Space::AfterRemovingOld => {
                    let r = io::remove(&old_pack);
                    log(format_args!("removed {} r={r:#x}", old_pack.as_str()));
                }
                Space::Short { need, free } => return Err(InstallError::NoSpace { need, free }),
            }
        }
        // The command is unverified on hardware: build anyway; a full
        // stick then fails the write (`InstallError::DiskFull`).
        Err(code) => log(format_args!("space unknown r={code:#x}")),
    }

    let rom_path = path(rom_root, &rom_name);
    ROM_TEXT.set(rom_path.as_str());
    let (file, len) =
        io::RomFile::open(&rom_path).map_err(|code| InstallError::RomOpen { code })?;
    let rom = match ssb_rom::rom_read::PagedRom::new(file, len as usize, ROM_PAGE, ROM_PAGES) {
        Ok(rom) => rom,
        Err(e) => {
            io::close_all();
            log(format_args!("error rom: {e}"));
            return Err(InstallError::from_rom(&e));
        }
    };
    log(format_args!(
        "rom path={} bytes={len} order={} page={ROM_PAGE} pages={ROM_PAGES} free={} max={}",
        rom_path.as_str(),
        rom.order().name(),
        memory::free(),
        memory::max_block(),
    ));
    let tmp = path(out, TMP_PACK);
    let spill = io::IoSpill::create(&path(out, TMP_SPILL)).map_err(|code| {
        cleanup();
        InstallError::write("a temporary file", code)
    })?;
    let mut sink = io::FileSink::create(&tmp).map_err(|code| {
        cleanup();
        InstallError::write("ssb64.pak", code)
    })?;
    phase_line("rom");
    lock();
    BUILD_START.store(elapsed(), Ordering::Relaxed);
    BUILDING.store(true, Ordering::Release);
    unlock();

    let opts = ssb_pack::PackOptions::default();
    let result = ssb_pack::build_with(
        &rom,
        &opts,
        &Log,
        &ssb_pack::NoObserver,
        &mut sink,
        Some(Box::new(spill)),
    );
    let build_us = elapsed().wrapping_sub(BUILD_START.load(Ordering::Relaxed));
    let flushed = sink.flush();
    sink.close();
    io::close_spill();
    io::remove(&path(out, TMP_SPILL));
    let reads = rom.stats();
    rom.into_file().close();
    log(format_args!(
        "rom_reads reads={} bytes={} misses={} direct={} file_reads={} file_bytes={} rom_us={} \
         spill_us={}",
        reads.reads,
        reads.bytes,
        reads.misses,
        reads.direct,
        reads.raw_reads,
        reads.raw_bytes,
        io::ROM_US.load(Ordering::Relaxed),
        io::SPILL_US.load(Ordering::Relaxed),
    ));
    phase_line("closed");
    let stats = match (result, flushed) {
        (Ok(stats), Ok(())) => stats,
        (Err(e), _) => {
            log(format_args!(
                "error t={} build_us={build_us}: {e}",
                elapsed()
            ));
            cleanup();
            return Err(write_error().unwrap_or_else(|| InstallError::from_build(&e)));
        }
        (Ok(_), Err(code)) => {
            log(format_args!("error t={} pack flush {code:#x}", elapsed()));
            cleanup();
            return Err(InstallError::write("ssb64.pak", code));
        }
    };
    log(format_args!(
        "built t={} build_us={build_us} pack_bytes={} sink_us={} sink_writes={} log_us={}",
        elapsed(),
        io::SINK_WRITTEN.load(Ordering::Relaxed),
        io::SINK_US.load(Ordering::Relaxed),
        io::SINK_CALLS.load(Ordering::Relaxed),
        io::LOG_US.load(Ordering::Relaxed),
    ));
    log(format_args!(
        "stats meshes={} triangles={} sprites={} objects={} fighters={} fighters_failed={:?} \
         packed_anims={} anims_failed={} particle_scripts={} particle_textures={}",
        stats.meshes,
        stats.triangles,
        stats.sprites,
        stats.objects,
        stats.fighters,
        stats.fighters_failed,
        stats.packed_anims,
        stats.anims_failed.len(),
        stats.particle_scripts,
        stats.particle_textures,
    ));
    drop(stats);

    // The written file must pass the boot's own gate before it replaces
    // anything.
    let len = io::file_len(&tmp).map_err(|code| {
        cleanup();
        InstallError::write("ssb64.pak", code)
    })?;
    let mut head = [0u8; ssb_rom::pack::Header::SIZE];
    let n = io::read_head(&tmp, &mut head).map_err(|code| {
        cleanup();
        InstallError::write("ssb64.pak", code)
    })?;
    let state = ssb_rom::pack::pack_state(&head[..n], len);
    if state != PackState::Ok {
        log(format_args!("error written pack len={len} state={state:?}"));
        cleanup();
        return Err(InstallError::Build(alloc::format!(
            "the written ssb64.pak fails its check ({state:?})"
        )));
    }
    // The PSP renames within the source's directory and fails onto an
    // existing name: remove the old pack first.
    let r = io::remove(&old_pack);
    log(format_args!("remove {} r={r:#x}", old_pack.as_str()));
    let r = io::rename(&tmp, &old_pack);
    log(format_args!(
        "rename {} -> {} r={r:#x}",
        tmp.as_str(),
        old_pack.as_str()
    ));
    if r < 0 {
        cleanup();
        return Err(InstallError::write("ssb64.pak", r));
    }
    let gate = assets::pack_status();
    log(format_args!("gate {gate:?}"));
    if !matches!(
        gate,
        Some(PackFile {
            state: PackState::Ok,
            ..
        })
    ) {
        return Err(InstallError::Build(alloc::format!(
            "ssb64.pak was written but the game does not accept it ({gate:?})"
        )));
    }
    Ok(out)
}

/// The failed write the build only saw as a message, by kind.
fn write_error() -> Option<InstallError> {
    let what = match io::WRITE_WHAT.load(Ordering::Relaxed) {
        io::WRITE_PACK => "ssb64.pak",
        io::WRITE_SPILL => "a temporary file",
        _ => return None,
    };
    Some(InstallError::write(
        what,
        io::WRITE_CODE.load(Ordering::Relaxed),
    ))
}

/// The ROM: the first search root holding one (`install::pick_rom`); its
/// root and file name.
fn find_rom() -> Result<(usize, String), InstallError> {
    for (i, root) in SEARCH_ROOTS.iter().enumerate() {
        let mut names: Vec<String> = Vec::new();
        let listed = io::list_files(root, |name| {
            if install::has_rom_extension(name) {
                names.push(String::from(name));
            }
        });
        // A directory that does not list, or lists no dump: the fixed
        // names by opening them.
        if names.is_empty() {
            for name in install::ROM_NAMES {
                if io::file_len(&path(i, name)).is_ok() {
                    names.push(String::from(name));
                }
            }
        }
        log(format_args!(
            "search root={root:?} listed={listed} roms={names:?}"
        ));
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        match install::pick_rom(&refs) {
            RomPick::Named(n) | RomPick::Single(n) => return Ok((i, names.swap_remove(n))),
            RomPick::Several(count) => return Err(InstallError::SeveralRoms { count }),
            RomPick::None => {}
        }
    }
    Err(InstallError::NoRom)
}

/// One `phase` log line: times, memory and I/O so far (the batch-G probe's
/// fields, RE-588 item 14).
fn phase_line(name: &str) {
    let t = elapsed();
    let dt = t.wrapping_sub(LAST_PHASE.swap(t, Ordering::Relaxed));
    let small = heap::stats();
    let (allocs, kernel_allocs) = heap::counts();
    memory::sample();
    let (_, low) = memory::low_water();
    log(format_args!(
        "phase {name} t={t} dt={dt} sink_us={} sink_bytes={} log_us={} free={} max={} low={low} \
         heap_chunks={} heap_chunk_bytes={} heap_live={} allocs={allocs} \
         kernel_allocs={kernel_allocs} stack_free={}",
        io::SINK_US.load(Ordering::Relaxed),
        io::SINK_WRITTEN.load(Ordering::Relaxed),
        io::LOG_US.load(Ordering::Relaxed),
        memory::free(),
        memory::max_block(),
        small.chunks,
        small.chunk_bytes,
        small.live_bytes,
        thread::stack_free_bytes(),
    ));
}

/// The build's [`ssb_pack::PackLog`].
struct Log;

impl ssb_pack::PackLog for Log {
    fn line(&self, args: fmt::Arguments<'_>) {
        log(format_args!("line t={} {args}", elapsed()));
    }

    /// Dropped: one line per texture, about 12 s of Memory Stick writes on
    /// the PSP (RE-588).
    fn diag(&self, _args: fmt::Arguments<'_>) {}

    fn phase(&self, name: &str) {
        phase_line(name);
        let t = elapsed();
        let before = ENDED.load(Ordering::Relaxed);
        let mut p = Progress::at(before, 0, 0);
        p.phase_ended(name);
        for end in &PHASE_END[before.min(PHASES.len())..p.ended()] {
            end.store(t, Ordering::Relaxed);
        }
        STEP_TOTAL.store(0, Ordering::Relaxed);
        STEP_DONE.store(0, Ordering::Relaxed);
        ENDED.store(p.ended(), Ordering::Relaxed);
    }

    fn progress(&self, done: usize, total: usize) {
        STEP_TOTAL.store(total, Ordering::Relaxed);
        STEP_DONE.store(done, Ordering::Relaxed);
    }
}

fn try_lock() -> bool {
    DRAW_LOCK
        .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
        .is_ok()
}

fn lock() {
    while !try_lock() {
        // SAFETY: plain sleep; the ticker holds the lock for one draw.
        unsafe { sys::sceKernelDelayThread(1000) };
    }
}

fn unlock() {
    DRAW_LOCK.store(false, Ordering::Release);
}

/// A path on screen: a relative one (the EBOOT's folder) named as such;
/// a search root (`SEARCH_ROOTS`) as the pack in it.
struct Shown<'a>(&'a str);

impl fmt::Display for Shown<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let p = self.0;
        if p.is_empty() {
            write!(f, "{PACK_NAME} (in the game's folder)")
        } else if p.ends_with('/') {
            write!(f, "{p}{PACK_NAME}")
        } else if p.contains(':') {
            f.write_str(p)
        } else {
            write!(f, "{p} (in the game's folder)")
        }
    }
}

/// The screen's first rows.
fn header(f: &mut Frame) {
    f.line(format_args!(
        "Super Smash Bros. 64 (PSP) - first-launch setup"
    ));
    f.line(format_args!(
        "-----------------------------------------------"
    ));
    f.blank();
}

/// A frame of `lines` under the header, from the main thread.
fn message(lines: &[&str]) {
    let mut f = Frame::new();
    header(&mut f);
    for l in lines {
        f.line(format_args!("{l}"));
    }
    lock();
    f.show();
    unlock();
}

/// The build's screen (the ticker's, once a second).
fn draw_progress() {
    let t = elapsed();
    let t0 = BUILD_START.load(Ordering::Relaxed);
    let ended = ENDED.load(Ordering::Relaxed).min(PHASES.len());
    let (done, total) = if ended + 1 == PHASES.len() {
        // The last phase writes the pack: its bytes are the steps.
        (
            io::SINK_WRITTEN.load(Ordering::Relaxed),
            install::PACK_BYTES as usize,
        )
    } else {
        (
            STEP_DONE.load(Ordering::Relaxed),
            STEP_TOTAL.load(Ordering::Relaxed),
        )
    };
    let progress = Progress::at(ended, done, total);
    let pm = progress.per_mille();
    let out = OUT_ROOT.load(Ordering::Relaxed);
    let mut f = Frame::new();
    header(&mut f);
    f.line(format_args!(
        "Building the game data from your ROM. This happens once, and takes about"
    ));
    f.line(format_args!("10 minutes on a PSP-1000."));
    f.blank();
    f.line(format_args!("ROM:  {}", Shown(ROM_TEXT.get())));
    f.line(format_args!(
        "Data: {}",
        Shown(SEARCH_ROOTS.get(out).copied().unwrap_or(""))
    ));
    f.blank();
    for (i, phase) in PHASES.iter().enumerate() {
        if i < ended {
            let end = PHASE_END[i].load(Ordering::Relaxed);
            let begin = if i == 0 {
                t0
            } else {
                PHASE_END[i - 1].load(Ordering::Relaxed)
            };
            let dt = u64::from(end.wrapping_sub(begin));
            f.line(format_args!("  done {:>6}  {}", Clock(dt), phase.label));
        } else if i == ended && total > 0 {
            let pct = (done.min(total) as u64 * 100 / total as u64).min(99);
            f.line(format_args!("  {pct:>3}%        {}", phase.label));
        } else if i == ended {
            f.line(format_args!("  ...          {}", phase.label));
        } else {
            f.line(format_args!("               {}", phase.label));
        }
    }
    f.blank();
    screen::bar(&mut f, pm, BAR);
    f.line(format_args!(" {:>3}.{}%", pm / 10, pm % 10));
    let spent = u64::from(t.wrapping_sub(t0));
    if pm >= 30 && pm < 1000 {
        let left = spent * u64::from(1000 - pm) / u64::from(pm);
        let minutes = left.div_ceil(60_000_000).max(1);
        f.line(format_args!(
            "Elapsed {}   About {minutes} min left",
            Clock(u64::from(t))
        ));
    } else {
        f.line(format_args!("Elapsed {}", Clock(u64::from(t))));
    }
    f.pad_to(screen::ROWS - 2);
    f.line(format_args!(
        "Keep the PSP charged or plugged in. Do not remove the Memory Stick."
    ));
    f.line(format_args!(
        "Press HOME to stop: the setup starts over at the next launch."
    ));
    f.show();
}

/// Waits for a button pressed after this call (HOME quits through the exit
/// callback), then quits to the XMB.
fn wait_and_exit() -> ! {
    let any = sys::CtrlButtons::SELECT
        | sys::CtrlButtons::START
        | sys::CtrlButtons::UP
        | sys::CtrlButtons::RIGHT
        | sys::CtrlButtons::DOWN
        | sys::CtrlButtons::LEFT
        | sys::CtrlButtons::LTRIGGER
        | sys::CtrlButtons::RTRIGGER
        | sys::CtrlButtons::TRIANGLE
        | sys::CtrlButtons::CIRCLE
        | sys::CtrlButtons::CROSS
        | sys::CtrlButtons::SQUARE;
    // SAFETY: the display is the debug console's; the screen is drawn.
    unsafe { sys::sceDisplayWaitVblankStart() };
    crate::gu::emit_headless_screenshot();
    let mut released = false;
    loop {
        // SAFETY: plain vblank wait.
        unsafe { sys::sceDisplayWaitVblankStart() };
        let mut pad = sys::SceCtrlData::default();
        // SAFETY: `pad` outlives the call.
        unsafe { sys::sceCtrlPeekBufferPositive(&mut pad, 1) };
        let down = pad.buttons.intersects(any);
        if !down {
            released = true;
        } else if released {
            break;
        }
    }
    log(format_args!("exit t={}", elapsed()));
    // SAFETY: no preconditions; does not return.
    unsafe { sys::sceKernelExitGame() };
    loop {
        // SAFETY: plain sleep, should the exit be slow.
        unsafe { sys::sceKernelDelayThread(TICK_US) };
    }
}

/// The success screen.
fn done(out: usize) -> ! {
    BUILDING.store(false, Ordering::Release);
    lock();
    let t = elapsed();
    log(format_args!(
        "done t={t} free={} low={}",
        memory::free(),
        memory::low_water().1
    ));
    let mut f = Frame::new();
    header(&mut f);
    f.line(format_args!("Setup complete in {}.", Clock(u64::from(t))));
    f.blank();
    f.line(format_args!(
        "The game data was written to {}.",
        Shown(SEARCH_ROOTS[out])
    ));
    f.line(format_args!(
        "Keep the ROM: a later version of the game may need to build it again."
    ));
    f.blank();
    f.line(format_args!(
        "Press any button to return to the home menu, then start the game again."
    ));
    f.show();
    wait_and_exit()
}

/// The error screen.
fn failed(e: &InstallError) -> ! {
    BUILDING.store(false, Ordering::Release);
    lock();
    log(format_args!("failed t={} {e:?}", elapsed()));
    let mut f = Frame::new();
    header(&mut f);
    f.line(format_args!("The setup stopped."));
    f.blank();
    f.line(format_args!("{e}"));
    f.blank();
    f.line(format_args!(
        "Details are in install.log, in the game's folder."
    ));
    f.pad_to(screen::ROWS - 1);
    f.line(format_args!("Press any button (or HOME) to quit."));
    f.show();
    wait_and_exit()
}

/// `memory::fatal`'s hook (an allocation failed, or another fatal screen):
/// logs, removes the temporary files and prints the top of the screen;
/// `fatal` prints its own 8 lines below.
fn on_fatal() {
    log(format_args!(
        "fatal t={} phase={} free={} max={} (cause on screen)",
        elapsed(),
        ENDED.load(Ordering::Relaxed),
        memory::free(),
        memory::max_block(),
    ));
    cleanup();
    BUILDING.store(false, Ordering::Release);
    // Bounded: the main thread may hold the lock itself (a fatal while a
    // final screen draws).
    for _ in 0..100 {
        if try_lock() {
            break;
        }
        // SAFETY: plain sleep.
        unsafe { sys::sceKernelDelayThread(1000) };
    }
    let mut f = Frame::new();
    header(&mut f);
    f.line(format_args!(
        "The setup stopped. Nothing was changed: the setup starts"
    ));
    f.line(format_args!("over at the next launch."));
    f.blank();
    f.line(format_args!(
        "If memory ran out: turn off plugins, restart the PSP, then try again."
    ));
    f.line(format_args!(
        "Details are in install.log, in the game's folder."
    ));
    f.show_rows(screen::ROWS - 8);
}

/// The ticker: keeps the PSP awake, samples free memory, owns the HOME
/// exit callback and draws the build's screen.
fn spawn_ticker() {
    // No `install.log` line here: the log's path is relative to the main
    // thread's working directory, which this thread does not share
    // (PPSSPP drops such writes).
    unsafe extern "C" fn exit_callback(_a: i32, _b: i32, _arg: *mut c_void) -> i32 {
        // SAFETY: no preconditions; the next start removes the
        // temporary files.
        unsafe { sys::sceKernelExitGame() };
        0
    }
    unsafe extern "C" fn entry(_args: usize, _argp: *mut c_void) -> i32 {
        // SAFETY: NUL-terminated static name; the callback runs on this
        // thread, in `sceKernelDelayThreadCB`.
        unsafe {
            let cb = sys::sceKernelCreateCallback(
                b"installer_exit\0".as_ptr(),
                exit_callback,
                core::ptr::null_mut(),
            );
            if cb.0 >= 0 {
                sys::sceKernelRegisterExitCallback(cb);
            }
        }
        loop {
            memory::sample();
            if BUILDING.load(Ordering::Acquire) {
                tick();
                if try_lock() {
                    if BUILDING.load(Ordering::Acquire) {
                        draw_progress();
                    }
                    unlock();
                }
            }
            // SAFETY: plain sleep that runs the exit callback.
            unsafe { sys::sceKernelDelayThreadCB(TICK_US) };
        }
    }
    // SAFETY: NUL-terminated static name; `entry` takes no arguments.
    unsafe {
        let id = sys::sceKernelCreateThread(
            b"installer_tick\0".as_ptr(),
            entry,
            TICK_PRIORITY,
            16 * 1024,
            sys::ThreadAttributes::USER,
            core::ptr::null_mut(),
        );
        if id.0 >= 0 {
            sys::sceKernelStartThread(id, 0, core::ptr::null_mut());
        }
    }
}
