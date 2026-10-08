//! The pack's audio section (D-049) and the ROM audio files it holds,
//! parsed once at boot into index-linked tables.
//!
//! The section keeps every file in its original big-endian ROM form;
//! `ssb_rom::audio` writes it. Parsing here follows `alBnkfNew` and the
//! `syAudioBnkfPatch*` chain (`src/sys/audio.c:312-422`) and
//! `syAudioLoadAssets` (audio.c:764-878): offsets in the files become
//! indices into the tables below, deduplicated by file offset, so two
//! pointers that were equal on the N64 are equal indices here.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;

/// `"SAUD"`, little-endian.
pub const SECTION_MAGIC: u32 = u32::from_le_bytes(*b"SAUD");
/// Bumped when the section's part list changes.
pub const SECTION_VERSION: u32 = 1;
/// Header: magic, version, part count, then `(offset, len)` per part.
pub const SECTION_HEADER: usize = 12 + PART_COUNT * 8;
/// Every part starts on this boundary inside the section.
pub const PART_ALIGN: usize = 16;

/// The section's parts, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum Part {
    /// `S1_music.sbk`: the `ALSeqFile` of the 47 compressed sequences.
    Sbk,
    /// `B1_sounds1.ctl`/`.tbl`: the music bank (`sSYAudioSequenceBank2`).
    MusicCtl,
    MusicTbl,
    /// `B1_sounds2.ctl`/`.tbl`: the FGM sound bank (`sSYAudioSequenceBank1`).
    SfxCtl,
    SfxTbl,
    /// `fgm.unk`: LFO records (count, then 16-byte records).
    FgmUnk,
    /// `fgm.tbl`: articulation scripts (a package: count, file offsets).
    FgmTbl,
    /// `fgm.ucd`: FGM scripts (a package).
    FgmUcd,
    /// `n_aspMain`'s resample filter, 64 phases x 4 taps of `s16`.
    ResampleTable,
    /// `n_eqpower`: 128 `s16` (n_env.c:777-800).
    EqPower,
    /// `dSYAudioCustomFXParams`: 114 `s32` (audio.c:56-64).
    CustomFx,
    /// `SMALLROOM_PARAMS_N` .. `NULL_PARAMS_N`: 100 `s32` (n_env.c:281-331).
    Presets,
    /// `gSYSinTable`: 2048 `u16`, `sin(x) * 32768` over `[0, pi)`.
    SinTable,
}

pub const PART_COUNT: usize = 13;

/// Byte sizes of the fixed-size parts.
pub const RESAMPLE_TABLE_LEN: usize = 64 * 4 * 2;
pub const EQPOWER_LEN: usize = 128 * 2;
pub const CUSTOM_FX_LEN: usize = 114 * 4;
pub const PRESETS_LEN: usize = 100 * 4;
pub const SIN_TABLE_LEN: usize = 2048 * 2;

/// A malformed section or file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseError(pub &'static str);

pub(crate) fn be16(d: &[u8], at: usize) -> Result<u16, ParseError> {
    d.get(at..at + 2)
        .map(|b| u16::from_be_bytes([b[0], b[1]]))
        .ok_or(ParseError("u16 out of range"))
}

pub(crate) fn be32(d: &[u8], at: usize) -> Result<u32, ParseError> {
    d.get(at..at + 4)
        .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
        .ok_or(ParseError("u32 out of range"))
}

fn byte(d: &[u8], at: usize) -> Result<u8, ParseError> {
    d.get(at).copied().ok_or(ParseError("u8 out of range"))
}

/// A view over the section's parts.
#[derive(Debug, Clone, Copy)]
pub struct Section<'a> {
    parts: [&'a [u8]; PART_COUNT],
}

impl<'a> Section<'a> {
    pub fn open(data: &'a [u8]) -> Result<Self, ParseError> {
        let le = |at: usize| -> Result<u32, ParseError> {
            data.get(at..at + 4)
                .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                .ok_or(ParseError("section header truncated"))
        };
        if le(0)? != SECTION_MAGIC || le(4)? != SECTION_VERSION || le(8)? as usize != PART_COUNT {
            return Err(ParseError("not an audio section"));
        }
        let mut parts = [&data[..0]; PART_COUNT];
        for (i, part) in parts.iter_mut().enumerate() {
            let off = le(12 + i * 8)? as usize;
            let len = le(16 + i * 8)? as usize;
            *part = data
                .get(off..off.checked_add(len).ok_or(ParseError("part overflows"))?)
                .ok_or(ParseError("part out of range"))?;
        }
        let fixed = [
            (Part::ResampleTable, RESAMPLE_TABLE_LEN),
            (Part::EqPower, EQPOWER_LEN),
            (Part::CustomFx, CUSTOM_FX_LEN),
            (Part::Presets, PRESETS_LEN),
            (Part::SinTable, SIN_TABLE_LEN),
        ];
        if fixed.iter().any(|&(p, n)| parts[p as usize].len() != n) {
            return Err(ParseError("constant table has the wrong size"));
        }
        Ok(Self { parts })
    }

