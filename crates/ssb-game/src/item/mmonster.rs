//! The Poké Ball's Pokémon (`itmonster/`): Onix, Snorlax, Goldeen, Meowth,
//! Charizard, Beedrill, Blastoise, Chansey, Starmie, Hitmonlee, Koffing,
//! Clefairy and Mew (RE-435).
//!
//! `itMainMakeMonster` makes one from the open ball. Each rises at 16 for
//! 22 frames (`ITMONSTER_RISE_*`) under its descriptor's procs
//! ([`Status::Common`]), then takes its own statuses. Clefairy draws one of
//! the twelve others' first statuses and runs it as itself, so every
//! "this kind" test of those statuses fails for it (the source's
//! `ip->kind == nITKindX`): it has no voice, no status animation, no
//! display-list swap, and spawns from its own offsets. Its twelfth choice is
//! Mew's flight, kept as Clefairy.
//!
//! The weapons they make are [`crate::monster_weapon`]'s. Sounds are not
//! ported; the status animations are presentation ([`Vars::status_anim`]).

use ssb_engine::math::{atan2, sin_cos, Vec3};

use super::{map, HitProc, Item, ItemAttributes, ItemKind, ItemStatus, ItemType, ItemWeight};
use super::{normal::CommonItems, OwnerView};
use crate::combat::{AttackState, Element, HitStatus};
use crate::ground::BodyColl;
use crate::monster_weapon::{MonsterShot, ShotParent};
use crate::status::BlastZone;
use crate::weapon::MapSurface;
use crate::wpeffect::{Emit, WeaponEffect as Fx};

/// `nITKindMBallMonsterStart` order (`ITKind` 32 to 44).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Kind {
    Iwark,
    Kabigon,
    Tosakinto,
    Nyars,
    Lizardon,
    Spear,
    Kamex,
    MLucky,
    Starmie,
    Sawamura,
    Dogas,
    Pippi,
    Mew,
}

impl Kind {
    pub const ALL: [Kind; 13] = [
        Kind::Iwark,
        Kind::Kabigon,
        Kind::Tosakinto,
        Kind::Nyars,
        Kind::Lizardon,
        Kind::Spear,
        Kind::Kamex,
        Kind::MLucky,
        Kind::Starmie,
        Kind::Sawamura,
        Kind::Dogas,
        Kind::Pippi,
        Kind::Mew,
    ];

    /// From an `ITKind`.
    pub fn from_item_kind(kind: u8) -> Option<Self> {
        Self::ALL
            .get(usize::from(kind.checked_sub(super::MBALL_MONSTER_START)?))
            .copied()
    }

    pub fn item_kind(self) -> u8 {
        super::MBALL_MONSTER_START + self as u8
    }
}

/// Each kind's statuses, with the descriptor's procs as `Common`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Common,
    IwarkFly,
    IwarkAttack,
    KabigonJump,
    KabigonFall,
    TosakintoAppear,
    TosakintoBounce,
    MewFly,
    NyarsAttack,
    LizardonFall,
    LizardonAttack,
    SpearAppear,
    SpearFly,
    KamexFall,
    KamexAppear,
    KamexAttack,
    MLuckyFall,
    MLuckyAppear,
    MLuckyMakeEgg,
    MLuckyDisappear,
    StarmieNFollow,
    StarmieAttack,
    SawamuraFall,
    SawamuraWait,
    SawamuraAttack,
    DogasAttack,
    DogasDisappear,
}

/// `ITMONSTER_RISE_STOP_WAIT` and `ITMONSTER_RISE_VEL_Y`.
pub const RISE_STOP_WAIT: u16 = 22;
pub const RISE_VEL_Y: f32 = 16.0;

/// `ITIWARK_*` and `WPIWARK_ROCK_RANDOM_VEL_MAX`.
const IWARK_FLY_WAIT: u16 = 30;
const IWARK_MODEL_ROTATE_WAIT: u16 = 6;
const IWARK_ROCK_RUMBLE_WAIT: u16 = 18;
const IWARK_ROCK_SPAWN_COUNT_RANDOM: i32 = 9;
const IWARK_ROCK_SPAWN_COUNT_MIN: u16 = 8;
const IWARK_ROCK_SPAWN_WAIT_MAX: i32 = 30;
const IWARK_ROCK_SPAWN_WAIT_MIN: i32 = 15;
const IWARK_FLY_VEL_Y: f32 = 80.0;
const IWARK_FLY_STOP_Y: f32 = 200.0;
const IWARK_ROCK_RANDOM_VEL_MAX: i32 = 3;
/// `ITKABIGON_*`.
const KABIGON_EFFECT_SPAWN_INT: i32 = 4;
const KABIGON_DROP_WAIT: u16 = 60;
const KABIGON_RUMBLE_WAIT: i32 = 18;
const KABIGON_DROP_VEL_Y: f32 = -220.0;
const KABIGON_DROP_SIZE_KABIGON: f32 = 4.0;
const KABIGON_DROP_SIZE_OTHER: f32 = 5.2;
const KABIGON_JUMP_VEL_Y: f32 = 80.0;
const KABIGON_MAP_OFF_Y: f32 = 200.0;
/// `ITTOSAKINTO_*`.
const TOSAKINTO_LIFETIME: u16 = 360;
const TOSAKINTO_FLAP_VEL_X: f32 = 10.0;
const TOSAKINTO_FLAP_VEL_Y: f32 = 60.0;
const TOSAKINTO_GRAVITY: f32 = 6.0;
const TOSAKINTO_TVEL: f32 = 90.0;
/// `ITMEW_*`.
const MEW_LIFETIME: u16 = 480;
const MEW_EFFECT_SPAWN_INT: i32 = 3;
const MEW_STARTVEL_X: f32 = 8.0;
const MEW_STARTVEL_Y: f32 = -20.0;
const MEW_FLY_ADD_VEL_Y: f32 = 0.8;
/// `ITNYARS_*`.
const NYARS_LIFETIME: u16 = 240;
const NYARS_MODEL_ROTATE_WAIT: u16 = 30;
const NYARS_COIN_SPAWN_MAX: u8 = 4;
const NYARS_COIN_SPAWN_WAIT: u16 = 8;
const NYARS_COIN_ANGLE_STEP: f32 = 13.0;
/// `ITLIZARDON_*`.
const LIZARDON_LIFETIME: u16 = 480;
const LIZARDON_FLAME_SPAWN_WAIT: u16 = 8;
const LIZARDON_TURN_WAIT: u16 = 26;
const LIZARDON_FLAME_ANGLE: f32 = -0.261_799_4;
const LIZARDON_FLAME_VEL: f32 = 50.0;
const LIZARDON_LIZARDON_FLAME_OFF_X: f32 = 180.0;
const LIZARDON_LIZARDON_FLAME_OFF_Y: f32 = 120.0;
const LIZARDON_OTHER_FLAME_OFF_X: f32 = 100.0;
const LIZARDON_DUST_OFF_X: f32 = -400.0;
const LIZARDON_GRAVITY: f32 = 1.0;
const LIZARDON_TVEL: f32 = 40.0;
/// `ITSPEAR_*`.
const SPEAR_SPAWN_COUNT: u16 = 16;
const SPEAR_SPAWN_WAIT_CONST: i32 = 12;
const SPEAR_SPAWN_WAIT_RANDOM: i32 = 9;
const SPEAR_SPAWN_OFF_Y_MUL: f32 = 1800.0;
const SPEAR_SPAWN_OFF_Y_ADD: f32 = -800.0;
const SPEAR_SWARM_CALL_VEL_X: f32 = 6.0;
const SPEAR_SWARM_CALL_VEL_Y: f32 = 60.0;
const SPEAR_SWARM_CALL_OFF_X: f32 = 500.0;
/// `ITSPEAR_SWARM_CALL_WAIT`: the frame of its appear animation on which
/// Beedrill flies off. The script reaches it on its 51st play and ends on
/// the next (a ROM test replays it).
pub const SPEAR_SWARM_CALL_WAIT: u16 = 51;
const SPEAR_GRAVITY: f32 = 1.0;
const SPEAR_TVEL: f32 = 90.0;
/// `ITKAMEX_*`.
const KAMEX_LIFETIME: u16 = 360;
const KAMEX_HYDRO_SPAWN_WAIT_CONST: i32 = 30;
const KAMEX_HYDRO_SPAWN_WAIT_RANDOM: i32 = 1;
const KAMEX_KAMEX_HYDRO_SPAWN_OFF_X: f32 = 360.0;
const KAMEX_KAMEX_HYDRO_SPAWN_OFF_Y: f32 = 100.0;
const KAMEX_OTHER_HYDRO_SPAWN_OFF_X: f32 = 100.0;
const KAMEX_DUST_SPAWN_OFF_X: f32 = -150.0;
const KAMEX_COLL_SIZE: f32 = 341.0;
const KAMEX_PUSH_VEL_X: f32 = 2.3;
const KAMEX_CONSTVEL_X: f32 = 38.0;
const KAMEX_GRAVITY: f32 = 1.0;
const KAMEX_TVEL: f32 = 40.0;
/// `ITMLUCKY_*`.
const MLUCKY_LIFETIME: u16 = 90;
const MLUCKY_EGG_SPAWN_WAIT_ADD: u16 = 4;
const MLUCKY_EGG_SPAWN_COUNT: u16 = 3;
const MLUCKY_EGG_SPAWN_WAIT_CONST: u16 = 30;
const MLUCKY_EGG_SPAWN_BASE_VEL: f32 = 8.0;
const MLUCKY_EGG_SPAWN_ADD_VEL_X: f32 = 7.0;
const MLUCKY_EGG_SPAWN_ADD_VEL_Y: f32 = 40.0;
const MLUCKY_GRAVITY: f32 = 1.0;
const MLUCKY_TVEL: f32 = 40.0;
/// `ITSTARMIE_*`.
const STARMIE_LIFETIME: u16 = 240;
const STARMIE_SWIFT_SPAWN_WAIT_CONST: i32 = 12;
const STARMIE_SWIFT_SPAWN_WAIT_RANDOM: i32 = 1;
const STARMIE_STARMIE_SWIFT_SPAWN_OFF_X: f32 = 200.0;
const STARMIE_STARMIE_SWIFT_SPAWN_OFF_Y: f32 = 100.0;
const STARMIE_OTHER_SWIFT_SPAWN_OFF_X: f32 = 100.0;
const STARMIE_TARGET_POS_OFF_X: f32 = 400.0;
const STARMIE_TARGET_POS_OFF_Y: f32 = 250.0;
const STARMIE_FOLLOW_VEL_X: f32 = 20.0;
const STARMIE_ADD_VEL_X: f32 = 10.0;
const STARMIE_PUSH_VEL_X: f32 = 70.0;
/// `ITSAWAMURA_*`.
const SAWAMURA_LIFETIME: u16 = 600;
const SAWAMURA_KICK_WAIT: u16 = 40;
const SAWAMURA_TARGET_POS_OFF_Y: f32 = 500.0;
const SAWAMURA_DESPAWN_OFF_X: f32 = 500.0;
const SAWAMURA_KICK_SIZE: f32 = 300.0;
const SAWAMURA_KICK_VEL_X: f32 = 400.0;
const SAWAMURA_GRAVITY: f32 = 2.4;
const SAWAMURA_TVEL: f32 = 100.0;
/// `ITDOGAS_*`.
const DOGAS_DESPAWN_WAIT: u16 = 90;
const DOGAS_SMOG_SPAWN_WAIT: i32 = 8;
const DOGAS_SMOG_SPAWN_COUNT: u16 = 32;
const DOGAS_SMOG_VEL: f32 = 18.0;
const DOGAS_SMOG_MUL_OFF_X: f32 = 400.0;
const DOGAS_SMOG_SUB_OFF_X: f32 = 200.0;
const DOGAS_SMOG_MUL_OFF_Y: f32 = 800.0;
const DOGAS_SMOG_SUB_OFF_Y: f32 = 400.0;

