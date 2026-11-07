//! The ROM's bytes behind a reader (RE-588): a slice on the host, a paged
//! file on the PSP, where the installer cannot hold the 16 MiB image.
//!
//! - [`RomRef`]: what the readers take; a slice reads zero-copy.
//! - [`PagedRom`]: a [`RawFile`] read through a small page cache, in any
//!   cartridge byte order ([`ByteOrder`]): pages are swapped to `.z64` as
//!   they are read, so every reader sees the big-endian image.
//!
//! Compiled into `ssb-tablegen` too (`#[path]`), with `rom.rs`.

use alloc::borrow::Cow;
use alloc::vec::Vec;
use core::cell::RefCell;

use crate::rom::ByteOrder;
use crate::{Error, Result};

/// Random access to a big-endian (`.z64`) ROM image.
pub trait RomRead {
    /// The image's length in bytes.
    fn len(&self) -> usize;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Fills `out` with the bytes at `at`; out of bounds is an error.
    fn read_at(&self, at: usize, out: &mut [u8]) -> Result<()>;
}

/// The ROM image a reader takes: a slice, or a [`RomRead`].
#[derive(Clone, Copy)]
pub enum RomRef<'a> {
    Bytes(&'a [u8]),
    Dyn(&'a dyn RomRead),
}

impl<'a> RomRef<'a> {
    pub fn len(&self) -> usize {
        match self {
            RomRef::Bytes(b) => b.len(),
            RomRef::Dyn(r) => r.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The whole image, when it is a slice.
    pub fn as_bytes(&self) -> Option<&'a [u8]> {
        match *self {
            RomRef::Bytes(b) => Some(b),
            RomRef::Dyn(_) => None,
        }
    }

    /// `len` bytes at `at`: borrowed from a slice, read from a reader.
    pub fn read(&self, at: usize, len: usize) -> Result<Cow<'a, [u8]>> {
        match *self {
            RomRef::Bytes(b) => b
                .get(
                    at..at
                        .checked_add(len)
                        .ok_or(Error::OutOfBounds { offset: at, len })?,
                )
                .map(Cow::Borrowed)
                .ok_or(Error::OutOfBounds { offset: at, len }),
            RomRef::Dyn(r) => {
                let mut out = alloc::vec![0; len];
                r.read_at(at, &mut out)?;
                Ok(Cow::Owned(out))
            }
        }
    }

    /// Fills `out` with the bytes at `at`.
    pub fn read_into(&self, at: usize, out: &mut [u8]) -> Result<()> {
        match *self {
            RomRef::Bytes(b) => {
                let src = at
                    .checked_add(out.len())
                    .and_then(|end| b.get(at..end))
                    .ok_or(Error::OutOfBounds {
                        offset: at,
                        len: out.len(),
                    })?;
                out.copy_from_slice(src);
                Ok(())
            }
            RomRef::Dyn(r) => r.read_at(at, out),
        }
    }

    pub fn u8(&self, at: usize) -> Result<u8> {
        let mut b = [0; 1];
        self.read_into(at, &mut b)?;
        Ok(b[0])
    }

    /// A big-endian `u16` at `at`.
    pub fn u16(&self, at: usize) -> Result<u16> {
        let mut b = [0; 2];
        self.read_into(at, &mut b)?;
        Ok(u16::from_be_bytes(b))
    }

    /// A big-endian `u32` at `at`.
    pub fn u32(&self, at: usize) -> Result<u32> {
        let mut b = [0; 4];
        self.read_into(at, &mut b)?;
        Ok(u32::from_be_bytes(b))
    }
}

impl<'a> From<&'a [u8]> for RomRef<'a> {
    fn from(b: &'a [u8]) -> Self {
        RomRef::Bytes(b)
    }
}

impl<'a> From<&'a Vec<u8>> for RomRef<'a> {
    fn from(b: &'a Vec<u8>) -> Self {
        RomRef::Bytes(b)
    }
}

impl<'a, const N: usize> From<&'a [u8; N]> for RomRef<'a> {
    fn from(b: &'a [u8; N]) -> Self {
        RomRef::Bytes(b)
    }
}

impl<'a> From<&'a dyn RomRead> for RomRef<'a> {
    fn from(r: &'a dyn RomRead) -> Self {
        RomRef::Dyn(r)
    }
}

impl<'a, F: RawFile> From<&'a PagedRom<F>> for RomRef<'a> {
    fn from(r: &'a PagedRom<F>) -> Self {
        RomRef::Dyn(r)
    }
}

/// A file's raw bytes (the dump as stored, any byte order).
pub trait RawFile {
    /// Reads up to `out.len()` bytes at `at`, returning how many (0 at the
    /// end), or the platform's error code.
    fn read_raw(&mut self, at: u64, out: &mut [u8]) -> core::result::Result<usize, i32>;
}

