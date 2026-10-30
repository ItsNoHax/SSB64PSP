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

use ssb_engine::memory::SceneBox;
use ssb_game::computer::CpuSight;
use ssb_game::fighter::FighterKind;
use ssb_rom::pack::{Pack, StageDesc};

pub use ssb_psp_runtime::scene::FighterScene;

/// A stationary, physics-ticked dummy target for Training Mode
/// (the retired F1 spec: "One player-controlled fighter vs. one
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
    /// `gSCManagerBattleState->game_type == nSCBattleGameType1PGame`, which
    /// the CPU's item tracking reads.
    pub is_1p_game: bool,
    /// How to Play's input script (`nFTPlayerKindGameKey`), which drives
    /// the fighter instead of the CPU.
    pub key: Option<ssb_game::key::Key>,
    /// Capture only (`cpufx-*`): the fighter reads the player's pad, as
    /// the N64 reference ROM's humans all read controller 0.
    pub mirror: bool,
    /// Capture only (`cpufx-r`): a mirror dummy's own scripted pad, read
    /// instead of the player's, as the N64 reference's port 2 reads
    /// controller 1.
    pub pad: Option<ssb_engine::input::ControllerState>,
    /// Its place in the fighter link, which orders every process
    /// (`gcMakeGObjSPAfter`): the port for the battle's own fighters, then
    /// increasing for a 1P Game team's next members, which link last.
    pub link: u32,
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
    ///
    /// Built in a slot of the scene arena, its final home: the 37 KB dummy
    /// never lands on the game stack (RE-469).
    #[allow(clippy::too_many_arguments)]
    #[inline(never)]
    pub fn new_at_spawn(
        pack: &Pack<'_>,
        stage: &StageDesc,
        kind: FighterKind,
        costume: u8,
        level: u8,
        spawn: u16,
        port: u8,
        counts: &ssb_game::manager_counts::ManagerCounts,
        groups: &[ssb_game::map::MapGroup],
    ) -> Option<SceneBox<Dummy>> {
        pack.spawn(stage, spawn)?;
        let mut b = ssb_psp_runtime::memory::scene_alloc_uninit::<Dummy>();
        let dst = b.as_mut_ptr();
        // SAFETY: the scene is written first, then every other field, before
        // `assume_init`; a panic between leaves the slot uninitialised, never
        // dropped.
        unsafe {
            FighterScene::write_at_spawn(core::ptr::addr_of_mut!((*dst).scene), pack, stage, kind, spawn, counts);
            Self::write_around_scene(dst, pack, stage, costume, level, port, groups);
            Some(b.assume_init())
        }
    }

    /// Replaces this fighter with a CPU fighter at `pos`, such as a 1P
    /// Game team's next member (`sc1PGameSpawnEnemyTeamNext`).
    ///
    /// Built in place: by value the 37 KB member was a stack temporary
    /// copied in (RE-469). The old member is dropped first.
    #[allow(clippy::too_many_arguments)]
    #[inline(never)]
    pub fn replace_at_position(
        &mut self,
        pack: &Pack<'_>,
        stage: &StageDesc,
        kind: FighterKind,
        costume: u8,
        level: u8,
        pos: ssb_engine::math::Vec3,
        port: u8,
        counts: &ssb_game::manager_counts::ManagerCounts,
        groups: &[ssb_game::map::MapGroup],
    ) {
        /// Between the drop and the last write `*self` holds no valid
        /// value, so a panic there must not unwind past its owner: a panic
        /// during unwinding aborts.
        struct AbortOnUnwind;
        impl Drop for AbortOnUnwind {
            fn drop(&mut self) {
                panic!("a fighter's in-place rebuild unwound");
            }
        }
        let guard = AbortOnUnwind;
        let dst: *mut Dummy = self;
        // SAFETY: `dst` is valid and aligned; the old value is dropped once,
        // then every field is written anew (the scene first).
        unsafe {
            core::ptr::drop_in_place(dst);
            FighterScene::write_at_position(core::ptr::addr_of_mut!((*dst).scene), pack, stage, kind, port, Some(pos), counts);
            Self::write_around_scene(dst, pack, stage, costume, level, port, groups);
        }
        core::mem::forget(guard);
    }

    /// The CPU around the scene already at `(*dst).scene`: the scene's port,
    /// costume and damage detect size, then every other field.
    ///
    /// # Safety
    ///
    /// `dst` is valid for writes and aligned, and its `scene` field is
    /// initialised. Every other field is overwritten without being read or
    /// dropped.
    unsafe fn write_around_scene(
        dst: *mut Dummy,
        pack: &Pack<'_>,
        stage: &StageDesc,
        costume: u8,
        level: u8,
        port: u8,
        groups: &[ssb_game::map::MapGroup],
    ) {
        use core::ptr::addr_of_mut;
        // SAFETY: the caller initialised the scene.
        ssb_engine::all_fields!(Dummy { scene, computer, is_1p_game, key, mirror, pad, link });
        let scene = unsafe { &mut *addr_of_mut!((*dst).scene) };
        scene.fighter.port = port;
        scene.fighter.costume = costume;
        // `ftManagerMakeFighter`'s `ftComputerSetFighterDamageDetectSize`.
        scene.set_damage_detect_size(pack);
        let mut computer = ssb_game::computer::Computer::setup(&scene.fighter, level);
        // `sc1PTrainingModeUpdateDummyBehavior`: the menu's Stand, no trait.
        computer.behavior = ssb_game::computer::Behavior::Stand;
        computer.trait_kind = ssb_game::computer::attack::Trait::None;
        // The groups as the stage's init left them: the cliff scan skips a
        // moving or switched group's lines (`mpCollisionCheckExistPlatformLineID`).
        let surfaces = || ssb_psp_runtime::scene::MapSegments::with_groups(pack, stage, groups);
        let sight = CpuSight::default();
        let world = cpu_world(stage, surfaces, &[], &sight, false);
        computer.setup_world(&scene.fighter, &world);
        // SAFETY: `dst` is valid for writes (the caller's contract); each
        // field is written once.
        unsafe {
            addr_of_mut!((*dst).computer).write(computer);
            addr_of_mut!((*dst).is_1p_game).write(false);
            addr_of_mut!((*dst).key).write(None);
            addr_of_mut!((*dst).mirror).write(false);
            addr_of_mut!((*dst).pad).write(None);
            addr_of_mut!((*dst).link).write(u32::from(port));
        }
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
        // `ftMainProcInterrupt`: a fighter whose control is locked
        // (`is_control_disable`) runs no CPU or script at all.
        let (controller, jump_held) = if locked {
            (ssb_engine::input::ControllerState::default(), false)
        } else if let Some(key) = self.key.as_mut() {
            key_input(key)
        } else {
            let input = computer_input(
                &mut self.computer,
                &self.scene,
                pack,
                stage,
                groups,
                opponents,
                sight,
                self.is_1p_game,
            );
            (input, false)
        };
        self.scene
            .tick_fighter_interrupt(pack, stage, controller, jump_held, groups);
    }
}

