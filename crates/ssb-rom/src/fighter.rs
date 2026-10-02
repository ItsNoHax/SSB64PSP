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
#[cfg(test)]
use crate::archive::{ExternReloc, InternReloc};

/// Number of `f32`/`s32` scalars decoded from the head of `FTAttributes`.
///
/// The struct continues past these with hurtbox descriptors, sound ids, joint
/// indices and pointers. Those are separate subsystems' data and are left
/// where they are; the scalars here are the ones physics, collision and the
/// camera read.
pub const SCALAR_COUNT: usize = 45;

/// Bytes from the start of `FTAttributes` to the end of `cliffcatch_coll`.
pub const SCALAR_BYTES: u32 = SCALAR_COUNT as u32 * 4;

/// Byte offset of `FTAttributes::halo_size` (after the item pickup boxes
/// and three `u16` sound and scale fields): the respawn halo's scale.
pub const HALO_SIZE_OFFSET: u32 = 0xEC;

/// `FTAttributes::halo_size` of the attributes at `offset`.
pub fn halo_size(data: &[u8], offset: u32) -> Option<f32> {
    let at = (offset + HALO_SIZE_OFFSET) as usize;
    (at + 4 <= data.len()).then(|| f32_be(data, at))
}

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
    /// Six cliff recovery kinetics words at 0x2B8, including the zero word
    /// named `unused_0x2CC` which EscapeSlow indexes as entry five.
    pub cliff_air_mask: u32,
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
            cliff_air_mask: (0..6).fold(0, |mask, i| {
                let word = at + 0x2B8 + i * 4;
                mask | if word + 4 <= data.len() && i32_be(data, word) == 1 {
                    1 << i
                } else {
                    0
                }
            }),
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
    /// Which `DObjDesc` entries become joints — see [`setup_parts`].
    pub setup_parts: u64,
    /// Which joints an animation is not allowed to move — see [`animlock`].
    pub animlock: u64,
}

/// Byte offset of `setup_parts` within `FTAttributes`.
///
/// Counted back from `unused_0x2CC`, the one field in the struct the
/// decompilation names after its own offset: `cliff_status_ga[5]` and
/// `effect_joint_ids[5]` are 20 bytes each and `animlock` is a pointer, which
/// puts `setup_parts` at `0x2CC - 20 - 20 - 4 - 4`.
pub const SETUP_PARTS_OFFSET: u32 = 0x29C;

/// Byte offset of `animlock`, immediately after `setup_parts`.
pub const ANIMLOCK_OFFSET: u32 = 0x2A0;

/// Byte offset of `hiddenparts`, immediately before `commonparts_container`.
pub const HIDDENPARTS_OFFSET: u32 = 0x2D0;

/// One `FTHiddenPart`: a joint `ftMainAddHiddenPartID` inserts while a
/// motion's `anim_desc` sets the matching bit (bit 31 - index).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HiddenPart {
    /// `FTStruct::joints` index the inserted `DObj` fills.
    pub root_joint_id: u32,
    pub parent_joint_id: u32,
    /// 0 appends it as the parent's last child; 3 interposes it.
    pub joint_kind: u32,
}

/// `FTAttributes.hiddenparts[index]`, through its intern relocation.
pub fn hidden_part(file: &File, entry: FighterFile, index: u32) -> Option<HiddenPart> {
    let target = file
        .intern_relocs
        .iter()
        .find(|r| r.at == entry.offset + HIDDENPARTS_OFFSET)?
        .target as usize
        + index as usize * 16;
    let word = |i: usize| -> Option<u32> {
        let raw = file.data.get(target + i * 4..target + i * 4 + 4)?;
        Some(u32::from_be_bytes(raw.try_into().ok()?))
    };
    Some(HiddenPart {
        root_joint_id: word(0)?,
        parent_joint_id: word(1)?,
        joint_kind: word(3)?,
    })
}

/// One `DObj` a fighter figatree script binds to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeJoint {
    /// `TransN`, `XRotN` or `YRotN`: no packed node.
    Runtime(u32),
    /// A `DObjDesc` index of the fighter's model.
    Desc(u32),
}