/// A dump in RAM as a [`RawFile`] (tests: the paged reader's code path).
impl RawFile for &[u8] {
    fn read_raw(&mut self, at: u64, out: &mut [u8]) -> core::result::Result<usize, i32> {
        let src = self.get(at as usize..).unwrap_or(&[]);
        let n = src.len().min(out.len());
        out[..n].copy_from_slice(&src[..n]);
        Ok(n)
    }
}

#[cfg(feature = "std")]
impl RawFile for std::fs::File {
    fn read_raw(&mut self, at: u64, out: &mut [u8]) -> core::result::Result<usize, i32> {
        use std::io::{Read, Seek, SeekFrom};
        self.seek(SeekFrom::Start(at))
            .and_then(|_| self.read(out))
            .map_err(|e| e.raw_os_error().unwrap_or(-1))
    }
}

/// A [`PagedRom`]'s traffic.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReadStats {
    /// `read_at` calls and their bytes.
    pub reads: usize,
    pub bytes: usize,
    /// Pages read into the cache.
    pub misses: usize,
    /// Reads too large for the cache, read straight through.
    pub direct: usize,
    /// `RawFile::read_raw` calls and their bytes.
    pub raw_reads: usize,
    pub raw_bytes: usize,
}

struct Paged<F> {
    file: F,
    /// `(page index, canonical bytes)`, least recently used first.
    pages: Vec<(usize, Vec<u8>)>,
    stats: ReadStats,
}

/// A ROM image read from a [`RawFile`] through a page cache, swapped to
/// `.z64` order page by page. Single-threaded (`RefCell`).
pub struct PagedRom<F: RawFile> {
    inner: RefCell<Paged<F>>,
    len: usize,
    order: ByteOrder,
    page: usize,
    max_pages: usize,
}

impl<F: RawFile> PagedRom<F> {
    /// Pages `file` (`len` bytes) in `page`-byte pages, at most
    /// `max_pages` cached. `page` is a power of two, at least 4. The byte
    /// order comes from the dump's first word; an unknown one is
    /// [`Error::WrongByteOrder`].
    pub fn new(file: F, len: usize, page: usize, max_pages: usize) -> Result<Self> {
        assert!(page.is_power_of_two() && page >= 4 && max_pages >= 1);
        if !len.is_multiple_of(4) {
            return Err(Error::BadRomSize(len));
        }
        let mut inner = Paged {
            file,
            pages: Vec::with_capacity(max_pages),
            stats: ReadStats::default(),
        };
        let mut magic = [0; 4];
        read_exact(&mut inner, 0, &mut magic)?;
        let order = ByteOrder::detect(&magic).ok_or(Error::WrongByteOrder {
            detected: "unrecognised",
        })?;
        Ok(Self {
            inner: RefCell::new(inner),
            len,
            order,
            page,
            max_pages,
        })
    }

    /// The dump's byte order on disk.
    pub fn order(&self) -> ByteOrder {
        self.order
    }

    pub fn stats(&self) -> ReadStats {
        self.inner.borrow().stats
    }

    /// The file back, cache dropped.
    pub fn into_file(self) -> F {
        self.inner.into_inner().file
    }

    /// Raw bytes `[at, at + out.len())`, both multiples of 4, as `.z64`.
    fn fill(&self, inner: &mut Paged<F>, at: usize, out: &mut [u8]) -> Result<()> {
        read_exact(inner, at, out)?;
        self.order.to_z64(out);
        Ok(())
    }

    /// The cache index of page `index`, read if absent.
    fn page_slot(&self, inner: &mut Paged<F>, index: usize) -> Result<usize> {
        let n = inner.pages.len();
        if n > 0 && inner.pages[n - 1].0 == index {
            return Ok(n - 1);
        }
        if let Some(k) = inner.pages.iter().position(|(i, _)| *i == index) {
            let entry = inner.pages.remove(k);
            inner.pages.push(entry);
            return Ok(n - 1);
        }
        let mut buf = if n >= self.max_pages {
            inner.pages.remove(0).1
        } else {
            Vec::with_capacity(self.page)
        };
        let start = index * self.page;
        let len = self.page.min(self.len - start);
        buf.clear();
        buf.resize(len, 0);
        inner.stats.misses += 1;
        self.fill(inner, start, &mut buf)?;
        inner.pages.push((index, buf));
        Ok(inner.pages.len() - 1)
    }
}

/// Reads exactly `out.len()` raw bytes at `at`.
fn read_exact<F: RawFile>(inner: &mut Paged<F>, at: usize, out: &mut [u8]) -> Result<()> {
    let mut done = 0;
    while done < out.len() {
        let got = inner
            .file
            .read_raw((at + done) as u64, &mut out[done..])
            .map_err(|code| Error::RomIo {
                offset: at + done,
                code,
            })?;
        inner.stats.raw_reads += 1;
        inner.stats.raw_bytes += got;
        if got == 0 {
            return Err(Error::OutOfBounds {
                offset: at + done,
                len: out.len() - done,
            });
        }
        done += got;
    }
    Ok(())
}

