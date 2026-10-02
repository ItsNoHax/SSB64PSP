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

/// Match bank assigned to `dFTYoshiData`'s `particles_unk2` pair.
/// The common bank remains 0; bank IDs are local to the match runtime.
pub const YOSHI_PARTICLE_BANK: u8 = 1;

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

/// `EFCOMMON_DUSTCOLL_OFF_BASE`, `_OFF_ADD` and `_VEL_BASE`.
const DUST_COLL_OFF_BASE: f32 = 300.0;
const DUST_COLL_OFF_ADD: f32 = -150.0;
const DUST_COLL_VEL: f32 = 15.0;

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

/// The priority-3 process a particle effect's `GObj` runs.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Proc {
    None,
    /// `efManagerDefaultProcUpdate`.
    Mover,
    /// `efManagerDustLightProcUpdate`, with `effect_vars.dust_light`'s
    /// `vel2` and `lifetime` (`vel1` is [`Slot::vel`]).
    DustLight {
        vel2: [f32; 2],
        lifetime: u8,
    },
    /// `efManagerDustHeavyDoubleProcUpdate`, with
    /// `effect_vars.dust_heavy`'s `anim_frame` and `lr`.
    DustHeavyDouble {
        frame: i32,
        lr: i8,
    },
}

/// One `EFStruct` and its effect `GObj`.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Slot {
    next: u8,
    kind: Kind,
    xf: u8,
    /// `effect_vars.common.vel`.
    vel: [f32; 2],
    proc: Proc,
    /// When its `GObj` was made: processes run in `GObj` order.
    seq: u32,
}

impl Slot {
    const EMPTY: Slot = Slot {
        next: NIL,
        kind: Kind::Default,
        xf: NIL,
        vel: [0.0; 2],
        proc: Proc::None,
        seq: 0,
    };
}

