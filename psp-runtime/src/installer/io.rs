//! The installer's files (from the batch-G pack probe, RE-588): the ROM
//! read in pages, the blob spill, the pack sink, the log, and the Memory
//! Stick queries. Every call times itself into a counter for the log.

use core::ffi::c_void;
use core::fmt::{self, Write};
use core::sync::atomic::{AtomicI32, AtomicU32, AtomicU8, AtomicUsize, Ordering};

use psp::sys;

/// Microseconds in the ROM's reads, the spill's reads and writes, the
/// pack's writes and the log's writes.
pub static ROM_US: AtomicU32 = AtomicU32::new(0);
pub static SPILL_US: AtomicU32 = AtomicU32::new(0);
pub static SINK_US: AtomicU32 = AtomicU32::new(0);
pub static LOG_US: AtomicU32 = AtomicU32::new(0);
/// Pack bytes written and `sceIoWrite` calls.
pub static SINK_WRITTEN: AtomicUsize = AtomicUsize::new(0);
pub static SINK_CALLS: AtomicU32 = AtomicU32::new(0);

/// The first failed write: its code and what it wrote ([`WRITE_PACK`],
/// [`WRITE_SPILL`]); 0 for none. The build's own error only says "sink".
pub static WRITE_CODE: AtomicI32 = AtomicI32::new(0);
pub static WRITE_WHAT: AtomicU8 = AtomicU8::new(0);
pub const WRITE_PACK: u8 = 1;
pub const WRITE_SPILL: u8 = 2;

/// The descriptors open during the build (ROM, spill, pack), so a failure
/// anywhere can close them before removing the temporary files; -1 for
/// none.
static OPEN: [AtomicI32; 3] = [AtomicI32::new(-1), AtomicI32::new(-1), AtomicI32::new(-1)];
const OPEN_ROM: usize = 0;
const OPEN_SPILL: usize = 1;
const OPEN_PACK: usize = 2;

/// The pack's write buffer: one `sceIoWrite` per this many bytes (RE-588
/// item 12: 46.3 MB in 27.1 s). On the heap, 64-byte aligned, taken at the
/// first write (the final phase, below the build's peak), never in `.bss`.
const SINK_BYTES: usize = 256 * 1024;

/// The install log, appended to, beside the EBOOT.
const LOG: &[u8] = b"install.log\0";

pub fn now() -> u32 {
    // SAFETY: a clock read with no preconditions.
    unsafe { sys::sceKernelGetSystemTimeLow() }
}

/// A NUL-terminated path: a root and a name.
pub struct CPath {
    buf: [u8; 288],
    len: usize,
}

impl CPath {
    pub const fn empty() -> Self {
        Self {
            buf: [0; 288],
            len: 0,
        }
    }

    /// `parts` joined; `None` when too long.
    pub fn new(parts: &[&str]) -> Option<Self> {
        let mut p = Self::empty();
        for part in parts {
            let b = part.as_bytes();
            if p.len + b.len() >= p.buf.len() || b.contains(&0) {
                return None;
            }
            p.buf[p.len..p.len + b.len()].copy_from_slice(b);
            p.len += b.len();
        }
        p.buf[p.len] = 0;
        Some(p)
    }

    pub fn ptr(&self) -> *const u8 {
        self.buf.as_ptr()
    }

    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.buf[..self.len]).unwrap_or("?")
    }
}

/// `sceIoRemove`'s result.
pub fn remove(path: &CPath) -> i32 {
    // SAFETY: NUL-terminated path.
    unsafe { sys::sceIoRemove(path.ptr()) }
}

/// `sceIoRename`'s result: `to` in `from`'s directory (the PSP ignores
/// `to`'s directory; both name the same one here).
pub fn rename(from: &CPath, to: &CPath) -> i32 {
    // SAFETY: NUL-terminated paths.
    unsafe { sys::sceIoRename(from.ptr(), to.ptr()) }
}