    pub fn part(&self, part: Part) -> &'a [u8] {
        self.parts[part as usize]
    }

    /// Lays `parts` (in [`Part`] order) out as a section.
    pub fn build(parts: &[&[u8]; PART_COUNT]) -> Vec<u8> {
        let mut out = alloc::vec![0u8; SECTION_HEADER];
        out[0..4].copy_from_slice(&SECTION_MAGIC.to_le_bytes());
        out[4..8].copy_from_slice(&SECTION_VERSION.to_le_bytes());
        out[8..12].copy_from_slice(&(PART_COUNT as u32).to_le_bytes());
        for (i, p) in parts.iter().enumerate() {
            let at = out.len().next_multiple_of(PART_ALIGN);
            out.resize(at, 0);
            out[12 + i * 8..16 + i * 8].copy_from_slice(&(at as u32).to_le_bytes());
            out[16 + i * 8..20 + i * 8].copy_from_slice(&(p.len() as u32).to_le_bytes());
            out.extend_from_slice(p);
        }
        out
    }
}

/// Reads a big-endian `s16` table.
pub fn be_i16s<const N: usize>(d: &[u8]) -> [i16; N] {
    let mut out = [0i16; N];
    for (o, b) in out.iter_mut().zip(d.as_chunks::<2>().0) {
        *o = i16::from_be_bytes([b[0], b[1]]);
    }
    out
}

/// Reads a big-endian `s32` table.
pub fn be_i32s<const N: usize>(d: &[u8]) -> [i32; N] {
    let mut out = [0i32; N];
    for (o, b) in out.iter_mut().zip(d.as_chunks::<4>().0) {
        *o = i32::from_be_bytes([b[0], b[1], b[2], b[3]]);
    }
    out
}

// --- ALSeqFile ---------------------------------------------------------------

/// One `ALSeqData`: where a compressed sequence lies in the sbk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SeqEntry {
    pub offset: u32,
    pub len: u32,
}

/// `ALSeqFile` (`syAudioSeqFileNew`, audio.c:296-309): revision, count,
/// then `{offset, len}` pairs relative to the file.
pub fn parse_sbk(sbk: &[u8]) -> Result<Vec<SeqEntry>, ParseError> {
    let count = be16(sbk, 2)? as usize;
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let offset = be32(sbk, 4 + i * 8)?;
        let len = be32(sbk, 8 + i * 8)?;
        if offset as usize + len as usize > sbk.len() {
            return Err(ParseError("sequence out of range"));
        }
        out.push(SeqEntry { offset, len });
    }
    Ok(out)
}

// --- ALBankFile --------------------------------------------------------------

/// `ALEnvelope`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Envelope {
    pub attack_time: i32,
    pub decay_time: i32,
    pub release_time: i32,
    pub attack_volume: u8,
    pub decay_volume: u8,
}

/// `ALKeyMap`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct KeyMap {
    pub velocity_min: u8,
    pub velocity_max: u8,
    pub key_min: u8,
    pub key_max: u8,
    pub key_base: u8,
    pub detune: i8,
}

/// `ALADPCMloop`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AdpcmLoop {
    pub start: u32,
    pub end: u32,
    pub count: u32,
    pub state: [i16; 16],
}

/// `ALADPCMBook`: `order * npredictors * 8` coefficients. The shipped
/// banks use order 2 with 2 or 4 predictors, so 64 is enough.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdpcmBook {
    pub order: i32,
    pub npredictors: i32,
    pub book: [i16; BOOK_MAX],
}

pub const BOOK_MAX: usize = 64;

/// `AL_ADPCM_WAVE` / `AL_RAW16_WAVE`.
pub const AL_ADPCM_WAVE: u8 = 0;
pub const AL_RAW16_WAVE: u8 = 1;

/// `ALWaveTable`. `base` is a byte offset into the bank's tbl.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WaveTable {
    pub base: u32,
    pub len: i32,
    pub kind: u8,
    /// Index into [`Bank::loops`].
    pub loop_: Option<u16>,
    /// Index into [`Bank::books`] (ADPCM only).
    pub book: Option<u16>,
}

