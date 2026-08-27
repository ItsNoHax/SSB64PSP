//! Per-character constants (`FTAttributes`).
//!
//! Every number that distinguishes one fighter from another — how fast Mario
//! walks, how hard Jigglypuff falls, how wide Donkey Kong's body is — lives in
//! one struct at a fixed offset inside that character's *main* archive file.
//! `ftManagerSetupFighter` reaches it as:
//!
//! ```c
//! attr = lbRelocGetFileData(FTAttributes*, *fp->data->p_file_main, fp->data->o_attributes);
//! ```
//!
//! so the pairing that matters is `(file id, byte offset)`, and it is held in
//! `dFT<Name>Data`, a struct that lives in the *game code's* data segment
//! rather than in any archive file. That segment is not something this crate
//! reads, so [`FIGHTER_FILES`] carries the pairing instead.
//!
//! ## Where the offsets come from, and why they can be trusted
//!
//! They are transcribed from the decompilation's `relocData` sources, which
//! annotate each fighter main file with the size of everything preceding the
//! attribute struct. That is a record naming both sides, not a value guessed
//! from what looked plausible in a hex dump.
//!
//! It is still a transcription, so it is checked rather than assumed:
//! `romtool fighters --verify` decodes all 27 fighters out of the ROM and
//! compares every scalar against the values the decompilation lists in its own
//! C literals. Two independent readings of the same bytes — one from the
//! compressed archive, one hand-written years ago by somebody else — agreeing
//! on 45 fields each is what makes an offset table believable. A wrong offset
//! does not produce 44 matches and one miss; it produces garbage.
//!
//! ## The units are not small
//!
//! Mario's gravity is `2.4` and his terminal velocity `44.0`, per *frame*.
//! Smash 64 works in the same large world units as its collision geometry,
//! where a stage spans several thousand units and Mario stands 320 tall. Any
//! "sensible-looking" small constant (`0.09` gravity, say) is off by more than
//! an order of magnitude and will look almost right while being wrong — the
//! fighter falls, just thirty times too slowly.

use alloc::vec::Vec;

use crate::archive::{Archive, File};

/// Number of `f32`/`s32` scalars decoded from the head of `FTAttributes`.
///
/// The struct continues past these with hurtbox descriptors, sound ids, joint
/// indices and pointers. Those are separate subsystems' data and are left
/// where they are; the scalars here are the ones physics, collision and the
/// camera read.
pub const SCALAR_COUNT: usize = 45;

/// Bytes from the start of `FTAttributes` to the end of `cliffcatch_coll`.
pub const SCALAR_BYTES: u32 = SCALAR_COUNT as u32 * 4;

/// A fighter's main archive file and the offset of its `FTAttributes`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FighterFile {
    /// `FTKind` ordinal, matching `ssb_game::fighter::FighterKind`.
    pub kind: u8,
    /// The decompilation's symbol prefix, for diagnostics.
    pub name: &'static str,
    /// Archive file id of `<Name>Main`.
    pub file: u32,
    /// Byte offset of `FTAttributes` within it — `dFT<Name>Data.o_attributes`.
    pub offset: u32,
}

