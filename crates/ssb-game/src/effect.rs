//! The effect manager's particle effects (`ef/efmanager.c`): the hit sparks
//! and the KO's particle halves, made on the [`crate::particle`] runtime.
//!
//! - **Pool.** `efManagerInitEffects` allocates `EFFECT_ALLOC_NUM` (38)
//!   `EFStruct`s. A maker that does not force takes one only while five or
//!   more are free, so four always stay (`efManagerGetNextStructAlloc`);
//!   otherwise it makes nothing, as the source returns `NULL`.
//! - **Makers.** Each hit maker takes a struct, starts its script under a
//!   transform whose `proc_dead` is the struct's, runs the particle's first
//!   update (`LBParticleProcessStruct`) and only then places, scales and
//!   aims the transform. A struct with a velocity gets
//!   `efManagerDefaultProcUpdate`, which moves the transform each frame
//!   ([`Effects::update`]).
//! - **Death.** When the last particle or generator using a transform goes,
//!   its `proc_dead` runs: `efManagerDefaultProcDead` frees the struct,
//!   `efManagerDamageNormalHeavyProcDead` first makes a light spark where
//!   the heavy one was, and `efManagerDamageCoinProcDead` a small dust cloud
//!   200 units above.
//!
//! The KO makers (`efManagerDeadExplodeMakeEffect`'s particle,
//! `efManagerSparkleWhiteDeadMakeEffect`) take no struct.
//!
//! Randomness is the game's one sequence ([`crate::rng`]), drawn in the
//! source's order: the particle's first update, then the maker's own draws.

use core::f32::consts::TAU;

use ssb_engine::math::{sin_cos, Vec3};

use crate::particle::{self as lb, Banks, Particles, ProcDead, TransformStatus, NIL};
use crate::rng;

/// `EFFECT_ALLOC_NUM` (`ef/efdef.h`).
pub const EFFECT_ALLOC_NUM: usize = 38;

/// `efManagerGetNextStructAlloc`: a maker that does not force needs this
/// many structs free.
const NO_FORCE_RESERVE: u8 = 5;

/// `dEFManagerDamageNormalLightIDs`, by the attacker's player.
pub const NORMAL_LIGHT_IDS: [u16; 4] = [0x49, 0x4A, 0x4B, 0x4C];
/// `efManagerDamageNormalHeavyMakeEffect`'s script.
pub const NORMAL_HEAVY_ID: u16 = 0x64;
/// `efManagerDamageFireMakeEffect`.
pub const FIRE_ID: u16 = 0x4D;
/// `efManagerDamageElectricMakeEffect`.
pub const ELECTRIC_ID: u16 = 0x53;
/// `efManagerDamageCoinMakeEffect`.
pub const COIN_ID: u16 = 0x60;
/// `efManagerSetOffMakeEffect`.
pub const SET_OFF_ID: u16 = 0x65;
/// `efManagerDustExpandSmallMakeEffect`: 0x56 for `f_index` 2, else 0x55.
pub const DUST_EXPAND_SMALL_IDS: [u16; 2] = [0x55, 0x56];
/// `dEFManagerDeadExplodeGenID`: `[(type % 2) * 4 + player]`.
pub const DEAD_EXPLODE_IDS: [u16; 8] = [0x2D, 0x2C, 0x2B, 0x2A, 0x3F, 0x3E, 0x3D, 0x3C];
/// `efManagerSparkleWhiteDeadMakeEffect`.
pub const SPARKLE_WHITE_DEAD_ID: u16 = 0x5C;

/// `dEFManagerDamageNormalHeavyPrimColor*` (all white) and
/// `...EnvColor*`, by player.
pub const NORMAL_HEAVY_ENV: [[u8; 3]; 4] = [
    [0xFF, 0x00, 0x00],
    [0x00, 0xFF, 0x00],
    [0x00, 0x00, 0xFF],
    [0x78, 0x78, 0x78],
];

/// `dEFManagerDeadExplodeRotateD`, by `type`.
const DEAD_EXPLODE_ROTATE_D: [f32; 4] = [0.0, 90.0, 180.0, 270.0];

/// `EFCOMMON_DUSTEXPANDSMALL_VEL_X` and `_Y`.
const DUST_EXPAND_SMALL_VEL: [f32; 2] = [0.0, 5.0];

