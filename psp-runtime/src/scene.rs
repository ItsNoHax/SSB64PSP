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
//! mapped pack as the query asks for them. A tick's collision work is
//! therefore proportional to the stage's floor count with no per-frame setup,
//! which is what keeps this affordable inside the frame budget.
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

/// The entry effects a manager `EFDesc` plays through its own `AnimJoint`
/// table and ejects at its end (`efManagerHaveStructProcUpdate`, RE-403),
/// keyed like `ssb_rom::effect::MANAGER_EFFECT_KEYS`: Mario's pipe
/// (`llMarioSpecial2EntryDokanDObjDesc`), Donkey Kong's barrel, Samus's
/// capsule, Link's wave and beam, Yoshi's egg and Kirby's star (whose
/// packed table is the leftward one). The Arwing, the car and the Poké Ball
/// run their own updates and are not listed.
pub const ENTRY_EFFECT_KEYS: [(u32, u32); 7] = [
    (356, 0x0608),
    (355, 0x07C8),
    (349, 0x0B90),
    (353, 0x03F8),
    (353, 0x07B8),
    (354, 0x0530),
    (348, 0x1DA8),
];

/// The [`ENTRY_EFFECT_KEYS`] indices a fighter's entry effect draws. Only
/// Mario's pipe and Kirby's leftward star are packed: Luigi's pipe comes
/// from his own file (`gFTDataLuigiSpecial2`) and the rightward star from
/// `llKirbySpecial2EntryStarRAnimJoint`, so neither draws.
pub fn entry_effect_parts(f: &ssb_game::fighter::Fighter) -> &'static [usize] {
    use ssb_game::appear::EntryEffect as E;
    use ssb_game::fighter::{Facing, FighterKind};
    let Some(e) = ssb_game::appear::entry_effect(f.kind) else {
        return &[];
    };
    match e {
        E::Pipe if f.kind == FighterKind::Luigi => &[],
        E::Star if f.entry.lr != Some(Facing::Left) => &[],
        E::Pipe => &[0],
        E::Barrel => &[1],
        E::Point => &[2],
        E::WaveAndBeam => &[3, 4],
        E::Egg => &[5],
        E::Star => &[6],
        E::Arwing | E::PokeBall | E::Car => &[],
    }
}