/// The `DObj`s `lbCommonAddFighterPartsFigatree` visits, in script order.
///
/// `lbCommonSetupFighterPartsDObjs` creates each mask-enabled descriptor as
/// the last child of the latest node one level up. `ftMainSetStatus` then
/// inserts each hidden part `anim_desc` enables (bit `31 - i`), in index
/// order, and the figatree walks `TopN`'s child pre-order (RE-432).
/// `depths` holds every descriptor's `id & 0xFFF`; `hidden` holds the
/// fighter's parts. Returns `None` for a part whose parent does not exist.
pub fn figatree_order(
    depths: &[u32],
    mask: u64,
    hidden: &[HiddenPart],
    anim_desc: u32,
) -> Option<Vec<TreeJoint>> {
    // Node 0 is `TopN`. Children lists in sibling order.
    let mut kinds = alloc::vec![TreeJoint::Runtime(0)];
    let mut children: Vec<Vec<usize>> = alloc::vec![Vec::new()];
    let mut joints: [Option<usize>; 64] = [None; 64];
    joints[0] = Some(0);
    let mut latest = [0usize; 19];
    for (i, &depth) in depths.iter().enumerate().take(64) {
        if mask >> i & 1 == 0 {
            continue;
        }
        let depth = depth as usize;
        let parent = if depth == 0 {
            0
        } else {
            *latest.get(depth - 1)?
        };
        let node = kinds.len();
        kinds.push(TreeJoint::Desc(i as u32));
        children.push(Vec::new());
        children[parent].push(node);
        *latest.get_mut(depth)? = node;
        joints[i + 4] = Some(node);
    }
    for i in 0..27 {
        if anim_desc >> (31 - i) & 1 == 0 {
            continue;
        }
        let part = hidden.get(i)?;
        let parent = (*joints.get(part.parent_joint_id as usize)?)?;
        let node = kinds.len();
        kinds.push(match part.root_joint_id {
            id @ 0..=3 => TreeJoint::Runtime(id),
            id => TreeJoint::Desc(id - 4),
        });
        children.push(Vec::new());
        match part.joint_kind {
            0 => children[parent].push(node),
            1 => children[parent].insert(0, node),
            2 => {
                let at = children[parent].len().min(1);
                children[parent].insert(at, node);
            }
            _ => children[node] = core::mem::take(&mut children[parent]),
        }
        if part.joint_kind == 3 {
            children[parent].push(node);
        }
        *joints.get_mut(part.root_joint_id as usize)? = Some(node);
    }
    // `lbCommonGetTreeDObjNextFromRoot` from `TopN->child`: its subtree only.
    let mut order = Vec::new();
    let mut stack = alloc::vec![*children[0].first()?];
    while let Some(node) = stack.pop() {
        order.push(kinds[node]);
        stack.extend(children[node].iter().rev());
    }
    Some(order)
}

/// Byte offset of `commonparts_container` within `FTAttributes`.
///
/// Counted forward from `unused_0x2CC` through `hiddenparts`, and checked
/// against the next field the decompilation names after its offset:
/// `dobj_lookup`, `shield_anim_joints[8]` and the four foot-joint fields put
/// the following filler at `0x30C`, which is what it is called.
pub const COMMONPARTS_OFFSET: u32 = 0x2D4;

/// Size of one `FTCommonPart`: three pointers and a `u8`, padded to 16.
const COMMONPART_SIZE: u32 = 16;

/// Byte offset of `dobj_lookup`, immediately after `commonparts_container`.
pub const DOBJ_LOOKUP_OFFSET: u32 = 0x2D8;

/// Byte offset of `shield_anim_joints[8]`, immediately after `dobj_lookup`.
pub const SHIELD_ANIM_JOINTS_OFFSET: u32 = 0x2DC;

/// Size of one `DObjDesc`: `id`, `dl`, then translate, rotate and scale.
pub const DOBJDESC_SIZE: u32 = 44;

/// `DOBJ_ARRAY_MAX`: the `id` that ends a `DObjDesc` array.
pub const DOBJ_ARRAY_MAX: u32 = 18;