/// Every fighter's main file, in `FTKind` order.
///
/// All 27 `FTKind` ordinals are present, so indexing this by kind is total.
/// The ordering is the enum's, not the file ids': Metal Mario (13) and Giant
/// DK (26) sit far from their base characters in the roster but adjacent to
/// them in the archive, and it is the roster ordering that indexes asset
/// tables elsewhere.
#[rustfmt::skip]   // a lookup table reads as a table
pub const FIGHTER_FILES: [FighterFile; 27] = [
    FighterFile { kind:  0, name: "Mario",    file: 203, offset: 0x0428 },
    FighterFile { kind:  1, name: "Fox",      file: 209, offset: 0x046C },
    FighterFile { kind:  2, name: "Donkey",   file: 213, offset: 0x04A4 },
    FighterFile { kind:  3, name: "Samus",    file: 217, offset: 0x0610 },
    FighterFile { kind:  4, name: "Luigi",    file: 221, offset: 0x0580 },
    FighterFile { kind:  5, name: "Link",     file: 225, offset: 0x0708 },
    FighterFile { kind:  6, name: "Yoshi",    file: 247, offset: 0x047C },
    FighterFile { kind:  7, name: "Captain",  file: 236, offset: 0x0488 },
    FighterFile { kind:  8, name: "Kirby",    file: 229, offset: 0x0808 },
    FighterFile { kind:  9, name: "Pikachu",  file: 243, offset: 0x041C },
    FighterFile { kind: 10, name: "Purin",    file: 233, offset: 0x0474 },
    FighterFile { kind: 11, name: "Ness",     file: 239, offset: 0x05BC },
    FighterFile { kind: 12, name: "Boss",     file: 250, offset: 0x00E8 },
    FighterFile { kind: 13, name: "MMario",   file: 206, offset: 0x02A8 },
    FighterFile { kind: 14, name: "NMario",   file: 207, offset: 0x0298 },
    FighterFile { kind: 15, name: "NFox",     file: 211, offset: 0x02A4 },
    FighterFile { kind: 16, name: "NDonkey",  file: 214, offset: 0x0298 },
    FighterFile { kind: 17, name: "NSamus",   file: 219, offset: 0x03BC },
    FighterFile { kind: 18, name: "NLuigi",   file: 223, offset: 0x0298 },
    FighterFile { kind: 19, name: "NLink",    file: 227, offset: 0x02D8 },
    FighterFile { kind: 20, name: "NYoshi",   file: 248, offset: 0x02B8 },
    FighterFile { kind: 21, name: "NCaptain", file: 237, offset: 0x029C },
    FighterFile { kind: 22, name: "NKirby",   file: 231, offset: 0x02C0 },
    FighterFile { kind: 23, name: "NPikachu", file: 245, offset: 0x02A8 },
    FighterFile { kind: 24, name: "NPurin",   file: 234, offset: 0x02A0 },
    FighterFile { kind: 25, name: "NNess",    file: 241, offset: 0x02F0 },
    FighterFile { kind: 26, name: "GDonkey",  file: 215, offset: 0x03C8 },
];

/// The body a fighter collides with — `MPObjectColl`.
///
/// A **diamond**, not a box, and the field names say so once you know what
/// they index: the four points are `(0, top)`, `(±width, center)` and
/// `(0, bottom)`. `center` is therefore a *height*, the waist where the body
/// is at its widest, not a centre point. Mario's `{320, 190, 0, 150}` is a
/// 320-tall body whose widest span is 300 across, at hip height.
///
/// `bottom` is `0.0` for every playable character: the origin is at the feet,
/// which is why `mpProcessSetCollideFloor` can put the translation straight on
/// the surface. `ftDisplayMain` reuses `width` and `center` to size the
/// shadow, so these are not physics-only numbers.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ObjectColl {
    pub top: f32,
    pub center: f32,
    pub bottom: f32,
    pub width: f32,
}

/// The scalar head of `FTAttributes`, in declaration order.
///
/// Field names and ordering are the decompilation's. Reordering would silently
/// mis-decode, since the layout is positional.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FighterAttributes {
    /// Model scale. Not a physics term — the collision diamond is already in
    /// world units and is *not* multiplied by this.
    pub size: f32,
    pub walkslow_anim_length: f32,
    pub walkmiddle_anim_length: f32,
    pub walkfast_anim_length: f32,
    pub throw_walkslow_anim_length: f32,
    pub throw_walkmiddle_anim_length: f32,
    pub throw_walkfast_anim_length: f32,
    pub rebound_anim_length: f32,
    /// Walk speed per unit of stick deflection: `|stick_x| * walk_speed_mul`.
    pub walk_speed_mul: f32,
    /// Ground deceleration per frame, before the floor material scales it.
    pub traction: f32,
    pub dash_speed: f32,
    pub dash_decel: f32,
    pub run_speed: f32,
    /// Jumpsquat length in frames. Mario's is 3.
    pub kneebend_anim_length: f32,
    pub jump_vel_x: f32,
    pub jump_height_mul: f32,
    pub jump_height_base: f32,
    pub jumpaerial_vel_x: f32,
    pub jumpaerial_height: f32,
    pub air_accel: f32,
    pub air_speed_max_x: f32,
    pub air_friction: f32,
    pub gravity: f32,
    pub tvel_base: f32,
    pub tvel_fast: f32,
    pub jumps_max: i32,
    /// Knockback multiplier, not a mass. Higher means *less* launch distance.
    pub weight: f32,
    pub attack1_followup_frames: f32,
    /// Frames of dash before it may become a run.
    pub dash_to_run: f32,
    pub shield_size: f32,
    pub shield_break_vel_y: f32,
    pub shadow_size: f32,
    pub jostle_width: f32,
    pub jostle_x: f32,
    /// Whether hits spark grey metal dust instead of blue.
    pub is_metallic: bool,
    pub cam_offset_y: f32,
    pub closeup_camera_zoom: f32,
    pub camera_zoom: f32,
    pub camera_zoom_base: f32,
    /// The collision diamond.
    pub map_coll: ObjectColl,
    /// Ledge-grab box, as `(width, height)`.
    pub cliffcatch_coll: (f32, f32),
}