const PI: f32 = core::f32::consts::PI;

const fn coll(top: f32, bottom: f32, width: f32) -> BodyColl {
    BodyColl {
        top,
        center: 0.0,
        bottom,
        width,
    }
}

/// The fields every Pokémon shares: no hurtbox, a light `Fighter` item that
/// gives hitlag, a shieldable attack of priority 1 that rehits items.
const BASE: ItemAttributes = ItemAttributes {
    sounds: crate::item_sounds::ItemSounds::NONE,
    is_give_hitlag: true,
    is_display_colanim: false,
    weight: ItemWeight::Light,
    attack_offsets: [Vec3::ZERO; 2],
    damage_coll_offset: Vec3::ZERO,
    damage_coll_size: Vec3::new(150.0, 150.0, 150.0),
    map_coll: coll(0.0, 0.0, 0.0),
    size: 300.0,
    angle: 361,
    kb_scale: 80,
    damage: 12,
    element: Element::Normal,
    kb_weight: 0,
    shield_damage: 0,
    attack_count: 1,
    can_setoff: false,
    priority: 1,
    can_rehit_item: true,
    can_rehit_fighter: true,
    can_hop: false,
    can_reflect: false,
    can_shield: true,
    kb_base: 60,
    ty: ItemType::Fighter,
    hitstatus: HitStatus::None,
    vel_scale: 100,
};

/// File 251's `ITAttributes`: 0x72C, 0x7A8, 0x7F0, 0x880, 0x8FC, 0x98C,
/// 0xA08, 0xA84, 0xB34, 0xBB0, 0xBF8, 0xC74 and 0x838
/// (`reloc_data.us.h`).
pub static ATTRIBUTES: [ItemAttributes; 13] = [
    ItemAttributes {
        sounds: crate::item_sounds::item::WARK,
        map_coll: coll(324.0, -324.0, 459.0),
        size: 600.0,
        angle: 90,
        kb_scale: 100,
        damage: 21,
        can_rehit_fighter: false,
        ..BASE
    },
    ItemAttributes {
        sounds: crate::item_sounds::item::KABIGON,
        damage_coll_size: Vec3::new(580.0, 580.0, 780.0),
        map_coll: coll(585.0, -585.0, 780.0),
        size: 800.0,
        kb_scale: 70,
        damage: 22,
        can_rehit_fighter: false,
        ..BASE
    },
    ItemAttributes {
        sounds: crate::item_sounds::item::TOSAKINTO,
        map_coll: coll(135.0, -135.0, 195.0),
        size: 200.0,
        can_setoff: true,
        ..BASE
    },
    ItemAttributes {
        sounds: crate::item_sounds::item::NYARS,
        map_coll: coll(244.0, -244.0, 244.0),
        ..BASE
    },
    ItemAttributes {
        sounds: crate::item_sounds::item::LIZARDON,
        map_coll: coll(400.0, -390.0, 420.0),
        size: 600.0,
        damage: 18,
        ..BASE
    },
    ItemAttributes {
        sounds: crate::item_sounds::item::SPEAR,
        map_coll: coll(244.0, -244.0, 244.0),
        angle: 90,
        kb_scale: 100,
        damage: 18,
        can_rehit_fighter: false,
        ..BASE
    },
    ItemAttributes {
        sounds: crate::item_sounds::item::KAMEX,
        map_coll: coll(317.0, -300.0, 300.0),
        size: 400.0,
        angle: 130,
        kb_scale: 60,
        damage: 13,
        ..BASE
    },
    ItemAttributes {
        sounds: crate::item_sounds::item::MLUCKY,
        damage_coll_size: Vec3::new(250.0, 250.0, 250.0),
        map_coll: coll(300.0, -200.0, 240.0),
        size: 500.0,
        can_setoff: true,
        ..BASE
    },
    ItemAttributes {
        sounds: crate::item_sounds::item::STARMIE,
        map_coll: coll(300.0, -220.0, 300.0),
        size: 400.0,
        ..BASE
    },
    ItemAttributes {
        sounds: crate::item_sounds::item::SAWAMURA,
        map_coll: coll(317.0, -317.0, 195.0),
        angle: 90,
        kb_scale: 100,
        damage: 24,
        shield_damage: 30,
        can_rehit_fighter: false,
        kb_base: 20,
        ..BASE
    },
    ItemAttributes {
        sounds: crate::item_sounds::item::DOGAS,
        map_coll: coll(240.0, -240.0, 240.0),
        size: 50.0,
        ..BASE
    },
    ItemAttributes {
        sounds: crate::item_sounds::item::PIPPI,
        map_coll: coll(192.0, -192.0, 192.0),
        can_setoff: true,
        ..BASE
    },
    ItemAttributes {
        sounds: crate::item_sounds::item::MEW,
        map_coll: coll(218.0, -218.0, 218.0),
        can_setoff: true,
        ..BASE
    },
];