/// The file's length, or the open error.
pub fn file_len(path: &CPath) -> Result<u64, i32> {
    // SAFETY: NUL-terminated path.
    let fd = unsafe { sys::sceIoOpen(path.ptr(), sys::IoOpenFlags::RD_ONLY, 0) };
    if fd.0 < 0 {
        return Err(fd.0);
    }
    // SAFETY: an open descriptor, closed once.
    let len = unsafe {
        let len = sys::sceIoLseek(fd, 0, sys::IoWhence::End);
        sys::sceIoClose(fd);
        len
    };
    if len < 0 {
        Err(len as i32)
    } else {
        Ok(len as u64)
    }
}

/// The first `out.len()` bytes of `path` (fewer at its end), or the error.
pub fn read_head(path: &CPath, out: &mut [u8]) -> Result<usize, i32> {
    // SAFETY: NUL-terminated path.
    let fd = unsafe { sys::sceIoOpen(path.ptr(), sys::IoOpenFlags::RD_ONLY, 0) };
    if fd.0 < 0 {
        return Err(fd.0);
    }
    // SAFETY: an open descriptor, closed once; `out` outlives the read.
    let n = unsafe {
        let n = sys::sceIoRead(fd, out.as_mut_ptr() as *mut c_void, out.len() as u32);
        sys::sceIoClose(fd);
        n
    };
    if n < 0 {
        Err(n)
    } else {
        Ok(n as usize)
    }
}

/// The files (not directories) in `root` (a search root: "" is the EBOOT's
/// directory), each passed to `each`; `false` when the directory does not
/// open.
pub fn list_files(root: &str, mut each: impl FnMut(&str)) -> bool {
    // The relative root: "." first, then "" (which the PSP's and PPSSPP's
    // path resolution may each want; unverified on hardware).
    let dirs: &[&str] = if root.is_empty() { &[".", ""] } else { &[root] };
    for dir in dirs {
        let Some(path) = CPath::new(&[dir]) else {
            continue;
        };
        // SAFETY: NUL-terminated path.
        let fd = unsafe { sys::sceIoDopen(path.ptr()) };
        if fd.0 < 0 {
            continue;
        }
        loop {
            // SAFETY: zeroed is a valid dirent (null `d_private`: no
            // long-name buffer); the PSP needs it cleared before each read.
            let mut entry: sys::SceIoDirent = unsafe { core::mem::zeroed() };
            // SAFETY: an open directory; `entry` outlives the call.
            if unsafe { sys::sceIoDread(fd, &mut entry) } <= 0 {
                break;
            }
            // Directories out; anything else (a file, a link) is a file
            // to try.
            if entry.d_stat.st_mode.contains(sys::IoStatMode::IFDIR) {
                continue;
            }
            let n = entry
                .d_name
                .iter()
                .position(|&b| b == 0)
                .unwrap_or(entry.d_name.len());
            if let Ok(name) = core::str::from_utf8(&entry.d_name[..n]) {
                each(name);
            }
        }
        // SAFETY: the open directory, closed once.
        unsafe { sys::sceIoDclose(fd) };
        return true;
    }
    false
}

/// `sceIoDevctl(0x02425818)` on the root's device: its free bytes, or the
/// error. A relative root is taken to be on `ms0:`.
pub fn free_bytes(root: &str) -> Result<u64, i32> {
    let dev: &[u8] = if root.starts_with("ef0:") {
        b"ef0:\0"
    } else {
        b"ms0:\0"
    };
    let mut info = ssb_pack::install::DevInfo::default();
    let mut at: *mut ssb_pack::install::DevInfo = &mut info;
    // SAFETY: NUL-terminated device; the input is a pointer to `info`
    // (pspsdk's form of this command), which outlives the call.
    let r = unsafe {
        sys::sceIoDevctl(
            dev.as_ptr(),
            0x0242_5818,
            (&mut at as *mut *mut ssb_pack::install::DevInfo).cast::<c_void>(),
            4,
            core::ptr::null_mut(),
            0,
        )
    };
    if r < 0 {
        Err(r)
    } else {
        Ok(info.free_bytes())
    }
}

