//! Item-made weapons (`WEAPON_FLAG_PARENT_ITEM`): Saffron City's Charmander
//! flame and Venusaur razor (`ithitokage.c`, `itfushigibana.c`) and the
//! Poké Ball Pokémon's (`itmonster/`): Onix's rocks, Meowth's coins,
//! Charizard's flame, the Beedrill and Clefairy swarms, Blastoise's Hydro
//! Pump, Starmie's Swift and Koffing's Smog (RE-435).
//!
//! A weapon takes its parent item's owner, player, team and facing. The
//! Hydro Pump's attack offset and the Smog's size follow their own `DObj`
//! animations; both scripts are fixed, so the port keeps their per-play
//! values ([`hydro_offset_x`], [`SMOG_SCALE`]) and a ROM test replays them.

use crate::attack::Hitbox;
use crate::combat::Element;
use crate::ground::BodyColl;
use crate::status::BlastZone;
use crate::weapon::{map_contact, MapSurface};
use crate::wpeffect::{Emit, WeaponEffect as Fx};
use ssb_engine::math::{atan2, sin_cos, Vec2, Vec3};

/// `nWPKindMonsterStart` .. `nWPKindMonsterEnd`, with the two swarm
/// descriptors apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShotKind {
    HitokageFlame,
    FushigibanaRazor,
    IwarkRock,
    NyarsCoin,
    LizardonFlame,
    SpearSwarm,
    PippiSwarm,
    KamexHydro,
    StarmieSwift,
    DogasSmog,
    RayGun,
    StarRod,
    FireFlower,
}

/// The `WPAttributes` fields gameplay reads.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShotAttributes {
    pub map_coll: BodyColl,
    /// Diameter; the attack is half of this.
    pub size: f32,
    pub angle: i32,
    pub kb_scale: i32,
    pub damage: i32,
    pub element: Element,
    pub kb_weight: i32,
    pub shield_damage: i32,
    pub can_setoff: bool,
    pub can_rehit_fighter: bool,
    pub can_rehit_item: bool,
    pub can_hop: bool,
    pub can_reflect: bool,
    pub can_absorb: bool,
    pub can_shield: bool,
    pub kb_base: i32,
}

const fn coll(top: f32, bottom: f32, width: f32) -> BodyColl {
    BodyColl {
        top,
        center: 0.0,
        bottom,
        width,
    }
}

const BASE: ShotAttributes = ShotAttributes {
    map_coll: coll(0.0, 0.0, 0.0),
    size: 0.0,
    angle: 0,
    kb_scale: 100,
    damage: 0,
    element: Element::Normal,
    kb_weight: 0,
    shield_damage: 1,
    can_setoff: true,
    can_rehit_fighter: false,
    can_rehit_item: false,
    can_hop: false,
    can_reflect: false,
    can_absorb: false,
    can_shield: true,
    kb_base: 0,
};

