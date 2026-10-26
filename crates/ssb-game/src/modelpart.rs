//! Fighter model parts: `FTModelPartStatus` and the `ftParam*ModelPart*`
//! setters (`ft/ftparam.c`), RE-425.
//!
//! Every joint from `nFTPartsJointCommonStart` has a `modelpart_id_base`
//! and a `modelpart_id_curr`. `ftManagerMakeFighter` starts both at 0 for a
//! joint whose descriptor has a display list and at -1 for one without, and
//! only for the joints `setup_parts` creates. A motion script's
//! `SetModelPartID` (a hand opening, Kirby's face, Link's shield moving to
//! his hand, Fox's Blaster) changes `curr` and marks the parts modified;
//! `ftMainSetStatus` restores every joint to its base unless the new status
//! keeps its parts (`FTSTATUS_PRESERVE_MODELPART`). `curr` is -1 for a
//! hidden joint, 0 for its own list (or its `FTModelPart` 0), and otherwise
//! the `FTModelPart` the renderer draws in its place.

use crate::fighter::{Fighter, FighterKind};

/// `nFTPartsJointCommonStart`.
pub const JOINT_COMMON_START: u8 = 4;

/// `FTPARTS_JOINT_NUM_MAX - nFTPartsJointCommonStart`.
pub const PARTS_MAX: usize = 33;

/// A hidden joint (`modelpart_id == -1`, `joint->dl = NULL`).
pub const HIDDEN: i8 = -1;

/// A descriptor `setup_parts` never makes a joint of: nothing of it draws,
/// not even an electric skeleton's part (`ftDisplayMainDrawSkeleton` walks
/// the joints).
pub const ABSENT: i8 = i8::MIN;

/// Per fighter, generated from the ROM by `crates/ssb-tablegen`
/// (`model_parts.rs`), bit `n` for descriptor `n` (joint `n + 4`):
///
/// - `JOINT_MASKS`: which descriptors `lbCommonSetupFighterPartsDObjs`
///   makes a joint of (`setup_parts`), and which of those have a
///   high-detail display list (`commonparts[0].dobjdesc[n].dl`), from each
///   fighter's `*Main` and `*Model` files.
/// - `ANIMLOCK_MASKS`: `FTAttributes::animlock`, the parts a motion without
///   `FTANIM_FLAG_ANIMLOCKS` leaves at their rest transform
///   (`ftParamSetAnimLocks`, RE-509).
/// - `HIDDEN_PARTS`: `FTAttributes::hiddenparts` ([`hidden_parts`]).
/// - `TEXTURE_PARTS`: `FTAttributes::textureparts_container`
///   ([`texture_part_table`]).
///
/// `crates/ssb-rom/tests/model_parts.rs`, `fighter_fidelity.rs` and
/// `texture_parts.rs` check them against `ssb-rom`'s own decoders.
#[allow(clippy::all)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/joint_masks.rs"));
}

/// `FTAttributes::animlock` (`generated::ANIMLOCK_MASKS`): bit `n` for
/// descriptor `n`.
pub fn animlock_mask(kind: FighterKind) -> u64 {
    generated::ANIMLOCK_MASKS
        .get(kind as usize)
        .copied()
        .unwrap_or(0)
}

/// `FTAttributes::hiddenparts` from joint 4 on, per fighter
/// (`generated::HIDDEN_PARTS`): the part's index (its `anim_desc` bit is
/// `0x80000000 >> index`), its root joint and whether its high-detail
/// descriptor has a display list. Empty for Master Hand, who has no
/// model-part state. `crates/ssb-rom/tests/fighter_fidelity.rs` checks them
/// against `ssb-rom`'s decoder (RE-468).
fn hidden_parts(kind: FighterKind) -> &'static [(u8, u8, bool)] {
    generated::HIDDEN_PARTS
        .get(kind as usize)
        .copied()
        .unwrap_or(&[])
}

/// [`hidden_parts`], for the ROM test.
pub fn hidden_part_joints(kind: FighterKind) -> &'static [(u8, u8, bool)] {
    hidden_parts(kind)
}