/// Which `proc_dead` a struct's transform has.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// `efManagerDefaultProcDead`.
    Default,
    /// `efManagerDamageNormalHeavyProcDead`, with
    /// `effect_vars.damage_normal_heavy`.
    NormalHeavy { player: u8, size: i32 },
    /// `efManagerDamageCoinProcDead`.
    Coin,
}

/// One `EFStruct` and its effect `GObj`.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Slot {
    next: u8,
    kind: Kind,
    xf: u8,
    /// `effect_vars.common.vel`.
    vel: [f32; 2],
    /// Whether `efManagerDefaultProcUpdate` runs on it.
    moves: bool,
}

impl Slot {
    const EMPTY: Slot = Slot {
        next: NIL,
        kind: Kind::Default,
        xf: NIL,
        vel: [0.0; 2],
        moves: false,
    };
}

/// The effect manager's struct pool and particle bank.
#[derive(Debug, Clone, PartialEq)]
pub struct Effects {
    slots: [Slot; EFFECT_ALLOC_NUM],
    live: [bool; EFFECT_ALLOC_NUM],
    free: u8,
    /// `sEFManagerStructsFreeNum`.
    pub free_num: u8,
    /// `gEFManagerParticleBankID`: the common bank's id.
    pub bank: u8,
}

impl Default for Effects {
    fn default() -> Self {
        Effects::new(0)
    }
}

impl Effects {
    /// `efManagerInitEffects`, with the common bank loaded as `bank`.
    pub fn new(bank: u8) -> Effects {
        let mut e = Effects {
            slots: [Slot::EMPTY; EFFECT_ALLOC_NUM],
            live: [false; EFFECT_ALLOC_NUM],
            free: 0,
            free_num: EFFECT_ALLOC_NUM as u8,
            bank,
        };
        for i in 0..EFFECT_ALLOC_NUM {
            e.slots[i].next = if i + 1 < EFFECT_ALLOC_NUM {
                (i + 1) as u8
            } else {
                NIL
            };
        }
        e
    }

    /// Structs in use.
    pub fn used(&self) -> usize {
        self.live.iter().filter(|&&l| l).count()
    }

    /// `efManagerGetNextStructAlloc(FALSE)`.
    fn get_no_force(&mut self) -> Option<u8> {
        if self.free_num < NO_FORCE_RESERVE || self.free == NIL {
            return None;
        }
        let i = self.free;
        self.free = self.slots[usize::from(i)].next;
        self.slots[usize::from(i)] = Slot::EMPTY;
        self.live[usize::from(i)] = true;
        self.free_num -= 1;
        Some(i)
    }

    /// `efManagerSetPrevStructAlloc` and `gcEjectGObj`.
    fn release(&mut self, i: u8) {
        let s = usize::from(i);
        if !self.live[s] {
            return;
        }
        self.live[s] = false;
        self.slots[s].moves = false;
        self.slots[s].next = self.free;
        self.free = i;
        self.free_num += 1;
    }

    /// Every struct's `efManagerDefaultProcUpdate` (process priority 3):
    /// the transform moves by the struct's velocity.
    pub fn update(&self, p: &mut Particles) {
        for (i, s) in self.slots.iter().enumerate() {
            if self.live[i] && s.moves && s.xf != NIL {
                let t = p.transform_mut(s.xf);
                t.translate.x += s.vel[0];
                t.translate.y += s.vel[1];
            }
        }
    }

    /// The shared head of the struct makers: a struct, the script under a
    /// new transform whose `proc_dead` is the struct's, and the first
    /// update. Returns the particle and transform while the transform
    /// still has a user.
    fn start(
        &mut self,
        p: &mut Particles,
        banks: &dyn Banks,
        bank_id: u8,
        script: u16,
        status: TransformStatus,
        kind: Kind,
    ) -> Option<(u8, u8, u8)> {
        let ep = self.get_no_force()?;
        let pc = lb::make_script_id(p, banks, bank_id, script);
        if pc == NIL {
            // `efManagerDestroyParticleGObj(NULL, effect_gobj)`.
            self.release(ep);
            return None;
        }
        let xf = p.add_transform_for_struct(pc, status);
        if xf == NIL {
            lb::eject_struct(p, banks, self, pc);
            self.release(ep);
            return None;
        }
        {
            let t = p.transform_mut(xf);
            t.owner = ep;
            t.has_proc_dead = true;
        }
        self.slots[usize::from(ep)].kind = kind;
        self.slots[usize::from(ep)].xf = xf;
        lb::process_struct(p, banks, self, pc);
        if p.transform(xf).users_num == 0 {
            return None;
        }
        Some((pc, xf, ep))
    }