/// `ITStruct::item_vars` for the Pokémon (a union in the source; Clefairy
/// uses the part of whichever status it took), and what the runtime draws.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Vars {
    /// The root `DObj`'s `rotate.y` (Beedrill's: its child's). Presentation
    /// only.
    pub rotate_y: f32,
    /// The status added the kind's own animation (`gcAddDObjAnimJoint` /
    /// `gcAddMObjMatAnimJoint` then `gcPlayAnimAll`) at
    /// [`Item::anim_ticks`] `status_anim`. `None`: the rise's scale script
    /// (`llITCommonDataMonsterAnimBankStart`) from the make.
    pub status_anim: Option<u16>,
    /// The status animation stopped where it was (Beedrill's fly status
    /// clears its child's `anim_joint`).
    pub anim_frozen: bool,
    /// The attack display list replaced the model (Onix, Blastoise,
    /// Hitmonlee).
    pub attack_dl: bool,
    /// Moved to DL link 18. Starmie/Hitmonlee keep their normal display
    /// callback; Snorlax's fall and Clefairy's imitations use AA_XLU_SURF.
    pub xlu: bool,
    /// The visual clock when gcMoveGObjDLHead last moved this item.
    pub link18_at: u16,
    // `ITMonsterItemVarsIwark`.
    pub rock_spawn_remain: u16,
    pub rock_spawn_wait: i32,
    pub rock_spawn_max: u16,
    pub rumble_frame: u16,
    pub iwark_rumble_wait: u16,
    pub rock_spawn_count: u16,
    // `ITMonsterItemVarsKabigon`.
    pub dust_effect_int: i32,
    pub kabigon_rumble_wait: i32,
    // `ITMonsterItemVarsMew`.
    pub esper_gfx_int: i32,
    // `ITMonsterItemVarsNyars`.
    pub coin_spawn_wait: u16,
    pub coin_rotate_step: u16,
    pub model_rotate_wait: u16,
    // `ITMonsterItemVarsLizardon`.
    pub turn_wait: u16,
    pub flame_spawn_wait: u16,
    // `ITMonsterItemVarsSpear`, and its appear animation's frame
    // (`item_gobj->anim_frame`).
    pub spear_spawn_count: u16,
    pub spear_spawn_wait: i32,
    pub spear_spawn_pos_y: f32,
    pub spear_anim_frame: u16,
    // `ITMonsterItemVarsKamex`.
    pub hydro_spawn_wait: i32,
    pub hydro_push_vel_x: f32,
    pub is_apply_push: bool,
    // `ITMonsterItemVarsMLucky`.
    pub egg_spawn_wait: u16,
    pub lucky_lifetime: u16,
    // `ITMonsterItemVarsStarmie`.
    pub swift_spawn_wait: i32,
    pub target_pos: Vec3,
    pub victim_pos: Vec3,
    pub add_vel_x: f32,
    // `ITMonsterItemVarsDogas`.
    pub smog_spawn_wait: i32,
}

/// The weapon pool's side of a weapon maker: `wpManagerMakeWeapon` fails
/// when no struct is free, before the maker draws anything.
pub(crate) trait ShotSink {
    fn has_free(&self) -> bool;
    fn push(&mut self, shot: MonsterShot);
}

/// What a Pokémon's callbacks reach.
pub(super) struct Ctx<'a> {
    pub owners: &'a [Option<OwnerView>; 4],
    pub bounds: Option<BlastZone>,
    pub shots: &'a mut dyn ShotSink,
    pub fx: &'a mut Emit,
    pub common: &'a mut dyn CommonItems,
    /// This item's pool handle.
    pub handle: u32,
}

fn kind(item: &Item) -> Kind {
    let ItemKind::MMonster(k) = item.kind else {
        unreachable!()
    };
    k
}

fn set(item: &mut Item, s: Status) {
    item.set_status(ItemStatus::MMonster(s));
}

fn v(item: &mut Item) -> &mut Vars {
    &mut item.vars.mmonster
}

/// The status animation starts now (the init's `gcPlayAnimAll`).
fn start_anim(item: &mut Item) {
    item.vars.mmonster.status_anim = Some(item.anim_ticks);
}

fn lr_sign(x: f32) -> f32 {
    if x < 0.0 {
        -1.0
    } else {
        1.0
    }
}

/// `itManagerMakeItem` for a Pokémon and its maker, from the open ball
/// `parent` (`ITEM_FLAG_COLLPROJECT | ITEM_FLAG_PARENT_ITEM`). Blastoise's
/// maker takes the ball's owner and team and faces its nearest opponent
/// (`itKamexCommonFindTargetsSetLR`) from where it is made.
pub(super) fn make<I, F>(
    k: Kind,
    parent: &Item,
    vel: Vec3,
    owners: &[Option<OwnerView>; 4],
    surfaces: &F,
) -> Item
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    let attack = match k {
        Kind::Tosakinto | Kind::MLucky | Kind::Dogas | Kind::Mew => AttackState::Off,
        _ => AttackState::New,
    };
    let pos = parent.pos;
    let mut item = Item::new(
        ItemKind::MMonster(k),
        &ATTRIBUTES[k as usize],
        ItemStatus::MMonster(Status::Common),
        attack,
        pos,
        vel,
        0,
    );
    map::run_default_collision(&mut item, parent.pos, parent.coll, surfaces);
    item.update_attack_positions();
    // The makers.
    if matches!(k, Kind::Iwark | Kind::Tosakinto | Kind::Spear) {
        item.clear_owner_stats();
    }
    if matches!(k, Kind::Iwark | Kind::Kabigon | Kind::Spear) {
        item.attack.interact_mask = super::INTERACT_FIGHTER;
    }
    // Every maker but Snorlax's and Mew's puts the root back at `pos`,
    // undoing the projection.
    if !matches!(k, Kind::Kabigon | Kind::Mew) {
        item.pos = pos;
    }
    item.multi = RISE_STOP_WAIT;
    item.vel_air = Vec3::new(0.0, RISE_VEL_Y, 0.0);
    if k == Kind::Spear {
        if crate::rng::rand_int_range(2) == 0 {
            v(&mut item).rotate_y = PI;
            item.lr = -1.0;
        } else {
            item.lr = 1.0;
        }
    }
    if k == Kind::Kamex {
        item.owner = parent.owner;
        item.team = parent.team;
        if let Some(victim) = find_victim(&item, owners) {
            item.lr = lr_sign(victim.pos.x - item.pos.x);
        }
        if item.lr == -1.0 {
            v(&mut item).rotate_y = PI;
        }
    }
    if k != Kind::Kabigon {
        item.pos.y -= item.attr.map_coll.bottom;
    }
    if matches!(k, Kind::Starmie | Kind::Sawamura) {
        v(&mut item).xlu = true;
        item.vars.mmonster.link18_at = item.anim_ticks;
    }
    // The makers' appear voices (`it{Kabigon,Pippi,Sawamura}MakeItem`);
    // the caller has checked for a free item, so the make succeeds.
    match k {
        Kind::Kabigon => crate::sound::play_fgm(crate::sound::id::nSYAudioVoiceMBallKabigonAppear),
        Kind::Pippi => crate::sound::play_fgm(crate::sound::id::nSYAudioVoiceMBallPippiAppear),
        Kind::Sawamura => {
            crate::sound::play_fgm(crate::sound::id::nSYAudioVoiceMBallSawamuraAppear)
        }
        _ => None,
    };
    item
}

/// The fighter loop of `itKamexCommonFindTargetsSetLR`,
/// `itStarmieNFollowInitVars` and `itSawamuraAttackInitVars` (US): of
/// the fighters that are not the owner and not on the item's team, the
/// nearest in XY, the later one on a tie. With none, the source reads an
/// uninitialised `victim_gobj`; the port leaves the facing and target as
/// they were (RE-435).
fn find_victim(item: &Item, owners: &[Option<OwnerView>; 4]) -> Option<OwnerView> {
    let mut best = None;
    let mut dist_xy = 0.0;
    let mut players = 0;
    for (port, view) in owners.iter().enumerate() {
        let Some(view) = view else { continue };
        if item.owner == Some(port as u8) || view.team == item.team {
            continue;
        }
        let d = view.pos - item.pos;
        let square = d.x * d.x + d.y * d.y;
        if players == 0 {
            dist_xy = square;
        }
        players += 1;
        if square <= dist_xy {
            dist_xy = square;
            best = Some(*view);
        }
    }
    best
}