impl<F: RawFile> RomRead for PagedRom<F> {
    fn len(&self) -> usize {
        self.len
    }

    fn read_at(&self, at: usize, out: &mut [u8]) -> Result<()> {
        let end = at
            .checked_add(out.len())
            .filter(|&end| end <= self.len)
            .ok_or(Error::OutOfBounds {
                offset: at,
                len: out.len(),
            })?;
        let mut inner = self.inner.borrow_mut();
        let inner = &mut *inner;
        inner.stats.reads += 1;
        inner.stats.bytes += out.len();
        if out.is_empty() {
            return Ok(());
        }
        // Large reads (a whole archive file, the SHA-1 pass, the audio
        // tbls) go straight through and leave the cache alone.
        if out.len() >= 2 * self.page {
            inner.stats.direct += 1;
            let (lo, hi) = (at & !3, end.next_multiple_of(4));
            if (lo, hi) == (at, end) {
                return self.fill(inner, at, out);
            }
            let mut tmp = alloc::vec![0; hi - lo];
            self.fill(inner, lo, &mut tmp)?;
            out.copy_from_slice(&tmp[at - lo..end - lo]);
            return Ok(());
        }
        let mut pos = at;
        while pos < end {
            let index = pos / self.page;
            let slot = self.page_slot(inner, index)?;
            let page = &inner.pages[slot].1;
            let from = pos - index * self.page;
            let n = (page.len() - from).min(end - pos);
            out[pos - at..pos - at + n].copy_from_slice(&page[from..from + n]);
            pos += n;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(n: usize) -> Vec<u8> {
        let mut x = 0x2468_ACE1u32;
        let mut v: Vec<u8> = (0..n)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                x as u8
            })
            .collect();
        v[..4].copy_from_slice(&[0x80, 0x37, 0x12, 0x40]);
        v
    }

    fn swapped(z64: &[u8], order: ByteOrder) -> Vec<u8> {
        let mut v = z64.to_vec();
        match order {
            ByteOrder::Z64 => {}
            ByteOrder::V64 => v
                .as_chunks_mut::<2>()
                .0
                .iter_mut()
                .for_each(|w| w.swap(0, 1)),
            ByteOrder::N64 => v
                .as_chunks_mut::<4>()
                .0
                .iter_mut()
                .for_each(|w| w.reverse()),
        }
        v
    }

    /// Every byte order reads back the `.z64` bytes, at any offset and
    /// length, through the cache and around it.
    #[test]
    fn paged_reads_match_the_image_in_every_order() {
        let z64 = image(64 * 1024 + 4 * 37);
        for order in [ByteOrder::Z64, ByteOrder::V64, ByteOrder::N64] {
            let raw = swapped(&z64, order);
            let rom = PagedRom::new(&raw[..], raw.len(), 1024, 3).unwrap();
            assert_eq!(rom.order(), order);
            let mut x = 5usize;
            for k in 0..2_000usize {
                x = x.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
                let at = (x >> 20) % z64.len();
                let len = [0usize, 1, 2, 3, 4, 5, 1023, 1024, 1025, 2047, 2048, 5000][k % 12]
                    .min(z64.len() - at);
                let mut out = alloc::vec![0xEE; len];
                rom.read_at(at, &mut out).unwrap();
                assert_eq!(out, &z64[at..at + len], "{order:?} at {at} len {len}");
            }
            let mut one = [0];
            assert!(rom.read_at(z64.len(), &mut one).is_err());
            let s = rom.stats();
            assert!(s.misses > 0 && s.direct > 0, "{s:?}");
        }
    }

    #[test]
    fn rom_ref_reads_agree() {
        let z64 = image(8192);
        let raw = swapped(&z64, ByteOrder::N64);
        let paged = PagedRom::new(&raw[..], raw.len(), 256, 2).unwrap();
        let (a, b) = (RomRef::from(&z64), RomRef::from(&paged));
        assert_eq!(a.len(), b.len());
        for at in [0usize, 1, 3, 255, 256, 4000, 8188] {
            assert_eq!(a.u32(at).unwrap(), b.u32(at).unwrap());
            assert_eq!(a.u16(at).unwrap(), b.u16(at).unwrap());
            assert_eq!(a.read(at, 4).unwrap(), b.read(at, 4).unwrap());
        }
        assert!(a.read(8190, 4).is_err() && b.read(8190, 4).is_err());
    }

    #[test]
    fn unknown_byte_order_is_refused() {
        let raw = alloc::vec![0u8; 64];
        assert!(matches!(
            PagedRom::new(&raw[..], raw.len(), 16, 1),
            Err(Error::WrongByteOrder { .. })
        ));
    }
}