/// Where a fighter's shield poses live.
///
/// `FTAttributes.dobj_lookup` and `shield_anim_joints[8]` both point into
/// the fighter's `*ShieldPose` file. `dobj_lookup` is the neutral shield
/// pose, one `DObjDesc` per joint from `XRotN` on. Each
/// `shield_anim_joints[i]` is a joint table for the 45-degree stick sector
/// `i`, in the same order (`ftCommonGuardInitJoints`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShieldPoseRefs {
    /// The `*ShieldPose` file.
    pub file: u32,
    /// Byte offset of the `dobj_lookup` array.
    pub lookup: u32,
    /// Byte offset of each sector's joint table.
    pub tables: [u32; 8],
}

/// Reads [`ShieldPoseRefs`] from a fighter's attributes. `None` when any of
/// the nine pointers is not an extern relocation into one shared file.
pub fn shield_pose_refs(file: &File, entry: FighterFile) -> Option<ShieldPoseRefs> {
    let target = |at: u32| {
        file.extern_relocs
            .iter()
            .find(|r| r.at == entry.offset + at)
            .map(|r| (r.target_file as u32, r.target_offset))
    };
    let (pose_file, lookup) = target(DOBJ_LOOKUP_OFFSET)?;
    let mut tables = [0; 8];
    for (i, table) in tables.iter_mut().enumerate() {
        let (f, at) = target(SHIELD_ANIM_JOINTS_OFFSET + 4 * i as u32)?;
        if f != pose_file {
            return None;
        }
        *table = at;
    }
    Some(ShieldPoseRefs {
        file: pose_file,
        lookup,
        tables,
    })
}

/// A fighter's skeleton: the `DObjDesc` array its `FTCommonPart` names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommonPart {
    /// Archive file the descriptor array lives in.
    pub model_file: u32,
    /// Byte offset of the array within that file.
    pub graph: u32,
}

/// The high- and low-detail skeletons a fighter's `FTAttributes` names.
///
/// ```c
/// struct FTCommonPartContainer { FTCommonPart commonparts[2]; };
/// ```
///
/// indexed by `FTPartsLevelDetail` — high poly, then low. Both entries name
/// the same model file; a fighter's *other* graphs (Mario and Luigi share a
/// 26-node one) are named by other records and are not its skeleton.
///
/// Reading it takes two hops, each an archive relocation rather than a guess:
/// an intern relocation from the attribute struct to the container, then an
/// extern relocation from the container into the model file.
pub fn common_parts(file: &File, entry: FighterFile) -> [Option<CommonPart>; 2] {
    let container = file
        .intern_relocs
        .iter()
        .find(|r| r.at == entry.offset + COMMONPARTS_OFFSET)
        .map(|r| r.target);
    let mut out = [None; 2];
    let Some(container) = container else {
        return out;
    };
    for (detail, slot) in out.iter_mut().enumerate() {
        let at = container + detail as u32 * COMMONPART_SIZE;
        *slot = file
            .extern_relocs
            .iter()
            .find(|r| r.at == at)
            .map(|r| CommonPart {
                model_file: r.target_file as u32,
                graph: r.target_offset,
            });
    }
    out
}

/// Reads a `u32 *flags` field of `FTAttributes` as one 64-bit mask.
///
/// Both `setup_parts` and `animlock` point at "two sets of flags" — two
/// big-endian `u32`s, the first covering joints 0..31 and the second 32..63,
/// each read most-significant bit first (`current_flags & (1 << 31)`, then
/// `flags <<= 1`). Storing them as one `u64` with bit *n* meaning joint *n*
/// puts them in the order the caller wants to index them.
///
/// The field is a pointer, so it only means anything if the archive relocated
/// that slot; an unrelocated slot is a null pointer, and the answer is `None`.
fn joint_mask(file: &File, at: u32) -> Option<u64> {
    let target = file
        .intern_relocs
        .iter()
        .find(|r| r.at == at)
        .map(|r| r.target)? as usize;
    let word = |i: usize| -> Option<u32> {
        let raw = file.data.get(target + i * 4..target + i * 4 + 4)?;
        Some(u32::from_be_bytes(raw.try_into().ok()?))
    };
    let (lo, hi) = (word(0)?, word(1)?);
    let bit = |w: u32, i: u32| u64::from(w >> (31 - i) & 1);
    Some((0..32).fold(0u64, |m, i| m | bit(lo, i) << i | bit(hi, i) << (i + 32)))
}

