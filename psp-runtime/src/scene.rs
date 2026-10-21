//! The pack-to-game bridge: a fighter standing on a real stage, on device.
//!
//! `ssb-rom` knows ROM/pack data. `ssb-game` knows portable gameplay. This
//! module joins those two systems on PSP: it walks the pack's collision
//! tables into the shape [`ssb_game::collision`] wants, converts packed
//! fighter data into `ssb-game`'s own attribute structs, ticks the fighter,
//! and keeps its skeleton/animation and battle-camera state in sync.
//!
//! ## Why the adapter is here and not in either crate
//!
//! `ssb-rom` must not know game logic and `ssb-game` must not know the pack
//! format, so [`FloorSegments`] is duplicated from `romtool`'s copy on
//! purpose. It is twenty lines, and the alternative is a shared type that
//! drags one crate into the other.
//!
//! [`FloorSegments`] allocates nothing: it reads segments straight out of the
//! mapped pack as the query asks for them, for the few queries that place a
//! fighter. [`MapSegments`], which every map query of every tick walks,
//! decodes a stage's surfaces once and keeps the last two stages' (RE-471).
//!
//! This module owns only what connects `ssb-rom`'s `Pack` to `ssb-game`'s
//! `Fighter` to skeleton/animation to battle-camera/runtime rendering state.
//! It does not own menus, Training rules, viewer diagnostics, CPU logic,
//! stocks, or match rules -- those stay in the applications that use it.

use ssb_game::camera::CameraStatus;
use ssb_game::collision::Segment;
use ssb_game::fighter::{Fighter, FighterKind};
use ssb_game::ground::BodyColl;
use ssb_game::physics::PhysicsAttributes;
use ssb_game::status::AnimLengths;
use ssb_game::weapon::{MapSurface, MapSurfaceKind, SurfaceTopology};
use ssb_rom::pack::ObjectDesc;
use ssb_rom::pack::{line_kind, FighterDesc, LineDesc, MeshDesc, Pack, StageDesc};

/// Mario Special1's direct weapon display list. Unlike fighters and stages,
/// the source descriptor names geometry directly instead of through a graph.
pub const MARIO_FIREBALL_SOURCE_FILE: u32 = 297;
pub const MARIO_FIREBALL_SOURCE_OFFSET: u32 = 0x1D8;

/// Luigi's Fireball: the same list bound to `palettes[1]`
/// (`wpMarioFireballMakeWeapon`'s `palette_id = anim_frame`). romtool keys it
/// by that palette's file offset, since the list is shared.
pub const LUIGI_FIREBALL_SOURCE_OFFSET: u32 = 0x08;

/// The packed Fireball mesh for each `FIREBALL_ATTRIBUTES` row: index 0 is
/// Mario, index 1 Luigi. This and the lookups below scan whole descriptor
/// tables; resolve them once per pack, never per frame (RE-360).
pub fn fireball_meshes(pack: &Pack<'_>) -> [Option<MeshDesc>; 2] {
    let find = |offset| {
        (0..pack.mesh_count())
            .filter_map(|i| pack.mesh(i))
            .find(|mesh| {
                mesh.source_file == MARIO_FIREBALL_SOURCE_FILE && mesh.source_offset == offset
            })
    };
    [
        find(MARIO_FIREBALL_SOURCE_OFFSET),
        find(LUIGI_FIREBALL_SOURCE_OFFSET),
    ]
}

/// Fox Special1's `WPAttributes.data` resolves to this direct weapon list.
pub const FOX_BLASTER_SOURCE_FILE: u32 = 316;
pub const FOX_BLASTER_SOURCE_OFFSET: u32 = 0x40;

pub fn fox_blaster_mesh(pack: &Pack<'_>) -> Option<MeshDesc> {
    (0..pack.mesh_count())
        .filter_map(|i| pack.mesh(i))
        .find(|mesh| {
            mesh.source_file == FOX_BLASTER_SOURCE_FILE
                && mesh.source_offset == FOX_BLASTER_SOURCE_OFFSET
        })
}

/// Samus Special1's `WPAttributes.data`: file 321's 30x30 quad list.
pub const SAMUS_CHARGE_SHOT_SOURCE_FILE: u32 = 321;
pub const SAMUS_CHARGE_SHOT_SOURCE_OFFSET: u32 = 0x270;

pub fn samus_charge_shot_mesh(pack: &Pack<'_>) -> Option<MeshDesc> {
    (0..pack.mesh_count())
        .filter_map(|i| pack.mesh(i))
        .find(|mesh| {
            mesh.source_file == SAMUS_CHARGE_SHOT_SOURCE_FILE
                && mesh.source_offset == SAMUS_CHARGE_SHOT_SOURCE_OFFSET
        })
}

/// Samus's Bomb: file 320's direct list at 0xE0D8 with `palettes[0]`; the
/// `palettes[1]` blink copy is keyed by that palette's offset, 0xDF38.
pub const SAMUS_BOMB_SOURCE_FILE: u32 = 320;
pub const SAMUS_BOMB_SOURCE_OFFSETS: [u32; 2] = [0xE0D8, 0xDF38];

/// The packed Bomb mesh for each `SamusBomb::blink_palette`.
pub fn samus_bomb_meshes(pack: &Pack<'_>) -> [Option<MeshDesc>; 2] {
    SAMUS_BOMB_SOURCE_OFFSETS.map(|offset| {
        (0..pack.mesh_count())
            .filter_map(|i| pack.mesh(i))
            .find(|mesh| mesh.source_file == SAMUS_BOMB_SOURCE_FILE && mesh.source_offset == offset)
    })
}

fn mesh_keyed(pack: &Pack<'_>, file: u32, offset: u32) -> Option<MeshDesc> {
    (0..pack.mesh_count())
        .filter_map(|i| pack.mesh(i))
        .find(|mesh| mesh.source_file == file && mesh.source_offset == offset)
}

/// Pikachu's Thunder frames in `mobj->texture_id_curr` order (RE-417): the
/// head, trail and fading segment draw one of them.
pub fn pikachu_thunder_meshes(pack: &Pack<'_>) -> [Option<MeshDesc>; 4] {
    ssb_rom::effect::PIKACHU_THUNDER_SPRITES
        .map(|sprite| mesh_keyed(pack, ssb_rom::effect::PIKACHU_THUNDER_FILE, sprite))
}

/// Ness's PK Thunder trail frames in `mobj->texture_id_curr` order
/// (RE-550): the trail list (attributes at file 239 + 0x40; file 335's
/// `DObjDLLink` array at 0x8B40) with each
/// of its `MObjSub`'s three sprites bound.
pub fn ness_pk_trail_meshes(pack: &Pack<'_>) -> [Option<MeshDesc>; 3] {
    ssb_rom::effect::NESS_PK_TRAIL_SPRITES
        .map(|sprite| mesh_keyed(pack, ssb_rom::effect::NESS_PK_TRAIL_FILE, sprite))
}

/// The Egg Lay egg's tree and its three animations by
/// `effect_vars.yoshi_egg_lay.index`: Wait, Break and the `EFDesc`'s own
/// Throw (RE-417).
pub fn yoshi_egg_lay_effect(pack: &Pack<'_>) -> Option<(ObjectDesc, [ssb_rom::pack::AnimDesc; 3])> {
    let (object, throw) = manager_effect(pack, ssb_rom::effect::YOSHI_EGG_LAY_KEY)?;
    let slot = ssb_rom::effect::YOSHI_EGG_LAY_ANIM_SLOT;
    Some((object, [pack.effect_anim(slot)?, pack.effect_anim(slot + 1)?, throw]))
}

/// Yoshi's Egg Throw `WPAttributes.data` (file 247 + 0x0C): file 338's
/// direct list at 0xA860. The list sets its own render mode, so the
/// discovered mesh draws as the weapon does.
pub const YOSHI_EGG_SOURCE: (u32, u32) = (338, 0xA860);
/// Yoshi's Bomb star: file 86's list at 0x5458 under the weapon seed,
/// keyed by `llYoshiMainStarWeaponAttributes` (file 247 + 0x40).
pub const YOSHI_STAR_SOURCE: (u32, u32) = (247, 0x40);

pub fn yoshi_egg_mesh(pack: &Pack<'_>) -> Option<MeshDesc> {
    mesh_keyed(pack, YOSHI_EGG_SOURCE.0, YOSHI_EGG_SOURCE.1)
}

pub fn yoshi_star_mesh(pack: &Pack<'_>) -> Option<MeshDesc> {
    mesh_keyed(pack, YOSHI_STAR_SOURCE.0, YOSHI_STAR_SOURCE.1)
}

/// The Kirby capture and lost stars' list (`llITCommonDataKirbyStarDObjDesc`,
/// the Star Rod star's `data`): the same file 86 list at 0x5458 as Yoshi's
/// star, keyed by the list itself and converted under the effect link's CLD
/// state on head 1 (`lbCommonDObjScaleXProcDisplay`, RE-551), not the
/// weapon's `G_RM_AA_XLU_SURF`.
pub const KIRBY_STAR_SOURCE: (u32, u32) = (86, 0x5458);

pub fn kirby_star_mesh(pack: &Pack<'_>) -> Option<MeshDesc> {
    mesh_keyed(pack, KIRBY_STAR_SOURCE.0, KIRBY_STAR_SOURCE.1)
}

/// Sector Z's Arwing lasers: file 153's list, keyed by the 2D shot's
/// `WPAttributes` (file 262 + 0xBC); the 3D shot names the same list
/// (RE-428).
pub fn arwing_laser_mesh(pack: &Pack<'_>) -> Option<MeshDesc> {
    mesh_keyed(
        pack,
        ssb_rom::sector::MAP_FILE,
        ssb_rom::sector::LASER_2D_ATTRIBUTES,
    )
}

/// Link Special1's `WPAttributes.data`: file 325's three-node Boomerang
/// `DObjDesc` tree.
pub const LINK_BOOMERANG_SOURCE_FILE: u32 = 325;
pub const LINK_BOOMERANG_SOURCE_OFFSET: u32 = 0x610;

pub fn link_boomerang_object(pack: &Pack<'_>) -> Option<ObjectDesc> {
    (0..pack.object_count())
        .filter_map(|i| pack.object(i))
        .find(|object| {
            object.source_file == LINK_BOOMERANG_SOURCE_FILE
                && object.source_offset == LINK_BOOMERANG_SOURCE_OFFSET
        })
}

/// `dEFManagerLinkSpinAttackEffectDesc`: file 353's two-node swirl, at
/// slot 38 of `ssb_rom::effect::MANAGER_EFFECT_KEYS`.
pub const LINK_SPIN_ATTACK_EFFECT_KEY: (u32, u32) = (353, 0x11C0);

/// The Spin Attack swirl's object and its manager-effect inventory slot.
pub fn link_spin_attack_effect(pack: &Pack<'_>) -> Option<(ObjectDesc, u32)> {
    let slot = ssb_rom::effect::MANAGER_EFFECT_KEYS
        .iter()
        .position(|&key| key == LINK_SPIN_ATTACK_EFFECT_KEY)?;
    let object = (0..pack.object_count())
        .filter_map(|i| pack.object(i))
        .find(|object| (object.source_file, object.source_offset) == LINK_SPIN_ATTACK_EFFECT_KEY)?;
    Some((object, slot as u32))
}

