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

use ssb_game::collision::Segment;
use ssb_game::fighter::{Fighter, FighterKind};
use ssb_game::ground::BodyColl;
use ssb_game::physics::PhysicsAttributes;
use ssb_game::status::AnimLengths;
use ssb_game::status::AnyStatus;
use ssb_game::status::Status;
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
/// The PK Thunder head (file 239 + 0x0C): file 335's two-node tree.
pub const NESS_PK_THUNDER_SOURCE: (u32, u32) = (335, 0x7C98);
/// The PK Thunder trail (file 239 + 0x40): file 335's `DObjDLLink` array,
/// packed as a one-node object.
pub const NESS_PK_TRAIL_SOURCE: (u32, u32) = (335, 0x8B40);

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
    /// `None` for the object's (`efManagerHaveStructProcUpdate`).
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

/// The entry effects' parts, each on its own `ssb_rom::effect` key: Mario's
/// and Luigi's pipe (`llMarioSpecial2EntryDokanDObjDesc`, file 356 for
/// both: `dFTLuigiData` names `llMarioSpecial2FileID`), Donkey Kong's
/// barrel, Samus's capsule, Link's wave and beam, Yoshi's egg, Kirby's star
/// both ways, Fox's Arwing both ways, Captain Falcon's car, the Poké Ball
/// both ways and its rays.
pub const ENTRY_PARTS: [EntryPart; 14] = [
    manager_part((356, 0x0608)),
    manager_part((355, 0x07C8)),
    manager_part((349, 0x0B90)),
    manager_part((353, 0x03F8)),
    manager_part((353, 0x07B8)),
    manager_part((354, 0x0530)),
    manager_part((348, 0x1DA8)),
    with_anim((348, 0x1DA8), ssb_rom::effect::ENTRY_KIRBY_STAR_R_ANIM_SLOT),
    EntryPart {
        root: EntryRoot::Outer,
        end_node: Some(0),
        link: EntryLink::SortZNeg(0),
        extra_billboards: &[10],
        ..with_anim((161, 0x2C30), ssb_rom::effect::ENTRY_ARWING_R_ANIM_SLOT)
    },
    EntryPart {
        root: EntryRoot::Outer,
        end_node: Some(0),
        link: EntryLink::SortZNeg(0),
        extra_billboards: &[10],
        ..with_anim((161, 0x2C30), ssb_rom::effect::ENTRY_ARWING_R_ANIM_SLOT + 1)
    },
    EntryPart {
        turns_left: true,
        end_node: Some(1),
        link: EntryLink::SortCar(1),
        extra_billboards: &[3, 5, 7, 9],
        ..with_anim((350, 0x5FC0), ssb_rom::effect::ENTRY_CAR_ANIM_SLOT)
    },
    EntryPart {
        link: EntryLink::Ball(1),
        ..manager_part((86, 0x9430))
    },
    EntryPart {
        link: EntryLink::Ball(1),
        ..with_anim((86, 0x9430), ssb_rom::effect::ENTRY_BALL_R_ANIM_SLOT)
    },
    EntryPart {
        clock: EntryClock::Rays,
        ..manager_part((85, 0x0628))
    },
];

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

/// `dEFManagerDeadExplodeEnvColorChild*[player]`: ENV of the blast's first
/// drawn DObj (`dobj->child`).
pub const DEAD_EXPLODE_ENV_CHILD: [[u8; 3]; 4] = [
    [0xA6, 0x62, 0x21],
    [0x1F, 0xFF, 0xA1],
    [0x3E, 0x6D, 0xFF],
    [0xFB, 0x66, 0xC7],
];

/// `dEFManagerDeadExplodeEnvColorSibling*[player]`: ENV of the third
/// (`dobj->child->sib_next->sib_next`). The second keeps its display
/// list's own `G_SETENVCOLOR`.
pub const DEAD_EXPLODE_ENV_SIBLING: [[u8; 3]; 4] = [
    [0xFF, 0x62, 0x4B],
    [0x00, 0x7E, 0xFF],
    [0xFF, 0xFF, 0x00],
    [0x00, 0xFF, 0x00],
];

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
/// alpha 0xC0.
pub const SHIELD_COLORS: [([u8; 3], [u8; 3]); 5] = [
    ([0xFF, 0xFF, 0xFF], [0xFF, 0x00, 0x00]),
    ([0xFF, 0xFF, 0xFF], [0x00, 0xFF, 0x00]),
    ([0xFF, 0xFF, 0xFF], [0x00, 0x00, 0xFF]),
    ([0xFF, 0xFF, 0xFF], [0x00, 0x00, 0x00]),
    ([0xFF, 0xFF, 0xFF], [0xC0, 0xC0, 0xC0]),
];

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
    pub animator: ssb_rom::skeleton::StageAnimator,
    pub groups: alloc::vec::Vec<ssb_game::map::MapGroup>,
    anim: Option<ssb_rom::pack::AnimDesc>,
    first_node: u32,
    previous_flags: alloc::vec::Vec<u16>,
}