fn parent(item: &Item, handle: u32) -> ShotParent {
    ShotParent {
        stat: item.attack.stat,
        owner: item.owner,
        player: item.player,
        team: item.team,
        lr: item.lr,
        handle,
    }
}

/// `ip->proc_update`. Returns whether the item lives on.
pub(super) fn update(item: &mut Item, status: Status, ctx: &mut Ctx<'_>) -> bool {
    let k = kind(item);
    match status {
        Status::Common => common_update(item, k, ctx),
        Status::IwarkFly => {
            if item.multi == 0 {
                iwark_attack(item, k, ctx);
            }
            item.multi = item.multi.wrapping_sub(1);
            true
        }
        Status::IwarkAttack => iwark_attack_update(item, ctx),
        Status::KabigonJump => {
            let top = ctx.bounds.map_or(f32::MAX, |b| b.top);
            if item.pos.y >= top - KABIGON_MAP_OFF_Y {
                item.multi = item.multi.wrapping_sub(1);
                item.vel_air.y = 0.0;
                if item.multi == 0 {
                    kabigon_fall(item, k);
                }
            }
            if v(item).dust_effect_int == 0 {
                let mut pos = item.pos;
                pos.x += crate::rng::rand_float() * 200.0 - 100.0;
                pos.y += crate::rng::rand_float() * 200.0 - 100.0;
                ctx.fx.push(Fx::DustExpandLarge(pos));
                v(item).dust_effect_int = KABIGON_EFFECT_SPAWN_INT;
            }
            v(item).dust_effect_int -= 1;
            true
        }
        Status::KabigonFall => {
            if v(item).kabigon_rumble_wait == 0 {
                ctx.fx.push(Fx::Quake(0));
                v(item).kabigon_rumble_wait = KABIGON_RUMBLE_WAIT;
            }
            v(item).kabigon_rumble_wait -= 1;
            let bottom = ctx.bounds.map_or(f32::MIN, |b| b.bottom);
            item.pos.y >= bottom + KABIGON_MAP_OFF_Y
        }
        Status::TosakintoAppear => {
            item.apply_gravity_clamp_tvel(TOSAKINTO_GRAVITY, TOSAKINTO_TVEL);
            true
        }
        Status::TosakintoBounce => {
            item.apply_gravity_clamp_tvel(TOSAKINTO_GRAVITY, TOSAKINTO_TVEL);
            if item.multi == 0 {
                return false;
            }
            item.multi -= 1;
            true
        }
        Status::MewFly => {
            let pos = item.pos;
            if item.multi == 0 {
                return false;
            }
            if v(item).esper_gfx_int == 0 {
                v(item).esper_gfx_int = MEW_EFFECT_SPAWN_INT;
                ctx.fx.push(Fx::HealSparkles(pos));
            }
            v(item).esper_gfx_int -= 1;
            item.multi -= 1;
            item.vel_air.y += MEW_FLY_ADD_VEL_Y;
            true
        }
        Status::NyarsAttack => {
            if item.multi == 0 {
                return false;
            }
            if item.multi == v(item).coin_spawn_wait {
                let angle = f32::from(v(item).coin_rotate_step) * NYARS_COIN_ANGLE_STEP;
                let p = parent(item, ctx.handle);
                for n in 0..NYARS_COIN_SPAWN_MAX {
                    if ctx.shots.has_free() {
                        ctx.shots.push(MonsterShot::coin(p, item.pos, n, angle));
                    }
                }
                v(item).coin_rotate_step = v(item).coin_rotate_step.wrapping_add(1);
                v(item).coin_spawn_wait = item.multi.wrapping_sub(NYARS_COIN_SPAWN_WAIT);
                crate::sound::play_fgm(crate::sound::id::nSYAudioFGMNyarsCoin);
            }
            if v(item).model_rotate_wait == 0 {
                v(item).rotate_y += PI;
                v(item).model_rotate_wait = NYARS_MODEL_ROTATE_WAIT;
            }
            v(item).model_rotate_wait = v(item).model_rotate_wait.wrapping_sub(1);
            item.multi -= 1;
            true
        }
        Status::LizardonFall => {
            item.apply_gravity_clamp_tvel(LIZARDON_GRAVITY, LIZARDON_TVEL);
            true
        }
        Status::LizardonAttack => lizardon_attack_update(item, k, ctx),
        Status::SpearAppear => {
            // `item_gobj->anim_frame`: one play per update.
            v(item).spear_anim_frame = v(item).spear_anim_frame.wrapping_add(1);
            if v(item).spear_anim_frame == SPEAR_SWARM_CALL_WAIT {
                spear_fly(item, k);
            }
            true
        }
        Status::SpearFly => spear_fly_update(item, k, ctx),
        Status::KamexFall | Status::KamexAppear => {
            item.apply_gravity_clamp_tvel(KAMEX_GRAVITY, KAMEX_TVEL);
            true
        }
        Status::KamexAttack => {
            if item.multi == 0 {
                return false;
            }
            kamex_update_hydro(item, k, ctx);
            if v(item).is_apply_push {
                item.vel_air.x += v(item).hydro_push_vel_x;
            }
            v(item).hydro_spawn_wait -= 1;
            item.multi -= 1;
            true
        }
        Status::MLuckyFall | Status::MLuckyAppear => {
            item.apply_gravity_clamp_tvel(MLUCKY_GRAVITY, MLUCKY_TVEL);
            true
        }
        Status::MLuckyMakeEgg => {
            mlucky_make_egg_update(item, ctx);
            true
        }
        Status::MLuckyDisappear => {
            if v(item).lucky_lifetime == 0 {
                return false;
            }
            v(item).lucky_lifetime -= 1;
            true
        }
        Status::StarmieNFollow => {
            let tx = v(item).target_pos.x;
            if (item.lr == 1.0 && item.pos.x >= tx) || (item.lr == -1.0 && item.pos.x <= tx) {
                item.vel_air.x = 0.0;
                item.vel_air.y = 0.0;
                starmie_attack(item);
            }
            true
        }
        Status::StarmieAttack => {
            if item.multi == 0 {
                return false;
            }
            starmie_update_swift(item, k, ctx);
            v(item).swift_spawn_wait -= 1;
            item.vel_air.x += v(item).add_vel_x;
            item.multi -= 1;
            true
        }
        Status::SawamuraFall => {
            item.apply_gravity_clamp_tvel(SAWAMURA_GRAVITY, SAWAMURA_TVEL);
            true
        }
        Status::SawamuraWait => {
            if item.multi == 0 {
                sawamura_attack(item, k, ctx.owners);
            }
            item.multi = item.multi.wrapping_sub(1);
            true
        }
        Status::SawamuraAttack => {
            item.apply_gravity_clamp_tvel(SAWAMURA_GRAVITY, SAWAMURA_TVEL);
            if let Some(b) = ctx.bounds {
                if (item.lr == 1.0 && item.pos.x >= b.right - SAWAMURA_DESPAWN_OFF_X)
                    || (item.lr == -1.0 && item.pos.x <= b.left + SAWAMURA_DESPAWN_OFF_X)
                {
                    return false;
                }
            }
            if item.multi == 0 {
                return false;
            }
            item.multi -= 1;
            true
        }
        Status::DogasAttack => {
            dogas_update_smog(item, ctx);
            if item.multi == 0 {
                item.multi = DOGAS_DESPAWN_WAIT;
                set(item, Status::DogasDisappear);
                return true;
            }
            v(item).smog_spawn_wait -= 1;
            true
        }
        Status::DogasDisappear => {
            if item.multi == 0 {
                return false;
            }
            item.multi -= 1;
            true
        }
    }
}

