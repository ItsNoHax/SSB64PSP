//! The weapons' own effects (`wp/*`): the dust, sparkles, shocks, glows and
//! quakes their `proc_update`, `proc_map`, `proc_hit`, `proc_shield`,
//! `proc_setoff` and `proc_absorb` make, and the set-offs of a
//! weapon-against-weapon clash (`wpProcessUpdateAttackStatWeapon`), RE-416.
//!
//! A weapon's callback records what it makes as a [`WeaponEffect`]; the pool
//! tags it with the weapon's place in the weapon link ([`WeaponFx`]) and the
//! match makes the queue where the source's process ends, in link order:
//! the weapons' main processes (priority 3), the clash search (priority 1)
//! and the hit collisions (priority 0). The textures a thunder trail picks
//! each frame (`syUtilsRandIntRange`) travel in the same queue so that their
//! draws keep their place among the makers' draws.

use ssb_engine::math::Vec3;

use crate::effect::Effects;
use crate::particle::{Banks, Particles};

/// One effect a weapon callback makes, or one random draw it takes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WeaponEffect {
    DustLight {
        pos: Vec3,
        lr: i8,
    },
    ScaledExplosion {
        pos: Vec3,
        scale: f32,
    },
    DamageSlash {
        pos: Vec3,
        size: i32,
        lr: f32,
    },
    MonsterFlame {
        pos: Vec3,
        vel: Vec3,
    },
    /// `efManagerDustExpandSmallMakeEffect(pos, 1.0F)`.
    DustExpandSmall(Vec3),
    /// `efManagerSparkleWhiteMakeEffect`.
    SparkleWhite(Vec3),
    /// `efManagerSparkleWhiteMultiExplodeMakeEffect`.
    SparkleWhiteMultiExplode(Vec3),
    /// `efManagerImpactShockMakeEffect(pos, wp->attack_coll.damage)`.
    ImpactShock {
        pos: Vec3,
        size: i32,
    },
    /// `efManagerFoxBlasterGlowMakeEffect`.
    FoxBlasterGlow(Vec3),
    /// `efManagerFireGrindMakeEffect`: the Fireball's rebound.
    FireGrind(Vec3),
    /// `efManagerDustCollideMakeEffect`: the Boomerang meets a surface.
    DustCollide(Vec3),
    /// `efManagerYoshiEggExplodeMakeEffect`: script 3 of Yoshi's own
    /// particle bank, which the pack does not hold (made as nothing).
    YoshiEggExplode(Vec3),
    /// `efManagerEggBreakMakeEffect`.
    EggBreak(Vec3),
    /// `efManagerItemSpawnSwirlMakeEffect`, common script 0x69.
    ItemSpawnSwirl(Vec3),
    /// `efManagerQuakeMakeEffect(magnitude)`.
    Quake(u8),
    /// `efManagerSetOffMakeEffect`: one side of a weapon clash.
    SetOff {
        pos: Vec3,
        size: i32,
    },
    /// `efManagerPikachuThunderTrailMakeEffect(pos, lifetime, texture)`.
    ThunderTrail {
        pos: Vec3,
        lifetime: u8,
        texture: u8,
    },
    /// `mobj->texture_id_curr = syUtilsRandIntRange(n)`: a thunder trail's
    /// frame, drawn where its `proc_update` draws it.
    /// [`crate::weapon::WeaponPool::flush_effects`] draws it itself and
    /// keeps the frame on the trail (RE-417).
    TextureRand(u8),
}

/// The effects one weapon callback makes, in order. No callback makes more
/// than four (the Egg's map contact: quake, explosion, shell, dust).
#[derive(Debug, Clone, Copy, Default)]
pub struct Emit {
    buf: [Option<WeaponEffect>; EMIT_MAX],
    len: u8,
}

const EMIT_MAX: usize = 6;

