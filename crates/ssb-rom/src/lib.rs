//! Reading of Super Smash Bros. 64 ROM data.
//!
//! Nothing in this crate embeds copyrighted material. It only describes *how*
//! to interpret a ROM the user already owns; the bytes stay on the user's disk.
//!
//! The crate is `no_std + alloc` so the same decoders can run on the PSP if we
//! ever want runtime loading, even though the intended path is build-time
//! conversion (see `tools/romtool`).

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod archive;
pub mod collision;
pub mod dl;
pub mod fighter;
pub mod mesh;
pub mod mobj;
pub mod pack;
pub mod psp_texture;
pub mod rom;
pub mod scan;
pub mod scene;
pub mod stage;
pub mod texture;
pub mod vpk0;

pub use archive::{Archive, TableEntry};
pub use rom::{Region, RomInfo};

/// Errors produced when reading ROM data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// The file is not the expected size for an N64 cartridge dump.
    BadRomSize(usize),
    /// The ROM is byte-swapped (`.v64`) or little-endian (`.n64`) rather than
    /// big-endian `.z64`.
    WrongByteOrder { detected: &'static str },
    /// The ROM's SHA-1 does not match any revision we support.
    UnsupportedRevision { sha1: alloc::string::String },
    /// A read ran past the end of the ROM.
    OutOfBounds { offset: usize, len: usize },
    /// VPK0 stream was malformed.
    Vpk0(&'static str),
    /// A relocation chain pointed outside its file.
    BadRelocation { file: u32, offset: usize },
    /// A display list command was not recognised.
    UnknownGbiOpcode(u8),
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Error::BadRomSize(n) => write!(f, "unexpected ROM size: {n} bytes"),
            Error::WrongByteOrder { detected } => write!(
                f,
                "ROM is in {detected} byte order; a big-endian .z64 dump is required"
            ),
            Error::UnsupportedRevision { sha1 } => {
                write!(f, "unsupported ROM revision (sha1 {sha1})")
            }
            Error::OutOfBounds { offset, len } => {
                write!(f, "read of {len} bytes at 0x{offset:X} is out of bounds")
            }
            Error::Vpk0(m) => write!(f, "malformed vpk0 stream: {m}"),
            Error::BadRelocation { file, offset } => {
                write!(f, "file {file} has a relocation pointing to 0x{offset:X}")
            }
            Error::UnknownGbiOpcode(op) => write!(f, "unknown GBI opcode 0x{op:02X}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}

pub type Result<T> = core::result::Result<T, Error>;