impl StageMap {
    pub fn new(pack: &Pack<'_>, index: u32, stage: &StageDesc) -> Self {
        let anim = pack.stage_anim(index);
        let mut animator = ssb_rom::skeleton::StageAnimator::new();
        if let Some(anim) = anim {
            animator.start(pack, &anim);
        }
        let first_node = pack
            .object(stage.layers[1])
            .map_or(u32::MAX, |o| o.first_node);
        let count = pack
            .stage_lines(stage)
            .map(|l| l.yakumono as usize + 1)
            .max()
            .unwrap_or(0);
        let mut groups = alloc::vec![ssb_game::map::MapGroup::default(); count];
        for (i, group) in groups.iter_mut().enumerate() {
            if let Some(node) = first_node.checked_add(i as u32).and_then(|n| pack.node(n)) {
                group.translate = ssb_engine::math::Vec3::new(
                    node.rest_translate[0],
                    node.rest_translate[1],
                    node.rest_translate[2],
                );
            }
            group.animated = (0..animator.joint_count()).any(|j| {
                animator
                    .joint(j)
                    .is_some_and(|(n, _)| n == first_node.saturating_add(i as u32))
            });
        }
        Self {
            platforms: alloc::vec::Vec::new(),
            animator,
            groups,
            anim,
            first_node,
            previous_flags: alloc::vec![0; count],
        }
    }