/// `dEFManagerCaptainFalconPunchEffectDesc`: file 333's single-DObj flame
/// (a direct display list, no `EFFECT_FLAG` tree bit), at slot 31 of
/// `ssb_rom::effect::MANAGER_EFFECT_KEYS`. It has a material animation and
/// no transform animation.
pub const CAPTAIN_FALCON_PUNCH_EFFECT_KEY: (u32, u32) = (333, 0x0760);

pub fn captain_falcon_punch_effect(pack: &Pack<'_>) -> Option<ObjectDesc> {
    (0..pack.object_count())
        .filter_map(|i| pack.object(i))
        .find(|object| {
            (object.source_file, object.source_offset) == CAPTAIN_FALCON_PUNCH_EFFECT_KEY
        })
}

/// Kirby's Final Cutter wave (`llKirbyMainCutterWeaponAttributes`, file
/// 229 + 0x08): file 328's two-node `DObjDesc` tree, packed under the weapon
/// seed (RE-378).
pub const KIRBY_CUTTER_SOURCE: (u32, u32) = (328, 0x1D388);

pub fn kirby_cutter_object(pack: &Pack<'_>) -> Option<ObjectDesc> {
    (0..pack.object_count())
        .filter_map(|i| pack.object(i))
        .find(|object| (object.source_file, object.source_offset) == KIRBY_CUTTER_SOURCE)
}

/// Pikachu's aerial Thunder Jolt (file 244 + 0x00): file 342's direct list
/// at 0x270, packed as a one-node object under the weapon seed (RE-379).
pub const PIKACHU_JOLT_AIR_SOURCE: (u32, u32) = (342, 0x270);
/// The ground Thunder Jolt (file 244 + 0x34): file 342's eight-node tree.
pub const PIKACHU_JOLT_GROUND_SOURCE: (u32, u32) = (342, 0x1888);

pub fn object_keyed(pack: &Pack<'_>, key: (u32, u32)) -> Option<ObjectDesc> {
    (0..pack.object_count())
        .filter_map(|i| pack.object(i))
        .find(|object| (object.source_file, object.source_offset) == key)
}

/// Peach's Castle's Bumper (`llITCommonDataGBumperItemAttributes`, file
/// 251 + 0xCF0): its exact extern target in `ITCommonObject` (RE-429).
pub const GBUMPER_ITEM_SOURCE: (u32, u32) = ssb_rom::ground_obj::GBUMPER_SOURCE;

/// Ness's PK Fire spark (file 240 + 0x00): file 336's direct list at 0x168,
/// packed as a one-node object (RE-381).
pub const NESS_PK_FIRE_SOURCE: (u32, u32) = (336, 0x168);
/// Link's Spin Attack weapon graphic
/// (`llLinkMainSpinAttackWeaponAttributes`, file 225 + 0x0C): file 324's
/// two-node tree, drawn through `wpDisplayDObjTreeDLLinks`.
pub const LINK_SPIN_ATTACK_WEAPON_SOURCE: (u32, u32) = (324, 0x11908);
/// The PK Thunder head (file 239 + 0x0C): file 335's two-node tree.
pub const NESS_PK_THUNDER_SOURCE: (u32, u32) = (335, 0x7C98);
/// `dEFManagerNessPKThunderTrailEffectDesc` and
/// `dEFManagerNessPKReflectTrailEffectDesc`'s one DObj
/// (`llNessModelPKThunderTrailDObjDesc`), no animation.
pub const NESS_PK_TRAIL_EFFECT_KEY: (u32, u32) = (335, 0x9050);
/// `dEFManagerNessPKThunderWaveEffectDesc`: file 335's three-node wave
/// tree and its animation.
pub const NESS_PK_WAVE_EFFECT_KEY: (u32, u32) = (335, 0x9A10);

/// `dEFManagerNessPsychicMagnetEffectDesc`: file 352's field tree, at slot
/// 33 of `ssb_rom::effect::MANAGER_EFFECT_KEYS`.
pub const NESS_PSI_MAGNET_EFFECT_KEY: (u32, u32) = (352, 0x09A8);

/// The PSI Magnet field's object and its manager-effect inventory slot.
pub fn ness_psi_magnet_effect(pack: &Pack<'_>) -> Option<(ObjectDesc, u32)> {
    let slot = ssb_rom::effect::MANAGER_EFFECT_KEYS
        .iter()
        .position(|&key| key == NESS_PSI_MAGNET_EFFECT_KEY)?;
    Some((object_keyed(pack, NESS_PSI_MAGNET_EFFECT_KEY)?, slot as u32))
}

/// Ness's PK Fire flame item (file 240 + 0x34): file 336's four-node tree
/// (RE-382).
pub const NESS_PK_FIRE_ITEM_SOURCE: (u32, u32) = (336, 0x0A08);

/// Link's Bomb item (`llLinkMainBombItemAttributes`, file 225 + 0x40): file
/// 353's three-node tree (RE-383).
pub const LINK_BOMB_ITEM_SOURCE: (u32, u32) = (353, 0x18D8);

/// Where an entry effect's tree goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryRoot {
    /// `dobj->translate.vec.f = *pos` on the desc's own root (flag 0x4
    /// without 0x1): the root's rest translation is replaced.
    Replace,
    /// Flag 0x1's empty outer `DObj` takes the position, and the desc's
    /// tree plays under it (the Arwing, whose root is its flight path).
    Outer,
}

/// Which DL link an entry effect draws on this frame (RE-425).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryLink {
    /// The `EFDesc`'s own link (10 for every entry effect).
    Fixed(u8),
    /// `efManagerSortZNeg` on a node after each play: link 2 behind z
    /// -1000, else 20 (the Arwing, and the car unturned).
    SortZNeg(usize),
    /// The car: `efManagerSortZNeg`, or `efManagerSortZPos` (link 2 past z
    /// 1000, else 20) once turned for a leftward entry.
    SortCar(usize),
    /// `efManagerMBallThrownProcUpdate`: link 20 past z 1000, else 10, from
    /// the node's pose before this frame's play; the maker's first frame
    /// sorts it with `efManagerSortZNeg`.
    Ball(usize),
}

/// Which clock an entry part plays to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryClock {
    /// `Entry::effect_ticks`: made with the entry.
    Effect,
    /// `Entry::rays_ticks`: the Poké Ball's rays, made when it opens.
    Rays,
}

/// One entry effect part (RE-403, RE-425): its object, the animation slot
/// it plays (`AnimDesc::EFFECT`: the manager effect's own, or one of
/// `ssb_rom::effect::ENTRY_ANIMS`), its placement, and what ends it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntryPart {
    pub key: (u32, u32),
    pub anim_slot: u32,
    pub root: EntryRoot,
    /// `dobj->rotate.vec.f.y = F_CLC_DTOR32(180.0F)` for `lr == -1`.
    pub turns_left: bool,
    /// The node whose script's end ejects it (`dobj->child->anim_frame`);
    /// `None` for the object's `GObj::anim_frame` reaching 0
    /// (`efManagerHaveStructProcUpdate`, `efManagerNoStructProcUpdate`).
    pub end_node: Option<usize>,
    pub link: EntryLink,
    pub clock: EntryClock,
    /// Nodes that get a second `0x2C` (kind 44) matrix, which multiplies
    /// their billboard scale by their own X scale once more: the Arwing's
    /// node 10, the car's exhaust sprites 3, 5, 7 and 9.
    pub extra_billboards: &'static [usize],
}

/// `key`'s `ssb_rom::effect::MANAGER_EFFECT_KEYS` slot, which the pack
/// keys its own animation by.
const fn manager_slot(key: (u32, u32)) -> u32 {
    let keys = ssb_rom::effect::MANAGER_EFFECT_KEYS;
    let mut i = 0;
    while i < keys.len() {
        if keys[i].0 == key.0 && keys[i].1 == key.1 {
            return i as u32;
        }
        i += 1;
    }
    panic!("not a manager effect")
}

const fn manager_part(key: (u32, u32)) -> EntryPart {
    with_anim(key, manager_slot(key))
}

const fn with_anim(key: (u32, u32), slot: u32) -> EntryPart {
    EntryPart {
        key,
        anim_slot: slot,
        root: EntryRoot::Replace,
        turns_left: false,
        end_node: None,
        link: EntryLink::Fixed(10),
        clock: EntryClock::Effect,
        extra_billboards: &[],
    }
}

/// The manager effect at `slot`'s `ssb_rom::effect::MANAGER_EFFECT_KEYS`
/// key, read from the generated table (`ssb_rom::effect::slot`).
const fn effect_key(slot: usize) -> (u32, u32) {
    ssb_rom::effect::MANAGER_EFFECT_KEYS[slot]
}

/// The entry effects' parts, each on its own `ssb_rom::effect` key: Mario's
/// and Luigi's pipe (`llMarioSpecial2EntryDokanDObjDesc`, file 356 for
/// both: `dFTLuigiData` names `llMarioSpecial2FileID`), Donkey Kong's
/// barrel, Samus's capsule, Link's wave and beam, Yoshi's egg, Kirby's star
/// both ways, Fox's Arwing both ways, Captain Falcon's car, the Poké Ball
/// both ways and its rays.
pub const ENTRY_PARTS: [EntryPart; 14] = {
    use ssb_rom::effect::slot;
    [
        manager_part(effect_key(slot::MARIO_ENTRY_DOKAN)),
        manager_part(effect_key(slot::DONKEY_ENTRY_TARU)),
        manager_part(effect_key(slot::SAMUS_ENTRY_POINT)),
        manager_part(effect_key(slot::LINK_ENTRY_WAVE)),
        manager_part(effect_key(slot::LINK_ENTRY_BEAM)),
        manager_part(effect_key(slot::YOSHI_ENTRY_EGG)),
        manager_part(effect_key(slot::KIRBY_ENTRY_STAR)),
        with_anim(
            effect_key(slot::KIRBY_ENTRY_STAR),
            ssb_rom::effect::ENTRY_KIRBY_STAR_R_ANIM_SLOT,
        ),
        EntryPart {
            root: EntryRoot::Outer,
            end_node: Some(0),
            link: EntryLink::SortZNeg(0),
            extra_billboards: &[10],
            ..with_anim(
                effect_key(slot::FOX_ENTRY_ARWING),
                ssb_rom::effect::ENTRY_ARWING_R_ANIM_SLOT,
            )
        },
        EntryPart {
            root: EntryRoot::Outer,
            end_node: Some(0),
            link: EntryLink::SortZNeg(0),
            extra_billboards: &[10],
            ..with_anim(
                effect_key(slot::FOX_ENTRY_ARWING),
                ssb_rom::effect::ENTRY_ARWING_R_ANIM_SLOT + 1,
            )
        },
        EntryPart {
            turns_left: true,
            end_node: Some(1),
            link: EntryLink::SortCar(1),
            extra_billboards: &[3, 5, 7, 9],
            ..with_anim(
                effect_key(slot::CAPTAIN_ENTRY_CAR),
                ssb_rom::effect::ENTRY_CAR_ANIM_SLOT,
            )
        },
        EntryPart {
            link: EntryLink::Ball(1),
            ..manager_part(effect_key(slot::M_BALL_THROWN))
        },
        EntryPart {
            link: EntryLink::Ball(1),
            ..with_anim(
                effect_key(slot::M_BALL_THROWN),
                ssb_rom::effect::ENTRY_BALL_R_ANIM_SLOT,
            )
        },
        EntryPart {
            clock: EntryClock::Rays,
            ..manager_part(effect_key(slot::M_BALL_RAYS))
        },
    ]
};