/// Which `DObjDesc` entries of a fighter's model become real joints.
///
/// `lbCommonSetupFighterPartsDObjs` walks the descriptor array and the mask
/// together, and only creates a `DObj` where the mask's bit is set:
///
/// ```c
/// for (i = 0; ((flags0 != 0) || (flags1 != 0)) && (dobjdesc->id != DOBJ_ARRAY_MAX); i++) {
///     current_flags = (i < NBITS(u32)) ? flags0 : flags1;
///     if (current_flags & (1 << 31)) { ... gcAddChildForDObj(...) ... }
///     dobjdesc++;
///     if (i < NBITS(u32)) flags0 <<= 1; else flags1 <<= 1;
/// }
/// ```
///
/// So a fighter's joint count is the mask's population count, not its model's
/// node count, and the two differ: Mario's model has 25 descriptors and 24
/// joints. That gap is exactly what a figatree's pointer table is sized to —
/// one entry per joint — so this mask is what maps animation joint *n* onto a
/// descriptor index.
pub fn setup_parts(file: &File, entry: FighterFile) -> Option<u64> {
    joint_mask(file, entry.offset + SETUP_PARTS_OFFSET)
}

/// Which joints an animation may not move.
///
/// "Ignores joints 0 through 3" — those are `TopN`, `TransN`, `XRotN` and
/// `YRotN`, the four the fighter code drives itself.
pub fn animlock(file: &File, entry: FighterFile) -> Option<u64> {
    joint_mask(file, entry.offset + ANIMLOCK_OFFSET)
}

/// `FTAttributes::skeleton` (`FTSkeleton **`), after `sprites` at 0x340.
pub const SKELETON_OFFSET: u32 = 0x344;

/// Size of one `FTSkeleton`: a display-list pointer and a `u8` of flags.
const SKELETON_ENTRY_SIZE: u32 = 8;

/// One joint of an electric-damage skeleton set (`FTSkeleton`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkeletonPart {
    /// `flags`: `& 0xF` is 0 for one list (`dl`), 1 for a pair (`dls`); the
    /// upper bits are `FTPARTS_FLAG_*`.
    pub flags: u8,
    /// The display lists, as `(file, offset)`: `[dl, -]` for kind 0, and
    /// `[dls[0], dls[1]]` for kind 1. `dls[0]` is drawn before the joint's
    /// own matrix (in its parent's space), as `ftDisplayMainDrawSkeleton`
    /// does.
    pub dls: [Option<(u32, u32)>; 2],
}

/// A fighter's electric-damage skeletons (`ftDisplayMainDrawSkeleton`):
/// `FTAttributes::skeleton[0]` is a joint id whose DObj must have a list
/// for any skeleton to draw, and `skeleton[id]` (id 1 or 2) is an
/// `FTSkeleton` per joint from `nFTPartsJointCommonStart` — that is, one per
/// model descriptor, in the pack's node order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skeletons {
    pub gate_joint: u32,
    /// `skeleton[1]` and `skeleton[2]`, `count` parts each when present.
    pub sets: [Option<Vec<SkeletonPart>>; 2],
}

