//! `ssb-rom`'s ROM reader seen as `ssb-tablegen`'s (RE-588).
//!
//! `ssb-tablegen` compiles `ssb-rom`'s reader files through `#[path]`, so
//! its `RomRead` and `Error` are distinct types with the same shape; this
//! forwards one to the other, so the table generators read the paged ROM
//! instead of a 16 MiB copy.

use ssb_rom::rom_read::RomRef;
use ssb_tablegen::rom_read::{RomRead as TgRead, RomRef as TgRef};

/// A ROM reader for `ssb_tablegen::Source::over`.
pub(crate) struct Bridge<'a>(pub RomRef<'a>);

impl<'a> From<&'a Bridge<'a>> for TgRef<'a> {
    /// A slice stays zero-copy; a reader is forwarded.
    fn from(bridge: &'a Bridge<'a>) -> Self {
        match bridge.0 {
            RomRef::Bytes(bytes) => TgRef::Bytes(bytes),
            RomRef::Dyn(_) => TgRef::Dyn(bridge),
        }
    }
}

impl TgRead for Bridge<'_> {
    fn len(&self) -> usize {
        self.0.len()
    }

    fn read_at(&self, at: usize, out: &mut [u8]) -> ssb_tablegen::Result<()> {
        self.0.read_into(at, out).map_err(convert)
    }
}

/// The same variant in `ssb-tablegen`'s copy of the error type.
fn convert(e: ssb_rom::Error) -> ssb_tablegen::Error {
    use ssb_rom::Error as R;
    use ssb_tablegen::Error as T;
    match e {
        R::BadRomSize(n) => T::BadRomSize(n),
        R::WrongByteOrder { detected } => T::WrongByteOrder { detected },
        R::UnsupportedRevision { sha1 } => T::UnsupportedRevision { sha1 },
        R::OutOfBounds { offset, len } => T::OutOfBounds { offset, len },
        R::RomIo { offset, code } => T::RomIo { offset, code },
        R::Vpk0(m) => T::Vpk0(m),
        R::BadRelocation { file, offset } => T::BadRelocation { file, offset },
        R::UnknownGbiOpcode(op) => T::UnknownGbiOpcode(op),
    }
}
