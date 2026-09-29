//! `ftParamMakeEffect` (`ft/ftparam.c`): the effects a fighter's motion
//! scripts, colour animations and status code ask for, and
//! `ftParamKirbyTryMakeMapStarEffect`.
//!
//! The fighter owns no particle runtime, so each request is queued on the
//! fighter ([`EffectQueue`]) where the source would make it, and the match
//! makes the queue ([`flush`]) at the next point it holds the runtime: after
//! the fighter's `ftMainProcUpdateInterrupt`, after its
//! `ftMainProcPhysicsMap` (whose `ftMainUpdateMotionEventsForwardEffect`
//! makes the motion scripts' effects), and after each of the hit processes.
//! [`flush`] runs `ftParamMakeEffect` itself, so its placement reads the
//! fighter as it then stands and its random draws (the scatter, then the
//! maker's) keep the source's order among the fighter's own effects.

use ssb_engine::math::Vec3;

use crate::effect::{script, EffectRuntime};
use crate::fighter::{Fighter, FighterKind};

/// `efKind` (`ef/efdef.h`): the ids the port makes.
pub mod kind {
    pub const DAMAGE_NORMAL: u16 = 0;
    pub const FLAME_LR: u16 = 6;
    pub const FLAME_RANDOM: u16 = 7;
    pub const FLAME_STATIC: u16 = 8;
    pub const SHOCK_SMALL: u16 = 10;
    pub const DUST_LIGHT: u16 = 11;
    pub const DUST_LIGHT_RAPID: u16 = 12;
    pub const DUST_HEAVY_DOUBLE: u16 = 13;
    pub const DUST_HEAVY_DOUBLE_RAPID: u16 = 14;
    pub const DUST_HEAVY: u16 = 15;
    pub const DUST_HEAVY_REVERSE: u16 = 16;
    pub const DUST_EXPAND_LARGE: u16 = 17;
    pub const DUST_EXPAND_SMALL: u16 = 18;
    pub const DUST_DASH_SMALL: u16 = 19;
    pub const DUST_DASH_LARGE: u16 = 20;
    pub const DAMAGE_FLY_ORBS: u16 = 21;
    pub const IMPACT_WAVE: u16 = 22;
    pub const STAR_ROD_SPARK: u16 = 23;
    pub const DAMAGE_FLY_SPARKS: u16 = 24;
    pub const DAMAGE_FLY_SPARKS_REVERSE: u16 = 25;
    pub const DAMAGE_FLY_MDUST: u16 = 26;
    pub const DAMAGE_FLY_MDUST_REVERSE: u16 = 27;
    pub const SPARKLE_WHITE: u16 = 28;
    pub const SPARKLE_WHITE_MULTI_EXPLODE: u16 = 29;
    pub const SPARKLE_WHITE_MULTI: u16 = 30;
    pub const SPARKLE_WHITE_SCALE: u16 = 31;
    pub const QUAKE_MAG0: u16 = 32;
    pub const QUAKE_MAG1: u16 = 33;
    pub const QUAKE_MAG2: u16 = 34;
    pub const FIRE_SPARK: u16 = 37;
    pub const FURA_SPARKLE: u16 = 40;
    pub const PSIONIC: u16 = 41;
    pub const FLASH_SMALL: u16 = 42;
    pub const FLASH_MIDDLE: u16 = 43;
    pub const FLASH_LARGE: u16 = 44;
    pub const BOX_SMASH: u16 = 46;
    pub const CRASH_THE_GAME: u16 = 47;
    pub const KIRBY_STAR: u16 = 54;
    pub const THUNDER_AMP: u16 = 70;
    pub const RIPPLE: u16 = 71;
    pub const CHARGE_SPARKLE: u16 = 73;
    pub const HEAL_SPARKLES: u16 = 74;
    pub const KIRBY_BANK_2: u16 = 0x4C;
    pub const KIRBY_BANK_5: u16 = 0x4D;
    pub const YOSHI_EGG_ESCAPE: u16 = 87;
    pub const MUSIC_NOTE: u16 = 90;
    pub const EGG_BREAK: u16 = 91;
}

/// `ftParamMakeEffect`'s arguments.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EffectRequest {
    /// `efKind`.
    pub kind: u16,
    /// The joint, after `ftParamGetJointID`; `-1` for none.
    pub joint: i8,
    /// `effect_pos`, in the joint's frame; `None` for `NULL`.
    pub offset: Option<Vec3>,
    /// `effect_scatter`; `None` for `NULL`.
    pub scatter: Option<Vec3>,
    /// `lr`: `+1` or `-1`.
    pub lr: i8,
    /// `is_scale_pos`: the offset is divided by the fighter's size.
    pub is_scale_pos: bool,
    /// `arg7` (only `nEFKindCrashTheGame` reads it).
    pub flag: u16,
}

