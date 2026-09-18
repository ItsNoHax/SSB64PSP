//! Training Mode's dummy target, plus the F1-specific hit application that
//! joins it to a real attacker.
//!
//! The pack-to-game bridge itself ([`Play`], `FloorSegments`, the packed-data
//! conversions, skeleton ticking) lives in `ssb_psp_runtime::scene`, shared
//! with `psp-asset-viewer`. [`Dummy`] stays here: `psp-asset-viewer` has no
//! training combat and no second fighter, so this is `psp-game`-only,
//! Training-session state, not a shared runtime type.

use ssb_game::fighter::{Fighter, FighterKind};
use ssb_game::status::Status;
use ssb_rom::pack::{Pack, StageDesc};

use ssb_psp_runtime::scene::{
    anim_of, body_of, fighter_object, physics_of, tick_skeleton_animation, FloorSegments,
};

pub use ssb_psp_runtime::scene::Play;

/// A stationary, physics-ticked dummy target for Training Mode
/// (`plans/gameplay/F1.md`: "One player-controlled fighter vs. one
/// stationary/dummy target, no AI"). `psp-asset-viewer` has no equivalent --
/// the debug viewer has no training combat -- so unlike [`Play`], `Dummy` is
/// `psp-game`-only, not shared runtime.
///
/// "Stationary" means no player/AI control, not "unsimulated": `Fighter` has
/// no separate idle/no-op mode, so standing still is real physics/animation
/// ticked every frame against permanently neutral input, the same as `Play`'s
/// fighter minus the input source.
pub struct Dummy {
    pub fighter: Fighter,
    pub skeleton: ssb_rom::skeleton::Skeleton,
    /// Object whose nodes the skeleton drives, or `u32::MAX` when the pack has
    /// no model for this character (mirrors `Play::object`).
    pub object: u32,
    started: Option<Status>,
    /// Whether the player's current `Attack11` has already connected. Cleared
    /// as soon as the player leaves `Attack11`, so the next jab can hit again
    /// -- the simplified stand-in for the original's per-attack
    /// `GMAttackRecord` hit list (`ssb_game::attack`'s module docs).
    hit_by_current_attack: bool,
}

impl Dummy {
    /// Puts a fighter at the stage's second player spawn (`pack.spawn(stage,
    /// 1)`), distinct from `Play::at_spawn`'s spawn 0. Returns `None` when
    /// the stage has no second spawn point rather than guessing a position.
    pub fn at_spawn(pack: &Pack<'_>, stage: &StageDesc) -> Option<Dummy> {
        let kind = FighterKind::Mario;
        let mut fighter = Fighter::new(kind, 1, 3);

        if let Some(d) = pack.fighter(kind as u32) {
            fighter.attributes = physics_of(&d);
            fighter.coll = body_of(&d);
            fighter.anim = anim_of(&d);
        }

        let spawn = pack.spawn(stage, 1)?;
        fighter.pos = ssb_engine::math::Vec3::new(spawn.x as f32, spawn.y as f32, 0.0);

        Some(Dummy {
            fighter,
            skeleton: ssb_rom::skeleton::Skeleton::new(),
            object: fighter_object(pack, kind as u32).unwrap_or(u32::MAX),
            started: None,
            hit_by_current_attack: false,
        })
    }

    /// Advances one tick: permanently neutral input (no AI, no player
    /// control), real physics/animation against the real stage collision --
    /// the same `Fighter::tick` path `Play::tick` drives, just with no input
    /// source and no camera.
    pub fn tick(&mut self, pack: &Pack<'_>, stage: &StageDesc) {
        self.fighter
            .set_input(ssb_engine::input::ControllerState::default(), false, false);
        self.fighter.tick(|| FloorSegments::new(pack, stage));
        if self.object == u32::MAX {
            return;
        }
        tick_skeleton_animation(
            pack,
            self.fighter.kind as u32,
            self.fighter.status.status,
            &mut self.skeleton,
            &mut self.started,
        );
    }

    /// `F1` criterion 5: tests `attacker`'s active hitbox against this dummy
    /// and applies the hit -- `ssb_game::attack`'s formulas, called from here
    /// rather than from `ssb-game` because this is the only place both
    /// fighters exist together (`Play` and `Dummy` are separate structs).
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