/// The effect manager's struct pool and particle bank.
#[derive(Debug, Clone, PartialEq)]
pub struct Effects {
    /// Custom seven-sibling DObj effects. Storage grows only on allocation,
    /// keeping the PSP's match initializer off a large stack temporary.
    containers: alloc::vec::Vec<ContainerSmash>,
    slots: [Slot; EFFECT_ALLOC_NUM],
    live: [bool; EFFECT_ALLOC_NUM],
    free: u8,
    /// `sEFManagerStructsFreeNum`.
    pub free_num: u8,
    /// `gEFManagerParticleBankID`: the common bank's id.
    pub bank: u8,
    /// The display effects' `GObj`s ([`Display`]).
    displays: [Option<Display>; DISPLAY_MAX],
    /// The next `GObj`'s place in the process order.
    seq: u32,
    /// Display effects the pool had no room for (none in any measured
    /// match; a test keeps it at zero).
    pub displays_refused: u16,
    /// The quake whose `gmCameraSetVelAt` this frame's processes left last,
    /// as `(magnitude, seq, gcPlayAnimAll count)` ([`Self::take_quake`]).
    quake_write: Option<(u8, u32, u16)>,
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
            containers: alloc::vec::Vec::new(),
            slots: [Slot::EMPTY; EFFECT_ALLOC_NUM],
            live: [false; EFFECT_ALLOC_NUM],
            free: 0,
            free_num: EFFECT_ALLOC_NUM as u8,
            bank,
            displays: [None; DISPLAY_MAX],
            seq: 0,
            displays_refused: 0,
            quake_write: None,
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
        if self.free_num < NO_FORCE_RESERVE {
            return None;
        }
        self.get_force()
    }

    /// `efManagerGetNextStructForce`: the last four are available too.
    fn get_force(&mut self) -> Option<u8> {
        if self.free == NIL {
            return None;
        }
        let i = self.free;
        self.free = self.slots[usize::from(i)].next;
        self.slots[usize::from(i)] = Slot::EMPTY;
        self.slots[usize::from(i)].seq = self.next_seq();
        self.live[usize::from(i)] = true;
        self.free_num -= 1;
        Some(i)
    }

    fn next_seq(&mut self) -> u32 {
        self.seq = self.seq.wrapping_add(1);
        self.seq
    }

    /// `efManagerSetPrevStructAlloc` and `gcEjectGObj`.
    fn release(&mut self, i: u8) {
        let s = usize::from(i);
        if !self.live[s] {
            return;
        }
        self.live[s] = false;
        self.slots[s].proc = Proc::None;
        self.slots[s].next = self.free;
        self.free = i;
        self.free_num += 1;
    }

    /// The effect processes (priority 3, after the fighters', weapons' and
    /// items' of the same priority), in `GObj` order: each particle
    /// struct's mover and each started display effect's `proc_update`.
    /// A display effect made during this pass starts next frame.
    pub fn process(&mut self, p: &mut Particles, banks: &dyn Banks) {
        let mut order = [(0u32, 0u8); EFFECT_ALLOC_NUM + DISPLAY_MAX];
        let mut n = 0;
        for (i, s) in self.slots.iter().enumerate() {
            if self.live[i] && s.proc != Proc::None && s.xf != NIL {
                order[n] = (s.seq, i as u8);
                n += 1;
            }
        }
        for (i, d) in self.displays.iter().enumerate() {
            if let Some(d) = d.filter(|d| d.started) {
                order[n] = (d.seq, 0x80 | i as u8);
                n += 1;
            }
        }
        let order = &mut order[..n];
        // `GObj` order is make order; the counter only wraps after years.
        order.sort_unstable_by_key(|&(seq, _)| seq);
        for &(_, tag) in order.iter() {
            if tag & 0x80 != 0 {
                self.display_proc(p, banks, usize::from(tag & 0x7F));
            } else {
                self.slot_proc(p, banks, tag);
            }
        }
    }

    fn slot_proc(&mut self, p: &mut Particles, banks: &dyn Banks, i: u8) {
        let s = self.slots[usize::from(i)];
        if !self.live[usize::from(i)] || s.xf == NIL {
            return;
        }
        match s.proc {
            Proc::None => {}
            Proc::Mover => {
                let t = p.transform_mut(s.xf);
                t.translate.x += s.vel[0];
                t.translate.y += s.vel[1];
            }
            Proc::DustLight { vel2, lifetime } => {
                let t = p.transform_mut(s.xf);
                t.translate.x += s.vel[0];
                t.translate.y += s.vel[1];
                if lifetime != 0 {
                    let slot = &mut self.slots[usize::from(i)];
                    slot.vel[0] += vel2[0];
                    slot.vel[1] += vel2[1];
                    slot.proc = Proc::DustLight {
                        vel2,
                        lifetime: lifetime - 1,
                    };
                }
            }
            Proc::DustHeavyDouble { frame, lr } => {
                let frame = frame + 1;
                self.slots[usize::from(i)].proc = Proc::DustHeavyDouble { frame, lr };
                if frame == 2 {
                    let mut pos = p.transform(s.xf).translate;
                    pos.y -= DUST_HEAVY_OFF_Y;
                    self.dust_heavy(p, banks, pos, -lr);
                }
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
        s.proc = Proc::Mover;
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
        s.proc = Proc::Mover;
        pc
    }

    /// `efManagerImpactShockMakeEffect`: a struct, the common bank's list-0
    /// script 0x25, then a speed and an angle and the damage's scale.
    pub fn impact_shock(
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
            crate::wpeffect::IMPACT_SHOCK_ID,
            TransformStatus::Default,
            Kind::Default,
        ) else {
            return NIL;
        };
        p.transform_mut(xf).translate = pos;
        self.aim(ep, rng::rand_float() * 8.0 + 2.0);
        set_scale(p, xf, damage_scale(size, 10, -0.05, 0.15));
        pc
    }

    /// `efManagerDustCollideMakeEffect`: the small dust (list 0) scattered
    /// by `EFCOMMON_DUSTCOLL_OFF_*`, thrown at 45 to 135 degrees at 15 and
    /// scaled by one to two.
    pub fn dust_collide(&mut self, p: &mut Particles, banks: &dyn Banks, pos: Vec3) -> u8 {
        let Some((pc, xf, ep)) = self.start(
            p,
            banks,
            self.bank,
            script::DUST_SMALL,
            TransformStatus::Default,
            Kind::Default,
        ) else {
            return NIL;
        };
        self.set_proc(ep, Proc::Mover);
        let t = p.transform_mut(xf);
        t.translate = pos;
        t.translate.x += rng::rand_float() * DUST_COLL_OFF_BASE + DUST_COLL_OFF_ADD;
        t.translate.y += rng::rand_float() * DUST_COLL_OFF_BASE + DUST_COLL_OFF_ADD;
        let angle = rng::rand_float() * dtor(90.0) + dtor(45.0);
        let (sin, cos) = sin_cos(angle);
        self.slots[usize::from(ep)].vel = [cos * DUST_COLL_VEL, sin * DUST_COLL_VEL];
        set_scale(p, xf, rng::rand_float() + 1.0);
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
        self.start_bare(
            p,
            banks,
            bank,
            DEAD_EXPLODE_IDS[index],
            TransformStatus::Ready,
            |t| {
                t.translate = pos;
                t.rotate.z = rotate_z;
            },
        )
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
        self.start_bare(
            p,
            banks,
            bank,
            SPARKLE_WHITE_DEAD_ID,
            TransformStatus::Ready,
            |t| {
                t.translate = pos;
                t.scale = Vec3::splat(scale);
            },
        )
    }

    /// A struct-less maker: the script under a transform with no
    /// `proc_dead`, the first update, then `place`.
    fn start_bare(
        &mut self,
        p: &mut Particles,
        banks: &dyn Banks,
        bank_id: u8,
        script: u16,
        status: TransformStatus,
        place: impl FnOnce(&mut lb::Transform),
    ) -> u8 {
        let pc = lb::make_script_id(p, banks, bank_id, script);
        if pc == NIL {
            return NIL;
        }
        let xf = p.add_transform_for_struct(pc, status);
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

    /// `efManagerYoshiEggExplodeMakeEffect`: Yoshi's script 3, first
    /// processed under a ready transform, then placed. No EFStruct.
    pub fn yoshi_egg_explode(&mut self, p: &mut Particles, banks: &dyn Banks, pos: Vec3) -> u8 {
        self.start_bare(
            p,
            banks,
            YOSHI_PARTICLE_BANK,
            3,
            TransformStatus::Ready,
            |t| {
                t.translate = pos;
            },
        )
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
            HitEffectKind::Slash { rotate } => {
                self.damage_slash(e.pos, e.damage, rotate);
            }
            HitEffectKind::SpawnOrbs => {
                self.damage_spawn_orbs_random(e.pos);
            }
            HitEffectKind::SpawnSparks { lr } => {
                self.damage_spawn_sparks_random(e.pos, lr as i8);
            }
            HitEffectKind::SpawnMDust { lr } => {
                self.damage_spawn_mdust_random(e.pos, lr as i8);
            }
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

/// Where the match hands its effects, in the order the source makes them:
/// the hit pipeline's, each fighter's queued ones ([`crate::fteffect`]),
/// and the effect processes' pass.
pub trait HitEffectSink {
    fn container_smash(&mut self, _pos: Vec3) {}
    fn make(&mut self, e: &HitEffect);

    /// Makes (or, without a runtime, drops) the effects `f` queued.
    fn fighter(&mut self, f: &mut crate::fighter::Fighter) {
        f.effects.clear();
    }

    /// The effect processes (priority 3).
    fn process(&mut self) {}

    /// The quake that moves the camera after this frame's processes
    /// ([`Effects::take_quake`]).
    fn take_quake(&mut self) -> Option<(u8, u16)> {
        None
    }

    /// Makes one weapon effect ([`crate::wpeffect`]). Without a runtime a
    /// thunder trail still draws its texture.
    fn weapon(&mut self, e: &crate::wpeffect::WeaponEffect) {
        if let crate::wpeffect::WeaponEffect::TextureRand(n) = *e {
            rng::rand_int_range(i32::from(n));
        }
    }
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
    fn container_smash(&mut self, pos: Vec3) {
        self.effects.container_smash(pos);
    }
    fn make(&mut self, e: &HitEffect) {
        self.effects.make_hit(self.particles, self.banks, e);
    }

    fn fighter(&mut self, f: &mut crate::fighter::Fighter) {
        crate::fteffect::flush(f, self);
    }

    fn process(&mut self) {
        self.effects.process(self.particles, self.banks);
    }

    fn weapon(&mut self, e: &crate::wpeffect::WeaponEffect) {
        crate::wpeffect::make(e, self.effects, self.particles, self.banks);
    }

    fn take_quake(&mut self) -> Option<(u8, u16)> {
        self.effects.take_quake()
    }
}

impl EffectRuntime<'_> {
    /// One frame's start: the particle and generator `func_run`s (link 0,
    /// before every process), then the effect link's: each display effect
    /// made last frame gets its process (`efManagerFuncRun`).
    pub fn run(&mut self) {
        lb::run(self.particles, self.banks, self.effects);
        self.effects.func_run();
    }

    /// [`Self::run`] and the effect processes: a frame with nothing else
    /// between (host tests).
    pub fn frame(&mut self) {
        self.run();
        self.effects.process(self.particles, self.banks);
    }
}

#[cfg(test)]
#[path = "effect_tests.rs"]
mod tests;

/// Display effects the pool holds: 38 struct-bearing ones at most, and the
/// struct-less slashes a frame's hits add.
pub const DISPLAY_MAX: usize = 64;

/// `EFCOMMON_DUSTNORMAL_*`.
const DUST_NORMAL_LIFETIME: u8 = 9;
const DUST_NORMAL_OFF_Y: f32 = 39.375;
const DUST_NORMAL_VEL_BASE: f32 = 36.0;
/// `EFCOMMON_DUSTHEAVY_OFF_Y`.
const DUST_HEAVY_OFF_Y: f32 = 126.0;
/// `EFCOMMON_DUSTEXPANDLARGE_SCALE`.
const DUST_EXPAND_LARGE_SCALE: f32 = 3.0;
/// `EFCOMMON_DUSTDASH_OFF_Y`.
const DUST_DASH_OFF_Y: f32 = 280.0;

/// Particle scripts of the common bank (`ef/efmanager.c`).
pub mod script {
    /// `efManagerConfettiMakeEffect` (the VS results, RE-420).
    pub const CONFETTI: u16 = 0x70;
    /// `efManagerFlameLRMakeEffect`.
    pub const FLAME_LR: u16 = 0x12;
    /// `efManagerFlameRandomMakeEffect`, `...FlameStatic...`,
    /// `...DustLight...` (`f_index` 1) and `...DustExpandSmall...`.
    pub const DUST_SMALL: u16 = 0x55;
    /// `efManagerDustLightMakeEffect` with `f_index` 2.
    pub const DUST_SMALL_RAPID: u16 = 0x56;
    /// `efManagerDustExpandLargeMakeEffect`.
    pub const DUST_EXPAND_LARGE: u16 = 0x57;
    /// `efManagerDustHeavyMakeEffect` and `...DustHeavyDouble...`.
    pub const DUST_HEAVY: u16 = 0x58;
    /// `efManagerDustHeavyDoubleMakeEffect` with `f_index` 1.7.
    pub const DUST_HEAVY_RAPID: u16 = 0x59;
    /// `efManagerDustDashMakeEffect`.
    pub const DUST_DASH: u16 = 0x5A;
    /// `efManagerSparkleWhiteScaleMakeEffect`.
    pub const SPARKLE_WHITE_SCALE: u16 = 0x5B;
    /// `efManagerSparkleWhiteMakeEffect`.
    pub const SPARKLE_WHITE: u16 = 0x73;
    /// `efManagerSparkleWhiteMultiMakeEffect`.
    pub const SPARKLE_WHITE_MULTI: u16 = 0x1A;
    /// `efManagerSparkleWhiteMultiExplodeMakeEffect`.
    pub const SPARKLE_WHITE_MULTI_EXPLODE: u16 = 0x22;
    /// `efManagerThunderAmpMakeEffect`.
    pub const THUNDER_AMP: u16 = 0x74;
    /// `efManagerHealSparklesMakeEffect`.
    pub const HEAL_SPARKLES: u16 = 0x0E;
    /// `efManagerEggBreakMakeEffect`.
    pub const EGG_BREAK: u16 = 0x54;
    /// `dEFManagerMusicNoteScriptIDs`.
    pub const MUSIC_NOTES: [u16; 3] = [0x40, 0x41, 0x42];
    /// `lbParticleMakeCommon` ids: `efManagerFuraSparkleMakeEffect` (on
    /// list 1), `...Psionic...`, `...FlashSmall...`, `...Middle...` and
    /// `...Large...`.
    pub const FURA_SPARKLE: u16 = 0;
    pub const PSIONIC: u16 = 7;
    pub const FLASH_SMALL: u16 = 4;
    pub const FLASH_MIDDLE: u16 = 5;
    pub const FLASH_LARGE: u16 = 6;
    /// Generators: `efManagerRippleMakeEffect` and `...KirbyStar...`.
    pub const RIPPLE_GEN: u16 = 0x61;
    pub const KIRBY_STAR_GEN: u16 = 0x0F;
}

/// A display effect: an `EFDesc` made through `efManagerMakeEffect`, its
/// `DObj` drawn by the host from the packed manager-effect object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayKind {
    /// `itBoxContainerSmashMakeEffect`: seven sibling pieces, one struct.
    ContainerSmash,
    /// `dEFManagerShockSmallEffectDesc` (`efManagerVelAddDestroyAnimEnd`).
    ShockSmall,
    /// `dEFManagerDamageSlashEffectDesc`: no `EFFECT_FLAG_USERDATA`, so no
    /// struct.
    Slash,
    /// `dEFManagerDamageSpawnOrbsEffectDesc`: no `DObj`, spawns
    /// [`Self::FlyOrbs`].
    SpawnOrbs,
    /// `dEFManagerDamageFlyOrbsEffectDesc`.
    FlyOrbs,
    /// `dEFManagerDamageSpawnSparksEffectDesc`: spawns [`Self::FlySparks`].
    SpawnSparks,
    /// `dEFManagerDamageFlySparksEffectDesc` (the common spark).
    FlySparks,
    /// `dEFManagerDamageSpawnMDustEffectDesc`: spawns [`Self::FlyMDust`].
    SpawnMDust,
    /// `dEFManagerDamageFlyMDustEffectDesc`.
    FlyMDust,
    /// `dEFManagerImpactWaveEffectDesc`.
    ImpactWave,
    /// `dEFManagerStarRodSparkEffectDesc` (the common spark).
    StarRodSpark,
    /// `efManagerQuakeMakeEffect`: a `DObj` whose animated translation
    /// moves the camera; nothing is drawn.
    Quake { magnitude: u8 },
    /// `dEFManagerFireSparkEffectDesc`: Samus's arm-cannon spark, attached
    /// to joint 16. Held for its animation; not drawn (RE-415).
    FireSpark,
    /// `dEFManagerPikachuThunderTrailEffectDesc`: a Thunder segment fading
    /// out, which ends by its own lifetime and picks a random frame while
    /// its texture is not 3. Its model is Pikachu's; not drawn (RE-416).
    ThunderTrail,
    /// `dEFManagerYoshiEggEscapeEffectDesc`: forced, attached to joint 5,
    /// no process or animation. Stopped by `ftParamProcStopEffect`.
    YoshiEggEscape,
}

/// `gcPlayAnimAll` calls until each display effect's animation reaches its
/// end and its `proc_update` ejects it (`anim_frame <= 0`), counting the
/// one `efManagerMakeEffect` makes. Replayed from the ROM's scripts
/// (`crates/ssb-rom/tests/display_effects.rs`, RE-415).
pub mod life {
    /// `llEFCommonEffects2ShockSmallMatAnimJoint` (the material's clock,
    /// which its `SetAnim` loop zeroes).
    pub const SHOCK_SMALL: u16 = 20;
    /// `llEFCommonEffects1DamageSlashAnimJoint`.
    pub const SLASH: u16 = 18;
    /// `llEFCommonEffects1CommonSparkAnimJoint`.
    pub const COMMON_SPARK: u16 = 18;
    /// `llEFCommonEffects1DamageFlyMDustAnimJoint`.
    pub const FLY_MDUST: u16 = 26;
    /// `llEFCommonEffects1ImpactWaveAnimJoint`.
    pub const IMPACT_WAVE: u16 = 13;
    /// `llEFCommonEffects1QuakeMag0AnimJoint` to `...Mag2...`.
    pub const QUAKE: [u16; 3] = [19, 31, 31];
    /// `llEFCommonEffects2FireSparkAnimJoint`.
    pub const FIRE_SPARK: u16 = 11;
}

impl DisplayKind {
    /// `EFDesc::dl_link`: the display link the effect draws on, which
    /// places it among the battle camera's passes (RE-422). The spawners and
    /// the quake have no display; they report 0.
    pub fn dl_link(self) -> u8 {
        match self {
            DisplayKind::ContainerSmash => 11,
            DisplayKind::ShockSmall | DisplayKind::Slash => 18,
            DisplayKind::ImpactWave => 10,
            DisplayKind::FlyOrbs
            | DisplayKind::FlySparks
            | DisplayKind::FlyMDust
            | DisplayKind::StarRodSpark
            | DisplayKind::FireSpark
            | DisplayKind::ThunderTrail
            | DisplayKind::YoshiEggEscape => 15,
            DisplayKind::SpawnOrbs
            | DisplayKind::SpawnSparks
            | DisplayKind::SpawnMDust
            | DisplayKind::Quake { .. } => 0,
        }
    }

    /// The `gcPlayAnimAll` count at which the effect ends by its
    /// animation, or `None` for one that ends by its own lifetime.
    pub fn life(self) -> Option<u16> {
        Some(match self {
            DisplayKind::ShockSmall => life::SHOCK_SMALL,
            DisplayKind::Slash => life::SLASH,
            DisplayKind::FlySparks | DisplayKind::StarRodSpark => life::COMMON_SPARK,
            DisplayKind::FlyMDust => life::FLY_MDUST,
            DisplayKind::ImpactWave => life::IMPACT_WAVE,
            DisplayKind::Quake { magnitude } => life::QUAKE[usize::from(magnitude.min(2))],
            DisplayKind::FireSpark => life::FIRE_SPARK,
            DisplayKind::SpawnOrbs
            | DisplayKind::FlyOrbs
            | DisplayKind::SpawnSparks
            | DisplayKind::SpawnMDust
            | DisplayKind::ThunderTrail
            | DisplayKind::YoshiEggEscape
            | DisplayKind::ContainerSmash => return None,
        })
    }

    /// Whether `efManagerMakeEffect` gives it a `DObj` with an animation
    /// (and so runs `gcPlayAnimAll` as it makes it).
    fn animated(self) -> bool {
        !matches!(
            self,
            DisplayKind::SpawnOrbs
                | DisplayKind::SpawnSparks
                | DisplayKind::SpawnMDust
                | DisplayKind::ThunderTrail
                | DisplayKind::YoshiEggEscape
                | DisplayKind::ContainerSmash
        )
    }
}

/// `dEFManagerImpactWavePrimColor*` (the ENV colours are all zero).
pub const IMPACT_WAVE_PRIM: [[u8; 3]; 5] = [
    [0xFF, 0x00, 0x00],
    [0x00, 0xFF, 0x00],
    [0x00, 0x00, 0xFF],
    [0xFF, 0xFF, 0x00],
    [0xFF, 0xFF, 0xFF],
];

/// `dEFManagerDamageSpawnSparksAngles` and `...MDustAngles`, in degrees.
const SPAWN_SPARK_ANGLES: [f32; 3] = [18.0, 0.0, -18.0];

/// One display effect's `GObj`: its `DObj`'s transform, its animation
/// clock and its `effect_vars`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Display {
    pub kind: DisplayKind,
    /// Its `EFStruct`, or [`NIL`] for a struct-less one.
    ep: u8,
    seq: u32,
    /// Whether `efManagerFuncRun` has given it its process (the frame
    /// after it is made).
    started: bool,
    /// `gcPlayAnimAll` calls so far.
    pub ticks: u16,
    /// The root `DObj`'s `translate`, `rotate` and `scale`.
    pub translate: Vec3,
    pub rotate: Vec3,
    pub scale: Vec3,
    /// `effect_vars.*.vel` (read by tests) and `.add`.
    pub vel: [f32; 2],
    add: [f32; 2],
    add_timer: u16,
    lifetime: i32,
    lr: i8,
    /// A spawner's point.
    pos: Vec3,
    /// `effect_vars.impact_wave`: its colour index and alpha.
    pub index: u8,
    pub alpha: f32,
    decay: f32,
    /// The fighter port a [`DisplayKind::FireSpark`] is attached to.
    pub owner: u8,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContainerPiece {
    pub pos: Vec3,
    pub rotate: Vec3,
    vel: Vec3,
    rotate_step: Vec3,
}
#[derive(Debug, Clone, PartialEq)]
struct ContainerSmash {
    seq: u32,
    pieces: [ContainerPiece; 7],
}

impl Display {
    fn new(kind: DisplayKind, ep: u8, seq: u32) -> Display {
        Display {
            kind,
            ep,
            seq,
            started: false,
            ticks: 0,
            translate: Vec3::ZERO,
            rotate: Vec3::ZERO,
            scale: Vec3::splat(1.0),
            vel: [0.0; 2],
            add: [0.0; 2],
            add_timer: 0,
            lifetime: 0,
            lr: 1,
            pos: Vec3::ZERO,
            index: 0,
            alpha: 0.0,
            decay: 0.0,
            owner: 0,
        }
    }
}

/// `F_CLC_DTOR32`.
fn dtor(degrees: f32) -> f32 {
    degrees * core::f32::consts::PI / 180.0
}

impl Effects {
    /// The live display effects, in pool order.
    pub fn displays(&self) -> impl Iterator<Item = &Display> + '_ {
        self.displays.iter().flatten()
    }
    /// One non-forced struct is obtained before the source's 42 RNG draws.
    pub fn container_smash(&mut self, pos: Vec3) {
        let Some(i) = self.make_display(DisplayKind::ContainerSmash) else {
            return;
        };
        let seq = self.display_mut(i).seq;
        self.display_mut(i).lifetime = 90;
        // Unlike `efManagerMakeEffect`, this maker installs its process
        // immediately; an item-main smash moves in this frame's effect pass.
        self.display_mut(i).started = true;
        let pieces = core::array::from_fn(|_| ContainerPiece {
            pos,
            rotate: Vec3::ZERO,
            vel: Vec3::new(
                rng::rand_float() * 48.0 - 24.0,
                rng::rand_float() * 50.0 + 10.0,
                rng::rand_float() * 32.0 - 16.0,
            ),
            rotate_step: Vec3::new(
                dtor(rng::rand_float() * 100.0 - 50.0),
                dtor(rng::rand_float() * 100.0 - 50.0),
                dtor(rng::rand_float() * 100.0 - 50.0),
            ),
        });
        self.containers.push(ContainerSmash { seq, pieces });
    }
    pub fn container_pieces(&self, display: &Display) -> Option<&[ContainerPiece; 7]> {
        self.containers
            .iter()
            .find(|s| s.seq == display.seq)
            .map(|s| &s.pieces)
    }

    /// The effect link's `func_run`s: every display effect made last frame
    /// gets its process.
    pub fn func_run(&mut self) {
        for d in self.displays.iter_mut().flatten() {
            d.started = true;
        }
    }

    /// `efManagerMakeEffect(desc, FALSE)`: a struct when the desc has
    /// `EFFECT_FLAG_USERDATA`, a `GObj`, and the creation's
    /// `gcPlayAnimAll`. `None` as the source returns `NULL`.
    fn make_display(&mut self, kind: DisplayKind) -> Option<usize> {
        let slot = self.displays.iter().position(Option::is_none);
        let Some(slot) = slot else {
            self.displays_refused = self.displays_refused.saturating_add(1);
            return None;
        };
        let ep = if kind == DisplayKind::Slash {
            NIL
        } else if kind == DisplayKind::YoshiEggEscape {
            self.get_force()?
        } else {
            self.get_no_force()?
        };
        let seq = self.next_seq();
        let mut d = Display::new(kind, ep, seq);
        if kind.animated() {
            d.ticks = 1;
        }
        self.displays[slot] = Some(d);
        Some(slot)
    }

    fn display_mut(&mut self, i: usize) -> &mut Display {
        self.displays[i].as_mut().expect("live display")
    }

    /// `efManagerSetPrevStructAlloc` and `gcEjectGObj`.
    fn eject_display(&mut self, i: usize) {
        if let Some(d) = self.displays[i].take() {
            if d.kind == DisplayKind::ContainerSmash {
                self.containers.retain(|s| s.seq != d.seq);
            }
            if d.ep != NIL {
                self.release(d.ep);
            }
        }
    }

    /// One display effect's `proc_update`.
    fn display_proc(&mut self, _p: &mut Particles, _banks: &dyn Banks, i: usize) {
        let Some(mut d) = self.displays[i] else {
            return;
        };
        match d.kind {
            DisplayKind::ContainerSmash => {
                d.lifetime -= 1;
                if d.lifetime == 0 {
                    self.eject_display(i);
                    return;
                }
                if let Some(smash) = self.containers.iter_mut().find(|s| s.seq == d.seq) {
                    for piece in &mut smash.pieces {
                        piece.vel.y -= 1.3;
                        piece.pos += piece.vel;
                        piece.rotate += piece.rotate_step;
                    }
                }
                self.displays[i] = Some(d);
            }
            DisplayKind::YoshiEggEscape => {}
            DisplayKind::ShockSmall
            | DisplayKind::Slash
            | DisplayKind::FlySparks
            | DisplayKind::FlyMDust
            | DisplayKind::ImpactWave
            | DisplayKind::StarRodSpark
            | DisplayKind::Quake { .. }
            | DisplayKind::FireSpark => {
                d.ticks = d.ticks.saturating_add(1);
                if d.kind.life().is_some_and(|life| d.ticks >= life) {
                    self.eject_display(i);
                    return;
                }
                match d.kind {
                    // `efManagerQuakeProcUpdate` writes the camera's
                    // `vel_at` from its `DObj`'s translation. The processes
                    // run at priority `3 - magnitude`, so the strongest
                    // quake (the latest made among equals) writes last.
                    DisplayKind::Quake { magnitude } => {
                        let key = (magnitude, d.seq);
                        if self.quake_write.is_none_or(|(m, seq, _)| (m, seq) < key) {
                            self.quake_write = Some((magnitude, d.seq, d.ticks));
                        }
                    }
                    // `efManagerVelAddDestroyAnimEnd`.
                    DisplayKind::ShockSmall => {
                        d.translate.x += d.vel[0];
                        d.translate.y += d.vel[1];
                    }
                    // `efManagerDamageFlySparksProcUpdate`.
                    DisplayKind::FlySparks | DisplayKind::FlyMDust => {
                        d.translate.x += d.vel[0];
                        d.translate.y += d.vel[1];
                        if d.add_timer != 0 {
                            d.add_timer -= 1;
                            d.vel[0] += d.add[0];
                            d.vel[1] += d.add[1];
                        }
                    }
                    // `efManagerImpactWaveProcUpdate`.
                    DisplayKind::ImpactWave => {
                        d.alpha = (d.alpha - d.decay).clamp(0.0, 255.0);
                    }
                    // `efManagerStarRodSparkProcUpdate`.
                    DisplayKind::StarRodSpark => {
                        if d.add_timer != 0 {
                            d.add_timer -= 1;
                            d.vel[0] += d.add[0];
                        }
                        d.translate.x += d.vel[0];
                    }
                    _ => {}
                }
                self.displays[i] = Some(d);
            }
            // `efManagerPikachuThunderTrailProcUpdate`.
            DisplayKind::ThunderTrail => {
                if d.lifetime == 0 {
                    self.eject_display(i);
                    return;
                }
                d.lifetime -= 1;
                if d.index != 3 {
                    if d.lifetime == 0 {
                        d.index = 3;
                        d.rotate.z = dtor(180.0);
                    } else {
                        d.index = rng::rand_int_range(3) as u8;
                    }
                }
                self.displays[i] = Some(d);
            }
            // `efManagerDamageFlyOrbsProcUpdate`.
            DisplayKind::FlyOrbs => {
                d.ticks = d.ticks.saturating_add(1);
                d.lifetime -= 1;
                if d.lifetime < 0 {
                    self.eject_display(i);
                    return;
                }
                d.translate.x += d.vel[0];
                d.translate.y += d.vel[1];
                d.vel[1] -= 10.0;
                self.displays[i] = Some(d);
            }
            // `efManagerDamageSpawnOrbsProcUpdate`.
            DisplayKind::SpawnOrbs => {
                if d.lifetime % 4 == 0 {
                    if let Some(j) = self.make_display(DisplayKind::FlyOrbs) {
                        let o = self.display_mut(j);
                        o.translate = d.pos;
                        let scale = rng::rand_float() * 2.0 + 3.0;
                        o.scale.x = scale;
                        o.scale.y = scale;
                        let vel = rng::rand_float() * 100.0 + 120.0;
                        let angle = rng::rand_float() * dtor(60.0) + dtor(-30.0) + dtor(90.0);
                        let (sin, cos) = sin_cos(angle);
                        o.vel = [cos * vel, sin * vel];
                        o.lifetime = rng::rand_int_range(4) + 12;
                    }
                }
                self.spawner_tick(i, d);
            }
            // `efManagerDamageSpawnSparksProcUpdate` and `...MDust...`.
            DisplayKind::SpawnSparks | DisplayKind::SpawnMDust => {
                let lifetime = d.lifetime;
                if lifetime % 4 == 0 {
                    let fly = if d.kind == DisplayKind::SpawnSparks {
                        DisplayKind::FlySparks
                    } else {
                        DisplayKind::FlyMDust
                    };
                    if let Some(j) = self.make_display(fly) {
                        let o = self.display_mut(j);
                        o.translate = d.pos;
                        o.rotate.z = rng::rand_float() * dtor(360.0);
                        let at = (2 - lifetime / 4).clamp(0, 2) as usize;
                        let (sin, cos) = sin_cos(dtor(SPAWN_SPARK_ANGLES[at]));
                        let lr = f32::from(d.lr);
                        o.vel = [cos * 50.0 * lr, sin * 50.0];
                        o.add = [-o.vel[0] * 0.004, -o.vel[1] * 0.004];
                        o.add_timer = 250;
                    }
                }
                self.spawner_tick(i, d);
            }
        }
    }

    /// `efManagerYoshiEggEscapeMakeEffect`: keep the forced struct until
    /// the fighter stops attached effects. Hiding happens only on success.
    pub fn yoshi_egg_escape(&mut self, owner: u8) -> bool {
        let Some(i) = self.make_display(DisplayKind::YoshiEggEscape) else {
            return false;
        };
        let d = self.display_mut(i);
        d.owner = owner;
        d.scale = Vec3::new(1.5, 1.5, 1.0);
        true
    }

    /// The roll egg's `ftParamProcStopEffect`, including every attached
    /// egg for this port, as the source walks the effect link.
    pub fn stop_yoshi_egg_escape(&mut self, owner: u8) {
        for i in 0..DISPLAY_MAX {
            if self.displays[i]
                .is_some_and(|d| d.kind == DisplayKind::YoshiEggEscape && d.owner == owner)
            {
                self.eject_display(i);
            }
        }
    }

    /// A spawner's `lifetime--`, ejected below zero.
    fn spawner_tick(&mut self, i: usize, mut d: Display) {
        d.lifetime -= 1;
        if d.lifetime < 0 {
            self.eject_display(i);
        } else {
            self.displays[i] = Some(d);
        }
    }

    /// `efManagerShockSmallMakeEffect`: five draws, the angle's wasted.
    pub fn shock_small(&mut self, mut pos: Vec3) -> bool {
        let Some(i) = self.make_display(DisplayKind::ShockSmall) else {
            return false;
        };
        pos.x += rng::rand_float() * 300.0 - 150.0;
        pos.y += rng::rand_float() * 300.0 - 150.0;
        let _angle = rng::rand_float() * dtor(360.0);
        let scale = rng::rand_float() * 0.5 + 0.75;
        let rotate = rng::rand_float() * dtor(360.0);
        let d = self.display_mut(i);
        d.translate = pos;
        d.vel = [0.0; 2];
        d.scale.x = scale;
        d.scale.y = scale;
        d.rotate.z = rotate;
        true
    }

    /// `efManagerDamageSlashMakeEffect`.
    pub fn damage_slash(&mut self, pos: Vec3, size: i32, rotate: f32) -> bool {
        let Some(i) = self.make_display(DisplayKind::Slash) else {
            return false;
        };
        let scale = if size < 5 {
            (5 - size) as f32 * -0.08 + 1.0
        } else {
            (size - 5) as f32 * 0.18 + 1.0
        };
        let d = self.display_mut(i);
        d.translate = pos;
        d.rotate.z = rotate;
        d.scale.x = scale;
        d.scale.y = scale;
        true
    }

    /// `efManagerDamageSpawnOrbsMakeEffect`.
    pub fn damage_spawn_orbs(&mut self, pos: Vec3) -> bool {
        let Some(i) = self.make_display(DisplayKind::SpawnOrbs) else {
            return false;
        };
        let lifetime = rng::rand_int_range(3) * 4 + 4;
        let d = self.display_mut(i);
        d.pos = pos;
        d.lifetime = lifetime;
        true
    }

    /// `efManagerDamageSpawnOrbsRandomMakeEffect`: one hit in four.
    pub fn damage_spawn_orbs_random(&mut self, pos: Vec3) -> bool {
        rng::rand_int_range(4) == 0 && self.damage_spawn_orbs(pos)
    }

    /// `efManagerDamageSpawnSparksMakeEffect` and `...MDust...`.
    pub fn damage_spawn_sparks(&mut self, pos: Vec3, lr: i8, metal: bool) -> bool {
        let kind = if metal {
            DisplayKind::SpawnMDust
        } else {
            DisplayKind::SpawnSparks
        };
        let Some(i) = self.make_display(kind) else {
            return false;
        };
        let d = self.display_mut(i);
        d.pos = pos;
        d.lifetime = 8;
        d.lr = lr;
        true
    }

    /// `efManagerDamageSpawnSparksRandomMakeEffect`: one hit in four.
    pub fn damage_spawn_sparks_random(&mut self, pos: Vec3, lr: i8) -> bool {
        rng::rand_int_range(4) == 0 && self.damage_spawn_sparks(pos, lr, false)
    }

    /// `efManagerDamageSpawnMDustRandomMakeEffect`: one hit in four.
    pub fn damage_spawn_mdust_random(&mut self, pos: Vec3, lr: i8) -> bool {
        rng::rand_int_range(4) == 0 && self.damage_spawn_sparks(pos, lr, true)
    }

    /// `efManagerImpactWaveMakeEffect` (and `...ImpactAirWave...` with no
    /// turn).
    pub fn impact_wave(&mut self, pos: Vec3, index: u8, rotate: f32) -> bool {
        let Some(i) = self.make_display(DisplayKind::ImpactWave) else {
            return false;
        };
        let d = self.display_mut(i);
        d.translate = pos;
        d.rotate.z = rotate;
        d.index = index;
        d.alpha = 255.0;
        d.decay = 127.0 / 11.0;
        true
    }

    /// `efManagerStarRodSparkMakeEffect`.
    pub fn star_rod_spark(&mut self, pos: Vec3, lr: i8) -> bool {
        let Some(i) = self.make_display(DisplayKind::StarRodSpark) else {
            return false;
        };
        let rotate = rng::rand_float() * dtor(360.0);
        let lr = f32::from(lr);
        let d = self.display_mut(i);
        d.translate = pos;
        d.rotate.z = rotate;
        d.scale.x = 0.75;
        d.scale.y = 0.75;
        d.vel[0] = lr * 25.0;
        d.add[0] = lr * -0.4;
        d.add_timer = 62;
        true
    }

    /// `efManagerQuakeMakeEffect`: a struct held for the animation; the
    /// camera shake is not ported (RE-412).
    pub fn quake(&mut self, magnitude: u8) -> bool {
        self.make_display(DisplayKind::Quake { magnitude })
            .is_some()
    }

    /// The quake that wrote the camera's `vel_at` last in this frame's
    /// processes: its magnitude and how many times its animation has
    /// played, whose `DObj` translation the host reads from the quake's
    /// animation ([`crate::camera::Camera::quake`]).
    pub fn take_quake(&mut self) -> Option<(u8, u16)> {
        self.quake_write.take().map(|(m, _, ticks)| (m, ticks))
    }

    /// `efManagerPikachuThunderTrailMakeEffect`: a struct and a `GObj` at
    /// half scale, whose `index` is its texture (3, or else 0).
    pub fn thunder_trail(&mut self, pos: Vec3, lifetime: u8, texture: u8) -> bool {
        let Some(i) = self.make_display(DisplayKind::ThunderTrail) else {
            return false;
        };
        let d = self.display_mut(i);
        d.translate = pos;
        d.scale = Vec3::splat(0.5);
        d.lifetime = i32::from(lifetime);
        d.index = if texture == 3 { 3 } else { 0 };
        true
    }

    /// `efManagerFireSparkMakeEffect`: the struct and `GObj`, held for the
    /// animation; not drawn.
    pub fn fire_spark(&mut self, owner: u8) -> bool {
        let Some(i) = self.make_display(DisplayKind::FireSpark) else {
            return false;
        };
        let d = self.display_mut(i);
        d.translate.y = 160.0;
        d.owner = owner;
        true
    }

    /// The struct makers' shared tail: the process and the transform's
    /// place.
    fn set_proc(&mut self, ep: u8, proc: Proc) {
        self.slots[usize::from(ep)].proc = proc;
    }

    /// `efManagerDustLightMakeEffect`.
    pub fn dust_light(
        &mut self,
        p: &mut Particles,
        banks: &dyn Banks,
        pos: Vec3,
        lr: i8,
        f_index: f32,
    ) -> u8 {
        let id = if f_index == 2.0 {
            script::DUST_SMALL_RAPID
        } else {
            script::DUST_SMALL
        };
        let bank = self.bank | lb::genlink(0);
        let Some((pc, xf, ep)) =
            self.start(p, banks, bank, id, TransformStatus::Default, Kind::Default)
        else {
            return NIL;
        };
        let t = p.transform_mut(xf);
        t.translate = pos;
        t.translate.y += DUST_NORMAL_OFF_Y;
        t.rotate.z = rng::rand_float() * dtor(360.0);
        let angle = rng::rand_float() * dtor(30.0) + dtor(-15.0);
        let (sin, cos) = sin_cos(angle);
        let mut vel1 = [cos * DUST_NORMAL_VEL_BASE, sin * DUST_NORMAL_VEL_BASE];
        if lr == 1 {
            vel1[0] = -vel1[0];
        }
        let scatter = 1.0 / f32::from(DUST_NORMAL_LIFETIME);
        self.slots[usize::from(ep)].vel = vel1;
        self.set_proc(
            ep,
            Proc::DustLight {
                vel2: [-vel1[0] * scatter, -vel1[1] * scatter],
                lifetime: DUST_NORMAL_LIFETIME,
            },
        );
        pc
    }

    /// `efManagerDustHeavyMakeEffect`: no struct and no first update.
    pub fn dust_heavy(&mut self, p: &mut Particles, banks: &dyn Banks, pos: Vec3, lr: i8) -> u8 {
        let bank = self.bank | lb::genlink(0);
        let pc = lb::make_script_id(p, banks, bank, script::DUST_HEAVY);
        if pc == NIL {
            return NIL;
        }
        let xf = p.add_transform_for_struct(pc, TransformStatus::Default);
        if xf == NIL {
            lb::eject_struct(p, banks, self, pc);
            return NIL;
        }
        let t = p.transform_mut(xf);
        t.translate = pos;
        t.translate.y += DUST_HEAVY_OFF_Y;
        if lr == -1 {
            t.rotate.y = core::f32::consts::PI;
        }
        pc
    }

    /// `efManagerDustHeavyDoubleMakeEffect`: a struct, no first update, and
    /// a second, reversed cloud on its second frame.
    pub fn dust_heavy_double(
        &mut self,
        p: &mut Particles,
        banks: &dyn Banks,
        pos: Vec3,
        lr: i8,
        f_index: f32,
    ) -> u8 {
        let Some(ep) = self.get_no_force() else {
            return NIL;
        };
        let id = if f_index == 1.7 {
            script::DUST_HEAVY_RAPID
        } else {
            script::DUST_HEAVY
        };
        let bank = self.bank | lb::genlink(0);
        let pc = lb::make_script_id(p, banks, bank, id);
        if pc == NIL {
            self.release(ep);
            return NIL;
        }
        let xf = p.add_transform_for_struct(pc, TransformStatus::Default);
        if xf == NIL {
            lb::eject_struct(p, banks, self, pc);
            self.release(ep);
            return NIL;
        }
        self.slots[usize::from(ep)].xf = xf;
        self.set_proc(ep, Proc::DustHeavyDouble { frame: 0, lr });
        let t = p.transform_mut(xf);
        t.translate = pos;
        t.translate.y += DUST_HEAVY_OFF_Y;
        if lr == -1 {
            t.rotate.y = core::f32::consts::PI;
        }
        t.owner = ep;
        t.has_proc_dead = true;
        pc
    }

    /// `efManagerDustExpandLargeMakeEffect`.
    pub fn dust_expand_large(&mut self, p: &mut Particles, banks: &dyn Banks, pos: Vec3) -> u8 {
        let bank = self.bank | lb::genlink(0);
        self.start_bare(
            p,
            banks,
            bank,
            script::DUST_EXPAND_LARGE,
            TransformStatus::Ready,
            |t| {
                t.translate = pos;
                t.scale = Vec3::splat(DUST_EXPAND_LARGE_SCALE);
            },
        )
    }

    /// `efManagerDustDashMakeEffect`.
    pub fn dust_dash(
        &mut self,
        p: &mut Particles,
        banks: &dyn Banks,
        pos: Vec3,
        lr: i8,
        scale: f32,
    ) -> u8 {
        let bank = self.bank | lb::genlink(0);
        self.start_bare(
            p,
            banks,
            bank,
            script::DUST_DASH,
            TransformStatus::Default,
            |t| {
                t.translate = pos;
                t.scale = Vec3::splat(scale);
                t.translate.y += DUST_DASH_OFF_Y;
                if lr == -1 {
                    t.rotate.y = core::f32::consts::PI;
                }
            },
        )
    }

    /// A struct-less script under a `Ready` transform at `pos`: the white
    /// sparkles, `ThunderAmp`, the heal sparkles and the egg break.
    pub fn ready_at(
        &mut self,
        p: &mut Particles,
        banks: &dyn Banks,
        genlink0: bool,
        id: u16,
        pos: Vec3,
        scale: f32,
    ) -> u8 {
        let bank = if genlink0 {
            self.bank | lb::genlink(0)
        } else {
            self.bank
        };
        self.start_bare(p, banks, bank, id, TransformStatus::Ready, |t| {
            t.translate = pos;
            if scale != 1.0 {
                t.scale = Vec3::splat(scale);
            }
        })
    }

    /// `efManagerConfettiMakeEffect`: the bank on list 0 with
    /// `is_genlink_mask`, otherwise ORed with `LBPARTICLE_MASK_GENLINK(3)`,
    /// under a `Ready` transform at `pos` (RE-420).
    pub fn confetti(
        &mut self,
        p: &mut Particles,
        banks: &dyn Banks,
        pos: Vec3,
        is_genlink_mask: bool,
    ) -> u8 {
        let bank = if is_genlink_mask {
            self.bank
        } else {
            self.bank | lb::genlink(3)
        };
        self.start_bare(
            p,
            banks,
            bank,
            script::CONFETTI,
            TransformStatus::Ready,
            |t| {
                t.translate = pos;
            },
        )
    }

    /// `efManagerMusicNoteMakeEffect`: the note is drawn before it is made.
    pub fn music_note(&mut self, p: &mut Particles, banks: &dyn Banks, pos: Vec3) -> u8 {
        let id = script::MUSIC_NOTES[rng::rand_int_range(3) as usize];
        self.ready_at(p, banks, true, id, pos, 1.0)
    }

    /// `efManagerFlameLRMakeEffect`.
    pub fn flame_lr(&mut self, p: &mut Particles, banks: &dyn Banks, pos: Vec3, lr: i8) -> u8 {
        let Some((pc, xf, ep)) = self.start(
            p,
            banks,
            self.bank,
            script::FLAME_LR,
            TransformStatus::Default,
            Kind::Default,
        ) else {
            return NIL;
        };
        self.set_proc(ep, Proc::Mover);
        let t = p.transform_mut(xf);
        t.translate = pos;
        t.translate.x += rng::rand_float() * 300.0 - 150.0;
        t.translate.y += rng::rand_float() * 200.0 - 150.0;
        let angle = rng::rand_float() * dtor(90.0);
        let (sin, cos) = sin_cos(angle);
        self.slots[usize::from(ep)].vel = [cos * 20.0 * -f32::from(lr), sin * 20.0];
        p.transform_mut(xf).scale = Vec3::splat(rng::rand_float() + 1.0);
        pc
    }

    /// `efManagerFlameRandomMakeEffect` (`moving`) and
    /// `efManagerFlameStaticMakeEffect`.
    pub fn flame(&mut self, p: &mut Particles, banks: &dyn Banks, pos: Vec3, moving: bool) -> u8 {
        let Some((pc, xf, ep)) = self.start(
            p,
            banks,
            self.bank,
            script::DUST_SMALL,
            TransformStatus::Default,
            Kind::Default,
        ) else {
            return NIL;
        };
        self.set_proc(ep, Proc::Mover);
        p.transform_mut(xf).translate = pos;
        self.slots[usize::from(ep)].vel = if moving {
            let angle = rng::rand_float() * dtor(90.0) + dtor(45.0);
            let (sin, cos) = sin_cos(angle);
            [cos * 15.0, sin * 15.0]
        } else {
            [0.0; 2]
        };
        p.transform_mut(xf).scale = Vec3::splat(rng::rand_float() + 1.0);
        pc
    }

    /// `lbParticleMakeCommon` at `pos`: `efManagerFuraSparkleMakeEffect`
    /// (list 1), `...Psionic...` and the three flashes.
    pub fn common_at(
        &mut self,
        p: &mut Particles,
        banks: &dyn Banks,
        genlink0: bool,
        id: u16,
        pos: Vec3,
    ) -> u8 {
        let bank = if genlink0 {
            self.bank | lb::genlink(0)
        } else {
            self.bank
        };
        let pc = lb::make_common(p, banks, self, bank, id);
        if pc != NIL {
            p.particle_mut(pc).pos = pos;
        }
        pc
    }

    /// A generator at `pos`: `efManagerRippleMakeEffect` and
    /// `...KirbyStar...`.
    pub fn generator_at(&mut self, p: &mut Particles, banks: &dyn Banks, id: u16, pos: Vec3) -> u8 {
        let gn = lb::make_generator(p, banks, self.bank, id);
        if gn != NIL {
            p.generator_mut(gn).pos = pos;
        }
        gn
    }
}
