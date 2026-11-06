//! What the packer reports to an observer while it converts textures: the
//! `romtool residuals` audit records every packed variant and its use sites
//! through [`PackObserver`]. Every hook only receives values the converter
//! already computed, so an observer cannot change the pack's bytes.

use alloc::vec::Vec;

use ssb_rom::filter_compensation::AlphaPolicy;
use ssb_rom::psp_texture::{Psm, PspTexture};
use ssb_rom::texture::Rgba8;

use crate::filter_coverage::Coverage;

/// A material-animation index field that one variant shares across palettes.
pub struct Animated {
    pub source_indices: Vec<u8>,
    pub final_indices: Vec<u8>,
    pub states: Vec<Vec<u32>>,
}

/// What one `convert_texture` call packed.
pub struct Conversion {
    pub texture: ssb_rom::mesh::TextureRef,
    /// File the display list came from; resolves `texture.data_file == None`.
    pub home: u32,
    /// Decoded, mirror-extended N64 texels: the 3-point reference input.
    pub source: Rgba8,
    /// Level-0 texels of the packed variant (static palette for indexed).
    pub final_level0: Rgba8,
    pub source_psm: Psm,
    pub final_psm: Psm,
    pub palette: Vec<u32>,
    pub animated: Option<Animated>,
    pub method: &'static str,
    /// Whether the compensator was run for this conversion at all.
    pub attempted: bool,
    pub policy: AlphaPolicy,
}

/// One primitive that binds a packed variant.
pub struct UseSite {
    pub variant: u32,
    pub file: u32,
    pub dl: u32,
    pub prim: usize,
    pub sprite_slot: Option<usize>,
    pub policy: AlphaPolicy,
    /// `Some(linear)` for `G_TEXTURE_GEN`.
    pub texgen: Option<bool>,
    pub mat_anim: bool,
    pub triangles: usize,
    pub coverage: Coverage,
    /// Packed primitive's RE-312 phase; zero for ordinary material state.
    pub phase: [i16; 2],
}

/// Receives the converter's texture records. Every method defaults to a
/// no-op; [`PackObserver::enabled`] is `false` unless an implementation
/// records, and the packer skips the work that only feeds a record (filter
/// coverage of uncompensated textures, source decodes) when it is `false`.
pub trait PackObserver {
    /// Whether the observer records anything.
    fn enabled(&self) -> bool {
        false
    }

    /// One conversion is pending until [`PackObserver::record_variant`]
    /// files it under its variant, or [`PackObserver::discard_conversion`]
    /// drops it.
    fn record_conversion(&self, _conversion: Conversion) {}

    /// The pending conversion was deduplicated into an existing variant, or
    /// failed.
    fn discard_conversion(&self) {}

    /// The pending conversion became packed variant `index`.
    fn record_variant(&self, _index: u32, _packed: &PspTexture, _clamp_s: bool, _clamp_t: bool) {}

    /// One primitive binds a packed variant.
    fn record_site(&self, _site: UseSite) {}
}

/// The observer that records nothing.
pub struct NoObserver;

impl PackObserver for NoObserver {}
