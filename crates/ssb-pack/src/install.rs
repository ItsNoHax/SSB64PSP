//! The first-launch installer's portable half (RE-588, D-048): which file
//! is the ROM, how far the build has got, whether the Memory Stick has
//! room, and what each failure tells the player. The PSP side
//! (`psp-runtime/src/installer`) does the file I/O and the screen.

use alloc::string::String;
use core::fmt;

use crate::PackError;

/// The ROM names the installer looks for first, in this order.
pub const ROM_NAMES: [&str; 3] = ["rom.z64", "rom.v64", "rom.n64"];

/// The dump extensions a lone ROM file may have (`ssb_rom::rom::ByteOrder`).
pub const ROM_EXTENSIONS: [&str; 3] = ["z64", "v64", "n64"];

/// Which entry of a directory listing is the ROM.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RomPick {
    /// One of [`ROM_NAMES`] (the first present, in their order).
    Named(usize),
    /// The only file with one of [`ROM_EXTENSIONS`].
    Single(usize),
    /// This many files with a ROM extension, none named as in
    /// [`ROM_NAMES`]: the player must say which.
    Several(usize),
    /// None.
    None,
}

/// Whether `name` ends in `.z64`, `.v64` or `.n64` (any case).
pub fn has_rom_extension(name: &str) -> bool {
    name.rsplit_once('.').is_some_and(|(stem, ext)| {
        !stem.is_empty() && ROM_EXTENSIONS.iter().any(|e| e.eq_ignore_ascii_case(ext))
    })
}

/// The ROM among a directory's file names (FAT: any case).
pub fn pick_rom(names: &[&str]) -> RomPick {
    for fixed in ROM_NAMES {
        if let Some(i) = names.iter().position(|n| n.eq_ignore_ascii_case(fixed)) {
            return RomPick::Named(i);
        }
    }
    let mut first = None;
    let mut count = 0;
    for (i, name) in names.iter().enumerate() {
        if has_rom_extension(name) {
            count += 1;
            first.get_or_insert(i);
        }
    }
    match (count, first) {
        (1, Some(i)) => RomPick::Single(i),
        (0, _) | (_, None) => RomPick::None,
        (n, _) => RomPick::Several(n),
    }
}

/// One build phase: the [`crate::PackLog::phase`] name that ends it, what
/// the screen calls it, and its share of the build.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Phase {
    pub ends_at: &'static str,
    pub label: &'static str,
    /// Centiseconds on a PSP-2000 at the PSP-1000 budget (RE-588 item 14).
    pub weight: u32,
}

/// The build's phases, in order (`build.rs`'s `phase` calls).
pub const PHASES: [Phase; 9] = [
    Phase {
        ends_at: "tables",
        label: "Checking the ROM, reading tables",
        weight: 3843,
    },
    Phase {
        ends_at: "load_all",
        label: "Loading the ROM's files",
        weight: 1841,
    },
    Phase {
        ends_at: "meshes",
        label: "Converting models and textures",
        weight: 29124,
    },
    Phase {
        ends_at: "objects+stages",
        label: "Placing objects and stages",
        weight: 32,
    },
    Phase {
        ends_at: "anims",
        label: "Converting animations",
        weight: 5263,
    },
    Phase {
        ends_at: "particles+interface",
        label: "Converting effects and the interface",
        weight: 4156,
    },
    Phase {
        ends_at: "menus+deps",
        label: "Linking the menus",
        weight: 1524,
    },
    Phase {
        ends_at: "audio",
        label: "Checking the audio",
        weight: 7,
    },
    Phase {
        ends_at: "finish",
        label: "Writing ssb64.pak",
        weight: 6811,
    },
];

/// How far the build has got: phases ended and steps inside the running
/// one ([`crate::PackLog::progress`], or the installer's bytes written).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Progress {
    ended: usize,
    done: usize,
    total: usize,
}

impl Progress {
    /// Nothing done yet (`Default`, usable in a `static`).
    pub const fn new() -> Self {
        Self {
            ended: 0,
            done: 0,
            total: 0,
        }
    }

    /// `ended` phases ended and `done` of `total` steps of the next (the
    /// installer keeps the three in atomics).
    pub fn at(ended: usize, done: usize, total: usize) -> Self {
        Self {
            ended: ended.min(PHASES.len()),
            done: done.min(total),
            total,
        }
    }

    /// Phases ended so far.
    pub fn ended(&self) -> usize {
        self.ended
    }

    /// `(done, total)` steps of the running phase; `total` 0 when it
    /// reports none.
    pub fn steps(&self) -> (usize, usize) {
        (self.done, self.total)
    }