/// `FTAnimDesc`'s low flags, which name no hidden part.
const ANIM_DESC_FLAGS: u32 = 0x1F;

/// `(setup_parts, has a display list)` for a playable fighter, as
/// `generated::JOINT_MASKS` holds them.
pub fn joint_masks(kind: FighterKind) -> Option<(u64, u64)> {
    generated::JOINT_MASKS
        .get(kind as usize)
        .copied()
        .filter(|_| kind != FighterKind::Boss)
}

/// `nFTPartsDetailHigh` / `nFTPartsDetailLow`: which `FTCommonPart` a
/// joint's list, `MObj`s and model parts come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Detail {
    #[default]
    High,
    Low,
}

impl Detail {
    /// `detail - nFTPartsDetailStart`.
    pub const fn index(self) -> usize {
        match self {
            Detail::High => 0,
            Detail::Low => 1,
        }
    }

    /// `desc.detail` in a battle scene (`scvsbattle.c:188`,
    /// `sc1pgame.c:1379`, `sc1ptrainingmode.c:1809`): high detail for up
    /// to two fighters, low for three or four.
    pub const fn for_fighters(count: usize) -> Detail {
        if count < 3 {
            Detail::High
        } else {
            Detail::Low
        }
    }
}

/// Texture parts per fighter (`FTTexturePartContainer`).
pub const TEXTURE_PARTS: usize = 2;

/// `FTAttributes::textureparts_container`, per fighter
/// (`generated::TEXTURE_PARTS`): each part's joint and the position of its
/// `MObj` in that joint's chain at high and low detail; `None` for a NULL
/// container (Donkey Kong, Samus).
/// A fighter the decompilation gives one part reads the next bytes as its
/// second (joint 0, `TopN`, which has no `MObj`) and never sets it.
/// `crates/ssb-rom/tests/texture_parts.rs` checks them against the ROM.
pub type TexturePartTable = [(u8, [u8; 2]); TEXTURE_PARTS];

/// `generated::TEXTURE_PARTS`' row for `kind`.
pub fn texture_part_table(kind: FighterKind) -> Option<TexturePartTable> {
    generated::TEXTURE_PARTS
        .get(kind as usize)
        .copied()
        .flatten()
}

/// Per texture part (`generated::TEXTURE_PART_MADE`, RE-569): the
/// `texture_id_curr` its `MObj` has once
/// `lbCommonAddMObjForFighterPartsDObj` has played the `MObj`'s costume
/// script (`gcAddMObjForDObj` zeroes it first), at high and low detail, for
/// the `FTCommonPart` chain; and whether its joint has an `FTModelPartDesc`,
/// whose parts carry their own costume scripts instead. Every costume gives
/// the same id (the generator checks), so the table has no costume axis.
/// Luigi's eye is 1 at both details (file 323, scripts 0x29A4 and 0x4F74);
/// every other fighter's is 0. `romtool costumes` checks the table against
/// the decomp reading, and that no model part's script sets an id there.
pub fn texture_part_made(kind: FighterKind) -> [([i8; 2], bool); TEXTURE_PARTS] {
    generated::TEXTURE_PART_MADE
        .get(kind as usize)
        .copied()
        .unwrap_or_default()
}

/// `FTStruct::texturepart_status` and `is_texturepart_modify`, with what
/// each part's `MObj` shows (RE-426).
///
/// `ftParamSetTexturePartID` writes `texture_id_curr` of the joint's
/// `detail`th `MObj`, which `gcDrawMObjForDObj` reads to load
/// `sprites[texture_id_curr]`, and records it in the status. A model-part
/// change on that joint replaces its `MObj`s (`gcRemoveMObjAll`, then
/// `gcAddMObjForDObj`, which zeroes `texture_id_curr`, then the costume
/// script, which can set it: [`texture_part_made`]), so what the face shows
/// can differ from the status until the next set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TextureParts {
    /// Each part's joint (0: none).
    pub joint: [u8; TEXTURE_PARTS],
    pub base: [i8; TEXTURE_PARTS],
    pub curr: [i8; TEXTURE_PARTS],
    /// `texture_id_curr` of the part's `MObj`: what draws.
    pub shown: [i8; TEXTURE_PARTS],
    /// Parts without the named MObj in either detail: Kirby's Stone head
    /// (2) and Ness's alternate head (1). ROM-backed in texture_parts.rs.
    pub missing_part: [Option<i8>; TEXTURE_PARTS],
    pub is_modify: bool,
    /// [`texture_part_made`]: each part's `texture_id_curr` when its
    /// `FTCommonPart` chain is made, per [`Detail::index`].
    pub made: [[i8; 2]; TEXTURE_PARTS],
    /// [`texture_part_made`]: the part's joint has an `FTModelPartDesc`.
    pub joint_has_parts: [bool; TEXTURE_PARTS],
}