/// The [`ENTRY_PARTS`] a fighter's entry effect draws: the rightward
/// tables for `lr == +1`, the leftward ones otherwise
/// (`ftCommonAppearSetStatus`).
pub fn entry_effect_parts(f: &ssb_game::fighter::Fighter) -> &'static [usize] {
    use ssb_game::appear::EntryEffect as E;
    use ssb_game::fighter::Facing;
    let Some(e) = ssb_game::appear::entry_effect(f.kind) else {
        return &[];
    };
    let right = f.entry.lr != Some(Facing::Left);
    match e {
        E::Pipe => &[0],
        E::Barrel => &[1],
        E::Point => &[2],
        E::WaveAndBeam => &[3, 4],
        E::Egg => &[5],
        E::Star if right => &[7],
        E::Star => &[6],
        E::Arwing if right => &[8],
        E::Arwing => &[9],
        E::Car => &[10],
        E::PokeBall if right => &[12, 13],
        E::PokeBall => &[11, 13],
    }
}

/// An entry part's object and animation.
pub fn entry_part(pack: &Pack<'_>, part: &EntryPart) -> Option<(ObjectDesc, ssb_rom::pack::AnimDesc)> {
    Some((object_keyed(pack, part.key)?, pack.effect_anim(part.anim_slot)?))
}

/// A manager effect's object and its transform animation.
pub fn manager_effect(pack: &Pack<'_>, key: (u32, u32)) -> Option<(ObjectDesc, ssb_rom::pack::AnimDesc)> {
    let slot = ssb_rom::effect::MANAGER_EFFECT_KEYS.iter().position(|&k| k == key)?;
    Some((object_keyed(pack, key)?, pack.effect_anim(slot as u32)?))
}

/// `dEFManagerDeadExplodeEffectDesc`: file 84's four-node blast tree
/// (`llEFCommonEffects2DeadExplodeDefaultMObjSub`, which the reloc header
/// names for the DObjDesc slot), slot 9 of
/// `ssb_rom::effect::MANAGER_EFFECT_KEYS` (RE-412).
pub const DEAD_EXPLODE_EFFECT_KEY: (u32, u32) = (84, 0x53E8);

/// `dEFManagerDeadExplodeMatAnimJoints[player]`: each player's material
/// script for the blast's three drawn DObjs, as file 84 offsets
/// (`llEFCommonEffects2DeadExplode1MatAnimJoint` at 0x58E0 for player 0,
/// `...2...` at 0x5800, `...3...` at 0x5950 and `...4...` at 0x5870; each
/// table's entries 1–3 name one script for the node's one MObj). The pack
/// binds player 0's.
pub const DEAD_EXPLODE_MAT_SCRIPTS: [[u32; 3]; 4] = [
    [0x58F0, 0x590C, 0x5928],
    [0x5810, 0x582C, 0x5848],
    [0x5960, 0x597C, 0x5998],
    [0x5880, 0x589C, 0x58B8],
];

/// `dEFManagerDeadExplodeEnvColor{Child,Sibling}*[player]`, generated from
/// the ROM (`ssb_game::ko`).
pub use ssb_game::ko::{DEAD_EXPLODE_ENV_CHILD, DEAD_EXPLODE_ENV_SIBLING};

/// `dEFManagerRebirthHaloEffectDesc`: file 85's halo tree
/// (`llEFCommonEffects3RebirthHaloDObjDesc`), slot 12 of
/// `ssb_rom::effect::MANAGER_EFFECT_KEYS`. Its animation spins the ring
/// forever (RE-412).
pub const REBIRTH_HALO_EFFECT_KEY: (u32, u32) = (85, 0x2AC0);

/// `nMPMapObjKindRebirth`: the map point a respawn's halo lowers onto.
pub const MAP_OBJ_KIND_REBIRTH: u16 = 0x20;

/// `dEFManagerShieldEffectDesc`: file 163's shield tree
/// (`llFTManagerCommonShieldDObjDesc`), slot 14 of
/// `ssb_rom::effect::MANAGER_EFFECT_KEYS` (RE-384).
pub const SHIELD_EFFECT_KEY: (u32, u32) = (163, 0x0300);

/// `dEFManagerShieldColors`: `(PRIM, ENV)` RGB per player, and the
/// damaged-shield pair last. `efManagerShieldProcDisplay` sets both with
/// alpha 0xC0. Generated from the ROM (`ssb_game::combat`).
pub use ssb_game::combat::SHIELD_COLORS;

/// `dEFManagerPikachuThunderShockEffectDesc`: file 347's four-node spark
/// tree and its three animations, in `gfx_id` order: the desc's own, then
/// `efManagerPikachuThunderShockMakeEffect`'s two variants
/// (`ssb_rom::effect::PIKACHU_THUNDER_SHOCK_ANIM_SLOT` on).
pub fn pikachu_thunder_shock_effect(
    pack: &Pack<'_>,
) -> Option<(ObjectDesc, [ssb_rom::pack::AnimDesc; 3])> {
    let (object, own) = manager_effect(pack, ssb_rom::effect::PIKACHU_THUNDER_SHOCK_KEY)?;
    let slot = ssb_rom::effect::PIKACHU_THUNDER_SHOCK_ANIM_SLOT;
    Some((
        object,
        [own, pack.effect_anim(slot)?, pack.effect_anim(slot + 1)?],
    ))
}

/// `dEFManagerPurinSingEffectDesc`: file 351's six-node note tree, at slot
/// 32 of `ssb_rom::effect::MANAGER_EFFECT_KEYS`.
pub const PURIN_SING_EFFECT_KEY: (u32, u32) = (351, 0x2130);

/// The Sing notes' object and their manager-effect inventory slot.
pub fn purin_sing_effect(pack: &Pack<'_>) -> Option<(ObjectDesc, u32)> {
    let slot = ssb_rom::effect::MANAGER_EFFECT_KEYS
        .iter()
        .position(|&key| key == PURIN_SING_EFFECT_KEY)?;
    Some((object_keyed(pack, PURIN_SING_EFFECT_KEY)?, slot as u32))
}

/// `dEFManagerCaptainFalconKickEffectDesc`: file 350's two-node flame tree,
/// at slot 29 of `ssb_rom::effect::MANAGER_EFFECT_KEYS`.
pub const CAPTAIN_FALCON_KICK_EFFECT_KEY: (u32, u32) = (350, 0x0B08);

/// The Falcon Kick flame's object and its manager-effect inventory slot.
pub fn captain_falcon_kick_effect(pack: &Pack<'_>) -> Option<(ObjectDesc, u32)> {
    let slot = ssb_rom::effect::MANAGER_EFFECT_KEYS
        .iter()
        .position(|&key| key == CAPTAIN_FALCON_KICK_EFFECT_KEY)?;
    let object = (0..pack.object_count())
        .filter_map(|i| pack.object(i))
        .find(|object| {
            (object.source_file, object.source_offset) == CAPTAIN_FALCON_KICK_EFFECT_KEY
        })?;
    Some((object, slot as u32))
}

/// `dEFManagerKirbyCutter{Draw,Up,Down,Trail}EffectDesc`: file 348's
/// blade, rising and falling swings and trail, in
/// `ssb_game::kirby::CutterEffect` order.
pub const KIRBY_CUTTER_EFFECT_KEYS: [(u32, u32); 4] =
    [(348, 0x2888), (348, 0x12E8), (348, 0x2390), (348, 0x0DF8)];

/// A Final Cutter effect's object and its transform animation; the Draw
/// blade has none (`o_anim_joint` 0).
pub fn kirby_cutter_effect(
    pack: &Pack<'_>,
    key: (u32, u32),
) -> Option<(ObjectDesc, Option<ssb_rom::pack::AnimDesc>)> {
    let slot = ssb_rom::effect::MANAGER_EFFECT_KEYS
        .iter()
        .position(|&k| k == key)?;
    Some((object_keyed(pack, key)?, pack.effect_anim(slot as u32)))
}

/// `dEFManagerSamusGrappleBeamEffectDesc`: file 349's two-node glow tree,
/// at slot 27 of `ssb_rom::effect::MANAGER_EFFECT_KEYS`.
pub const SAMUS_GRAPPLE_BEAM_EFFECT_KEY: (u32, u32) = (349, 0x0380);

/// The Grapple Beam glow's object and its manager-effect inventory slot.
pub fn samus_grapple_beam_effect(pack: &Pack<'_>) -> Option<(ObjectDesc, u32)> {
    let slot = ssb_rom::effect::MANAGER_EFFECT_KEYS
        .iter()
        .position(|&key| key == SAMUS_GRAPPLE_BEAM_EFFECT_KEY)?;
    Some((object_keyed(pack, SAMUS_GRAPPLE_BEAM_EFFECT_KEY)?, slot as u32))
}

/// Fox Special2's three-entry Reflector effect hierarchy.
/// Master Hand's bullets' `WPAttributes.data`: `dBossModel_DObjDescs_0x2CB8`
/// in `BossModel` (file 344).
pub fn boss_bullet_object(pack: &Pack<'_>) -> Option<ObjectDesc> {
    (0..pack.object_count())
        .filter_map(|i| pack.object(i))
        .find(|object| object.source_file == 344 && object.source_offset == 0x2CB8)
}

pub fn fox_reflector_object(pack: &Pack<'_>) -> Option<ObjectDesc> {
    (0..pack.object_count())
        .filter_map(|i| pack.object(i))
        .find(|object| object.source_file == 346 && object.source_offset == 0x2B0)
}

/// `dEFManagerFoxReflectorEffectDesc`: the hexagon's tree and its four
/// tables by `effect_vars.reflector.index`: the `EFDesc`'s own Start, then
/// Loop, Hit and End (`ssb_rom::effect::FOX_REFLECTOR_ANIM_SLOT` on).
pub fn fox_reflector_effect(pack: &Pack<'_>) -> Option<(ObjectDesc, [ssb_rom::pack::AnimDesc; 4])> {
    let (object, start) = manager_effect(pack, ssb_rom::effect::FOX_REFLECTOR_KEY)?;
    let slot = ssb_rom::effect::FOX_REFLECTOR_ANIM_SLOT;
    Some((
        object,
        [
            start,
            pack.effect_anim(slot)?,
            pack.effect_anim(slot + 1)?,
            pack.effect_anim(slot + 2)?,
        ],
    ))
}

/// Walks a stage's floor polylines as the `(line_id, segment)` pairs the
/// collision query consumes.
///
/// A polyline of *n* points is *n-1* segments, so this holds the previous
/// point and emits a segment each time it takes another.
pub struct FloorSegments<'a, 'p> {
    pack: &'a Pack<'p>,
    stage: &'a StageDesc,
    /// Index of the line being walked, within the stage's lines.
    line: u32,
    /// The line itself, once it turned out to be a floor.
    current: Option<LineDesc>,
    /// Index of the next point within the current line.
    point: u16,
    /// The previous point, which is the segment's start.
    prev: Option<(i16, i16, u16, u16)>,
}

