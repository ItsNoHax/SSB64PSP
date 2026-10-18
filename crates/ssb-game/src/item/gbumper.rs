//! The ground Bumper, `itgbumper.c`. `grCastleInitAll` makes Peach's
//! Castle's on the moving ground and `grCastleBumperProcUpdate` carries its
//! X every frame; `grBonus3MakeBumpers` lines Race to the Finish with them.
//! It hits fighters only, never takes damage, and swells and lights up for
//! a few frames each time it hits.

use ssb_engine::math::Vec3;

use super::{HitProc, Item, ItemAttributes, ItemKind, ItemStatus};
use crate::combat::AttackState;

/// `ITBUMPER_CASTLE_KNOCKBACK`, `ITBUMPER_CASTLE_ANGLE`.
pub const CASTLE_KNOCKBACK: i32 = 300;
pub const CASTLE_ANGLE: i32 = 361;
/// `ITBUMPER_HIT_SCALE`, `ITBUMPER_HIT_ANIM_LENGTH`.
pub const HIT_SCALE: u16 = 10;
pub const HIT_ANIM_LENGTH: u16 = 3;

/// `llITCommonDataGBumperItemAttributes` (file 251, 0xCF0).
pub static ATTRIBUTES: ItemAttributes = super::attributes::GBUMPER;

/// The Bumper has no statuses: its descriptor's procs run throughout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Common,
}

/// `itGBumperMakeItem` (`ITEM_FLAG_PARENT_GROUND`: no map projection).
/// `castle` is `gSCManagerBattleState->gkind == nGRKindCastle`: only Peach's
/// Castle's Bumper takes the Castle knockback; Race to the Finish's keep
/// the attributes'.
pub(super) fn make(pos: Vec3, motion_count: u16, castle: bool) -> Item {
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
    if castle {
        item.attack.kb_weight = CASTLE_KNOCKBACK;
        item.attack.angle = CASTLE_ANGLE;
    }
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