    /// Platform children draw under their yakumono's translation on link 6.
    /// Safety: the caller has initialized GU and the current battle matrices.
    pub unsafe fn draw_platforms(&self, pack: &Pack<'_>, base: &psp::sys::ScePspFMatrix4, st: &mut crate::meshdraw::DrawState) {
        use crate::meshdraw;
        use ssb_rom::scene::Mat4;
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
        let Some(anim) = self.anim else {
            return Ok(());
        };
        let Some(script) = pack.anim_script(&anim) else {
            return Ok(());
        };
        let first = self.first_node;
        for (i, flag) in self.previous_flags.iter_mut().enumerate() {
            *flag = self.animator.flags(first.saturating_add(i as u32));
        }
        let groups = &self.groups;
        self.animator.tick_nodes(script, |node| {
            node.checked_sub(first)
                .and_then(|i| groups.get(i as usize))
                .is_none_or(|g| {
                    !matches!(
                        g.status,
                        ssb_game::map::GroupStatus::On | ssb_game::map::GroupStatus::Off
                    )
                })
        })?;
        for i in 0..self.animator.joint_count() {
            let Some((node, pose)) = self.animator.joint(i) else {
                continue;
            };
            let Some(group) = node
                .checked_sub(first)
                .and_then(|i| self.groups.get_mut(i as usize))
            else {
                continue;
            };
            if !matches!(
                group.status,
                ssb_game::map::GroupStatus::On | ssb_game::map::GroupStatus::Off
            ) {
                let flags = self.animator.flags(node);
                let old_flags = self.previous_flags[(node - first) as usize];
                if old_flags == 0 && flags != 0 {
                    group.status = ssb_game::map::GroupStatus::Hidden;
                } else if old_flags != 0 && flags == 0 {
                    group.status = ssb_game::map::GroupStatus::Show;
                }
                group.set_position(ssb_engine::math::Vec3::new(
                    pose.translate[0],
                    pose.translate[1],
                    pose.translate[2],
                ));
            }
        }
        Ok(())
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
/// stage map.  `ftShadowProcDisplay` uses the standing floor while grounded;
/// otherwise it calls `mpCollisionCheckProjectFloor` from the fighter's root
/// straight down.  It does *not* shrink or fade for altitude.
///
/// The portable fighter carries the source `is_invisible` and
/// `is_shadow_hide` gates. Common lifecycle statuses maintain the latter;
/// capture/other visual systems can set it without knowing PSP draw details.
/// Invincibility alone is deliberately not a hide condition — the original
/// keeps its shadow.
pub fn fighter_shadow(
    pack: &Pack<'_>,
    stage: &StageDesc,
    fighter: &Fighter,
    shadow_size: f32,
) -> ssb_game::shadow::ShadowGeometry {
    if !ssb_game::shadow::visible_for(
        fighter.status.status,
        fighter.is_invisible,
        fighter.is_shadow_hidden,
    ) {
        return ssb_game::shadow::ShadowGeometry::EMPTY;
    }

    let line = fighter.floor.map(|f| f.line).or_else(|| {
        ssb_game::collision::project_floor(
            FloorSegments::new(pack, stage),
            ssb_engine::math::Vec2::new(fighter.pos.x, fighter.pos.y),
        )
        .map(|f| f.line)
    });
    let Some(line) = line else {
        return ssb_game::shadow::ShadowGeometry::EMPTY;
    };
    ssb_game::shadow::build(
        fighter.pos,
        shadow_size,
        FloorSegments::new(pack, stage)
            .filter(move |(candidate, _)| *candidate == line)
            .map(|(_, segment)| segment),
    )
}

/// Advances a fighter's skeleton pose for `status`. Shared by every fighter
/// scene that owns a skeleton: restarted on a status *entry* rather than
/// every tick, since an animation carries its own clock and re-seeding it
/// each frame would freeze every fighter on frame zero. `ftMainSetStatus`
/// restarts the figatree on every entry, even into the same status, and
/// starts it at the entry's `frame_begin`. A looping animation
/// is left to loop; a finite one that has run out holds its last pose, which
/// is what the original does when a status outlives its animation.
pub fn tick_skeleton_animation(
    pack: &Pack<'_>,
    kind: u32,
    first_node: u32,
    status: &ssb_game::status::StatusState,
    speed: f32,
    skeleton: &mut ssb_rom::skeleton::Skeleton,
    started: &mut Option<(AnyStatus, u32)>,
) -> Option<ssb_rom::figatree::JointPose> {
    let entry = status.entry;
    // The clip starts on the status's current frame: `ftMainSetStatus`
    // parses `frame_begin`, and a setter's `ftMainPlayAnimEventsAll` or the
    // frame's own advance has moved it on before this first parse, which
    // poses without advancing (RE-466).
    let frame_begin = status.anim_frame;
    // A setter's `ftMainPlayAnimEventsAll` after `ftMainSetStatus`'s own
    // first parse: the clip stands one play past `anim_frame_begin`.
    let setter_played = status.anim_frame > status.anim_frame_begin;
    let status = status.status;
    // The pack row is the fighter's; the slot is the status's. Common
    // statuses use the shared slots, which resolve per fighter through the
    // status -> motion pairing (`ssb_rom::anim::SLOT_APPEAL`).
    let slot = status.anim_slot() as u32;
    let mut restarted = false;
    if *started != Some((status, entry)) {
        *started = Some((status, entry));
        restarted = !status.keeps_motion();
        // A status with no motion of its own (`CatchWait`, `CaptureWait`)
        // keeps playing the previous one; its slot is that status's slot.
        if status.keeps_motion() {
            // Yoshi Bomb's fall keeps the start clip at speed zero.
            skeleton.speed = speed;
        } else {
            if let Some(anim) = pack.fighter_anim(kind, slot) {
                skeleton.start(
                    pack,
                    &anim,
                    if setter_played { frame_begin - speed } else { frame_begin },
                    speed,
                );
                skeleton.lead_xrotn = u8::try_from(kind)
                    .ok()
                    .and_then(FighterKind::from_ordinal)
                    .is_some_and(|k| ssb_game::motion::leads_with_xrotn(k, status));
            }
        }
    }
    // A rate the status changes mid-clip (`gcSetAnimSpeed`) applies from
    // the next play.
    skeleton.speed = speed;
    // The slot is read back rather than remembered, so a status whose
    // animation the pack lacks -- Kirby has no aerial jump -- simply keeps
    // the pose it had.
    // `ftMainSetStatus` zeroes TransN before the new clip's first parse, so
    // its first step is the clip's own first translation.
    let root_before = if restarted && setter_played {
        // `ftMainSetStatus` parsed `frame_begin` (TransN zeroed first, then
        // posed); the setter's play reads that pose as `anim_vel` and
        // advances, so the first step is from the clip's own first pose
        // (RE-468: Samus's roll starts 23.27 along, not 22.53).
        if let Some(anim) = pack.fighter_anim(kind, slot) {
            if let Some(script) = pack.anim_script(&anim) {
                let _ = skeleton.tick_scaled(
                    script,
                    pack.fighter_translate_scales(kind),
                    first_node,
                );
            }
        }
        skeleton.pose(0).copied()
    } else if restarted {
        skeleton.pose(0).map(|p| ssb_rom::figatree::JointPose {
            translate: [0.0; 3],
            ..*p
        })
    } else {
        skeleton.pose(0).copied()
    };
    if let Some(anim) = pack.fighter_anim(kind, slot) {
        if let Some(script) = pack.anim_script(&anim) {
            let _ = skeleton.tick_scaled(
                script,
                pack.fighter_translate_scales(kind),
                first_node,
            );
        }
    }
    root_before
}

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
    pub fighter: Fighter,
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
    /// The fighter's animation, and the status it was started for.
    ///
    /// Kept here rather than in `ssb-game` because starting one needs the
    /// pack, which Layer A must not know about — the same split the physics
    /// constants use.
    pub skeleton: ssb_rom::skeleton::Skeleton,
    /// Object whose nodes the skeleton drives, or `u32::MAX` when the pack has
    /// no model for this character.
    pub object: u32,
    /// The low-detail model's object ([`fighter_low_object`], RE-426), or
    /// `u32::MAX`. It is posed by the high-detail object's nodes: the two
    /// trees are the same shape.
    pub object_low: u32,
    /// The real battle camera (RE-131), ticked alongside the fighter each
    /// frame in [`FighterScene::tick`].
    pub camera: ssb_game::camera::Camera,
    /// The entry focus's zoom (`ifCommonEntryFocusThread`'s
    /// `gmCameraSetStatusPlayerZoom`, RE-402): a target with its
    /// `cam_offset_y` added and a distance. `None` runs the battle camera.
    pub entry_zoom: Option<(ssb_engine::math::Vec3, f32)>,
    pub bonus_follow: bool,
    /// A camera status other than the battle camera's
    /// (`gmCameraSetStatusMapZoom`, `gmCameraSetStatusAnim`): Master Hand's
    /// background attacks and the boss stage's camera animations.
    pub camera_status: Option<CameraStatus>,
    /// Training's Close-Up view (`sc1PTrainingModeUpdateViewOption`'s
    /// `gmCameraSetStatusPlayerZoom`): the fighter's `closeup_camera_zoom`.
    /// `None` runs the battle camera.
    pub player_zoom: Option<f32>,
    /// The auto demo's focus (`scAutoDemoSetCameraPlayerZoom`), on any
    /// player's fighter: the host refreshes its target before each camera
    /// tick. `None` runs the battle camera.
    pub demo_zoom: Option<DemoZoom>,
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
    started: Option<(AnyStatus, u32)>,
    /// `YRotN`'s local pose, the shield's joint. Only the shield poses move
    /// it (RE-367); the original keeps it between shields too.
    shield_yrotn: ssb_rom::figatree::JointPose,
    /// TransN pose immediately before the last animation parser advance. On
    /// the next fighter tick it pairs with the current hidden pose to recover
    /// the exact `transn - anim_vel` delta the original physics reads.
    root_motion_before_tick: Option<ssb_rom::figatree::JointPose>,
    /// This frame's clip was already played after the interrupt: a throw's
    /// release reads the catcher's hand as `ftMainPlayAnimEventsAll` left it
    /// (RE-472).
    anim_ticked_early: bool,
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
        self.skeleton.compose(pack, object, out)
    }

