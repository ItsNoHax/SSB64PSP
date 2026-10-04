//! The Poké Ball, `itmball.c`. Thrown or dropped, it opens where it lands
//! and, 30 frames later, releases a Pokémon (`itMainMakeMonster`) and is
//! gone. A ball that bounces off a target lands at its next floor contact;
//! otherwise its first landing may still despawn it. A reflected ball stays
//! its thrower's. Opening makes the rays (`efManagerMBallRaysMakeEffect`),
//! which follow the ball until the Pokémon comes out.

use ssb_engine::math::Vec3;

use super::normal::CommonItems;
use super::{map, HitProc, Item, ItemAttributes, ItemKind, ItemStatus, ItemType, ItemWeight};
use crate::combat::{AttackState, Element, HitStatus};
use crate::ground::BodyColl;
use crate::weapon::MapSurface;

/// `ITMBALL_*`.
pub const SPAWN_WAIT: u16 = 30;
pub const GRAVITY: f32 = 1.5;
pub const TVEL: f32 = 120.0;
pub const MAP_REBOUND_COMMON: f32 = 0.2;
pub const MAP_REBOUND_GROUND: f32 = 0.2;

/// File 251, `llITCommonDataMBallItemAttributes` (0x6E4).
pub static ATTRIBUTES: ItemAttributes = ItemAttributes {
    is_give_hitlag: true,
    is_display_colanim: false,
    weight: ItemWeight::Light,
    attack_offsets: [Vec3::ZERO; 2],
    damage_coll_offset: Vec3::ZERO,
    damage_coll_size: Vec3::new(150.0, 150.0, 150.0),
    map_coll: BodyColl {
        top: 109.0,
        center: 0.0,
        bottom: -109.0,
        width: 164.0,
    },
    size: 200.0,
    angle: 361,
    kb_scale: 80,
    damage: 12,
    element: Element::Normal,
    kb_weight: 0,
    shield_damage: 0,
    attack_count: 1,
    can_setoff: true,
    priority: 1,
    can_rehit_item: false,
    can_rehit_fighter: false,
    can_hop: true,
    can_reflect: true,
    can_shield: true,
    kb_base: 60,
    ty: ItemType::Throw,
    hitstatus: HitStatus::None,
    vel_scale: 80,
};

/// `itMBallStatus`, with the descriptor's procs as `Init`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Init,
    Wait,
    Fall,
    Hold,
    Thrown,
    Dropped,
    Open,
    OpenAir,
}

/// `itMBallMakeItem`.
pub(super) fn make(pos: Vec3, vel: Vec3) -> Item {
    let mut item = Item::new(
        ItemKind::MBall,
        &ATTRIBUTES,
        ItemStatus::MBall(Status::Init),
        AttackState::Off,
        pos,
        vel,
        0,
    );
    item.multi = SPAWN_WAIT;
    item.vars.mball_is_rebound = false;
    item.rotate_z = 0.0;
    item
}

fn set(item: &mut Item, s: Status) {
    item.set_status(ItemStatus::MBall(s));
}

/// `itMBallWaitSetStatus`.
fn wait(item: &mut Item) {
    item.set_ground_allow_pickup();
    set(item, Status::Wait);
}

/// `itMBallFallSetStatus`.
fn fall(item: &mut Item) {
    item.is_allow_pickup = false;
    map::set_air(item);
    set(item, Status::Fall);
}

/// `itMBallHoldSetStatus`: the holder stays the ball's owner through a
/// reflection.
pub(super) fn hold(item: &mut Item, team: u8, handicap: u8) {
    item.vars.mball_owner = item.owner.map(|port| (port, team, handicap));
    set(item, Status::Hold);
}

/// `itMBallThrownSetStatus` / `itMBallDroppedSetStatus`.
/// `itMBallOpenAddAnim` adds `llITCommonDataMBallMatAnimJoint` to the
/// closed ball's `MObj` (`dobj->child->child->sib_next`, the attach joint
/// still above the root) and plays the tree: display only.
pub(super) fn thrown(item: &mut Item) {
    item.add_root_script();
    set(item, Status::Thrown);
}
pub(super) fn dropped(item: &mut Item) {
    item.add_root_script();
    set(item, Status::Dropped);
}