/// Opens `path` with `flags`; records the descriptor in slot `slot`.
fn open(path: &CPath, flags: sys::IoOpenFlags, slot: usize) -> Result<sys::SceUid, i32> {
    // SAFETY: NUL-terminated path.
    let fd = unsafe { sys::sceIoOpen(path.ptr(), flags, 0o777) };
    if fd.0 < 0 {
        return Err(fd.0);
    }
    OPEN[slot].store(fd.0, Ordering::Relaxed);
    Ok(fd)
}

/// Closes the descriptor in `slot`, if open.
fn close(slot: usize) {
    let fd = OPEN[slot].swap(-1, Ordering::Relaxed);
    if fd >= 0 {
        // SAFETY: an open descriptor, closed once (the swap).
        unsafe { sys::sceIoClose(sys::SceUid(fd)) };
    }
}

/// Closes every descriptor the build opened (after an error, a panic or
/// an allocation failure).
pub fn close_all() {
    for slot in 0..OPEN.len() {
        close(slot);
    }
}

/// `out.len()` bytes or fewer at `at`; the error code on failure.
fn seek_read(fd: sys::SceUid, at: u64, out: &mut [u8]) -> Result<usize, i32> {
    // SAFETY: an open descriptor.
    let pos = unsafe { sys::sceIoLseek(fd, at as i64, sys::IoWhence::Set) };
    if pos != at as i64 {
        return Err(if pos < 0 { pos as i32 } else { -1 });
    }
    // SAFETY: as above; `out` outlives the call.
    let n = unsafe { sys::sceIoRead(fd, out.as_mut_ptr() as *mut c_void, out.len() as u32) };
    if n < 0 {
        return Err(n);
    }
    Ok(n as usize)
}

/// The ROM file, read by `ssb_rom::rom_read::PagedRom`.
pub struct RomFile {
    fd: sys::SceUid,
}

impl RomFile {
    /// Opens the ROM and returns it with its length.
    pub fn open(path: &CPath) -> Result<(Self, u64), i32> {
        let fd = open(path, sys::IoOpenFlags::RD_ONLY, OPEN_ROM)?;
        // SAFETY: an open descriptor.
        let len = unsafe { sys::sceIoLseek(fd, 0, sys::IoWhence::End) };
        if len < 0 {
            close(OPEN_ROM);
            return Err(len as i32);
        }
        Ok((Self { fd }, len as u64))
    }

    /// Closes the ROM.
    pub fn close(self) {
        close(OPEN_ROM);
    }
}

impl ssb_rom::rom_read::RawFile for RomFile {
    fn read_raw(&mut self, at: u64, out: &mut [u8]) -> Result<usize, i32> {
        let t = now();
        let n = seek_read(self.fd, at, out);
        ROM_US.fetch_add(now().wrapping_sub(t), Ordering::Relaxed);
        n
    }
}

/// The blob's spill file (`ssb_rom::blob_store::BlobSpill`). The build
/// owns it and drops it unclosed: [`close_spill`] closes it.
pub struct IoSpill {
    fd: sys::SceUid,
    len: u64,
}

impl IoSpill {
    /// Creates `path`, empty.
    pub fn create(path: &CPath) -> Result<Self, i32> {
        let flags = sys::IoOpenFlags::RD_WR | sys::IoOpenFlags::CREAT | sys::IoOpenFlags::TRUNC;
        let fd = open(path, flags, OPEN_SPILL)?;
        Ok(Self { fd, len: 0 })
    }
}

/// Records the first failed write.
fn write_failed(what: u8, code: i32) {
    if WRITE_WHAT.load(Ordering::Relaxed) == 0 {
        WRITE_CODE.store(code, Ordering::Relaxed);
        WRITE_WHAT.store(what, Ordering::Relaxed);
    }
}