/// What went wrong decoding a fighter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FighterError {
    /// The attribute struct would run past the end of the file.
    OutOfBounds { file: u32, offset: u32, len: usize },
    /// No such `FTKind` ordinal.
    UnknownKind(u8),
}

impl core::fmt::Display for FighterError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            FighterError::OutOfBounds { file, offset, len } => write!(
                f,
                "file {file} is {len} bytes, too short for FTAttributes at {offset:#x}"
            ),
            FighterError::UnknownKind(k) => write!(f, "no fighter with FTKind ordinal {k}"),
        }
    }
}

fn f32_be(d: &[u8], at: usize) -> f32 {
    f32::from_bits(u32::from_be_bytes([d[at], d[at + 1], d[at + 2], d[at + 3]]))
}

fn i32_be(d: &[u8], at: usize) -> i32 {
    i32::from_be_bytes([d[at], d[at + 1], d[at + 2], d[at + 3]])
}

impl FighterAttributes {
    /// Decodes the scalar head of an `FTAttributes` at `offset` within `data`.
    ///
    /// `data` is a decompressed archive file with its intern relocations
    /// applied. None of these fields are pointers, so relocation state does
    /// not affect the result — but the offset is measured in the relocated
    /// file, so the caller must pass the same bytes the game would see.
    pub fn decode(data: &[u8], file: u32, offset: u32) -> Result<Self, FighterError> {
        let end = offset as usize + SCALAR_BYTES as usize;
        if end > data.len() {
            return Err(FighterError::OutOfBounds {
                file,
                offset,
                len: data.len(),
            });
        }
        let at = offset as usize;
        let f = |i: usize| f32_be(data, at + i * 4);

        Ok(FighterAttributes {
            size: f(0),
            walkslow_anim_length: f(1),
            walkmiddle_anim_length: f(2),
            walkfast_anim_length: f(3),
            throw_walkslow_anim_length: f(4),
            throw_walkmiddle_anim_length: f(5),
            throw_walkfast_anim_length: f(6),
            rebound_anim_length: f(7),
            walk_speed_mul: f(8),
            traction: f(9),
            dash_speed: f(10),
            dash_decel: f(11),
            run_speed: f(12),
            kneebend_anim_length: f(13),
            jump_vel_x: f(14),
            jump_height_mul: f(15),
            jump_height_base: f(16),
            jumpaerial_vel_x: f(17),
            jumpaerial_height: f(18),
            air_accel: f(19),
            air_speed_max_x: f(20),
            air_friction: f(21),
            gravity: f(22),
            tvel_base: f(23),
            tvel_fast: f(24),
            jumps_max: i32_be(data, at + 25 * 4),
            weight: f(26),
            attack1_followup_frames: f(27),
            dash_to_run: f(28),
            shield_size: f(29),
            shield_break_vel_y: f(30),
            shadow_size: f(31),
            jostle_width: f(32),
            jostle_x: f(33),
            is_metallic: i32_be(data, at + 34 * 4) != 0,
            cam_offset_y: f(35),
            closeup_camera_zoom: f(36),
            camera_zoom: f(37),
            camera_zoom_base: f(38),
            map_coll: ObjectColl {
                top: f(39),
                center: f(40),
                bottom: f(41),
                width: f(42),
            },
            cliffcatch_coll: (f(43), f(44)),
        })
    }

    /// Whether the values look like a fighter rather than like a misread.
    ///
    /// Not a checksum — a coarse sanity test for the offset having been right.
    /// Every real fighter passes, and a struct read a few words off does not,
    /// because it lands in pointers (huge as floats) or zero padding.
    pub fn looks_plausible(&self) -> bool {
        self.size > 0.0
            && self.size < 10.0
            && self.gravity > 0.0
            && self.tvel_base > 0.0
            && self.map_coll.top > 0.0
            && self.map_coll.width > 0.0
            && (1..=6).contains(&self.jumps_max)
    }
}

#[cfg(feature = "std")]
impl std::error::Error for FighterError {}

/// One decoded fighter, with the record that located it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fighter {
    pub file: FighterFile,
    pub attributes: FighterAttributes,
}

/// Decodes one fighter out of a loaded archive file.
pub fn decode_file(entry: FighterFile, file: &File) -> Result<Fighter, FighterError> {
    let attributes = FighterAttributes::decode(&file.data, entry.file, entry.offset)?;
    Ok(Fighter {
        file: entry,
        attributes,
    })
}