/// `itMBallOpenSetStatus`: the open halves show and the closed ball hides.
/// `itMBallOpenClearAnim` clears the same `MObj`'s script, now
/// `dobj->child->sib_next` with the ball released.
fn open(item: &mut Item, common: &mut dyn CommonItems) {
    item.vel_air = Vec3::ZERO;
    item.vars.mball_open = !item.vars.mball_open;
    item.attach_line = item.floor_line();
    item.vars.mball_rays = common.mball_rays(item.pos);
    item.clear_root_script();
    item.attack.state = AttackState::Off;
    item.attack.can_reflect = false;
    set(item, Status::Open);
}

/// What the ball's update asks of the pool: `itMainMakeMonster` from its
/// position, then its own end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Update {
    Live,
    MakeMonster,
}

pub(super) fn update(item: &mut Item, status: Status, common: &mut dyn CommonItems) -> Update {
    match status {
        Status::Init | Status::Fall | Status::Thrown | Status::Dropped => {
            item.apply_gravity_clamp_tvel(GRAVITY, TVEL);
            item.rotate_z += item.spin_step;
        }
        Status::Wait | Status::Hold => {}
        // `itMBallOpenProcUpdate` / `itMBallOpenAirProcUpdate`. The rays
        // follow the ball.
        Status::Open | Status::OpenAir => {
            if item.multi == 0 {
                return Update::MakeMonster;
            }
            item.multi -= 1;
            if let Some(seq) = item.vars.mball_rays {
                common.move_display(seq, item.pos);
            }
        }
    }
    Update::Live
}

pub(super) fn has_proc_map(status: Status) -> bool {
    status != Status::Hold
}

pub(super) fn proc_map<I, F>(
    item: &mut Item,
    status: Status,
    surfaces: &F,
    common: &mut dyn CommonItems,
) -> bool
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    match status {
        Status::Wait => {
            if !map::check_lr_wall_proc_no_floor(item, surfaces) {
                fall(item);
            }
        }
        // `itMBallFallProcMap` keeps the source's ignored despawn.
        Status::Init | Status::Fall => {
            let out =
                map::check_destroy_dropped(item, MAP_REBOUND_COMMON, MAP_REBOUND_GROUND, surfaces);
            if out.goto_wait {
                wait(item);
            }
        }
        // `itMBallThrownProcMap`: the first landing opens it unless the
        // landing despawns it, which the source ignores; a second landing
        // opens it too.
        Status::Thrown | Status::Dropped => {
            if item.vars.mball_is_rebound {
                if map::check_landing(item, MAP_REBOUND_COMMON, MAP_REBOUND_GROUND, surfaces) {
                    open(item, common);
                }
            } else {
                let out = map::check_destroy_dropped(
                    item,
                    MAP_REBOUND_COMMON,
                    MAP_REBOUND_GROUND,
                    surfaces,
                );
                if out.goto_wait {
                    open(item, common);
                }
            }
        }
        Status::Open => {
            if !item
                .attach_line
                .is_some_and(|line| crate::map::line_exists(surfaces, line))
            {
                item.attach_line = None;
                set(item, Status::OpenAir);
            }
        }
        Status::OpenAir => {
            let out =
                map::check_destroy_dropped(item, MAP_REBOUND_COMMON, MAP_REBOUND_GROUND, surfaces);
            if out.goto_wait {
                open(item, common);
            }
        }
        Status::Hold => {}
    }
    true
}

/// `itMBallCommonProcHit`.
fn rebound(item: &mut Item) {
    item.attack.state = AttackState::Off;
    item.vars.mball_is_rebound = true;
    item.vel_set_rebound();
}

/// `itMBallCommonProcReflector`: the thrower takes the ball back.
fn reflector(item: &mut Item) {
    rebound(item);
    if let Some((port, team, handicap)) = item.vars.mball_owner {
        item.owner = Some(port);
        item.team = team;
        item.player = Some(port);
        item.handicap = handicap;
    }
}

pub(super) fn hit_proc(item: &mut Item, status: Status, proc: HitProc) -> Option<bool> {
    let thrown = matches!(status, Status::Thrown | Status::Dropped);
    match proc {
        HitProc::Hit | HitProc::Shield | HitProc::SetOff if thrown => rebound(item),
        HitProc::Hop if thrown => item.common_proc_hop(),
        HitProc::Reflector if thrown || status == Status::OpenAir => reflector(item),
        _ => return None,
    }
    Some(true)
}