    /// [`Self::compose_model`] for gameplay positions: the joints' matrices
    /// from `lbCommonSin`, as `gmCollisionGetFighterPartsWorldPosition`
    /// builds them for hit and hurt collisions and attachments (RE-468).
    pub fn compose_collision(
        &self,
        pack: &Pack<'_>,
        object: &ObjectDesc,
        out: &mut [ssb_rom::scene::Mat4],
    ) -> usize {
        self.skeleton.compose_collision(pack, object, out)
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
    ) -> FighterScene {
        let pos = pack
            .spawn(stage, spawn_index)
            .map(|spawn| ssb_engine::math::Vec3::new(spawn.x as f32, spawn.y as f32, 0.0));
        Self::at_position(pack, stage, kind, spawn_index as u8, pos)
    }

    /// [`FighterScene::at_spawn`] at an explicit position, such as the drop
    /// point `sc1PGameSpawnEnemyTeamNext` picks; `None` leaves the fighter
    /// unplaced.
    pub fn at_position(
        pack: &Pack<'_>,
        stage: &StageDesc,
        kind: FighterKind,
        port: u8,
        pos: Option<ssb_engine::math::Vec3>,
    ) -> FighterScene {
        let spawn_index = port;
        // `Fighter::new`'s `port` and `pack.spawn`'s `player` are different
        // concepts that happen to share the same value by this project's own
        // convention (port N spawns at spawn N) -- the same convention the
        // pre-generalization `Play`/`Dummy` split already baked in (ports 0
        // and 1 for spawns 0 and 1).
        let mut fighter = Fighter::new(kind, spawn_index, 3);

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
        FighterScene {
            fighter,
            placed,
            airborne_ticks: 0,
            jump_was_held: false,
            from_pack,
            skeleton: ssb_rom::skeleton::Skeleton::new(),
            // Which object a character's animations drive is stored per joint,
            // so any one of them names it; a scan over the objects finds which
            // one owns that node. Done once, here, rather than per tick.
            object: fighter_object(pack, kind as u32).unwrap_or(u32::MAX),
            object_low: fighter_low_object(pack, kind as u32).unwrap_or(u32::MAX),
            // Matches real hardware's own `dGMCameraCObjVecDefault` rest
            // state (RE-131) rather than starting already converged on the
            // fighter -- the pan-in from that rest state as the match
            // begins is real, observable behaviour, not a startup glitch to
            // hide.
            camera: ssb_game::camera::Camera::default(),
            entry_zoom: None,
            bonus_follow: false,
            camera_status: None,
            player_zoom: None,
            demo_zoom: None,
            cam_offset_y,
            camera_zoom_frame,
            shadow_size,
            started: None,
            shield_yrotn: ssb_rom::figatree::JointPose::default(),
            root_motion_before_tick: None,
            anim_ticked_early: false,
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
        let tapped = jump_held && !self.jump_was_held;
        let released = !jump_held && self.jump_was_held;
        self.jump_was_held = jump_held;

        self.fighter.pikachu.map_bound_top = Some(stage.bounds.top as f32);
        self.fighter.set_input(input, tapped, released);
        self.sample_held_child_offset();
        if matches!(
            self.fighter.status.status,
            AnyStatus::Fox(
                ssb_game::status::FoxStatus::SpecialN | ssb_game::status::FoxStatus::SpecialAirN
            )
        ) {
            if let Some(anchor) = self.weapon_anchor(pack, 17, 60.0) {
                self.fighter.set_weapon_spawn_anchor(anchor);
            }
        }
        // Every status gets its TransN sample: `ssb-game`'s physics reads it
        // only where the source's `proc_physics` applies TransN, so a list
        // here could only miss statuses (Final Cutter, the Falcon Kick,
        // RE-378).
        if let (Some(before), Some(current)) = (self.root_motion_before_tick, self.skeleton.pose(0))
        {
            self.fighter.set_root_motion(ssb_game::physics::RootMotion {
                delta: ssb_engine::math::Vec3::new(
                    current.translate[0] - before.translate[0],
                    current.translate[1] - before.translate[1],
                    current.translate[2] - before.translate[2],
                ),
                rotate_z: current.rotate[2],
                translate: ssb_engine::math::Vec3::new(
                    current.translate[0],
                    current.translate[1],
                    current.translate[2],
                ),
            });
        }
        self.fighter
            .tick_interrupt(&|| MapSegments::with_groups(pack, stage, groups));
        // `ftCommonThrowProcUpdate` releases from the hand this frame's play
        // has posed (`ftCommonThrownProcPhysics` then the release), before
        // `ftMainProcPhysicsMap`: play the clip now and sample the hand for
        // the release `grab::exchange` delivers next.
        if ssb_game::grab::release_pending(&self.fighter) && !self.fighter.is_in_hitlag() {
            self.tick_animation(pack);
            self.anim_ticked_early = true;
            self.sample_gameplay_joints(pack);
            if let Some(joint) = ssb_game::grab::itemheavy_joint(self.fighter.kind) {
                let hand = self.fighter.joint_transforms.get(joint).copied().flatten();
                self.fighter.grab.anchor = hand.map(|j| j.origin);
                self.fighter.grab.anchor_transform = hand;
            }
        }
    }

    /// The priority-4 half: `ftMainProcPhysicsMap`, then the animation and
    /// the joint samples gameplay reads.
    pub fn tick_fighter_physics(
        &mut self,
        pack: &Pack<'_>,
        stage: &StageDesc,
        groups: &[ssb_game::map::MapGroup],
    ) {
        // `ftMainProcUpdateInterrupt`'s `ftMainPlayAnimEventsAll` has played
        // this frame's pose before `ftMainProcPhysicsMap` runs, so TransN's
        // step (`translate - anim_vel`) and the entry's placement read the
        // frame the status is on (RE-466). Hitlag freezes the motion with
        // the status.
        if !self.fighter.is_in_hitlag() {
            if !core::mem::take(&mut self.anim_ticked_early) {
                let t = crate::profile::start();
                self.tick_animation(pack);
                crate::profile::stop(crate::profile::Span::Anim, t);
            }
            if let (Some(before), Some(current)) =
                (self.root_motion_before_tick, self.skeleton.pose(0))
            {
                // The interrupt may have turned TransN (Mario's Super Jump
                // Punch); its rotation is kept.
                let rotate_z = self.fighter.root_motion.rotate_z;
                self.fighter.set_root_motion(ssb_game::physics::RootMotion {
                    delta: ssb_engine::math::Vec3::new(
                        current.translate[0] - before.translate[0],
                        current.translate[1] - before.translate[1],
                        current.translate[2] - before.translate[2],
                    ),
                    rotate_z,
                    translate: ssb_engine::math::Vec3::new(
                        current.translate[0],
                        current.translate[1],
                        current.translate[2],
                    ),
                });
            }
        } else {
            // No TransN motion accrues while frozen.
            self.root_motion_before_tick = self.skeleton.pose(0).copied();
        }
        let t = crate::profile::start();
        self.fighter
            .tick_physics_map_before_accessory(&|| MapSegments::with_groups(pack, stage, groups));
        crate::profile::stop(crate::profile::Span::Map, t);
        if self.fighter.is_grounded() {
            self.airborne_ticks = 0;
        } else {
            self.airborne_ticks = self.airborne_ticks.saturating_add(1);
        }
        // A status the map step set (a landing) parses its first frame at
        // once (`ftMainSetStatus`).
        if !self.fighter.is_in_hitlag()
            && self.started != Some((self.fighter.status.status, self.fighter.status.entry))
        {
            let t = crate::profile::start();
            self.tick_animation(pack);
            crate::profile::stop(crate::profile::Span::Anim, t);
        }
        self.sample_held_child_offset();
        if (ssb_game::map::is_cliff_hold(self.fighter.status.status)
            || ssb_game::map::is_cliff_phase2(self.fighter.status.status))
            && self.skeleton.joint_node(0).is_none()
        {
            if let Some(pose) = self.skeleton.pose(0) {
                self.fighter.transn = ssb_engine::math::Vec3::new(
                    pose.translate[0],
                    pose.translate[1],
                    pose.translate[2],
                );
                if ssb_game::map::is_cliff_hold(self.fighter.status.status) {
                    let f = &mut self.fighter;
                    f.pos.x = f.cliff.corner.x + f.transn.z * f.facing.sign() * f.attributes.size;
                    f.pos.y = f.cliff.corner.y + f.transn.y * f.attributes.size;
                }
            }
        }
        ssb_game::grab::refresh_held_attachment(&mut self.fighter);
        let t = crate::profile::start();
        self.sample_gameplay_joints(pack);
        crate::profile::stop(crate::profile::Span::Joints, t);
        // `proc_accessory` reads the updated hand, after the map and parts
        // transform pass (`ftMainProcPhysicsMap`).
        ssb_game::item_use::accessory(&mut self.fighter);
        // A held fighter hangs from this fighter's `joint_itemheavy_id`; the
        // match loop hands the sampled position over in `grab::exchange`.
        // A catch status samples it too: the catch is made in the hit phase
        // and `ftCommonCapturePulledProcCapture` places the caught fighter
        // from this frame's hand at once (RE-472).
        let holds = self.fighter.grab.catch.is_some() || self.fighter.grab.is_catchstatus;
        self.fighter.grab.anchor = if holds {
            ssb_game::grab::itemheavy_joint(self.fighter.kind)
                .and_then(|joint| self.fighter.joint_transforms.get(joint).copied().flatten())
                .map(|joint| joint.origin)
        } else {
            None
        };
        self.fighter.grab.anchor_transform = if holds {
            ssb_game::grab::itemheavy_joint(self.fighter.kind)
                .and_then(|joint| self.fighter.joint_transforms.get(joint).copied().flatten())
        } else {
            None
        };
    }

    /// The fighter's entry in `gmCameraUpdateInterests`, by its camera mode:
    /// the fighter's position, the entry position while entering, or
    /// `gmCameraSetDeadUpStarPosition`'s top-out point; `None` for a ghost.
    pub fn camera_interest(&self, stage: &StageDesc) -> Option<ssb_game::camera::Interest> {
        use ssb_game::dead::CameraMode;
        let f = &self.fighter;
        let (mut target, facing) = match f.dead.camera_mode {
            CameraMode::Ghost => return None,
            CameraMode::Entry | CameraMode::Explain => (f.entry.pos, f.entry.lr.unwrap_or(f.facing)),
            CameraMode::DeadUp => (f.dead.up_pos, f.facing),
            CameraMode::Default => (f.pos, f.facing),
        };
        target.y += self.cam_offset_y;
        // `gmCameraUpdateInterests`: a 1P Game enemy (the only fighter with
        // team blast bounds) is clamped to the team camera bounds instead,
        // whatever its camera mode.
        let team_bounds = f.dead.team_bounds.map(|_| ssb_game::camera::Bounds {
            top: f32::from(stage.team_camera.top),
            bottom: f32::from(stage.team_camera.bottom),
            left: f32::from(stage.team_camera.left),
            right: f32::from(stage.team_camera.right),
        });
        let dead_up = f.dead.camera_mode == CameraMode::DeadUp && team_bounds.is_none();
        if dead_up {
            let top = f32::from(stage.camera.top);
            target.x = top * target.x / f32::from(stage.bounds.top);
            target.y = top;
        }
        Some(ssb_game::camera::Interest {
            target_pos: target,
            facing_left: matches!(facing, ssb_game::fighter::Facing::Left),
            zoom_frame: self.camera_zoom_frame,
            zoom_range: 1.0,
            idle_zoomed_out: f.status.status == Status::Wait && f.status.anim_frame >= 120.0,
            dead_up,
            team_bounds,
        })
    }

    /// Advances the battle camera one tick, framing the fighter and
    /// `others` (the other fighters' [`FighterScene::camera_interest`]s, in
    /// link order); any past the fourth interest are ignored.
    pub fn tick_camera(&mut self, stage: &StageDesc, others: &[ssb_game::camera::Interest]) {
        // `gcEndProcessAll` on the movie camera: its scene moves it.
        if self.camera.movie.is_some() {
            return;
        }
        match self.camera_status {
            Some(CameraStatus::MapZoom { origin, target }) => {
                self.camera.tick_map_zoom(origin, target);
                return;
            }
            // `gmCameraAnimFuncCamera`: the host plays the animation
            // ([`play_camera_anim`]); once its script has ended the battle
            // camera takes over (`gmCameraSetStatusDefault`).
            Some(CameraStatus::Anim { ended, .. }) => {
                if ended {
                    self.camera_status = None;
                } else {
                    return;
                }
            }
            None => {}
        }
        if self.bonus_follow {
            let pos = self.fighter.pos;
            let bounds = ssb_game::camera::Bounds {
                top: stage.camera.top as f32, bottom: stage.camera.bottom as f32,
                left: stage.camera.left as f32, right: stage.camera.right as f32,
            };
            let target = bounds.clamp(ssb_engine::math::Vec3::new(pos.x, pos.y + self.cam_offset_y, 0.0));
            let race = stage.source_file == ssb_rom::ground_obj::BONUS3_FILE;
            let pitch = if race || ssb_rom::bonus2::kind(stage.source_file).is_some() { -15.0 } else { -9.0 };
            self.camera.tick_player_zoom(target, (0.0, pitch * core::f32::consts::PI / 180.0), if race { 7000.0 } else { 9000.0 }, 0.3, 31.5);
            return;
        }
        if let Some((target, dist)) = self.entry_zoom {
            self.camera.tick_player_zoom(
                target,
                (0.0, 0.0),
                dist,
                ssb_game::pause::ZOOM_PAN_SCALE,
                ssb_game::appear::FOCUS_ZOOM_FOV,
            );
            return;
        }
        let (_, _, vw, vh) = ssb_engine::coord::pillarboxed_viewport();
        let bounds = ssb_game::camera::Bounds {
            top: stage.camera.top as f32,
            bottom: stage.camera.bottom as f32,
            left: stage.camera.left as f32,
            right: stage.camera.right as f32,
        };
        // `gmCameraPlayerZoomFuncCamera`: the battle camera while the
        // fighter is out of the camera bounds.
        if let Some(z) = self.demo_zoom {
            if ssb_game::pause::kind_for(z.pos, bounds) != ssb_game::pause::PauseKind::PlayerNA {
                let target = ssb_engine::math::Vec3::new(z.pos.x, z.pos.y + z.cam_offset_y, z.pos.z);
                self.camera.tick_player_zoom(target, z.eye, z.dist, z.pan_scale, z.fov);
                return;
            }
        }
        if let Some(dist) = self.player_zoom {
            let pos = self.fighter.pos;
            if ssb_game::pause::kind_for(pos, bounds) != ssb_game::pause::PauseKind::PlayerNA {
                let target = ssb_engine::math::Vec3::new(pos.x, pos.y + self.cam_offset_y, pos.z);
                self.camera.tick_player_zoom(
                    target,
                    (0.0, 0.0),
                    dist,
                    ssb_game::training::CLOSE_UP_PAN_SCALE,
                    ssb_game::training::CLOSE_UP_FOV,
                );
                return;
            }
        }
        let mut list = [ssb_game::camera::Interest::default(); 4];
        let mut count = 0;
        let all = self.camera_interest(stage).into_iter().chain(others.iter().copied());
        for interest in all.take(list.len()) {
            list[count] = interest;
            count += 1;
        }
        // How to Play's battle viewport (300 x 150) frames by its own
        // aspect; the full battle viewport keeps the screen's.
        let aspect = if self.camera.viewport == ssb_game::camera::BATTLE_VIEWPORT {
            vw as f32 / vh as f32
        } else {
            let (w, h) = self.camera.viewport_size();
            w / h
        };
        self.camera.tick_interests(&list[..count], bounds, stage.camera_light_angle_z, aspect);
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
        self.tick_camera(stage, additional_camera_interest.as_slice());
    }

    /// Starts the animation the current status calls for, then advances it.
    fn tick_animation(&mut self, pack: &Pack<'_>) {
        if self.object == u32::MAX {
            return;
        }
        self.root_motion_before_tick = tick_skeleton_animation(
            pack,
            self.fighter.kind as u32,
            pack.object(self.object).map_or(0, |o| o.first_node),
            &self.fighter.status,
            ssb_game::status::clip_speed(&self.fighter),
            &mut self.skeleton,
            &mut self.started,
        );
        self.apply_shield_pose(pack);
    }

    /// `ftCommonGuardUpdateJoints` / `ftCommonGuardInitJoints`: the stick
    /// tilts the shield and, in `Guard`, the whole fighter (RE-367).
    fn apply_shield_pose(&mut self, pack: &Pack<'_>) {
        use ssb_game::status::ShieldPose;
        use ssb_rom::skeleton::ShieldJoints;
        let Some(pose) = ssb_game::status::shield_pose(&self.fighter) else {
            return;
        };
        let guard = &self.fighter.guard;
        let Some(anim) = pack.shield_pose(self.fighter.kind as u32, guard.angle_i as u32) else {
            return;
        };
        let joints = match pose {
            ShieldPose::ShieldJoint => ShieldJoints::ShieldJoint,
            ShieldPose::AllJoints => ShieldJoints::All,
        };
        ssb_rom::skeleton::apply_shield_pose(
            pack,
            &anim,
            guard.angle_f,
            guard.shield_rotate_range,
            joints,
            &mut self.skeleton,
            &mut self.shield_yrotn,
        );
    }

    /// The tail of `ftCommonCapturePulledProcCapture`, for a fighter caught
    /// in this frame's hit phase: `ftMainPlayAnimEventsAll` plays
    /// `CapturePulled`'s first frame, then `ftCommonCapturePulledProcPhysics`
    /// and `ftCommonCapturePulledProcMap` place it at the catcher's hand on
    /// the catch frame itself (RE-472). Call after `grab::exchange` has
    /// delivered the capture.
    pub fn settle_capture(
        &mut self,
        pack: &Pack<'_>,
        stage: &StageDesc,
        groups: &[ssb_game::map::MapGroup],
    ) {
        let status = &self.fighter.status;
        if status.status != ssb_game::status::Status::CapturePulled
            || self.started == Some((status.status, status.entry))
            || self.fighter.grab.holder.is_none()
        {
            return;
        }
        self.tick_animation(pack);
        self.sample_held_child_offset();
        let segments = || MapSegments::with_groups(pack, stage, groups);
        ssb_game::grab::tick_held(&mut self.fighter, || ssb_game::map::floors(segments()));
        self.sample_gameplay_joints(pack);
    }

    /// `ftMainSetStatus` in the hit phase (a hit's damage status, a
    /// release): the new figatree is parsed at its first frame at once, so
    /// the pose drawn and the joints sampled through the hitlag that follows
    /// are the new status's (RE-472: Luigi's `DamageAir1` joints while a
    /// second flame meets him in hitlag). Call after the hit phase.
    pub fn settle_status(&mut self, pack: &Pack<'_>) {
        let status = &self.fighter.status;
        if self.started == Some((status.status, status.entry)) {
            return;
        }
        self.tick_animation(pack);
        self.sample_gameplay_joints(pack);
    }

    fn sample_held_child_offset(&mut self) {
        self.fighter.grab.held_child_offset = if ssb_game::grab::is_held(self.fighter.status.status)
        {
            self.skeleton.pose(0).map(|p| {
                ssb_engine::math::Vec3::new(p.translate[0], p.translate[1], p.translate[2])
            })
        } else {
            None
        };
    }

    /// Maps an authored weapon attachment joint and forward offset into
    /// match coordinates before the fighter's motion event consumes it.
    /// Mario's Fireball uses joint 16; Fox's Blaster uses joint 17 plus 60.
    fn weapon_anchor(
        &self,
        pack: &Pack<'_>,
        joint: usize,
        offset_x: f32,
    ) -> Option<ssb_engine::math::Vec3> {
        let node = self.skeleton.joint_node(joint)?;
        self.node_anchor(pack, node, offset_x)
    }

    /// Samples all posed model nodes once for gameplay collision. Joint IDs
    /// 4.. are the packed DObjDesc nodes; 0 is TopN. The other runtime joints
    /// have no packed node and are not used by the ported motion collisions.
    fn sample_gameplay_joints(&mut self, pack: &Pack<'_>) {
        use ssb_engine::math::Vec3;
        use ssb_game::fighter::{JointTransform, FIGHTER_JOINTS};
        self.fighter.joint_transforms = [None; FIGHTER_JOINTS];
        // TopN carries `attr->size` (`ftManagerMakeFighter`), which every
        // joint below it inherits (RE-458).
        let size = self.fighter.attributes.size;
        let axes = ssb_game::item_throw::collision_axes(&self.fighter).map(|a| a * size);
        let world = |v: Vec3| axes[0] * v.x + axes[1] * v.y + axes[2] * v.z;
        let root = JointTransform {
            axes,
            origin: self.fighter.pos,
        };
        self.fighter.joint_transforms[0] = Some(root);
        let Some(object) = pack.object(self.object) else {
            return;
        };
        let mut posed = [ssb_rom::scene::Mat4::IDENTITY; ssb_rom::skeleton::MAX_NODES];
        let count = self
            .compose_collision(pack, &object, &mut posed)
            .min(FIGHTER_JOINTS - 4);
        let scale = ssb_rom::pack::MODEL_SCALE;
        for (i, matrix) in posed[..count].iter().enumerate() {
            let axis = |col: usize| {
                let m = &matrix.0;
                world(Vec3::new(m[col * 4], m[col * 4 + 1], m[col * 4 + 2]))
            };
            let t = matrix.translation();
            self.fighter.joint_transforms[i + 4] = Some(JointTransform {
                axes: [axis(0), axis(1), axis(2)],
                origin: self.fighter.pos
                    + world(Vec3::new(t[0] * scale, t[1] * scale, t[2] * scale)),
            });
        }
        // `YRotN` hangs off `XRotN`, which nothing rotates, so its local
        // pose is its model-space pose. Only a raised shield reads it.
        if ssb_game::status::shield_pose(&self.fighter).is_some() {
            let pose = &self.shield_yrotn;
            let scale = ssb_rom::pack::MODEL_SCALE;
            let local = |p: &ssb_rom::figatree::JointPose, s: [f32; 3]| {
                let t = [p.translate[0] / scale, p.translate[1] / scale, p.translate[2] / scale];
                ssb_rom::scene::Mat4::from_trs_collision(t, p.rotate, s)
            };
            let mut m = local(pose, [1.0; 3]);
            // Under `XRotN` when the guard clip interposes it.
            if let Some(lead) = self.skeleton.lead_xrotn_pose() {
                m = local(lead, lead.scale).mul(&m);
            }
            let axis = |col: usize| {
                world(Vec3::new(m.0[col * 4], m.0[col * 4 + 1], m.0[col * 4 + 2]))
            };
            let t = m.translation();
            self.fighter.joint_transforms[3] = Some(JointTransform {
                axes: [axis(0), axis(1), axis(2)],
                origin: self.fighter.pos + world(Vec3::new(t[0] * scale, t[1] * scale, t[2] * scale)),
            });
        }
    }

    fn node_anchor(
        &self,
        pack: &Pack<'_>,
        node: u32,
        offset_x: f32,
    ) -> Option<ssb_engine::math::Vec3> {
        let object = pack.object(self.object)?;
        let mut posed = [ssb_rom::scene::Mat4::IDENTITY; ssb_rom::skeleton::MAX_NODES];
        let count = self.compose_collision(pack, &object, &mut posed);
        let local_index = node.checked_sub(object.first_node)? as usize;
        if local_index >= count {
            return None;
        }
        let local = posed[local_index].translation();
        let scale = ssb_rom::pack::MODEL_SCALE * self.fighter.attributes.size;
        let axes = ssb_game::item_throw::collision_axes(&self.fighter);
        Some(self.fighter.pos
            + axes[0] * (local[0] * scale)
            + axes[1] * (local[1] * scale)
            + axes[2] * (local[2] * scale + offset_x))
    }
}

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

/// `gmCameraSetStatusPlayerZoom(fighter, eye_x, eye_y, dist, pan, fov)`
/// as the auto demo sets it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DemoZoom {
    /// The fighter's port.
    pub port: u8,
    /// `pzoom_eye_x`, `pzoom_eye_y` in radians.
    pub eye: (f32, f32),
    /// `pzoom_dist`: the fighter's `closeup_camera_zoom`.
    pub dist: f32,
    pub pan_scale: f32,
    pub fov: f32,
    /// The fighter's translation and `cam_offset_y` at this tick.
    pub pos: ssb_engine::math::Vec3,
    pub cam_offset_y: f32,
}