/// The descriptors' `proc_update`: the rise ends in the first status.
fn common_update(item: &mut Item, k: Kind, ctx: &mut Ctx<'_>) -> bool {
    if item.multi == 0 {
        match k {
            Kind::Iwark => iwark_fly(item),
            Kind::Kabigon => kabigon_jump(item),
            Kind::Tosakinto => {
                item.vel_air.y = 0.0;
                tosakinto_appear(item);
            }
            Kind::Mew => {
                item.vel_air.y = 0.0;
                mew_fly(item, ctx);
            }
            Kind::Nyars => {
                item.vel_air.x = 0.0;
                item.vel_air.y = 0.0;
                nyars_attack(item);
            }
            Kind::Lizardon => {
                item.multi = LIZARDON_LIFETIME;
                item.vel_air.y = 0.0;
                set(item, Status::LizardonFall);
            }
            Kind::Spear => spear_appear(item, k),
            Kind::Kamex => {
                item.vel_air.y = 0.0;
                kamex_appear(item);
            }
            Kind::MLucky => {
                item.vel_air.y = 0.0;
                // `itMLuckyAppearSetStatus`: Chansey itself, not
                // Clefairy's copy.
                crate::sound::play_fgm(crate::sound::id::nSYAudioVoiceMBallLuckyAppear);
                set(item, Status::MLuckyAppear);
            }
            Kind::Starmie => {
                item.vel_air.x = 0.0;
                item.vel_air.y = 0.0;
                starmie_nfollow(item, k, ctx.owners);
            }
            Kind::Sawamura => {
                item.multi = SAWAMURA_KICK_WAIT;
                item.vel_air.y = 0.0;
                set(item, Status::SawamuraFall);
            }
            Kind::Dogas => {
                item.vel_air.x = 0.0;
                item.vel_air.y = 0.0;
                dogas_attack(item, k);
            }
            Kind::Pippi => {
                item.vel_air.x = 0.0;
                item.vel_air.y = 0.0;
                pippi_select(item, ctx);
            }
        }
    }
    item.multi = item.multi.wrapping_sub(1);
    true
}

/// `itPippiCommonSelectMonster`: `rand(12)` names a kind from Onix; its
/// first status runs on Clefairy. The twelfth entry is Mew's flight.
fn pippi_select(item: &mut Item, ctx: &mut Ctx<'_>) {
    let index = crate::rng::rand_int_range(12) as usize;
    let as_kind = Kind::ALL[index];
    if matches!(as_kind, Kind::Spear | Kind::Kamex) {
        if crate::rng::rand_int_range(2) == 0 {
            v(item).rotate_y = PI;
            item.lr = 1.0;
        } else {
            item.lr = -1.0;
        }
    }
    if matches!(as_kind, Kind::Pippi | Kind::Tosakinto | Kind::MLucky) {
        item.attack.state = AttackState::Off;
    }
    if as_kind == Kind::Sawamura {
        item.multi = SAWAMURA_KICK_WAIT;
    }
    if matches!(as_kind, Kind::Sawamura | Kind::Starmie) {
        v(item).xlu = true;
        item.vars.mmonster.link18_at = item.anim_ticks;
    }
    if as_kind == Kind::Lizardon {
        item.multi = LIZARDON_LIFETIME;
    }
    let k = Kind::Pippi;
    match as_kind {
        Kind::Iwark => iwark_attack(item, k, ctx),
        Kind::Kabigon => kabigon_jump(item),
        Kind::Tosakinto => tosakinto_appear(item),
        Kind::Nyars => nyars_attack(item),
        Kind::Lizardon => set(item, Status::LizardonFall),
        Kind::Spear => spear_fly(item, k),
        Kind::Kamex => kamex_appear(item),
        Kind::MLucky => set(item, Status::MLuckyAppear),
        Kind::Starmie => starmie_nfollow(item, k, ctx.owners),
        Kind::Sawamura => set(item, Status::SawamuraFall),
        Kind::Dogas => dogas_attack(item, k),
        Kind::Pippi | Kind::Mew => mew_fly(item, ctx),
    }
}

/// `itIwarkFlySetStatus`.
fn iwark_fly(item: &mut Item) {
    item.multi = IWARK_FLY_WAIT;
    item.vel_air.x = 0.0;
    item.vel_air.y = 0.0;
    set(item, Status::IwarkFly);
}

/// `itIwarkAttackSetStatus` (US: the item is airborne again).
fn iwark_attack(item: &mut Item, k: Kind, ctx: &mut Ctx<'_>) {
    item.ga = super::Ga::Air;
    item.vel_air.y = IWARK_FLY_VEL_Y;
    let remain = crate::rng::rand_int_range(IWARK_ROCK_SPAWN_COUNT_RANDOM) as u16
        + IWARK_ROCK_SPAWN_COUNT_MIN;
    let vars = v(item);
    vars.rock_spawn_remain = remain;
    vars.rock_spawn_max = remain;
    vars.rock_spawn_count = 0;
    vars.rock_spawn_wait = 0;
    vars.rumble_frame = 0;
    vars.iwark_rumble_wait = 0;
    item.multi = 0;
    let mut pos = item.pos;
    if k == Kind::Iwark {
        v(item).attack_dl = true;
        pos.y += -660.0;
    } else {
        pos.y += -100.0;
    }
    ctx.fx.push(Fx::DustHeavyDouble { pos, lr: -1 });
    if k == Kind::Iwark {
        crate::sound::play_fgm(crate::sound::id::nSYAudioVoiceMBallIwarkAppear);
    }
    set(item, Status::IwarkAttack);
}

/// `itIwarkAttackProcUpdate` (US): it stops 200 under the top bound and
/// drops rocks until all it made have left the stage.
fn iwark_attack_update(item: &mut Item, ctx: &mut Ctx<'_>) -> bool {
    let pos_y = ctx.bounds.map_or(f32::MAX, |b| b.top) - IWARK_FLY_STOP_Y;
    if item.pos.y >= pos_y {
        item.pos.y = pos_y;
        item.vel_air.y = 0.0;
        if v(item).rock_spawn_remain != 0 {
            iwark_update_rock(item, ctx);
        } else if v(item).rock_spawn_count == v(item).rock_spawn_max {
            return false;
        }
        let vars = v(item);
        if vars.iwark_rumble_wait == 0 && vars.rumble_frame != 0 {
            ctx.fx.push(Fx::Quake(0));
            vars.iwark_rumble_wait = IWARK_ROCK_RUMBLE_WAIT;
        }
        if vars.rumble_frame != 0 {
            vars.iwark_rumble_wait = vars.iwark_rumble_wait.wrapping_sub(1);
        }
        vars.rock_spawn_wait -= 1;
    }
    // The fly status's last countdown leaves `multi` at 0xFFFF, so the
    // first turn waits one more frame.
    if item.multi == IWARK_MODEL_ROTATE_WAIT {
        v(item).rotate_y += PI;
        item.multi = 0;
    }
    item.multi = item.multi.wrapping_add(1);
    true
}

/// `itIwarkAttackUpdateRock`: a failed make draws the position and speed
/// but not the facing, and retries next frame.
fn iwark_update_rock(item: &mut Item, ctx: &mut Ctx<'_>) {
    if v(item).rock_spawn_wait > 0 {
        return;
    }
    let mut pos = item.pos;
    pos.x += 2000.0 * crate::rng::rand_float() - 1000.0;
    let random = crate::rng::rand_int_range(IWARK_ROCK_RANDOM_VEL_MAX) as u8;
    if !ctx.shots.has_free() {
        return;
    }
    ctx.shots
        .push(MonsterShot::rock(parent(item, ctx.handle), pos, random));
    let vars = v(item);
    vars.rock_spawn_remain -= 1;
    vars.rock_spawn_wait =
        crate::rng::rand_int_range(IWARK_ROCK_SPAWN_WAIT_MAX) + IWARK_ROCK_SPAWN_WAIT_MIN;
}

/// One of Onix's rocks left the stage (`itIwarkWeaponRockProcDead`), or met
/// a new floor (`rumble_frame`).
pub(super) fn rock_event(item: &mut Item, dead: bool) {
    let vars = v(item);
    if dead {
        vars.rock_spawn_count = vars.rock_spawn_count.wrapping_add(1);
    } else {
        vars.rumble_frame = vars.rumble_frame.wrapping_add(1);
    }
}

/// `itKabigonJumpSetStatus`.
fn kabigon_jump(item: &mut Item) {
    crate::sound::play_fgm(crate::sound::id::nSYAudioFGMKabigonJump);
    item.multi = KABIGON_DROP_WAIT;
    v(item).dust_effect_int = KABIGON_EFFECT_SPAWN_INT;
    item.vel_air.y = KABIGON_JUMP_VEL_Y;
    set(item, Status::KabigonJump);
}