impl Emit {
    pub fn push(&mut self, e: WeaponEffect) {
        if let Some(slot) = self.buf.get_mut(usize::from(self.len)) {
            *slot = Some(e);
            self.len += 1;
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = WeaponEffect> + '_ {
        self.buf[..usize::from(self.len)].iter().flatten().copied()
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

/// One pass's queued weapon effects: sixteen weapons, each making up to
/// four as it moves (the Egg's map contact), or up to eight as it hits.
pub const WEAPON_FX_MAX: usize = 128;

/// The clash search's set-offs in one frame: two for every pair of weapons
/// of different owners. Four players with four weapons each give
/// `C(16, 2) - 4 * C(4, 2)` = 96 pairs.
pub const CLASH_FX_MAX: usize = 2 * (16 * 15 / 2 - 4 * (4 * 3 / 2));

/// Weapon effects waiting for the end of the process that made them, each
/// with its weapon's place in the link (`GObj` make order).
#[derive(Debug, Clone)]
pub struct WeaponFx<const N: usize = WEAPON_FX_MAX> {
    items: [(u32, Option<WeaponEffect>); N],
    len: usize,
    /// Effects the queue had no room for (none in any measured frame; a
    /// test keeps it at zero).
    pub dropped: u16,
}

/// Two queues are equal when their queued effects are: the slots past the
/// length are stale.
impl<const N: usize> PartialEq for WeaponFx<N> {
    fn eq(&self, other: &Self) -> bool {
        self.items[..self.len] == other.items[..other.len] && self.dropped == other.dropped
    }
}

impl<const N: usize> Default for WeaponFx<N> {
    fn default() -> Self {
        WeaponFx {
            items: [(0, None); N],
            len: 0,
            dropped: 0,
        }
    }
}

impl<const N: usize> WeaponFx<N> {
    pub fn push(&mut self, seq: u32, e: WeaponEffect) {
        match self.items.get_mut(self.len) {
            Some(slot) => {
                *slot = (seq, Some(e));
                self.len += 1;
            }
            None => self.dropped = self.dropped.saturating_add(1),
        }
    }

    pub fn extend(&mut self, seq: u32, emit: &Emit) {
        for e in emit.iter() {
            self.push(seq, e);
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    /// Empties the queue in place.
    pub fn clear(&mut self) {
        self.len = 0;
        self.dropped = 0;
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The queue in link order (each weapon's effects in the order its
    /// callbacks made them), emptied.
    pub fn drain_sorted(&mut self) -> impl Iterator<Item = WeaponEffect> + '_ {
        self.drain_sorted_tagged().map(|(_, e)| e)
    }

    /// [`Self::drain_sorted`] with each effect's link place.
    pub fn drain_sorted_tagged(&mut self) -> impl Iterator<Item = (u32, WeaponEffect)> + '_ {
        let n = core::mem::take(&mut self.len);
        let items = &mut self.items[..n];
        // A stable insertion sort (no allocator): a weapon's own effects
        // keep their order.
        for i in 1..items.len() {
            let mut j = i;
            while j > 0 && items[j - 1].0 > items[j].0 {
                items.swap(j - 1, j);
                j -= 1;
            }
        }
        items
            .iter_mut()
            .filter_map(|(seq, e)| Some((*seq, e.take()?)))
    }

    /// The queue in push order, emptied: the clash search's set-offs, which
    /// it makes as it walks the link.
    pub fn drain(&mut self) -> impl Iterator<Item = WeaponEffect> + '_ {
        let n = core::mem::take(&mut self.len);
        self.items[..n].iter_mut().filter_map(|(_, e)| e.take())
    }
}

/// `efManagerFireGrindMakeEffect`'s script (list 0 of the common bank).
pub const FIRE_GRIND_ID: u16 = 0x0B;
/// `efManagerImpactShockMakeEffect`'s script.
pub const IMPACT_SHOCK_ID: u16 = 0x25;
/// `efManagerFoxBlasterGlowMakeEffect`'s `lbParticleMakeCommon` id.
pub const FOX_BLASTER_GLOW_ID: u16 = 0x62;

/// Makes one weapon effect on the match's particle runtime.
pub fn make(e: &WeaponEffect, effects: &mut Effects, p: &mut Particles, banks: &dyn Banks) {
    use crate::effect::script;
    match *e {
        WeaponEffect::DustLight { pos, lr } => {
            effects.dust_light(p, banks, pos, lr, 1.0);
        }
        WeaponEffect::ScaledExplosion { pos, scale } => {
            effects.ready_at(
                p,
                banks,
                true,
                script::SPARKLE_WHITE_MULTI_EXPLODE,
                pos,
                scale,
            );
        }
        WeaponEffect::DamageSlash { pos, size, lr } => {
            effects.damage_slash(pos, size, lr);
        }
        WeaponEffect::MonsterFlame { pos, vel } => {
            for id in [2, 0] {
                let pc = crate::particle::make_script_id(p, banks, 2, id);
                if pc != crate::particle::NIL {
                    let particle = p.particle_mut(pc);
                    particle.pos = pos;
                    particle.vel = vel;
                    crate::particle::process_struct(p, banks, effects, pc);
                }
            }
        }
        WeaponEffect::DustExpandSmall(pos) => {
            effects.dust_expand_small(p, banks, pos, 1.0);
        }
        WeaponEffect::SparkleWhite(pos) => {
            effects.ready_at(p, banks, true, script::SPARKLE_WHITE, pos, 1.0);
        }
        WeaponEffect::SparkleWhiteMultiExplode(pos) => {
            effects.ready_at(
                p,
                banks,
                true,
                script::SPARKLE_WHITE_MULTI_EXPLODE,
                pos,
                1.0,
            );
        }
        WeaponEffect::ImpactShock { pos, size } => {
            effects.impact_shock(p, banks, pos, size);
        }
        WeaponEffect::FoxBlasterGlow(pos) => {
            effects.common_at(p, banks, false, FOX_BLASTER_GLOW_ID, pos);
        }
        WeaponEffect::FireGrind(pos) => {
            effects.ready_at(p, banks, true, FIRE_GRIND_ID, pos, 1.0);
        }
        WeaponEffect::DustCollide(pos) => {
            effects.dust_collide(p, banks, pos);
        }
        WeaponEffect::YoshiEggExplode(pos) => {
            effects.yoshi_egg_explode(p, banks, pos);
        }
        WeaponEffect::ItemSpawnSwirl(pos) => {
            effects.ready_at(p, banks, true, 0x69, pos, 1.0);
        }
        WeaponEffect::EggBreak(pos) => {
            effects.ready_at(p, banks, false, script::EGG_BREAK, pos, 1.0);
        }
        WeaponEffect::Quake(magnitude) => {
            effects.quake(magnitude);
        }
        WeaponEffect::SetOff { pos, size } => {
            effects.set_off(p, banks, pos, size);
        }
        WeaponEffect::ThunderTrail {
            pos,
            lifetime,
            texture,
        } => {
            effects.thunder_trail(pos, lifetime, texture);
        }
        WeaponEffect::TextureRand(n) => {
            crate::rng::rand_int_range(i32::from(n));
        }
    }
}

#[cfg(test)]
#[path = "wpeffect_tests.rs"]
mod tests;
