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
use core::sync::atomic::{AtomicU8, Ordering};

/// `gcPlayAnimAll` calls until `llEFCommonEffects2DeadExplodeDefaultAnimJoint`
/// ends and `efManagerHaveStructProcUpdate` ejects the explosion. Replayed
/// from the ROM's scripts (RE-412).
pub const EXPLODE_TICKS: u16 = 36;

/// `dEFManagerDeadExplodeEnvColorChild{R,G,B}[player]`: ENV of the blast's
/// first drawn DObj (`dobj->child`), generated from the ROM.
pub const DEAD_EXPLODE_ENV_CHILD: [[u8; 3]; 4] = dead_explode_env(3);
/// `dEFManagerDeadExplodeEnvColorSibling{R,G,B}[player]`: ENV of the third
/// (`dobj->child->sib_next->sib_next`). The second keeps its display
/// list's own `G_SETENVCOLOR`.
pub const DEAD_EXPLODE_ENV_SIBLING: [[u8; 3]; 4] = dead_explode_env(0);

/// The `[R, G, B]` per player of the three channel arrays from `first`.
const fn dead_explode_env(first: usize) -> [[u8; 3]; 4] {
    let c = crate::d_arrays::EF_MANAGER_DEAD_EXPLODE_ENV_COLORS;
    let mut out = [[0; 3]; 4];
    let mut p = 0;
    while p < 4 {
        out[p] = [c[first][p], c[first + 1][p], c[first + 2][p]];
        p += 1;
    }
    out
}

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
    /// `!is_allow_screenflash`: `ifScreenFlashMakeInterface` made no
    /// flash, so nothing sets or draws it.
    pub flash_disabled: bool,
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
        let dead = core::mem::take(&mut f.dead.flash);
        let hit = f.screen_flash.take();
        if dead && !self.flash_disabled {
            self.flash
                .check_set(ColAnimId::SCREEN_FLASH_DEAD_EXPLODE, 0);
        }
        if let Some(id) = hit.filter(|_| !self.flash_disabled) {
            self.flash.check_set(id, 0);
        }
        // The sparkle and the quake are effects ([`Self::observe_with`]).
        f.dead.sparkle = None;
        take(usize::from(f.port), Pending::Score);
        take(usize::from(f.port), Pending::Explode);
        f.dead.quake = false;
    }

    /// [`Self::observe`], first making on `rt` the explosion's particle
    /// half and the star KO's sparkle (TopN, scale 5) that the fighter's
    /// process left: the effect runtime makes them as it flushes the
    /// fighter ([`crate::effect::HitEffectSink::fighter`]), as the dead
    /// statuses do on the N64.
    pub fn observe_with(&mut self, f: &mut Fighter, rt: &mut crate::effect::EffectRuntime<'_>) {
        make_dead_particles(f, rt);
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

/// A dead status's effect its fighter's process made, waiting for the
/// effect runtime's flush of that fighter ([`make_dead_particles`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pending {
    /// `ftCommonDeadUpdateScore`'s interface half.
    Score = 0,
    /// `efManagerDeadExplodeMakeEffect`'s particle, at `dead.explode`.
    Explode = 4,
}

game_static! {
    /// Bit `kind + port`: a [`Pending`] effect. A global, like the
    /// original's managers' state: kept out of `Fighter`, whose by-value
    /// temporaries sit on the game's deepest stack path.
    static PENDING: AtomicU8 = AtomicU8::new(0);
}

/// Records `kind` for `port`'s flush.
pub(crate) fn request(port: usize, kind: Pending) {
    PENDING.with(|b| b.fetch_or(1 << (kind as usize + (port & 3)), Ordering::Relaxed));
}

/// Takes `port`'s `kind`, if requested.
fn take(port: usize, kind: Pending) -> bool {
    let bit = 1 << (kind as usize + (port & 3));
    PENDING.with(|b| b.fetch_and(!bit, Ordering::Relaxed)) & bit != 0
}

/// Whether `port`'s `kind` still waits for its flush.
pub fn is_pending(port: usize, kind: Pending) -> bool {
    PENDING.with(|b| b.load(Ordering::Relaxed)) & (1 << (kind as usize + (port & 3))) != 0
}

/// The dead statuses' effects, in their fighter's process and order:
/// `DeadUpStar`'s `efManagerSparkleWhiteDeadMakeEffect`, then
/// `ftCommonDeadUpdateScore`'s interface half ([`make_score_effects`]),
/// then the Down, Left and Right setters' `efManagerDeadExplodeMakeEffect`
/// particle (`ftMainProcPhysicsMap`). Each is made once.
pub fn make_dead_particles(f: &mut Fighter, rt: &mut crate::effect::EffectRuntime<'_>) {
    if let Some(pos) = f.dead.sparkle.take() {
        rt.effects
            .sparkle_white_dead(rt.particles, rt.banks, pos, 5.0);
    }
    if take(usize::from(f.port), Pending::Score) {
        make_score_effects(f, rt);
    }
    if let Some((pos, kind)) = f
        .dead
        .explode
        .filter(|_| take(usize::from(f.port), Pending::Explode))
    {
        rt.effects
            .dead_explode(rt.particles, rt.banks, pos, f.port.min(3), kind as u8);
    }
}

/// `ftCommonDeadUpdateScore`'s interface calls, in order:
/// `ifCommonPlayerDamageStartBreakAnim`'s draws (one per glyph, left for
/// the host's display, [`crate::hud::take_break_vel_x`]), `ifCommonPlayerStockMakeStockSnap`,
/// then where the battle shows scores `ifCommonPlayerScoreMakeEffect`'s
/// "-1" over the fallen player and "+1" over the one credited (RE-464).
fn make_score_effects(f: &mut Fighter, rt: &mut crate::effect::EffectRuntime<'_>) {
    let ui = crate::hud::fall_interface();
    let port = usize::from(f.port & 3);
    let mut vel_x = [0.0; 4];
    for v in vel_x[..usize::from(ui.char_display_count[port]).min(4)].iter_mut() {
        *v = crate::rng::rand_float() * 2.0 - 1.0;
    }
    crate::hud::put_break_vel_x(port, vel_x);
    let Some((x, y)) = ui.pos[port] else {
        return;
    };
    let (sx, sy) = crate::hud::stock_snap_position_at(x, y);
    rt.effects.stock_snap(rt.particles, rt.banks, sx, sy);
    if !ui.show_score {
        return;
    }
    rt.effects.battle_score(
        rt.particles,
        rt.banks,
        crate::hud::score_position_at(x, y),
        -1,
    );
    if let Some((kx, ky)) = f
        .damage_player
        .and_then(|k| ui.pos.get(usize::from(k)).copied().flatten())
    {
        rt.effects.battle_score(
            rt.particles,
            rt.banks,
            crate::hud::score_position_at(kx, ky),
            1,
        );
    }
}