/// `itKabigonFallSetStatus`: it falls somewhere along a 2000-wide band,
/// four (Clefairy 5.2) times its size.
fn kabigon_fall(item: &mut Item, k: Kind) {
    item.vel_air.y = KABIGON_DROP_VEL_Y;
    item.pos.x += 2000.0 * crate::rng::rand_float() - 1000.0;
    item.refresh_attack_coll();
    v(item).kabigon_rumble_wait = 0;
    crate::sound::play_fgm(crate::sound::id::nSYAudioFGMKabigonFall);
    if k == Kind::Kabigon {
        crate::sound::play_fgm(crate::sound::id::nSYAudioVoiceMBallKabigonFall);
    }
    let size = if k == Kind::Kabigon {
        KABIGON_DROP_SIZE_KABIGON
    } else {
        KABIGON_DROP_SIZE_OTHER
    };
    item.scale.x = size;
    item.scale.y = size;
    item.attack.size *= size;
    v(item).xlu = true;
    item.vars.mmonster.link18_at = item.anim_ticks;
    set(item, Status::KabigonFall);
}

/// `itTosakintoAppearSetStatus`.
fn tosakinto_appear(item: &mut Item) {
    item.multi = TOSAKINTO_LIFETIME;
    if kind(item) == Kind::Tosakinto {
        crate::sound::play_fgm(crate::sound::id::nSYAudioVoiceMBallTosakintoAppear);
    }
    set(item, Status::TosakintoAppear);
}

/// `itTosakintoBounceSetStatus`.
fn tosakinto_bounce(item: &mut Item) {
    item.vel_air.y = TOSAKINTO_FLAP_VEL_Y;
    item.vel_air.x = TOSAKINTO_FLAP_VEL_X;
    if kind(item) == Kind::Tosakinto {
        start_anim(item);
    }
    set(item, Status::TosakintoBounce);
}

/// `itMewFlySetStatus`.
fn mew_fly(item: &mut Item, ctx: &mut Ctx<'_>) {
    item.multi = MEW_LIFETIME;
    item.vel_air.x = if crate::rng::rand_int_range(2) != 0 {
        MEW_STARTVEL_X
    } else {
        -MEW_STARTVEL_X
    };
    item.vel_air.y = MEW_STARTVEL_Y;
    crate::sound::play_fgm(crate::sound::id::nSYAudioFGMMewFly);
    if kind(item) == Kind::Mew {
        crate::sound::play_fgm(crate::sound::id::nSYAudioVoiceMBallMewAppear);
    }
    ctx.fx.push(Fx::Ripple(item.pos));
    v(item).esper_gfx_int = 0;
    set(item, Status::MewFly);
}

/// `itNyarsAttackSetStatus`.
fn nyars_attack(item: &mut Item) {
    item.multi = NYARS_LIFETIME;
    let vars = v(item);
    vars.coin_spawn_wait = NYARS_LIFETIME - NYARS_COIN_SPAWN_WAIT / 2;
    vars.coin_rotate_step = 0;
    vars.model_rotate_wait = NYARS_MODEL_ROTATE_WAIT;
    set(item, Status::NyarsAttack);
}

/// `itLizardonAttackSetStatus` then `itLizardonAttackInitVars`.
fn lizardon_attack(item: &mut Item, k: Kind) {
    set(item, Status::LizardonAttack);
    v(item).turn_wait = LIZARDON_TURN_WAIT;
    v(item).flame_spawn_wait = 0;
    item.lr = -1.0;
    if k == Kind::Lizardon {
        start_anim(item);
    }
}

/// `itLizardonAttackProcUpdate`: a flame every eight frames, a turn every
/// 26 (only Clefairy's model turns with it; Charizard's animation turns).
fn lizardon_attack_update(item: &mut Item, k: Kind, ctx: &mut Ctx<'_>) -> bool {
    let mut pos = item.pos;
    if k == Kind::Lizardon {
        pos.y += LIZARDON_LIZARDON_FLAME_OFF_Y;
        pos.x += LIZARDON_LIZARDON_FLAME_OFF_X * item.lr;
    } else {
        pos.x += LIZARDON_OTHER_FLAME_OFF_X * item.lr;
    }
    if v(item).flame_spawn_wait == 0 {
        let (sin, cos) = sin_cos(LIZARDON_FLAME_ANGLE);
        let vel = Vec3::new(
            cos * LIZARDON_FLAME_VEL * item.lr,
            sin * LIZARDON_FLAME_VEL,
            0.0,
        );
        if ctx.shots.has_free() {
            let shot = MonsterShot::lizardon_flame(parent(item, ctx.handle), pos, vel);
            shot.make_fx(ctx.fx);
            ctx.shots.push(shot);
        }
        v(item).flame_spawn_wait = LIZARDON_FLAME_SPAWN_WAIT;
    }
    v(item).flame_spawn_wait = v(item).flame_spawn_wait.wrapping_sub(1);
    if item.multi == 0 {
        return false;
    }
    if v(item).turn_wait == 0 {
        v(item).turn_wait = LIZARDON_TURN_WAIT;
        item.lr = -item.lr;
        let attr = item.attr.map_coll;
        let mut pos = item.pos;
        pos.y += attr.bottom;
        pos.x += (attr.width + LIZARDON_DUST_OFF_X) * -item.lr;
        ctx.fx.push(Fx::DustHeavy {
            pos,
            lr: -item.lr as i8,
        });
        if k == Kind::Pippi {
            v(item).rotate_y += PI;
        }
    }
    v(item).turn_wait = v(item).turn_wait.wrapping_sub(1);
    item.multi -= 1;
    true
}

/// `itSpearAppearSetStatus`.
fn spear_appear(item: &mut Item, k: Kind) {
    item.multi = 0;
    item.vel_air.y = 0.0;
    if k == Kind::Spear {
        start_anim(item);
    }
    v(item).spear_anim_frame = 0;
    set(item, Status::SpearAppear);
}

/// `itSpearFlySetStatus`.
fn spear_fly(item: &mut Item, k: Kind) {
    item.vel_air.y = SPEAR_SWARM_CALL_VEL_Y;
    let y = item.pos.y;
    let vars = v(item);
    vars.spear_spawn_pos_y = y;
    vars.spear_spawn_wait = 0;
    vars.spear_spawn_count = SPEAR_SPAWN_COUNT;
    // `dobj->child->anim_joint.event32 = NULL`: the appear script stops.
    if k == Kind::Spear {
        vars.anim_frozen = true;
    }
    set(item, Status::SpearFly);
}

/// `itSpearFlyProcUpdate`: it flies off to one side and calls its swarm
/// across the stage, then is gone.
fn spear_fly_update(item: &mut Item, k: Kind, ctx: &mut Ctx<'_>) -> bool {
    item.apply_gravity_clamp_tvel(SPEAR_GRAVITY, SPEAR_TVEL);
    item.vel_air.x += SPEAR_SWARM_CALL_VEL_X * item.lr;
    let (left, right) = ctx
        .bounds
        .map_or((f32::MIN, f32::MAX), |b| (b.left, b.right));
    let arrived = (item.lr == 1.0 && item.pos.x >= right - SPEAR_SWARM_CALL_OFF_X)
        || (item.lr == -1.0 && item.pos.x <= left + SPEAR_SWARM_CALL_OFF_X);
    if arrived {
        item.vel_air.x = 0.0;
        item.vel_air.y = 0.0;
        if v(item).spear_spawn_count == 0 {
            return false;
        }
        if v(item).spear_spawn_wait <= 0 {
            let mut pos = item.pos;
            pos.y = v(item).spear_spawn_pos_y;
            pos.y += SPEAR_SPAWN_OFF_Y_MUL * crate::rng::rand_float() + SPEAR_SPAWN_OFF_Y_ADD;
            if ctx.shots.has_free() {
                ctx.shots.push(MonsterShot::swarm(
                    parent(item, ctx.handle),
                    pos,
                    k != Kind::Spear,
                ));
            }
            let vars = v(item);
            vars.spear_spawn_count -= 1;
            vars.spear_spawn_wait =
                crate::rng::rand_int_range(SPEAR_SPAWN_WAIT_RANDOM) + SPEAR_SPAWN_WAIT_CONST;
        }
        v(item).spear_spawn_wait -= 1;
    }
    true
}

