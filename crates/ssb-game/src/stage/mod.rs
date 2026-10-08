//! Stage controllers (`src/gr/grcommon/*.c`, `grmainsetup.c`) and the ground
//! hazard registries of `ftmain.c` (`sFTMainGroundObstacles`,
//! `sFTMainGroundHazards`).
//!
//! A controller is a stage GObj process at priority 4 on the Ground link: it
//! runs after every fighter's `ftMainProcUpdateInterrupt` (priority 5) and
//! before any fighter's `ftMainProcPhysicsMap` (priority 4, Fighter link).
//! [`Stage::tick`] is that slot. Its stage objects' own animation runs at
//! priority 5 ahead of it, through [`StageObjects`].
//!
//! The objects themselves (Whispy, the barrel, clouds, the acid) are drawn and
//! animated by the runtime. Controllers see them only through the
//! [`StageObjects`] port, which reports the `GObj::anim_frame` and
//! `MObj::anim_wait` values the source polls. Items (the Bumper, POW Block,
//! Piranha Plants and Pokémon) are made through the [`StageItems`] port,
//! which the item pool implements; a kind it cannot make returns `None`,
//! which is a real source outcome (`itManagerMakeItemSetupCommon` fails
//! when the pool is full). The Pokémon are not ported yet. Sector Z's
//! Arwing has an object port of its own ([`sector::ArwingObject`]), and its
//! lasers go to the weapon pool. Of the bonus stages, Race to the Finish has
//! its controller ([`bonus3`]); the others have none. Rumble is not
//! ported; the controllers play their sounds where the source does.

pub mod bonus3;
pub mod castle;
pub mod hyrule;
pub mod inishie;
pub mod jungle;
pub mod pupupu;
pub mod sector;
pub mod yamabuki;
pub mod yoster;
pub mod zebes;

#[cfg(test)]
mod tests;

use crate::fighter::Fighter;
use crate::hazard::{GroundAttack, HazardThrow};
use crate::map::MapGroup;
use crate::weapon::MapSurface;
use ssb_engine::math::Vec3;

mod ground_tables {
    include!(concat!(env!("OUT_DIR"), "/ground_tables.rs"));
}

/// `gMPCollisionGroundData->alt_warning` of the loaded ground, set by
/// [`set_ground_kind`].
static ALT_WARNING: core::sync::atomic::AtomicI32 =
    core::sync::atomic::AtomicI32::new(i16::MIN as i32);

/// `mpCollisionInitGroundData`'s `dMPCollisionGroundFileInfos[gkind]`: the
/// ground data the battle's checks read. Only `alt_warning` is kept here;
/// the rest reaches the game through the pack's stage.
pub fn set_ground_kind(gkind: u8) {
    let alt = ground_tables::ALT_WARNING
        .get(gkind as usize)
        .copied()
        .unwrap_or(i16::MIN);
    ALT_WARNING.store(i32::from(alt), core::sync::atomic::Ordering::Relaxed);
}

/// `MPGroundData::alt_warning`: falling below it plays
/// `nSYAudioFGMAltitudeWarn` (`ftMainProcPhysicsMap`).
pub fn alt_warning() -> f32 {
    ALT_WARNING.load(core::sync::atomic::Ordering::Relaxed) as f32
}

/// `GRKind` for the nine VS stages, in source order.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageKind {
    Castle,
    Sector,
    Jungle,
    Zebes,
    Hyrule,
    Yoster,
    Pupupu,
    Yamabuki,
    Inishie,
    /// `nGRKindBonus3`, Race to the Finish.
    Bonus3,
}

impl StageKind {
    pub fn from_gkind(gkind: u8) -> Option<Self> {
        Some(match gkind {
            0 => StageKind::Castle,
            1 => StageKind::Sector,
            2 => StageKind::Jungle,
            3 => StageKind::Zebes,
            4 => StageKind::Hyrule,
            5 => StageKind::Yoster,
            6 => StageKind::Pupupu,
            7 => StageKind::Yamabuki,
            8 => StageKind::Inishie,
            15 => StageKind::Bonus3,
            _ => return None,
        })
    }