/// Walks every static stage collision segment with its authored one-sided
/// kind. Fighters and weapons share this allocation-free static map input.
pub struct StageMap {
    pub platforms: alloc::vec::Vec<ssb_rom::bonus2::Visual>,
    /// The moving groups, shared with the host replays (D-052).
    pub ground: ssb_pose::ground::StageGroups,
}

impl core::ops::Deref for StageMap {
    type Target = ssb_pose::ground::StageGroups;
    fn deref(&self) -> &Self::Target {
        &self.ground
    }
}

impl core::ops::DerefMut for StageMap {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.ground
    }
}

impl StageMap {
    pub fn new(pack: &Pack<'_>, index: u32, stage: &StageDesc) -> Self {
        Self {
            platforms: alloc::vec::Vec::new(),
            ground: ssb_pose::ground::StageGroups::new(pack, index, stage),
        }
    }

    /// Platform children draw under their yakumono's translation on link 6.
    /// Safety: the caller has initialized GU and the current battle matrices.
    pub unsafe fn draw_platforms(&self, pack: &Pack<'_>, base: &psp::sys::ScePspFMatrix4, st: &mut crate::meshdraw::DrawState) {
        use crate::meshdraw;
        use ssb_rom::scene::Mat4;
        #[cfg(feature = "texture_probe")]
        let _cat = meshdraw::probe_cat(st, crate::texprobe::Cat::Stage);
        // The course's empty root still contributes its authored matrix.
        let root = pack.node(self.first_node).map_or(Mat4::IDENTITY, |n| Mat4(n.world));
        for platform in &self.platforms {
            let Some(group) = self.groups.get(platform.group as usize) else { continue };
            if !group.exists() { continue; }
            let pos = group.translate;
            let parent = root.mul(&Mat4::from_trs([pos.x / meshdraw::MODEL_SCALE, pos.y / meshdraw::MODEL_SCALE, pos.z / meshdraw::MODEL_SCALE], [0.0; 3], [1.0; 3]));
            let mut posed = [Mat4::IDENTITY; 8];
            let count = platform.anim.compose(pack, &platform.object, &mut posed);
            for pose in &mut posed[..count] { *pose = parent.mul(pose); }
            meshdraw::draw_object_posed_hiding(pack, &platform.object, base, &posed[..count], st, Some(&platform.materials), &|node| !platform.anim.visible(pack, node));
        }
    }

    pub fn tick(&mut self, pack: &Pack<'_>) -> Result<(), ssb_rom::objanim::AnimError> {
        for platform in &mut self.platforms { platform.tick(pack)?; }
        self.ground.tick(pack)
    }
}

/// A stage's map surfaces in table order, each with its line's group:
/// what [`Decoder`] gives with every group present and still.
type Surfaces = alloc::rc::Rc<[(MapSurface, u16)]>;

/// The surfaces of the last two stages queried, keyed by pack and line
/// range (RE-471). Every map query walks every surface, several times per
/// fighter per tick, and decoding them from the pack each time was most
/// of the map's cost.
static mut SURFACE_CACHE: [Option<((usize, u32, u32), Surfaces)>; 2] = [None, None];

fn stage_surfaces(pack: &Pack<'_>, stage: &StageDesc) -> Surfaces {
    let key = (pack.identity(), stage.first_line, stage.line_count);
    // SAFETY: the game is single-threaded, and no reference into the cache
    // outlives this function: callers hold their own `Rc`.
    let cache = unsafe { &mut *core::ptr::addr_of_mut!(SURFACE_CACHE) };
    if let Some((_, surfaces)) = cache.iter().flatten().find(|(k, _)| *k == key) {
        return surfaces.clone();
    }
    let surfaces: Surfaces = Decoder::new(pack, stage).collect();
    cache[1] = cache[0].take();
    cache[0] = Some((key, surfaces.clone()));
    surfaces
}

/// A stage's map surfaces as the map queries see them: in table order,
/// those of lines whose group exists, each with its group's motion.
pub struct MapSegments<'a> {
    surfaces: Surfaces,
    next: usize,
    groups: &'a [ssb_game::map::MapGroup],
}

impl<'a> MapSegments<'a> {
    pub fn new(pack: &Pack<'_>, stage: &StageDesc) -> Self {
        Self {
            surfaces: stage_surfaces(pack, stage),
            next: 0,
            groups: &[],
        }
    }
    pub fn with_groups(pack: &Pack<'_>, stage: &StageDesc, groups: &'a [ssb_game::map::MapGroup]) -> Self {
        Self {
            groups,
            ..Self::new(pack, stage)
        }
    }
}

impl Iterator for MapSegments<'_> {
    type Item = MapSurface;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(&(surface, yakumono)) = self.surfaces.get(self.next) {
            self.next += 1;
            let group = self.groups.get(usize::from(yakumono));
            if group.is_none_or(|g| g.exists()) {
                return Some(MapSurface {
                    motion: group.and_then(|g| g.motion()),
                    ..surface
                });
            }
        }
        None
    }
}

/// Decodes a stage's surfaces from the pack, each with its line's group.
struct Decoder<'a, 'p> {
    pack: &'a Pack<'p>,
    stage: &'a StageDesc,
    line: u32,
    current: Option<LineDesc>,
    point: u16,
    prev: Option<(i16, i16, u16, u16)>,
}

impl<'a, 'p> Decoder<'a, 'p> {
    fn new(pack: &'a Pack<'p>, stage: &'a StageDesc) -> Self {
        Self {
            pack,
            stage,
            line: 0,
            current: None,
            point: 0,
            prev: None,
        }
    }
}

impl Iterator for Decoder<'_, '_> {
    type Item = (MapSurface, u16);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let Some(line) = self.current else {
                if self.line >= self.stage.line_count {
                    return None;
                }
                let candidate = self.pack.line(self.stage.first_line + self.line);
                self.line += 1;
                if let Some(candidate) = candidate.filter(|line| line.vertex_count >= 2) {
                    self.current = Some(candidate);
                    self.point = 0;
                    self.prev = None;
                }
                continue;
            };

            if self.point >= line.vertex_count {
                self.current = None;
                continue;
            }
            let vertex = self.pack.coll_vertex(line.first_vertex + self.point as u32);
            self.point += 1;
            let Some(vertex) = vertex else {
                self.current = None;
                continue;
            };
            let Some((x1, y1, flags, vertex1)) =
                self.prev
                    .replace((vertex.x, vertex.y, vertex.flags, vertex.vertex_id))
            else {
                continue;
            };
            let kind = match line.kind {
                line_kind::FLOOR => MapSurfaceKind::Floor,
                line_kind::CEILING => MapSurfaceKind::Ceiling,
                line_kind::RIGHT_WALL => MapSurfaceKind::RightWall,
                line_kind::LEFT_WALL => MapSurfaceKind::LeftWall,
                _ => continue,
            };
            return Some((
                MapSurface {
                    motion: None,
                    topology: Some(SurfaceTopology {
                        line: line.id,
                        point: self.point - 2,
                        segments: line.vertex_count - 1,
                        vertex1,
                        vertex2: vertex.vertex_id,
                    }),
                    kind,
                    segment: Segment {
                        x1,
                        y1,
                        x2: vertex.x,
                        y2: vertex.y,
                        flags,
                    },
                },
                line.yakumono,
            ));
        }
    }
}

impl<'a, 'p> FloorSegments<'a, 'p> {
    pub fn new(pack: &'a Pack<'p>, stage: &'a StageDesc) -> Self {
        FloorSegments {
            pack,
            stage,
            line: 0,
            current: None,
            point: 0,
            prev: None,
        }
    }
}

impl Iterator for FloorSegments<'_, '_> {
    type Item = (u16, Segment);

    fn next(&mut self) -> Option<(u16, Segment)> {
        loop {
            let Some(line) = self.current else {
                // Advance to the next floor line, skipping walls and ceilings.
                if self.line >= self.stage.line_count {
                    return None;
                }
                let l = self.pack.line(self.stage.first_line + self.line);
                self.line += 1;
                if let Some(l) = l {
                    if l.kind == line_kind::FLOOR && l.vertex_count >= 2 {
                        self.current = Some(l);
                        self.point = 0;
                        self.prev = None;
                    }
                }
                continue;
            };

            if self.point >= line.vertex_count {
                self.current = None;
                continue;
            }
            let v = self.pack.coll_vertex(line.first_vertex + self.point as u32);
            self.point += 1;
            let Some(v) = v else {
                self.current = None;
                continue;
            };

            match self.prev.replace((v.x, v.y, v.flags, v.vertex_id)) {
                None => continue,
                Some((x1, y1, flags, _)) => {
                    return Some((
                        line.id,
                        Segment {
                            x1,
                            y1,
                            x2: v.x,
                            y2: v.y,
                            // The original reports the flags of the segment's
                            // *first* vertex through `stand_coll_flags`.
                            flags,
                        },
                    ));
                }
            }
        }
    }
}

/// The object a character's animations drive
/// ([`ssb_rom::scene_deps::fighter_object`]), found once when a fighter is
/// created.
pub fn fighter_object(pack: &Pack<'_>, kind: u32) -> Option<u32> {
    ssb_rom::scene_deps::fighter_object(pack, kind)
}

/// The object of a character's low-detail `FTCommonPart`
/// (`ssb_rom::pack::FighterModelDesc`, RE-426), when it has one with the
/// high-detail object's node count.
pub fn fighter_low_object(pack: &Pack<'_>, kind: u32) -> Option<u32> {
    let m = pack.fighter_model(kind)?;
    let (high, low) = (pack.object(m.high)?, pack.object(m.low)?);
    (high.node_count == low.node_count).then_some(m.low)
}

/// The object a fighter at `detail` draws: its low-detail model at
/// `nFTPartsDetailLow` when it has one, else `object`.
pub fn fighter_draw_object(object: u32, object_low: u32, detail: ssb_game::modelpart::Detail) -> u32 {
    if detail == ssb_game::modelpart::Detail::Low && object_low != u32::MAX {
        object_low
    } else {
        object
    }
}

/// The physics constants out of a packed [`FighterDesc`].
///
/// Duplicated from `romtool`'s copy for the same reason [`FloorSegments`] is:
/// `ssb-game` must not know the pack format, and a shared type would drag one
/// crate into the other for the sake of eighteen field copies.
pub fn physics_of(d: &FighterDesc) -> PhysicsAttributes {
    PhysicsAttributes {
        size: d.size,
        traction: d.traction,
        dash_speed: d.dash_speed,
        dash_decel: d.dash_decel,
        run_speed: d.run_speed,
        walk_speed_mul: d.walk_speed_mul,
        jump_vel_x: d.jump_vel_x,
        jump_height_mul: d.jump_height_mul,
        jump_height_base: d.jump_height_base,
        jumpaerial_vel_x: d.jumpaerial_vel_x,
        jumpaerial_height: d.jumpaerial_height,
        air_accel: d.air_accel,
        air_speed_max_x: d.air_speed_max_x,
        air_friction: d.air_friction,
        gravity: d.gravity,
        tvel_base: d.tvel_base,
        tvel_fast: d.tvel_fast,
        jumps_max: d.jumps_max,
        weight: d.weight,
        kneebend_anim_length: d.kneebend_anim_length,
        dash_to_run: d.dash_to_run,
        walkslow_anim_length: d.walkslow_anim_length,
        walkmiddle_anim_length: d.walkmiddle_anim_length,
        walkfast_anim_length: d.walkfast_anim_length,
    }
}