/// `itKamexAppearSetStatus`.
fn kamex_appear(item: &mut Item) {
    item.multi = KAMEX_LIFETIME;
    if kind(item) == Kind::Kamex {
        crate::sound::play_fgm(crate::sound::id::nSYAudioVoiceMBallKamexAppear);
    }
    set(item, Status::KamexAppear);
}

/// `itKamexFallSetStatus`.
fn kamex_fall(item: &mut Item) {
    map::set_air(item);
    item.vel_air.x = 0.0;
    item.vel_air.y = 0.0;
    item.is_allow_pickup = false;
    v(item).hydro_push_vel_x = 0.0;
    set(item, Status::KamexFall);
}

/// `itKamexAttackInitVars` then `itKamexAttackSetStatus`. Blastoise's
/// attack model takes a larger map box.
fn kamex_attack(item: &mut Item, is_ignore_setup: bool) {
    if !is_ignore_setup {
        item.multi = KAMEX_LIFETIME;
        if kind(item) == Kind::Kamex {
            v(item).attack_dl = true;
            item.coll = coll(KAMEX_COLL_SIZE, -KAMEX_COLL_SIZE, KAMEX_COLL_SIZE);
        }
    }
    item.vel_air.x = 0.0;
    item.vel_air.y = 0.0;
    let lr = item.lr;
    let vars = v(item);
    vars.hydro_push_vel_x = lr * KAMEX_PUSH_VEL_X;
    vars.hydro_spawn_wait = 0;
    vars.is_apply_push = false;
    set(item, Status::KamexAttack);
}

/// `itKamexAttackUpdateHydro`: each stream pushes Blastoise back.
fn kamex_update_hydro(item: &mut Item, k: Kind, ctx: &mut Ctx<'_>) {
    if v(item).hydro_spawn_wait > 0 {
        return;
    }
    let mut pos = item.pos;
    if k == Kind::Kamex {
        pos.x += KAMEX_KAMEX_HYDRO_SPAWN_OFF_X * item.lr;
        pos.y += KAMEX_KAMEX_HYDRO_SPAWN_OFF_Y;
    } else {
        pos.x += KAMEX_OTHER_HYDRO_SPAWN_OFF_X * item.lr;
    }
    if ctx.shots.has_free() {
        let shot = MonsterShot::hydro(parent(item, ctx.handle), pos);
        shot.make_fx(ctx.fx);
        ctx.shots.push(shot);
    }
    ctx.fx.push(Fx::DamageSpawnSparks {
        pos,
        lr: item.lr as i8,
    });
    crate::sound::play_fgm(crate::sound::id::nSYAudioFGMKamexHydro);
    v(item).hydro_spawn_wait =
        crate::rng::rand_int_range(KAMEX_HYDRO_SPAWN_WAIT_RANDOM) + KAMEX_HYDRO_SPAWN_WAIT_CONST;
    let attr = item.attr.map_coll;
    let mut pos = item.pos;
    pos.y += attr.bottom;
    if k == Kind::Kamex {
        pos.x += (attr.width + KAMEX_DUST_SPAWN_OFF_X) * -item.lr;
    }
    v(item).is_apply_push = true;
    item.vel_air.x = -item.lr * KAMEX_CONSTVEL_X;
    ctx.fx.push(Fx::DustHeavy {
        pos,
        lr: -item.lr as i8,
    });
}

/// `itMLuckyMakeEggSetStatus` then `itMLuckyMakeEggInitVars`.
fn mlucky_make_egg(item: &mut Item) {
    set(item, Status::MLuckyMakeEgg);
    if kind(item) == Kind::MLucky {
        start_anim(item);
    }
    item.damage_coll.hitstatus = HitStatus::Normal;
    v(item).egg_spawn_wait = MLUCKY_EGG_SPAWN_WAIT_CONST;
    item.multi = MLUCKY_EGG_SPAWN_COUNT;
}

/// `itMLuckyFallSetStatus`.
fn mlucky_fall(item: &mut Item) {
    item.is_allow_pickup = false;
    map::set_air(item);
    set(item, Status::MLuckyFall);
}

/// `itMLuckyDisappearSetStatus`.
fn mlucky_disappear(item: &mut Item) {
    v(item).lucky_lifetime = MLUCKY_LIFETIME;
    item.damage_coll.hitstatus = HitStatus::None;
    set(item, Status::MLuckyDisappear);
}

/// `itMLuckyMakeEggProcUpdate`: three eggs, thirty frames apart; a failed
/// make retries the next frame. With eggs switched off the count still
/// runs.
fn mlucky_make_egg_update(item: &mut Item, ctx: &mut Ctx<'_>) {
    if item.multi == 0 {
        mlucky_disappear(item);
        return;
    }
    if v(item).egg_spawn_wait == 0 {
        if ctx.common.eggs_enabled() {
            let pos = item.pos;
            let vel = Vec3::new(
                crate::rng::rand_float() * MLUCKY_EGG_SPAWN_BASE_VEL + MLUCKY_EGG_SPAWN_ADD_VEL_X,
                crate::rng::rand_float() * MLUCKY_EGG_SPAWN_BASE_VEL + MLUCKY_EGG_SPAWN_ADD_VEL_Y,
                0.0,
            );
            if let Some(egg_lr) = ctx.common.make_common_egg(item, pos, vel) {
                crate::sound::play_fgm(crate::sound::id::nSYAudioFGMKirbySpecialLwStart);
                v(item).egg_spawn_wait = MLUCKY_EGG_SPAWN_WAIT_CONST;
                item.multi -= 1;
                ctx.fx.push(Fx::DustLight { pos, lr: egg_lr });
            }
        } else {
            v(item).egg_spawn_wait = MLUCKY_EGG_SPAWN_WAIT_CONST;
            item.multi -= 1;
        }
    }
    if v(item).egg_spawn_wait > 0 {
        v(item).egg_spawn_wait -= 1;
    }
}

/// `itStarmieNFollowSetStatus`: it flies at 20 toward a point 400 beside
/// its nearest opponent and 250 over its feet.
fn starmie_nfollow(item: &mut Item, k: Kind, owners: &[Option<OwnerView>; 4]) {
    if let Some(victim) = find_victim(item, owners) {
        let mut target = victim.pos;
        let dx = victim.pos.x - item.pos.x;
        target.y += STARMIE_TARGET_POS_OFF_Y - victim.coll.bottom;
        target.x -= (victim.coll.width + STARMIE_TARGET_POS_OFF_X) * lr_sign(dx);
        let dist = target - item.pos;
        item.vel_air = rotate_z(
            Vec3::new(STARMIE_FOLLOW_VEL_X, 0.0, 0.0),
            atan2(dist.y, dist.x),
        );
        let vars = v(item);
        vars.target_pos = target;
        vars.victim_pos = victim.pos;
        item.lr = lr_sign(dist.x);
        if item.lr == 1.0 {
            v(item).rotate_y = PI;
        }
    }
    if k == Kind::Starmie {
        start_anim(item);
    }
    set(item, Status::StarmieNFollow);
}

/// `itStarmieAttackSetStatus`.
fn starmie_attack(item: &mut Item) {
    let lr_bak = item.lr;
    item.lr = if v(item).victim_pos.x < item.pos.x {
        -1.0
    } else {
        1.0
    };
    if item.lr != lr_bak {
        v(item).rotate_y += PI;
    }
    item.multi = STARMIE_LIFETIME;
    let lr = item.lr;
    v(item).swift_spawn_wait = 0;
    v(item).add_vel_x = lr * STARMIE_ADD_VEL_X;
    set(item, Status::StarmieAttack);
}