/// `ftKeyProcessKeyEvents`, then `cp` as the fighter reads it; its
/// C-buttons jump as a pad's do.
pub fn key_input(key: &mut ssb_game::key::Key) -> (ssb_engine::input::ControllerState, bool) {
    key.process();
    let c = key.controller();
    (c, c.buttons.contains(crate::JUMP_BUTTON_MASK))
}

/// `ftComputerProcessAll` for `scene`'s fighter: the CPU's controller.
#[allow(clippy::too_many_arguments)]
pub fn computer_input(
    computer: &mut ssb_game::computer::Computer,
    scene: &FighterScene,
    pack: &Pack<'_>,
    stage: &StageDesc,
    groups: &[ssb_game::map::MapGroup],
    opponents: &[ssb_game::computer::behave::Opponent],
    sight: &CpuSight,
    is_1p_game: bool,
) -> ssb_engine::input::ControllerState {
    let surfaces = || ssb_psp_runtime::scene::MapSegments::with_groups(pack, stage, groups);
    let world = cpu_world(stage, surfaces, opponents, sight, is_1p_game);
    computer.process(&scene.fighter, &world);
    computer.controller()
}

/// What drives the fighter on port 0: the pad, or in the title's demos an
/// input script (How to Play) or a CPU (the auto demo).
#[derive(Default)]
pub enum Lead {
    #[default]
    Pad,
    Key(ssb_game::key::Key),
    Computer(alloc::boxed::Box<ssb_game::computer::Computer>),
}

/// `ftComputerSetupAll` for a CPU on port 0, as [`Dummy::at_spawn`] sets
/// one up, running `behavior` with `trait_kind`.
pub fn lead_computer(
    pack: &Pack<'_>,
    stage: &StageDesc,
    scene: &FighterScene,
    level: u8,
    behavior: ssb_game::computer::Behavior,
    trait_kind: ssb_game::computer::attack::Trait,
    groups: &[ssb_game::map::MapGroup],
) -> ssb_game::computer::Computer {
    let mut computer = ssb_game::computer::Computer::setup(&scene.fighter, level);
    computer.behavior = behavior;
    computer.trait_kind = trait_kind;
    let surfaces = || ssb_psp_runtime::scene::MapSegments::with_groups(pack, stage, groups);
    let sight = CpuSight::default();
    let world = cpu_world(stage, surfaces, &[], &sight, false);
    computer.setup_world(&scene.fighter, &world);
    computer
}

/// `gSCManagerBattleState->players[dummy].level` in Training.
pub const TRAINING_CPU_LEVEL: u8 = 3;

/// The map, bounds, opponents and the rest of the match a CPU reads this
/// frame.
fn cpu_world<'a, F, I>(
    stage: &StageDesc,
    surfaces: F,
    opponents: &'a [ssb_game::computer::behave::Opponent],
    sight: &'a CpuSight,
    is_1p_game: bool,
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
        gkind: ssb_rom::stage::common_ground_kind(stage.source_file),
        opponents,
        items: &sight.items,
        weapon_threats: &sight.weapons,
        team_rules: sight.team_rules,
        is_1p_game,
        pk_thunder_trail: sight.pk_thunder_trail,
        twister: sight.twister,
        acid: sight.acid,
    }
}