/// The animation lengths out of a packed [`FighterDesc`].
///
/// These come from the figatree files rather than `FTAttributes`, which is why
/// they are a separate struct rather than more fields on [`physics_of`]'s.
pub fn anim_of(d: &FighterDesc) -> AnimLengths {
    AnimLengths {
        dash: d.dash_anim_length,
        turn: d.turn_anim_length,
        run_brake: d.runbrake_anim_length,
        squat: d.squat_anim_length,
        squat_rv: d.squatrv_anim_length,
        landing: d.landing_anim_length,
        pass: d.pass_anim_length,
    }
}

/// The collision diamond out of a packed [`FighterDesc`].
pub fn body_of(d: &FighterDesc) -> BodyColl {
    BodyColl {
        top: d.coll_top,
        center: d.coll_center,
        bottom: d.coll_bottom,
        width: d.coll_width,
    }
}

/// Resolves one fighter's source floor shadow from the live fighter and the
/// stage map's surfaces where their groups are now
/// ([`ssb_game::shadow::fighter_shadow`]). It does *not* shrink or fade for
/// altitude.
///
/// The portable fighter carries the source `is_invisible` and
/// `is_shadow_hide` gates. Common lifecycle statuses maintain the latter;
/// capture/other visual systems can set it without knowing PSP draw details.
/// Invincibility alone is deliberately not a hide condition — the original
/// keeps its shadow.
pub fn fighter_shadow(
    pack: &Pack<'_>,
    stage: &StageDesc,
    groups: &[ssb_game::map::MapGroup],
    fighter: &Fighter,
    shadow_size: f32,
) -> ssb_game::shadow::ShadowGeometry {
    ssb_game::shadow::fighter_shadow(
        &|| MapSegments::with_groups(pack, stage, groups),
        fighter,
        shadow_size,
    )
}

pub use ssb_pose::{fighter_skeleton, tick_skeleton_animation};

/// The on-device gameplay slice: one fighter, connected end to end from
/// `ssb-rom` `Pack` data through `ssb-game` physics/collision to skeleton
/// animation and the battle camera.
///
/// No opponent, no match rules — the point is that the ported physics and
/// the ported collision run together against real stage data at 60 Hz, which
/// is the thing neither host tests nor a static render can show. Owns only
/// what connects those systems; it does not own menus, Training rules,
/// viewer diagnostics, CPU logic, stocks, or match rules.
pub struct FighterScene {
    /// The fighter, its skeleton and the clip state the host tests share
    /// (`ssb_pose`, D-052). `FighterScene` derefs to it, so `scene.fighter`
    /// and `scene.skeleton` read through.
    pub pose: ssb_pose::PosedFighter,
    /// Whether the fighter found a floor when it was placed.
    pub placed: bool,
    /// Ticks since the fighter last touched the ground, for the overlay.
    pub airborne_ticks: u32,
    /// Jump button last frame, so this frame's tap and release can be derived.
    pub jump_was_held: bool,
    /// Whether the pack supplied this character's real constants. When false
    /// the fighter falls under [`PhysicsAttributes::MARIO`], and the overlay
    /// says so rather than letting a stale pack look like a physics bug.
    pub from_pack: bool,
    /// The low-detail model's object ([`fighter_low_object`], RE-426), or
    /// `u32::MAX`. It is posed by the high-detail object's nodes: the two
    /// trees are the same shape.
    pub object_low: u32,
    /// The real battle camera (RE-131) and the statuses set over it,
    /// ticked alongside the fighter each frame in [`FighterScene::tick`].
    /// Every scene carries one; the battle runs the lead's
    /// ([`ssb_game::matchtick::camera_pass`]).
    pub camera: ssb_game::camera::CameraRig,
    /// `FTAttributes.cam_offset_y` (`refs/ssb-decomp-re/src/ft/fttypes.h`) --
    /// deliberately *not* part of `PhysicsAttributes` (that struct's own doc
    /// comment already carves camera offsets out as belonging to "other
    /// systems"), so it lives here beside the camera that actually uses it,
    /// added to the fighter's own `y` before every [`ssb_game::camera::Camera::tick`]
    /// call, matching `gmCameraUpdateInterests`' own `target_pos.y +=
    /// fp->attr->cam_offset_y`.
    pub cam_offset_y: f32,
    /// Initial `FTStruct.camera_zoom_frame`, copied from the fighter's real
    /// `FTAttributes.camera_zoom` value.
    pub camera_zoom_frame: f32,
    /// `FTAttributes::shadow_size`, preserved separately from physics because
    /// it feeds `ftShadowProcDisplay`, not a gameplay calculation.
    pub shadow_size: f32,
}

impl core::ops::Deref for FighterScene {
    type Target = ssb_pose::PosedFighter;

    fn deref(&self) -> &ssb_pose::PosedFighter {
        &self.pose
    }
}

impl core::ops::DerefMut for FighterScene {
    fn deref_mut(&mut self) -> &mut ssb_pose::PosedFighter {
        &mut self.pose
    }
}

impl FighterScene {
    /// Compose a fighter model, including the extra runtime joint in the
    /// held pose. `CapturePulled`'s first script drives TransN; its model
    /// root script is relative to that transform. The pack stores TransN
    /// as `NO_NODE`, so a plain `Skeleton::compose` leaves the model below
    /// its gameplay root even though floor collision is correct (RE-331).
    pub fn compose_model(
        &self,
        pack: &Pack<'_>,
        object: &ObjectDesc,
        out: &mut [ssb_rom::scene::Mat4],
    ) -> usize {
        // The parts this frame's gameplay queried draw from their collision
        // locals (`transform_update_mode == 1`, RE-509). Joint `n + 4` is
        // node `n`; joint 2 is the lead `XRotN` when the clip interposes it.
        let queried = self.fighter.queried_joints.get();
        let lead = self.skeleton.lead_xrotn_pose().is_some() && queried & 1 << 2 != 0;
        self.skeleton.compose_queried(pack, object, out, queried >> 4, lead)
    }

    /// The fighter's TopN display matrix in world units, the pack's model
    /// scale multiplied in: the base [`Self::compose_model`]'s matrices draw
    /// under. TopN is kind `0x4B` (`lbCommonFighterPartsFuncMatrix`): the
    /// collision local through `syMatrixF2LFixedW` once a query this frame
    /// marked it, otherwise the fixed-point rotate-translate-scale chain
    /// (`lbCommonMatrixTraRotScaInv` in a flagged motion). Its `rotate` is
    /// the catcher's hand for a held fighter
    /// (`ftCommonCapturePulledRotateScale`), its `scale` `attr->size`.
    pub fn topn_display(&self) -> ssb_rom::scene::Mat4 {
        use ssb_rom::scene::Mat4;
        let f = &self.fighter;
        let queried = (f.queried_joints.get() & 1 != 0)
            .then(|| f.topn_query_local.get())
            .flatten();
        let k = ssb_rom::pack::MODEL_SCALE;
        Mat4::fighter_topn_display(
            [f.pos.x, f.pos.y, f.pos.z],
            ssb_game::slope_contour::topn_rotate(f),
            [f.attributes.size; 3],
            self.skeleton.locks.flagged,
            queried,
        )
        .mul(&Mat4::from_trs([0.0; 3], [0.0; 3], [k; 3]))
    }

    /// Puts a fighter of `kind` at a stage's `spawn_index`'th spawn point.
    ///
    /// A spawn sits a few units up (RE-030); as `ftManagerInitFighter` does,
    /// the fighter is stood on the floor below it when that floor is less
    /// than 300 units down (RE-466). Returns a `FighterScene` even when the
    /// stage has no such
    /// spawn, so a caller can say so (`placed`) rather than the view going
    /// blank; a caller that needs to distinguish "no spawn point" from
    /// "spawn point, but nothing to stand on" should check
    /// `pack.spawn(stage, spawn_index)` itself before calling this.
    pub fn at_spawn(
        pack: &Pack<'_>,
        stage: &StageDesc,
        kind: FighterKind,
        spawn_index: u16,
        counts: &ssb_game::manager_counts::ManagerCounts,
    ) -> FighterScene {
        let pos = pack
            .spawn(stage, spawn_index)
            .map(|spawn| ssb_engine::math::Vec3::new(spawn.x as f32, spawn.y as f32, 0.0));
        Self::at_position(pack, stage, kind, spawn_index as u8, pos, counts)
    }

    /// [`FighterScene::at_spawn`] at an explicit position, such as the drop
    /// point `sc1PGameSpawnEnemyTeamNext` picks; `None` leaves the fighter
    /// unplaced. `counts` is the battle's motion and stat counts
    /// ([`ssb_game::manager_counts`]), which the first status draws from.
    pub fn at_position(
        pack: &Pack<'_>,
        stage: &StageDesc,
        kind: FighterKind,
        port: u8,
        pos: Option<ssb_engine::math::Vec3>,
        counts: &ssb_game::manager_counts::ManagerCounts,
    ) -> FighterScene {
        let spawn_index = port;
        // `Fighter::new`'s `port` and `pack.spawn`'s `player` are different
        // concepts that happen to share the same value by this project's own
        // convention (port N spawns at spawn N) -- the same convention the
        // pre-generalization `Play`/`Dummy` split already baked in (ports 0
        // and 1 for spawns 0 and 1).
        let mut fighter = Fighter::new(kind, spawn_index, 3);
        fighter.stats.counts = counts.clone();

        // Real constants if the pack has them: gravity 2.4 and terminal
        // velocity 44 rather than the 0.09 and 1.7 the first port guessed.
        let desc = pack.fighter(kind as u32);
        let from_pack = desc.is_some();
        let mut cam_offset_y = 0.0;
        let mut camera_zoom_frame = 1.0;
        let mut shadow_size = 200.0;
        if let Some(d) = desc {
            fighter.attributes = physics_of(&d);
            fighter.coll = body_of(&d);
            fighter.cliff_reach =
                ssb_engine::math::Vec2::new(d.cliffcatch_width, d.cliffcatch_height);
            fighter.cliff_air_mask = d.cliff_air_mask;
            fighter.anim = anim_of(&d);
            cam_offset_y = d.cam_offset_y;
            camera_zoom_frame = d.camera_zoom;
            fighter.jostle_width = d.jostle_width;
            fighter.jostle_x = d.jostle_x;
            shadow_size = d.shadow_size;
        }

        let mut placed = false;
        if let Some(pos) = pos {
            fighter.pos = pos;
            fighter.facing = ssb_game::fighter::Facing::at_spawn_x(fighter.pos.x);
            fighter.topn_lr = fighter.facing.sign();
            // `ftManagerInitFighter`: stood on the floor below, if near;
            // then `mpCommonSetFighterWaitOrFall` (an entry or a demo sets
            // its own status after).
            placed = fighter.init_floor(FloorSegments::new(pack, stage));
            ssb_game::status::set_wait_or_fall(&mut fighter);
        }
        // `MPGroundData`'s `map_bound_*` and `camera_bound_*`, and the
        // `nMPMapObjKindRebirth` point the halo lowers the fighter onto.
        {
            let zone = |e: &ssb_rom::pack::Extent| ssb_game::status::BlastZone {
                top: f32::from(e.top),
                bottom: f32::from(e.bottom),
                left: f32::from(e.left),
                right: f32::from(e.right),
            };
            let rebirth = pack
                .stage_points(stage)
                .find(|p| p.kind == MAP_OBJ_KIND_REBIRTH)
                .map_or(ssb_engine::math::Vec2::new(0.0, 0.0), |p| {
                    ssb_engine::math::Vec2::new(f32::from(p.x), f32::from(p.y))
                });
            fighter.dead.bounds = Some(ssb_game::dead::StageBounds {
                map: zone(&stage.bounds),
                camera: zone(&stage.camera),
                rebirth,
                fog_color: stage.fog_color,
            });
        }
        // Every other field before the pose, so `PosedFighter::new` is the
        // literal's only call and builds into the scene's slot: no 37 KB
        // temporary on the game stack (D-052).
        let object_low = fighter_low_object(pack, kind as u32).unwrap_or(u32::MAX);
        // Matches real hardware's own `dGMCameraCObjVecDefault` rest
        // state (RE-131) rather than starting already converged on the
        // fighter -- the pan-in from that rest state as the match
        // begins is real, observable behaviour, not a startup glitch to
        // hide.
        let camera = ssb_game::camera::CameraRig::default();
        FighterScene {
            pose: ssb_pose::PosedFighter::new(pack, fighter),
            placed,
            airborne_ticks: 0,
            jump_was_held: false,
            from_pack,
            object_low,
            camera,
            cam_offset_y,
            camera_zoom_frame,
            shadow_size,
        }
    }

