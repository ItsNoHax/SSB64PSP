//! The ROM-derived tables, read at run time from the pack's `TABLES`
//! section instead of compiled into the binary (D-048).
//!
//! - Layout (ids, element kinds, format) is `ssb_tablegen::tables`'s,
//!   written by `build.rs`; this crate holds no ROM data.
//! - A runtime loads the section once at boot and calls [`install`]
//!   (`psp_runtime::tables::load`; `romtool pack` installs the section it
//!   builds).
//! - A consumer declares `static NAME: Table<[T; N]> = Table::new(id)`;
//!   `NAME[i]`, `NAME.len()` and `NAME.iter()` read it through `Deref`.
//!   Kind and size are layout (`TableId::kind`, `TableId::size`), checked
//!   at compile time by `Table::new` and once by [`install`], which points
//!   every table at its bytes (in place when aligned).
//! - Feature `eager` (the PSP, which installs at boot): every table has a
//!   fixed place in one `.bss` store (`TableId::offset`) that [`install`]
//!   fills, so a read is a constant address, as a `.rodata` table was.
//!   [`install`] is then `unsafe`: no table may be read during it.
//! - Otherwise a read checks the install, then loads the table's pointer.
//! - A table the section lacks reads as zeros: the empty section, or no
//!   section installed, is the stub (`SSB64_STUB_TABLES=1`).
//! - Feature `host-rom` (host tests and tools): the first read before
//!   [`install`] builds the section from the user's ROM
//!   (`ssb_tablegen::test_rom`), or installs the empty section under
//!   `cfg(ssb64_stub_tables)`.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::boxed::Box;
#[cfg(feature = "eager")]
use core::cell::UnsafeCell;
use core::marker::PhantomData;
use core::ops::Deref;
use core::ptr;
use core::sync::atomic::{AtomicPtr, AtomicU8, Ordering};

#[cfg(target_endian = "big")]
compile_error!("the TABLES section is little-endian and read in place");

include!(concat!(env!("OUT_DIR"), "/table_ids.rs"));

#[cfg(all(feature = "eager", feature = "host-rom"))]
compile_error!(
    "`eager` reads no fallback: it is for runtimes that install at boot, never with `host-rom`"
);

mod reader;
pub use reader::{Section, SectionError};

/// Every table at `TableId::offset`, zeros until [`install`] copies the
/// section's tables in (`eager`); a table the section lacks stays zeros.
#[cfg(feature = "eager")]
#[repr(C, align(4))]
struct Store(UnsafeCell<[u8; STORE_SIZE]>);

// SAFETY: only `install` writes the store, and its contract excludes any
// concurrent read.
#[cfg(feature = "eager")]
unsafe impl Sync for Store {}

#[cfg(feature = "eager")]
static STORE: Store = Store(UnsafeCell::new([0; STORE_SIZE]));

/// Zeros, as large as the largest table and 4-byte aligned: every table
/// before [`install`] (the stub), and a table the section lacks.
#[cfg(not(feature = "eager"))]
#[repr(C, align(4))]
struct Zeros([u8; MAX_SIZE]);

#[cfg(not(feature = "eager"))]
static ZEROS: Zeros = Zeros([0; MAX_SIZE]);

/// Each table's bytes, by `TableId`: [`ZEROS`] until [`install`].
#[cfg(not(feature = "eager"))]
static TABLES: [AtomicPtr<u8>; TableId::ALL.len()] =
    [const { AtomicPtr::new(ptr::addr_of!(ZEROS) as *mut u8) }; TableId::ALL.len()];

/// [`install`]'s progress: [`NONE`], [`INSTALLING`] or [`READY`].
static STATE: AtomicU8 = AtomicU8::new(NONE);
const NONE: u8 = 0;
const INSTALLING: u8 = 1;
const READY: u8 = 2;

/// The installed section, leaked.
static INSTALLED: AtomicPtr<Section<'static>> = AtomicPtr::new(ptr::null_mut());

/// Why [`install`] refused a section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallError {
    Section(SectionError),
    /// The section's table has another kind or size than this build's
    /// layout (a pack this build does not read).
    Table(TableId),
    /// A section is already installed (or a table was read before
    /// `install` and the `host-rom` fallback installed one).
    AlreadyInstalled,
}

