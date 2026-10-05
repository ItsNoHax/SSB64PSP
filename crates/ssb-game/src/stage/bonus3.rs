//! Race to the Finish (`grbonus3.c`): the four animated Bumpers, the
//! bomb-barrel dropper and the finish-gate check.
//!
//! `grBonus3MakeGround` makes the Bumpers from `llGRBonus3MapBumpersDObjDesc`
//! (file 162 at 0, US), skipping descriptor 0 and stopping at the
//! `DOBJ_ARRAY_MAX` terminator, each with its `llGRBonus3MapBumpersAnimJoint`
//! entry (file 162 at 0x110) added to the item's root and played at once.
//! The runtime plays those scripts through [`crate::item::ItemAnims`] as
//! [`crate::item::ItemAnimTarget::Bonus3Bumper`]: they move the Bumpers, so
//! gameplay reads them.
//!
//! Its two processes are Ground-link GObjs at priority 4, made in this
//! order after the Bumpers.

use super::{objects_of, StageInit, StageItem, StageItems};
use crate::fighter::Fighter;
use ssb_engine::math::Vec3;

/// `nMPMapObjKind1PGameBonus3TaruBomb`.
pub const MAPOBJ_TARUBOMB: u16 = 0x29;
/// `grBonus3TaruBombMakeActor` and `grBonus3TaruBombProcUpdate`'s reset.
pub const TARUBOMB_MAKE_WAIT: i32 = 180;
/// `nMPMaterialDetect`: the finish gate's floor.
pub const MATERIAL_DETECT: u16 = 14;

/// One `llGRBonus3MapBumpersDObjDesc` entry past descriptor 0: its
/// translation, and whether its `llGRBonus3MapBumpersAnimJoint` entry is
/// set (all four are in the US ROM).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BumperDesc {
    pub translate: Vec3,
    pub animated: bool,
}

/// `GRCommonGroundVarsBonus3`, less the file heads.
#[derive(Debug, Clone, PartialEq)]
pub struct Bonus3 {
    /// The Bumpers made, in descriptor order.
    pub bumpers: [Option<u32>; BUMPERS_MAX],
    pub tarubomb_make_pos: Vec3,
    pub tarubomb_make_wait: i32,
    /// `gSCManagerSceneData.player`: the fighter the finish check reads.
    pub player: u8,
    /// `grBonus3FinishProcUpdate` found the player on the gate this frame:
    /// the source announces "Complete!" (`ifCommonAnnounceCompleteInitInterface`,
    /// which ends the battle) and queues the bonus-complete sound. The PSP
    /// campaign consumes this flag immediately after the priority-4 stage tick.
    pub complete: bool,
}

/// The descriptors before the terminator in the US file: four.
pub const BUMPERS_MAX: usize = 4;

impl Bonus3 {
    /// `grBonus3MakeGround`: `grBonus3InitHeaders`, `grBonus3MakeBumpers`,
    /// `grBonus3TaruBombMakeActor` and `grBonus3FinishMakeActor`.
    ///
    /// `grBonus3TaruBombMakeActor` spins forever printing "Too many
    /// barrels!" unless the map holds exactly one barrel point; the port
    /// panics instead.
    pub fn new(init: &StageInit<'_>, items: &mut dyn StageItems) -> Self {
        let mut bumpers = [None; BUMPERS_MAX];
        for (i, desc) in init.bonus3_bumpers.iter().enumerate() {
            // `itManagerMakeItemSetupCommon(NULL, nITKindGBumper, ...)`;
            // the script plays only on an item that was made.
            let handle = items.make_item(
                StageItem::Bumper {
                    castle: false,
                    joint: desc.animated.then_some(i as u8),
                },
                desc.translate,
            );
            if let Some(slot) = bumpers.get_mut(i) {
                *slot = handle;
            }
        }
        let mut points = objects_of(init.map_objects, MAPOBJ_TARUBOMB);
        let tarubomb_make_pos = points.next();
        assert!(
            tarubomb_make_pos.is_some() && points.next().is_none(),
            "Too many barrels!"
        );
        Bonus3 {
            bumpers,
            tarubomb_make_pos: tarubomb_make_pos.unwrap_or(Vec3::ZERO),
            tarubomb_make_wait: TARUBOMB_MAKE_WAIT,
            player: init.player,
            complete: false,
        }
    }

    /// `grBonus3TaruBombProcUpdate`, then `grBonus3FinishProcUpdate`. The
    /// first barrel drops on the 181st tick, then one every 180; a full
    /// item pool skips that drop.
    pub fn tick(&mut self, fighters: &[&mut Fighter], items: &mut dyn StageItems) {
        if self.tarubomb_make_wait == 0 {
            items.make_item(StageItem::TaruBomb, self.tarubomb_make_pos);
            self.tarubomb_make_wait = TARUBOMB_MAKE_WAIT;
        }
        self.tarubomb_make_wait -= 1;

        self.complete = fighters
            .iter()
            .find(|f| f.port == self.player)
            .is_some_and(|f| on_gate(f));
    }
}

/// `fp->ga == nMPKineticsGround` on a floor whose material is
/// `nMPMaterialDetect`.
fn on_gate(f: &Fighter) -> bool {
    f.is_grounded() && f.floor.is_some_and(|s| s.material() == MATERIAL_DETECT)
}