    /// Adds `efManagerDefaultProcUpdate` with velocity `speed` at a random
    /// angle.
    fn aim(&mut self, ep: u8, speed: f32) {
        let angle = rng::rand_float() * TAU;
        let (sin, cos) = sin_cos(angle);
        let s = &mut self.slots[usize::from(ep)];
        s.vel = [cos * speed, sin * speed];
        s.moves = true;
    }

    /// `efManagerDamageNormalLightMakeEffect`.
    pub fn damage_normal_light(
        &mut self,
        p: &mut Particles,
        banks: &dyn Banks,
        pos: Vec3,
        player: u8,
        size: i32,
        is_static: bool,
    ) -> u8 {
        let script = NORMAL_LIGHT_IDS[usize::from(player.min(3))];
        let Some((pc, xf, ep)) = self.start(
            p,
            banks,
            self.bank,
            script,
            TransformStatus::Default,
            Kind::Default,
        ) else {
            return NIL;
        };
        p.transform_mut(xf).translate = pos;
        let speed = if is_static {
            0.0
        } else {
            rng::rand_float() * 38.0 + 12.0
        };
        self.aim(ep, speed);
        set_scale(p, xf, damage_scale(size, 10, -0.05, 0.13));
        pc
    }

    /// `efManagerDamageNormalHeavyMakeEffect`: white with the player's ENV,
    /// becoming a light spark when it ends.
    pub fn damage_normal_heavy(
        &mut self,
        p: &mut Particles,
        banks: &dyn Banks,
        pos: Vec3,
        player: u8,
        size: i32,
    ) -> u8 {
        let player = player.min(3);
        let kind = Kind::NormalHeavy { player, size };
        let Some((pc, xf, _)) = self.start(
            p,
            banks,
            self.bank,
            NORMAL_HEAVY_ID,
            TransformStatus::Default,
            kind,
        ) else {
            return NIL;
        };
        p.transform_mut(xf).translate = pos;
        let s = p.particle_mut(pc);
        s.primcolor = [0xFF; 4];
        let [r, g, b] = NORMAL_HEAVY_ENV[usize::from(player)];
        s.envcolor[0] = r;
        s.envcolor[1] = g;
        s.envcolor[2] = b;
        pc
    }

    /// `efManagerDamageFireMakeEffect`.
    pub fn damage_fire(
        &mut self,
        p: &mut Particles,
        banks: &dyn Banks,
        pos: Vec3,
        size: i32,
    ) -> u8 {
        let Some((pc, xf, ep)) = self.start(
            p,
            banks,
            self.bank,
            FIRE_ID,
            TransformStatus::Default,
            Kind::Default,
        ) else {
            return NIL;
        };
        p.transform_mut(xf).translate = pos;
        self.aim(ep, rng::rand_float() * 18.0 + 12.0);
        set_scale(p, xf, damage_scale(size, 10, -0.05, 0.15));
        pc
    }

    /// `efManagerDamageElectricMakeEffect`.
    pub fn damage_electric(
        &mut self,
        p: &mut Particles,
        banks: &dyn Banks,
        pos: Vec3,
        size: i32,
    ) -> u8 {
        let Some((pc, xf, ep)) = self.start(
            p,
            banks,
            self.bank,
            ELECTRIC_ID,
            TransformStatus::Default,
            Kind::Default,
        ) else {
            return NIL;
        };
        p.transform_mut(xf).translate = pos;
        self.aim(ep, rng::rand_float() * 7.0 + 3.0);
        set_scale(p, xf, damage_scale(size, 5, -0.08, 0.15));
        pc
    }

    /// `efManagerDamageCoinMakeEffect`.
    pub fn damage_coin(&mut self, p: &mut Particles, banks: &dyn Banks, pos: Vec3) -> u8 {
        let Some((pc, xf, _)) = self.start(
            p,
            banks,
            self.bank,
            COIN_ID,
            TransformStatus::Default,
            Kind::Coin,
        ) else {
            return NIL;
        };
        p.transform_mut(xf).translate = pos;
        pc
    }