/// Follows `FTAttributes::skeleton`. `count` is the model's descriptor
/// count (the arrays carry no length of their own). `model` resolves the
/// `dls` pairs, which live in the model file.
pub fn skeletons(main: &File, entry: FighterFile, count: usize, model: &File) -> Option<Skeletons> {
    let (file, array) = crate::sprite::pointer_place(main, entry.offset + SKELETON_OFFSET)?;
    if file != main.id {
        return None;
    }
    let word = |f: &File, at: u32| -> Option<u32> {
        let raw = f.data.get(at as usize..at as usize + 4)?;
        Some(u32::from_be_bytes(raw.try_into().ok()?))
    };
    let gate_joint = word(main, array)?;
    let mut sets = [None, None];
    for (i, set) in sets.iter_mut().enumerate() {
        let Some((sf, parts)) = crate::sprite::pointer_place(main, array + 4 + i as u32 * 4) else {
            continue;
        };
        if sf != main.id {
            continue;
        }
        let list = (0..count as u32)
            .map(|j| {
                let at = parts + j * SKELETON_ENTRY_SIZE;
                let flags = main.data.get(at as usize + 4).copied().unwrap_or(0);
                let target = crate::sprite::pointer_place(main, at);
                let dls = match (flags & 0xF, target) {
                    (0, Some(dl)) => [Some(dl), None],
                    (1, Some((pf, pair))) if pf == model.id => [
                        crate::sprite::pointer_place(model, pair),
                        crate::sprite::pointer_place(model, pair + 4),
                    ],
                    _ => [None, None],
                };
                SkeletonPart { flags, dls }
            })
            .collect();
        *set = Some(list);
    }
    Some(Skeletons { gate_joint, sets })
}

/// `FTAttributes::modelparts_container` (`FTModelPartContainer *`), after
/// `translate_scales` at 0x324 (RE-417).
pub const MODELPARTS_OFFSET: u32 = 0x328;

/// `nFTPartsJointCommonStart`: the container, like the model's descriptor
/// array, starts at joint 4.
pub const JOINT_COMMON_START: u32 = 4;

/// Size of one `FTModelPart`: four pointers and a `u8` of flags.
const MODELPART_SIZE: u32 = 20;

/// One `FTModelPart`: what `ftParamSetModelPartID` gives a joint for a part
/// id at one detail level. Every pointer is a `(file, offset)` place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelPart {
    /// `dl`: the joint's new display list.
    pub dl: (u32, u32),
    /// `mobjsubs`: the `MObjSub *` chain `lbCommonAddMObjForFighterPartsDObj`
    /// gives the joint, if any.
    pub mobjsubs: Option<(u32, u32)>,
    /// `costume_matanim_joints`: one `AObjEvent32 *` per chain `MObj`, played
    /// at the costume's frame.
    pub costume_matanim_joints: Option<(u32, u32)>,
    pub flags: u8,
}

/// `modelparts_container->modelparts_desc[joint - 4]->modelparts[part]
/// [detail]` (detail 0 is `nFTPartsDetailHigh`). The descriptor carries no
/// part count; the caller names a part the game sets.
pub fn model_part(
    main: &File,
    entry: FighterFile,
    joint: u32,
    part: u32,
    detail: u32,
) -> Option<ModelPart> {
    let (file, container) = crate::sprite::pointer_place(main, entry.offset + MODELPARTS_OFFSET)?;
    if file != main.id {
        return None;
    }
    let slot = container + joint.checked_sub(JOINT_COMMON_START)? * 4;
    let (file, desc) = crate::sprite::pointer_place(main, slot)?;
    if file != main.id {
        return None;
    }
    let at = desc + (part * 2 + detail) * MODELPART_SIZE;
    Some(ModelPart {
        dl: crate::sprite::pointer_place(main, at)?,
        mobjsubs: crate::sprite::pointer_place(main, at + 4),
        costume_matanim_joints: crate::sprite::pointer_place(main, at + 8),
        flags: *main.data.get(at as usize + 16)?,
    })
}

/// `FTAttributes::accesspart` (`FTAccessPart *`), after
/// `modelparts_container` at 0x328 (RE-425).
pub const ACCESSPART_OFFSET: u32 = 0x32C;

/// `FTAccessPart`: a headgear accessory (Pikachu's hat, Jigglypuff's bow).
/// `ftManagerMakeFighter` and `ftParamInitAllParts` give joint `joint_id` a
/// parts `GObj` whose `DObj` holds `dl`, with `mobjsubs` coloured by
/// `costume_matanim_joints` at the costume's frame, for every costume but 0;
/// `ftDisplayMainDrawAccessory` draws it in the joint's matrix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccessPart {
    /// `joint_id`: the fighter joint (4 is the model's first descriptor).
    pub joint: u32,
    pub dl: (u32, u32),
    pub mobjsubs: Option<(u32, u32)>,
    pub costume_matanim_joints: Option<(u32, u32)>,
}