/// `FTStruct::modelpart_status` and `is_modelpart_modify`, the texture
/// parts and the detail level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelParts {
    /// `setup_parts`: descriptor `n`'s joint exists. Zero for a fighter the
    /// table does not cover, whose parts never change.
    pub present: u64,
    pub base: [i8; PARTS_MAX],
    pub curr: [i8; PARTS_MAX],
    pub is_modify: bool,
    /// The hidden-part bits of the last motion with a figatree
    /// (`fp->anim_desc`), which decide the next status's makes and ejects.
    pub hidden_bits: u32,
    pub texture: TextureParts,
    /// `detail_curr` and `detail_base`.
    pub detail_curr: Detail,
    pub detail_base: Detail,
}

impl Default for ModelParts {
    fn default() -> Self {
        ModelParts {
            present: 0,
            base: [0; PARTS_MAX],
            curr: [0; PARTS_MAX],
            is_modify: false,
            hidden_bits: 0,
            texture: TextureParts::default(),
            detail_curr: Detail::High,
            detail_base: Detail::High,
        }
    }
}

impl ModelParts {
    /// `ftManagerMakeFighter`'s part loop, then its per-fighter defaults:
    /// Kirby's joint 6 wears his copy's part and Link's shield starts on
    /// his back (joint 21 hidden, joint 19 shown). Both only set the base,
    /// which the first status change applies.
    pub fn new(kind: FighterKind) -> Self {
        let Some((present, dl)) = joint_masks(kind) else {
            return ModelParts::default();
        };
        let mut p = ModelParts {
            present,
            ..ModelParts::default()
        };
        if let Some(t) = texture_part_table(kind) {
            p.texture.joint = [t[0].0, t[1].0];
        }
        // `lbCommonSetupFighterPartsDObjs` makes every joint from the
        // `FTCommonPart` chain, costume script included.
        let made = texture_part_made(kind);
        for (part, &(ids, has_parts)) in made.iter().enumerate() {
            p.texture.made[part] = ids;
            p.texture.joint_has_parts[part] = has_parts;
            p.texture.shown[part] = ids[p.detail_curr.index()];
        }
        p.texture.missing_part[0] = match kind {
            FighterKind::Kirby => Some(2),
            FighterKind::Ness => Some(1),
            _ => None,
        };
        for i in 0..PARTS_MAX {
            let id = if dl >> i & 1 != 0 { 0 } else { HIDDEN };
            p.base[i] = id;
            p.curr[i] = id;
        }
        match kind {
            // `copy[copy_kind].copy_modelpart_id`, Kirby's own row: 0.
            FighterKind::Kirby => p.set_default(6, 0),
            FighterKind::Link | FighterKind::PolyLink => {
                p.set_default(21, HIDDEN);
                p.set_default(19, 0);
            }
            _ => {}
        }
        p
    }

    fn exists(&self, joint: u8) -> bool {
        joint
            .checked_sub(JOINT_COMMON_START)
            .is_some_and(|i| usize::from(i) < PARTS_MAX && self.present >> i & 1 != 0)
    }

    /// `ftParamSetModelPartID`.
    pub fn set(&mut self, joint: u8, id: i8) {
        if !self.exists(joint) {
            return;
        }
        let i = usize::from(joint - JOINT_COMMON_START);
        if self.curr[i] != id {
            self.curr[i] = id;
            self.is_modify = true;
            self.remake(joint);
        }
    }