    /// The descriptor a hazard controller reads at `GRxxxMap` + 0xBC:
    /// Zebes' acid and Inishie's POW Block take a `GRAttackColl`, Jungle's
    /// barrel and Hyrule's Twister an `FTThrowHitDesc` (RE-356).
    pub fn hazard_descs(self, words: [i32; 7]) -> (Option<GroundAttack>, Option<HazardThrow>) {
        match self {
            StageKind::Zebes | StageKind::Inishie => (Some(GroundAttack::from_words(words)), None),
            StageKind::Jungle | StageKind::Hyrule => (None, Some(HazardThrow::from_words(words))),
            _ => (None, None),
        }
    }
}

/// `nMPMapObjKind*` values the controllers look up.
pub mod mapobj {
    /// `nMPMapObjKindItem`: where the appearance actor drops items.
    pub const ITEM: u16 = 0x4;
    pub const SCALE_L: u16 = 0x5;
    pub const SCALE_R: u16 = 0x6;
    pub const PAKKUN_L: u16 = 0x7;
    pub const PAKKUN_R: u16 = 0x8;
    pub const POWER_BLOCK: u16 = 0x9;
    pub const TWISTER: u16 = 0xD;
    pub const MONSTER: u16 = 0xE;
    pub const BUMPER: u16 = 0x13;
}

/// One `MPMapObjData` entry.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MapObject {
    pub kind: u16,
    pub pos: Vec3,
}

/// `mpCollisionGetMapObjIDsKind` + `mpCollisionGetMapObjPositionID`: the
/// entries of one kind, in table order.
fn objects_of(objects: &[MapObject], kind: u16) -> impl Iterator<Item = Vec3> + '_ {
    objects
        .iter()
        .filter(move |o| o.kind == kind)
        .map(|o| o.pos)
}

/// A controller-owned stage object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageObj {
    /// `gGRCommonStruct.pupupu.map_gobj[0..4]`.
    WhispyEyes,
    WhispyMouth,
    FlowersBack,
    FlowersFront,
    /// A Yoshi's Island cloud, 0–2.
    Cloud(u8),
    /// The Kongo Jungle barrel; its root carries the default path animation.
    TaruCann,
    /// A Mushroom Kingdom scale platform, 0 left, 1 right.
    Scale(u8),
    /// The Mushroom Kingdom pulley and strings
    /// (`llGRInishieMapScaleDObjDesc`), five nodes in tree order.
    ScaleStrings,
    /// The Saffron City gate.
    Gate,
    /// Peach's Castle's animated ground GObj that carries the Bumper.
    CastleGround,
    /// Planet Zebes' acid (`gGRCommonStruct.zebes.map_gobj`).
    Acid,
}

/// A source animation the controller starts. Each names a
/// `llGR*AnimJoint` (and matching `MatAnimJoint`) of the stage file;
/// `lr` is 0 for left-facing, 1 for right-facing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageAnim {
    /// `dGRPupupuWhispyEyesAnims[lr][status]` on `WhispyEyes`.
    WhispyEyes {
        lr: u8,
        status: pupupu::EyesAnim,
    },
    /// `dGRPupupuWhispyMouthAnims[lr][status]` on `WhispyMouth`.
    WhispyMouth {
        lr: u8,
        status: pupupu::MouthAnim,
    },
    /// `dGRPupupuWhispyMouthTextures[lr][phase]`, played on `FlowersBack`.
    FlowersBack {
        lr: u8,
        phase: u8,
    },
    /// `dGRPupupuWhispyEyesTextures[lr][phase]`, played on `FlowersFront`.
    FlowersFront {
        lr: u8,
        phase: u8,
    },
    /// `dGRYosterCloudMatAnimJoints[0]` / `[1]` on every mesh of a cloud.
    CloudSolid(u8),
    CloudEvaporate(u8),
    /// `llGRJungleMapTaruCannDefaultAnimJoint` on the whole barrel.
    TaruCannDefault,
    /// `llGRJungleMapTaruCann{Fill,Shoot}AnimJoint` on the barrel's child.
    TaruCannFill,
    TaruCannShoot,
    /// `llGRInishieMapScaleRetractAnimJoint` on one platform.
    ScaleRetract(u8),
    /// `llGRYamabukiMapGate{Open,Close}AnimJoint`.
    GateOpen,
    GateClose,
    /// `map_nodes`' animation on the Castle ground GObj.
    CastleGround,
    /// `llGRZebesMapAcidAnimJoint` on the acid.
    Acid,
}