/// One seat of the battle frame ([`ssb_game::matchtick`]): the lead
/// (pad, key script or CPU) or a dummy.
pub enum Player<'a> {
    Lead {
        scene: &'a mut FighterScene,
        lead: &'a mut Lead,
        /// The pad, already cleared while the countdown locks it.
        controller: ssb_engine::input::ControllerState,
    },
    Dummy {
        dummy: &'a mut Dummy,
        /// The lead's pad, for a mirror dummy.
        controller: ssb_engine::input::ControllerState,
    },
}

impl Player<'_> {
    pub fn scene(&self) -> &FighterScene {
        match self {
            Player::Lead { scene, .. } => scene,
            Player::Dummy { dummy, .. } => dummy,
        }
    }

    pub fn scene_mut(&mut self) -> &mut FighterScene {
        match self {
            Player::Lead { scene, .. } => scene,
            Player::Dummy { dummy, .. } => dummy,
        }
    }
}

impl ssb_game::matchtick::HasFighter for Player<'_> {
    fn fighter(&self) -> &ssb_game::fighter::Fighter {
        &self.scene().fighter
    }
    fn fighter_mut(&mut self) -> &mut ssb_game::fighter::Fighter {
        &mut self.scene_mut().fighter
    }
    /// Only the lead carries the battle camera.
    fn rig(&self) -> Option<&ssb_game::camera::CameraRig> {
        match self {
            Player::Lead { scene, .. } => Some(&scene.camera),
            Player::Dummy { .. } => None,
        }
    }
    fn rig_mut(&mut self) -> Option<&mut ssb_game::camera::CameraRig> {
        match self {
            Player::Lead { scene, .. } => Some(&mut scene.camera),
            Player::Dummy { .. } => None,
        }
    }
    fn framing(&self) -> ssb_game::camera::Framing {
        self.scene().framing()
    }
}

impl<'g, 'p> ssb_game::matchtick::Seat<ssb_psp_runtime::arena::PackGround<'g, 'p>> for Player<'_> {
    fn interrupt(
        &mut self,
        ground: &ssb_psp_runtime::arena::PackGround<'g, 'p>,
        view: &ssb_game::matchtick::InputView<'_>,
    ) {
        let (pack, stage, groups) = (ground.pack, ground.stage, ground.groups());
        match self {
            Player::Lead {
                scene,
                lead,
                controller,
            } => {
                // Real jump binding (RE-295): any N64 C-button tap is a
                // real `FTCOMMON_KNEEBEND` button-jump input
                // (`ftCommonKneeBendCheckButtonTap`). An upward stick
                // flick is the game's other real jump input and needs no
                // separate wiring here: `Fighter::tick`'s own status
                // machine reads `stick_y` directly.
                let jump_held = controller.buttons.contains(crate::JUMP_BUTTON_MASK);
                let (input, jump) = match lead {
                    Lead::Pad => (*controller, jump_held),
                    _ if view.locked => (ssb_engine::input::ControllerState::default(), false),
                    Lead::Key(key) => key_input(key),
                    Lead::Computer(computer) => {
                        let mut sight = ssb_game::computer::CpuSight::default();
                        view.sight_into(scene.fighter.port, &mut sight);
                        let input =
                            computer_input(computer, scene, pack, stage, groups, view.opponents, &sight, false);
                        (input, false)
                    }
                };
                scene.tick_fighter_interrupt(pack, stage, input, jump, groups);
            }
            Player::Dummy { dummy, controller } => {
                if dummy.mirror && !view.locked {
                    let controller = dummy.pad.unwrap_or(*controller);
                    let jump_held = controller.buttons.contains(crate::JUMP_BUTTON_MASK);
                    dummy.tick_fighter_interrupt(pack, stage, controller, jump_held, groups);
                } else {
                    // The items as the fighters before it left them this
                    // pass.
                    let mut sight = ssb_game::computer::CpuSight::default();
                    view.sight_into(dummy.fighter.port, &mut sight);
                    dummy.tick_interrupt(pack, stage, groups, view.opponents, &sight, view.locked);
                }
            }
        }
    }

    fn physics(&mut self, ground: &ssb_psp_runtime::arena::PackGround<'g, 'p>) {
        let groups = ground.groups();
        self.scene_mut().tick_fighter_physics(ground.pack, ground.stage, groups);
    }

    fn settle_catch(&mut self, ground: &ssb_psp_runtime::arena::PackGround<'g, 'p>) {
        let groups = ground.groups();
        self.scene_mut().settle_catch(ground.pack, ground.stage, groups);
    }

    fn settle_capture(&mut self, ground: &ssb_psp_runtime::arena::PackGround<'g, 'p>) {
        let groups = ground.groups();
        self.scene_mut().settle_capture(ground.pack, ground.stage, groups);
    }

    fn settle_status(&mut self, ground: &ssb_psp_runtime::arena::PackGround<'g, 'p>) {
        self.scene_mut().settle_status(ground.pack);
    }
}