impl core::fmt::Display for InstallError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            InstallError::Section(e) => e.fmt(f),
            InstallError::Table(id) => write!(
                f,
                "table {id:?} is not {:?}[{}] as this build reads it",
                id.kind(),
                id.size()
            ),
            InstallError::AlreadyInstalled => write!(f, "a tables section is already installed"),
        }
    }
}

/// Installs the section every [`Table`] reads, once, before the first
/// table read. `bytes` lives for the rest of the program; a table is read
/// in place when 4-byte aligned, else from a leaked copy. Refuses a section
/// whose tables differ in kind or size from this build's layout.
#[cfg(not(feature = "eager"))]
pub fn install(bytes: &'static [u8]) -> Result<Section<'static>, InstallError> {
    install_with(bytes, |tables| {
        for ((slot, id), t) in TABLES.iter().zip(TableId::ALL).zip(tables) {
            if let Some(t) = t {
                slot.store(placed(id, t) as *mut u8, Ordering::Relaxed);
            }
        }
    })
}

/// Installs the section every [`Table`] reads, once: copies each table into
/// its place in the store. `bytes` stays live ([`installed`]). Refuses a
/// section whose tables differ in kind or size from this build's layout.
///
/// # Safety
///
/// No [`Table`] is read during the call, on any thread: call it at boot,
/// before the first read.
#[cfg(feature = "eager")]
pub unsafe fn install(bytes: &'static [u8]) -> Result<Section<'static>, InstallError> {
    install_with(bytes, |tables| {
        let store = STORE.0.get().cast::<u8>();
        for (id, t) in TableId::ALL.into_iter().zip(tables) {
            if let Some(t) = t {
                // SAFETY: the table fits its place (`id.size()`, checked by
                // `checked`); the caller excludes concurrent reads.
                unsafe { ptr::copy_nonoverlapping(t.as_ptr(), store.add(id.offset()), t.len()) };
            }
        }
    })
}

/// [`install`]'s checks and state; `place` publishes the checked tables
/// (`None`: the section lacks it), before `READY`.
fn install_with(
    bytes: &'static [u8],
    place: impl FnOnce(&[Option<&'static [u8]>; TableId::ALL.len()]),
) -> Result<Section<'static>, InstallError> {
    let section = Section::parse(bytes).map_err(InstallError::Section)?;
    let mut tables = [None; TableId::ALL.len()];
    for (t, id) in tables.iter_mut().zip(TableId::ALL) {
        *t = checked(id, &section)?;
    }
    if STATE
        .compare_exchange(NONE, INSTALLING, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {
        return Err(InstallError::AlreadyInstalled);
    }
    place(&tables);
    INSTALLED.store(Box::into_raw(Box::new(section)), Ordering::Relaxed);
    STATE.store(READY, Ordering::Release);
    Ok(section)
}

/// Table `id`'s bytes in `section`, checked against the layout; `None`
/// when the section lacks it.
fn checked(id: TableId, section: &Section<'static>) -> Result<Option<&'static [u8]>, InstallError> {
    let Some((kind, bytes)) = section.get(id) else {
        return Ok(None);
    };
    if kind != id.kind() || bytes.len() != id.size() {
        return Err(InstallError::Table(id));
    }
    Ok(Some(bytes))
}

/// The installed section, if any.
pub fn installed() -> Option<Section<'static>> {
    if STATE.load(Ordering::Acquire) != READY {
        return None;
    }
    let p = INSTALLED.load(Ordering::Relaxed);
    // SAFETY: `READY` follows the store of a leaked `Section` (`install`).
    Some(unsafe { *p })
}

/// A checked table's bytes, 4-byte aligned: in place when aligned, else a
/// leaked copy.
#[cfg(not(feature = "eager"))]
fn placed(id: TableId, bytes: &'static [u8]) -> *const u8 {
    if bytes.as_ptr().align_offset(4) == 0 || bytes.is_empty() {
        return bytes.as_ptr();
    }
    let layout = core::alloc::Layout::from_size_align(id.size(), 4).expect("table layout");
    // SAFETY: the layout is not zero-sized (`bytes` is not empty).
    let p = unsafe { alloc::alloc::alloc(layout) };
    if p.is_null() {
        alloc::alloc::handle_alloc_error(layout);
    }
    // SAFETY: `p` holds `bytes.len()` bytes; the copy is leaked.
    unsafe { ptr::copy_nonoverlapping(bytes.as_ptr(), p, bytes.len()) };
    p
}

/// Table `id` of `section` as a read sees it after [`install`]: checked,
/// placed, or [`ZEROS`] when the section lacks it.
#[cfg(all(test, not(feature = "eager")))]
fn table_ptr(id: TableId, section: &Section<'static>) -> Result<*const u8, InstallError> {
    Ok(checked(id, section)?.map_or(ptr::addr_of!(ZEROS) as *const u8, |t| placed(id, t)))
}

/// Before a checked read: installs the `host-rom` section if none is, or
/// waits out an install in progress. Without `host-rom`, an uninstalled
/// section reads zeros.
#[cfg(not(feature = "eager"))]
#[cold]
#[inline(never)]
fn ensure() {
    #[cfg(feature = "host-rom")]
    if STATE.load(Ordering::Acquire) == NONE {
        let _ = install(fallback());
    }
    while STATE.load(Ordering::Acquire) == INSTALLING {
        core::hint::spin_loop();
    }
}

/// The host fallback: the ROM's section (the empty one under
/// `cfg(ssb64_stub_tables)`), built and leaked.
#[cfg(all(feature = "host-rom", not(feature = "eager")))]
fn fallback() -> &'static [u8] {
    let stub = cfg!(ssb64_stub_tables);
    let source = (!stub).then(|| {
        let path = ssb_tablegen::test_rom(env!("CARGO_MANIFEST_DIR")).unwrap_or_else(|| {
            panic!(
                "ssb-tables: a table was read with no section installed and no ROM found.\n\
                 Put the ROM in rom/ or set {}=/path/to/rom.z64; CI sets {}=1.",
                ssb_tablegen::ROM_ENV,
                ssb_tablegen::STUB_ENV
            )
        });
        ssb_tablegen::Source::open(&path)
            .unwrap_or_else(|e| panic!("ssb-tables: {}: {e}", path.display()))
    });
    let bytes = ssb_tablegen::tables_section(source.as_ref())
        .unwrap_or_else(|e| panic!("ssb-tables: reading the ROM's tables failed: {e}"));
    Box::leak(bytes.into_boxed_slice())
}

/// A type every bit pattern of which is a value, with no padding: a table's
/// element type or a fixed array of one.
///
/// # Safety
///
/// `Self` must be valid for every bit pattern (zeros included), have no
/// padding and no pointers, and be laid out as `KIND` elements.
pub unsafe trait Pod: Copy + Sync + 'static {
    /// The element kind of the section's table.
    const KIND: Kind;
}

macro_rules! pod {
    ($($t:ty => $k:ident),*) => {
        $(
            // SAFETY: a primitive integer or float.
            unsafe impl Pod for $t {
                const KIND: Kind = Kind::$k;
            }
        )*
    };
}
pod!(u8 => U8, u16 => U16, u32 => U32, i16 => I16, i32 => I32, f32 => F32);

// SAFETY: an array of `Pod` elements has no padding between them.
unsafe impl<T: Pod, const N: usize> Pod for [T; N] {
    const KIND: Kind = T::KIND;
}

/// One table of the section, read as `T`.
pub struct Table<T: Pod> {
    id: TableId,
    _t: PhantomData<T>,
}

impl<T: Pod> Table<T> {
    /// Table `id`; `T` must be its kind and size (checked at compile time).
    pub const fn new(id: TableId) -> Self {
        assert!(
            T::KIND as u16 == id.kind() as u16,
            "the table's element type is not its kind"
        );
        assert!(
            core::mem::size_of::<T>() == id.size(),
            "the table's type is not its size"
        );
        assert!(core::mem::align_of::<T>() <= 4, "a table is 4-byte aligned");
        Self {
            id,
            _t: PhantomData,
        }
    }

    /// The table: its place in the store, a constant address (`eager`).
    #[cfg(feature = "eager")]
    #[inline(always)]
    pub fn get(&self) -> &T {
        let p = STORE.0.get().cast::<u8>().wrapping_add(self.id.offset());
        // SAFETY: the place is `id.size()` bytes of the store, 4-byte
        // aligned (`offset`), written only by `install`, which excludes
        // reads; `T` is `Pod` of that size and alignment (`new`).
        unsafe { &*(p as *const T) }
    }

    /// The table: installs the `host-rom` section first if none is.
    #[cfg(not(feature = "eager"))]
    #[inline]
    pub fn get(&self) -> &T {
        if STATE.load(Ordering::Acquire) != READY {
            ensure();
        }
        let p = TABLES[self.id as usize].load(Ordering::Relaxed);
        // SAFETY: the pointer is always `id.size()` live bytes, 4-byte
        // aligned (`ZEROS`, or a table `install` checked against the
        // layout), and `T` is `Pod` of that size and alignment (`new`);
        // `READY` (or no install at all) orders this load after
        // `install`'s stores.
        unsafe { &*(p as *const T) }
    }
}

impl<T: Pod> Deref for Table<T> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &T {
        self.get()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    #[cfg(not(feature = "eager"))]
    /// Table `id` of `s` as `T`, through [`table_ptr`].
    fn read<T: Pod>(id: TableId, s: &Section<'static>) -> &'static T {
        // SAFETY: `table_ptr` checked the table against the layout and
        // returns live, 4-byte aligned bytes; `T` is that layout's type.
        unsafe { &*(table_ptr(id, s).unwrap() as *const T) }
    }

    fn leak(v: Vec<u8>) -> &'static [u8] {
        Box::leak(v.into_boxed_slice())
    }

    /// A section with `tables` (id, kind, bytes), laid out as the writer
    /// does.
    fn section_bytes(tables: &[(TableId, Kind, &[u8])]) -> Vec<u8> {
        let mut out = Vec::new();
        for v in [MAGIC, SECTION_VERSION, tables.len() as u32, 0] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        let mut at = HEADER_SIZE + tables.len() * ENTRY_SIZE;
        let mut offsets = Vec::new();
        for t in tables {
            at = at.next_multiple_of(4);
            offsets.push(at);
            at += t.2.len();
        }
        for (t, &o) in tables.iter().zip(&offsets) {
            out.extend_from_slice(&(t.0 as u16).to_le_bytes());
            out.extend_from_slice(&(t.1 as u16).to_le_bytes());
            out.extend_from_slice(&(o as u32).to_le_bytes());
            out.extend_from_slice(&(t.2.len() as u32).to_le_bytes());
        }
        for (t, &o) in tables.iter().zip(&offsets) {
            out.resize(o, 0);
            out.extend_from_slice(t.2);
        }
        out
    }

    #[test]
    #[cfg(not(feature = "eager"))]
    fn the_writers_empty_section_reads_zeros() {
        let bytes = leak(ssb_tablegen::tables_section(None).unwrap());
        let s = Section::parse(bytes).unwrap();
        assert!(s.is_empty());
        assert_eq!(s.get(TableId::SinTable), None);
        assert!(read::<[u16; 2048]>(TableId::SinTable, &s)
            .iter()
            .all(|&v| v == 0));
        assert!(read::<[f32; 1024]>(TableId::LbCommonSinLookup, &s)
            .iter()
            .all(|&v| v == 0.0));
    }

    #[test]
    #[cfg(not(feature = "eager"))]
    fn a_table_round_trips_in_place_and_misaligned() {
        let words: Vec<u8> = (0..2048u16).flat_map(|v| (v * 3).to_le_bytes()).collect();
        let bytes = section_bytes(&[(TableId::SinTable, Kind::U16, &words)]);
        let aligned = leak(bytes.clone());
        let s = Section::parse(aligned).unwrap();
        let p = table_ptr(TableId::SinTable, &s).unwrap();
        assert_eq!(p, s.get(TableId::SinTable).unwrap().1.as_ptr());
        assert_eq!(read::<[u16; 2048]>(TableId::SinTable, &s)[1000], 3000);
        // One byte in: the table is copied.
        let mut shifted = Vec::with_capacity(bytes.len() + 1);
        shifted.push(0);
        shifted.extend_from_slice(&bytes);
        let shifted = &leak(shifted)[1..];
        let s = Section::parse(shifted).unwrap();
        let p = table_ptr(TableId::SinTable, &s).unwrap();
        assert_ne!(p, s.get(TableId::SinTable).unwrap().1.as_ptr());
        let t = read::<[u16; 2048]>(TableId::SinTable, &s);
        assert_eq!((t[0], t[1], t[2047]), (0, 3, 2047 * 3));
    }

    #[test]
    #[cfg(not(feature = "eager"))]
    fn a_table_of_another_kind_or_size_is_refused() {
        let refused = Err(InstallError::Table(TableId::SinTable));
        let bytes = leak(section_bytes(&[(TableId::SinTable, Kind::U16, &[0; 8])]));
        assert_eq!(
            table_ptr(TableId::SinTable, &Section::parse(bytes).unwrap()),
            refused
        );
        let bytes = leak(section_bytes(&[(TableId::SinTable, Kind::I16, &[0; 4096])]));
        assert_eq!(
            table_ptr(TableId::SinTable, &Section::parse(bytes).unwrap()),
            refused
        );
    }

    #[test]
    fn malformed_sections_are_refused() {
        assert_eq!(Section::parse(&[0; 8]).err(), Some(SectionError::TooSmall));
        let mut b = section_bytes(&[(TableId::SinTable, Kind::U16, &[0; 4])]);
        let good = b.clone();
        b[0] = b'X';
        assert!(matches!(Section::parse(&b), Err(SectionError::BadMagic(_))));
        let mut b = good.clone();
        b[4] = 9;
        assert_eq!(Section::parse(&b).err(), Some(SectionError::BadVersion(9)));
        let mut b = good.clone();
        b.truncate(b.len() - 1);
        assert_eq!(Section::parse(&b).err(), Some(SectionError::BadEntry(0)));
        let mut b = good;
        b[HEADER_SIZE + 2] = 99;
        assert_eq!(Section::parse(&b).err(), Some(SectionError::BadEntry(0)));
    }

    /// The ROM's section, read through the writer (D-034: the engine's
    /// tests compare its tables with the ROM's bytes read directly).
    #[test]
    #[cfg(not(feature = "eager"))]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn the_roms_section_holds_both_sine_tables() {
        let Some(path) = ssb_tablegen::test_rom(env!("CARGO_MANIFEST_DIR")) else {
            return;
        };
        let source = ssb_tablegen::Source::open(&path).unwrap();
        let bytes = leak(ssb_tablegen::tables_section(Some(&source)).unwrap());
        let s = Section::parse(bytes).unwrap();
        assert_eq!(s.len(), TableId::ALL.len());
        let sin = read::<[u16; 2048]>(TableId::SinTable, &s);
        assert_eq!((sin[0], sin[1], sin[1024], sin[2047]), (0, 50, 0x8000, 0));
        let lb = read::<[f32; 1024]>(TableId::LbCommonSinLookup, &s);
        assert_eq!(lb[0], 0.0);
        // `sinf(i / 651.8986)` to about six decimals (entry 1023 is 1.0,
        // 1.15e-6 over the sine).
        for (i, &v) in lb.iter().enumerate() {
            let exact = (i as f64 / 651.8986).sin();
            assert!((f64::from(v) - exact).abs() < 2e-6, "entry {i}: {v}");
        }
        assert_eq!(lb[1023], 1.0);
    }

    /// The one test that installs: a table static reads the installed
    /// section, and a second install is refused.
    #[test]
    fn an_installed_section_feeds_a_table_static() {
        static LB: Table<[f32; 1024]> = Table::new(TableId::LbCommonSinLookup);
        let values: Vec<u8> = (0..1024)
            .flat_map(|i| (i as f32 * 0.5).to_le_bytes())
            .collect();
        let bytes = leak(section_bytes(&[(
            TableId::LbCommonSinLookup,
            Kind::F32,
            &values,
        )]));
        // SAFETY: no other test reads a table.
        #[cfg(feature = "eager")]
        let install = |b| unsafe { install(b) };
        install(bytes).unwrap();
        assert_eq!(LB[3], 1.5);
        assert_eq!(LB.len(), 1024);
        assert_eq!(installed().unwrap().len(), 1);
        assert_eq!(install(bytes).err(), Some(InstallError::AlreadyInstalled));
    }
}