/// `ALSound`; the indices name [`Bank`] tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sound {
    pub envelope: u16,
    pub keymap: u16,
    pub wavetable: u16,
    pub sample_pan: u8,
    pub sample_volume: u8,
}

/// `ALInstrument`; `sounds` indexes [`Bank::sound_lists`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instrument {
    pub volume: u8,
    pub pan: u8,
    pub priority: u8,
    pub trem_type: u8,
    pub trem_rate: u8,
    pub trem_depth: u8,
    pub trem_delay: u8,
    pub vib_type: u8,
    pub vib_rate: u8,
    pub vib_depth: u8,
    pub vib_delay: u8,
    pub bend_range: i16,
    /// `soundArray`, as [`Bank::sounds`] indices.
    pub sounds: Vec<u16>,
}

/// `ALBank` with everything it reaches, flattened.
#[derive(Debug, Clone, Default)]
pub struct Bank {
    pub sample_rate: i32,
    /// `instArray`, as [`Self::instruments`] indices.
    pub inst_array: Vec<u16>,
    pub percussion: Option<u16>,
    pub instruments: Vec<Instrument>,
    pub sounds: Vec<Sound>,
    pub envelopes: Vec<Envelope>,
    pub keymaps: Vec<KeyMap>,
    pub wavetables: Vec<WaveTable>,
    pub books: Vec<AdpcmBook>,
    pub loops: Vec<AdpcmLoop>,
}

/// Interns structs by their ctl offset so equal pointers stay equal.
struct Interner<T> {
    by_offset: BTreeMap<u32, u16>,
    items: Vec<T>,
}

impl<T> Interner<T> {
    fn new() -> Self {
        Self {
            by_offset: BTreeMap::new(),
            items: Vec::new(),
        }
    }

    fn get(
        &mut self,
        offset: u32,
        parse: impl FnOnce(usize) -> Result<T, ParseError>,
    ) -> Result<u16, ParseError> {
        if let Some(&i) = self.by_offset.get(&offset) {
            return Ok(i);
        }
        let item = parse(offset as usize)?;
        let i = u16::try_from(self.items.len()).map_err(|_| ParseError("too many items"))?;
        self.items.push(item);
        self.by_offset.insert(offset, i);
        Ok(i)
    }
}

