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

/// `FTAttributes::textureparts_container`, per playable fighter: each
/// part's joint and the position of its `MObj` in that joint's chain at
/// high and low detail; `None` for a NULL container (Donkey Kong, Samus).
/// A fighter the decompilation gives one part reads the next bytes as its
/// second (joint 0, `TopN`, which has no `MObj`) and never sets it.
/// `crates/ssb-rom/tests/texture_parts.rs` checks them against the ROM.
pub type TexturePartTable = [(u8, [u8; 2]); TEXTURE_PARTS];

const TEXTURE_PART_TABLE: [Option<TexturePartTable>; 12] = [
    Some([(12, [0, 0]), (0, [0, 0])]),  // Mario
    Some([(12, [0, 0]), (12, [1, 1])]), // Fox
    None,                               // Donkey
    None,                               // Samus
    Some([(12, [0, 0]), (0, [0, 0])]),  // Luigi
    Some([(23, [0, 0]), (23, [1, 1])]), // Link
    Some([(7, [0, 0]), (7, [1, 1])]),   // Yoshi
    Some([(12, [0, 0]), (0, [0, 0])]),  // Captain
    Some([(6, [0, 0]), (0, [0, 0])]),   // Kirby
    Some([(11, [0, 0]), (11, [1, 1])]), // Pikachu
    Some([(6, [0, 0]), (6, [1, 1])]),   // Purin
    Some([(12, [0, 0]), (0, [0, 0])]),  // Ness
];

/// [`TEXTURE_PART_TABLE`]'s row for `kind`.
pub fn texture_part_table(kind: FighterKind) -> Option<TexturePartTable> {
    TEXTURE_PART_TABLE
        .get(kind as usize)
        .copied()
        .flatten()
        .filter(|_| (kind as u8) < 12)
}

/// `FTStruct::texturepart_status` and `is_texturepart_modify`, with what
/// each part's `MObj` shows (RE-426).
///
/// `ftParamSetTexturePartID` writes `texture_id_curr` of the joint's
/// `detail`th `MObj`, which `gcDrawMObjForDObj` reads to load
/// `sprites[texture_id_curr]`, and records it in the status. A model-part
/// change on that joint replaces its `MObj`s (`gcRemoveMObjAll`, then
/// `gcAddMObjForDObj`, which zeroes `texture_id_curr`), so what the face
/// shows can differ from the status until the next set.
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
            self.remake(joint);
        }
    }

    /// `gcRemoveMObjAll` on `joint`, then (unless hidden) new `MObj`s
    /// with `texture_id_curr` 0.
    fn remake(&mut self, joint: u8) {
        for p in 0..TEXTURE_PARTS {
            if self.texture.joint[p] == joint {
                self.texture.shown[p] = 0;
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

/// A demo fighter's parts (the selects' and the results' fighters,
/// RE-426): `ftManagerMakeFighter`'s, then `scSubsysFighterSetStatus`
/// (`ftMainSetStatus` with the make's defaults) and each frame's
/// `scSubsysFighterProcUpdate`, whose demo-status script swaps the model
/// and texture parts (Mario's blinks, Kirby's Win mouth, the claps' open
/// hands).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DemoParts {
    pub parts: ModelParts,
    script: crate::motion::DemoScript,
}

impl DemoParts {
    /// The fighter made, then set to demo status row `row`
    /// (`nFTDemoStatusNull + row`: 0 the selects' wait, 1 to 4 Win1 to
    /// Win4, 5 Lose).
    pub fn start(kind: FighterKind, row: usize) -> Self {
        let mut parts = ModelParts::new(kind);
        let script = crate::motion::DemoScript::start(kind, row, &mut parts);
        DemoParts { parts, script }
    }

    /// One `scSubsysFighterProcUpdate` frame.
    pub fn tick(&mut self) {
        self.script.tick(&mut self.parts);
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
    /// (`relocData/*MainMotion.c` and `scsubsysdata*.c`); Samus's 0x0D10 script (joints 17..25,
    /// never made) is reached too.
    #[test]
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
    fn mario_demo_blink_follows_the_subroutine_waits() {
        // scsubsysdatamario.c: sprite 2 for two frames, 3 for three,
        // 2 for two, then 0. The wait loop calls it again at tick 98.
        let mut d = DemoParts::start(FighterKind::Mario, 0);
        let mut ids = Vec::new();
        for _ in 0..9 {
            ids.push(d.parts.draw_textures()[0]);
            d.tick();
        }
        assert_eq!(ids, [2, 2, 3, 3, 3, 2, 2, 0, 0]);
        for _ in 9..98 {
            d.tick();
        }
        assert_eq!(d.parts.draw_textures()[0], 2);
    }

    #[test]
    fn kirby_win_mouth_changes_at_the_source_frame() {
        let mut d = DemoParts::start(FighterKind::Kirby, 1);
        for _ in 0..157 {
            d.tick();
        }
        assert_eq!(d.parts.draw_textures(), [0, 0]);
        d.tick();
        assert_eq!(d.parts.draw_textures(), [5, 0]);
        assert_eq!(
            DemoParts::start(FighterKind::Kirby, 5)
                .parts
                .draw_textures(),
            [5, 0]
        );
    }

    #[test]
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
