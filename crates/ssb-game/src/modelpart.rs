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

/// Per fighter, bit `n` for descriptor `n` (joint `n + 4`): which
/// descriptors `lbCommonSetupFighterPartsDObjs` makes a joint of
/// (`setup_parts`), and which of those have a high-detail display list
/// (`commonparts[0].dobjdesc[n].dl`). Read from each fighter's `*Main` and
/// `*Model` files; `crates/ssb-rom/tests/model_parts.rs` checks them
/// against the ROM.
const JOINT_MASKS: [(u64, u64); 12] = [
    // Mario: 25 descriptors
    (0xffffff, 0xb59d74),
    // Fox: 27 descriptors
    (0x3ffffff, 0x3b59df6),
    // Donkey: 26 descriptors
    (0x1ffffff, 0x16b5d76),
    // Samus: 33 descriptors
    (0xffc01fff, 0xb5801a76),
    // Luigi: 25 descriptors
    (0xffffff, 0xb59d74),
    // Link: 32 descriptors
    (0x7fff9fff, 0x5adc8ef6),
    // Yoshi: 28 descriptors
    (0x7ffffdf, 0x5adbb9e),
    // Captain: 26 descriptors
    (0x1ffffff, 0x16b5df6),
    // Kirby: 27 descriptors
    (0x3ff3ef7, 0x21018c4),
    // Pikachu: 27 descriptors
    (0x3ffffff, 0x36b66e6),
    // Purin: 26 descriptors
    (0x1ff9ff7, 0x1080cc4),
    // Ness: 27 descriptors
    (0x3ffffff, 0x34d1d74),
];

/// `(setup_parts, has a display list)` for a playable fighter, as
/// [`JOINT_MASKS`] holds them.
pub fn joint_masks(kind: FighterKind) -> Option<(u64, u64)> {
    JOINT_MASKS
        .get(kind as usize)
        .copied()
        .filter(|_| (kind as u8) < 12)
}

/// `FTStruct::modelpart_status` and `is_modelpart_modify`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelParts {
    /// `setup_parts`: descriptor `n`'s joint exists. Zero for a fighter the
    /// table does not cover, whose parts never change.
    pub present: u64,
    pub base: [i8; PARTS_MAX],
    pub curr: [i8; PARTS_MAX],
    pub is_modify: bool,
}

impl Default for ModelParts {
    fn default() -> Self {
        ModelParts {
            present: 0,
            base: [0; PARTS_MAX],
            curr: [0; PARTS_MAX],
            is_modify: false,
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
        for i in 0..PARTS_MAX {
            let id = if dl >> i & 1 != 0 { 0 } else { HIDDEN };
            p.base[i] = id;
            p.curr[i] = id;
        }
        match kind {
            // `copy[copy_kind].copy_modelpart_id`, Kirby's own row: 0.
            FighterKind::Kirby => p.set_default(6, 0),
            FighterKind::Link => {
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
        }
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

    /// `ftParamResetModelPartAll`.
    pub fn reset_all(&mut self) {
        for i in 0..PARTS_MAX {
            if self.present >> i & 1 != 0 {
                self.curr[i] = self.base[i];
            }
        }
        self.is_modify = false;
    }

    /// `ftParamHideModelPartAll`.
    pub fn hide_all(&mut self) {
        for i in 0..PARTS_MAX {
            if self.present >> i & 1 != 0 {
                self.curr[i] = HIDDEN;
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

/// A demo fighter's parts (the selects' and the results' fighters):
/// `ftManagerMakeFighter`'s, then `scSubsysFighterSetStatus`'s first
/// `ftMainSetStatus` applies the make's defaults. The demo statuses'
/// motion scripts are not run (RE-425), so the parts stay there.
pub fn demo(kind: FighterKind) -> ModelParts {
    let mut p = ModelParts::new(kind);
    if p.is_modify {
        p.reset_all();
    }
    p
}

/// `ftMainSetStatus`: `if (!(flags & FTSTATUS_PRESERVE_MODELPART) &&
/// fp->is_modelpart_modify) ftParamResetModelPartAll(fighter_gobj)`, the
/// flag from every call that passes it
/// ([`crate::colanim::PRESERVE_MODELPART`]).
pub(crate) fn on_set_status(f: &mut Fighter, to: crate::status::AnyStatus) {
    let preserve = crate::colanim::preserved_in(
        &crate::colanim::PRESERVE_MODELPART,
        f.kind,
        f.status.status,
        to,
    );
    if !preserve && f.model_parts.is_modify {
        f.model_parts.reset_all();
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
    fn hide_all_hides_every_made_joint() {
        let mut p = ModelParts::new(FighterKind::Kirby);
        p.hide_all();
        assert!((0..PARTS_MAX).all(|n| matches!(p.node_part(n), Some(HIDDEN | ABSENT))));
        p.reset_all();
        assert_eq!(p.node_part(2), Some(0));
    }

    #[test]
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
    /// (`relocData/*MainMotion.c`); Samus's 0x0D10 script (joints 17..25,
    /// never made) is reached too.
    #[test]
    fn the_scripts_name_the_decomps_model_part_events() {
        use crate::motion::model_part_events;
        let v = |k| model_part_events(k).into_iter().collect::<Vec<_>>();
        assert!(v(FighterKind::Mario).is_empty());
        assert_eq!(v(FighterKind::Fox), [(17, 0)]);
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
            [(10, 2), (12, 1), (16, 2), (17, 0), (30, 0)]
        );
        assert_eq!(v(FighterKind::Samus).len(), 11);
        assert_eq!(v(FighterKind::Link).len(), 12);
        assert_eq!(v(FighterKind::Captain).len(), 5);
    }
}