impl Bank {
    /// Parses bank 0 of an `ALBankFile` (`alBnkfNew`). `tbl_len` bounds the
    /// wave data.
    ///
    /// `instArray` entries are offsets that `syAudioBnkfPatchBank` adds the
    /// file base to *before* its NULL test, so an entry of 0 becomes a
    /// pointer to the file header itself, whose `flags` byte (the bank
    /// count's low byte, 1) makes `syAudioBnkfPatchInst` skip it. The
    /// header then reads as an instrument (the music bank's `instArray[0]`,
    /// which `__n_initFromBank` picks as every channel's first
    /// instrument). Its `soundArray` holds unpatched garbage that no note
    /// may reach, so it is parsed with no sounds.
    pub fn parse(ctl: &[u8], tbl_len: usize) -> Result<Self, ParseError> {
        if be16(ctl, 2)? < 1 {
            return Err(ParseError("bank file has no bank"));
        }
        let b = be32(ctl, 4)? as usize;
        let inst_count = be16(ctl, b)? as usize;
        let sample_rate = be32(ctl, b + 4)? as i32;
        let perc = be32(ctl, b + 8)?;

        let mut insts: Interner<Instrument> = Interner::new();
        let mut sounds: Interner<Sound> = Interner::new();
        let mut envs: Interner<Envelope> = Interner::new();
        let mut keymaps: Interner<KeyMap> = Interner::new();
        let mut waves: Interner<WaveTable> = Interner::new();
        let mut books: Interner<AdpcmBook> = Interner::new();
        let mut loops: Interner<AdpcmLoop> = Interner::new();

        let mut parse_inst = |at: usize| -> Result<Instrument, ParseError> {
            let mut inst = Instrument {
                volume: byte(ctl, at)?,
                pan: byte(ctl, at + 1)?,
                priority: byte(ctl, at + 2)?,
                trem_type: byte(ctl, at + 4)?,
                trem_rate: byte(ctl, at + 5)?,
                trem_depth: byte(ctl, at + 6)?,
                trem_delay: byte(ctl, at + 7)?,
                vib_type: byte(ctl, at + 8)?,
                vib_rate: byte(ctl, at + 9)?,
                vib_depth: byte(ctl, at + 10)?,
                vib_delay: byte(ctl, at + 11)?,
                bend_range: be16(ctl, at + 12)? as i16,
                sounds: Vec::new(),
            };
            if at == 0 {
                return Ok(inst);
            }
            let count = be16(ctl, at + 14)? as usize;
            for i in 0..count {
                let s = be32(ctl, at + 16 + i * 4)?;
                let idx = sounds.get(s, |at| {
                    let env = be32(ctl, at)?;
                    let km = be32(ctl, at + 4)?;
                    let wt = be32(ctl, at + 8)?;
                    let envelope = envs.get(env, |at| {
                        Ok(Envelope {
                            attack_time: be32(ctl, at)? as i32,
                            decay_time: be32(ctl, at + 4)? as i32,
                            release_time: be32(ctl, at + 8)? as i32,
                            attack_volume: byte(ctl, at + 12)?,
                            decay_volume: byte(ctl, at + 13)?,
                        })
                    })?;
                    let keymap = keymaps.get(km, |at| {
                        Ok(KeyMap {
                            velocity_min: byte(ctl, at)?,
                            velocity_max: byte(ctl, at + 1)?,
                            key_min: byte(ctl, at + 2)?,
                            key_max: byte(ctl, at + 3)?,
                            key_base: byte(ctl, at + 4)?,
                            detune: byte(ctl, at + 5)? as i8,
                        })
                    })?;
                    let wavetable = waves.get(wt, |at| {
                        let base = be32(ctl, at)?;
                        let len = be32(ctl, at + 4)? as i32;
                        let kind = byte(ctl, at + 8)?;
                        if base as usize + len.max(0) as usize > tbl_len {
                            return Err(ParseError("wave out of the tbl"));
                        }
                        let lp = be32(ctl, at + 12)?;
                        let parse_loop = |at: usize| {
                            let mut state = [0i16; 16];
                            for (k, s) in state.iter_mut().enumerate() {
                                *s = be16(ctl, at + 12 + k * 2)? as i16;
                            }
                            Ok(AdpcmLoop {
                                start: be32(ctl, at)?,
                                end: be32(ctl, at + 4)?,
                                count: be32(ctl, at + 8)?,
                                state,
                            })
                        };
                        let loop_ = if lp != 0 {
                            Some(loops.get(lp, parse_loop)?)
                        } else {
                            None
                        };
                        let book = if kind == AL_ADPCM_WAVE {
                            let bk = be32(ctl, at + 16)?;
                            Some(books.get(bk, |at| {
                                let order = be32(ctl, at)? as i32;
                                let npredictors = be32(ctl, at + 4)? as i32;
                                let n = (order * npredictors * 8) as usize;
                                if n > BOOK_MAX {
                                    return Err(ParseError("ADPCM book too large"));
                                }
                                let mut book = [0i16; BOOK_MAX];
                                for (k, c) in book[..n].iter_mut().enumerate() {
                                    *c = be16(ctl, at + 8 + k * 2)? as i16;
                                }
                                Ok(AdpcmBook {
                                    order,
                                    npredictors,
                                    book,
                                })
                            })?)
                        } else {
                            None
                        };
                        Ok(WaveTable {
                            base,
                            len,
                            kind,
                            loop_,
                            book,
                        })
                    })?;
                    Ok(Sound {
                        envelope,
                        keymap,
                        wavetable,
                        sample_pan: byte(ctl, at + 12)?,
                        sample_volume: byte(ctl, at + 13)?,
                    })
                })?;
                inst.sounds.push(idx);
            }
            Ok(inst)
        };

        let percussion = if perc != 0 {
            Some(insts.get(perc, &mut parse_inst)?)
        } else {
            None
        };
        let mut inst_array = Vec::with_capacity(inst_count);
        for i in 0..inst_count {
            let off = be32(ctl, b + 12 + i * 4)?;
            inst_array.push(insts.get(off, &mut parse_inst)?);
        }
        Ok(Self {
            sample_rate,
            inst_array,
            percussion,
            instruments: insts.items,
            sounds: sounds.items,
            envelopes: envs.items,
            keymaps: keymaps.items,
            wavetables: waves.items,
            books: books.items,
            loops: loops.items,
        })
    }
}

// --- FGM files -----------------------------------------------------------------

/// A package (`fgm.tbl`, `fgm.ucd`): a count, then `u32` offsets relative to
/// the file start (audio.c:858-876). Returns the offsets.
pub fn parse_package(file: &[u8]) -> Result<Vec<u32>, ParseError> {
    let count = be32(file, 0)? as usize;
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let off = be32(file, 4 + i * 4)?;
        if off as usize >= file.len() {
            return Err(ParseError("package entry out of range"));
        }
        out.push(off);
    }
    Ok(out)
}