impl EffectRequest {
    /// A request with no offset or scatter, as the status code makes them.
    pub fn at_joint(kind: u16, joint: i8, lr: i8) -> EffectRequest {
        EffectRequest {
            kind,
            joint,
            offset: None,
            scatter: None,
            lr,
            is_scale_pos: false,
            flag: 0,
        }
    }
}

/// One queued fighter effect.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FighterEffect {
    /// `ftParamMakeEffect`.
    Param(EffectRequest),
    /// `ftParamKirbyTryMakeMapStarEffect`'s `efManagerKirbyStarMakeEffect`.
    KirbyStar(Vec3),
    /// A maker the status code calls at a point of its own
    /// (`efManagerFlashMiddleMakeEffect` at a caught ledge).
    At { kind: u16, pos: Vec3 },
}

/// Requests a fighter can queue between two flushes. A frame's worst case
/// is a status change's time-zero effects on top of the old status's and a
/// colour animation's: well under this (a test runs every script). A host
/// that never flushes (most host tests) only counts the overflow.
pub const EFFECT_QUEUE_MAX: usize = 16;

/// The fighter's queued effects, in the order the source makes them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EffectQueue {
    items: [Option<FighterEffect>; EFFECT_QUEUE_MAX],
    len: u8,
    /// Requests dropped for a full queue (kept at zero by the tests).
    pub dropped: u16,
}

impl Default for EffectQueue {
    fn default() -> Self {
        EffectQueue {
            items: [None; EFFECT_QUEUE_MAX],
            len: 0,
            dropped: 0,
        }
    }
}

impl EffectQueue {
    pub fn push(&mut self, e: FighterEffect) {
        if usize::from(self.len) < EFFECT_QUEUE_MAX {
            self.items[usize::from(self.len)] = Some(e);
            self.len += 1;
        } else {
            self.dropped = self.dropped.saturating_add(1);
        }
    }

    pub fn clear(&mut self) {
        self.len = 0;
    }

    pub fn len(&self) -> usize {
        usize::from(self.len)
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The queued effects, in order.
    pub fn iter(&self) -> impl Iterator<Item = &FighterEffect> + '_ {
        self.items[..usize::from(self.len)].iter().flatten()
    }
}

/// `ftParamMakeEffect`, queued.
pub fn request(f: &mut Fighter, r: EffectRequest) {
    f.effects.push(FighterEffect::Param(r));
}

/// `FTAttributes` fields the effects read, for any fighter kind (a polygon
/// or Metal Mario reads its base fighter's).
fn attrs(kind: FighterKind) -> Option<&'static crate::motion::CombatAttrs> {
    crate::motion::combat_attrs(kind)
        .or_else(|| crate::motion::combat_attrs(crate::grab::base_kind(kind)))
}

/// `ftParamGetJointID`: `-2` is the light-item joint.
pub fn joint_id(kind: FighterKind, joint: i8) -> i8 {
    if joint == -2 {
        attrs(kind).map_or(-1, |a| a.joint_itemlight_id as i8)
    } else {
        joint
    }
}

/// Makes every effect `f` queued, in order, and empties the queue.
pub fn flush(f: &mut Fighter, rt: &mut EffectRuntime<'_>) {
    let queue = core::mem::take(&mut f.effects);
    for e in queue.iter() {
        match *e {
            FighterEffect::Param(r) => make(f, rt, r),
            FighterEffect::KirbyStar(pos) => {
                rt.effects
                    .generator_at(rt.particles, rt.banks, script::KIRBY_STAR_GEN, pos);
            }
            FighterEffect::At { kind, pos } => {
                let lr = f.facing.sign() as i8;
                dispatch(f, rt, kind, pos, lr);
            }
        }
    }
    f.effects.dropped = queue.dropped;
}

/// `ftParamGetEffectJointPosition`: the next of the five
/// `effect_joint_ids`, round robin.
fn effect_joint_position(f: &mut Fighter) -> Vec3 {
    f.effect_joint_array_id += 1;
    if f.effect_joint_array_id == 5 {
        f.effect_joint_array_id = 0;
    }
    let joint = attrs(f.kind).map_or(0, |a| {
        a.effect_joint_ids[usize::from(f.effect_joint_array_id)]
    });
    f.joint_world(joint, Vec3::ZERO)
}

/// `FTSAMUS_CHARGE_EFFECT_JOINT`, `FTDONKEY_...` and `FTKIRBY_...`.
const CHARGE_EFFECT_JOINT: i8 = 16;

