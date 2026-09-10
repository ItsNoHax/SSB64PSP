//! Host-side reference model for `PLAN.md` R2.0/P0b: the real N64 RDP tile
//! **addressing** pipeline (`coordinate -> tile shift -> tile-origin-relative
//! -> mask/mirror/clamp`), as distinct from `n64_filter`'s reconstruction
//! (P0a). RE-218 found that R0.5's mirror/clamp/mask completion claims were
//! never checked against a real addressing model, only against reference
//! ports.
//!
//! Transcribed field-for-field from `angrylion-rdp-plus`
//! (<https://github.com/ata4/angrylion-rdp-plus>), a cycle-accurate,
//! hardware-validated RDP low-level emulator, not a reference port
//! (`BattleShip`/`sf64-psp`/`oot-PSP`/`n64psp`), per `AGENTS.md` §6 and
//! `DECISIONS.md` D-037:
//!
//! * `src/core/n64video.c`: the `SIGN16`/`TRELATIVE` macros
//!   (`#define SIGN16(x) ((int16_t)(x))`, `#define TRELATIVE(x, y) ((x) -
//!   ((y) << 3))`).
//! * `src/core/n64video/rdp/tcoord.c`: `tcshift_cycle` (per-axis
//!   `G_SETTILE` shift).
//! * `src/core/n64video/rdp/tex.c`: `calculate_tile_derivs`
//!   (`clampens = cs || !mask_s`), `calculate_clamp_diffs`, `tcclamp_cycle`
//!   (bounds check against the tile's drawn-rect far edge, `sh`/`th`), and
//!   `tcmask_coupled` (mask + mirror).
//!
//! Scope: only the sampled (`sample_type` set, i.e. `G_TF_BILERP`/CI+TLUT)
//! pipeline branch is modeled, matching RE-124's archive-wide finding that
//! every real `G_MDSFT_TEXTFILT` in this ROM requests `G_TF_BILERP` (151/151)
//! -- the "nearest, unfiltered" branch (`tcclamp_cycle_light`/plain `tcmask`)
//! never fires on real content. Only the RGB/I/IA texel-address path is
//! modeled, not the YUV `sfracrg` special case (RE-*: this ROM's textures are
//! never `FORMAT_YUV`).

/// One axis (`S` or `T`) of a real `G_SETTILE`/`G_SETTILESIZE` render-tile
/// state, in the RDP's own units.
#[derive(Debug, Clone, Copy)]
pub struct TileAxis {
    /// `G_SETTILE` `shift_s`/`shift_t`, 0..15. 0 means no shift; 1..10 is a
    /// right shift by that amount; 11..15 is a left shift by `16 - shift`.
    pub shift: u8,
    /// `G_SETTILESIZE` `uls`/`ult` (tile origin, `sl`/`tl`), S10.2
    /// (quarter-texel) fixed point.
    pub origin_q2: i32,
    /// `G_SETTILESIZE` `lrs`/`lrt` (tile far edge, `sh`/`th`), S10.2.
    pub far_edge_q2: i32,
    /// `G_SETTILE` `mask_s`/`mask_t`, 0..15. The texture repeats every
    /// `1 << mask` texels; 0 means "no wrap period" (RE-044).
    pub mask: u8,
    /// `G_TX_MIRROR`, bit 0 of `cms`/`cmt`.
    pub mirror: bool,
    /// `G_TX_CLAMP`, bit 1 of `cms`/`cmt`.
    pub clamp_bit: bool,
}

/// `SIGN16(x)`: truncate to 16 bits, then sign-extend.
fn sign16(x: i32) -> i32 {
    x as i16 as i32
}

/// `tcshift_cycle`'s per-axis shift, applied to the raw S10.5 coordinate
/// before anything else touches it.
fn tcshift(coord: i32, shift: u8) -> i32 {
    if shift < 11 {
        sign16(coord) >> shift
    } else {
        sign16(coord.wrapping_shl((16 - shift) as u32))
    }
}

