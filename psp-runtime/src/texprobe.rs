//! Where the GE's texel traffic comes from (`texture_probe`, RE-490).
//!
//! Each textured draw belongs to a [`Cat`], set by the draw entry points
//! around it. `texoff.txt` beside the executable switches texturing off by
//! category (`cat HEX`, a bit per [`Cat`]) or for single textures (`tex HEX`,
//! a `TextureDesc::data_offset`), one directive a line; `keep` inverts the
//! texture list, so only the listed textures stay textured. The census
//! writes every texture bound over frames [`CENSUS_FIRST`]..[`CENSUS_LAST`]
//! to `texprobe.log`: key, category, size, format and binds.
//!
//! A diagnostic only: single-threaded, fixed tables, nothing kept between
//! runs.

use core::ffi::c_void;
use psp::sys;

/// What a bound texture is drawn for.
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Cat {
    /// Items, weapons, effects, particles, shadows, the interface.
    Other = 0,
    /// The stage's layers, platforms and objects.
    Stage = 1,
    /// Fighter models, the magnifiers' included.
    Fighter = 2,
    /// The wallpaper sprite.
    Wallpaper = 3,
}

const CENSUS_FIRST: u32 = 1500;
const CENSUS_LAST: u32 = 1559;
const MAX_KEYS: usize = 64;
const MAX_TEXTURES: usize = 512;

#[derive(Clone, Copy)]
struct Entry {
    key: u32,
    cat: u32,
    dims: u32,
    psm: u32,
    bytes: u32,
    binds: u32,
}

struct State {
    loaded: bool,
    cat: Cat,
    cat_off: u32,
    keys: [u32; MAX_KEYS],
    key_count: usize,
    keep: bool,
    frame: u32,
    census: [Entry; MAX_TEXTURES],
    census_len: usize,
    written: bool,
}

static mut STATE: State = State {
    loaded: false,
    cat: Cat::Other,
    cat_off: 0,
    keys: [0; MAX_KEYS],
    key_count: 0,
    keep: false,
    frame: 0,
    census: [Entry { key: 0, cat: 0, dims: 0, psm: 0, bytes: 0, binds: 0 }; MAX_TEXTURES],
    census_len: 0,
    written: false,
};

fn state() -> &'static mut State {
    // SAFETY: the game is single-threaded.
    let s = unsafe { &mut *core::ptr::addr_of_mut!(STATE) };
    if !s.loaded {
        s.loaded = true;
        load(s);
    }
    s
}

fn load(s: &mut State) {
    let mut buf = [0u8; 1024];
    // SAFETY: plain file reads into a local buffer.
    let n = unsafe {
        let fd = sys::sceIoOpen(b"texoff.txt\0".as_ptr(), sys::IoOpenFlags::RD_ONLY, 0);
        if fd.0 < 0 {
            return;
        }
        let n = sys::sceIoRead(fd, buf.as_mut_ptr() as *mut c_void, buf.len() as u32);
        sys::sceIoClose(fd);
        n
    };
    let Ok(n) = usize::try_from(n) else { return };
    let text = core::str::from_utf8(&buf[..n.min(buf.len())]).unwrap_or("");
    for line in text.lines() {
        let mut words = line.split_whitespace();
        let (Some(word), value) = (words.next(), words.next()) else { continue };
        let value = value.and_then(|v| u32::from_str_radix(v.trim_start_matches("0x"), 16).ok());
        match (word, value) {
            ("cat", Some(v)) => s.cat_off = v,
            ("tex", Some(v)) if s.key_count < MAX_KEYS => {
                s.keys[s.key_count] = v;
                s.key_count += 1;
            }
            ("keep", _) => s.keep = true,
            _ => {}
        }
    }
}

/// Sets the category of the draws that follow; returns the previous one.
pub fn set_cat(cat: Cat) -> Cat {
    let s = state();
    core::mem::replace(&mut s.cat, cat)
}

/// Whether the texture at `key` draws untextured, and records the bind.
pub fn bind(key: u32, width: u16, height: u16, psm: u8, bytes: usize) -> bool {
    let s = state();
    if (CENSUS_FIRST..=CENSUS_LAST).contains(&s.frame) {
        let cat = s.cat as u32;
        match s.census[..s.census_len].iter_mut().find(|e| e.key == key && e.cat == cat) {
            Some(e) => e.binds += 1,
            None if s.census_len < MAX_TEXTURES => {
                s.census[s.census_len] = Entry {
                    key,
                    cat,
                    dims: u32::from(width) << 16 | u32::from(height),
                    psm: u32::from(psm),
                    bytes: bytes as u32,
                    binds: 1,
                };
                s.census_len += 1;
            }
            None => {}
        }
    }
    if s.cat_off & 1 << s.cat as u32 != 0 {
        return true;
    }
    let listed = s.keys[..s.key_count].contains(&key);
    if s.keep { s.key_count != 0 && !listed } else { listed }
}

/// Counts a frame; after the census window, writes `texprobe.log` once.
pub fn frame() {
    let s = state();
    s.frame += 1;
    if s.frame > CENSUS_LAST && !s.written {
        s.written = true;
        write(s);
    }
}

fn write(s: &State) {
    let mut out = alloc::string::String::new();
    use core::fmt::Write;
    let frames = CENSUS_LAST - CENSUS_FIRST + 1;
    let _ = writeln!(out, "# frames {frames}; key cat w h psm bytes binds/frame");
    for e in &s.census[..s.census_len] {
        let _ = writeln!(
            out,
            "{:08x} {} {} {} {} {} {:.2}",
            e.key,
            e.cat,
            e.dims >> 16,
            e.dims & 0xFFFF,
            e.psm,
            e.bytes,
            e.binds as f32 / frames as f32,
        );
    }
    // SAFETY: plain file writes from a local buffer.
    unsafe {
        let fd = sys::sceIoOpen(
            b"texprobe.log\0".as_ptr(),
            sys::IoOpenFlags::WR_ONLY | sys::IoOpenFlags::CREAT | sys::IoOpenFlags::TRUNC,
            0o777,
        );
        if fd.0 >= 0 {
            sys::sceIoWrite(fd, out.as_ptr() as *const c_void, out.len());
            sys::sceIoClose(fd);
        }
    }
}