/// `fgm.unk`'s record count (the records follow at +4, 16 bytes each).
pub fn fgm_unk_count(file: &[u8]) -> Result<usize, ParseError> {
    let count = be32(file, 0)? as usize;
    if 4 + count * 16 > file.len() {
        return Err(ParseError("fgm.unk truncated"));
    }
    Ok(count)
}

// --- Everything the engine reads ----------------------------------------------

/// The parsed section: both banks with their wave data, the sequences, the
/// FGM files and the constant tables. Built once by `AudioSystem::new`.
pub struct AudioData {
    /// `B1_sounds1`: the sequence player's bank. [`WaveRef::bank`] 0.
    pub music: Bank,
    pub music_tbl: &'static [u8],
    /// `B1_sounds2`: the FGM engine's sounds. [`WaveRef::bank`] 1.
    pub sfx: Bank,
    pub sfx_tbl: &'static [u8],
    pub sbk: &'static [u8],
    pub seqs: Vec<SeqEntry>,
    /// `fgm.unk` records (16 bytes each, from file offset 4).
    pub fgm_unk: &'static [u8],
    pub fgm_unk_count: usize,
    pub fgm_tbl: &'static [u8],
    pub fgm_tbl_offsets: Vec<u32>,
    pub fgm_ucd: &'static [u8],
    pub fgm_ucd_offsets: Vec<u32>,
    /// `n_aspMain`'s 64 x 4 resample filter.
    pub resample: [[i16; 4]; 64],
    /// `n_eqpower`, plus index 128: `SET_FXAMT_ALT` with `moredata` 0 reads
    /// one past the table, the word after it in RDRAM, which is 0.
    pub eqpower: [i16; 129],
    pub custom_fx: [i32; 114],
    pub presets: [i32; 100],
    pub sin_table: [u16; 2048],
}

/// A wave table in one of the two banks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WaveRef {
    pub bank: u8,
    pub index: u16,
}

impl AudioData {
    pub fn new(section: &'static [u8]) -> Result<Self, ParseError> {
        let s = Section::open(section)?;
        let music_tbl = s.part(Part::MusicTbl);
        let sfx_tbl = s.part(Part::SfxTbl);
        let mut resample = [[0i16; 4]; 64];
        let flat: [i16; 256] = be_i16s(s.part(Part::ResampleTable));
        for (row, c) in resample.iter_mut().zip(flat.as_chunks::<4>().0) {
            *row = *c;
        }
        let mut eqpower = [0i16; 129];
        let eq: [i16; 128] = be_i16s(s.part(Part::EqPower));
        eqpower[..128].copy_from_slice(&eq);
        let mut sin_table = [0u16; 2048];
        for (o, b) in sin_table
            .iter_mut()
            .zip(s.part(Part::SinTable).as_chunks::<2>().0)
        {
            *o = u16::from_be_bytes([b[0], b[1]]);
        }
        Ok(Self {
            music: Bank::parse(s.part(Part::MusicCtl), music_tbl.len())?,
            music_tbl,
            sfx: Bank::parse(s.part(Part::SfxCtl), sfx_tbl.len())?,
            sfx_tbl,
            sbk: s.part(Part::Sbk),
            seqs: parse_sbk(s.part(Part::Sbk))?,
            fgm_unk: s.part(Part::FgmUnk),
            fgm_unk_count: fgm_unk_count(s.part(Part::FgmUnk))?,
            fgm_tbl: s.part(Part::FgmTbl),
            fgm_tbl_offsets: parse_package(s.part(Part::FgmTbl))?,
            fgm_ucd: s.part(Part::FgmUcd),
            fgm_ucd_offsets: parse_package(s.part(Part::FgmUcd))?,
            resample,
            eqpower,
            custom_fx: be_i32s(s.part(Part::CustomFx)),
            presets: be_i32s(s.part(Part::Presets)),
            sin_table,
        })
    }

    /// The bank and wave data a [`WaveRef`] names.
    #[inline]
    pub fn wave(&self, w: WaveRef) -> (&WaveTable, &Bank, &'static [u8]) {
        let (bank, tbl) = if w.bank == 0 {
            (&self.music, self.music_tbl)
        } else {
            (&self.sfx, self.sfx_tbl)
        };
        (&bank.wavetables[w.index as usize], bank, tbl)
    }
}