/// Writes all of `bytes` to `fd`; the failing call's code.
fn write_all(fd: sys::SceUid, mut bytes: &[u8]) -> Result<u32, i32> {
    let mut calls = 0;
    while !bytes.is_empty() {
        // SAFETY: an open descriptor; `bytes` outlives the call.
        let n = unsafe { sys::sceIoWrite(fd, bytes.as_ptr() as *const c_void, bytes.len()) };
        calls += 1;
        if n <= 0 {
            // A write of nothing: the stick is full.
            return Err(if n == 0 {
                ssb_pack::install::ERRNO_ENOSPC
            } else {
                n
            });
        }
        bytes = &bytes[n as usize..];
    }
    Ok(calls)
}

impl ssb_rom::blob_store::BlobSpill for IoSpill {
    fn append(&mut self, bytes: &[u8]) -> Result<u64, ssb_rom::pack::SinkError> {
        let t = now();
        let at = self.len;
        // SAFETY: an open descriptor.
        let pos = unsafe { sys::sceIoLseek(self.fd, at as i64, sys::IoWhence::Set) };
        let result = if pos != at as i64 {
            let code = if pos < 0 { pos as i32 } else { -1 };
            write_failed(WRITE_SPILL, code);
            Err(alloc::format!("spill seek {code:#x}").into())
        } else {
            match write_all(self.fd, bytes) {
                Ok(_) => {
                    self.len += bytes.len() as u64;
                    Ok(at)
                }
                Err(code) => {
                    write_failed(WRITE_SPILL, code);
                    Err(alloc::format!("spill write {code:#x}").into())
                }
            }
        };
        SPILL_US.fetch_add(now().wrapping_sub(t), Ordering::Relaxed);
        result
    }

    fn read_at(&mut self, at: u64, out: &mut [u8]) -> Result<(), ssb_rom::pack::SinkError> {
        let t = now();
        let mut done = 0;
        let mut result = Ok(());
        while done < out.len() {
            match seek_read(self.fd, at + done as u64, &mut out[done..]) {
                Ok(0) => {
                    result = Err(alloc::format!("spill short read at {}", at + done as u64).into());
                    break;
                }
                Ok(n) => done += n,
                Err(code) => {
                    result = Err(alloc::format!("spill read {code:#x}").into());
                    break;
                }
            }
        }
        SPILL_US.fetch_add(now().wrapping_sub(t), Ordering::Relaxed);
        result
    }
}

/// Closes the spill.
pub fn close_spill() {
    close(OPEN_SPILL);
}

/// The pack's write buffer, cache-line aligned: the Memory Stick driver
/// writes an unaligned buffer about 7x slower (installer run: 129 s for
/// the pack from a byte-aligned `Vec`, against 18 s from the probe's
/// 64-byte-aligned buffer, RE-588).
#[repr(C, align(64))]
struct SinkBuf([u8; SINK_BYTES]);

/// The pack's temporary file through a heap buffer.
pub struct FileSink {
    fd: sys::SceUid,
    /// Taken at the first write; `None` while the heap has no room.
    buf: Option<alloc::boxed::Box<SinkBuf>>,
    len: usize,
}

impl FileSink {
    /// Creates `path`, empty.
    pub fn create(path: &CPath) -> Result<Self, i32> {
        let flags = sys::IoOpenFlags::WR_ONLY | sys::IoOpenFlags::CREAT | sys::IoOpenFlags::TRUNC;
        let fd = open(path, flags, OPEN_PACK)?;
        Ok(Self {
            fd,
            buf: None,
            len: 0,
        })
    }

