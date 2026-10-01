//! Mushroom Kingdom's POW Block, `itpowerblock.c`. The stage drops it at
//! one of its spawn points; the first fighter hit squashes it, quakes the
//! ground for every grounded fighter but the hitter
//! (`grInishiePowerBlockSetDamage`), and the block goes once its squash
//! animation ends (`grInishiePowerBlockSetWait`).

use ssb_engine::math::Vec3;

use super::{
    HitProc, Item, ItemAnim, ItemAnims, ItemAttributes, ItemKind, ItemStatus, ItemType, ItemWeight,
    StageItemEvent,
};
use crate::combat::{AttackState, Element, HitStatus};
use crate::ground::BodyColl;

/// `llGRInishieMapPowerBlockItemAttributes` (offset 0xD8 of relocData file
/// 260, US): light, a 400 x 300 x 300 damage box, map box 110 either side
/// and 159 wide, an attack it never turns on, a fighter item with hit
/// status none until it settles.
pub static ATTRIBUTES: ItemAttributes = ItemAttributes {
    is_give_hitlag: true,
    weight: ItemWeight::Light,
    attack_offsets: [Vec3::ZERO, Vec3::ZERO],
    damage_coll_offset: Vec3::ZERO,
    damage_coll_size: Vec3::new(400.0, 300.0, 300.0),
    map_coll: BodyColl {
        top: 110.0,
        center: 0.0,
        bottom: -110.0,
        width: 159.0,
    },
    size: 10.0,
    angle: 361,
    kb_scale: 100,
    damage: 5,
    element: Element::Normal,
    kb_weight: 0,
    shield_damage: 0,
    attack_count: 1,
    can_setoff: true,
    priority: 1,
    can_rehit_item: true,
    can_rehit_fighter: false,
    can_hop: true,
    can_reflect: false,
    can_shield: true,
    kb_base: 20,
    ty: ItemType::Fighter,
    hitstatus: HitStatus::None,
    vel_scale: 100,
};

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
pub(super) fn make(pos: Vec3, motion_count: u16) -> Item {
    let mut item = Item::new(
        ItemKind::PowerBlock,
        &ATTRIBUTES,
        ItemStatus::PowerBlock(Status::Init),
        AttackState::Off,
        pos,
        Vec3::ZERO,
        motion_count,
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

/// `itPowerBlockWaitProcDamage`: the hitter's handicap and port go with
/// the quake.
pub(super) fn hit_proc(
    item: &mut Item,
    status: Status,
    proc: HitProc,
    anims: &mut dyn ItemAnims,
    events: &mut dyn FnMut(StageItemEvent),
) -> Option<bool> {
    match (status, proc) {
        (Status::Wait, HitProc::Damage) => {
            item.status = ItemStatus::PowerBlock(Status::Damaged);
            item.damage_coll.hitstatus = HitStatus::None;
            let write = anims.add_play(item.anim_target(), ItemAnim::PowerBlockDamage);
            item.apply_root_write(write);
            events(StageItemEvent::PowerBlockDamage {
                handicap: item.damage_handicap,
                hitter: item.damage_by,
            });
            Some(true)
        }
        _ => None,
    }
}