/// `ftParamMakeEffect`.
pub fn make(f: &mut Fighter, rt: &mut EffectRuntime<'_>, r: EffectRequest) {
    let mut joint = r.joint;
    let mut offset = r.offset;
    if r.kind == kind::CHARGE_SPARKLE {
        // The source's default case points at an uninitialised offset; no
        // other fighter charges.
        let x = match f.kind {
            FighterKind::Samus => 180.0,
            FighterKind::Donkey | FighterKind::GiantDonkey => 100.0,
            FighterKind::Kirby => 50.0,
            _ => 0.0,
        };
        if x != 0.0 {
            joint = CHARGE_EFFECT_JOINT;
        }
        offset = Some(Vec3::new(x, 0.0, 0.0));
    }
    // With joint `-1` the source's `pos` is uninitialised; every maker that
    // a script reaches that way places itself.
    let mut pos = Vec3::ZERO;
    if joint != -1 {
        pos = offset.unwrap_or(Vec3::ZERO);
        if let Some(s) = r.scatter {
            if s.x != 0.0 {
                pos.x += (crate::rng::rand_float() - 0.5) * (s.x * 2.0);
            }
            if s.y != 0.0 {
                pos.y += (crate::rng::rand_float() - 0.5) * (s.y * 2.0);
            }
            if s.z != 0.0 {
                pos.z += (crate::rng::rand_float() - 0.5) * (s.z * 2.0);
            }
        }
        if r.is_scale_pos {
            let k = 1.0 / f.attributes.size;
            pos *= k;
        }
        pos = f.joint_world(joint.max(0) as u8, pos);
    }
    dispatch(f, rt, r.kind, pos, r.lr);
}

