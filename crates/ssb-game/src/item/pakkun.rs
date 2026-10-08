//! Mushroom Kingdom's Piranha Plants, `itpakkun.c`. Each waits in its pipe,
//! rises on its appear animation unless a fighter stands over the pipe,
//! bites with an attack that only turns on once it clears the rim, and
//! sinks back. A hit strong enough knocks it out of the pipe; it flies
//! upside down until it leaves the map, then waits to grow back
//! (`itPakkunDamagedProcDead`). A fighter entering the pipe sends it back
//! down (`grInishiePakkunSetWaitFighter`).

use ssb_engine::math::Vec3;

use super::{
    HitProc, Item, ItemAnim, ItemAnims, ItemAttributes, ItemKind, ItemStatus, ItemType, ItemWeight,
};
use crate::combat::{AttackState, Element, HitStatus};
use crate::fighter::Fighter;
use crate::ground::BodyColl;

/// `ITPAKKUN_*`.
pub const APPEAR_WAIT: u16 = 180;
pub const REBIRTH_WAIT: u16 = 1200;
pub const APPEAR_OFF_Y: f32 = 245.0;
pub const CLAMP_OFF_Y: f32 = 360.0;
pub const HURT_SIZE_MUL_Y: f32 = 0.5;
pub const DETECT_SIZE_WIDTH: f32 = 600.0;
pub const DETECT_SIZE_BOTTOM: f32 = -300.0;
pub const DETECT_SIZE_TOP: f32 = 700.0;
pub const NDAMAGE_KNOCKBACK_MIN: f32 = 100.0;
pub const GRAVITY: f32 = 1.5;
pub const TVEL: f32 = 100.0;
/// `F_CST_DTOR32(180.0F)`.
const DEG_180: f32 = core::f32::consts::PI;

/// `llGRInishieMapPakkunItemAttributes` (offset 0x120 of relocData file
/// 260, US): light, a 450 x 490 x 450 damage box that starts out normal
/// (inside the pipe), map box 110 either side and 159 wide, size 200, 5
/// damage at 80 degrees with 80 scale and 80 base knockback, a fighter
/// item whose attack clanks and rehits both items and fighters.
pub static ATTRIBUTES: ItemAttributes = ItemAttributes {
    sounds: crate::item_sounds::item::PAKKUN,
    is_give_hitlag: true,
    is_display_colanim: false,
    weight: ItemWeight::Light,
    attack_offsets: [Vec3::ZERO, Vec3::ZERO],
    damage_coll_offset: Vec3::ZERO,
    damage_coll_size: Vec3::new(450.0, 490.0, 450.0),
    map_coll: BodyColl {
        top: 110.0,
        center: 0.0,
        bottom: -110.0,
        width: 159.0,
    },
    size: 200.0,
    angle: 80,
    kb_scale: 80,
    damage: 5,
    element: Element::Normal,
    kb_weight: 0,
    shield_damage: 0,
    attack_count: 1,
    can_setoff: true,
    priority: 1,
    can_rehit_item: true,
    can_rehit_fighter: true,
    can_hop: false,
    can_reflect: false,
    can_shield: true,
    kb_base: 80,
    ty: ItemType::Fighter,
    hitstatus: HitStatus::Normal,
    vel_scale: 100,
};

/// `itPakkunStatus`. The descriptor's procs are the Wait ones, so a new
/// plant starts in `Wait`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Wait,
    Appear,
    Damaged,
}

/// `itPakkunMakeItem` (`ITEM_FLAG_PARENT_GROUND`). `index` is the plant's
/// `pakkun_gobj` slot, which names its tree in the runtime.
pub(super) fn make(index: u8, pos: Vec3, motion_count: u16) -> Item {
    let mut item = Item::new(
        ItemKind::Pakkun,
        &ATTRIBUTES,
        ItemStatus::Pakkun(Status::Wait),
        AttackState::Off,
        pos,
        Vec3::ZERO,
        motion_count,
    );
    item.update_attack_positions();
    item.vars.pakkun_index = index;
    item.vars.pakkun_pos = pos;
    item.pos = pos;
    item.multi = APPEAR_WAIT;
    item.is_allow_knockback = true;
    item.vars.pakkun_is_wait_fighter = false;
    item.attack.can_rehit_shield = true;
    item
}

/// `itPakkunWaitSetStatus`.
fn wait_set_status(item: &mut Item) {
    item.set_status(ItemStatus::Pakkun(Status::Wait));
}

/// `itPakkunCommonSetWaitFighter`.
pub(super) fn set_wait_fighter(item: &mut Item) {
    item.vars.pakkun_is_wait_fighter = true;
}

/// `itPakkunCommonCheckNoFighter`: no fighter's `TopN` stands within the
/// pipe's detection box.
fn check_no_fighter(item: &Item, fighters: &[Vec3]) -> bool {
    let (x, y) = (item.vars.pakkun_pos.x, item.vars.pakkun_pos.y);
    !fighters.iter().any(|p| {
        let dist_x = if p.x < x { -(p.x - x) } else { p.x - x };
        dist_x < DETECT_SIZE_WIDTH && p.y > y + DETECT_SIZE_BOTTOM && p.y < y + DETECT_SIZE_TOP
    })
}