/// A manager effect's object and its transform animation.
pub fn manager_effect(pack: &Pack<'_>, key: (u32, u32)) -> Option<(ObjectDesc, ssb_rom::pack::AnimDesc)> {
    let slot = ssb_rom::effect::MANAGER_EFFECT_KEYS.iter().position(|&k| k == key)?;
    Some((object_keyed(pack, key)?, pack.effect_anim(slot as u32)?))
}

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
            animator,
            groups,
            anim,
            first_node,
            previous_flags: alloc::vec![0; count],
        }
    }

    pub fn tick(&mut self, pack: &Pack<'_>) -> Result<(), ssb_rom::objanim::AnimError> {
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

pub struct MapSegments<'a, 'p> {
    pack: &'a Pack<'p>,
    stage: &'a StageDesc,
    line: u32,
    current: Option<LineDesc>,
    point: u16,
    prev: Option<(i16, i16, u16, u16)>,
    groups: &'a [ssb_game::map::MapGroup],
}

impl<'a, 'p> MapSegments<'a, 'p> {
    pub fn new(pack: &'a Pack<'p>, stage: &'a StageDesc) -> Self {
        Self {
            pack,
            stage,
            line: 0,
            current: None,
            point: 0,
            prev: None,
            groups: &[],
        }
    }
    pub fn with_groups(
        pack: &'a Pack<'p>,
        stage: &'a StageDesc,
        groups: &'a [ssb_game::map::MapGroup],
    ) -> Self {
        Self {
            groups,
            ..Self::new(pack, stage)
        }
    }
}

impl Iterator for MapSegments<'_, '_> {
    type Item = MapSurface;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let Some(line) = self.current else {
                if self.line >= self.stage.line_count {
                    return None;
                }
                let candidate = self.pack.line(self.stage.first_line + self.line);
                self.line += 1;
                if let Some(candidate) = candidate.filter(|line| {
                    line.vertex_count >= 2
                        && self
                            .groups
                            .get(line.yakumono as usize)
                            .is_none_or(|g| g.exists())
                }) {
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
            return Some(MapSurface {
                motion: self
                    .groups
                    .get(line.yakumono as usize)
                    .and_then(|g| g.motion()),
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
            });
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
    let frame_begin = status.anim_frame_begin;
    let status = status.status;
    // The pack row is the fighter's; the slot is the status's. Common
    // statuses use the shared slots, which resolve per fighter through the
    // status -> motion pairing (`ssb_rom::anim::SLOT_APPEAL`).
    let slot = status.anim_slot() as u32;
    if *started != Some((status, entry)) {
        *started = Some((status, entry));
        // A status with no motion of its own (`CatchWait`, `CaptureWait`)
        // keeps playing the previous one; its slot is that status's slot.
        if status.keeps_motion() {
            // Yoshi Bomb's fall keeps the start clip at speed zero.
            skeleton.speed = speed;
        } else {
            if let Some(anim) = pack.fighter_anim(kind, slot) {
                skeleton.start(pack, &anim, frame_begin, speed);
            }
        }
    }
    // The slot is read back rather than remembered, so a status whose
    // animation the pack lacks -- Kirby has no aerial jump -- simply keeps
    // the pose it had.
    let root_before = skeleton.pose(0).copied();
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
    /// The real battle camera (RE-131), ticked alongside the fighter each
    /// frame in [`FighterScene::tick`].
    pub camera: ssb_game::camera::Camera,
    /// The entry focus's zoom (`ifCommonEntryFocusThread`'s
    /// `gmCameraSetStatusPlayerZoom`, RE-402): a target with its
    /// `cam_offset_y` added and a distance. `None` runs the battle camera.
    pub entry_zoom: Option<(ssb_engine::math::Vec3, f32)>,
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
        let count = self.skeleton.compose(pack, object, out);
        if matches!(
            self.fighter.status.status,
            AnyStatus::Common(Status::CapturePulled | Status::CaptureWait)
        ) && self.skeleton.joint_node(0).is_none()
        {
            if let Some(pose) = self.skeleton.pose(0) {
                let scale = ssb_rom::pack::MODEL_SCALE;
                let runtime = ssb_rom::scene::Mat4::from_trs(
                    [
                        pose.translate[0] / scale,
                        pose.translate[1] / scale,
                        pose.translate[2] / scale,
                    ],
                    pose.rotate,
                    pose.scale,
                );
                for matrix in &mut out[..count] {
                    *matrix = runtime.mul(matrix);
                }
            }
        }
        count
    }

    /// Puts a fighter of `kind` at a stage's `spawn_index`'th spawn point.
    ///
    /// Deliberately *not* settled onto the surface: a spawn sits a few units
    /// up (RE-030) and letting it fall that distance is the first thing worth
    /// watching. Returns a `FighterScene` even when the stage has no such
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
        // `Fighter::new`'s `port` and `pack.spawn`'s `player` are different
        // concepts that happen to share the same value by this project's own
        // convention (port N spawns at spawn N) -- the same convention the
        // pre-generalization `Play`/`Dummy` split already baked in (ports 0
        // and 1 for spawns 0 and 1).
        let mut fighter = Fighter::new(kind, spawn_index as u8, 3);

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
            shadow_size = d.shadow_size;
        }

        let mut placed = false;
        if let Some(spawn) = pack.spawn(stage, spawn_index) {
            fighter.pos = ssb_engine::math::Vec3::new(spawn.x as f32, spawn.y as f32, 0.0);
            fighter.facing = ssb_game::fighter::Facing::at_spawn_x(fighter.pos.x);
            placed = ssb_game::collision::project_floor(
                FloorSegments::new(pack, stage),
                ssb_engine::math::Vec2::new(fighter.pos.x, fighter.pos.y),
            )
            .is_some();
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
            // Matches real hardware's own `dGMCameraCObjVecDefault` rest
            // state (RE-131) rather than starting already converged on the
            // fighter -- the pan-in from that rest state as the match
            // begins is real, observable behaviour, not a startup glitch to
            // hide.
            camera: ssb_game::camera::Camera::default(),
            entry_zoom: None,
            cam_offset_y,
            camera_zoom_frame,
            shadow_size,
            started: None,
            shield_yrotn: ssb_rom::figatree::JointPose::default(),
            root_motion_before_tick: None,
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
            AnyStatus::Mario(
                ssb_game::status::MarioStatus::SpecialN
                    | ssb_game::status::MarioStatus::SpecialAirN
            )
        ) {
            if let Some(anchor) = self.weapon_anchor(pack, 16, 0.0) {
                self.fighter.set_weapon_spawn_anchor(anchor);
            }
        }
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
    }

    /// The priority-4 half: `ftMainProcPhysicsMap`, then the animation and
    /// the joint samples gameplay reads.
    pub fn tick_fighter_physics(
        &mut self,
        pack: &Pack<'_>,
        stage: &StageDesc,
        groups: &[ssb_game::map::MapGroup],
    ) {
        self.fighter
            .tick_physics_map(&|| MapSegments::with_groups(pack, stage, groups));
        if self.fighter.is_grounded() {
            self.airborne_ticks = 0;
        } else {
            self.airborne_ticks = self.airborne_ticks.saturating_add(1);
        }
        // Hitlag freezes the motion with the status (`ftMainProcUpdateMain`
        // skips the animation while `hitlag_tics` is nonzero).
        if !self.fighter.is_in_hitlag() {
            self.tick_animation(pack);
        } else {
            // No TransN motion accrues while frozen.
            self.root_motion_before_tick = self.skeleton.pose(0).copied();
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
        self.sample_gameplay_joints(pack);
        // A held fighter hangs from this fighter's `joint_itemheavy_id`; the
        // match loop hands the sampled position over in `grab::exchange`.
        self.fighter.grab.anchor = if self.fighter.grab.catch.is_some() {
            ssb_game::grab::itemheavy_joint(self.fighter.kind)
                .and_then(|joint| self.fighter.joint_transforms.get(joint).copied().flatten())
                .map(|joint| joint.origin)
        } else {
            None
        };
        self.fighter.grab.anchor_transform = if self.fighter.grab.catch.is_some() {
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
            CameraMode::Entry => (f.entry.pos, f.entry.lr.unwrap_or(f.facing)),
            CameraMode::DeadUp => (f.dead.up_pos, f.facing),
            CameraMode::Default => (f.pos, f.facing),
        };
        target.y += self.cam_offset_y;
        let dead_up = f.dead.camera_mode == CameraMode::DeadUp;
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
        })
    }

    /// Advances the battle camera one tick, framing the fighter and
    /// `others` (the other fighters' [`FighterScene::camera_interest`]s, in
    /// link order); any past the fourth interest are ignored.
    pub fn tick_camera(&mut self, stage: &StageDesc, others: &[ssb_game::camera::Interest]) {
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
        let mut list = [ssb_game::camera::Interest::default(); 4];
        let mut count = 0;
        let all = self.camera_interest(stage).into_iter().chain(others.iter().copied());
        for interest in all.take(list.len()) {
            list[count] = interest;
            count += 1;
        }
        self.camera.tick_interests(
            &list[..count],
            bounds,
            stage.camera_light_angle_z,
            vw as f32 / vh as f32,
        );
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
        let sign = self.fighter.facing.sign();
        let root = JointTransform {
            axes: [
                Vec3::new(0.0, 0.0, -sign),
                Vec3::new(0.0, 1.0, 0.0),
                Vec3::new(sign, 0.0, 0.0),
            ],
            origin: self.fighter.pos,
        };
        self.fighter.joint_transforms[0] = Some(root);
        let Some(object) = pack.object(self.object) else {
            return;
        };
        let mut posed = [ssb_rom::scene::Mat4::IDENTITY; ssb_rom::skeleton::MAX_NODES];
        let count = self
            .compose_model(pack, &object, &mut posed)
            .min(FIGHTER_JOINTS - 4);
        let scale = ssb_rom::pack::MODEL_SCALE;
        for (i, matrix) in posed[..count].iter().enumerate() {
            let axis = |col: usize| {
                let m = &matrix.0;
                Vec3::new(m[col * 4 + 2] * sign, m[col * 4 + 1], -m[col * 4] * sign)
            };
            let t = matrix.translation();
            self.fighter.joint_transforms[i + 4] = Some(JointTransform {
                axes: [axis(0), axis(1), axis(2)],
                origin: self.fighter.pos
                    + Vec3::new(t[2] * scale * sign, t[1] * scale, -t[0] * scale * sign),
            });
        }
        // `YRotN` hangs off `XRotN`, which nothing rotates, so its local
        // pose is its model-space pose. Only a raised shield reads it.
        if ssb_game::status::shield_pose(&self.fighter).is_some() {
            let pose = &self.shield_yrotn;
            let m = ssb_rom::scene::Mat4::from_trs([0.0; 3], pose.rotate, [1.0; 3]).0;
            let axis =
                |col: usize| Vec3::new(m[col * 4 + 2] * sign, m[col * 4 + 1], -m[col * 4] * sign);
            let t = pose.translate;
            self.fighter.joint_transforms[3] = Some(JointTransform {
                axes: [axis(0), axis(1), axis(2)],
                origin: self.fighter.pos + Vec3::new(t[2] * sign, t[1], -t[0] * sign),
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
        let count = self.compose_model(pack, &object, &mut posed);
        let local_index = node.checked_sub(object.first_node)? as usize;
        if local_index >= count {
            return None;
        }
        let local = posed[local_index].translation();
        let scale = ssb_rom::pack::MODEL_SCALE;
        let facing = self.fighter.facing.sign();
        Some(ssb_engine::math::Vec3::new(
            self.fighter.pos.x + (local[2] * scale + offset_x) * facing,
            self.fighter.pos.y + local[1] * scale,
            self.fighter.pos.z - local[0] * scale * facing,
        ))
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

/// A fighter's model yaw: [`facing_turn`], or none at all while it enters
/// (`lr = 0` faces the camera; Captain Falcon's leftward entry turns
/// around), `ssb_game::appear::model_yaw`.
pub fn fighter_turn(f: &ssb_game::fighter::Fighter) -> f32 {
    ssb_game::appear::model_yaw(f).unwrap_or_else(|| facing_turn(f.facing))
}

/// The file data `grMainSetupMakeGround` hands a VS stage's controller:
/// its kind (`dMPCollisionGroundFileInfos` row), map objects, bottom bound
/// and hazard descriptors.
pub struct StageSetup {
    pub kind: ssb_game::stage::StageKind,
    pub map_objects: alloc::vec::Vec<ssb_game::stage::MapObject>,
    pub bound_bottom: f32,
    pub hazard: [i32; 7],
    pub hazard_surface_y: f32,
}

/// The pack's stage whose `GR*Map` file is VS stage `gkind`
/// (`ssb_rom::stage::VS_GROUND_FILES`).
pub fn vs_stage_index(pack: &Pack<'_>, gkind: u8) -> Option<u32> {
    (0..pack.stage_count()).find(|&i| {
        pack.stage(i)
            .is_some_and(|s| ssb_rom::stage::vs_ground_kind(s.source_file) == Some(gkind))
    })
}

impl StageSetup {
    /// `None` for a stage that is not one of the nine VS stages.
    pub fn new(pack: &Pack<'_>, stage: &StageDesc) -> Option<Self> {
        let gkind = ssb_rom::stage::vs_ground_kind(stage.source_file)?;
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
}