/// Stage items a controller makes (`itManagerMakeItemSetupCommon`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageItem {
    Target(u8),
    /// `sc1PBonusStageMakeBumpers`, indexed past descriptor 0.
    Bonus2Bumper(u8),
    /// `nITKindGBumper`; `castle` is the source's `gkind` test, and
    /// `joint` the `llGRBonus3MapBumpersAnimJoint` script a Race to the
    /// Finish Bumper plays, by its index past descriptor 0.
    Bumper {
        castle: bool,
        joint: Option<u8>,
    },
    /// `nITKindTaruBomb`.
    TaruBomb,
    PowerBlock,
    Pakkun(u8),
    /// `nITKindGroundMonsterStart + id`.
    Monster(u8),
}

/// The runtime half of the stage objects.
///
/// [`StageObjects::play`] is `gcAddAnimAll` followed by the caller's
/// immediate `gcPlayAnimAll`: the script is parsed at frame zero at once.
/// The runtime advances every object's animation at priority 5, before
/// [`Stage::tick`]. The defaults describe an object with no animation.
pub trait StageObjects {
    fn play(&mut self, _anim: StageAnim) {}
    /// `anim_wait = AOBJ_ANIM_NULL; flags = DOBJ_FLAG_NONE`.
    fn stop(&mut self, _obj: StageObj) {}
    /// `GObj::anim_frame`.
    fn anim_frame(&self, _obj: StageObj) -> f32 {
        0.0
    }
    /// `dobj[0]->mobj->anim_wait == AOBJ_ANIM_NULL` for a cloud.
    fn mat_anim_idle(&self, _obj: StageObj) -> bool {
        true
    }
    /// The object's root `DObj` translation.
    fn translate(&self, _obj: StageObj) -> Vec3 {
        Vec3::ZERO
    }
    /// `DObjGetStruct(gobj)->translate.vec.f.y = y`.
    fn set_translate_y(&mut self, _obj: StageObj, _y: f32) {}
    /// `DObjGetStruct(gobj)->translate.vec.f = pos`.
    fn set_translate(&mut self, _obj: StageObj, _pos: Vec3) {}
    /// Node `node`'s translation in tree order (`map_dobjs[node]`), or
    /// `None` when the runtime has no such object.
    fn node_translate(&self, _obj: StageObj, _node: u8) -> Option<Vec3> {
        None
    }
    /// `map_dobjs[node]->translate.vec.f.y = y`.
    fn set_node_translate_y(&mut self, _obj: StageObj, _node: u8, _y: f32) {}
    /// `DObjGetStruct(gobj)->child->translate`, or `None` when the runtime
    /// has no such object.
    fn child_translate(&self, _obj: StageObj) -> Option<Vec3> {
        None
    }
    /// Sector Z's Arwing, or `None` when the runtime has none.
    fn arwing(&mut self) -> Option<&mut dyn sector::ArwingObject> {
        None
    }
}

/// An Arwing with no runtime: every node is idle, so a pattern ends the
/// frame it starts.
struct NoArwing;
impl sector::ArwingObject for NoArwing {
    fn add_anim(&mut self, _: u8, _: Option<sector::ArwingAnim>) {}
    fn add_anim_joint(&mut self, _: u8, _: sector::ArwingAnim) {}
    fn play_all(&mut self) {}
    fn anim_null(&self, _: u8) -> bool {
        true
    }
    fn stop(&mut self, _: u8) {}
    fn flags(&self, _: u8) -> u16 {
        0
    }
    fn set_flags(&mut self, _: u8, _: u16) {}
    fn set_hidden(&mut self, _: bool) {}
    fn translate(&self, _: u8) -> Vec3 {
        Vec3::ZERO
    }
    fn set_translate(&mut self, _: u8, _: Vec3) {}
    fn rotate(&self, _: u8) -> Vec3 {
        Vec3::ZERO
    }
    fn set_rotate(&mut self, _: u8, _: Vec3) {}
    fn path_fraction(&self, _: u8) -> Option<f32> {
        None
    }
    fn path_tangent(&self, _: u8, _: f32) -> Option<Vec3> {
        None
    }
    fn path_point(&self, _: u8, _: f32) -> Option<Vec3> {
        None
    }
    fn set_root(&mut self, _: [[f32; 4]; 4]) {}
}

