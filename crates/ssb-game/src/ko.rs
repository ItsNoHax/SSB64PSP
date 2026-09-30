//! The effects a KO leaves behind the fighter: the blast explosion
//! (`efManagerDeadExplodeMakeEffect`, `ef/efmanager.c`) and the screen
//! flash (`ifScreenFlashSetColAnimID`, `if/ifscreenflash.c`).
//!
//! The dead statuses record what they make ([`crate::dead::DeadState`]);
//! the match hands each fighter to [`KoEffects::observe`] after its physics
//! and runs [`KoEffects::tick`] once a frame after every fighter, where the
//! effect and interface processes run. The host plays and draws the
//! explosion up to [`Explosion::ticks`].
//!
//! The explosion's `LBParticle` half (`dEFManagerDeadExplodeGenID`'s
//! streaks) and the star KO's sparkle (`efManagerSparkleWhiteDeadMakeEffect`)
//! are particle scripts: [`KoEffects::observe_with`] makes them on the
//! match's runtime ([`crate::effect`], RE-413).

use ssb_engine::math::Vec3;

use crate::colanim::{ColAnim, ColAnimId};
use crate::dead::ExplodeKind;
use crate::fighter::Fighter;

/// `gcPlayAnimAll` calls until `llEFCommonEffects2DeadExplodeDefaultAnimJoint`
/// ends and `efManagerHaveStructProcUpdate` ejects the explosion. Replayed
/// from the ROM's scripts (RE-412).
pub const EXPLODE_TICKS: u16 = 36;

/// One live explosion.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Explosion {
    /// `dobj->translate`: the clamped death point.
    pub pos: Vec3,
    pub kind: ExplodeKind,
    /// `fp->player`: picks the material scripts
    /// (`dEFManagerDeadExplodeMatAnimJoints[player]`) and the ENV colours
    /// (`dEFManagerDeadExplodeEnvColor*`), which the host draws with.
    pub player: u8,
    /// `gcPlayAnimAll` calls so far.
    pub ticks: u16,
}

/// The match's KO effects.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct KoEffects {
    /// By port. A fighter cannot die again before its explosion ends.
    pub explosions: [Option<Explosion>; 4],
    /// `sIFScreenFlashColAnim`.
    pub flash: ColAnim,
}

impl KoEffects {
    /// Takes the effects `f`'s dead status made this frame.
    pub fn observe(&mut self, f: &mut Fighter) {
        if let Some((pos, kind)) = f.dead.explode.take() {
            let player = f.port.min(3);
            self.explosions[usize::from(player)] = Some(Explosion {
                pos,
                kind,
                player,
                ticks: 0,
            });
        }
        if core::mem::take(&mut f.dead.flash) {
            self.flash
                .check_set(ColAnimId::SCREEN_FLASH_DEAD_EXPLODE, 0);
        }
        if let Some(id) = f.screen_flash.take() {
            self.flash.check_set(id, 0);
        }
        // The sparkle and the quake are effects ([`Self::observe_with`]).
        f.dead.sparkle = None;
        f.dead.quake = false;
    }

    /// [`Self::observe`], first making the explosion's particle half and
    /// the star KO's sparkle (TopN, scale 5) on `rt`. They are made after
    /// the fighters' processes rather than inside the dead status setters
    /// (RE-413).
    pub fn observe_with(&mut self, f: &mut Fighter, rt: &mut crate::effect::EffectRuntime<'_>) {
        if let Some((pos, kind)) = f.dead.explode {
            rt.effects
                .dead_explode(rt.particles, rt.banks, pos, f.port.min(3), kind as u8);
        }
        if let Some(pos) = f.dead.sparkle {
            rt.effects
                .sparkle_white_dead(rt.particles, rt.banks, pos, 5.0);
        }
        // `efManagerQuakeMakeEffect(2)` (RE-420).
        if f.dead.quake {
            rt.effects.quake(2);
        }
        self.observe(f);
    }

    /// One frame of the effect and interface processes: each explosion's
    /// `efManagerHaveStructProcUpdate`, then `ifScreenFlashProcUpdate`.
    pub fn tick(&mut self) {
        for slot in &mut self.explosions {
            if let Some(e) = slot {
                e.ticks += 1;
                if e.ticks >= EXPLODE_TICKS {
                    *slot = None;
                }
            }
        }
        if self.flash.update() {
            self.flash.reset();
        }
    }

    /// `ifScreenFlashProcDisplay`'s fill colour, with the interface's alpha
    /// 0xFF (`ifScreenFlashMakeInterface`).
    pub fn flash_color(&self) -> Option<[u8; 4]> {
        self.flash.color()
    }
}

#[cfg(test)]
#[path = "ko_tests.rs"]
mod tests;