/// Follows `FTAttributes::accesspart`; `None` when the fighter has none.
pub fn access_part(main: &File, entry: FighterFile) -> Option<AccessPart> {
    let (file, at) = crate::sprite::pointer_place(main, entry.offset + ACCESSPART_OFFSET)?;
    if file != main.id {
        return None;
    }
    let raw = main.data.get(at as usize..at as usize + 4)?;
    Some(AccessPart {
        joint: u32::from_be_bytes(raw.try_into().ok()?),
        dl: crate::sprite::pointer_place(main, at + 4)?,
        mobjsubs: crate::sprite::pointer_place(main, at + 8),
        costume_matanim_joints: crate::sprite::pointer_place(main, at + 12),
    })
}

/// `FTAttributes::textureparts_container`, after `accesspart` (RE-426).
pub const TEXTUREPARTS_OFFSET: u32 = 0x330;

/// `FTTexturePart`: the joint whose `MObj` chain holds a face texture, and
/// that `MObj`'s position in the chain at high (`detail[0]`) and low
/// (`detail[1]`) detail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TexturePart {
    pub joint: u8,
    pub detail: [u8; 2],
}

/// `textureparts_container->textureparts[0..2]` (3 bytes each), or `None`
/// when the container is NULL (Donkey Kong, Samus). The container is the
/// struct's full two entries even where the decompilation declares one: a
/// fighter whose scripts set only part 0 (Mario, Luigi, Kirby, Captain
/// Falcon, Ness) reads the bytes that follow for part 1, and never uses
/// them.
pub fn texture_parts(main: &File, entry: FighterFile) -> Option<[TexturePart; 2]> {
    let (file, at) = crate::sprite::pointer_place(main, entry.offset + TEXTUREPARTS_OFFSET)?;
    if file != main.id {
        return None;
    }
    let raw = main.data.get(at as usize..at as usize + 6)?;
    Some(core::array::from_fn(|i| TexturePart {
        joint: raw[3 * i],
        detail: [raw[3 * i + 1], raw[3 * i + 2]],
    }))
}