    /// The phase named `name` ended (an unknown name changes nothing but
    /// the steps).
    pub fn phase_ended(&mut self, name: &str) {
        if let Some(i) = PHASES.iter().position(|p| p.ends_at == name) {
            self.ended = self.ended.max(i + 1);
        }
        self.done = 0;
        self.total = 0;
    }

    /// `done` of `total` steps of the running phase.
    pub fn step(&mut self, done: usize, total: usize) {
        self.done = done.min(total);
        self.total = total;
    }

    /// The running phase and its index; `None` once every phase ended.
    pub fn current(&self) -> Option<(usize, &'static Phase)> {
        PHASES.get(self.ended).map(|p| (self.ended, p))
    }

    /// Thousandths of the whole build, by [`Phase::weight`]: never down,
    /// and 1000 only once every phase ended.
    pub fn per_mille(&self) -> u32 {
        let total: u64 = PHASES.iter().map(|p| u64::from(p.weight)).sum();
        let before: u64 = PHASES[..self.ended]
            .iter()
            .map(|p| u64::from(p.weight))
            .sum();
        let Some((_, phase)) = self.current() else {
            return 1000;
        };
        let within = if self.total == 0 {
            0
        } else {
            u64::from(phase.weight) * self.done as u64 / self.total as u64
        };
        ((before + within) * 1000 / total).min(999) as u32
    }
}

/// `sceIoDevctl(dev, 0x02425818)`'s answer: the Memory Stick's size in
/// clusters (pspsdk `SceIoDevInfo`; the call takes a pointer to it).
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DevInfo {
    pub max_clusters: u32,
    pub free_clusters: u32,
    pub max_sectors: u32,
    pub sector_size: u32,
    pub sector_count: u32,
}

impl DevInfo {
    /// Free bytes: clusters of `sector_count` sectors of `sector_size`.
    pub fn free_bytes(&self) -> u64 {
        u64::from(self.free_clusters) * u64::from(self.sector_count) * u64::from(self.sector_size)
    }
}

/// Free bytes the build needs: the pack (46.8 MB at v148), the blob spill
/// (14.1 MB, removed after the build) and a margin (RE-588 item 14).
pub const SPACE_NEEDED: u64 = 80_000_000;

/// About the pack's size (46,768,296 B at v148): the installer's progress
/// through the final write.
pub const PACK_BYTES: u64 = 46_800_000;

/// Whether the Memory Stick has room for the build.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Space {
    Enough,
    /// Only once the old pack (another version, or cut short: useless to
    /// this EBOOT) is removed first.
    AfterRemovingOld,
    Short {
        need: u64,
        free: u64,
    },
}

/// [`Space`] for `free` bytes free and an old pack of `old_pack` bytes.
pub fn plan_space(free: u64, old_pack: u64) -> Space {
    if free >= SPACE_NEEDED {
        Space::Enough
    } else if free.saturating_add(old_pack) >= SPACE_NEEDED {
        Space::AfterRemovingOld
    } else {
        Space::Short {
            need: SPACE_NEEDED,
            free,
        }
    }
}

/// `SCE_ERROR_ERRNO_ENOSPC`: a write to a full Memory Stick.
pub const ERRNO_ENOSPC: i32 = 0x8001_001C_u32 as i32;

/// Why the install stopped; [`fmt::Display`] is the screen's text, one
/// line per `\n`, at most 78 columns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallError {
    NoRom,
    SeveralRoms {
        count: usize,
    },
    RomOpen {
        code: i32,
    },
    /// No known N64 dump byte order.
    NotARom,
    RomSize {
        bytes: u64,
    },
    /// Another game, region or revision.
    WrongRom {
        sha1: String,
    },
    NoSpace {
        need: u64,
        free: u64,
    },
    /// A write failed with the Memory Stick full.
    DiskFull,
    Write {
        what: &'static str,
        code: i32,
    },
    OutOfMemory,
    Build(String),
    Panic(String),
}

impl InstallError {
    /// A failed write of `what` (`code` from `sceIoWrite`/`sceIoOpen`).
    pub fn write(what: &'static str, code: i32) -> Self {
        if code == ERRNO_ENOSPC {
            Self::DiskFull
        } else {
            Self::Write { what, code }
        }
    }

    /// A failed `crate::build_with`: the ROM's own errors by kind, the
    /// rest as the build's message.
    pub fn from_build(e: &PackError) -> Self {
        match e.downcast_ref::<ssb_rom::Error>() {
            Some(e) => Self::from_rom(e),
            None => Self::Build(alloc::format!("{e}")),
        }
    }

