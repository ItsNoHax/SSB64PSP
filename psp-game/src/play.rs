//! Training Mode's dummy target, plus the F1-specific hit application that
//! joins it to a real attacker.
//!
//! The pack-to-game bridge itself ([`FighterScene`], `FloorSegments`, the
//! packed-data conversions, skeleton ticking) lives in
//! `ssb_psp_runtime::scene`, shared with `psp-asset-viewer`. [`Dummy`] stays
//! here: `psp-asset-viewer` has no training combat and no second fighter, so
//! this is `psp-game`-only, Training-session state, not a shared runtime
//! type.

use core::ops::{Deref, DerefMut};

use ssb_game::fighter::{Fighter, FighterKind};
use ssb_game::status::Status;
use ssb_rom::pack::{Pack, StageDesc};

pub use ssb_psp_runtime::scene::{facing_turn, FighterScene};

/// A stationary, physics-ticked dummy target for Training Mode
/// (`plans/gameplay/F1.md`: "One player-controlled fighter vs. one
/// stationary/dummy target, no AI"). `psp-asset-viewer` has no equivalent --
/// the debug viewer has no training combat -- so unlike [`FighterScene`],
/// `Dummy` is `psp-game`-only, not shared runtime. Wraps a [`FighterScene`]
/// (`Deref`/`DerefMut` to it) rather than duplicating its fields, so both
/// the player and the dummy are instantiated and ticked through the same
/// shared implementation.
///
/// "Stationary" means no player/AI control, not "unsimulated": `Fighter` has
/// no separate idle/no-op mode, so standing still is real physics/animation
/// ticked every frame against permanently neutral input, the same as the
/// player's own fighter minus the input source and the camera.
pub struct Dummy {
    scene: FighterScene,
    /// Whether the player's current `Attack11` has already connected. Cleared
    /// as soon as the player leaves `Attack11`, so the next jab can hit again
    /// -- the simplified stand-in for the original's per-attack
    /// `GMAttackRecord` hit list (`ssb_game::attack`'s module docs).
    hit_by_current_attack: bool,
}

impl Deref for Dummy {
    type Target = FighterScene;
    fn deref(&self) -> &FighterScene {
        &self.scene
    }
}

impl DerefMut for Dummy {
    fn deref_mut(&mut self) -> &mut FighterScene {
        &mut self.scene
    }
}

impl Dummy {
    /// Puts a fighter at the stage's second player spawn (`pack.spawn(stage,
    /// 1)`), distinct from the player's own spawn 0. Returns `None` when the
    /// stage has no second spawn point rather than guessing a position --
    /// [`FighterScene::at_spawn`] itself has no such distinction (it always
    /// returns a scene, `placed: false` when nothing to stand on), so that
    /// check stays here.
    pub fn at_spawn(pack: &Pack<'_>, stage: &StageDesc) -> Option<Dummy> {
        pack.spawn(stage, 1)?;
        Some(Dummy {
            scene: FighterScene::at_spawn(pack, stage, FighterKind::Mario, 1),
            hit_by_current_attack: false,
        })
    }

    /// Advances one tick: permanently neutral input (no AI, no player
    /// control), real physics/animation against the real stage collision --
    /// the same `Fighter::tick` path the player's own scene drives, just with
    /// no input source and no camera (`FighterScene::tick_fighter`, not
    /// `FighterScene::tick`).
    pub fn tick(&mut self, pack: &Pack<'_>, stage: &StageDesc) {
        self.scene.tick_fighter(
            pack,
            stage,
            ssb_engine::input::ControllerState::default(),
            false,
        );
    }

    /// `F1` criterion 5: tests `attacker`'s active hitbox against this dummy
    /// and applies the hit -- `ssb_game::attack`'s formulas, called from here
    /// rather than from `ssb-game` because this is the only place both
    /// fighters exist together (the player's own scene and `Dummy` are
    /// separate values).
    ///
    /// Only `Attack11`'s hitbox is ported (module docs), so anything else the
    /// attacker is doing is a no-op call.
    pub fn apply_hit_from(&mut self, attacker: &Fighter) {
        if attacker.status.status != Status::Attack11 {
            self.hit_by_current_attack = false;
            return;
        }
        if self.hit_by_current_attack {
            return;
        }
        if !ssb_game::attack::jab1_hitbox_active(attacker.status.anim_frame) {
            return;
        }
        let hitbox = ssb_game::attack::MARIO_JAB1_HITBOX;
        let hitbox_pos = attacker.pos + hitbox.offset;
        if !ssb_game::attack::spheres_overlap(
            hitbox_pos,
            hitbox.radius,
            self.fighter.pos,
            ssb_game::attack::MARIO_HURTBOX_RADIUS,
        ) {
            return;
        }
        let result = ssb_game::attack::resolve_hit(
            &hitbox,
            attacker.pos,
            self.fighter.pos,
            self.fighter.damage,
            self.fighter.attributes.weight,
            !self.fighter.is_grounded(),
        );
        self.fighter.damage = self.fighter.damage.saturating_add(result.damage as u16);
        self.fighter.physics.vel_knockback = result.knockback_vel;
        self.fighter.hitstun = result.hitstun;
        self.hit_by_current_attack = true;
    }
}