/// File 264 + 0x244 and 0x308 (Saffron); file 251 + 0x774, 0x8C8, 0x944,
/// 0x9D4, 0xCBC, 0xA50, 0xB7C and 0xC40 (`reloc_data.us.h`).
pub static ATTRIBUTES: [ShotAttributes; 13] = [
    ShotAttributes {
        map_coll: coll(50.0, -50.0, 50.0),
        size: 320.0,
        damage: 2,
        element: Element::Fire,
        kb_weight: 3,
        can_setoff: false,
        can_reflect: true,
        can_absorb: true,
        ..BASE
    },
    ShotAttributes {
        map_coll: coll(120.0, -120.0, 360.0),
        size: 200.0,
        angle: 90,
        kb_scale: 60,
        damage: 3,
        kb_base: 30,
        can_hop: true,
        can_reflect: true,
        can_absorb: true,
        ..BASE
    },
    ShotAttributes {
        map_coll: coll(210.0, -210.0, 210.0),
        size: 360.0,
        angle: 80,
        kb_scale: 80,
        damage: 8,
        can_hop: true,
        kb_base: 30,
        ..BASE
    },
    ShotAttributes {
        map_coll: coll(113.0, -113.0, 94.0),
        size: 200.0,
        angle: 90,
        kb_scale: 60,
        damage: 6,
        element: Element::Coin,
        shield_damage: 2,
        can_hop: true,
        can_reflect: true,
        can_absorb: true,
        kb_base: 30,
        ..BASE
    },
    ShotAttributes {
        map_coll: coll(4.0, -4.0, 4.0),
        size: 320.0,
        damage: 4,
        element: Element::Fire,
        kb_weight: 8,
        can_setoff: false,
        can_reflect: true,
        can_absorb: true,
        ..BASE
    },
    ShotAttributes {
        map_coll: coll(244.0, -244.0, 244.0),
        size: 300.0,
        angle: 80,
        damage: 12,
        kb_base: 40,
        ..BASE
    },
    ShotAttributes {
        map_coll: coll(192.0, -192.0, 192.0),
        size: 200.0,
        angle: 80,
        damage: 12,
        can_rehit_fighter: true,
        kb_base: 40,
        ..BASE
    },
    ShotAttributes {
        map_coll: coll(213.0, -213.0, 180.0),
        size: 200.0,
        kb_scale: 30,
        damage: 6,
        kb_base: 78,
        ..BASE
    },
    ShotAttributes {
        map_coll: coll(113.0, -113.0, 309.0),
        size: 200.0,
        angle: 100,
        kb_scale: 20,
        damage: 3,
        can_hop: true,
        can_reflect: true,
        can_absorb: true,
        kb_base: 40,
        ..BASE
    },
    ShotAttributes {
        map_coll: coll(75.0, -75.0, 75.0),
        size: 50.0,
        angle: 150,
        damage: 3,
        kb_weight: 60,
        can_shield: false,
        ..BASE
    },
    ShotAttributes {
        map_coll: coll(10.0, -10.0, 10.0),
        size: 120.0,
        angle: 70,
        kb_scale: 40,
        damage: 10,
        element: Element::Electric,
        shield_damage: -8,
        can_setoff: false,
        can_hop: true,
        can_reflect: true,
        can_absorb: true,
        kb_base: 50,
        ..BASE
    },
    ShotAttributes {
        map_coll: BodyColl {
            top: 10.0,
            center: -10.0,
            bottom: -10.0,
            width: 10.0,
        },
        size: 200.0,
        angle: 361,
        damage: 8,
        can_hop: true,
        can_reflect: true,
        can_absorb: true,
        kb_base: 10,
        ..BASE
    },
    ShotAttributes {
        map_coll: coll(50.0, -50.0, 50.0),
        size: 270.0,
        damage: 3,
        can_rehit_item: true,
        element: Element::Fire,
        kb_weight: 3,
        can_setoff: false,
        can_reflect: true,
        can_absorb: true,
        ..BASE
    },
];

/// `ITKAMEX_HYDRO_LIFETIME`, `ITSTARMIE_SWIFT_LIFETIME`,
/// `ITDOGAS_SMOG_LIFETIME`, `ITLIZARDON_FLAME_LIFETIME`,
/// `ITNYARS_COIN_LIFETIME`.
pub const HYDRO_LIFETIME: u16 = 20;
pub const SWIFT_LIFETIME: u16 = 30;
pub const SMOG_LIFETIME: u16 = 30;
pub const LIZARDON_FLAME_LIFETIME: u16 = 30;
pub const COIN_LIFETIME: u16 = 10;
/// `ITNYARS_COIN_VEL_X`, `ITSTARMIE_SWIFTVEL_X`, `ITSPEAR_SWARM_FLY_VEL_X`.
pub const COIN_VEL_X: f32 = 130.0;
pub const SWIFT_VEL_X: f32 = 150.0;
pub const SWARM_VEL_X: f32 = 130.0;
/// `ITSPEAR_SWARM_CALL_OFF_X`: a swarm member is gone this far inside the
/// side bound it flies toward.
pub const SWARM_DESPAWN_OFF_X: f32 = 500.0;
/// `WPIWARK_ROCK_*`.
pub const ROCK_GRAVITY: f32 = 2.0;
pub const ROCK_TVEL: f32 = 200.0;
pub const ROCK_ROTATE_STEP: f32 = -0.5;
pub const ROCK_VEL_Y_START: [f32; 3] = [-100.0, -50.0, 0.0];
pub const ROCK_COLLIDE_MUL_VEL: f32 = 0.1;
pub const ROCK_COLLIDE_ADD_Y: f32 = -150.0;