    /// The buffer, zeroed, or `None` when the heap has no room for it.
    fn take_buf() -> Option<alloc::boxed::Box<SinkBuf>> {
        let layout = core::alloc::Layout::new::<SinkBuf>();
        // SAFETY: `layout` has a nonzero size; a non-null result is a fresh
        // zeroed allocation of `SinkBuf`'s layout, owned by the `Box`.
        unsafe {
            let p = alloc::alloc::alloc_zeroed(layout) as *mut SinkBuf;
            (!p.is_null()).then(|| alloc::boxed::Box::from_raw(p))
        }
    }

    /// Writes `bytes` now, timed.
    fn put(fd: sys::SceUid, bytes: &[u8]) -> Result<(), i32> {
        let t = now();
        let r = write_all(fd, bytes);
        SINK_US.fetch_add(now().wrapping_sub(t), Ordering::Relaxed);
        match r {
            Ok(calls) => {
                SINK_CALLS.fetch_add(calls, Ordering::Relaxed);
                SINK_WRITTEN.fetch_add(bytes.len(), Ordering::Relaxed);
                Ok(())
            }
            Err(code) => {
                write_failed(WRITE_PACK, code);
                Err(code)
            }
        }
    }

    /// Writes what the buffer holds.
    pub fn flush(&mut self) -> Result<(), i32> {
        let len = core::mem::take(&mut self.len);
        match &self.buf {
            Some(buf) if len > 0 => Self::put(self.fd, &buf.0[..len]),
            _ => Ok(()),
        }
    }

    /// Closes the file (after [`FileSink::flush`]).
    pub fn close(self) {
        let t = now();
        close(OPEN_PACK);
        SINK_US.fetch_add(now().wrapping_sub(t), Ordering::Relaxed);
    }
}

impl ssb_rom::pack::PackSink for FileSink {
    fn write(&mut self, bytes: &[u8]) -> Result<(), ssb_rom::pack::SinkError> {
        if self.buf.is_none() {
            // Unbuffered when the heap has no room: slower, same bytes.
            self.buf = Self::take_buf();
        }
        let room = if self.buf.is_some() { SINK_BYTES } else { 0 };
        if self.len + bytes.len() > room {
            self.flush()
                .map_err(|code| alloc::format!("pack write {code:#x}"))?;
        }
        match &mut self.buf {
            Some(buf) if bytes.len() < room => {
                buf.0[self.len..self.len + bytes.len()].copy_from_slice(bytes);
                self.len += bytes.len();
            }
            _ => {
                Self::put(self.fd, bytes).map_err(|code| alloc::format!("pack write {code:#x}"))?
            }
        }
        Ok(())
    }
}

/// One line appended to the log (open, write, close: kept on a crash).
/// Formats into a fixed buffer, so it allocates nothing; a longer line is
/// cut.
pub fn log(args: fmt::Arguments<'_>) {
    let t = now();
    let mut line = Line {
        buf: [0; 512],
        len: 0,
    };
    let _ = line.write_fmt(args);
    let _ = line.write_str("\n");
    if line.len == line.buf.len() {
        line.buf[line.len - 1] = b'\n';
    }
    // SAFETY: NUL-terminated path; the buffer outlives the write.
    unsafe {
        let fd = sys::sceIoOpen(
            LOG.as_ptr(),
            sys::IoOpenFlags::WR_ONLY | sys::IoOpenFlags::CREAT | sys::IoOpenFlags::APPEND,
            0o777,
        );
        if fd.0 >= 0 {
            sys::sceIoWrite(fd, line.buf.as_ptr() as *const c_void, line.len);
            sys::sceIoClose(fd);
        }
    }
    LOG_US.fetch_add(now().wrapping_sub(t), Ordering::Relaxed);
}

/// A fixed line buffer; bytes past its end are dropped.
pub struct Line {
    pub buf: [u8; 512],
    pub len: usize,
}

impl Write for Line {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let n = s.len().min(self.buf.len() - self.len);
        self.buf[self.len..self.len + n].copy_from_slice(&s.as_bytes()[..n]);
        self.len += n;
        Ok(())
    }
}
