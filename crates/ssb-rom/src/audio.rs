//! The ROM's audio files and the pack's audio section (D-049).
//!
//! Only layout lives here (D-048): where each file and constant table lies
//! in the ROM, and how big it is. [`build_section`] copies them from the
//! user's ROM into the section `ssb_engine::audio::data::Section` reads;
//! the parsers that check them are the engine's own
//! (`ssb_engine::audio::data`), so the pack builder and the runtime agree
//! on every byte.

use alloc::vec::Vec;

use ssb_engine::audio::data::{
    self as fmt, Bank, ParseError, Part, Section, CUSTOM_FX_LEN, EQPOWER_LEN, PART_COUNT,
    PRESETS_LEN, RESAMPLE_TABLE_LEN, SIN_TABLE_LEN,
};

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

/// Every part, in [`Part`] order.
pub const PARTS: [Range; PART_COUNT] = [
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

/// Copies the audio files and tables out of `rom` (big-endian, as the N64
/// reads them) and checks that they parse.
pub fn build_section(rom: &[u8]) -> Result<Vec<u8>, AudioBuildError> {
    const NAMES: [&str; PART_COUNT] = [
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
    let mut parts: [&[u8]; PART_COUNT] = [&[]; PART_COUNT];
    for (i, &(off, len)) in PARTS.iter().enumerate() {
        parts[i] = rom
            .get(off..off + len)
            .ok_or(AudioBuildError::Truncated(NAMES[i]))?;
    }
    let section = Section::build(&parts);
    let counts = check(&section)?;
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
    Ok(section)
}

/// Parses a built section; returns its counts in [`build_section`]'s order.
pub fn check(section: &[u8]) -> Result<[usize; 5], AudioBuildError> {
    let s = Section::open(section).map_err(|e| AudioBuildError::Parse("section", e))?;
    let seqs = fmt::parse_sbk(s.part(Part::Sbk)).map_err(|e| AudioBuildError::Parse("sbk", e))?;
    Bank::parse(s.part(Part::MusicCtl), s.part(Part::MusicTbl).len())
        .map_err(|e| AudioBuildError::Parse("music bank", e))?;
    let sfx = Bank::parse(s.part(Part::SfxCtl), s.part(Part::SfxTbl).len())
        .map_err(|e| AudioBuildError::Parse("sfx bank", e))?;
    let fgm_sounds = sfx
        .inst_array
        .first()
        .map_or(0, |&i| sfx.instruments[i as usize].sounds.len());
    let unk = fmt::fgm_unk_count(s.part(Part::FgmUnk))
        .map_err(|e| AudioBuildError::Parse("fgm.unk", e))?;
    let tbl = fmt::parse_package(s.part(Part::FgmTbl))
        .map_err(|e| AudioBuildError::Parse("fgm.tbl", e))?;
    let ucd = fmt::parse_package(s.part(Part::FgmUcd))
        .map_err(|e| AudioBuildError::Parse("fgm.ucd", e))?;
    Ok([seqs.len(), fgm_sounds, unk, tbl.len(), ucd.len()])
}
