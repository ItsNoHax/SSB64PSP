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

pub use ssb_psp_runtime::scene::{fighter_turn, FighterScene};

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
    /// The CPU player driving it (`ftComputerSetupAll`, RE-390):
    /// Training's `nFTComputerBehaviorStand` at level 3
    /// (`sc1PTrainingModeInitVars`).
    pub computer: ssb_game::computer::Computer,
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
    /// Puts a CPU fighter at the stage's `spawn`th player spawn
    /// (`mpCollisionGetPlayerMapObjPosition(player)`) on `port`, at CPU
    /// `level`. Training's dummy takes spawn and port 1. Returns `None`
    /// when the stage has no such spawn point rather than guessing a
    /// position -- [`FighterScene::at_spawn`] itself has no such
    /// distinction (it always returns a scene, `placed: false` when nothing
    /// to stand on), so that check stays here.
    #[allow(clippy::too_many_arguments)]
    pub fn at_spawn(
        pack: &Pack<'_>,
        stage: &StageDesc,
        kind: FighterKind,
        costume: u8,
        level: u8,
        spawn: u16,
        port: u8,
    ) -> Option<Dummy> {
        pack.spawn(stage, spawn)?;
        let mut scene = FighterScene::at_spawn(pack, stage, kind, spawn);
        scene.fighter.port = port;
        scene.fighter.costume = costume;
        let mut computer = ssb_game::computer::Computer::setup(&scene.fighter, level);
        // `sc1PTrainingModeUpdateDummyBehavior`: the menu's Stand, no trait.
        computer.behavior = ssb_game::computer::Behavior::Stand;
        computer.trait_kind = ssb_game::computer::attack::Trait::None;
        let surfaces = || ssb_psp_runtime::scene::MapSegments::new(pack, stage);
        let sight = CpuSight::default();
        let world = cpu_world(stage, surfaces, &[], &sight);
        computer.setup_world(&scene.fighter, &world);
        Some(Dummy { scene, computer })
    }

    /// The priority-5 half of a tick, driven by the CPU: `ftComputerProcessAll`
    /// sets `fp->input.cp` and the fighter reads it as its controller. The
    /// physics half is [`FighterScene::tick_fighter_physics`].
    pub fn tick_interrupt(
        &mut self,
        pack: &Pack<'_>,
        stage: &StageDesc,
        groups: &[ssb_game::map::MapGroup],
        opponents: &[ssb_game::computer::behave::Opponent],
        sight: &CpuSight,
        locked: bool,
    ) {
        let surfaces = || ssb_psp_runtime::scene::MapSegments::with_groups(pack, stage, groups);
        let world = cpu_world(stage, surfaces, opponents, sight);
        self.computer.process(&self.scene.fighter, &world);
        let controller = if locked {
            ssb_engine::input::ControllerState::default()
        } else {
            self.computer.controller()
        };
        self.scene
            .tick_fighter_interrupt(pack, stage, controller, false, groups);
    }
}

/// `gSCManagerBattleState->players[dummy].level` in Training.
pub const TRAINING_CPU_LEVEL: u8 = 3;

/// What a CPU reads of the match besides the fighters, as it stands when
/// its interrupt runs: the items and weapons in link order, the Twister,
/// the Zebes acid and its own PK Thunder trail.
pub struct CpuSight {
    pub items: alloc::vec::Vec<ssb_game::computer::behave::ItemSight>,
    pub weapons: alloc::vec::Vec<ssb_game::computer::behave::WeaponThreat>,
    pub team_rules: ssb_game::team::TeamRules,
    pub twister: Option<ssb_engine::math::Vec2>,
    pub acid: Option<(f32, f32)>,
    pub pk_thunder_trail: Option<ssb_engine::math::Vec2>,
}

impl Default for CpuSight {
    fn default() -> Self {
        CpuSight {
            items: alloc::vec::Vec::new(),
            weapons: alloc::vec::Vec::new(),
            team_rules: ssb_game::team::TeamRules::FREE_FOR_ALL,
            twister: None,
            acid: None,
            pk_thunder_trail: None,
        }
    }
}

impl CpuSight {
    /// The view of the CPU on `port`.
    pub fn observe(
        port: u8,
        items: &ssb_game::item::ItemPool,
        weapons: &ssb_game::weapon::WeaponPool,
        stage: &ssb_game::stage::Stage,
    ) -> Self {
        use ssb_game::stage::Controller;
        let flat = |v: ssb_engine::math::Vec3| ssb_engine::math::Vec2::new(v.x, v.y);
        CpuSight {
            items: items.cpu_sights().collect(),
            weapons: weapons.cpu_threats().collect(),
            team_rules: items.team_rules,
            twister: match &stage.controller {
                Controller::Hyrule(h) => h.visible_position().map(flat),
                _ => None,
            },
            acid: match &stage.controller {
                Controller::Zebes(z) => Some(z.level_info()),
                _ => None,
            },
            pk_thunder_trail: weapons.own_pk_trail(port).map(flat),
        }
    }
}

/// The map, bounds, opponents and the rest of the match a CPU reads this
/// frame.
fn cpu_world<'a, F, I>(
    stage: &StageDesc,
    surfaces: F,
    opponents: &'a [ssb_game::computer::behave::Opponent],
    sight: &'a CpuSight,
) -> ssb_game::computer::behave::World<'a, F>
where
    F: Fn() -> I,
    I: IntoIterator<Item = ssb_game::weapon::MapSurface>,
{
    let zone = |e: &ssb_rom::pack::Extent| ssb_game::status::BlastZone {
        top: f32::from(e.top),
        bottom: f32::from(e.bottom),
        left: f32::from(e.left),
        right: f32::from(e.right),
    };
    let geometry = ssb_game::computer::behave::geometry_bounds(surfaces());
    ssb_game::computer::behave::World {
        surfaces,
        geometry,
        stage: ssb_game::dead::StageBounds {
            map: zone(&stage.bounds),
            camera: zone(&stage.camera),
            rebirth: ssb_engine::math::Vec2::new(0.0, 0.0),
            fog_color: stage.fog_color,
        },
        gkind: ssb_rom::stage::vs_ground_kind(stage.source_file),
        opponents,
        items: &sight.items,
        weapon_threats: &sight.weapons,
        team_rules: sight.team_rules,
        // `nSCBattleGameType1PGame`: the 1P game is not ported.
        is_1p_game: false,
        pk_thunder_trail: sight.pk_thunder_trail,
        twister: sight.twister,
        acid: sight.acid,
    }
}