    /// `gcRemoveMObjAll` on `joint`, then (unless hidden) new `MObj`s
    /// (`ftParamSetModelPartID`): `texture_id_curr` 0, then what the
    /// costume script sets. A joint with an `FTModelPartDesc` takes its
    /// part's `MObj`s and scripts, which set no texture id (`romtool
    /// costumes`); one without takes the `FTCommonPart` chain again
    /// ([`texture_part_made`]).
    fn remake(&mut self, joint: u8) {
        let t = &mut self.texture;
        for p in 0..TEXTURE_PARTS {
            if t.joint[p] == joint {
                t.shown[p] = if t.joint_has_parts[p] {
                    0
                } else {
                    t.made[p][self.detail_curr.index()]
                };
            }
        }
    }

    /// Whether the texture part's `MObj` exists: its joint is made and not
    /// hidden, and its current model part has the named material.
    fn texture_mobj(&self, part: usize) -> bool {
        let joint = self.texture.joint[part];
        if joint < JOINT_COMMON_START {
            return false;
        }
        let i = usize::from(joint - JOINT_COMMON_START);
        i < PARTS_MAX
            && self.present >> i & 1 != 0
            && self.curr[i] != HIDDEN
            && self.texture.missing_part[part] != Some(self.curr[i])
    }

    /// `ftParamSetTexturePartID`.
    pub fn set_texture(&mut self, part: usize, id: i8) {
        if part >= TEXTURE_PARTS || !self.texture_mobj(part) {
            return;
        }
        self.texture.shown[part] = id;
        self.texture.curr[part] = id;
        self.texture.is_modify = true;
    }

    /// `ftParamResetTexturePartAll`.
    pub fn reset_textures(&mut self) {
        for p in 0..TEXTURE_PARTS {
            let t = &mut self.texture;
            if t.curr[p] != t.base[p] {
                t.curr[p] = t.base[p];
                if self.texture_mobj(p) {
                    self.texture.shown[p] = self.texture.base[p];
                }
            }
        }
        self.texture.is_modify = false;
    }

    /// `ftParamInitTexturePartAll`.
    pub fn init_textures(&mut self) {
        for p in 0..TEXTURE_PARTS {
            if self.texture.curr[p] != self.texture.base[p] && self.texture_mobj(p) {
                self.texture.shown[p] = self.texture.curr[p];
            }
        }
        self.texture.is_modify = true;
    }

    /// `ftParamSetModelPartDetailAll`: every shown joint takes its part
    /// again at the new detail (new `MObj`s), then the texture parts are
    /// applied to them.
    pub fn set_detail_all(&mut self, detail: Detail) {
        if detail == self.detail_curr {
            return;
        }
        self.detail_curr = detail;
        for i in 0..PARTS_MAX {
            if self.present >> i & 1 != 0 && self.curr[i] != HIDDEN {
                self.remake(i as u8 + JOINT_COMMON_START);
            }
        }
        self.is_modify = true;
        self.init_textures();
    }

    /// The texture id each part's `MObj` shows, for the renderer.
    pub fn draw_textures(&self) -> [i8; TEXTURE_PARTS] {
        self.texture.shown
    }

    /// `ftParamSetModelPartDefaultID`: the base only (no joint check).
    pub fn set_default(&mut self, joint: u8, id: i8) {
        if let Some(slot) = joint
            .checked_sub(JOINT_COMMON_START)
            .and_then(|i| self.base.get_mut(usize::from(i)))
        {
            *slot = id;
            self.is_modify = true;
        }
    }

    /// `ftMainSetStatus`'s hidden-part walk over the old and new
    /// `anim_desc`: a part the new motion names is made
    /// (`ftMainUpdateHiddenPartID`: shown on its own list, or hidden
    /// without one), kept, or ejected when it no longer names it.
    pub fn apply_hidden_parts(&mut self, kind: FighterKind, anim_desc: u32) {
        let new = anim_desc & !ANIM_DESC_FLAGS;
        let old = self.hidden_bits;
        if self.present != 0 {
            for &(index, root, dl) in hidden_parts(kind) {
                let bit = 0x8000_0000u32 >> index;
                let n = usize::from(root - JOINT_COMMON_START);
                if n >= PARTS_MAX {
                    continue;
                }
                match (old & bit != 0, new & bit != 0) {
                    (false, true) => {
                        self.present |= 1 << n;
                        let id = if dl { 0 } else { HIDDEN };
                        self.base[n] = id;
                        self.curr[n] = id;
                    }
                    (true, false) => self.present &= !(1u64 << n),
                    _ => {}
                }
            }
        }
        self.hidden_bits = new;
    }