/// Decodes every fighter in [`FIGHTER_FILES`] from an archive.
///
/// Loading 27 files decompresses ~40 KB and is not worth caching; the pack
/// build does it once.
pub fn decode_all(archive: &Archive<'_>) -> Vec<Result<Fighter, FighterError>> {
    FIGHTER_FILES
        .iter()
        .map(|&entry| match archive.load(entry.file) {
            Ok(file) => decode_file(entry, &file),
            Err(_) => Err(FighterError::OutOfBounds {
                file: entry.file,
                offset: entry.offset,
                len: 0,
            }),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mario's real values, from `dMarioMain_attr` in the decompilation.
    fn mario_bytes() -> Vec<u8> {
        let scalars: [f32; SCALAR_COUNT] = [
            1.12,
            90.0,
            60.0,
            40.0,
            0.0,
            0.0,
            0.0,
            16.0,
            0.3,
            1.5,
            54.0,
            2.8,
            44.0,
            3.0,
            0.35,
            0.7,
            26.0,
            0.35,
            0.9,
            0.025,
            30.0,
            0.2,
            2.4,
            44.0,
            70.0,
            f32::from_bits(2), // jumps_max, punned so the table stays one array
            1.0,
            24.0,
            14.0,
            260.0,
            70.0,
            200.0,
            112.5,
            0.0,
            f32::from_bits(0), // is_metallic
            250.0,
            1600.0,
            1.0,
            500.0,
            320.0,
            190.0,
            0.0,
            150.0,
            400.0,
            360.0,
        ];
        let mut out = alloc::vec![0u8; 0x40];
        for s in scalars {
            out.extend_from_slice(&s.to_bits().to_be_bytes());
        }
        out
    }

    #[test]
    fn every_fighter_kind_has_exactly_one_file() {
        for (i, entry) in FIGHTER_FILES.iter().enumerate() {
            assert_eq!(entry.kind as usize, i, "{} is out of order", entry.name);
        }
        let mut files: Vec<u32> = FIGHTER_FILES.iter().map(|e| e.file).collect();
        files.sort_unstable();
        let before = files.len();
        files.dedup();
        assert_eq!(before, files.len(), "two fighters share a main file");
    }

    #[test]
    fn mario_decodes_to_the_values_the_decompilation_lists() {
        let a = FighterAttributes::decode(&mario_bytes(), 203, 0x40).unwrap();
        assert_eq!(a.size, 1.12);
        assert_eq!(a.walk_speed_mul, 0.3);
        assert_eq!(a.traction, 1.5);
        assert_eq!(a.dash_speed, 54.0);
        assert_eq!(a.run_speed, 44.0);
        assert_eq!(a.kneebend_anim_length, 3.0);
        assert_eq!(a.gravity, 2.4);
        assert_eq!(a.tvel_base, 44.0);
        assert_eq!(a.tvel_fast, 70.0);
        assert_eq!(a.jumps_max, 2);
        assert_eq!(a.dash_to_run, 14.0);
        assert!(!a.is_metallic);
        assert!(a.looks_plausible());
    }

    #[test]
    fn the_collision_body_is_a_diamond_standing_on_the_origin() {
        let a = FighterAttributes::decode(&mario_bytes(), 203, 0x40).unwrap();
        // Feet at the origin is what lets the floor solver place the
        // translation directly on the surface.
        assert_eq!(a.map_coll.bottom, 0.0);
        assert_eq!(a.map_coll.top, 320.0);
        // `center` is a height, not a midpoint: the waist sits below halfway.
        assert_eq!(a.map_coll.center, 190.0);
        assert!(a.map_coll.center < a.map_coll.top);
        assert_eq!(a.map_coll.width, 150.0);
    }

    #[test]
    fn a_struct_read_past_the_end_of_the_file_is_refused() {
        let short = alloc::vec![0u8; 0x40 + 8];
        assert!(matches!(
            FighterAttributes::decode(&short, 203, 0x40),
            Err(FighterError::OutOfBounds { .. })
        ));
    }

    #[test]
    fn a_misread_offset_does_not_look_plausible() {
        // Two words late: `size` picks up an animation length, gravity picks
        // up a terminal velocity, and the diamond runs off the end of what we
        // wrote. The plausibility test is there to catch exactly this.
        let mut bytes = mario_bytes();
        bytes.extend_from_slice(&[0u8; 64]);
        let a = FighterAttributes::decode(&bytes, 203, 0x48).unwrap();
        assert!(!a.looks_plausible());
    }
}