    /// A ROM error (`ssb_rom::rom_read::PagedRom::new`, the build's ROM
    /// check) by kind.
    pub fn from_rom(e: &ssb_rom::Error) -> Self {
        use ssb_rom::Error as R;
        match e {
            R::BadRomSize(n) => Self::RomSize { bytes: *n as u64 },
            R::WrongByteOrder { .. } => Self::NotARom,
            R::UnsupportedRevision { sha1 } => Self::WrongRom { sha1: sha1.clone() },
            R::RomIo { code, .. } => Self::RomOpen { code: *code },
            _ => Self::Build(alloc::format!("{e}")),
        }
    }
}

const ROM_HELP: &str = "Copy your Super Smash Bros. (USA) ROM next to EBOOT.PBP\n\
     (ms0:/PSP/GAME/ssb64/ on a standard install) and name it rom.z64.\n\
     A .v64 or .n64 dump works too: name it rom.v64 or rom.n64.";

impl fmt::Display for InstallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        const MB: u64 = 1_000_000;
        let us = ssb_rom::rom::Region::Us.sha1();
        match self {
            Self::NoRom => write!(
                f,
                "No ROM found.\n\n{ROM_HELP}\n\nSearched: the EBOOT's folder, ms0:/PSP/GAME/ssb64/ and ms0:/."
            ),
            Self::SeveralRoms { count } => write!(
                f,
                "{count} ROM files found and none is named rom.z64.\n\n\
                 Rename the one to use to rom.z64 (or rom.v64, rom.n64)."
            ),
            Self::RomOpen { code } => write!(
                f,
                "The ROM could not be read (error {:#010x}).\n\n\
                 Check the Memory Stick, or copy the ROM again.",
                *code as u32
            ),
            Self::NotARom => write!(
                f,
                "This file is not an N64 ROM dump.\n\n{ROM_HELP}"
            ),
            Self::RomSize { bytes } => write!(
                f,
                "Wrong ROM size: {bytes} bytes.\n\n\
                 Super Smash Bros. (USA) is 16 MiB (16,777,216 bytes).\n\n{ROM_HELP}"
            ),
            Self::WrongRom { sha1 } => write!(
                f,
                "This ROM is not Super Smash Bros. (USA), revision 1.0.\n\n\
                 A US 1.0 ROM is required.\n\
                 Required SHA-1: {us}\n\
                 This ROM's SHA-1: {sha1}"
            ),
            Self::NoSpace { need, free } => write!(
                f,
                "Not enough free space on the Memory Stick.\n\n\
                 The setup needs {} MB free; {} MB are free.\n\
                 Free some space, then start the game again.",
                need.div_ceil(MB),
                free / MB
            ),
            Self::DiskFull => write!(
                f,
                "The Memory Stick is full.\n\n\
                 The setup needs {} MB free. Free some space, then start the\n\
                 game again.",
                SPACE_NEEDED / MB
            ),
            Self::Write { what, code } => write!(
                f,
                "Could not write {what} (error {:#010x}).\n\n\
                 Check the Memory Stick (not locked, not full), then start the\n\
                 game again.",
                *code as u32
            ),
            Self::OutOfMemory => write!(
                f,
                "Not enough memory for the setup.\n\n\
                 Turn off plugins, then start the game again."
            ),
            Self::Build(msg) => write!(f, "The game data could not be built:\n{msg}"),
            Self::Panic(msg) => write!(f, "Internal error:\n{msg}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;
    use alloc::vec::Vec;

    #[test]
    fn picks_the_named_rom_first_then_a_lone_dump() {
        assert_eq!(
            pick_rom(&["EBOOT.PBP", "ROM.N64", "rom.v64"]),
            RomPick::Named(2)
        );
        assert_eq!(pick_rom(&["rom.n64", "a.z64"]), RomPick::Named(0));
        assert_eq!(
            pick_rom(&["EBOOT.PBP", "Super Smash Bros. (USA).z64"]),
            RomPick::Single(1)
        );
        assert_eq!(pick_rom(&["x.V64"]), RomPick::Single(0));
        assert_eq!(pick_rom(&["a.z64", "b.n64", "c.txt"]), RomPick::Several(2));
        assert_eq!(
            pick_rom(&["EBOOT.PBP", "ssb64.pak", ".z64", "z64", "rom.z64.txt"]),
            RomPick::None
        );
        assert_eq!(pick_rom(&[]), RomPick::None);
    }

    #[test]
    fn progress_rises_to_1000_only_at_the_end() {
        let mut p = Progress::default();
        assert_eq!(p.per_mille(), 0);
        assert_eq!(p.current().map(|(i, _)| i), Some(0));
        let mut last = 0;
        for phase in PHASES {
            for done in 0..=10 {
                p.step(done, 10);
                let now = p.per_mille();
                assert!(
                    now >= last && now < 1000,
                    "{} {done}: {now} < {last}",
                    phase.ends_at
                );
                last = now;
            }
            p.phase_ended(phase.ends_at);
            assert!(p.per_mille() >= last);
            last = p.per_mille();
        }
        assert_eq!(p.per_mille(), 1000);
        assert_eq!(p.current(), None);
        // Meshes is the long phase: half its files is a quarter of the build.
        let mut p = Progress::default();
        p.phase_ended("load_all");
        p.step(1, 2);
        assert!((350..=450).contains(&p.per_mille()), "{}", p.per_mille());
        // An unknown phase name only resets the steps.
        p.phase_ended("rom");
        assert_eq!(p.current().map(|(i, _)| i), Some(2));
    }

    /// The phase names are `build.rs`'s, in its order.
    #[test]
    fn phases_name_the_builds_phase_calls_in_order() {
        let src = include_str!("build.rs");
        let mut at = 0;
        for phase in PHASES {
            let call = alloc::format!("ctx.log.phase(\"{}\")", phase.ends_at);
            let found = src[at..]
                .find(&call)
                .unwrap_or_else(|| panic!("{call} not after {at}"));
            at += found + call.len();
        }
        let calls = src.matches("ctx.log.phase(").count();
        assert_eq!(calls, PHASES.len());
    }

    #[test]
    fn space_counts_the_old_pack_only_when_it_must_go() {
        assert_eq!(plan_space(SPACE_NEEDED, 0), Space::Enough);
        assert_eq!(plan_space(40_000_000, 46_768_296), Space::AfterRemovingOld);
        assert_eq!(
            plan_space(40_000_000, 0),
            Space::Short {
                need: SPACE_NEEDED,
                free: 40_000_000
            }
        );
        let info = DevInfo {
            max_clusters: 30_000,
            free_clusters: 2_000,
            max_sectors: 0,
            sector_size: 512,
            sector_count: 64,
        };
        assert_eq!(info.free_bytes(), 2_000 * 64 * 512);
        assert_eq!(core::mem::size_of::<DevInfo>(), 20);
    }

    #[test]
    fn errors_name_their_cause_on_screen() {
        let wrong =
            InstallError::from_build(&PackError::from(ssb_rom::Error::UnsupportedRevision {
                sha1: "00".repeat(20),
            }));
        let text = wrong.to_string();
        assert!(matches!(wrong, InstallError::WrongRom { .. }));
        assert!(
            text.contains("US 1.0 ROM is required")
                && text.contains(ssb_rom::rom::Region::Us.sha1())
        );
        assert_eq!(
            InstallError::from_build(&PackError::from(ssb_rom::Error::BadRomSize(5))),
            InstallError::RomSize { bytes: 5 }
        );
        assert_eq!(
            InstallError::from_build(&PackError::from(ssb_rom::Error::WrongByteOrder {
                detected: "x"
            })),
            InstallError::NotARom
        );
        assert_eq!(
            InstallError::from_build(&PackError::from(ssb_rom::Error::RomIo {
                offset: 0,
                code: -5
            })),
            InstallError::RomOpen { code: -5 }
        );
        assert!(matches!(
            InstallError::from_build(&PackError::from("tables: x")),
            InstallError::Build(m) if m == "tables: x"
        ));
        assert_eq!(
            InstallError::write("ssb64.pak", ERRNO_ENOSPC),
            InstallError::DiskFull
        );
        assert!(matches!(
            InstallError::write("ssb64.pak", -1),
            InstallError::Write { .. }
        ));
        let all = [
            InstallError::NoRom,
            InstallError::SeveralRoms { count: 2 },
            InstallError::RomOpen { code: -1 },
            InstallError::NotARom,
            InstallError::RomSize { bytes: 1 },
            wrong,
            InstallError::NoSpace {
                need: SPACE_NEEDED,
                free: 1,
            },
            InstallError::DiskFull,
            InstallError::Write {
                what: "ssb64.pak",
                code: -1,
            },
            InstallError::OutOfMemory,
            InstallError::Build("x".into()),
            InstallError::Panic("x".into()),
        ];
        for e in all {
            let text = e.to_string();
            let lines: Vec<&str> = text.lines().collect();
            assert!(lines.len() <= 12, "{text}");
            assert!(lines.iter().all(|l| l.len() <= 78), "{text}");
        }
        assert!(InstallError::NoRom.to_string().contains("rom.z64"));
        assert!(InstallError::NoSpace {
            need: SPACE_NEEDED,
            free: 0
        }
        .to_string()
        .contains("80 MB"));
    }
}