    /// Advances fighter physics/collision and skeleton animation one tick,
    /// without touching the battle camera.
    ///
    /// `input` is the mapped N64 pad; `jump_held` is the jump button's state
    /// this frame, from which the tap and release edges are derived. The
    /// status machine wants edges rather than levels because a short hop is
    /// defined by the button coming back *up* inside the jumpsquat.
    ///
    /// Used directly by scenes with no camera of their own (e.g. a
    /// stationary training target); [`FighterScene::tick`] calls this then
    /// also advances the camera, for scenes that own one.
    pub fn tick_fighter(
        &mut self,
        pack: &Pack<'_>,
        stage: &StageDesc,
        input: ssb_engine::input::ControllerState,
        jump_held: bool,
    ) {
        self.tick_fighter_map(pack, stage, input, jump_held, &[]);
    }

    pub fn tick_fighter_map(
        &mut self,
        pack: &Pack<'_>,
        stage: &StageDesc,
        input: ssb_engine::input::ControllerState,
        jump_held: bool,
        groups: &[ssb_game::map::MapGroup],
    ) {
        self.tick_fighter_interrupt(pack, stage, input, jump_held, groups);
        self.tick_fighter_physics(pack, stage, groups);
    }

    /// The priority-5 half of [`Self::tick_fighter_map`]: input, runtime
    /// samples and `ftMainProcUpdateInterrupt`. A match runs this for every
    /// fighter, then the stage controller, then [`Self::tick_fighter_physics`].
    pub fn tick_fighter_interrupt(
        &mut self,
        pack: &Pack<'_>,
        stage: &StageDesc,
        input: ssb_engine::input::ControllerState,
        jump_held: bool,
        groups: &[ssb_game::map::MapGroup],
    ) {
        crate::stack_poison::poison();
        let tapped = jump_held && !self.jump_was_held;
        let released = !jump_held && self.jump_was_held;
        self.jump_was_held = jump_held;
        self.pose.tick_interrupt(
            pack,
            Some(stage.bounds.top as f32),
            input,
            tapped,
            released,
            &|| MapSegments::with_groups(pack, stage, groups),
        );
    }

    /// The priority-4 half: `ftMainProcPhysicsMap`, then the animation and
    /// the joint samples gameplay reads.
    pub fn tick_fighter_physics(
        &mut self,
        pack: &Pack<'_>,
        stage: &StageDesc,
        groups: &[ssb_game::map::MapGroup],
    ) {
        crate::stack_poison::poison();
        let segments = || MapSegments::with_groups(pack, stage, groups);
        let mut hooks = SceneHooks {
            airborne_ticks: &mut self.airborne_ticks,
        };
        self.pose.tick_physics_with(pack, &segments, &mut hooks);
    }

    /// What the battle camera reads of this fighter besides the fighter.
    pub fn framing(&self) -> ssb_game::camera::Framing {
        ssb_game::camera::Framing {
            cam_offset_y: self.cam_offset_y,
            zoom_frame: self.camera_zoom_frame,
        }
    }

    /// Advances the battle camera one tick ([`ssb_game::camera::CameraRig::tick`]),
    /// framing the fighter and `others` (the other fighters'
    /// [`ssb_game::camera::interest_of`]s, in link order); any past the
    /// fourth interest are ignored. `entry` is the rig's `entry_zoom`
    /// resolved on the zoomed fighter as its process left it this tick.
    pub fn tick_camera(
        &mut self,
        stage: &StageDesc,
        others: &[ssb_game::camera::Interest],
        entry: Option<ssb_game::camera::EntryFocusTarget>,
    ) {
        let stage = camera_stage(stage);
        let mut list = [ssb_game::camera::Interest::default(); 4];
        let mut count = 0;
        let own = ssb_game::camera::interest_of(&self.fighter, self.framing(), &stage);
        for interest in own
            .into_iter()
            .chain(others.iter().copied())
            .take(list.len())
        {
            list[count] = interest;
            count += 1;
        }
        let pos = self.fighter.pos;
        self.camera
            .tick(pos, self.cam_offset_y, &list[..count], entry, &stage);
    }

    /// Advances one tick against the stage: fighter physics/collision and
    /// skeleton animation ([`FighterScene::tick_fighter`]), then the battle
    /// camera ([`FighterScene::tick_camera`]). Convenience wrapper for
    /// scenes that own a camera; a scene that does not (e.g. a stationary
    /// training target) calls `tick_fighter` directly instead.
    pub fn tick(
        &mut self,
        pack: &Pack<'_>,
        stage: &StageDesc,
        input: ssb_engine::input::ControllerState,
        jump_held: bool,
        additional_camera_interest: Option<ssb_game::camera::Interest>,
    ) {
        self.tick_fighter(pack, stage, input, jump_held);
        // The entry focus only runs in the battle, whose camera pass
        // resolves it.
        self.tick_camera(stage, additional_camera_interest.as_slice(), None);
    }

    /// [`ssb_pose::PosedFighter::settle_catch`] on this stage's map.
    /// Call for every fighter before [`Self::settle_capture`].
    pub fn settle_catch(
        &mut self,
        pack: &Pack<'_>,
        stage: &StageDesc,
        groups: &[ssb_game::map::MapGroup],
    ) {
        let segments = || MapSegments::with_groups(pack, stage, groups);
        self.pose.settle_catch(pack, &segments);
    }

    /// [`ssb_pose::PosedFighter::settle_capture`] on this stage's map.
    /// Call after [`Self::settle_catch`] and the `grab::exchange` that
    /// delivers the capture and the hand.
    pub fn settle_capture(
        &mut self,
        pack: &Pack<'_>,
        stage: &StageDesc,
        groups: &[ssb_game::map::MapGroup],
    ) {
        let segments = || MapSegments::with_groups(pack, stage, groups);
        self.pose.settle_capture(pack, &segments);
    }
}

/// What the PSP adds to the physics half: the profiler's spans and the
/// overlay's airborne count.
struct SceneHooks<'a> {
    airborne_ticks: &'a mut u32,
}

impl ssb_pose::PhysicsHooks for SceneHooks<'_> {
    #[inline(always)]
    fn span<R>(&mut self, span: ssb_pose::PhysicsSpan, f: impl FnOnce() -> R) -> R {
        let span = match span {
            ssb_pose::PhysicsSpan::Anim => crate::profile::Span::Anim,
            ssb_pose::PhysicsSpan::Map => crate::profile::Span::Map,
            ssb_pose::PhysicsSpan::Joints => crate::profile::Span::Joints,
        };
        let t = crate::profile::start();
        let r = f();
        crate::profile::stop(span, t);
        r
    }

    #[inline(always)]
    fn after_map(&mut self, fighter: &Fighter) {
        if fighter.is_grounded() {
            *self.airborne_ticks = 0;
        } else {
            *self.airborne_ticks = self.airborne_ticks.saturating_add(1);
        }
    }
}

pub use ssb_pose::sample_demo_joints;

/// Which way to turn a model so it faces the way the fighter does.
///
/// Fighter models are authored facing `+Z` — shoulders spanning X — while a
/// match runs along X, so every one of them is a quarter turn off (RE-038).
/// Feeds `Gpu::model_transform`'s `rot_radians` argument.
pub fn facing_turn(facing: ssb_game::fighter::Facing) -> f32 {
    match facing {
        ssb_game::fighter::Facing::Right => core::f32::consts::FRAC_PI_2,
        ssb_game::fighter::Facing::Left => -core::f32::consts::FRAC_PI_2,
    }
}

/// What the battle camera reads of `stage` ([`ssb_game::camera::CameraStage`]).
pub fn camera_stage(stage: &StageDesc) -> ssb_game::camera::CameraStage {
    use ssb_game::camera::{BonusFollow, Bounds, StageCamera};
    let bounds = |e: &ssb_rom::pack::Extent| Bounds {
        top: f32::from(e.top),
        bottom: f32::from(e.bottom),
        left: f32::from(e.left),
        right: f32::from(e.right),
    };
    ssb_game::camera::CameraStage {
        camera: bounds(&stage.camera),
        team_camera: bounds(&stage.team_camera),
        map_top: f32::from(stage.bounds.top),
        light_angle_z: stage.camera_light_angle_z,
        // `gGMCameraStruct.status_default` by `gkind`
        // (`gmCameraMakeDefaultCamera`).
        stage_camera: match stage.source_file {
            ssb_rom::ground_obj::ZEBES_FILE => StageCamera::Zebes,
            ssb_rom::ground_obj::INISHIE_FILE => StageCamera::Inishie,
            _ => StageCamera::Default,
        },
        follow: if stage.source_file == ssb_rom::ground_obj::BONUS3_FILE {
            BonusFollow::Race
        } else if ssb_rom::bonus2::kind(stage.source_file).is_some() {
            BonusFollow::Platforms
        } else {
            BonusFollow::Other
        },
    }
}

/// A baked boss camera animation (`ssb_rom::campaign::camera_frames`).
#[derive(Clone)]
pub struct CameraAnim {
    pub frames: alloc::vec::Vec<[f32; ssb_rom::campaign::CAMERA_FRAME_FLOATS]>,
}