    /// `ftParamResetModelPartAll`.
    pub fn reset_all(&mut self) {
        for i in 0..PARTS_MAX {
            if self.present >> i & 1 != 0 && self.curr[i] != self.base[i] {
                self.curr[i] = self.base[i];
                self.remake(i as u8 + JOINT_COMMON_START);
            }
        }
        self.is_modify = false;
    }

    /// `ftParamHideModelPartAll`.
    pub fn hide_all(&mut self) {
        for i in 0..PARTS_MAX {
            if self.present >> i & 1 != 0 && self.curr[i] != HIDDEN {
                self.curr[i] = HIDDEN;
                self.remake(i as u8 + JOINT_COMMON_START);
            }
        }
        self.is_modify = true;
    }

    /// The part descriptor `node` (joint `node + 4`) draws: `None` for a
    /// fighter the table does not cover (every node its own list),
    /// [`ABSENT`] for a never-made joint, [`HIDDEN`] for a hidden one, else
    /// its `modelpart_id_curr`.
    pub fn node_part(&self, node: usize) -> Option<i8> {
        if self.present == 0 {
            return None;
        }
        if node >= PARTS_MAX || self.present >> node & 1 == 0 {
            return Some(ABSENT);
        }
        Some(self.curr[node])
    }

    /// [`Self::node_part`] for descriptors `0..N`, for the renderer; `None`
    /// for a fighter the table does not cover.
    pub fn draw_parts(&self) -> Option<[i8; PARTS_MAX]> {
        (self.present != 0).then(|| core::array::from_fn(|n| self.node_part(n).unwrap_or(0)))
    }
}

/// `ftMainSetStatus`: `if (!(flags & FTSTATUS_PRESERVE_MODELPART) &&
/// fp->is_modelpart_modify) ftParamResetModelPartAll(fighter_gobj)`, the
/// flag from every call that passes it
/// ([`crate::colanim::PRESERVE_MODELPART`]).
///
/// Before it, `if (fp->detail_curr != fp->detail_base)
/// ftParamSetModelPartDetailAll(fighter_gobj, fp->detail_base)`; after it
/// the texture parts reset the same way unless the status keeps them
/// (`FTSTATUS_PRESERVE_TEXTUREPART`, RE-426).
pub(crate) fn on_set_status(f: &mut Fighter, to: crate::status::AnyStatus) {
    // `ftMainSetStatus`'s hidden parts, for a motion with a figatree.
    if let Some(d) = crate::motion::motion_desc(f.kind, to).filter(|d| d.anim_length != 0) {
        f.model_parts.apply_hidden_parts(f.kind, d.anim_flags);
    }
    let base = f.model_parts.detail_base;
    f.model_parts.set_detail_all(base);
    let preserve = |table| crate::colanim::preserved_in(table, f.kind, f.status.status, to);
    // `ftHammerGetStatUpdateFlags`: every Hammer status plays HammerWait
    // or HammerWalk and keeps these parts when switching within that set.
    let hammer = crate::item_use::is_hammer(f.status.status) && crate::item_use::is_hammer(to);
    let keep_model = hammer || preserve(&crate::colanim::PRESERVE_MODELPART);
    let keep_texture = hammer || preserve(&crate::colanim::PRESERVE_TEXTUREPART);
    if !keep_model && f.model_parts.is_modify {
        f.model_parts.reset_all();
    }
    if !keep_texture && f.model_parts.texture.is_modify {
        f.model_parts.reset_textures();
    }
}

