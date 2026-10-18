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

    /// The ROM offset of `vram`, which a layout table names as lying in
    /// this overlay. Panics otherwise: the layout, not the ROM, is wrong.
    pub fn at(self, vram: u32) -> u32 {
        self.rom_of(vram).unwrap_or_else(|| {
            panic!(
                "0x{vram:08X} is outside the overlay at 0x{:08X} (0x{:X} bytes)",
                self.vram, self.size
            )
        })
    }
}

/// Reads from one overlay of the ROM, or zeros for the stub tables: the
/// generators run the same code with and without a ROM.
#[derive(Clone, Copy)]
pub struct OverlayData<'a> {
    pub rom: Option<&'a Source>,
    pub overlay: Overlay,
}

impl<'a> OverlayData<'a> {
    pub fn new(rom: Option<&'a Source>, overlay: Overlay) -> Self {
        Self { rom, overlay }
    }

    /// The ROM offset of `vram`, checked against the overlay even for a stub.
    fn at(&self, vram: u32, len: u32) -> u32 {
        let at = self.overlay.at(vram);
        self.overlay.at(vram + len - 1);
        at
    }

    pub fn u8(&self, vram: u32) -> Result<u8> {
        let at = self.at(vram, 1);
        self.rom.map_or(Ok(0), |rom| rom.u8(at))
    }

    pub fn u16(&self, vram: u32) -> Result<u16> {
        let at = self.at(vram, 2);
        self.rom.map_or(Ok(0), |rom| rom.u16(at))
    }

    pub fn i16(&self, vram: u32) -> Result<i16> {
        Ok(self.u16(vram)? as i16)
    }

    pub fn u32(&self, vram: u32) -> Result<u32> {
        let at = self.at(vram, 4);
        self.rom.map_or(Ok(0), |rom| rom.u32(at))
    }

    pub fn i32(&self, vram: u32) -> Result<i32> {
        Ok(self.u32(vram)? as i32)
    }

    pub fn f32(&self, vram: u32) -> Result<f32> {
        Ok(f32::from_bits(self.u32(vram)?))
    }

    /// `n` bytes from `vram`.
    pub fn bytes(&self, vram: u32, n: u32) -> Result<Vec<u8>> {
        (0..n).map(|i| self.u8(vram + i)).collect()
    }
}

/// `ovl0` (the core library: `lb/lbtransition.c`'s wipe table).
pub const OVL0: Overlay = Overlay {
    vram: 0x800C_7840,
    rom: 0x04_3220,
    size: 0xEA70,
};
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
/// `ovl10` (the title scene: `mn/mncommon/mntitle.c` and the Sound Test).
pub const OVL10: Overlay = Overlay {
    vram: 0x8013_1B00,
    rom: 0x11_7180,
    size: 0x2940,
};
/// `ovl33` (`mn/mndata/mncharacters.c`: the Characters scene).
pub const OVL33: Overlay = Overlay {
    vram: 0x8013_1B00,
    rom: 0x15_DB50,
    size: 0x4AF0,
};
/// `ovl36` (`mv/mvopening/mvopeningmario.c`: the opening's Mario scene).
pub const OVL36: Overlay = Overlay {
    vram: 0x8018_D0C0,
    rom: 0x16_66F0,
    size: 0x1140,
};
/// `ovl37` (`mv/mvopening/mvopeningdonkey.c`).
pub const OVL37: Overlay = Overlay {
    vram: 0x8018_D0C0,
    rom: 0x16_7830,
    size: 0x1100,
};
/// `ovl38` (`mv/mvopening/mvopeningsamus.c`).
pub const OVL38: Overlay = Overlay {
    vram: 0x8018_D0C0,
    rom: 0x16_8930,
    size: 0x11B0,
};
/// `ovl39` (`mv/mvopening/mvopeningfox.c`).
pub const OVL39: Overlay = Overlay {
    vram: 0x8018_D0C0,
    rom: 0x16_9AE0,
    size: 0x1130,
};
/// `ovl40` (`mv/mvopening/mvopeninglink.c`).
pub const OVL40: Overlay = Overlay {
    vram: 0x8018_D0C0,
    rom: 0x16_AC10,
    size: 0x1100,
};
/// `ovl41` (`mv/mvopening/mvopeningyoshi.c`).
pub const OVL41: Overlay = Overlay {
    vram: 0x8018_D0C0,
    rom: 0x16_BD10,
    size: 0x1160,
};
/// `ovl42` (`mv/mvopening/mvopeningpikachu.c`).
pub const OVL42: Overlay = Overlay {
    vram: 0x8018_D0C0,
    rom: 0x16_CE70,
    size: 0x1160,
};
/// `ovl43` (`mv/mvopening/mvopeningkirby.c`).
pub const OVL43: Overlay = Overlay {
    vram: 0x8018_D0C0,
    rom: 0x16_DFD0,
    size: 0x1160,
};
/// `ovl51` (`mv/mvopening/mvopeningjungle.c`: the opening's jungle scene).
pub const OVL51: Overlay = Overlay {
    vram: 0x8018_D0C0,
    rom: 0x17_4940,
    size: 0x980,
};
/// `ovl56` (`sc/sc1pmode/sc1pstageclear.c`: the 1P Game's stage clear).
pub const OVL56: Overlay = Overlay {
    vram: 0x8013_1B00,
    rom: 0x17_AD50,
    size: 0x37C0,
};
/// `ovl6` (`sc/sc1pmode/sc1pbonusstage.c`: the bonus stages' target, bumper
/// and platform tables).
pub const OVL6: Overlay = Overlay {
    vram: 0x8018_D0C0,
    rom: 0x11_1800,
    size: 0x20E0,
};
/// `ovl24` (`sc/sc1pmode/sc1pintro.c`: the 1P Game's intro).
pub const OVL24: Overlay = Overlay {
    vram: 0x8013_1B00,
    rom: 0x12_AE40,
    size: 0x4120,
};
/// `ovl57` (`mn/mncommon/mncongra.c`: the congratulations picture).
pub const OVL57: Overlay = Overlay {
    vram: 0x8013_1B00,
    rom: 0x17_E510,
    size: 0x7B0,
};
/// `ovl59` (`sc/sccommon/scstaffroll.c`: the staff roll).
pub const OVL59: Overlay = Overlay {
    vram: 0x8013_1B00,
    rom: 0x17_F200,
    size: 0x8CD0,
};
/// `ovl65` (`sc/sc1pmode/sc1pgame.c`, `sc1pgameboss.c`: the 1P Game scene).
pub const OVL65: Overlay = Overlay {
    vram: 0x8018_D0C0,
    rom: 0x10_B920,
    size: 0x5EE0,
};