impl CameraAnim {
    pub fn load(pack: &Pack<'_>, slot: u32) -> Option<Self> {
        let frames: alloc::vec::Vec<_> = ssb_rom::campaign::packed_camera_frames(pack, slot)?.collect();
        (!frames.is_empty()).then_some(CameraAnim { frames })
    }
}

/// `gmCameraSetStatusAnim`: the camera takes the animation's first play,
/// offset by `vel` (`gGMCameraStruct.vel_all`).
pub fn start_camera_anim(scene: &mut FighterScene, anim: &CameraAnim, vel: ssb_engine::math::Vec3) {
    scene.camera.status = Some(CameraStatus::Anim {
        frame: 0,
        vel,
        ended: false,
    });
    play_camera_anim(scene, anim);
}

/// `gmCameraUpdateAnimVel` (`gcPlayCamAnim`, then `vel_all`): one play.
/// Returns whether the animation was playing.
pub fn play_camera_anim(scene: &mut FighterScene, anim: &CameraAnim) -> bool {
    let Some(CameraStatus::Anim { frame, vel, .. }) = scene.camera.status else {
        return false;
    };
    let last = anim.frames.len().saturating_sub(1);
    let f = anim.frames[frame.min(last)];
    scene.camera.eye = ssb_engine::math::Vec3::new(f[0], f[1], f[2]) + vel;
    scene.camera.at = ssb_engine::math::Vec3::new(f[3], f[4], f[5]) + vel;
    scene.camera.fovy_degrees = f[6];
    scene.camera.status = Some(CameraStatus::Anim {
        frame: frame + 1,
        vel,
        ended: frame >= last,
    });
    true
}

/// A fighter's model yaw: [`facing_turn`], or none at all while it enters
/// (`lr = 0` faces the camera; Captain Falcon's leftward entry turns
/// around), `ssb_game::appear::model_yaw`.
pub fn fighter_turn(f: &ssb_game::fighter::Fighter) -> f32 {
    ssb_game::slope_contour::topn_yaw(f)
}

/// The file data `grMainSetupMakeGround` hands a stage's controller:
/// its kind (`dMPCollisionGroundFileInfos` row), map objects, bottom bound
/// and hazard descriptors.
pub struct StageSetup {
    pub kind: ssb_game::stage::StageKind,
    pub map_objects: alloc::vec::Vec<ssb_game::stage::MapObject>,
    pub bound_bottom: f32,
    pub hazard: [i32; 7],
    pub hazard_surface_y: f32,
    pub bonus3_bumpers: alloc::vec::Vec<ssb_game::stage::bonus3::BumperDesc>,
}

/// The pack's stage whose `GR*Map` file is VS stage `gkind`
/// (`ssb_rom::stage::VS_GROUND_FILES`).
pub fn vs_stage_index(pack: &Pack<'_>, gkind: u8) -> Option<u32> {
    (0..pack.stage_count()).find(|&i| {
        pack.stage(i)
            .is_some_and(|s| ssb_rom::stage::vs_ground_kind(s.source_file) == Some(gkind))
    })
}

/// The pack's stage whose `GR*Map` file is common stage `gkind`
/// (`ssb_rom::stage::COMMON_GROUND_FILES`): the VS stages and the 1P
/// Game's.
pub fn common_stage_index(pack: &Pack<'_>, gkind: u8) -> Option<u32> {
    if (29..41).contains(&gkind) { return pack.stage_of_file(283 + u32::from(gkind - 29)); }
    if (17..29).contains(&gkind) {
        return pack.stage_of_file(271 + u32::from(gkind - 17));
    }
    let file = *ssb_rom::stage::COMMON_GROUND_FILES.get(usize::from(gkind))?;
    pack.stage_of_file(file)
}

impl StageSetup {
    /// VS stages and Race to the Finish, whose header names file 162's
    /// Bumper descriptors. The host sets `StageInit::player` on entry.
    pub fn new(pack: &Pack<'_>, stage: &StageDesc) -> Option<Self> {
        let gkind = if stage.source_file == ssb_rom::ground_obj::BONUS3_FILE {
            15
        } else {
            ssb_rom::stage::vs_ground_kind(stage.source_file)?
        };
        Some(StageSetup {
            kind: ssb_game::stage::StageKind::from_gkind(gkind)?,
            map_objects: pack
                .stage_points(stage)
                .map(|point| ssb_game::stage::MapObject {
                    kind: point.kind,
                    pos: ssb_engine::math::Vec3::new(point.x as f32, point.y as f32, 0.0),
                })
                .collect(),
            bound_bottom: stage.bounds.bottom as f32,
            hazard: stage.hazard,
            hazard_surface_y: stage.hazard_surface_y,
            bonus3_bumpers: if gkind == 15 {
                let object = (0..pack.object_count())
                    .filter_map(|i| pack.object(i))
                    .find(|o| {
                        o.source_file == ssb_rom::ground_obj::BONUS3_NODES_FILE
                            && o.source_offset == 0
                    })?;
                (1..=4)
                    .map(|i| {
                        let node = pack.node(object.first_node + i)?;
                        Some(ssb_game::stage::bonus3::BumperDesc {
                            translate: ssb_engine::math::Vec3::new(
                                node.rest_translate[0],
                                node.rest_translate[1],
                                node.rest_translate[2],
                            ),
                            animated: true,
                        })
                    })
                    .collect::<Option<alloc::vec::Vec<_>>>()?
            } else {
                alloc::vec::Vec::new()
            },
        })
    }

    pub fn init(&self) -> ssb_game::stage::StageInit<'_> {
        let (hazard_attack, hazard_throw) = self.kind.hazard_descs(self.hazard);
        ssb_game::stage::StageInit {
            kind: self.kind,
            map_objects: &self.map_objects,
            bound_bottom: self.bound_bottom,
            hazard_attack,
            hazard_throw,
            acid_surface_y: self.hazard_surface_y,
            bonus3_bumpers: &self.bonus3_bumpers,
            player: 0,
        }
    }
}

/// The runtime half of the stage controller's objects (RE-357): the pack's
/// controller objects, played through [`ssb_rom::ground_obj::GroundObjects`].
///
/// Objects or clips the pack lacks behave as `NoObjects` does: nothing
/// animates and every clock reads 0.
pub struct StageObjectsPort<'a, 'p> {
    pub pack: &'a Pack<'p>,
    pub objects: &'a mut ssb_rom::ground_obj::GroundObjects,
}

/// The runtime half of the stage items' trees ([`ssb_game::item::ItemAnims`]):
/// the POW Block and the Piranha Plants are ground objects of Mushroom
/// Kingdom ([`ssb_rom::ground_obj::POWER_BLOCK`], [`ssb_rom::ground_obj::PAKKUN`]),
/// played by their items rather than by [`ssb_rom::ground_obj::GroundObjects::advance`].
/// The same port as [`StageObjectsPort`], so one value serves both traits
/// ([`ssb_game::matchtick::Arena::Props`]).
pub type ItemAnimsPort<'a, 'p> = StageObjectsPort<'a, 'p>;

/// The ground object and instance an item's tree is.
fn item_tree(target: ssb_game::item::ItemAnimTarget, objects: &ssb_rom::ground_obj::GroundObjects) -> Option<(u8, u8)> {
    use ssb_game::item::ItemAnimTarget;
    use ssb_rom::ground_obj as g;
    match target {
        ItemAnimTarget::PowerBlock => Some((g::POWER_BLOCK, 0)),
        ItemAnimTarget::Pakkun(i) => Some((g::PAKKUN, i)),
        ItemAnimTarget::Monster(k) => Some((g::MONSTER_FIRST + k as u8, 0)),
        ItemAnimTarget::Bonus3Bumper(i) => Some((g::BONUS3_BUMPER_FIRST + i, 0)),
        ItemAnimTarget::Bonus2Bumper(i) => objects.iter().find(|o| o.asset >= ssb_rom::bonus2::FIRST_BUMPER_ASSET).map(|o| (o.asset, i)),
        ItemAnimTarget::Untracked => None,
        ItemAnimTarget::Target(i) => objects.iter()
            .find(|o| (ssb_rom::bonus1::FIRST_ASSET..ssb_rom::bonus2::FIRST_BUMPER_ASSET).contains(&o.asset)).map(|o| (o.asset, i)),
    }
}

fn root_write(w: ssb_rom::ground_obj::RootWrite) -> ssb_game::item::RootWrite {
    ssb_game::item::RootWrite { translate: w }
}

impl ssb_game::item::ItemAnims for ItemAnimsPort<'_, '_> {
    fn root_frame(&self, target: ssb_game::item::ItemAnimTarget) -> f32 {
        item_tree(target, self.objects).map_or(0.0, |(a, i)| self.objects.item_root_frame(a, i))
    }
    fn make(&mut self, target: ssb_game::item::ItemAnimTarget) {
        if let Some((asset, instance)) = item_tree(target, self.objects) {
            self.objects.item_make(self.pack, asset, instance);
        }
    }
    fn play(&mut self, target: ssb_game::item::ItemAnimTarget) -> ssb_game::item::RootWrite {
        let Some((asset, instance)) = item_tree(target, self.objects) else {
            return ssb_game::item::RootWrite::default();
        };
        root_write(self.objects.item_play(self.pack, asset, instance))
    }
    fn add_play(
        &mut self,
        target: ssb_game::item::ItemAnimTarget,
        anim: ssb_game::item::ItemAnim,
    ) -> ssb_game::item::RootWrite {
        use ssb_game::item::ItemAnim;
        use ssb_rom::ground_obj as g;
        let Some((asset, instance)) = item_tree(target, self.objects) else {
            return ssb_game::item::RootWrite::default();
        };
        let (joint, mat) = match anim {
            ItemAnim::PowerBlockDamage => (Some(g::POWER_BLOCK_DAMAGE), None),
            ItemAnim::PakkunAppear => (Some(g::PAKKUN_APPEAR), Some(g::PAKKUN_APPEAR_MAT)),
            ItemAnim::PakkunDamaged => (None, Some(g::PAKKUN_DAMAGED_MAT)),
        };
        root_write(
            self.objects
                .item_add_play(self.pack, joint, mat, asset, instance),
        )
    }
    fn root_idle(&self, target: ssb_game::item::ItemAnimTarget) -> bool {
        item_tree(target, self.objects)
            .is_none_or(|(asset, instance)| self.objects.item_root_idle(asset, instance))
    }
    fn stop_root(&mut self, target: ssb_game::item::ItemAnimTarget) {
        if let Some((asset, instance)) = item_tree(target, self.objects) {
            self.objects.item_stop_root(asset, instance);
        }
    }
    fn stop_material(&mut self, target: ssb_game::item::ItemAnimTarget) {
        if let Some((asset, instance)) = item_tree(target, self.objects) {
            self.objects.item_stop_material(asset, instance);
        }
    }
}

/// Shows the live stage items' trees where their items stand, and hides
/// the rest. Call after the frame's item processes, before drawing.
pub fn place_item_trees(
    objects: &mut ssb_rom::ground_obj::GroundObjects,
    items: &ssb_game::item::ItemPool,
) {
    objects.hide_items();
    for item in items.items().filter(|i| !i.hidden) {
        if let Some((asset, instance)) = item_tree(item.anim_target(), objects) {
            if let Some(o) = objects.instance_mut(asset, instance) {
                o.texture = item.texture;
            }
            objects.item_place(
                asset,
                instance,
                [item.pos.x, item.pos.y, item.pos.z],
                item.rotate_z,
            );
        }
    }
}