/// `itStarmieAttackUpdateSwift`.
fn starmie_update_swift(item: &mut Item, k: Kind, ctx: &mut Ctx<'_>) {
    if v(item).swift_spawn_wait > 0 {
        return;
    }
    let mut pos = item.pos;
    if k == Kind::Starmie {
        pos.x += STARMIE_STARMIE_SWIFT_SPAWN_OFF_X * item.lr;
        pos.y += STARMIE_STARMIE_SWIFT_SPAWN_OFF_Y;
    } else {
        pos.x += STARMIE_OTHER_SWIFT_SPAWN_OFF_X * item.lr;
    }
    if ctx.shots.has_free() {
        let shot = MonsterShot::swift(parent(item, ctx.handle), pos);
        shot.make_fx(ctx.fx);
        ctx.shots.push(shot);
    }
    v(item).swift_spawn_wait = crate::rng::rand_int_range(STARMIE_SWIFT_SPAWN_WAIT_RANDOM)
        + STARMIE_SWIFT_SPAWN_WAIT_CONST;
    item.vel_air.x = -item.lr * STARMIE_PUSH_VEL_X;
}

/// `itSawamuraAttackSetStatus`: a 400 kick at a point 500 over its nearest
/// opponent's feet, with a 300 attack.
fn sawamura_attack(item: &mut Item, k: Kind, owners: &[Option<OwnerView>; 4]) {
    if let Some(victim) = find_victim(item, owners) {
        let mut target = victim.pos;
        target.y += SAWAMURA_TARGET_POS_OFF_Y - victim.coll.bottom;
        let dist = target - item.pos;
        item.vel_air = rotate_z(
            Vec3::new(SAWAMURA_KICK_VEL_X, 0.0, 0.0),
            atan2(dist.y, dist.x),
        );
        item.lr = lr_sign(dist.x);
        if item.lr == 1.0 {
            v(item).rotate_y = PI;
        }
    }
    if k == Kind::Sawamura {
        v(item).attack_dl = true;
    }
    item.multi = SAWAMURA_LIFETIME;
    item.attack.size = SAWAMURA_KICK_SIZE;
    set(item, Status::SawamuraAttack);
}

/// `itDogasAttackSetStatus`.
fn dogas_attack(item: &mut Item, k: Kind) {
    item.multi = DOGAS_SMOG_SPAWN_COUNT;
    v(item).smog_spawn_wait = 0;
    if k == Kind::Dogas {
        start_anim(item);
        crate::sound::play_fgm(crate::sound::id::nSYAudioVoiceMBallDogasAppear);
    }
    set(item, Status::DogasAttack);
}

/// `itDogasAttackUpdateSmog`: a cloud every eight frames, somewhere in a
/// 400 by 800 box, drifting away from Koffing.
fn dogas_update_smog(item: &mut Item, ctx: &mut Ctx<'_>) {
    if v(item).smog_spawn_wait > 0 {
        return;
    }
    let mut vel = Vec3::new(DOGAS_SMOG_VEL, DOGAS_SMOG_VEL, 0.0);
    let mut pos = item.pos;
    pos.x += crate::rng::rand_float() * DOGAS_SMOG_MUL_OFF_X - DOGAS_SMOG_SUB_OFF_X;
    pos.y += crate::rng::rand_float() * DOGAS_SMOG_MUL_OFF_Y - DOGAS_SMOG_SUB_OFF_Y;
    if pos.x < item.pos.x {
        vel.x = -vel.x;
    }
    if pos.y < item.pos.y {
        vel.y = -vel.y;
    }
    if ctx.shots.has_free() {
        ctx.shots
            .push(MonsterShot::smog(parent(item, ctx.handle), pos, vel));
    }
    crate::sound::play_fgm(crate::sound::id::nSYAudioFGMDogasSmog);
    v(item).smog_spawn_wait = DOGAS_SMOG_SPAWN_WAIT;
    item.multi -= 1;
}

/// `syVectorRotate3D(v, SYVECTOR_AXIS_Z, angle)`.
fn rotate_z(v: Vec3, angle: f32) -> Vec3 {
    let (sin, cos) = sin_cos(angle);
    Vec3::new(v.x * cos - v.y * sin, v.x * sin + v.y * cos, v.z)
}

/// Whether the status has a `proc_map`.
pub(super) fn has_proc_map(item: &Item, status: Status) -> bool {
    match status {
        // Snorlax's descriptor has none.
        Status::Common => kind(item) != Kind::Kabigon,
        Status::TosakintoAppear
        | Status::TosakintoBounce
        | Status::LizardonFall
        | Status::LizardonAttack
        | Status::KamexFall
        | Status::KamexAppear
        | Status::KamexAttack
        | Status::MLuckyFall
        | Status::MLuckyAppear
        | Status::MLuckyMakeEgg
        | Status::MLuckyDisappear
        | Status::SawamuraFall
        | Status::SawamuraWait => true,
        _ => false,
    }
}

/// `ip->proc_map`. Returns whether the item lives on. Each arm is one
/// source callback.
#[allow(clippy::collapsible_match)]
pub(super) fn proc_map<I, F>(item: &mut Item, status: Status, surfaces: &F) -> bool
where
    F: Fn() -> I,
    I: IntoIterator<Item = MapSurface>,
{
    use crate::map::{MASK_CEIL, MASK_FLOOR, MASK_LWALL, MASK_RWALL};
    match status {
        // The descriptors': a floor stops the rise (Onix also grounds).
        Status::Common => {
            if map::test_all_collision_flag(item, MASK_FLOOR, surfaces) {
                item.vel_air.y = 0.0;
                if kind(item) == Kind::Iwark {
                    map::set_ground(item);
                }
            }
        }
        Status::TosakintoAppear => {
            if map::test_all_check_coll_end(item, surfaces) {
                item.vel_air.y = TOSAKINTO_FLAP_VEL_Y;
                tosakinto_bounce(item);
                crate::sound::play_fgm(crate::sound::id::nSYAudioFGMTosakintoSplash);
            }
        }
        Status::TosakintoBounce => {
            if map::test_all_check_coll_end(item, surfaces) {
                item.vel_air.y = TOSAKINTO_FLAP_VEL_Y;
                if crate::rng::rand_int_range(2) != 0 {
                    item.vel_air.x = -item.vel_air.x;
                }
                crate::sound::play_fgm(crate::sound::id::nSYAudioFGMTosakintoSplash);
            }
        }
        Status::LizardonFall => {
            if map::test_all_check_coll_end(item, surfaces) {
                item.vel_air.y = 0.0;
                lizardon_attack(item, kind(item));
            }
        }
        Status::LizardonAttack => {
            if !map::check_lr_wall_proc_no_floor(item, surfaces) {
                set(item, Status::LizardonFall);
            }
        }
        Status::KamexFall | Status::KamexAppear => {
            map::test_all_collision_flag(item, MASK_CEIL | MASK_RWALL | MASK_LWALL, surfaces);
            if item.mask_curr & MASK_FLOOR != 0 {
                let appear = status == Status::KamexAppear;
                if appear {
                    item.vel_air.y = 0.0;
                }
                kamex_attack(item, !appear);
            }
        }
        Status::KamexAttack => {
            if !map::check_lr_wall_proc_no_floor(item, surfaces) {
                kamex_fall(item);
            }
        }
        Status::MLuckyFall => {
            if map::test_all_check_coll_end(item, surfaces) {
                item.vel_air.y = 0.0;
                if item.multi != 0 {
                    set(item, Status::MLuckyMakeEgg);
                } else {
                    mlucky_disappear(item);
                }
            }
        }
        Status::MLuckyAppear => {
            if map::test_all_check_coll_end(item, surfaces) {
                item.vel_air.y = 0.0;
                mlucky_make_egg(item);
            }
        }
        Status::MLuckyMakeEgg | Status::MLuckyDisappear => {
            if !map::check_lr_wall_proc_no_floor(item, surfaces) {
                mlucky_fall(item);
            }
        }
        Status::SawamuraFall => {
            if map::test_all_collision_flag(item, MASK_FLOOR, surfaces) {
                item.vel_air.y = 0.0;
                set(item, Status::SawamuraWait);
            }
        }
        Status::SawamuraWait => {
            if !map::check_lr_wall_proc_no_floor(item, surfaces) {
                set(item, Status::SawamuraFall);
            }
        }
        _ => {}
    }
    true
}

/// `itMLuckyMakeEggProcDamage`: a hit puts the next egg back four frames.
pub(super) fn hit_proc(item: &mut Item, status: Status, proc: HitProc) -> Option<bool> {
    if status == Status::MLuckyMakeEgg && proc == HitProc::Damage {
        v(item).egg_spawn_wait = v(item)
            .egg_spawn_wait
            .wrapping_add(MLUCKY_EGG_SPAWN_WAIT_ADD);
        return Some(true);
    }
    None
}
