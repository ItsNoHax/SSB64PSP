//! The verified ROM image the tables are read from.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use crate::archive::{Archive, File};
use crate::rom::{self, Region};
use crate::{Error, Result};

/// A verified Super Smash Bros. (USA) ROM, read once.
pub struct Source {
    rom: Vec<u8>,
    files: Mutex<HashMap<u32, File>>,
}

impl Source {
    /// Reads and verifies the ROM at `path` (size, byte order and SHA-1).
    pub fn open(path: &Path) -> std::result::Result<Self, String> {
        let rom = std::fs::read(path).map_err(|e| e.to_string())?;
        Self::from_bytes(rom).map_err(|e| e.to_string())
    }

    /// Verifies `rom` as a supported dump.
    pub fn from_bytes(rom: Vec<u8>) -> Result<Self> {
        rom::identify(&rom)?;
        Ok(Self {
            rom,
            files: Mutex::new(HashMap::new()),
        })
    }

    /// `len` bytes of the cartridge image at ROM offset `at` (code and data
    /// segments outside the archive).
    pub fn bytes(&self, at: u32, len: usize) -> Result<&[u8]> {
        rom::slice(&self.rom, at as usize, len)
    }

    pub fn u32(&self, at: u32) -> Result<u32> {
        rom::read_u32(&self.rom, at as usize)
    }

    pub fn u16(&self, at: u32) -> Result<u16> {
        rom::read_u16(&self.rom, at as usize)
    }

    pub fn u8(&self, at: u32) -> Result<u8> {
        Ok(self.bytes(at, 1)?[0])
    }

    pub fn f32(&self, at: u32) -> Result<f32> {
        Ok(f32::from_bits(self.u32(at)?))
    }

    /// Archive file `id`, decompressed, with its relocation records.
    /// Intern pointer slots hold file-relative byte offsets; extern slots
    /// are zero (`archive::Archive::load`).
    pub fn file(&self, id: u32) -> Result<File> {
        let mut files = self.files.lock().expect("file cache");
        if let Some(file) = files.get(&id) {
            return Ok(file.clone());
        }
        let file = Archive::open(&self.rom, Region::Us)?.load(id)?;
        files.insert(id, file.clone());
        Ok(file)
    }
}

/// Big-endian reads within a decompressed file.
pub fn be_u32(data: &[u8], at: usize) -> Result<u32> {
    let b = data
        .get(at..at + 4)
        .ok_or(Error::OutOfBounds { offset: at, len: 4 })?;
    Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
}

pub fn be_u16(data: &[u8], at: usize) -> Result<u16> {
    let b = data
        .get(at..at + 2)
        .ok_or(Error::OutOfBounds { offset: at, len: 2 })?;
    Ok(u16::from_be_bytes([b[0], b[1]]))
}

/// A code overlay: where it runs and where it sits in the ROM. Data inside
/// an overlay points at other data by run-time (`vram`) address.
#[derive(Debug, Clone, Copy)]
pub struct Overlay {
    pub vram: u32,
    pub rom: u32,
    pub size: u32,
}

impl Overlay {
    /// The ROM offset of run-time address `vram`, if it lies in this overlay.
    pub fn rom_of(self, vram: u32) -> Option<u32> {
        vram.checked_sub(self.vram)
            .filter(|off| *off < self.size)
            .map(|off| self.rom + off)
    }
}

/// `ovl1` (the `scsubsys` scene data: demo motion scripts and tables).
pub const OVL1: Overlay = Overlay {
    vram: 0x8039_03E0,
    rom: 0x10_79C0,
    size: 0x25F0,
};
/// `ovl2` (the fighter, item and effect code and its data).
pub const OVL2: Overlay = Overlay {
    vram: 0x800D_6490,
    rom: 0x05_1C90,
    size: 0x5_A8B0,
};
/// `ovl3` (the battle scene: CPU scripts and attack tables).
pub const OVL3: Overlay = Overlay {
    vram: 0x8013_1B00,
    rom: 0x0A_C540,
    size: 0x5_B480,
};