/// The [`ssb_rom::ground_obj::ANIMS`] index of a controller's animation.
pub fn ground_anim(anim: ssb_game::stage::StageAnim) -> Option<usize> {
    use ssb_game::stage::pupupu::{EyesAnim, MouthAnim};
    use ssb_game::stage::StageAnim;
    use ssb_rom::ground_obj as g;
    let lr = |lr: u8| (lr <= 1).then_some(lr);
    let phase = |phase: u8| (phase <= 2).then_some(phase);
    Some(match anim {
        StageAnim::WhispyEyes { lr: l, status } => {
            g::whispy_eyes(lr(l)?, status == EyesAnim::Blink)
        }
        StageAnim::WhispyMouth { lr: l, status } => g::whispy_mouth(
            lr(l)?,
            match status {
                MouthAnim::Stretch => 0,
                MouthAnim::Turn => 1,
                MouthAnim::Open => 2,
                MouthAnim::Close => 3,
            },
        ),
        StageAnim::FlowersBack { lr: l, phase: p } => g::flowers_back(lr(l)?, phase(p)?),
        StageAnim::FlowersFront { lr: l, phase: p } => g::flowers_front(lr(l)?, phase(p)?),
        StageAnim::TaruCannDefault => g::TARUCANN_DEFAULT,
        StageAnim::TaruCannFill => g::TARUCANN_FILL,
        StageAnim::TaruCannShoot => g::TARUCANN_SHOOT,
        StageAnim::GateOpen => g::GATE_OPEN,
        StageAnim::GateClose => g::GATE_CLOSE,
        StageAnim::Acid => g::ACID_ANIM,
        StageAnim::ScaleRetract(_) => g::SCALE_RETRACT,
        StageAnim::CastleGround => g::CASTLE_GROUND_ANIM,
        _ => return None,
    })
}

/// The [`ssb_rom::ground_obj::OBJECTS`] index and instance of a
/// controller's object.
pub fn ground_object(obj: ssb_game::stage::StageObj) -> Option<(u8, u8)> {
    use ssb_game::stage::StageObj;
    use ssb_rom::ground_obj as g;
    Some(match obj {
        StageObj::WhispyEyes => (g::WHISPY_EYES, 0),
        StageObj::WhispyMouth => (g::WHISPY_MOUTH, 0),
        StageObj::FlowersBack => (g::FLOWERS_BACK, 0),
        StageObj::FlowersFront => (g::FLOWERS_FRONT, 0),
        StageObj::TaruCann => (g::TARUCANN, 0),
        StageObj::Gate => (g::GATE, 0),
        StageObj::Acid => (g::ACID, 0),
        StageObj::Cloud(i) => (g::CLOUD, i),
        StageObj::Scale(i) => (g::SCALE_PLATFORM, i),
        StageObj::ScaleStrings => (g::SCALE_STRINGS, 0),
        StageObj::CastleGround => (g::CASTLE_GROUND, 0),
    })
}

impl StageObjectsPort<'_, '_> {
    fn get(&self, obj: ssb_game::stage::StageObj) -> Option<&ssb_rom::ground_obj::GroundObject> {
        let (asset, instance) = ground_object(obj)?;
        self.objects.instance(asset, instance)
    }

    fn get_mut(
        &mut self,
        obj: ssb_game::stage::StageObj,
    ) -> Option<&mut ssb_rom::ground_obj::GroundObject> {
        let (asset, instance) = ground_object(obj)?;
        self.objects.instance_mut(asset, instance)
    }
}

fn vec3([x, y, z]: [f32; 3]) -> ssb_engine::math::Vec3 {
    ssb_engine::math::Vec3::new(x, y, z)
}

/// The pack script a controller's Arwing animation names.
fn arwing_script(anim: ssb_game::stage::sector::ArwingAnim) -> ssb_rom::sector::Script {
    use ssb_game::stage::sector::ArwingAnim as A;
    use ssb_rom::sector::Script as S;
    match anim {
        A::Flight { pattern, field } => S::Flight { pattern, field },
        A::Pilot(id) => S::Pilot(id),
        A::LaserCharge => S::LaserCharge,
        A::LaserFire => S::LaserFire,
        A::Flare => S::Flare,
        A::Glow => S::Glow,
    }
}

/// Sector Z's Arwing through the same port (RE-428). Every call on a stage
/// without one is a no-op; `arwing()` never hands it out there.
impl ssb_game::stage::sector::ArwingObject for StageObjectsPort<'_, '_> {
    fn add_anim(&mut self, node: u8, anim: Option<ssb_game::stage::sector::ArwingAnim>) {
        if let Some(a) = self.objects.arwing.as_mut() {
            // A script that fails to parse leaves the node where it is.
            let _ = a.add_anim(self.pack, node as usize, anim.map(arwing_script));
        }
    }
    fn add_anim_joint(&mut self, node: u8, anim: ssb_game::stage::sector::ArwingAnim) {
        if let Some(a) = self.objects.arwing.as_mut() {
            a.add_anim_joint(self.pack, node as usize, arwing_script(anim));
        }
    }
    fn play_all(&mut self) {
        if let Some(a) = self.objects.arwing.as_mut() {
            let _ = a.play_all(self.pack);
        }
    }
    fn anim_null(&self, node: u8) -> bool {
        self.objects
            .arwing
            .as_ref()
            .is_none_or(|a| a.anim_null(node as usize))
    }
    fn stop(&mut self, node: u8) {
        if let Some(a) = self.objects.arwing.as_mut() {
            a.stop(node as usize);
        }
    }
    fn flags(&self, node: u8) -> u16 {
        self.objects.arwing.as_ref().map_or(0, |a| a.flags(node as usize))
    }
    fn set_flags(&mut self, node: u8, flags: u16) {
        if let Some(a) = self.objects.arwing.as_mut() {
            a.set_flags(node as usize, flags);
        }
    }
    fn set_hidden(&mut self, hidden: bool) {
        if let Some(a) = self.objects.arwing.as_mut() {
            a.hidden = hidden;
        }
    }
    fn translate(&self, node: u8) -> ssb_engine::math::Vec3 {
        self.objects
            .arwing
            .as_ref()
            .map_or(ssb_engine::math::Vec3::ZERO, |a| {
                vec3(a.translate(node as usize))
            })
    }
    fn set_translate(&mut self, node: u8, t: ssb_engine::math::Vec3) {
        if let Some(a) = self.objects.arwing.as_mut() {
            a.set_translate(node as usize, [t.x, t.y, t.z]);
        }
    }
    fn rotate(&self, node: u8) -> ssb_engine::math::Vec3 {
        self.objects
            .arwing
            .as_ref()
            .map_or(ssb_engine::math::Vec3::ZERO, |a| vec3(a.rotate(node as usize)))
    }
    fn set_rotate(&mut self, node: u8, r: ssb_engine::math::Vec3) {
        if let Some(a) = self.objects.arwing.as_mut() {
            a.set_rotate(node as usize, [r.x, r.y, r.z]);
        }
    }
    fn path_fraction(&self, node: u8) -> Option<f32> {
        self.objects.arwing.as_ref()?.path_fraction(node as usize)
    }
    fn path_tangent(&self, node: u8, t: f32) -> Option<ssb_engine::math::Vec3> {
        self.objects
            .arwing
            .as_ref()?
            .path_tangent(self.pack, node as usize, t)
            .map(vec3)
    }
    fn path_point(&self, node: u8, t: f32) -> Option<ssb_engine::math::Vec3> {
        self.objects
            .arwing
            .as_ref()?
            .path_point(self.pack, node as usize, t)
            .map(vec3)
    }
    fn set_root(&mut self, m: [[f32; 4]; 4]) {
        if let Some(a) = self.objects.arwing.as_mut() {
            a.root = ssb_rom::scene::Mat4(core::array::from_fn(|i| m[i / 4][i % 4]));
        }
    }
}

impl ssb_game::stage::StageObjects for StageObjectsPort<'_, '_> {
    fn play(&mut self, anim: ssb_game::stage::StageAnim) {
        use ssb_game::stage::StageAnim;
        use ssb_rom::ground_obj as g;
        match anim {
            StageAnim::CloudSolid(i) => {
                self.objects
                    .play_cloud(self.pack, i as usize, g::CLOUD_SOLID_MAT)
            }
            StageAnim::CloudEvaporate(i) => {
                self.objects
                    .play_cloud(self.pack, i as usize, g::CLOUD_EVAPORATE_MAT)
            }
            _ => {
                let instance = match anim {
                    StageAnim::ScaleRetract(i) => i,
                    _ => 0,
                };
                if let Some(a) = ground_anim(anim) {
                    // A clip that fails to parse leaves the object where it is.
                    let _ = self.objects.play_on(self.pack, a, instance);
                }
            }
        }
    }
    fn stop(&mut self, obj: ssb_game::stage::StageObj) {
        if let Some(o) = self.get_mut(obj) {
            o.stop();
        }
    }
    fn mat_anim_idle(&self, obj: ssb_game::stage::StageObj) -> bool {
        match obj {
            ssb_game::stage::StageObj::Cloud(i) => self.objects.cloud_idle(self.pack, i as usize),
            _ => true,
        }
    }
    fn anim_frame(&self, obj: ssb_game::stage::StageObj) -> f32 {
        self.get(obj).map_or(0.0, |o| o.frame)
    }
    fn translate(&self, obj: ssb_game::stage::StageObj) -> ssb_engine::math::Vec3 {
        self.get(obj)
            .map_or(ssb_engine::math::Vec3::ZERO, |o| vec3(o.translate()))
    }
    fn set_translate_y(&mut self, obj: ssb_game::stage::StageObj, y: f32) {
        if let Some(o) = self.get_mut(obj) {
            o.set_translate_y(y);
        }
    }
    fn set_translate(&mut self, obj: ssb_game::stage::StageObj, pos: ssb_engine::math::Vec3) {
        if let Some(o) = self.get_mut(obj) {
            o.set_translate([pos.x, pos.y, pos.z]);
        }
    }
    fn node_translate(
        &self,
        obj: ssb_game::stage::StageObj,
        node: u8,
    ) -> Option<ssb_engine::math::Vec3> {
        self.get(obj)?.node_translate(node as usize).map(vec3)
    }
    fn set_node_translate_y(&mut self, obj: ssb_game::stage::StageObj, node: u8, y: f32) {
        if let Some(o) = self.get_mut(obj) {
            o.set_node_translate_y(node as usize, y);
        }
    }
    fn child_translate(&self, obj: ssb_game::stage::StageObj) -> Option<ssb_engine::math::Vec3> {
        self.get(obj)?.child_translate(self.pack).map(vec3)
    }
    fn arwing(&mut self) -> Option<&mut dyn ssb_game::stage::sector::ArwingObject> {
        if self.objects.arwing.is_some() {
            Some(self)
        } else {
            None
        }
    }
}
