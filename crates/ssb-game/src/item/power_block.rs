//! Mushroom Kingdom's POW Block, `itpowerblock.c`. The stage drops it at
//! one of its spawn points; the first fighter hit squashes it, quakes the
//! ground for every grounded fighter but the hitter
//! (`grInishiePowerBlockSetDamage`), and the block goes once its squash
//! animation ends (`grInishiePowerBlockSetWait`).

use ssb_engine::math::Vec3;

use super::{
    HitProc, Item, ItemAnim, ItemAnims, ItemAttributes, ItemKind, ItemStatus, StageItemEvent,
};
use crate::combat::{AttackState, HitStatus};
use crate::wpeffect::{Emit, WeaponEffect as Fx};

/// `llGRInishieMapPowerBlockItemAttributes` (file 260, 0xD8).
pub static ATTRIBUTES: ItemAttributes = super::attributes::POWER_BLOCK;

/// `itPowerBlockStatus`, with the descriptor's own procs as `Init` and
/// `itPowerBlockWaitProcDamage`'s swapped `proc_update` as `Damaged`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Init,
    Wait,
    Damaged,
}

/// `itPowerBlockMakeItem` (`ITEM_FLAG_PARENT_GROUND`). `anim_joints` gives
/// the descriptor's child a pop-in; `itManagerMakeItem`'s
/// `lbCommonEjectTreeDObj` then makes that child the item's root, so the
/// block settles when the pop-in ends (RE-429).
pub(super) fn make(counts: &crate::manager_counts::ManagerCounts, pos: Vec3) -> Item {
    let mut item = Item::new(
        ItemKind::PowerBlock,
        &ATTRIBUTES,
        ItemStatus::PowerBlock(Status::Init),
        AttackState::Off,
        pos,
        Vec3::ZERO,
        counts,
    );
    item.update_attack_positions();
    item.damage_coll.interact_mask = super::INTERACT_FIGHTER;
    item
}

/// `itPowerBlockWaitSetStatus`.
fn wait_set_status(item: &mut Item) {
    item.set_status(ItemStatus::PowerBlock(Status::Wait));
    item.damage_coll.hitstatus = HitStatus::Normal;
}

/// `proc_update`. Returns whether the block lives on; `events` takes the
/// stage call the source makes.
pub(super) fn proc_update(
    item: &mut Item,
    status: Status,
    anims: &mut dyn ItemAnims,
    events: &mut dyn FnMut(StageItemEvent),
) -> bool {
    match status {
        // `itPowerBlockCommonProcUpdate`.
        Status::Init => {
            if anims.root_idle(item.anim_target()) {
                wait_set_status(item);
            }
            true
        }
        Status::Wait => true,
        // `itPowerBlockNDamageProcUpdate`.
        Status::Damaged => {
            if anims.root_idle(item.anim_target()) {
                events(StageItemEvent::PowerBlockGone);
                false
            } else {
                true
            }
        }
    }
}

/// `itPowerBlockWaitProcDamage`: the magnitude-3 quake, then the hitter's
/// handicap and port go with the stage's damage.
pub(super) fn hit_proc(
    item: &mut Item,
    status: Status,
    proc: HitProc,
    anims: &mut dyn ItemAnims,
    fx: &mut Emit,
    events: &mut dyn FnMut(StageItemEvent),
) -> Option<bool> {
    match (status, proc) {
        (Status::Wait, HitProc::Damage) => {
            item.status = ItemStatus::PowerBlock(Status::Damaged);
            item.damage_coll.hitstatus = HitStatus::None;
            let write = anims.add_play(item.anim_target(), ItemAnim::PowerBlockDamage);
            item.apply_root_write(write);
            crate::sound::play_fgm(crate::sound::id::nSYAudioFGMInishiePowerBlock);
            fx.push(Fx::Quake(3));
            events(StageItemEvent::PowerBlockDamage {
                handicap: item.damage_handicap,
                hitter: item.damage_by,
            });
            Some(true)
        }
        _ => None,
    }
}