/// Objects with no runtime: nothing animates.
pub struct NoObjects;
impl StageObjects for NoObjects {}

/// The items a controller makes and moves (`itManagerMakeItemSetupCommon`
/// and the item GObjs it keeps). A handle names one item for as long as it
/// lives. The defaults make nothing.
pub trait StageItems {
    /// Returns a handle, or `None` when the item cannot be made.
    fn make_item(&mut self, _item: StageItem, _pos: Vec3) -> Option<u32> {
        None
    }
    /// A live item's translation and `map_coll.width`.
    fn item_pos_width(&self, _handle: u32) -> Option<(Vec3, f32)> {
        None
    }
    /// `DObjGetStruct(item_gobj)->translate.vec.f.x = x`.
    fn set_item_x(&mut self, _handle: u32, _x: f32) {}
    /// `itPakkunCommonSetWaitFighter`.
    fn pakkun_set_wait_fighter(&mut self, _handle: u32) {}
}

/// No item pool: nothing is made.
pub struct NoItems;
impl StageItems for NoItems {}

/// Stage data the runtime reads from the stage file.
#[derive(Debug, Clone, Copy)]
pub struct StageInit<'a> {
    pub kind: StageKind,
    pub map_objects: &'a [MapObject],
    /// `MPGroundData::map_bound_bottom`.
    pub bound_bottom: f32,
    /// `llGRZebesMapAcidGRAttackColl` or `llGRInishieMapPowerBlockGRAttackColl`.
    pub hazard_attack: Option<GroundAttack>,
    /// `llGRHyruleMapTwisterThrowHitDesc` or `llGRJungleMapTaruCannThrowHitDesc`.
    pub hazard_throw: Option<HazardThrow>,
    /// The acid `DObj`'s child rest translation Y
    /// (`llGRZebesMapAcidDObjDesc[1]`), read while the runtime has no acid
    /// object to animate it.
    pub acid_surface_y: f32,
    /// Race to the Finish's Bumpers (`llGRBonus3MapBumpersDObjDesc`).
    pub bonus3_bumpers: &'a [bonus3::BumperDesc],
    /// `gSCManagerSceneData.player`.
    pub player: u8,
}

/// The two ground obstacles (`GRObstacle` kinds).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Obstacle {
    Twister,
    TaruCann,
}

/// The ground hazard (`GRHazard` kinds).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hazard {
    Acid,
    /// The POW Block item, with its `damage_handicap` and the fighter that
    /// hit it (`damage_gobj`), whom the quake spares.
    PowerBlock {
        handicap: u8,
        hitter: Option<u8>,
    },
}

/// `sFTMainGroundObstacles[2]` / `sFTMainGroundHazards[1]` and their counts.
///
/// The source walks the first `num` slots, not every live one: clearing the
/// first of two obstacles hides the second until the count grows again.
/// Kept as is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Registry {
    obstacles: [Option<Obstacle>; 2],
    obstacles_num: usize,
    hazards: [Option<Hazard>; 1],
    hazards_num: usize,
}

impl Registry {
    /// `ftMainCheckAddGroundObstacle`.
    pub fn add_obstacle(&mut self, o: Obstacle) -> bool {
        if let Some(slot) = self.obstacles.iter_mut().find(|s| s.is_none()) {
            *slot = Some(o);
            self.obstacles_num += 1;
            true
        } else {
            false
        }
    }
    /// `ftMainClearGroundObstacle`.
    pub fn clear_obstacle(&mut self, o: Obstacle) {
        if let Some(slot) = self.obstacles.iter_mut().find(|s| **s == Some(o)) {
            *slot = None;
            self.obstacles_num -= 1;
        }
    }
    /// `ftMainCheckAddGroundHazard`.
    pub fn add_hazard(&mut self, h: Hazard) -> bool {
        if let Some(slot) = self.hazards.iter_mut().find(|s| s.is_none()) {
            *slot = Some(h);
            self.hazards_num += 1;
            true
        } else {
            false
        }
    }
    /// `ftMainClearHazard`.
    pub fn clear_hazard(&mut self, matches: impl Fn(Hazard) -> bool) {
        if let Some(slot) = self.hazards.iter_mut().find(|s| s.is_some_and(&matches)) {
            *slot = None;
            self.hazards_num -= 1;
        }
    }
    pub fn obstacles(&self) -> impl Iterator<Item = Obstacle> + '_ {
        self.obstacles[..self.obstacles_num]
            .iter()
            .flatten()
            .copied()
    }
    pub fn hazards(&self) -> impl Iterator<Item = Hazard> + '_ {
        self.hazards[..self.hazards_num].iter().flatten().copied()
    }
}