/// The Hydro Pump's child `DObj` X after `play` plays of
/// `llITCommonDataKamexHydro` (file 86 + 0xFA9C); the first is the make
/// frame's. Its attack offset is this times the facing.
pub fn hydro_offset_x(play: u16) -> f32 {
    60.0 + 144.375 * f32::from(play)
}

/// The Smog's child `DObj` X scale over its 30 plays (file 86 + 0x13198);
/// its attack radius is this times `WPAttributes::size` (50).
pub const SMOG_SCALE: [f32; 30] = [
    1.0,
    1.0,
    1.685_911_3,
    2.302_642_8,
    2.853_437,
    3.341_536_3,
    3.770_181_7,
    4.142_616_7,
    4.462_083,
    4.731_823,
    4.955_078_6,
    5.135_091,
    5.275_103_6,
    5.378_359,
    5.448_098_7,
    5.487_565,
    5.5,
    5.5,
    5.5,
    5.5,
    5.5,
    5.5,
    5.5,
    5.5,
    5.5,
    5.5,
    5.5,
    5.5,
    5.5,
    5.5,
];

/// The item a weapon is made from: `wpManagerMakeWeapon`'s
/// `WEAPON_FLAG_PARENT_ITEM` copies.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShotParent {
    pub stat: crate::spgame::live::AttackStat,
    pub owner: Option<u8>,
    pub player: Option<u8>,
    pub team: u8,
    pub lr: f32,
    /// The parent's pool handle (Onix's rocks report their end to it).
    pub handle: u32,
}

impl ShotParent {
    /// A stage item's: no owner or player, the default team.
    pub const GROUND: ShotParent = ShotParent {
        stat: crate::spgame::live::AttackStat {
            flags: crate::spgame::live::Flags(0),
            count: 0,
        },
        owner: None,
        player: None,
        team: crate::team::TEAM_DEFAULT,
        lr: 1.0,
        handle: u32::MAX,
    };
}

/// Which collision callback runs ([`MonsterShot::survives`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShotProc {
    Hit,
    Shield,
    SetOff,
    Absorb,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MonsterShot {
    pub stat: crate::spgame::live::AttackStat,
    /// Ray Gun root X scale; its second attack is -5 local X.
    pub scale_x: f32,
    pub attack_tail: Option<(Vec3, Vec3)>,
    /// Star Rod descriptor remains on smash attributes after a smash.
    pub star_smash: bool,
    pub kind: ShotKind,
    /// `owner_gobj`: the fighter it never hits.
    pub owner: Option<u8>,
    /// `wp->player`: who its hits credit.
    pub player: Option<u8>,
    pub team: u8,
    pub damage: i32,
    pub position: Vec3,
    pub velocity: Vec3,
    /// `wp->lifetime`, or the coin's `weapon_vars.coin.lifetime`.
    pub lifetime: u16,
    pub lr: f32,
    /// The root `DObj`'s `rotate.z` and `rotate.y`. Presentation only.
    pub rotate_z: f32,
    pub rotate_y: f32,
    /// `gcPlayAnimAll` calls since the make.
    pub plays: u16,
    /// `attack_coll.size`, the radius.
    pub size: f32,
    /// `attack_coll.offsets[0].x` (the Hydro Pump's).
    pub offset_x: f32,
    /// `attack_pos[0]`: current and previous, once positioned.
    pub attack: Option<(Vec3, Vec3)>,
    /// `weapon_vars.rock.floor_line_id`.
    pub floor_line: Option<u16>,
    /// The rock's `texture_id_curr`.
    pub texture: u8,
    /// `weapon_vars.rock.owner_gobj`.
    pub parent: u32,
    /// The rock met a new floor this update: Onix's `rumble_frame` counts
    /// it ([`crate::weapon::WeaponPool::take_rock_events`]).
    pub rumbled: bool,
}

