//! The pack's `TABLES` section (D-048, `ssb-tables`): read once at boot
//! into a leaked cache-line-aligned buffer and installed for every
//! `ssb_tables::Table`, before the first scene reads one.

use psp::sys;

use crate::assets::LoadError;

/// Why the section was not installed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TablesError {
    /// The pack's header names no section (`ssb_rom::pack::tables_range`).
    Missing,
    Load(LoadError),
    Install(ssb_tables::InstallError),
}

impl TablesError {
    pub fn as_str(self) -> &'static str {
        match self {
            TablesError::Missing => "the pack has no tables section",
            TablesError::Load(e) => e.as_str(),
            TablesError::Install(ssb_tables::InstallError::AlreadyInstalled) => {
                "tables already installed"
            }
            TablesError::Install(ssb_tables::InstallError::Section(_)) => {
                "the tables section is malformed"
            }
            TablesError::Install(ssb_tables::InstallError::Table(_)) => {
                "a table differs from this EBOOT's layout"
            }
        }
    }
}

/// Reads the section (`ssb_rom::pack::tables_range`, from the header in
/// `head`) from `path` (NUL-terminated, as `assets::c_path` returns) and
/// installs it. The buffer is never freed. Returns the section's length.
///
/// # Safety
///
/// No `ssb_tables::Table` is read during the call, on any thread
/// (`ssb_tables::install`): call it at boot, before the first scene.
pub unsafe fn load(path: &'static str, head: &[u8]) -> Result<usize, TablesError> {
    let (offset, len) = ssb_rom::pack::tables_range(head).ok_or(TablesError::Missing)?;
    // SAFETY: path is NUL-terminated.
    let fd = unsafe { sys::sceIoOpen(path.as_ptr(), sys::IoOpenFlags::RD_ONLY, 0o777) };
    if fd.0 < 0 {
        return Err(TablesError::Load(LoadError::NotFound));
    }
    let bytes = crate::audio::read_leaked(fd, offset, len);
    // SAFETY: `fd` is open.
    unsafe { sys::sceIoClose(fd) };
    let bytes = bytes.map_err(TablesError::Load)?;
    // SAFETY: the caller excludes table reads (this function's contract).
    unsafe { ssb_tables::install(bytes) }.map_err(TablesError::Install)?;
    // SAFETY: as above; nothing has read a hot value yet (RE-589).
    unsafe { ssb_game::tables::fill() };
    Ok(len)
}