/// The map queries a controller needs.
pub struct MapQuery<'a, F> {
    /// The live collision, moving groups included.
    pub surfaces: F,
    /// `mpCollisionSetDObjNoID`: the group a line belongs to.
    pub line_group: &'a dyn Fn(u16) -> Option<u8>,
}

/// One frame's controller input beyond the fighters.
pub struct TickInput<'a, F> {
    pub groups: &'a mut [MapGroup],
    pub objects: &'a mut dyn StageObjects,
    pub items: &'a mut dyn StageItems,
    pub map: MapQuery<'a, F>,
    /// `gSCManagerBattleState->game_status != nSCBattleGameStatusWait`.
    pub started: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Controller {
    None,
    Castle(castle::Castle),
    Jungle(jungle::Jungle),
    Zebes(zebes::Zebes),
    Hyrule(hyrule::Hyrule),
    Yoster(yoster::Yoster),
    Pupupu(pupupu::Pupupu),
    Yamabuki(yamabuki::Yamabuki),
    Inishie(inishie::Inishie),
    Sector(sector::Sector),
    Bonus3(bonus3::Bonus3),
}

/// A stage's controller, its hazard registries and its file data.
#[derive(Debug, Clone, PartialEq)]
pub struct Stage {
    target_breaks: u8,
    pub controller: Controller,
    pub registry: Registry,
    pub attack: Option<GroundAttack>,
    pub throw: Option<HazardThrow>,
    /// The pipes' map points, which [`Stage::tick`] hands every fighter.
    pub dokan: crate::dokan::Points,
}

impl Stage {
    /// A stage with no controller (bonus stages, Training on unported
    /// stages, host tests).
    pub fn none() -> Self {
        Stage {
            target_breaks: 0,
            controller: Controller::None,
            registry: Registry::default(),
            attack: None,
            throw: None,
            dokan: crate::dokan::Points::default(),
        }
    }

    /// `grMainSetupMakeGround` for a VS stage or Race to the Finish, run by
    /// `grCommonSetupInitAll` after `mpCollisionClearYakumonoAll`.
    pub fn new(
        init: &StageInit<'_>,
        groups: &mut [MapGroup],
        objects: &mut dyn StageObjects,
        items: &mut dyn StageItems,
    ) -> Self {
        let mut registry = Registry::default();
        let controller = match init.kind {
            StageKind::Castle => Controller::Castle(castle::Castle::new(init, objects, items)),
            StageKind::Sector => {
                let mut none = NoArwing;
                let arwing = match objects.arwing() {
                    Some(a) => a,
                    None => &mut none,
                };
                Controller::Sector(sector::Sector::new(groups, arwing))
            }
            StageKind::Jungle => Controller::Jungle(jungle::Jungle::new(objects, &mut registry)),
            StageKind::Zebes => Controller::Zebes(zebes::Zebes::new(init, objects, &mut registry)),
            StageKind::Hyrule => Controller::Hyrule(hyrule::Hyrule::new(init)),
            StageKind::Yoster => Controller::Yoster(yoster::Yoster::new(groups, objects)),
            StageKind::Pupupu => Controller::Pupupu(pupupu::Pupupu::new()),
            StageKind::Yamabuki => Controller::Yamabuki(yamabuki::Yamabuki::with_monster_pos(
                groups,
                objects,
                items,
                objects_of(init.map_objects, mapobj::MONSTER)
                    .next()
                    .unwrap_or(Vec3::ZERO),
            )),
            StageKind::Inishie => {
                Controller::Inishie(inishie::Inishie::new(init, groups, objects, items))
            }
            StageKind::Bonus3 => Controller::Bonus3(bonus3::Bonus3::new(init, items)),
        };
        let point = |kind| objects_of(init.map_objects, kind).next();
        Stage {
            target_breaks: 0,
            controller,
            registry,
            attack: init.hazard_attack,
            throw: init.hazard_throw,
            dokan: crate::dokan::Points {
                left: point(crate::dokan::MAPOBJ_DOKAN_L),
                right: point(crate::dokan::MAPOBJ_DOKAN_R),
                wall: point(crate::dokan::MAPOBJ_DOKAN_WALL),
            },
        }
    }