/// Decodes one fighter out of a loaded archive file.
pub fn decode_file(entry: FighterFile, file: &File) -> Result<Fighter, FighterError> {
    let attributes = FighterAttributes::decode(&file.data, entry.file, entry.offset)?;
    Ok(Fighter {
        file: entry,
        attributes,
        setup_parts: setup_parts(file, entry).unwrap_or(0),
        animlock: animlock(file, entry).unwrap_or(0),
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

    /// Builds a `*Main` file holding an attribute struct at `attrs`, with
    /// `setup_parts` pointing at `mask` and `commonparts_container` pointing
    /// at a container whose two entries name graphs in `model`.
    fn main_file(attrs: u32, mask: (u32, u32), model: u16) -> File {
        let mask_at = 0x800u32;
        let container_at = 0x810u32;
        let mut data = alloc::vec![0u8; 0x900];
        data[mask_at as usize..mask_at as usize + 4].copy_from_slice(&mask.0.to_be_bytes());
        data[mask_at as usize + 4..mask_at as usize + 8].copy_from_slice(&mask.1.to_be_bytes());
        File {
            id: 203,
            data,
            intern_relocs: alloc::vec![
                InternReloc {
                    at: attrs + SETUP_PARTS_OFFSET,
                    target: mask_at
                },
                InternReloc {
                    at: attrs + COMMONPARTS_OFFSET,
                    target: container_at
                },
            ],
            extern_relocs: alloc::vec![
                ExternReloc {
                    at: container_at,
                    target_file: model,
                    target_offset: 0x2200
                },
                ExternReloc {
                    at: container_at + 16,
                    target_file: model,
                    target_offset: 0x4590
                },
            ],
        }
    }

    #[test]
    fn setup_parts_reads_its_two_words_most_significant_bit_first() {
        // `current_flags & (1 << 31)` then `flags <<= 1`, so joint 0 is the
        // top bit of the first word and joint 32 the top bit of the second.
        // Reading them as plain little-endian bitmasks would reverse every
        // joint in the fighter.
        let entry = FIGHTER_FILES[0];
        let f = main_file(entry.offset, (0x8000_0001, 0x4000_0000), 296);
        let mask = setup_parts(&f, entry).unwrap();
        assert_eq!(mask & 1, 1, "joint 0 is the first word's top bit");
        assert_eq!(mask >> 31 & 1, 1, "joint 31 is its bottom bit");
        assert_eq!(mask >> 33 & 1, 1, "joint 33 is the second word's bit 30");
        assert_eq!(mask.count_ones(), 3);
    }

    #[test]
    fn a_fighter_whose_mask_slot_was_never_relocated_has_no_mask() {
        // The field is a pointer; an unrelocated slot is a null one, and
        // inventing a mask from the raw bytes would silently drop joints.
        let entry = FIGHTER_FILES[0];
        let mut f = main_file(entry.offset, (0xFFFF_FFFF, 0), 296);
        f.intern_relocs.clear();
        assert_eq!(setup_parts(&f, entry), None);
    }

    #[test]
    fn the_common_part_container_names_a_high_and_a_low_detail_skeleton() {
        let entry = FIGHTER_FILES[0];
        let f = main_file(entry.offset, (0xFFFF_FF00, 0), 296);
        let parts = common_parts(&f, entry);
        assert_eq!(
            parts[0],
            Some(CommonPart {
                model_file: 296,
                graph: 0x2200
            })
        );
        // `commonparts[1]` is one 16-byte FTCommonPart further on, and both
        // detail levels live in the same model file.
        assert_eq!(
            parts[1],
            Some(CommonPart {
                model_file: 296,
                graph: 0x4590
            })
        );
    }

    /// Cross-checks `common_parts`'s recovered model-file id against the
    /// decompilation's own relocData naming, for every fighter (RE-242,
    /// `PLAN.md` R2.2/C2's mesh/costume-file-to-fighter recovery). Ground
    /// truth is `tools/fighter-model-ground-truth.py`, which reads the
    /// archive-file id straight out of each `<id>_<Name>Model.c` relocData
    /// source filename -- an independent record naming the same file, not a
    /// value this crate invented.
    ///
    /// Two fighters have no `*Model.c` of their own and are checked against
    /// the base character whose file they really share: Giant DK (317,
    /// Donkey Kong's) and NLuigi (301, NMario's) are scaled/recoloured
    /// variants of another fighter's model, both real ROM findings from this
    /// same cross-check, not guesses.
    /// `halo_size` in `relocData/2xx_*Main.c`, which `ssb_game::dead::halo_size`
    /// transcribes.
    #[test]
    fn real_rom_halo_sizes_match_the_decomp() {
        let Some(path) = std::env::var_os("SSB64_ROM") else {
            return;
        };
        let data = std::fs::read(path).unwrap();
        let info = crate::rom::identify(&data).unwrap();
        let archive = Archive::open(&data, info.region).unwrap();
        #[rustfmt::skip]
        const EXPECTED: [f32; 27] = [
            1.0, 1.1, 1.7, 1.2, 1.02, 1.1, 1.2, 1.2, 1.14, 1.1, 1.2, 1.0,
            1.0, 1.0,
            1.0, 1.1, 1.7, 1.2, 1.02, 1.1, 1.2, 1.2, 1.14, 1.1, 1.2, 1.0,
            1.7,
        ];
        for (entry, want) in FIGHTER_FILES.iter().zip(EXPECTED) {
            let main = archive.load(entry.file).unwrap();
            assert_eq!(
                halo_size(&main.data, entry.offset),
                Some(want),
                "{}",
                entry.name
            );
        }
    }

    /// `dFT<Name>Main_skeleton` in `relocData/2xx_*Main.c`: the gate joint,
    /// which sets exist, and a few named lists (Fox's joint 1 is
    /// `dFoxModel_gap_0x5A38_sub_0x808`, 0x6240; Samus's parts are `dls`
    /// pairs; Kirby's second set is `dKirbyMain_sub_0x724`).
    #[test]
    fn real_rom_skeletons_match_the_decomp() {
        let Some(path) = std::env::var_os("SSB64_ROM") else {
            return;
        };
        let data = std::fs::read(path).unwrap();
        let info = crate::rom::identify(&data).unwrap();
        let archive = Archive::open(&data, info.region).unwrap();
        #[rustfmt::skip]
        const EXPECTED: [(u32, usize, bool); 12] = [
            (12, 25, false), (12, 27, false), (12, 26, false), (13, 33, false),
            (12, 25, false), (23, 32, false), (7, 28, false), (12, 26, false),
            (10, 27, true), (11, 27, false), (10, 26, true), (12, 27, false),
        ];
        for (entry, (gate, count, two)) in FIGHTER_FILES.iter().zip(EXPECTED) {
            let main = archive.load(entry.file).unwrap();
            let part = common_parts(&main, *entry)[0].unwrap();
            let model = archive.load(part.model_file).unwrap();
            let s = skeletons(&main, *entry, count, &model).unwrap();
            assert_eq!(s.gate_joint, gate, "{}", entry.name);
            assert!(s.sets[0].is_some(), "{}", entry.name);
            assert_eq!(s.sets[1].is_some(), two, "{}", entry.name);
            let lists = s.sets[0]
                .as_ref()
                .unwrap()
                .iter()
                .filter(|p| p.dls.iter().any(Option::is_some));
            assert!(lists.count() > 3, "{}", entry.name);
        }
        let fox = &FIGHTER_FILES[1];
        let main = archive.load(fox.file).unwrap();
        let model = archive.load(313).unwrap();
        let s = skeletons(&main, *fox, 27, &model).unwrap();
        let set = s.sets[0].as_ref().unwrap();
        assert_eq!(set[0].dls, [None, None]);
        assert_eq!(set[1].dls, [Some((313, 0x5A38 + 0x808)), None]);
        assert_eq!(set[8].dls, [Some((313, 0x66B0)), None]);
        let samus = &FIGHTER_FILES[3];
        let main = archive.load(samus.file).unwrap();
        let model = archive.load(320).unwrap();
        let s = skeletons(&main, *samus, 33, &model).unwrap();
        let set = s.sets[0].as_ref().unwrap();
        assert!(set.iter().all(|p| p.flags & 0xF == 1));
        assert!(set[1].dls[1].is_some());
    }

    #[test]
    fn real_rom_common_parts_match_every_named_model_file() {
        let Some(path) = std::env::var_os("SSB64_ROM") else {
            return;
        };
        let data = std::fs::read(path).unwrap();
        let info = crate::rom::identify(&data).unwrap();
        let archive = Archive::open(&data, info.region).unwrap();

        #[rustfmt::skip]
        const EXPECTED: &[(&str, u32)] = &[
            ("Mario", 296), ("MMario", 300), ("NMario", 301), ("NFox", 303),
            ("NYoshi", 304), ("NKirby", 305), ("NPurin", 306), ("NPikachu", 307),
            ("NDonkey", 308), ("NSamus", 309), ("NLink", 310), ("NCaptain", 311),
            ("NNess", 312), ("Fox", 313), ("Donkey", 317), ("Samus", 320),
            ("Luigi", 323), ("Link", 324), ("Kirby", 328), ("Purin", 330),
            ("Captain", 332), ("Ness", 335), ("Yoshi", 338), ("Pikachu", 341),
            ("Boss", 344),
            ("GDonkey", 317), ("NLuigi", 301),
        ];
        assert_eq!(EXPECTED.len(), FIGHTER_FILES.len());

        for &entry in &FIGHTER_FILES {
            let want = EXPECTED
                .iter()
                .find(|&&(n, _)| n == entry.name)
                .map(|&(_, f)| f)
                .unwrap_or_else(|| panic!("{}: no ground-truth row", entry.name));
            let main = archive.load(entry.file).unwrap();
            let parts = common_parts(&main, entry);
            for (detail, part) in parts.iter().enumerate() {
                let part = part
                    .unwrap_or_else(|| panic!("{}: no detail-{detail} FTCommonPart", entry.name));
                assert_eq!(part.model_file, want, "{} detail {detail}", entry.name);
            }
        }
    }
}