impl MonsterShot {
    fn make(kind: ShotKind, parent: ShotParent, position: Vec3) -> Self {
        let attr = &ATTRIBUTES[kind as usize];
        Self {
            stat: parent.stat,
            kind,
            scale_x: 1.0,
            attack_tail: Some((position, position)),
            star_smash: false,
            owner: parent.owner,
            player: parent.player,
            team: parent.team,
            damage: attr.damage,
            position,
            velocity: Vec3::ZERO,
            lifetime: 0,
            lr: parent.lr,
            rotate_z: 0.0,
            rotate_y: 0.0,
            plays: 0,
            size: attr.size * 0.5,
            offset_x: 0.0,
            // `wpManagerMakeWeapon` plays `wpProcessUpdateHitPositions`
            // before returning: the first process sweeps from this point.
            attack: Some((position, position)),
            floor_line: None,
            texture: 0,
            parent: parent.handle,
            rumbled: false,
        }
    }

    /// Saffron's flame (`razor` false) or razor, from a ground Pokémon.
    pub fn saffron(razor: bool, position: Vec3) -> Self {
        let (sin, cos) = sin_cos(-0.209_439_52);
        let kind = if razor {
            ShotKind::FushigibanaRazor
        } else {
            ShotKind::HitokageFlame
        };
        Self {
            velocity: if razor {
                Vec3::new(-100.0, 0.0, 0.0)
            } else {
                Vec3::new(cos * -45.0, sin * 45.0, 0.0)
            },
            lifetime: if razor { 24 } else { 20 },
            lr: -1.0,
            team: crate::weapon::sector::GROUND_TEAM,
            ..Self::make(kind, ShotParent::GROUND, position)
        }
    }

    /// `itIwarkWeaponRockMakeWeapon`, after the pool has a free struct:
    /// `random` picks the fall speed and the texture; the facing is drawn
    /// here.
    pub fn rock(parent: ShotParent, position: Vec3, random: u8) -> Self {
        let lr = if crate::rng::rand_int_range(2) == 0 {
            -1.0
        } else {
            1.0
        };
        Self {
            velocity: Vec3::new(0.0, ROCK_VEL_Y_START[usize::from(random.min(2))], 0.0),
            lr,
            texture: random,
            ..Self::make(ShotKind::IwarkRock, parent, position)
        }
    }

    /// `itNyarsWeaponCoinMakeWeapon`: `angle` in degrees.
    pub fn coin(parent: ShotParent, position: Vec3, coin_number: u8, angle: f32) -> Self {
        let rad = (f32::from(coin_number) * 90.0 + angle) * core::f32::consts::PI / 180.0;
        let (sin, cos) = sin_cos(rad);
        Self {
            lifetime: COIN_LIFETIME,
            velocity: Vec3::new(COIN_VEL_X * cos, COIN_VEL_X * sin, 0.0),
            ..Self::make(ShotKind::NyarsCoin, parent, position)
        }
    }

    /// `itLizardonWeaponFlameMakeWeapon`.
    pub fn lizardon_flame(parent: ShotParent, position: Vec3, velocity: Vec3) -> Self {
        Self {
            velocity,
            lifetime: LIZARDON_FLAME_LIFETIME,
            ..Self::make(ShotKind::LizardonFlame, parent, position)
        }
    }

    /// `itSpearWeaponSwarmMakeWeapon`: Beedrill's swarm, or Clefairy's.
    pub fn swarm(parent: ShotParent, position: Vec3, pippi: bool) -> Self {
        let kind = if pippi {
            ShotKind::PippiSwarm
        } else {
            ShotKind::SpearSwarm
        };
        let lr = -parent.lr;
        let turned = if pippi { lr == 1.0 } else { lr == -1.0 };
        Self {
            lr,
            velocity: Vec3::new(lr * SWARM_VEL_X, 0.0, 0.0),
            rotate_y: if turned { core::f32::consts::PI } else { 0.0 },
            ..Self::make(kind, parent, position)
        }
    }

