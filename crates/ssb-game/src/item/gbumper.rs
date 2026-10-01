//! Peach's Castle's Bumper, `itgbumper.c`. `grCastleInitAll` makes it on the
//! moving ground and `grCastleBumperProcUpdate` carries its X every frame.
//! It hits fighters only, never takes damage, and swells and lights up for
//! a few frames each time it hits.

use ssb_engine::math::Vec3;

use super::{HitProc, Item, ItemAttributes, ItemKind, ItemStatus, ItemType, ItemWeight};
use crate::combat::{AttackState, Element, HitStatus};
use crate::ground::BodyColl;

/// `ITBUMPER_CASTLE_KNOCKBACK`, `ITBUMPER_CASTLE_ANGLE`.
pub const CASTLE_KNOCKBACK: i32 = 300;
pub const CASTLE_ANGLE: i32 = 361;
/// `ITBUMPER_HIT_SCALE`, `ITBUMPER_HIT_ANIM_LENGTH`.
pub const HIT_SCALE: u16 = 10;
pub const HIT_ANIM_LENGTH: u16 = 3;

/// `llITCommonDataGBumperItemAttributes` (offset 0xCF0 of relocData file
/// 251, US): the NBumper model, light, a 150-unit damage cube it never uses
/// (hit status none), map box 180 either side, size 250, 1 damage at the
/// bumper angle 362 with 50 scale and 200 set knockback, a throw item that
/// rehits fighters and can be shielded.
pub static ATTRIBUTES: ItemAttributes = ItemAttributes {
    is_give_hitlag: true,
    weight: ItemWeight::Light,
    attack_offsets: [Vec3::ZERO, Vec3::ZERO],
    damage_coll_offset: Vec3::ZERO,
    damage_coll_size: Vec3::new(150.0, 150.0, 150.0),
    map_coll: BodyColl {
        top: 180.0,
        center: 0.0,
        bottom: -180.0,
        width: 180.0,
    },
    size: 250.0,
    angle: 362,
    kb_scale: 50,
    damage: 1,
    element: Element::Normal,
    kb_weight: 200,
    shield_damage: 0,
    attack_count: 1,
    can_setoff: false,
    priority: 1,
    can_rehit_item: false,
    can_rehit_fighter: true,
    can_hop: false,
    can_reflect: false,
    can_shield: true,
    kb_base: 0,
    ty: ItemType::Throw,
    hitstatus: HitStatus::None,
    vel_scale: 100,
};

/// The Bumper has no statuses: its descriptor's procs run throughout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Common,
}

/// `itGBumperMakeItem` (`ITEM_FLAG_PARENT_GROUND`: no map projection). The
/// pool only makes it for Peach's Castle, so the Castle knockback always
/// applies.
pub(super) fn make(pos: Vec3, motion_count: u16) -> Item {
    let mut item = Item::new(
        ItemKind::GBumper,
        &ATTRIBUTES,
        ItemStatus::GBumper(Status::Common),
        AttackState::New,
        pos,
        Vec3::ZERO,
        motion_count,
    );
    item.update_attack_positions();
    item.clear_owner_stats();
    item.multi = 0;
    item.attack.interact_mask = super::INTERACT_FIGHTER;
    item.attack.can_rehit_shield = true;
    item.vel_air = Vec3::ZERO;
    item.palette = 0;
    item.attack.kb_weight = CASTLE_KNOCKBACK;
    item.attack.angle = CASTLE_ANGLE;
    item
}

/// `itGBumperCommonProcUpdate`: the lit palette lasts its three frames,
/// and the swell shrinks back over ten.
pub(super) fn proc_update(item: &mut Item) -> bool {
    if item.vars.bumper_hit_anim_length == 0 && item.palette == 1 {
        item.palette = 0;
    } else {
        item.vars.bumper_hit_anim_length = item.vars.bumper_hit_anim_length.wrapping_sub(1);
    }
    let s = if item.multi != 0 {
        let s = 2.0 - (10 - i32::from(item.multi)) as f32 * 0.1;
        item.multi -= 1;
        s
    } else {
        1.0
    };
    item.scale.x = s;
    item.scale.y = s;
    true
}

/// `itGBumperCommonProcHit`.
pub(super) fn hit_proc(item: &mut Item, proc: HitProc) -> Option<bool> {
    match proc {
        HitProc::Hit => {
            item.scale.x = 2.0;
            item.scale.y = 2.0;
            item.vars.bumper_hit_anim_length = HIT_ANIM_LENGTH;
            item.palette = 1;
            item.multi = HIT_SCALE;
            Some(true)
        }
        _ => None,
    }
}
