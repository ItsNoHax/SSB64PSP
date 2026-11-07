//! The ROM's audio files and the pack's audio section (D-049).
//!
//! Only layout lives here (D-048): where each file and constant table lies
//! in the ROM, and how big it is. [`build_section`] copies them from the
//! user's ROM into the section `ssb_engine::audio::data::Section` reads;
//! the parsers that check them are the engine's own
//! (`ssb_engine::audio::data`), so the pack builder and the runtime agree
//! on every byte.
//!
//! [`write_section`] streams the same bytes from a paged ROM without
//! assembling the section (the on-device pack build, RE-588).

use alloc::borrow::Cow;
use alloc::vec::Vec;

use crate::pack::{PackSink, SinkError};
use crate::rom_read::RomRef;

use ssb_engine::audio::data::{
    self as fmt, Bank, ParseError, Part, Section, CUSTOM_FX_LEN, EQPOWER_LEN, PART_COUNT,
    PRESETS_LEN, RESAMPLE_TABLE_LEN, SIN_TABLE_LEN,
};
use ssb_engine::audio::stream;

/// A ROM range: `(offset, len)`.
pub type Range = (usize, usize);

/// `S1_music.sbk`: 47 sequences (`dSYAudioPublicSettings.sbk_start`).
pub const SBK: Range = (0xB2_77B0, 159_248);
/// `B1_sounds1.ctl` / `.tbl`: the music bank (bank2).
pub const MUSIC_CTL: Range = (0xB4_E5C0, 26_400);
pub const MUSIC_TBL: Range = (0xB5_4CE0, 1_141_104);
/// `B1_sounds2.ctl` / `.tbl`: the FGM sound bank (bank1).
pub const SFX_CTL: Range = (0xC6_B650, 64_416);
pub const SFX_TBL: Range = (0xC7_B1F0, 2_998_752);
/// `fgm.unk` (LFO records), `fgm.tbl` (articulations), `fgm.ucd` (scripts).
pub const FGM_UNK: Range = (0xF5_73D0, 2_080);
pub const FGM_TBL: Range = (0xF5_7BF0, 11_728);
pub const FGM_UCD: Range = (0xF5_A9C0, 19_232);

/// `n_aspMain`'s resample filter: 64 x 4 `s16` in the microcode's data
/// segment (`n_aspMainDataStart`, ROM 0x40000).
pub const RESAMPLE_TABLE: Range = (0x4_00B0, RESAMPLE_TABLE_LEN);
/// `n_eqpower` (n_env.c:777-800).
pub const EQPOWER: Range = (0x3_DE10, EQPOWER_LEN);
/// `dSYAudioCustomFXParams` (audio.c:56-64).
pub const CUSTOM_FX: Range = (0x3_D550, CUSTOM_FX_LEN);
/// `SMALLROOM_PARAMS_N` through `NULL_PARAMS_N` (n_env.c:281-331).
pub const PRESETS: Range = (0x3_DC80, PRESETS_LEN);
/// `gSYSinTable`.
pub const SIN_TABLE: Range = (0x3_C550, SIN_TABLE_LEN);

/// The parts copied from the ROM: every [`Part`] but the last
/// ([`Part::Heads`], built from the banks).
pub const ROM_PART_COUNT: usize = PART_COUNT - 1;

/// Every ROM part, in [`Part`] order.
pub const PARTS: [Range; ROM_PART_COUNT] = [
    SBK,
    MUSIC_CTL,
    MUSIC_TBL,
    SFX_CTL,
    SFX_TBL,
    FGM_UNK,
    FGM_TBL,
    FGM_UCD,
    RESAMPLE_TABLE,
    EQPOWER,
    CUSTOM_FX,
    PRESETS,
    SIN_TABLE,
];

/// Counts the section must hold (R1 §5.1), checked by [`build_section`].
pub const SEQUENCE_COUNT: usize = 47;
pub const SFX_SOUND_COUNT: usize = 322;
pub const FGM_LFO_COUNT: usize = 100;
pub const FGM_ARTICULATION_COUNT: usize = 464;
/// `nSYAudioFGMVoiceEnd`.
pub const FGM_SCRIPT_COUNT: usize = 695;