    /// `itKamexWeaponHydroMakeWeapon`. It stays where it is made.
    pub fn hydro(parent: ShotParent, position: Vec3) -> Self {
        Self {
            lifetime: HYDRO_LIFETIME,
            rotate_y: if parent.lr == -1.0 {
                core::f32::consts::PI
            } else {
                0.0
            },
            ..Self::make(ShotKind::KamexHydro, parent, position)
        }
    }

    /// `itStarmieWeaponSwiftMakeWeapon`.
    pub fn swift(parent: ShotParent, position: Vec3) -> Self {
        Self {
            velocity: Vec3::new(parent.lr * SWIFT_VEL_X, 0.0, 0.0),
            lifetime: SWIFT_LIFETIME,
            rotate_y: if parent.lr == 1.0 {
                core::f32::consts::PI
            } else {
                0.0
            },
            ..Self::make(ShotKind::StarmieSwift, parent, position)
        }
    }

    /// `itDogasWeaponSmogMakeWeapon`.
    pub fn smog(parent: ShotParent, position: Vec3, velocity: Vec3) -> Self {
        Self {
            velocity,
            lifetime: SMOG_LIFETIME,
            ..Self::make(ShotKind::DogasSmog, parent, position)
        }
    }

    /// Fighter-owned item weapon; the caller handles ammo even when allocation fails.
    pub fn equipment(
        kind: ShotKind,
        parent: ShotParent,
        position: Vec3,
        smash: bool,
        angle_index: u8,
    ) -> Self {
        let mut m = Self::make(kind, parent, position);
        match kind {
            ShotKind::RayGun => {
                m.attack_tail = Some((
                    position + Vec3::new(-5.0, 0.0, 0.0),
                    position + Vec3::new(-5.0, 0.0, 0.0),
                ));
                m.velocity.x = parent.lr * 300.0;
                m.rotate_z = atan2(0.0, m.velocity.x);
            }
            ShotKind::StarRod => {
                m.position.z = 0.0;
                m.velocity.x = parent.lr * if smash { 120.0 } else { 80.0 };
                m.lifetime = 30;
            }
            ShotKind::FireFlower => {
                let angle =
                    (f32::from(angle_index.min(4)) * 7.5 - 15.0) * core::f32::consts::PI / 180.0;
                let (s, c) = sin_cos(angle);
                m.velocity = Vec3::new(c * 30.0 * parent.lr, s * 30.0, 0.0);
                m.lifetime = 30;
            }
            _ => unreachable!(),
        }
        m
    }