/// A camera status set over the battle camera.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CameraStatus {
    /// `gmCameraSetStatusMapZoom(&origin, &target)`.
    MapZoom {
        origin: ssb_engine::math::Vec3,
        target: ssb_engine::math::Vec3,
    },
    /// `gmCameraSetStatusAnim`: the host plays the baked frames
    /// ([`CameraAnim`]) into the camera; `ended` hands back to the battle
    /// camera on the next tick.
    Anim {
        frame: usize,
        vel: ssb_engine::math::Vec3,
        ended: bool,
    },
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
    scene.camera_status = Some(CameraStatus::Anim {
        frame: 0,
        vel,
        ended: false,
    });
    play_camera_anim(scene, anim);
}

/// `gmCameraUpdateAnimVel` (`gcPlayCamAnim`, then `vel_all`): one play.
/// Returns whether the animation was playing.
pub fn play_camera_anim(scene: &mut FighterScene, anim: &CameraAnim) -> bool {
    let Some(CameraStatus::Anim { frame, vel, .. }) = scene.camera_status else {
        return false;
    };
    let last = anim.frames.len().saturating_sub(1);
    let f = anim.frames[frame.min(last)];
    scene.camera.eye = ssb_engine::math::Vec3::new(f[0], f[1], f[2]) + vel;
    scene.camera.at = ssb_engine::math::Vec3::new(f[3], f[4], f[5]) + vel;
    scene.camera.fovy_degrees = f[6];
    scene.camera_status = Some(CameraStatus::Anim {
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
    ssb_game::boss::model_yaw(f)
        .or_else(|| ssb_game::appear::model_yaw(f))
        .or_else(|| ssb_game::item_throw::model_yaw(f))
        .or_else(|| ssb_game::dokan::model_yaw(f))
        // TopN's yaw, set from `lr` by `ftMainSetStatus`: a status that
        // flips `lr` part-way (Turn's pivot, a roll) keeps it (RE-468).
        .unwrap_or_else(|| core::f32::consts::FRAC_PI_2 * f.topn_lr)
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
pub struct ItemAnimsPort<'a, 'p> {
    pub pack: &'a Pack<'p>,
    pub objects: &'a mut ssb_rom::ground_obj::GroundObjects,
}

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