    /// The stage's priority-4 processes, then the captor state fighters
    /// read in their own priority-4 physics.
    pub fn tick<I, F>(&mut self, fighters: &mut [&mut Fighter], input: TickInput<'_, F>)
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        let TickInput {
            groups,
            objects,
            items,
            map,
            started,
        } = input;
        // `grJungleTaruCannAddAnimShoot` from the fighter's own update, and
        // `grInishiePakkunSetWaitFighter` from a pipe entry: both reach the
        // stage objects before their own processes run.
        for f in fighters.iter_mut() {
            if core::mem::take(&mut f.hazard.shoot_request) {
                objects.play(StageAnim::TaruCannShoot);
            }
            if core::mem::take(&mut f.dokan.plant_request) {
                self.pakkun_set_wait_fighter(items);
            }
        }
        match &mut self.controller {
            Controller::None => {}
            Controller::Castle(c) => c.tick(objects, items),
            Controller::Jungle(c) => c.tick(),
            Controller::Zebes(c) => c.tick(started, objects),
            Controller::Hyrule(c) => c.tick(fighters, &mut self.registry, &map, started),
            Controller::Yoster(c) => c.tick(fighters, groups, objects, &map),
            Controller::Pupupu(c) => c.tick(fighters, objects, started),
            Controller::Yamabuki(c) => c.tick(fighters, groups, objects, items, started),
            Controller::Inishie(c) => c.tick(
                fighters,
                groups,
                objects,
                items,
                &map,
                started,
                &mut self.registry,
            ),
            Controller::Sector(c) => {
                let mut none = NoArwing;
                let arwing = match objects.arwing() {
                    Some(a) => a,
                    None => &mut none,
                };
                c.tick(fighters, groups, arwing, &map, started)
            }
            Controller::Bonus3(c) => c.tick(fighters, items),
        }
        self.publish(fighters, objects);
    }

    /// Captured fighters follow their captor's `DObj` in `proc_physics`.
    fn publish(&self, fighters: &mut [&mut Fighter], objects: &dyn StageObjects) {
        for f in fighters.iter_mut() {
            f.hazard.throw = self.throw;
            f.dokan.points = self.dokan;
            match f.status.status {
                crate::status::AnyStatus::Common(crate::status::Status::Twister) => {
                    if let Controller::Hyrule(h) = &self.controller {
                        f.hazard.anchor = h.twister_pos;
                    }
                }
                crate::status::AnyStatus::Common(crate::status::Status::TaruCann) => {
                    if let Controller::Jungle(j) = &self.controller {
                        f.hazard.anchor = objects.translate(StageObj::TaruCann);
                        f.hazard.barrel_rotate = j.rotate;
                    }
                }
                _ => {}
            }
        }
    }

    /// Makes the effects this frame's stage process made
    /// (`grYosterCloudVaporMakeEffect`, `efManagerSparkleWhiteScaleMakeEffect`),
    /// in order. The runtime calls it right after [`Stage::tick`], where the
    /// source makes them.
    pub fn flush_effects(&mut self, sink: &mut dyn crate::effect::HitEffectSink) {
        let fx = match &mut self.controller {
            Controller::Yoster(c) => core::mem::take(&mut c.fx),
            Controller::Inishie(c) => core::mem::take(&mut c.fx),
            _ => return,
        };
        for e in fx.iter() {
            sink.weapon(&e);
        }
    }