/// Runs the real RDP addressing pipeline for one axis and returns the final
/// integer texel index the RDP fetches from TMEM (post shift, tile-origin
/// subtraction, clamp, mask and mirror) -- `tcshift_cycle` ->
/// `TRELATIVE` -> `tcclamp_cycle` -> `tcmask_coupled`, in that order, exactly
/// as `texture_pipeline_cycle` (`tex.c`) sequences them for a sampled texel.
///
/// `coord_s10_5` is the S10.5 fixed-point coordinate (`v.uv` in this crate's
/// own convention, `crates/ssb-rom/src/mesh.rs`'s `push_vertex` doc comment)
/// *before* any origin has been subtracted from it -- this function does
/// that subtraction itself via `TRELATIVE`, matching real hardware, which
/// subtracts unconditionally regardless of clamp/mirror.
pub fn address_axis(axis: &TileAxis, coord_s10_5: i32) -> i32 {
    let shifted = tcshift(coord_s10_5, axis.shift);
    // `maxs`/`maxt`: is the shifted-but-not-yet-origin-relative coordinate at
    // or past the tile's drawn-rect far edge? Compared in the tile's own
    // absolute S10.2 basis, same as `tile->sh` -- not origin-relative.
    let past_far_edge = (shifted >> 3) >= axis.far_edge_q2;
    // `TRELATIVE(x, y) = x - (y << 3)`: align S10.2 tile origin to the S10.5
    // coordinate scale (matches this crate's own `mesh.rs:1477-1488`
    // convention for the clamp-axis case; real hardware does this
    // unconditionally for every axis, clamped or not).
    let rel = shifted - (axis.origin_q2 << 3);
    // `calculate_tile_derivs`: `clampens = cs || !mask_s` -- a `mask == 0`
    // axis is *always* clamped by real hardware, regardless of the `cm`
    // clamp bit (`PLAN.md` R2.0/P0b's `mask == 0` question).
    let clamp_enabled = axis.clamp_bit || axis.mask == 0;
    let mut s = if clamp_enabled {
        if past_far_edge {
            // `calculate_clamp_diffs`: `((sh >> 2) - (sl >> 2)) & 0x3ff` --
            // the tile's drawn-rect width in whole texels, held as the
            // clamped edge index. `& 0x3ff` is a 10-bit hardware register
            // width; real archive tile widths never approach 1024 texels, so
            // this is included for fidelity but never observed to matter.
            ((axis.far_edge_q2 >> 2) - (axis.origin_q2 >> 2)) & 0x3ff
        } else if rel & 0x10000 == 0 {
            // Non-negative (this 17-bit-ish signed test's sign bit is clear):
            // drop the 5 fraction bits to get the integer texel index.
            rel >> 5
        } else {
            // Negative: hold the near edge.
            0
        }
    } else {
        rel >> 5
    };
    // `tcmask_coupled`: mask (and, on top of it, mirror) always runs after
    // clamp, including on an already-clamped value -- clamping only freezes
    // *which* value flows into the mask stage, it does not bypass masking.
    if axis.mask != 0 {
        let shift_amount = axis.mask.min(10);
        let maskbits = (1i32 << shift_amount) - 1;
        if axis.mirror {
            let wrap = (s >> shift_amount) & 1;
            s ^= -wrap;
        }
        s &= maskbits;
    }
    s
}

/// The mask+mirror fold for a non-negative, already-clamped-or-in-range raw
/// texel offset, in closed (non-bit-twiddled) form: unflipped on an even
/// period index, reversed on an odd one. Equivalent to [`address_axis`]'s
/// own `tcmask_coupled` bit-twiddling for every non-negative `s` real
/// content produces (real archive periods never approach the 10-bit
/// register width `address_axis`'s `shift_amount = axis.mask.min(10)`
/// guards against) -- kept as a separate closed form here because
/// [`texture::mirror_extend`](crate::texture::mirror_extend) needs the same
/// fold to actually bake pixels, not just compute one index.
fn fold_period_mirror(s: i32, period: i32, mirror: bool) -> i32 {
    if period <= 0 {
        return 0;
    }
    let phase = s.rem_euclid(period);
    if mirror && s.div_euclid(period) % 2 != 0 {
        period - 1 - phase
    } else {
        phase
    }
}