/// `ftParamMakeEffect`'s second switch: the maker for `kind` at `pos`.
fn dispatch(f: &mut Fighter, rt: &mut EffectRuntime<'_>, effect: u16, mut pos: Vec3, lr: i8) {
    let (p, e, b) = (&mut *rt.particles, &mut *rt.effects, rt.banks);
    match effect {
        kind::DAMAGE_NORMAL => {
            let pos = effect_joint_position(f);
            e.damage_normal_light(p, b, pos, f.port, 10, false);
        }
        kind::FLAME_LR => {
            let pos = effect_joint_position(f);
            e.flame_lr(p, b, pos, lr);
        }
        kind::FLAME_RANDOM | kind::FLAME_STATIC => {
            let pos = effect_joint_position(f);
            e.flame(p, b, pos, effect == kind::FLAME_RANDOM);
        }
        kind::SHOCK_SMALL => {
            let pos = effect_joint_position(f);
            e.shock_small(pos);
        }
        kind::DUST_LIGHT => {
            e.dust_light(p, b, pos, lr, 1.0);
        }
        kind::DUST_LIGHT_RAPID => {
            e.dust_light(p, b, pos, lr, 2.0);
        }
        kind::DUST_HEAVY_DOUBLE => {
            e.dust_heavy_double(p, b, pos, lr, 1.0);
        }
        kind::DUST_HEAVY_DOUBLE_RAPID => {
            e.dust_heavy_double(p, b, pos, lr, 1.7);
        }
        kind::DUST_HEAVY => {
            e.dust_heavy(p, b, pos, lr);
        }
        kind::DUST_HEAVY_REVERSE => {
            e.dust_heavy(p, b, pos, -lr);
        }
        kind::DUST_EXPAND_LARGE => {
            pos.x += crate::rng::rand_float() * 160.0 - 80.0;
            pos.y += crate::rng::rand_float() * 160.0 - 80.0;
            e.dust_expand_large(p, b, pos);
        }
        kind::DUST_EXPAND_SMALL => {
            e.dust_expand_small(p, b, pos, 1.0);
        }
        kind::DUST_DASH_SMALL => {
            e.dust_dash(p, b, pos, lr, 1.0);
        }
        kind::DUST_DASH_LARGE => {
            e.dust_dash(p, b, pos, lr, 1.5);
        }
        kind::DAMAGE_FLY_ORBS => {
            e.damage_spawn_orbs(pos);
        }
        kind::IMPACT_WAVE => {
            let floor = f.floor.filter(|_| f.is_grounded());
            match floor {
                Some(floor) => {
                    let rotate = crate::particle::arc_tan2(-floor.normal.x, floor.normal.y);
                    e.impact_wave(pos, 4, rotate);
                }
                None => {
                    e.impact_wave(pos, 4, 0.0);
                }
            }
        }
        kind::STAR_ROD_SPARK => {
            e.star_rod_spark(pos, -lr);
        }
        kind::DAMAGE_FLY_SPARKS => {
            e.damage_spawn_sparks(pos, lr, false);
        }
        kind::DAMAGE_FLY_SPARKS_REVERSE => {
            e.damage_spawn_sparks(pos, -lr, false);
        }
        kind::DAMAGE_FLY_MDUST => {
            e.damage_spawn_sparks(pos, lr, true);
        }
        kind::DAMAGE_FLY_MDUST_REVERSE => {
            e.damage_spawn_sparks(pos, -lr, true);
        }
        kind::SPARKLE_WHITE => {
            e.ready_at(p, b, true, script::SPARKLE_WHITE, pos, 1.0);
        }
        kind::SPARKLE_WHITE_MULTI_EXPLODE => {
            e.ready_at(p, b, true, script::SPARKLE_WHITE_MULTI_EXPLODE, pos, 1.0);
        }
        kind::SPARKLE_WHITE_MULTI => {
            e.ready_at(p, b, true, script::SPARKLE_WHITE_MULTI, pos, 1.0);
        }
        kind::SPARKLE_WHITE_SCALE => {
            e.ready_at(p, b, false, script::SPARKLE_WHITE_SCALE, pos, 1.0);
        }
        // `fp->pkind != nFTPlayerKindDemo`: no port battle is a demo.
        kind::QUAKE_MAG0 | kind::QUAKE_MAG1 | kind::QUAKE_MAG2 => {
            e.quake((effect - kind::QUAKE_MAG0) as u8);
        }
        kind::PSIONIC => {
            e.common_at(p, b, false, script::PSIONIC, pos);
        }
        kind::FLASH_SMALL => {
            e.common_at(p, b, false, script::FLASH_SMALL, pos);
        }
        kind::FLASH_MIDDLE => {
            e.common_at(p, b, false, script::FLASH_MIDDLE, pos);
        }
        kind::FLASH_LARGE => {
            e.common_at(p, b, false, script::FLASH_LARGE, pos);
        }
        kind::FURA_SPARKLE => {
            e.common_at(p, b, true, script::FURA_SPARKLE, pos);
        }
        kind::KIRBY_STAR => {
            e.generator_at(p, b, script::KIRBY_STAR_GEN, pos);
        }
        kind::CHARGE_SPARKLE => {
            let pc = e.ready_at(p, b, false, script::SPARKLE_WHITE_SCALE, pos, 0.7);
            if pc != crate::particle::NIL {
                p.particle_mut(pc).primcolor[3] = 0xC0;
            }
        }
        kind::THUNDER_AMP => {
            e.ready_at(p, b, false, script::THUNDER_AMP, pos, 1.0);
        }
        kind::RIPPLE => {
            e.generator_at(p, b, script::RIPPLE_GEN, pos);
        }
        kind::HEAL_SPARKLES => {
            e.ready_at(p, b, true, script::HEAL_SPARKLES, pos, 1.0);
        }
        kind::MUSIC_NOTE => {
            e.music_note(p, b, pos);
        }
        kind::EGG_BREAK => {
            e.ready_at(p, b, false, script::EGG_BREAK, pos, 1.0);
        }
        kind::FIRE_SPARK => {
            e.fire_spark(f.port);
        }
        // Not made (RE-415): the Kirby bank's two scripts (the bank is not
        // loaded), Donkey Kong's crate pieces (the item file's), Yoshi's
        // roll egg (`efManagerYoshiEggEscapeMakeEffect`, which also hides
        // the model) and `func_ovl2_8010183C`.
        kind::KIRBY_BANK_2
        | kind::KIRBY_BANK_5
        | kind::BOX_SMASH
        | kind::YOSHI_EGG_ESCAPE
        | kind::CRASH_THE_GAME => {}
        _ => {}
    }
}

/// `ftParamKirbyTryMakeMapStarEffect`: a star where Kirby newly touches a
/// wall, the ceiling or the floor this frame, after `proc_map`.
pub fn kirby_map_star(f: &mut Fighter) {
    if f.kind != FighterKind::Kirby {
        return;
    }
    let (prev, curr) = (f.map_contacts_prev, f.map_contacts);
    let pos = f.pos;
    let c = f.coll;
    if curr.left_wall.is_some() && prev.left_wall.is_none() {
        f.effects.push(FighterEffect::KirbyStar(Vec3::new(
            pos.x + c.width,
            pos.y + c.center,
            pos.z,
        )));
    }
    if curr.right_wall.is_some() && prev.right_wall.is_none() {
        f.effects.push(FighterEffect::KirbyStar(Vec3::new(
            pos.x - c.width,
            pos.y + c.center,
            pos.z,
        )));
    }
    if curr.ceiling.is_some() && prev.ceiling.is_none() {
        f.effects.push(FighterEffect::KirbyStar(Vec3::new(
            pos.x,
            pos.y + c.top,
            pos.z,
        )));
    }
    if curr.floor && !prev.floor {
        f.effects.push(FighterEffect::KirbyStar(Vec3::new(
            pos.x,
            pos.y + c.bottom,
            pos.z,
        )));
    }
}

#[cfg(test)]
#[path = "fteffect_tests.rs"]
mod tests;