    /// `efManagerSetOffMakeEffect`: a clank or a hit on an invincible box.
    /// Its transform is `Ready`, so the matrix its first draw builds stays.
    pub fn set_off(&mut self, p: &mut Particles, banks: &dyn Banks, pos: Vec3, size: i32) -> u8 {
        let bank = self.bank | lb::genlink(0);
        let Some((pc, xf, ep)) = self.start(
            p,
            banks,
            bank,
            SET_OFF_ID,
            TransformStatus::Ready,
            Kind::Default,
        ) else {
            return NIL;
        };
        p.transform_mut(xf).translate = pos;
        self.aim(ep, rng::rand_float() * 18.0 + 12.0);
        set_scale(p, xf, damage_scale(size, 10, -0.05, 0.15));
        pc
    }

    /// `efManagerDustExpandSmallMakeEffect`.
    pub fn dust_expand_small(
        &mut self,
        p: &mut Particles,
        banks: &dyn Banks,
        pos: Vec3,
        f_index: f32,
    ) -> u8 {
        let script = DUST_EXPAND_SMALL_IDS[usize::from(f_index == 2.0)];
        let bank = self.bank | lb::genlink(0);
        let Some((pc, xf, ep)) = self.start(
            p,
            banks,
            bank,
            script,
            TransformStatus::Default,
            Kind::Default,
        ) else {
            return NIL;
        };
        p.transform_mut(xf).translate = pos;
        let s = &mut self.slots[usize::from(ep)];
        s.vel = DUST_EXPAND_SMALL_VEL;
        s.moves = true;
        pc
    }

    /// `efManagerDeadExplodeMakeEffect`'s particle half: the player's
    /// streaks, turned like the blast, on list 2.
    pub fn dead_explode(
        &mut self,
        p: &mut Particles,
        banks: &dyn Banks,
        pos: Vec3,
        player: u8,
        kind: u8,
    ) -> u8 {
        let index = usize::from(kind % 2) * 4 + usize::from(player.min(3));
        let bank = self.bank | lb::genlink(1);
        let rotate_z = DEAD_EXPLODE_ROTATE_D[usize::from(kind % 4)] * core::f32::consts::PI / 180.0;
        self.start_bare(p, banks, bank, DEAD_EXPLODE_IDS[index], |t| {
            t.translate = pos;
            t.rotate.z = rotate_z;
        })
    }

    /// `efManagerSparkleWhiteDeadMakeEffect`: the star KO's sparkle.
    pub fn sparkle_white_dead(
        &mut self,
        p: &mut Particles,
        banks: &dyn Banks,
        pos: Vec3,
        scale: f32,
    ) -> u8 {
        let bank = self.bank | lb::genlink(1);
        self.start_bare(p, banks, bank, SPARKLE_WHITE_DEAD_ID, |t| {
            t.translate = pos;
            t.scale = Vec3::splat(scale);
        })
    }

    /// A struct-less maker: the script under a `Ready` transform with no
    /// `proc_dead`, the first update, then `place`.
    fn start_bare(
        &mut self,
        p: &mut Particles,
        banks: &dyn Banks,
        bank_id: u8,
        script: u16,
        place: impl FnOnce(&mut lb::Transform),
    ) -> u8 {
        let pc = lb::make_script_id(p, banks, bank_id, script);
        if pc == NIL {
            return NIL;
        }
        let xf = p.add_transform_for_struct(pc, TransformStatus::Ready);
        if xf == NIL {
            lb::eject_struct(p, banks, self, pc);
            return NIL;
        }
        lb::process_struct(p, banks, self, pc);
        if p.transform(xf).users_num == 0 {
            return NIL;
        }
        place(p.transform_mut(xf));
        pc
    }

    /// Makes one hit effect ([`HitEffect`]). The display effects (the
    /// slash, the orbs, the sparks and the metal dust) are not ported.
    pub fn make_hit(&mut self, p: &mut Particles, banks: &dyn Banks, e: &HitEffect) {
        match e.kind {
            HitEffectKind::NormalLight => {
                self.damage_normal_light(p, banks, e.pos, e.player, e.damage, false);
            }
            HitEffectKind::NormalHeavy => {
                self.damage_normal_heavy(p, banks, e.pos, e.player, e.damage);
            }
            HitEffectKind::Fire => {
                self.damage_fire(p, banks, e.pos, e.damage);
            }
            HitEffectKind::Electric => {
                self.damage_electric(p, banks, e.pos, e.damage);
            }
            HitEffectKind::Coin => {
                self.damage_coin(p, banks, e.pos);
            }
            HitEffectKind::SetOff => {
                self.set_off(p, banks, e.pos, e.damage);
            }
            HitEffectKind::Slash { .. }
            | HitEffectKind::SpawnOrbs
            | HitEffectKind::SpawnSparks { .. }
            | HitEffectKind::SpawnMDust { .. } => {}
        }
    }
}