/// `ftParamGetJointID`: -2 is `joint_itemlight_id`.
pub(crate) fn motion_joint(f: &Fighter, joint: i32) -> Option<u8> {
    if joint == -2 {
        return crate::motion::combat_attrs(f.kind).map(|a| a.joint_itemlight_id);
    }
    u8::try_from(joint).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn a_catch_makes_samuss_beam_strands_and_yoshis_tongue_then_ejects_them() {
        let mut f = Fighter::new(FighterKind::Samus, 0, 3);
        assert_eq!(f.model_parts.node_part(20), Some(ABSENT));
        on_set_status(&mut f, crate::status::Status::Catch.into());
        // Joints 24 and 25 draw their lists; the chain (17..23) is joints.
        assert_eq!(f.model_parts.node_part(20), Some(0));
        assert_eq!(f.model_parts.node_part(21), Some(0));
        assert_eq!(f.model_parts.node_part(13), Some(HIDDEN));
        on_set_status(&mut f, crate::status::Status::Wait.into());
        assert_eq!(f.model_parts.node_part(20), Some(ABSENT));
        assert_eq!(f.model_parts.node_part(13), Some(ABSENT));

        let mut y = Fighter::new(FighterKind::Yoshi, 0, 3);
        assert_eq!(y.model_parts.node_part(5), Some(ABSENT));
        on_set_status(&mut y, crate::status::Status::Catch.into());
        assert_eq!(y.model_parts.node_part(5), Some(0));
    }

    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn joints_start_shown_with_a_list_and_hidden_without() {
        let p = ModelParts::new(FighterKind::Mario);
        // Mario's descriptor 0 (TransN's child) has no list; 2 has one.
        assert_eq!(p.node_part(0), Some(HIDDEN));
        assert_eq!(p.node_part(2), Some(0));
        // Descriptor 24 is not made (`setup_parts` 0xFFFFFF).
        assert_eq!(p.node_part(24), Some(ABSENT));
        assert!(!p.is_modify);
        // A fighter the table does not cover draws every node as packed.
        assert_eq!(ModelParts::new(FighterKind::Boss).node_part(3), None);
    }

    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn a_set_part_lasts_until_the_reset_and_a_missing_joint_ignores_it() {
        let mut p = ModelParts::new(FighterKind::Donkey);
        p.set(12, 2);
        assert_eq!(p.node_part(8), Some(2));
        assert!(p.is_modify);
        p.reset_all();
        assert_eq!(p.node_part(8), Some(0));
        assert!(!p.is_modify);
        // Samus's joints 17..25 are never made.
        let mut s = ModelParts::new(FighterKind::Samus);
        s.set(17, 0);
        assert_eq!(s.node_part(13), Some(ABSENT));
        assert!(!s.is_modify);
    }

    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn link_moves_his_shield_to_his_back_on_the_first_reset() {
        let mut p = ModelParts::new(FighterKind::Link);
        // Joint 19 has the shield's list; joint 21 has none.
        assert_eq!((p.node_part(15), p.node_part(17)), (Some(0), Some(HIDDEN)));
        assert!(p.is_modify);
        p.set(21, 0);
        p.set(19, HIDDEN);
        p.reset_all();
        assert_eq!((p.node_part(15), p.node_part(17)), (Some(0), Some(HIDDEN)));
    }

    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn hide_all_hides_every_made_joint() {
        let mut p = ModelParts::new(FighterKind::Kirby);
        p.hide_all();
        assert!((0..PARTS_MAX).all(|n| matches!(p.node_part(n), Some(HIDDEN | ABSENT))));
        p.reset_all();
        assert_eq!(p.node_part(2), Some(0));
    }

    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn donkey_kong_dashes_with_his_second_face_and_waits_with_his_own() {
        // `dDonkeyMainMotion_Dash` opens with `SetModelPartID(12, 2)`.
        let mut f = Fighter::new(FighterKind::Donkey, 0, 3);
        f.situation = crate::fighter::Situation::Ground;
        crate::status::set_dash(&mut f);
        assert_eq!(f.model_parts.node_part(8), Some(2));
        assert!(f.model_parts.is_modify);
        crate::status::set_wait(&mut f);
        assert_eq!(f.model_parts.node_part(8), Some(0));
    }

    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn a_preserving_switch_keeps_the_part() {
        use crate::status::{AnyStatus, StatusTiming};
        // Yoshi's ground and air Egg Lay switch with
        // `FTSTATUS_PRESERVE_MODELPART`; entering Wait does not.
        let mut f = Fighter::new(FighterKind::Yoshi, 0, 3);
        let ground = AnyStatus::Yoshi(crate::status::YoshiStatus::SpecialN);
        let air = AnyStatus::Yoshi(crate::status::YoshiStatus::SpecialAirN);
        assert_eq!((ground.id(), air.id()), (0xE4, 0xE7));
        crate::status::set_any_status(&mut f, ground, 0.0, StatusTiming::unknown());
        f.model_parts.set(7, 1);
        crate::status::set_any_status(&mut f, air, 0.0, StatusTiming::unknown());
        assert_eq!(f.model_parts.node_part(3), Some(1));
        crate::status::set_wait(&mut f);
        assert_eq!(f.model_parts.node_part(3), Some(0));
    }

    /// The walk finds exactly the decompilation's `SetModelPartID` events
    /// (`relocData/*MainMotion.c` and `scsubsysdata*.c`); Samus's 0x0D10 script (joints 17..25,
    /// never made) is reached too.
    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn the_scripts_name_the_decomps_model_part_events() {
        use crate::motion::model_part_events;
        let v = |k| model_part_events(k).into_iter().collect::<Vec<_>>();
        assert_eq!(v(FighterKind::Mario), [(10, 1), (16, 1)]);
        assert_eq!(v(FighterKind::Fox), [(10, 1), (16, 1), (17, -1), (17, 0)]);
        assert_eq!(
            v(FighterKind::Donkey),
            [
                (10, 0),
                (10, 1),
                (12, 0),
                (12, 1),
                (12, 2),
                (16, 0),
                (16, 1)
            ]
        );
        assert_eq!(v(FighterKind::Yoshi), [(7, 0), (7, 1)]);
        assert_eq!(
            v(FighterKind::Kirby),
            [
                (6, 1),
                (6, 2),
                (6, 14),
                (7, 0),
                (12, -1),
                (12, 0),
                (17, 0),
                (18, 0),
                (19, 0)
            ]
        );
        assert_eq!(
            v(FighterKind::Ness),
            [(10, 2), (12, 1), (16, 2), (17, 0), (17, 1), (30, 0)]
        );
        assert_eq!(v(FighterKind::Samus).len(), 11);
        // Link's stage-card poses (IntroL and IntroR, `D_ovl1_803919EC`) add
        // four: joints 22 to 24 to part 1 and joint 11 to part 2.
        let link = v(FighterKind::Link);
        assert_eq!(link.len(), 16);
        for e in [(22, 1), (23, 1), (24, 1), (11, 2)] {
            assert!(link.contains(&e), "{e:?}");
        }
        assert_eq!(v(FighterKind::Captain).len(), 5);
    }

    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn texture_status_and_material_differ_after_a_model_part_remake() {
        let mut p = ModelParts::new(FighterKind::Kirby);
        p.set_texture(0, 5);
        p.set(6, 1);
        assert_eq!(p.texture.curr[0], 5);
        assert_eq!(p.draw_textures()[0], 0);
        p.init_textures();
        assert_eq!(p.draw_textures()[0], 5);
        p.reset_textures();
        assert_eq!(p.draw_textures()[0], 0);
        p.hide_all();
        p.set_texture(0, 8);
        assert_eq!(p.texture.curr[0], 0);
    }

    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn luigi_eye_is_made_on_sprite_one_for_every_costume() {
        // RE-569: file 323's costume scripts 0x29A4 (high) and 0x4F74
        // (low) set TextureIDCurrent 1 on the eye `MObj` (joint 12) at
        // every costume; the status stays 0.
        assert_eq!(
            texture_part_made(FighterKind::Luigi),
            [([1, 1], false), ([0, 0], false)]
        );
        for costume in 0..4 {
            let d = crate::demo_fighter::DemoFighter::new(FighterKind::Luigi, 0, costume);
            assert_eq!(d.parts().draw_textures(), [1, 0], "costume {costume}");
            let mut f = Fighter::new(FighterKind::Luigi, 0, 3);
            f.costume = costume;
            let p = &mut f.model_parts;
            assert_eq!((p.draw_textures()[0], p.texture.curr[0]), (1, 0));
            // `ftParamResetTexturePartAll` writes only a changed status.
            p.reset_textures();
            assert_eq!(p.draw_textures()[0], 1);
            p.set_detail_all(Detail::Low);
            assert_eq!(p.draw_textures()[0], 1, "low-detail remake");
            p.set_texture(0, 2);
            p.reset_textures();
            assert_eq!(p.draw_textures()[0], 0, "reset writes the base");
            p.set_detail_all(Detail::High);
            assert_eq!(p.draw_textures()[0], 1, "high-detail remake");
        }
        // Every other fighter's texture parts are made on sprite 0, and so
        // is every part whose joint has its own `FTModelPart`s.
        for kind in (0..27).filter_map(FighterKind::from_ordinal) {
            for (part, (ids, has_parts)) in texture_part_made(kind).into_iter().enumerate() {
                if kind != FighterKind::Luigi || has_parts {
                    assert_eq!(ids, [0, 0], "{kind:?} part {part}");
                }
            }
        }
    }

    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn luigi_eye_after_a_status_detail_restore_matches_the_n64_trace() {
        // RE-569 N64 RDRAM trace (Training, Luigi P1, stick at x=60): the
        // Wait blink ends on 0; with `detail_curr` set to low, Dash's
        // `ftParamSetModelPartDetailAll(detail_base)` remakes the eye `MObj`
        // with `texture_id_curr` 1, which Run and RunBrake keep. Without the
        // detail change it stays 0.
        use crate::status::{AnyStatus, Status, StatusTiming};
        for low in [true, false] {
            let mut f = Fighter::new(FighterKind::Luigi, 0, 3);
            f.model_parts.set_texture(0, 3);
            f.model_parts.set_texture(0, 0);
            if low {
                f.model_parts.detail_curr = Detail::Low;
            }
            for status in [Status::Dash, Status::Run, Status::RunBrake] {
                crate::status::set_any_status(
                    &mut f,
                    AnyStatus::Common(status),
                    0.0,
                    StatusTiming::unknown(),
                );
                assert_eq!(f.model_parts.detail_curr, Detail::High);
                assert_eq!(
                    f.model_parts.draw_textures()[0],
                    i8::from(low),
                    "{status:?} low {low}"
                );
            }
        }
        // WalkSlow's script sets 3 at time 0, after the remake (N64: 3).
        let mut f = Fighter::new(FighterKind::Luigi, 0, 3);
        f.model_parts.set_texture(0, 0);
        f.model_parts.detail_curr = Detail::Low;
        crate::status::set_any_status(
            &mut f,
            AnyStatus::Common(Status::WalkSlow),
            0.0,
            StatusTiming::unknown(),
        );
        assert_eq!(f.model_parts.draw_textures()[0], 3);
    }

    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn detail_switch_reapplies_the_face_and_status_restores_the_base() {
        use crate::status::{AnyStatus, Status, StatusTiming};
        assert_eq!(Detail::for_fighters(2), Detail::High);
        assert_eq!(Detail::for_fighters(3), Detail::Low);
        assert_eq!(Detail::for_fighters(4), Detail::Low);
        let mut f = Fighter::new(FighterKind::Mario, 0, 3);
        f.model_parts.detail_base = Detail::Low;
        f.model_parts.set_texture(0, 3);
        f.model_parts.set_detail_all(Detail::Low);
        assert_eq!(f.model_parts.draw_textures()[0], 3);
        f.model_parts.set_detail_all(Detail::High);
        crate::status::set_any_status(
            &mut f,
            AnyStatus::Common(Status::Fall),
            0.0,
            StatusTiming::unknown(),
        );
        assert_eq!(f.model_parts.detail_curr, Detail::Low);
        assert_eq!(f.model_parts.texture.curr[0], 0);
    }
}
