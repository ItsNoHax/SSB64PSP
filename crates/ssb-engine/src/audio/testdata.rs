//! Test support: the audio section built from the user's ROM.

extern crate std;

use std::sync::OnceLock;

use super::data::AudioData;

/// The section built from `SSB64_ROM`, or `None` (the test skips) when the
/// variable is unset.
pub fn section() -> Option<&'static [u8]> {
    static SECTION: OnceLock<Option<&'static [u8]>> = OnceLock::new();
    *SECTION.get_or_init(|| {
        let path = std::env::var_os("SSB64_ROM")?;
        let rom = std::fs::read(path).ok()?;
        let bytes = ssb_rom::audio::build_section(&rom).expect("audio section");
        Some(std::boxed::Box::leak(bytes.into_boxed_slice()))
    })
}

/// The parsed section, or `None` without a ROM.
pub fn data() -> Option<AudioData> {
    Some(AudioData::new(section()?).expect("parse audio section"))
}