    pub fn attributes(&self) -> &'static ShotAttributes {
        &ATTRIBUTES[self.kind as usize]
    }

    /// Saffron's Venusaur razor.
    pub fn is_razor(&self) -> bool {
        self.kind == ShotKind::FushigibanaRazor
    }

    pub fn hitbox(&self) -> Hitbox {
        let a = self.attributes();
        Hitbox {
            damage: self.damage,
            offset: Vec3::ZERO,
            radius: self.size,
            angle: a.angle,
            kb_scale: a.kb_scale,
            kb_weight: a.kb_weight,
            kb_base: a.kb_base,
            element: a.element,
            shield_damage: a.shield_damage,
        }
    }

    /// `wp->is_hitlag_victim`: the rocks and swarms make the victim's hit
    /// effect.
    pub fn is_hitlag_victim(&self) -> bool {
        matches!(
            self.kind,
            ShotKind::IwarkRock | ShotKind::SpearSwarm | ShotKind::PippiSwarm
        )
    }

    /// The attack's current and previous centres.
    pub fn attack_positions(&self) -> (Vec3, Vec3) {
        self.attack
            .unwrap_or((self.attack_point(), self.attack_point()))
    }

    /// `wpProcessUpdateHitOffsets`: a non-zero offset turns with the root.
    fn attack_point(&self) -> Vec3 {
        if self.offset_x == 0.0 {
            return self.position;
        }
        let (sin, cos) = sin_cos(self.rotate_z);
        self.position + Vec3::new(self.offset_x * cos, self.offset_x * sin, 0.0)
    }

    /// `wpProcessUpdateHitPositions`.
    fn update_attack(&mut self) {
        let curr = self.attack_point();
        if self.kind == ShotKind::RayGun {
            let (sin, cos) = sin_cos(self.rotate_z);
            let tail = self.position
                + Vec3::new(-5.0 * self.scale_x * cos, -5.0 * self.scale_x * sin, 0.0);
            self.attack_tail = Some((tail, self.attack_tail.map_or(tail, |(p, _)| p)));
        }
        self.attack = Some(match self.attack {
            None => (curr, curr),
            Some((prev, _)) => (curr, prev),
        });
    }

    /// The make's own effects, at the weapon's position.
    pub(crate) fn make_fx(&self, fx: &mut Emit) {
        match self.kind {
            ShotKind::HitokageFlame | ShotKind::LizardonFlame | ShotKind::FireFlower => {
                fx.push(Fx::MonsterFlame {
                    pos: self.position,
                    vel: self.velocity,
                })
            }
            ShotKind::KamexHydro | ShotKind::StarmieSwift => {
                fx.push(Fx::SparkleWhiteScale(self.position));
            }
            _ => {}
        }
    }

    /// The effects `proc` makes before it decides the weapon's fate.
    pub(crate) fn proc_fx(&self, proc: ShotProc, fx: &mut Emit) {
        let pos = self.position;
        match self.kind {
            ShotKind::FushigibanaRazor => fx.push(Fx::DamageSlash {
                pos,
                size: self.damage,
                lr: self.lr,
            }),
            // No `proc_absorb`.
            // `it{Hitokage,Lizardon}WeaponFlameProcHit`.
            ShotKind::HitokageFlame | ShotKind::LizardonFlame if proc != ShotProc::Absorb => {
                crate::sound::play_fgm(crate::sound::id::nSYAudioFGMExplodeS);
                fx.push(Fx::SparkleWhite(pos));
            }
            ShotKind::NyarsCoin => fx.push(Fx::DamageCoin(pos)),
            ShotKind::RayGun => fx.push(Fx::ImpactShock {
                pos,
                size: self.damage,
            }),
            // `itFFlowerWeaponFlameProcHit`.
            ShotKind::FireFlower if proc != ShotProc::Absorb => {
                crate::sound::play_fgm(crate::sound::id::nSYAudioFGMExplodeS);
                fx.push(Fx::SparkleWhite(pos));
            }
            ShotKind::StarmieSwift | ShotKind::StarRod => fx.push(Fx::StarSplash {
                pos,
                lr: self.lr as i8,
            }),
            _ => {}
        }
    }

    /// Whether the weapon lives on after `proc`: a kind without the
    /// callback, or whose callback returns `FALSE`, does.
    pub fn survives(&self, proc: ShotProc) -> bool {
        match self.kind {
            ShotKind::FushigibanaRazor
            | ShotKind::NyarsCoin
            | ShotKind::StarmieSwift
            | ShotKind::RayGun
            | ShotKind::StarRod => false,
            ShotKind::HitokageFlame
            | ShotKind::LizardonFlame
            | ShotKind::IwarkRock
            | ShotKind::SpearSwarm
            | ShotKind::PippiSwarm
            | ShotKind::KamexHydro
            | ShotKind::DogasSmog
            | ShotKind::FireFlower => {
                let _ = proc;
                true
            }
        }
    }

    /// `proc_hop`: every hop turns the velocity; the kinds rederive their
    /// facing (and model roll).
    pub(crate) fn hop(&mut self, velocity: Vec3) {
        self.velocity = velocity;
        match self.kind {
            ShotKind::FushigibanaRazor => {
                self.reface();
                self.lr = if self.velocity.x > 0.0 { 1.0 } else { -1.0 };
            }
            ShotKind::RayGun => {
                self.rotate_z = atan2(self.velocity.y, self.velocity.x);
                self.scale_x = 1.0;
            }
            ShotKind::IwarkRock
            | ShotKind::NyarsCoin
            | ShotKind::StarmieSwift
            | ShotKind::StarRod => {
                self.scale_x = 1.0;
                self.rotate_z = atan2(self.velocity.y, self.velocity.x);
                self.lr = if self.velocity.x > 0.0 { 1.0 } else { -1.0 };
            }
            _ => {}
        }
    }

    /// `wpProcessProcHitCollisions`'s reflection: the reflector takes the
    /// weapon, its `proc_reflector` runs, then the damage bonus.
    /// `reflector_lr` is the reflector's facing. The make-style effects go
    /// to `fx`.
    pub(crate) fn reflect(&mut self, port: u8, team: u8, reflector_lr: f32, fx: &mut Emit) {
        self.owner = Some(port);
        self.player = Some(port);
        self.team = team;
        // `wpMainReflectorSetLR`.
        if self.velocity.x * reflector_lr < 0.0 {
            self.velocity.x = -self.velocity.x;
        }
        match self.kind {
            ShotKind::FushigibanaRazor => {
                self.reface();
                self.lr = -self.lr;
            }
            ShotKind::HitokageFlame => {
                self.lifetime = 20;
                self.make_fx(fx);
            }
            ShotKind::RayGun => {
                self.rotate_z = atan2(self.velocity.y, self.velocity.x);
                self.scale_x = 1.0;
            }
            ShotKind::FireFlower => {
                self.lifetime = 30;
                self.make_fx(fx);
            }
            ShotKind::LizardonFlame => {
                self.lifetime = LIZARDON_FLAME_LIFETIME;
                self.make_fx(fx);
            }
            ShotKind::IwarkRock
            | ShotKind::NyarsCoin
            | ShotKind::KamexHydro
            | ShotKind::StarmieSwift
            | ShotKind::StarRod => {
                self.scale_x = 1.0;
                self.rotate_z = atan2(self.velocity.y, self.velocity.x);
                self.lr = -self.lr;
            }
            _ => {}
        }
        self.damage = ((self.damage as f32 * 1.8 + 0.99) as i32).min(100);
    }

    /// `wpProcessProcWeaponMain` without the blast-zone test the pool keeps:
    /// the play, `proc_update`, the move, the bounds and `proc_map`, then
    /// the attack position. Returns whether the weapon lives on.
    pub(crate) fn tick<I, F>(
        &mut self,
        surfaces: F,
        bounds: Option<BlastZone>,
        fx: &mut Emit,
    ) -> bool
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        self.plays = self.plays.saturating_add(1);
        let play = self.plays - 1;
        // `proc_update`.
        match self.kind {
            ShotKind::HitokageFlame | ShotKind::FushigibanaRazor => {
                if self.kind == ShotKind::FushigibanaRazor {
                    self.velocity.x += 5.0 * self.lr;
                }
                self.lifetime = self.lifetime.saturating_sub(1);
                if self.lifetime == 0 {
                    return false;
                }
            }
            ShotKind::RayGun => self.scale_x = (self.scale_x + 10.0).min(160.0 / 3.0),
            ShotKind::StarRod => {
                if self.lifetime == 0 {
                    fx.push(Fx::SparkleWhiteScale(self.position));
                    return false;
                }
                self.lifetime -= 1;
                self.rotate_z += -0.2 * self.lr;
                if !self.lifetime.is_multiple_of(2) {
                    let pos = Vec3::new(
                        self.position.x,
                        self.position.y - 125.0 + crate::rng::rand_int_range(250) as f32,
                        0.0,
                    );
                    fx.push(Fx::StarRodSpark { pos, lr: -self.lr });
                }
            }
            ShotKind::IwarkRock => {
                self.velocity.y -= ROCK_GRAVITY;
                let mag = Vec2::new(self.velocity.x, self.velocity.y).length();
                if mag > ROCK_TVEL {
                    self.velocity.x = self.velocity.x / mag * ROCK_TVEL;
                    self.velocity.y = self.velocity.y / mag * ROCK_TVEL;
                }
                self.rotate_z += ROCK_ROTATE_STEP;
            }
            // `itNyarsWeaponCoinProcUpdate` tests before it counts down.
            ShotKind::NyarsCoin => {
                if self.lifetime == 0 {
                    return false;
                }
                self.lifetime -= 1;
            }
            ShotKind::SpearSwarm | ShotKind::PippiSwarm => {
                if let Some(b) = bounds {
                    if (self.lr == 1.0 && self.position.x >= b.right - SWARM_DESPAWN_OFF_X)
                        || (self.lr == -1.0 && self.position.x <= b.left + SWARM_DESPAWN_OFF_X)
                    {
                        return false;
                    }
                }
            }
            ShotKind::KamexHydro
            | ShotKind::StarmieSwift
            | ShotKind::LizardonFlame
            | ShotKind::DogasSmog
            | ShotKind::FireFlower => {
                match self.kind {
                    ShotKind::KamexHydro => self.offset_x = hydro_offset_x(play) * self.lr,
                    ShotKind::DogasSmog => {
                        let scale = SMOG_SCALE[usize::from(play).min(SMOG_SCALE.len() - 1)];
                        self.size = scale * self.attributes().size;
                    }
                    _ => {}
                }
                // `wpMainDecLifeCheckExpire`.
                self.lifetime = self.lifetime.wrapping_sub(1);
                if self.lifetime == 0 {
                    return false;
                }
            }
        }
        let prev = self.position;
        self.position += self.velocity;
        if let Some(b) = bounds {
            if crate::weapon::out_of_bounds(b, self.position) {
                return false;
            }
        }
        // `proc_map`.
        match self.kind {
            ShotKind::HitokageFlame
            | ShotKind::LizardonFlame
            | ShotKind::RayGun
            | ShotKind::FireFlower
            | ShotKind::StarRod => {
                let c = self.attributes().map_coll;
                if let Some(hit) = map_contact(surfaces(), prev, self.position, c) {
                    self.position = hit.position;
                    if self.kind == ShotKind::StarRod {
                        // `itStarRodWeaponStarProcMap`.
                        fx.push(Fx::StarSplash {
                            pos: self.position,
                            lr: self.lr as i8,
                        });
                        crate::sound::play_fgm(crate::sound::id::nSYAudioFGMStarMapCollide);
                    } else {
                        fx.push(Fx::DustExpandSmall(self.position));
                    }
                    return false;
                }
            }
            ShotKind::IwarkRock => self.rock_map(&surfaces, prev, fx),
            _ => {}
        }
        self.update_attack();
        true
    }

    /// `itIwarkWeaponRockProcMap`: `wpMapTestAllCheckCollEnd` only notes a
    /// floor the rock's bottom crossed this frame (it does not stop the
    /// rock); a floor it had not met before turns its velocity at a tenth
    /// and counts toward Onix's rumble.
    fn rock_map<I, F>(&mut self, surfaces: &F, prev: Vec3, fx: &mut Emit)
    where
        F: Fn() -> I,
        I: IntoIterator<Item = MapSurface>,
    {
        let bottom = self.attributes().map_coll.bottom;
        let from = Vec2::new(prev.x, prev.y + bottom);
        let to = Vec2::new(self.position.x, self.position.y + bottom);
        let Some((hit, _)) = crate::map::floor_crossing(surfaces, from, to) else {
            return;
        };
        if self.floor_line == Some(hit.line) {
            return;
        }
        crate::item::reflect_2d(&mut self.velocity, hit.normal);
        self.velocity.x *= ROCK_COLLIDE_MUL_VEL;
        self.velocity.y *= ROCK_COLLIDE_MUL_VEL;
        self.floor_line = Some(hit.line);
        crate::sound::play_fgm(crate::sound::id::nSYAudioFGMIwarkRockMake);
        let pos = self.position + Vec3::new(0.0, ROCK_COLLIDE_ADD_Y, 0.0);
        fx.push(Fx::DustLight {
            pos,
            lr: self.lr as i8,
        });
        self.lr = -self.lr;
        self.rumbled = true;
    }

    pub(crate) fn reface(&mut self) {
        self.rotate_z = atan2(self.velocity.y, self.velocity.x) + core::f32::consts::PI;
    }
}