    /// Delivers the stage calls this frame's items made, in order.
    pub fn apply_item_events(
        &mut self,
        events: impl IntoIterator<Item = crate::item::StageItemEvent>,
        groups: &[MapGroup],
        objects: &mut dyn StageObjects,
    ) {
        use crate::item::StageItemEvent;
        for e in events {
            if e == StageItemEvent::TargetBroken {
                self.target_breaks += 1;
            }
            if let Controller::Yamabuki(c) = &mut self.controller {
                match e {
                    StageItemEvent::MonsterClose => c.set_closed_wait(groups, objects),
                    StageItemEvent::MonsterClear => c.clear_monster(),
                    _ => {}
                }
            }
            if let Controller::Inishie(c) = &mut self.controller {
                match e {
                    StageItemEvent::PowerBlockDamage { handicap, hitter } => {
                        c.set_power_block_damage(&mut self.registry, handicap, hitter)
                    }
                    StageItemEvent::PowerBlockGone => c.power_block_gone(),
                    _ => {}
                }
            }
        }
    }

    /// `grInishiePakkunSetWaitFighter`, from `ftCommonDokanStartSetStatus`
    /// ([`crate::dokan::DokanState::plant_request`]).
    pub fn pakkun_set_wait_fighter(&self, items: &mut dyn StageItems) {
        if let Controller::Inishie(c) = &self.controller {
            for h in c.pakkun.into_iter().flatten() {
                items.pakkun_set_wait_fighter(h);
            }
        }
    }

    /// The target damage callbacks since the bonus scene last observed them.
    pub fn take_target_breaks(&mut self) -> u8 {
        core::mem::take(&mut self.target_breaks)
    }

    /// The Arwing lasers this frame's tick made, for the weapon pool
    /// (`wpManagerMakeWeapon` at the controller's own time).
    pub fn take_lasers(&mut self) -> impl Iterator<Item = crate::weapon::ArwingLaser> {
        let lasers = match &mut self.controller {
            Controller::Sector(s) => Some(s.take_lasers()),
            _ => None,
        };
        lasers.into_iter().flatten()
    }

    /// `grJungleTaruCannGetRotate`.
    pub fn barrel_rotate(&self) -> f32 {
        match &self.controller {
            Controller::Jungle(j) => j.rotate,
            _ => 0.0,
        }
    }

    /// The obstacle loop of `ftMainSearchHitHazard`: the first obstacle
    /// whose check passes, with its current position.
    pub fn check_obstacles(
        &mut self,
        f: &Fighter,
        objects: &mut dyn StageObjects,
        barrel_taken: bool,
    ) -> Option<(i32, Vec3)> {
        let mut hit = None;
        for o in self.registry.obstacles() {
            let result = match (o, &self.controller) {
                (Obstacle::Twister, Controller::Hyrule(h)) => h
                    .check_twister(f)
                    .then_some((crate::hazard::ENV_TWISTER, h.twister_pos)),
                (Obstacle::TaruCann, Controller::Jungle(_)) => {
                    let pos = objects.translate(StageObj::TaruCann);
                    jungle::check_tarucann(f, pos, barrel_taken).then(|| {
                        objects.play(StageAnim::TaruCannFill);
                        (crate::hazard::ENV_TARUCANN, pos)
                    })
                }
                _ => None,
            };
            // Each passing check sets the status in turn; the last wins.
            if result.is_some() {
                hit = result;
            }
        }
        hit
    }

    /// The hazard loop of `ftMainSearchGroundHit`.
    pub fn check_hazards(&self, f: &Fighter) -> Option<(GroundAttack, u8)> {
        let mut hit = None;
        for h in self.registry.hazards() {
            let result = match (h, &self.controller) {
                (Hazard::Acid, Controller::Zebes(z)) => z
                    .check_acid(f)
                    .then_some(self.attack)
                    .flatten()
                    .map(|a| (a, crate::hazard::GROUND_HANDICAP)),
                // `grInishiePowerBlockCheckGetDamageKind`.
                (Hazard::PowerBlock { handicap, hitter }, _) => (f.is_grounded()
                    && hitter != Some(f.port))
                .then_some(self.attack)
                .flatten()
                .map(|a| (a, handicap)),
                _ => None,
            };
            if result.is_some() {
                hit = result;
            }
        }
        hit
    }
}

/// Every fighter's `joints[TopN]->translate`.
fn top_n(f: &Fighter) -> Vec3 {
    f.pos
}

/// The floor group a grounded fighter stands on (`floor_line_id != -2`).
fn standing_group<F>(f: &Fighter, map: &MapQuery<'_, F>) -> Option<u8> {
    if !f.is_grounded() {
        return None;
    }
    (map.line_group)(f.floor?.line)
}
