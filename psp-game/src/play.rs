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

use ssb_game::fighter::FighterKind;
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
    pub fn at_spawn(pack: &Pack<'_>, stage: &StageDesc, costume: u8) -> Option<Dummy> {
        pack.spawn(stage, 1)?;
        let mut scene = FighterScene::at_spawn(pack, stage, FighterKind::Mario, 1);
        scene.fighter.costume = costume;
        Some(Dummy { scene })
    }

    /// The priority-5 half of a tick with permanently neutral input (no AI,
    /// no player control) -- the same path the player's own scene drives,
    /// just with no input source and no camera. The physics half is
    /// [`FighterScene::tick_fighter_physics`].
    pub fn tick_interrupt(
        &mut self,
        pack: &Pack<'_>,
        stage: &StageDesc,
        groups: &[ssb_game::map::MapGroup],
    ) {
        self.scene.tick_fighter_interrupt(
            pack,
            stage,
            ssb_engine::input::ControllerState::default(),
            false,
            groups,
        );
    }
}