/// Models the *PSP lowering* for one mirrored axis, for direct comparison
/// against [`address_axis`]: [`texture::mirror_extend`](crate::texture::mirror_extend)
/// pre-bakes a mirrored image and `sceGuTexWrap` addresses it directly.
///
/// A mirrored axis with no clamp bakes exactly one mirrored pair, which a
/// plain `Repeat` wrap then mirrors forever, exactly. A mirrored *and*
/// clamped axis (`PLAN.md` R2.0/P0c, RE-220/RE-221) bakes every period up
/// to `drawn` (the tile's own drawn-rect extent on this axis,
/// `TextureRef::drawn_width`/`drawn_height`) instead of just the first
/// mirrored pair, so `sceGuTexWrap(Clamp)` then holds exactly the real
/// far-edge texel forever -- matching [`address_axis`]'s own clamp target
/// (the drawn rect's far edge, folded through the same mask/mirror stage)
/// for every coordinate a real drawn primitive can reach.
///
/// Returns an index into the *original* (pre-mirror-baking) image, so it is
/// directly comparable to [`address_axis`]'s result: both name which texel
/// of the real decoded texture ends up sampled.
///
/// `coord_rel_s10_5` must already be relative to the tile origin, matching
/// what `mesh::Builder::push_vertex` bakes into `MeshVertex::uv` for a
/// clamped axis (RE-152) -- the same basis [`address_axis`] uses when called
/// with `origin_q2: 0`.
pub fn psp_lowering_axis(coord_rel_s10_5: i32, period: u32, drawn: u32, mirror: bool, clamp: bool) -> i32 {
    let period = period as i32;
    let raw_index = coord_rel_s10_5.div_euclid(32);
    if !mirror {
        return if clamp {
            raw_index.clamp(0, period - 1)
        } else {
            raw_index.rem_euclid(period)
        };
    }
    if !clamp {
        let image_width = period * 2;
        let idx = raw_index.rem_euclid(image_width);
        return if idx >= period { 2 * period - 1 - idx } else { idx };
    }
    let last = (drawn.max(1) as i32) - 1;
    fold_period_mirror(raw_index.clamp(0, last), period, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn axis(mask: u8, mirror: bool, clamp_bit: bool, origin_q2: i32, far_edge_q2: i32) -> TileAxis {
        TileAxis {
            shift: 0,
            origin_q2,
            far_edge_q2,
            mask,
            mirror,
            clamp_bit,
        }
    }

    /// A 32-texel-wide tile (`mask = 5`), drawn rect exactly one period wide
    /// (`sh - sl = 31 texels`, S10.2), no mirror, no clamp bit: plain repeat.
    /// Coordinates one and two periods past the origin land back at the same
    /// phase as the origin itself.
    #[test]
    fn plain_repeat_wraps_every_period() {
        let a = axis(5, false, false, 0, 31 << 2);
        assert_eq!(address_axis(&a, 0), 0);
        assert_eq!(address_axis(&a, 5 << 5), 5);
        assert_eq!(address_axis(&a, (32 + 5) << 5), 5, "one period past: same phase");
        assert_eq!(
            address_axis(&a, (3 * 32 + 5) << 5),
            5,
            "three periods past: still same phase"
        );
    }

    /// Mirror (no clamp): odd periods read the period backwards.
    #[test]
    fn mirror_flips_every_odd_period() {
        let a = axis(5, true, false, 0, 31 << 2);
        assert_eq!(address_axis(&a, 5 << 5), 5, "first period: unflipped");
        assert_eq!(
            address_axis(&a, (32 + 5) << 5),
            31 - 5,
            "second period: flipped"
        );
        assert_eq!(
            address_axis(&a, (2 * 32 + 5) << 5),
            5,
            "third period: unflipped again"
        );
        assert_eq!(
            address_axis(&a, (3 * 32 + 5) << 5),
            31 - 5,
            "fourth period: flipped"
        );
    }

    /// `PLAN.md` R2.0/P0b bullet 1: a drawn rect spanning *four* mask
    /// periods (`sh - sl = 127 texels` over a 32-texel mask) with
    /// mirror+clamp (`cms == 3`). Real hardware keeps mirroring at every
    /// period boundary all the way to the drawn-rect edge, not just through
    /// the first mirrored pair -- clamping only takes over once the
    /// coordinate reaches `sh` itself.
    #[test]
    fn mirror_plus_clamp_mirrors_through_every_period_not_just_the_first() {
        let a = axis(5, true, true, 0, 127 << 2);
        // Third period (past the *first* mirrored pair, still short of the
        // 127-texel far edge): a naive "mirror once then clamp" model would
        // have already clamped here. Real hardware has not.
        assert_eq!(
            address_axis(&a, (2 * 32 + 5) << 5),
            5,
            "third period still mirrors, unflipped phase"
        );
        assert_eq!(
            address_axis(&a, (3 * 32 + 5) << 5),
            31 - 5,
            "fourth period still mirrors, flipped phase"
        );
        // Past the drawn-rect far edge: now it clamps, to the drawn width
        // itself (127), which mask+mirror still folds.
        let clamped = address_axis(&a, 200 << 5);
        assert_eq!(clamped, address_axis(&a, 500 << 5), "clamp holds one fixed index");
    }

    /// `PLAN.md` R2.0/P0c (RE-221): at the *third* mask period (past the
    /// first mirrored pair, still short of the drawn-rect far edge), the
    /// fixed PSP lowering model now agrees with real hardware -- pre-fix, a
    /// mirror-double + `sceGuTexWrap(Clamp)` model would have already
    /// clamped here (RE-220's measured divergence), because it only ever
    /// baked two periods regardless of how far the drawn rect actually
    /// extends. This axis's drawn rect is 128 texels wide (`sh = 127`, one
    /// past the fourth 32-texel period boundary), so the fixed model bakes
    /// four periods, not two.
    #[test]
    fn psp_lowering_no_longer_diverges_from_hardware_past_the_first_mirrored_period() {
        let a = axis(5, true, true, 0, 127 << 2);
        let coord = (2 * 32 + 5) << 5; // third period, real hardware: still mirroring
        let hw = address_axis(&a, coord);
        let psp = psp_lowering_axis(coord, 1 << 5, 128, true, true);
        assert_eq!(hw, 5, "hardware: third period unflipped");
        assert_eq!(psp, 5, "fixed PSP model: also still mirroring, not yet clamped");
    }

    /// The fixed PSP lowering model matches real hardware at every period a
    /// 128-texel drawn rect spans over a 32-texel mask -- not just the
    /// first mirrored pair the old two-period bake covered.
    #[test]
    fn psp_lowering_matches_hardware_through_every_period_the_drawn_rect_spans() {
        let a = axis(5, true, true, 0, 127 << 2);
        for texel in [0i32, 5, 31, 32 + 5, 2 * 32 + 5, 3 * 32 + 5, 3 * 32 + 31] {
            let coord = texel << 5;
            assert_eq!(
                address_axis(&a, coord),
                psp_lowering_axis(coord, 1 << 5, 128, true, true),
                "texel {texel}"
            );
        }
        // Past the drawn-rect far edge: both models now clamp to the same
        // held index.
        let coord = 500 << 5;
        assert_eq!(
            address_axis(&a, coord),
            psp_lowering_axis(coord, 1 << 5, 128, true, true),
            "past the far edge"
        );
    }

    /// `PLAN.md` R2.0/P0b bullet 2: `mask == 0` is *always* clamped by real
    /// hardware (`clampens = cs || !mask_s`), even when the `cm` clamp bit
    /// itself is clear (`cms` requesting plain wrap/mirror with no period).
    #[test]
    fn mask_zero_clamps_even_without_the_clamp_bit() {
        // 24-texel-wide tile (sh - sl = 23), mask = 0, clamp bit *not* set.
        let a = axis(0, false, false, 0, 23 << 2);
        // Well past the far edge: a naive "mask==0 means no bound at all"
        // model would let this run away; real hardware holds it at 23.
        assert_eq!(address_axis(&a, 1000 << 5), 23);
        // Inside bounds: passes through unclamped-looking, i.e. exact.
        assert_eq!(address_axis(&a, 10 << 5), 10);
    }

    /// Negative coordinates on a clamped axis hold the near edge (index 0),
    /// not a wrapped/negative index.
    #[test]
    fn clamp_holds_the_near_edge_for_negative_coordinates() {
        let a = axis(0, false, true, 0, 23 << 2);
        assert_eq!(address_axis(&a, -100 << 5), 0);
    }

    /// A nonzero tile origin shifts the whole pipeline: `TRELATIVE`
    /// subtracts it before clamp/mask ever see the coordinate.
    #[test]
    fn nonzero_origin_shifts_the_addressed_texel() {
        // Origin at texel 4 (`origin_q2 = 4 << 2`), 16-wide clamped tile.
        let a = axis(0, false, true, 4 << 2, (4 + 15) << 2);
        assert_eq!(address_axis(&a, (4 + 6) << 5), 6);
        assert_eq!(address_axis(&a, 0), 0, "before the origin clamps to the near edge");
    }
}