impl ProcDead for Effects {
    fn proc_dead(&mut self, p: &mut Particles, banks: &dyn Banks, xf: u8) {
        let owner = p.transform(xf).owner;
        if owner == NIL || !self.live[usize::from(owner)] {
            return;
        }
        match self.slots[usize::from(owner)].kind {
            Kind::Default => {}
            Kind::NormalHeavy { player, size } => {
                let pos = p.transform(xf).translate;
                self.damage_normal_light(p, banks, pos, player, size, false);
            }
            Kind::Coin => {
                let mut pos = p.transform(xf).translate;
                pos.y += 200.0;
                self.dust_expand_small(p, banks, pos, 2.0);
            }
        }
        self.release(owner);
    }
}

/// The makers' scale by damage: `size < pivot` gives
/// `(pivot - size) * below + 1`, otherwise `(size - pivot) * above + 1`.
pub fn damage_scale(size: i32, pivot: i32, below: f32, above: f32) -> f32 {
    if size < pivot {
        (pivot - size) as f32 * below + 1.0
    } else {
        (size - pivot) as f32 * above + 1.0
    }
}

fn set_scale(p: &mut Particles, xf: u8, k: f32) {
    p.transform_mut(xf).scale = Vec3::splat(k);
}

/// Which hit effect `ftMainProcessHitCollisionStatsMain` (or a clank)
/// makes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HitEffectKind {
    /// `efManagerDamageNormalLightMakeEffect`: knockback under 180.
    NormalLight,
    /// `efManagerDamageNormalHeavyMakeEffect`.
    NormalHeavy,
    Fire,
    Electric,
    Coin,
    /// `efManagerDamageSlashMakeEffect`, a display effect, with
    /// `gmCollisionGetDamageSlashRotation`.
    Slash {
        rotate: f32,
    },
    /// `efManagerDamageSpawnOrbsRandomMakeEffect`, a display effect.
    SpawnOrbs,
    /// `efManagerDamageSpawnSparksRandomMakeEffect`, a display effect.
    SpawnSparks {
        lr: f32,
    },
    /// `efManagerDamageSpawnMDustRandomMakeEffect`, a display effect.
    SpawnMDust {
        lr: f32,
    },
    /// `efManagerSetOffMakeEffect`.
    SetOff,
}

/// One hit effect to make.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HitEffect {
    pub kind: HitEffectKind,
    /// The impact point (`gmCollisionGetCommonImpactPosition`).
    pub pos: Vec3,
    /// `hitlog->attacker_player`.
    pub player: u8,
    /// The damage the makers scale by.
    pub damage: i32,
}

/// Where the hit pipeline hands its effects, in the order the source
/// makes them.
pub trait HitEffectSink {
    fn make(&mut self, e: &HitEffect);
}

/// Drops every effect (host tests, or a scene without particles).
pub struct NoEffects;

impl HitEffectSink for NoEffects {
    fn make(&mut self, _: &HitEffect) {}
}

/// The match's particle runtime and effect manager together, as the host
/// owns them.
pub struct EffectRuntime<'b> {
    pub particles: &'b mut Particles,
    pub effects: &'b mut Effects,
    pub banks: &'b dyn Banks,
}

impl HitEffectSink for EffectRuntime<'_> {
    fn make(&mut self, e: &HitEffect) {
        self.effects.make_hit(self.particles, self.banks, e);
    }
}

impl EffectRuntime<'_> {
    /// One frame's start: the particle and generator `func_run`s (link 0,
    /// before every process), then each struct's move (priority 3).
    pub fn run(&mut self) {
        lb::run(self.particles, self.banks, self.effects);
        self.effects.update(self.particles);
    }
}

#[cfg(test)]
#[path = "effect_tests.rs"]
mod tests;