/// Why the section could not be built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioBuildError {
    /// The ROM is shorter than a range.
    Truncated(&'static str),
    Parse(&'static str, ParseError),
    /// A count disagrees with the US ROM's.
    Count(&'static str, usize),
}

/// Each ROM part's name, in [`PARTS`] order.
const NAMES: [&str; ROM_PART_COUNT] = [
    "sbk",
    "music ctl",
    "music tbl",
    "sfx ctl",
    "sfx tbl",
    "fgm.unk",
    "fgm.tbl",
    "fgm.ucd",
    "resample",
    "eqpower",
    "custom fx",
    "presets",
    "sin table",
];

/// Copies the audio files and tables out of `rom` (big-endian, as the N64
/// reads them) and checks that they parse.
pub fn build_section(rom: &[u8]) -> Result<Vec<u8>, AudioBuildError> {
    let mut parts: [&[u8]; PART_COUNT] = [&[]; PART_COUNT];
    for (i, &(off, len)) in PARTS.iter().enumerate() {
        parts[i] = rom
            .get(off..off + len)
            .ok_or(AudioBuildError::Truncated(NAMES[i]))?;
    }
    let bank = |ctl: Part, tbl: Part, name| {
        Bank::parse(parts[ctl as usize], parts[tbl as usize].len())
            .map_err(|e| AudioBuildError::Parse(name, e))
    };
    let music = bank(Part::MusicCtl, Part::MusicTbl, "music bank")?;
    let sfx = bank(Part::SfxCtl, Part::SfxTbl, "sfx bank")?;
    let heads = stream::build_heads(
        [&music, &sfx],
        [parts[Part::MusicTbl as usize], parts[Part::SfxTbl as usize]],
    );
    parts[Part::Heads as usize] = &heads;
    let section = Section::build(&parts);
    check_counts(check(&section)?)?;
    Ok(section)
}

/// The resident parts read from the ROM, the banks and the section's
/// layout: what [`section_len`] and [`write_section`] share.
struct Plan<'a> {
    /// Every resident ROM part; the streamed tbls and the heads are empty.
    parts: [Cow<'a, [u8]>; PART_COUNT],
    banks: [Bank; 2],
    ranges: [(usize, usize); PART_COUNT],
    len: usize,
}

/// The tbl parts: copied from the ROM piece by piece, never held.
const TBLS: [Part; 2] = [Part::MusicTbl, Part::SfxTbl];

/// Reads and checks everything but the tbls' bytes: the parts parse, the
/// counts are the US ROM's and the tbl parts stream. The heads are the
/// tbls' bytes by construction, so [`check`]'s heads test holds.
fn plan(rom: RomRef<'_>) -> Result<Plan<'_>, AudioBuildError> {
    let mut parts: [Cow<'_, [u8]>; PART_COUNT] = core::array::from_fn(|_| Cow::Borrowed(&[][..]));
    let mut lens = [0usize; PART_COUNT];
    for (i, &(off, len)) in PARTS.iter().enumerate() {
        let truncated = AudioBuildError::Truncated(NAMES[i]);
        if off + len > rom.len() {
            return Err(truncated);
        }
        lens[i] = len;
        if !TBLS.iter().any(|&p| p as usize == i) {
            parts[i] = rom.read(off, len).map_err(|_| truncated)?;
        }
    }
    let bank = |ctl: Part, tbl: Part, name| {
        Bank::parse(&parts[ctl as usize], lens[tbl as usize])
            .map_err(|e| AudioBuildError::Parse(name, e))
    };
    let banks = [
        bank(Part::MusicCtl, Part::MusicTbl, "music bank")?,
        bank(Part::SfxCtl, Part::SfxTbl, "sfx bank")?,
    ];
    let mut heads = 0;
    for (k, base, len) in stream::head_windows([&banks[0], &banks[1]], stream::HEAD) {
        if base + len > lens[TBLS[k] as usize] {
            return Err(AudioBuildError::Parse(
                "heads",
                ParseError("head past the tbl"),
            ));
        }
        heads += len;
    }
    lens[Part::Heads as usize] = heads;
    let counts = counts(
        &parts[Part::Sbk as usize],
        &banks[1],
        [
            &parts[Part::FgmUnk as usize],
            &parts[Part::FgmTbl as usize],
            &parts[Part::FgmUcd as usize],
        ],
    )?;
    check_counts(counts)?;
    let (ranges, len) = Section::layout(&lens);
    if Section::stream_layout(&Section::header(&ranges)).is_none() {
        return Err(AudioBuildError::Parse(
            "layout",
            ParseError("tbl parts not streamable"),
        ));
    }
    Ok(Plan {
        parts,
        banks,
        ranges,
        len,
    })
}

/// [`build_section`]'s length, from the resident parts alone.
pub fn section_len(rom: RomRef<'_>) -> Result<usize, AudioBuildError> {
    Ok(plan(rom)?.len)
}

/// Writes [`build_section`]'s bytes to `sink` without holding the section
/// (the on-device pack build, RE-588): the header and resident parts
/// (about 0.3 MB), each wave's head read from its tbl, then both tbls
/// copied from the ROM in 64 KiB pieces. Returns the bytes written.
pub fn write_section(rom: RomRef<'_>, sink: &mut dyn PackSink) -> Result<usize, SinkError> {
    const PIECE: usize = 64 * 1024;
    let plan = plan(rom).map_err(|e| SinkError::from(alloc::format!("audio section: {e:?}")))?;
    let rom_err = |e: crate::error::Error| SinkError::from(alloc::format!("audio section: {e}"));
    let mut out = Writer { sink, pos: 0 };
    out.put(&Section::header(&plan.ranges))?;
    let mut order: [usize; PART_COUNT] = core::array::from_fn(|i| i);
    order.sort_by_key(|&i| plan.ranges[i].0);
    let mut buf = Vec::new();
    for i in order {
        let (at, len) = plan.ranges[i];
        out.pad_to(at)?;
        if i == Part::Heads as usize {
            for (k, base, n) in stream::head_windows([&plan.banks[0], &plan.banks[1]], stream::HEAD)
            {
                buf.resize(n, 0);
                rom.read_into(PARTS[TBLS[k] as usize].0 + base, &mut buf)
                    .map_err(rom_err)?;
                out.put(&buf)?;
            }
        } else if let Some(&tbl) = TBLS.iter().find(|&&p| p as usize == i) {
            let from = PARTS[tbl as usize].0;
            for k in (0..len).step_by(PIECE) {
                buf.resize(PIECE.min(len - k), 0);
                rom.read_into(from + k, &mut buf).map_err(rom_err)?;
                out.put(&buf)?;
            }
        } else {
            out.put(&plan.parts[i])?;
        }
        assert_eq!(out.pos, at + len, "audio part {i} length");
    }
    out.pad_to(plan.len)?;
    Ok(plan.len)
}

/// A sink that counts its position and zero-pads forward.
struct Writer<'s> {
    sink: &'s mut dyn PackSink,
    pos: usize,
}

impl Writer<'_> {
    fn put(&mut self, bytes: &[u8]) -> Result<(), SinkError> {
        self.sink.write(bytes)?;
        self.pos += bytes.len();
        Ok(())
    }

    fn pad_to(&mut self, at: usize) -> Result<(), SinkError> {
        const ZEROS: [u8; 64] = [0; 64];
        assert!(self.pos <= at, "audio part out of order");
        while self.pos < at {
            let n = (at - self.pos).min(ZEROS.len());
            self.put(&ZEROS[..n])?;
        }
        Ok(())
    }
}

/// The counts [`check`] returns, from the parts that hold them.
fn counts(
    sbk: &[u8],
    sfx: &Bank,
    [unk, tbl, ucd]: [&[u8]; 3],
) -> Result<[usize; 5], AudioBuildError> {
    let seqs = fmt::parse_sbk(sbk).map_err(|e| AudioBuildError::Parse("sbk", e))?;
    let fgm_sounds = sfx
        .inst_array
        .first()
        .map_or(0, |&i| sfx.instruments[i as usize].sounds.len());
    let unk = fmt::fgm_unk_count(unk).map_err(|e| AudioBuildError::Parse("fgm.unk", e))?;
    let tbl = fmt::parse_package(tbl).map_err(|e| AudioBuildError::Parse("fgm.tbl", e))?;
    let ucd = fmt::parse_package(ucd).map_err(|e| AudioBuildError::Parse("fgm.ucd", e))?;
    Ok([seqs.len(), fgm_sounds, unk, tbl.len(), ucd.len()])
}

/// Each count against the US ROM's.
fn check_counts(counts: [usize; 5]) -> Result<(), AudioBuildError> {
    let want = [
        ("sequences", SEQUENCE_COUNT),
        ("fgm sounds", SFX_SOUND_COUNT),
        ("fgm.unk", FGM_LFO_COUNT),
        ("fgm.tbl", FGM_ARTICULATION_COUNT),
        ("fgm.ucd", FGM_SCRIPT_COUNT),
    ];
    for ((name, want), got) in want.into_iter().zip(counts) {
        if got != want {
            return Err(AudioBuildError::Count(name, got));
        }
    }
    Ok(())
}

/// Parses a built section; returns its counts in [`build_section`]'s order.
pub fn check(section: &[u8]) -> Result<[usize; 5], AudioBuildError> {
    let s = Section::open(section).map_err(|e| AudioBuildError::Parse("section", e))?;
    let music = Bank::parse(s.part(Part::MusicCtl), s.part(Part::MusicTbl).len())
        .map_err(|e| AudioBuildError::Parse("music bank", e))?;
    let sfx = Bank::parse(s.part(Part::SfxCtl), s.part(Part::SfxTbl).len())
        .map_err(|e| AudioBuildError::Parse("sfx bank", e))?;
    let heads = stream::build_heads(
        [&music, &sfx],
        [s.part(Part::MusicTbl), s.part(Part::SfxTbl)],
    );
    if heads != s.part(Part::Heads) {
        return Err(AudioBuildError::Parse(
            "heads",
            ParseError("heads differ from the tbl"),
        ));
    }
    if Section::stream_layout(section).is_none() {
        return Err(AudioBuildError::Parse(
            "layout",
            ParseError("tbl parts not streamable"),
        ));
    }
    counts(
        s.part(Part::Sbk),
        &sfx,
        [
            s.part(Part::FgmUnk),
            s.part(Part::FgmTbl),
            s.part(Part::FgmUcd),
        ],
    )
}