/// `itPakkunWaitInitVars`.
fn wait_init_vars(item: &mut Item) {
    item.multi = APPEAR_WAIT;
    wait_set_status(item);
    item.damage_coll.hitstatus = HitStatus::None;
    item.attack.state = AttackState::Off;
    item.pos.y = item.vars.pakkun_pos.y;
}

/// `itPakkunAppearUpdateDamageColl`: the box grows with the part of the
/// plant above the rim, and the attack turns on as it clears it.
fn appear_update_damage_coll(item: &mut Item) {
    let pos_y = item.pos.y - item.vars.pakkun_pos.y;
    let off_y = pos_y + APPEAR_OFF_Y;
    if off_y <= CLAMP_OFF_Y {
        item.damage_coll.hitstatus = HitStatus::None;
        item.attack.state = AttackState::Off;
    } else {
        if item.damage_coll.hitstatus == HitStatus::None {
            item.damage_coll.hitstatus = HitStatus::Normal;
            item.refresh_attack_coll();
        }
        item.damage_coll.size.y = (off_y - CLAMP_OFF_Y) * HURT_SIZE_MUL_Y;
        item.damage_coll.offset.y = (item.damage_coll.size.y + CLAMP_OFF_Y) - pos_y;
    }
}

/// `proc_update`. A plant never removes itself. `fighters` holds every
/// fighter's `TopN` translation in link order.
pub(super) fn proc_update(
    item: &mut Item,
    status: Status,
    fighters: &[Vec3],
    anims: &mut dyn ItemAnims,
) -> bool {
    match status {
        // `itPakkunWaitProcUpdate`.
        Status::Wait => {
            if item.vars.pakkun_is_wait_fighter {
                item.multi = APPEAR_WAIT;
                item.vars.pakkun_is_wait_fighter = false;
            }
            item.multi = item.multi.wrapping_sub(1);
            if item.multi == 0 {
                if check_no_fighter(item, fighters) {
                    let write = anims.add_play(item.anim_target(), ItemAnim::PakkunAppear);
                    item.apply_root_write(write);
                    item.pos.y += item.vars.pakkun_pos.y;
                    item.set_status(ItemStatus::Pakkun(Status::Appear));
                } else {
                    item.multi = APPEAR_WAIT;
                }
            }
        }
        // `itPakkunAppearProcUpdate`.
        Status::Appear => {
            if item.vars.pakkun_is_wait_fighter {
                anims.stop_root(item.anim_target());
                wait_init_vars(item);
                item.vars.pakkun_is_wait_fighter = false;
            }
            if anims.root_idle(item.anim_target()) {
                wait_init_vars(item);
            } else {
                item.pos.y += item.vars.pakkun_pos.y;
            }
            appear_update_damage_coll(item);
        }
        // `itPakkunDamagedProcUpdate`.
        Status::Damaged => item.apply_gravity_clamp_tvel(GRAVITY, TVEL),
    }
    true
}

/// `itPakkunAppearProcDamage`: a hit with at least 100 knockback sends the
/// plant flying upside down.
pub(super) fn hit_proc(
    item: &mut Item,
    status: Status,
    proc: HitProc,
    anims: &mut dyn ItemAnims,
) -> Option<bool> {
    match (status, proc) {
        (Status::Appear, HitProc::Damage) => {
            if item.damage_knockback >= NDAMAGE_KNOCKBACK_MIN {
                item.rotate_z = DEG_180;
                let angle = crate::attack::sakurai_angle_radians(
                    item.damage_angle,
                    item.ga == super::Ga::Air,
                    item.damage_knockback,
                );
                let (sin, cos) = ssb_engine::math::sin_cos(angle);
                item.vel_air.x = cos * item.damage_knockback * -item.damage_lr;
                item.vel_air.y = sin * item.damage_knockback;
                item.damage_coll.hitstatus = HitStatus::None;
                item.attack.state = AttackState::Off;
                item.set_status(ItemStatus::Pakkun(Status::Damaged));
                anims.stop_root(item.anim_target());
                anims.add_play(item.anim_target(), ItemAnim::PakkunDamaged);
            }
            Some(true)
        }
        _ => None,
    }
}

/// `itPakkunDamagedProcDead`: back to the pipe, to grow again after
/// [`REBIRTH_WAIT`].
pub(super) fn proc_dead(item: &mut Item, anims: &mut dyn ItemAnims) {
    item.pos = item.vars.pakkun_pos;
    item.multi = REBIRTH_WAIT;
    item.vel_air = Vec3::ZERO;
    item.rotate_z = 0.0;
    anims.stop_material(item.anim_target());
    wait_set_status(item);
    item.vars.pakkun_is_wait_fighter = false;
}

/// `TopN` of every fighter, as [`check_no_fighter`] reads it.
pub fn fighter_top(f: &Fighter) -> Vec3 {
    f.pos
}
