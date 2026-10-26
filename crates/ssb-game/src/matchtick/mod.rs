//! The battle frame's process order (`gcRunAll`, `src/sys/objman.c`):
//! one copy, run by the PSP's match loop and by the host replays.
//!
//! The original runs every GObj's processes by priority, 5 down to 0;
//! within a priority by link id (Ground 1, ItemActor 2, Fighter 3, Item 4,
//! Weapon 5, Effect 6, Camera 9, Interface 10 and 11), then in link order.
//! [`step`] runs the battle's processes in that order, over up to four
//! fighter seats (port order is link order):
//!
//! | Priority | Link | Process | Here |
//! |---|---|---|---|
//! | 5 | 1 | `gcPlayAnimAll` (stage objects) | [`Arena::play_anim`] |
//! | 5 | 3 | `ftMainProcUpdateInterrupt` | [`Seat::interrupt`], jostle, rebirth halo |
//! | 5 | 10, 11 | entry, focus, countdown threads; "Go" unlock | [`MatchHooks::interface`], [`World::go`] |
//! | 4 | 1 | `gr*ProcUpdate` | [`Arena::tick_stage`] |
//! | 4 | 2 | `mpCollisionPlayYakumonoAnim` (moving groups) | [`Arena::play_yakumono_anim`] |
//! | 4 | 3 | `ftMainProcPhysicsMapDefault` | [`Seat::physics`] |
//! | 3 | 2 | item appear actor | [`ItemPool::tick_appear`] |
//! | 3 | 3 | `ftMainProcPhysicsMapCapture` | [`Seat::physics`], held fighters |
//! | 3 | 4, 5 | item, weapon main | the pools |
//! | 3 | 6, 9 | effect `proc_update`, camera | [`MatchHooks::tick_camera`] |
//! | 2 | 3 | `ftMainProcSearchCatch` | hazard and catch search |
//! | 1 | 3 | `ftMainProcSearchHitAll`, fighter by fighter | [`crate::combat::process_hits`] |
//! | 1 | 4, 5 | item and weapon hit searches | the pools |
//! | 0 | 3, 4, 5 | `ftMainProcParams`, item then weapon hit collisions | |
//!
//! The order test (`tests.rs`) pins [`step`] to this table (RE-556). The
//! one port-only phase is [`Phase::Settle`] (D-052); the weapons' clash
//! search runs ahead of the fighters' hit searches because those react at
//! once (RE-416).
//!
//! Platform and pack concerns stay outside: the stage's map, objects and
//! animation behind [`Arena`], the fighter's skeleton and clip behind
//! [`Seat`], the profiler, stack poisoning and camera behind [`MatchHooks`].

use crate::boss::CameraRequest;
use crate::camera::{Camera, CameraRig, CameraStage, EntryFocusTarget, Framing, Interest};
use crate::computer::behave::Opponent;
use crate::effect::HitEffectSink;
use crate::fighter::Fighter;
use crate::item::{ItemAnims, ItemPool};
use crate::map::MapGroup;
use crate::stage::{Controller, Stage, StageItems, StageObjects};
use crate::status::{AnyStatus, BlastZone, Status};
use crate::team::TeamRules;
use crate::weapon::{MapSurface, WeaponPool};
use ssb_engine::array_vec::ArrayVec;
use ssb_engine::math::Vec3;

#[cfg(test)]
mod tests;

/// Seats per match: four ports.
pub const SEATS: usize = 4;

/// One fighter slot of the match: whatever carries a [`Fighter`].
pub trait HasFighter {
    fn fighter(&self) -> &Fighter;
    fn fighter_mut(&mut self) -> &mut Fighter;
    /// The battle camera and its statuses this seat carries
    /// (`gGMCameraGObj`): the lead's on the PSP, none on the host.
    fn rig(&self) -> Option<&CameraRig> {
        None
    }
    fn rig_mut(&mut self) -> Option<&mut CameraRig> {
        None
    }
    /// The battle camera itself ([`HasFighter::rig`]'s).
    fn camera(&self) -> Option<&Camera> {
        self.rig().map(|r| &r.camera)
    }
    fn camera_mut(&mut self) -> Option<&mut Camera> {
        self.rig_mut().map(|r| &mut r.camera)
    }
    /// What the camera reads of this fighter besides the [`Fighter`].
    fn framing(&self) -> Framing {
        Framing::default()
    }
}

impl HasFighter for Fighter {
    fn fighter(&self) -> &Fighter {
        self
    }
    fn fighter_mut(&mut self) -> &mut Fighter {
        self
    }
}

/// The match's fighters by slot, in port (link) order.
pub type Seats<'s, S> = [Option<&'s mut S>; SEATS];

/// The other fighters a CPU reads (`ftComputerProcessAll`).
pub type Opponents = ArrayVec<Opponent, 3>;

/// What a seat's input reads in its priority-5 process.
pub struct InputView<'a> {
    /// The other fighters as the seats before this one left them.
    pub opponents: &'a [Opponent],
    pub items: &'a ItemPool,
    pub weapons: &'a WeaponPool,
    pub stage: &'a Stage,
    /// The VS countdown locks every fighter's control, the CPUs' too.
    pub locked: bool,
}

impl InputView<'_> {
    /// What the CPU on `port` sees of the pools and the stage as the seats
    /// before it left them ([`CpuSight::observe`]).
    #[inline]
    pub fn sight(&self, port: u8) -> crate::computer::CpuSight {
        crate::computer::CpuSight::observe(port, self.items, self.weapons, self.stage)
    }
}

/// The stage's map as fighters and pools read it.
pub trait Ground {
    type Surfaces<'a>: Iterator<Item = MapSurface>
    where
        Self: 'a;
    /// The collision lines, the moving groups where they stand.
    fn surfaces(&self) -> Self::Surfaces<'_>;
    /// The moving collision groups (`gMPCollisionYakumonoDObjs`).
    fn groups(&self) -> &[MapGroup];
    /// `gMPCollisionGroundData->map_bound_*`.
    fn blast_zone(&self) -> BlastZone;
    /// `efManagerQuakeProcUpdate`'s translation `(y, z)` after `ticks`
    /// plays of quake `magnitude`'s animation (RE-416).
    fn quake_translate(&self, magnitude: u8, ticks: u16) -> Option<(f32, f32)>;
}

/// The stage: its map, its objects and the processes on the Ground link.
pub trait Arena {
    type Ground: Ground;
    type Props: StageObjects + ItemAnims;
    fn ground(&self) -> &Self::Ground;
    /// The map and the stage's objects at once.
    fn split(&mut self) -> (&Self::Ground, &mut Self::Props);
    /// Priority 5, Ground link: `gcPlayAnimAll` on the stage's objects.
    fn play_anim(&mut self) {}
    /// Priority 4, GroundDisplay link (2), after the stage controller and
    /// before the fighters' physics: the geometry layer's
    /// `mpCollisionPlayYakumonoAnim` (`grdisplay.c:209`), which moves the
    /// stage's groups and turns them on and off. The fighters' interrupts
    /// (the CPU's floor checks) see the previous frame's groups.
    fn play_yakumono_anim(&mut self) {}
    /// The collision group of floor `line` (`mpCollisionSetDObjNoID`), for
    /// a stage with moving groups.
    fn floor_group(&self, _line: u16) -> Option<u16> {
        None
    }
    /// Bonus 2's platform `index`, boarded: its texture swap.
    fn board(&mut self, _index: usize) {}
    /// Priority 4, Ground link: the stage controller.
    fn tick_stage(
        &mut self,
        _stage: &mut Stage,
        _fighters: &mut [&mut Fighter],
        _items: &mut dyn StageItems,
        _started: bool,
    ) {
    }
    /// Sector Z's Arwing roll, for its lasers.
    fn arwing_roll(&self) -> Option<f32> {
        None
    }
    /// The trees the items carry, placed after the frame (Inishie).
    fn place_item_trees(&mut self, _items: &ItemPool) {}
}

/// A fighter seat's pack-bound halves (`ssb_pose::PosedFighter` on both
/// platforms).
pub trait Seat<G: ?Sized>: HasFighter {
    /// Priority 5: this seat's input (pad, key or CPU) and
    /// `ftMainProcUpdateInterrupt`.
    fn interrupt(&mut self, ground: &G, view: &InputView<'_>);
    /// `ftMainProcPhysicsMap` (priority 4, or 3 for a held fighter).
    fn physics(&mut self, ground: &G);
    /// The catcher's `proc_catch` after the catch search.
    fn settle_catch(&mut self, ground: &G);
    /// The caught fighter's `proc_capture`.
    fn settle_capture(&mut self, ground: &G);
    /// A status the hit phase set, at its first frame.
    fn settle_status(&mut self, ground: &G);
}

/// The profiler's spans over the passes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Span {
    Interrupt,
    Physics,
    FighterPhysics,
    Items,
    Weapons,
    Camera,
    Hit,
}

/// One process of the frame as [`step`] runs it, for the order trace.
/// Seats are by slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// P5, Ground: `gcPlayAnimAll`.
    GroundAnim,
    /// P5, Fighter: `ftMainProcUpdateInterrupt`.
    Interrupt(u8),
    /// P5, Interface (links 10, 11): the entry, wait and countdown threads.
    Interface,
    /// Bonus 2's board check (`sc1PBonusStage`).
    Board,
    /// P4, Ground: the stage controller.
    StageController,
    /// P4, GroundDisplay: `mpCollisionPlayYakumonoAnim`.
    Yakumono,
    /// P4, Fighter: `ftMainProcPhysicsMapDefault`.
    Physics(u8),
    /// P3, Fighter: `ftMainProcPhysicsMapCapture`.
    Capture(u8),
    /// P3, ItemActor: `itManagerAppearActorProcUpdate`.
    ItemActor,
    /// P3, Item: `itProcessProcItemMain`.
    Items,
    /// P3, Weapon: `wpProcessProcWeaponMain`.
    Weapons,
    /// P3, Effect: the effects' `proc_update`.
    Effects,
    /// P3, Camera: `gmCameraRunFuncCamera`.
    Camera,
    /// P2, Fighter: `ftMainProcSearchCatch`.
    Catch(u8),
    /// P1, Fighter: `ftMainProcSearchHitAll`, one fighter at a time: its
    /// searches of the fighters, items, weapons and ground, then its logged
    /// hits' stats and effects (RE-556 G3).
    Hits(u8),
    /// P1, Item: `itProcessProcSearchHitAll`.
    ItemHits,
    /// P1, Weapon: `wpProcessProcSearchHitWeapon`'s reactions and
    /// set-offs; its search runs ahead of the fighters' (RE-416).
    WeaponHits,
    /// P0, Fighter: `ftMainProcParams` (after the P1 effects).
    Params,
    /// P0, Weapon: the weapons' hit collisions' effects.
    WeaponCollisions,
    /// P0, Item: `itProcessProcHitCollisions`.
    ItemCollisions,
    /// No process: the statuses the hit phase set stand at their first
    /// frame (D-052).
    Settle,
}

/// What the platform adds around the passes: the profiler, stack
/// poisoning, the order trace, the camera.
pub trait MatchHooks<S: ?Sized> {
    fn phase(&mut self, _phase: Phase) {}
    fn poison(&mut self) {}
    fn begin(&mut self, _span: Span) {}
    fn end(&mut self, _span: Span) {}
    /// What the camera's process (priority 3, link 9) reads of the stage;
    /// `None` runs no camera.
    fn camera_stage(&self) -> Option<CameraStage> {
        None
    }
    /// The interface threads (priority 5, links 10 and 11, after every
    /// fighter's interrupt): the entry focus's or the 1P wait thread's
    /// `ftCommonAppearSetStatus`, the countdown.
    fn interface(&mut self, _seats: &mut Seats<'_, S>, _battle: Option<&crate::battle::Battle>) {}
}

impl<S: ?Sized> MatchHooks<S> for () {}

/// The match state [`step`] runs over, borrowed for one frame.
pub struct World<'w> {
    pub weapons: &'w mut WeaponPool,
    pub items: &'w mut ItemPool,
    pub stage: &'w mut Stage,
    pub effects: &'w mut dyn HitEffectSink,
    pub bonus: Option<&'w mut crate::spgame::bonus_stage::BonusStage>,
    pub battle: Option<&'w mut crate::battle::Battle>,
    /// `gSCManagerBattleState->game_status != nSCBattleGameStatusWait`.
    pub started: bool,
    /// The control lock the fighters' interrupts run under: the VS
    /// countdown's, Training's menu, Master Hand's defeat.
    pub locked: bool,
    /// "Go" lands in this frame's interface threads
    /// ([`crate::battle::Battle::go_lands`]): the countdown's lock lifts
    /// after the interrupts (`ftParamUnlockPlayerControl`).
    pub go: bool,
    /// The rumble actor (`gmRumbleMakeActor`); `None` drops the fighters'
    /// rumble commands.
    pub rumble: Option<&'w mut crate::rumble::Rumble>,
}

/// One battle frame, priority 5 down to 0. The frame's `func_run`s (the
/// particles) ran before; the interface processes run after (see
/// [`Phase`]).
pub fn step<A, S, H>(arena: &mut A, seats: &mut Seats<'_, S>, world: &mut World<'_>, hooks: &mut H)
where
    A: Arena,
    S: Seat<A::Ground> + ?Sized,
    H: MatchHooks<S>,
{
    // One battle, one pair of motion and stat counts (`ftmanager.c:163-164`):
    // a fighter or pool made with a handle of its own joins the items'.
    world.weapons.counts.adopt(&world.items.counts);
    for f in seats.iter_mut().flatten() {
        f.fighter_mut().stats.counts.adopt(&world.items.counts);
    }
    // Priority 5, Ground link: `gcPlayAnimAll` precedes every fighter
    // interrupt and the priority-4 controller.
    hooks.phase(Phase::GroundAnim);
    arena.play_anim();
    hooks.begin(Span::Interrupt);
    interrupt_pass(arena, seats, world, hooks);
    hooks.end(Span::Interrupt);
    // Priority 5, Interface links: the threads that enter the fighters,
    // after their interrupts and before their priority-4 physics plays the
    // entry's first frame (RE-556 G2).
    hooks.phase(Phase::Interface);
    hooks.interface(seats, world.battle.as_deref());
    // `ifCommonAnnounceGoSetStatus` (RE-556 G2b).
    if core::mem::take(&mut world.go) {
        world.locked = false;
        for f in seats.iter_mut().flatten() {
            f.fighter_mut().interface.control_disable = false;
        }
    }
    board_and_stage(arena, seats, world, hooks);
    hooks.begin(Span::Physics);
    physics_pass(arena, seats, world, hooks);
    hooks.end(Span::Physics);
    hooks.begin(Span::Hit);
    hit_pass(arena, seats, world, hooks);
    hooks.end(Span::Hit);
    // The stage calls the items made (`grInishiePowerBlockSetDamage` in
    // their hit collisions, `grInishiePowerBlockSetWait` in their main
    // process): no stage process runs between them and here.
    let (ground, props) = arena.split();
    world
        .stage
        .apply_item_events(world.items.take_stage_events(), ground.groups(), props);
    arena.place_item_trees(world.items);
    // `gmRumbleActorProcUpdate`: `gcMakeGObjSPAfter(nGCCommonKindRumble,
    // NULL, nGCCommonLinkIDRumble (13), ...)` with a priority-0 process
    // (gmrumble.c:643); `gcLinkGObjProcess` (objman.c:246) runs a
    // priority's processes in link order, so after the fighters', weapons'
    // and items' priority-0 processes (links 3-5).
    if let Some(r) = world.rumble.as_deref_mut() {
        r.tick();
    }
}

/// Priority 5: every fighter's `ftMainProcUpdateInterrupt`, in port order,
/// each from its seat's input against every other fighter as it stands.
/// Each fighter's grab events land before the next one's half, matching
/// the original's direct status writes ([`crate::grab`] module docs).
#[inline(never)]
fn interrupt_pass<A, S, H>(
    arena: &mut A,
    seats: &mut Seats<'_, S>,
    world: &mut World<'_>,
    hooks: &mut H,
) where
    A: Arena,
    S: Seat<A::Ground> + ?Sized,
    H: MatchHooks<S>,
{
    // `DeadUpFall` drops from above `gGMCameraGObj`'s eye.
    let eye = seats[0].as_deref().and_then(|s| s.camera()).map(|c| c.eye);
    for f in seats.iter_mut().flatten() {
        let f = f.fighter_mut();
        if let Some(eye) = eye {
            f.dead.camera_eye = eye;
        }
        f.interface.control_disable = world.locked;
    }
    for i in 0..SEATS {
        // Master Hand reads his target as it stands (`target_gobj`).
        refresh_boss_targets(seats);
        if seats[i].is_none() {
            continue;
        }
        hooks.phase(Phase::Interrupt(i as u8));
        // The CPU's fighter walks skip itself and its team.
        let opponents = opponents_of(seats, i);
        let Some(seat) = seats[i].as_deref_mut() else {
            continue;
        };
        world.items.publish(seat.fighter_mut());
        let view = InputView {
            opponents: &opponents,
            items: world.items,
            weapons: world.weapons,
            stage: world.stage,
            locked: world.locked,
        };
        seat.interrupt(arena.ground(), &view);
        after_interrupt(seats, i);
        jostle_from(seats, i);
        exchange_from(seats, i);
        flush_fighter_effects(seats, world.effects);
        flush_rumble(seats, world.rumble.as_deref_mut());
    }
}

/// Bonus 2's board check, then priority 4 on the Ground link (the stage
/// controller and what it made) and the GroundDisplay link (the moving
/// groups).
#[inline(never)]
fn board_and_stage<A, S, H>(
    arena: &mut A,
    seats: &mut Seats<'_, S>,
    world: &mut World<'_>,
    hooks: &mut H,
) where
    A: Arena,
    S: HasFighter + ?Sized,
    H: MatchHooks<S>,
{
    // Bonus 2's priority-4 process observes the fighter after interrupts,
    // before priority-3 movement, item hits and death scoring.
    if let (Some(bonus), Some(battle), Some(lead)) = (
        world.bonus.as_deref_mut(),
        world.battle.as_deref_mut(),
        seats[0].as_deref(),
    ) {
        hooks.phase(Phase::Board);
        let lead = lead.fighter();
        if let Some(group) = lead.floor.and_then(|s| arena.floor_group(s.line)) {
            if let Some(i) = bonus.board(lead, group, battle) {
                arena.board(i);
            }
        }
    }
    hooks.phase(Phase::StageController);
    hooks.poison();
    arena.tick_stage(
        world.stage,
        &mut fighters_mut(seats),
        world.items,
        world.started,
    );
    // The finish process runs at priority 4, before fighter physics and
    // death scoring. A timer callback already dispatched keeps its end.
    if let Controller::Bonus3(race) = &world.stage.controller {
        if race.complete {
            if let Some(battle) = world.battle.as_deref_mut() {
                battle.announce_complete(crate::sound::id::nSYAudioVoiceAnnounceComplete);
                battle.add_end_sound(crate::sound::id::nSYAudioFGMBonusComplete);
            }
        }
    }
    // The vapor and sparkles the stage process made.
    world.stage.flush_effects(world.effects);
    // Sector Z's Arwing lasers: `wpManagerMakeWeapon` inside the
    // controller; a pair's second shot only after the first.
    for laser in world.stage.take_lasers() {
        if !world.weapons.spawn_arwing_laser(laser) {
            break;
        }
    }
    if let Some(roll) = arena.arwing_roll() {
        world.weapons.observe_arwing_roll(roll);
    }
    // Priority 4, GroundDisplay link: the moving groups.
    hooks.phase(Phase::Yakumono);
    arena.play_yakumono_anim();
}

/// Priority 4, Fighter link: every fighter's `ftMainProcPhysicsMap` in port
/// order (`ftMainProcPhysicsMapDefault`), then priority 3's
/// `ftMainProcPhysicsMapCapture` for the held fighters, which follow their
/// catchers' (RE-511), then the item, weapon, effect and camera processes.
#[inline(never)]
fn physics_pass<A, S, H>(
    arena: &mut A,
    seats: &mut Seats<'_, S>,
    world: &mut World<'_>,
    hooks: &mut H,
) where
    A: Arena,
    S: Seat<A::Ground> + ?Sized,
    H: MatchHooks<S>,
{
    fighter_physics(arena, seats, world, hooks, false);
    // Priority 3, Item actor link (2), ahead of the Fighter link's capture
    // pass (3): the appearance actor (RE-556 G1). The Star's maker reads
    // the camera's look-at, which the camera (link 9) last moved in the
    // frame before.
    if let Some(camera) = seats[0].as_deref().and_then(|s| s.camera()) {
        world.items.camera_at_x = camera.at.x;
    }
    hooks.phase(Phase::ItemActor);
    {
        let ground = arena.ground();
        world.items.tick_appear(world.started, || ground.surfaces());
    }
    // The capture pass, priority 3 on the Fighter link: before the item,
    // weapon, effect and camera processes.
    fighter_physics(arena, seats, world, hooks, true);
    let ground = arena.ground();
    for f in seats.iter().flatten() {
        world.weapons.observe_owner(f.fighter());
    }
    let blast_zone = ground.blast_zone();
    // `gMPCollisionGroundData->map_bound_*` for the effect processes.
    world.effects.map_bounds(blast_zone);
    for f in seats.iter().flatten() {
        world.items.observe_owner(f.fighter());
    }
    // `itPakkunCommonCheckNoFighter` walks the fighter link's `TopN`s.
    let mut tops = [Vec3::ZERO; SEATS];
    let mut top_count = 0;
    for f in seats.iter().flatten() {
        tops[top_count] = crate::item::pakkun::fighter_top(f.fighter());
        top_count += 1;
    }
    // The weapon link's last pass: free structs for the items' weapon
    // makers, and Onix's rock events.
    hooks.begin(Span::Items);
    hooks.poison();
    world.items.observe_weapons(world.weapons);
    hooks.phase(Phase::Items);
    {
        let (ground, props) = arena.split();
        let map = || ground.surfaces();
        world.items.tick_with_effects(
            map,
            Some(blast_zone),
            &tops[..top_count],
            props,
            world.effects,
        );
        // Monster completion calls the gate from the item main process,
        // before the weapon link and its particle RNG draws.
        world
            .stage
            .apply_item_events(world.items.take_stage_events(), ground.groups(), props);
    }
    for f in seats.iter_mut().flatten() {
        world.items.sync_owner(f.fighter_mut());
    }
    flush_item_rumble(seats, world.items, world.rumble.as_deref_mut());
    world.items.flush_effects(world.effects);
    world.items.flush_monster_shots(world.weapons);
    hooks.end(Span::Items);
    hooks.begin(Span::Weapons);
    hooks.poison();
    hooks.phase(Phase::Weapons);
    let ground = arena.ground();
    let map = || ground.surfaces();
    world.weapons.tick(map, Some(blast_zone));
    for f in seats.iter_mut().flatten() {
        world.weapons.sync_owner(f.fighter_mut());
    }
    // The weapons' main processes' effects: the weapon link runs after the
    // item link (RE-416).
    world.weapons.flush_effects(world.effects);
    hooks.end(Span::Weapons);
    // `grZebesAcidGetLevelInfo` as the acid's process left it this tick.
    let acid = match &world.stage.controller {
        Controller::Zebes(z) => z.level_info(),
        _ => (0.0, 0.0),
    };
    effects_and_camera(ground, seats, world, hooks, acid);
}

/// One half of [`physics_pass`]'s fighters: the default pass (priority 4)
/// or the capture pass (priority 3).
#[inline(never)]
fn fighter_physics<A, S, H>(
    arena: &mut A,
    seats: &mut Seats<'_, S>,
    world: &mut World<'_>,
    hooks: &mut H,
    capture_pass: bool,
) where
    A: Arena,
    S: Seat<A::Ground> + ?Sized,
    H: MatchHooks<S>,
{
    let ground = arena.ground();
    let map = || ground.surfaces();
    for i in 0..SEATS {
        if seats[i]
            .as_deref()
            .is_none_or(|f| crate::grab::runs_in_capture_pass(f.fighter()) != capture_pass)
        {
            continue;
        }
        hooks.phase(if capture_pass {
            Phase::Capture(i as u8)
        } else {
            Phase::Physics(i as u8)
        });
        let held = occupied_cliffs(seats, i);
        refresh_boss_targets(seats);
        let Some(f) = seats[i].as_deref_mut() else {
            continue;
        };
        f.fighter_mut().occupied_cliffs = held;
        hooks.begin(Span::FighterPhysics);
        f.physics(ground);
        hooks.end(Span::FighterPhysics);
        if core::mem::take(&mut f.fighter_mut().dead.died) {
            world.weapons.destroy_boomerang(f.fighter().port);
        }
        if i == 0 {
            // The Boomerang projects through the camera last drawn.
            if let Some(camera) = f.camera() {
                world.weapons.observe_camera(camera);
            }
        }
        world.items.take_requests(f.fighter_mut(), map);
        let spawn = f.fighter_mut().take_weapon_spawn_with_parent();
        exchange_from(seats, i);
        // The motion scripts' effects, made at the end of this fighter's
        // `ftMainProcPhysicsMap`.
        flush_fighter_effects(seats, world.effects);
        flush_rumble(seats, world.rumble.as_deref_mut());
        if let Some((spawn, parent)) = spawn {
            // `wpManagerMakeWeapon`'s `WEAPON_FLAG_COLLPROJECT` pass.
            world.weapons.spawn_projected(spawn, Some(parent), &map);
            // A weapon's making effect (the Blaster's glow), in the
            // fighter's process.
            world.weapons.flush_effects(world.effects);
        }
    }
}

/// Priority 3 past the weapon link (5): the effects' processes (link 6),
/// then the camera's (link 9), whose interests read the weapons where
/// their processes left them (RE-542); the quakes past magnitude 0
/// (priorities 2 to 0) write after it
/// ([`crate::camera::effects_then_camera`]).
#[inline(never)]
fn effects_and_camera<G, S, H>(
    ground: &G,
    seats: &mut Seats<'_, S>,
    world: &mut World<'_>,
    hooks: &mut H,
    acid: (f32, f32),
) where
    G: Ground,
    S: HasFighter + ?Sized,
    H: MatchHooks<S>,
{
    hooks.phase(Phase::Effects);
    let weapons = &*world.weapons;
    crate::camera::effects_then_camera(
        seats,
        world.effects,
        &|magnitude, ticks| ground.quake_translate(magnitude, ticks),
        &|s: &mut Seats<'_, S>| s[0].as_deref_mut().and_then(|f| f.camera_mut()),
        &mut |s: &mut Seats<'_, S>| {
            hooks.phase(Phase::Camera);
            // Master Hand's camera requests reach the camera before its
            // process.
            if let (Some(r), Some(rig)) = (
                take_boss_camera(s),
                s[0].as_deref_mut().and_then(|f| f.rig_mut()),
            ) {
                rig.apply_boss(r);
            }
            hooks.begin(Span::Camera);
            // `gmCameraUpdateInterests`' weapon loop: the
            // `is_camera_follow` weapons, an egg still in Yoshi's hand
            // included.
            let follow = weapons.camera_follow_positions();
            let held = s
                .iter()
                .flatten()
                .filter_map(|f| crate::yoshi::held_egg_position(f.fighter()));
            let follow = crate::camera::WeaponInterests::collect(follow.chain(held));
            if let Some(camera) = s[0].as_deref_mut().and_then(|f| f.camera_mut()) {
                camera.weapon_interests = follow;
                camera.acid_level = acid;
            }
            if let Some(stage) = hooks.camera_stage() {
                camera_pass(s, &stage);
            }
            hooks.end(Span::Camera);
        },
    );
}

/// `ftMainProcSearchCatch`, then `ftMainProcSearchHitAll` (fighters, then
/// weapons), then `ftMainProcParams` for every fighter: the original's
/// process priorities, each over the fighters in port order.
#[inline(never)]
fn hit_pass<A, S, H>(arena: &mut A, seats: &mut Seats<'_, S>, world: &mut World<'_>, hooks: &mut H)
where
    A: Arena,
    S: Seat<A::Ground> + ?Sized,
    H: MatchHooks<S>,
{
    // The battle's team rule, which the match's setup gave the pools.
    let rules = world.weapons.team_rules;
    for i in 0..SEATS {
        if seats[i].is_none() {
            continue;
        }
        hooks.phase(Phase::Catch(i as u8));
        // `ftMainProcSearchCatch` opens with the obstacle search
        // (`ftMainSearchHitHazard`), which reads the others' statuses.
        let (statuses, count) = other_statuses(seats, i);
        let Some(f) = seats[i].as_deref_mut() else {
            continue;
        };
        let (_, props) = arena.split();
        crate::hazard::search_hit_hazard(f.fighter_mut(), world.stage, props, &statuses[..count]);
        // `ftMainSearchFighterCatch` over the fighter link, then the
        // catch of the nearest.
        catch_search(seats, i, rules);
    }
    let ground = arena.ground();
    let map = || ground.surfaces();
    // The catcher's `proc_catch` poses `CatchPull` before the caught
    // fighter's `proc_capture` reads the hand (RE-511).
    for f in seats.iter_mut().flatten() {
        f.settle_catch(ground);
    }
    for i in 0..SEATS {
        exchange_from(seats, i);
    }
    // A fighter caught above is at its catcher's hand this frame.
    for f in seats.iter_mut().flatten() {
        f.settle_capture(ground);
    }
    // `wpProcessProcSearchHitWeapon` reads the weapons as they stand before
    // any hit reacts (RE-416): the port's fighter searches react at once,
    // so the clash search's view is taken here.
    world.weapons.begin_clashes();
    {
        let present: ArrayVec<u8, SEATS> = (0..SEATS as u8)
            .filter(|&i| seats[usize::from(i)].is_some())
            .collect();
        let mut fs = fighters_mut(seats);
        crate::combat::begin_search(&mut fs);
        crate::combat::begin_finish(&mut fs, world.effects);
        // Each fighter's `ftMainProcSearchHitAll` in turn: its searches, then
        // its logged hits' stats and effects, before the next fighter's
        // search (RE-556 G3).
        for (k, &seat) in present.iter().enumerate() {
            hooks.phase(Phase::Hits(seat));
            crate::combat::search_fighter_row(&mut fs, k, rules);
            world.items.search_fighter(fs[k]);
            world.weapons.search_hits(fs[k]);
            for a in 0..fs.len() {
                if a != k {
                    let (attacker, defender) = crate::combat::pair_mut(&mut fs, a, k);
                    crate::link::apply_spin_attack_hits(attacker, defender, rules);
                }
            }
            crate::hazard::search_ground_hit(fs[k], world.stage);
            crate::combat::process_hits(fs[k], world.effects);
        }
    }
    // `wpProcessProcHitCollisions` for the fighters' searches (`proc_hit`,
    // shield, set-off; the reflector and absorb wait for the weapons' turn
    // below): after every fighter's (priority 0 after 1), so two fighters
    // can meet one shot in a frame. PSP deviation:
    // on the N64 they also follow the items' and the weapons' own searches;
    // those read the weapons from the clash view taken before any search.
    world.weapons.react_to_fighter_hits();
    // The items' searches' set-offs and sparks (RE-473).
    hooks.phase(Phase::ItemHits);
    world
        .items
        .search_hurt(&mut fighters_mut(seats), world.weapons);
    world.items.flush_effects(world.effects);
    // `wpProcessProcSearchHitWeapon` for each weapon (priority 1, link 5:
    // after the fighters' and items' searches and their records), then
    // the clashes' `proc_setoff`s, after every `proc_hit`, and their
    // set-offs.
    hooks.phase(Phase::WeaponHits);
    world.weapons.search_weapons();
    world.weapons.finish_clashes();
    for f in seats.iter_mut().flatten() {
        world.weapons.sync_hit_owner(f.fighter_mut());
    }
    world.weapons.flush_clash_effects(world.effects);
    hooks.phase(Phase::Params);
    crate::combat::end_finish(&mut fighters_mut(seats), world.effects, &mut |_| {});
    for f in seats.iter_mut().flatten() {
        f.fighter_mut().resolve_cliff_release(&map);
    }
    for f in seats.iter_mut().flatten() {
        world.items.take_requests(f.fighter_mut(), map);
    }
    hooks.phase(Phase::ItemCollisions);
    {
        let all: ArrayVec<&Fighter, SEATS> = seats.iter().flatten().map(|x| x.fighter()).collect();
        let (ground, props) = arena.split();
        let map = || ground.surfaces();
        world
            .items
            .resolve_with_effects(&all, props, map, world.effects);
    }
    world.items.flush_effects(world.effects);
    for f in seats.iter_mut().flatten() {
        world.items.sync_owner(f.fighter_mut());
    }
    // The weapons' hit collisions (priority 0, link 5): after the items'
    // (link 4, RE-556 G6). PK Fire's flame item is made in its weapon's.
    hooks.phase(Phase::WeaponCollisions);
    {
        let lr: ArrayVec<(u8, f32), SEATS> = seats
            .iter()
            .flatten()
            .map(|x| (x.fighter().port, x.fighter().facing.sign()))
            .collect();
        world
            .weapons
            .react_to_reflectors(&|port| lr.iter().find(|&&(p, _)| p == port).map(|&(_, l)| l));
    }
    // A reflected or absorbed PK Thunder's `is_thunder_destroy`.
    for f in seats.iter_mut().flatten() {
        world.weapons.sync_hit_owner(f.fighter_mut());
    }
    world.weapons.flush_effects(world.effects);
    let ground = arena.ground();
    let map = || ground.surfaces();
    world.items.take_weapon_spawns(world.weapons, map);
    for f in seats.iter_mut().flatten() {
        world.items.record_landed(f.fighter_mut());
    }
    for f in seats.iter_mut().flatten() {
        world.weapons.record_landed(f.fighter_mut());
    }
    for i in 0..SEATS {
        exchange_from(seats, i);
    }
    // The statuses the hit phase set stand at their first frame.
    hooks.phase(Phase::Settle);
    for f in seats.iter_mut().flatten() {
        f.settle_status(ground);
    }
    flush_fighter_effects(seats, world.effects);
    flush_rumble(seats, world.rumble.as_deref_mut());
}

/// `ftMainSearchFighterCatch` for seat `i` over the others, then the catch
/// of the nearest.
fn catch_search<S: HasFighter + ?Sized>(seats: &mut Seats<'_, S>, i: usize, rules: TeamRules) {
    let others = seats
        .iter()
        .enumerate()
        .filter(|&(j, _)| j != i)
        .filter_map(|(_, x)| x.as_deref());
    let caught = seats[i]
        .as_deref()
        .and_then(|f| crate::grab::nearest_catch(f.fighter(), others.map(|o| o.fighter()), rules))
        .and_then(|port| index_of(seats, port));
    if let Some((catcher, other)) = caught.and_then(|j| pair(seats, i, j)) {
        crate::grab::search_catch(catcher, other, rules);
    }
}

/// The others' statuses, for seat `i`'s obstacle search.
fn other_statuses<S: HasFighter + ?Sized>(
    seats: &Seats<'_, S>,
    i: usize,
) -> ([AnyStatus; 3], usize) {
    let mut statuses = [AnyStatus::Common(Status::Wait); 3];
    let mut count = 0;
    for o in others(seats, i) {
        statuses[count] = o.fighter().status.status;
        count += 1;
    }
    (statuses, count)
}

/// The cliffs the other fighters hold, for seat `i`'s cliff catch.
fn occupied_cliffs<S: HasFighter + ?Sized>(
    seats: &Seats<'_, S>,
    i: usize,
) -> [Option<(u16, crate::fighter::Facing)>; 3] {
    let mut held = [None; 3];
    for (slot, o) in held.iter_mut().zip(others(seats, i)) {
        let o = o.fighter();
        *slot = crate::map::is_cliff_hold(o.status.status).then_some((o.cliff.line, o.facing));
    }
    held
}

/// The CPU view of every fighter other than seat `i`'s, by
/// `ftComputerProcessAll`'s team rule.
fn opponents_of<S: HasFighter + ?Sized>(seats: &Seats<'_, S>, i: usize) -> Opponents {
    let Some(me) = seats[i].as_deref() else {
        return Opponents::new();
    };
    seats
        .iter()
        .flatten()
        .filter(|x| crate::computer::behave::is_opponent(me.fighter(), x.fighter()))
        .map(|x| crate::computer::behave::opponent(x.fighter()))
        .collect()
}

/// Every seat but `i`, in port order.
fn others<'a, S: HasFighter + ?Sized>(
    seats: &'a Seats<'_, S>,
    i: usize,
) -> impl Iterator<Item = &'a S> + 'a {
    seats
        .iter()
        .enumerate()
        .filter(move |&(j, _)| j != i)
        .filter_map(|(_, x)| x.as_deref())
}

/// Fighters `a` and `b` of `seats` at once, in that order.
pub fn pair<'b, S: HasFighter + ?Sized>(
    seats: &'b mut Seats<'_, S>,
    a: usize,
    b: usize,
) -> Option<(&'b mut Fighter, &'b mut Fighter)> {
    if a == b || a.max(b) >= seats.len() {
        return None;
    }
    let (lo, hi) = seats.split_at_mut(a.max(b));
    let low = lo[a.min(b)].as_deref_mut()?.fighter_mut();
    let high = hi[0].as_deref_mut()?.fighter_mut();
    Some(if a < b { (low, high) } else { (high, low) })
}

/// The index in `seats` of the fighter on `port`.
pub fn index_of<S: HasFighter + ?Sized>(seats: &Seats<'_, S>, port: u8) -> Option<usize> {
    seats
        .iter()
        .position(|x| x.as_deref().is_some_and(|x| x.fighter().port == port))
}

/// Every fighter in `seats` in port order, for the passes that take them
/// all.
pub fn fighters_mut<'b, S: HasFighter + ?Sized>(
    seats: &'b mut Seats<'_, S>,
) -> ArrayVec<&'b mut Fighter, SEATS> {
    seats
        .iter_mut()
        .flatten()
        .map(|x| x.fighter_mut())
        .collect()
}

/// [`crate::grab::exchange`] from fighter `from` to its grab partner
/// ([`crate::grab::partner`], the fighter the original's direct writes
/// reach through `catch_gobj` or `capture_gobj`), or to the first other
/// fighter in port order when it has never been linked.
pub fn exchange_from<S: HasFighter + ?Sized>(seats: &mut Seats<'_, S>, from: usize) {
    let Some(f) = seats[from].as_deref() else {
        return;
    };
    let to = crate::grab::partner(f.fighter())
        .and_then(|port| index_of(seats, port))
        .filter(|&to| to != from)
        .or_else(|| (0..seats.len()).find(|&i| i != from && seats[i].is_some()));
    if let Some((a, b)) = to.and_then(|to| pair(seats, from, to)) {
        crate::grab::exchange(a, b);
    }
}

/// `ftMainProcUpdateInterrupt`'s jostle for fighter `i`, outside hitlag,
/// against the others as their interrupts have left them (RE-466).
pub fn jostle_from<S: HasFighter + ?Sized>(seats: &mut Seats<'_, S>, i: usize) {
    let Some(me) = seats[i].as_deref() else {
        return;
    };
    if me.fighter().is_in_hitlag() {
        return;
    }
    let mut others = [(crate::fighter::JostleBody::of(me.fighter()), false); 3];
    let mut count = 0;
    for (j, o) in seats.iter().enumerate() {
        if j == i {
            continue;
        }
        if let Some(o) = o.as_deref() {
            others[count] = (crate::fighter::JostleBody::of(o.fighter()), j > i);
            count += 1;
        }
    }
    if let Some(me) = seats[i].as_deref_mut() {
        crate::fighter::jostle(me.fighter_mut(), &others[..count]);
    }
}

/// The half of `ftCommonDeadCheckRebirth` fighter `i` cannot do itself:
/// the rebirth takes the lowest halo no other fighter is using.
pub fn after_interrupt<S: HasFighter + ?Sized>(seats: &mut Seats<'_, S>, i: usize) {
    let mut others = [(AnyStatus::Common(Status::Wait), 0u8); 3];
    let mut count = 0;
    for o in self::others(seats, i) {
        let o = o.fighter();
        others[count] = (o.status.status, o.dead.rebirth.halo_number);
        count += 1;
    }
    let Some(f) = seats[i].as_deref_mut().map(|x| x.fighter_mut()) else {
        return;
    };
    if f.dead.rebirth_pending {
        let halo = crate::dead::halo_number(others[..count].iter().copied());
        crate::dead::rebirth_down(f, halo);
    }
}

/// Every fighter's view of Master Hand's target: the first other fighter
/// in link (port) order (`ftCommonAppearSetStatus`), as it stands now.
pub fn refresh_boss_targets<S: HasFighter + ?Sized>(seats: &mut Seats<'_, S>) {
    for i in 0..seats.len() {
        let is_boss = seats[i]
            .as_deref()
            .is_some_and(|f| f.fighter().kind == crate::fighter::FighterKind::Boss);
        if !is_boss {
            continue;
        }
        let target = others(seats, i).next().map(|t| {
            let t = t.fighter();
            crate::boss::Target {
                port: t.port,
                pos: t.pos,
                floor_line: t.floor.map(|f| f.line),
            }
        });
        if let Some(f) = seats[i].as_deref_mut() {
            f.fighter_mut().boss.target = target;
        }
    }
}

/// The battle camera's process (priority 3, link 9, after every fighter's
/// physics and the item, weapon and effect processes):
/// `gmCameraUpdateInterests` over the fighters in link order, through the
/// lead's (`seats[0]`) camera. The auto demo's focus and the entry focus
/// follow their fighters as this tick left them (`gmCameraUpdatePlayerZoom`
/// reads the fighter's `TopN` live, RE-543).
#[inline(never)]
pub fn camera_pass<S: HasFighter + ?Sized>(seats: &mut Seats<'_, S>, stage: &CameraStage) {
    let Some(lead) = seats[0].as_deref() else {
        return;
    };
    let Some(rig) = lead.rig() else {
        return;
    };
    let (demo_zoom, entry_zoom) = (rig.demo_zoom, rig.entry_zoom);
    let f = lead.fighter();
    let framing = lead.framing();
    let (pos, cam_offset_y) = (f.pos, framing.cam_offset_y);
    let mut list = [Interest::default(); 4];
    let mut count = 0;
    let own = crate::camera::interest_of(f, framing, stage);
    let others = seats[1..]
        .iter()
        .flatten()
        .filter_map(|s| crate::camera::interest_of(s.fighter(), s.framing(), stage));
    for interest in own.into_iter().chain(others).take(list.len()) {
        list[count] = interest;
        count += 1;
    }
    let focus = demo_zoom.and_then(|z| {
        let s = seats
            .iter()
            .flatten()
            .find(|s| s.fighter().port == z.port)?;
        Some((s.fighter().pos, s.framing().cam_offset_y))
    });
    let entry = entry_zoom.and_then(|z| {
        let s = seats.get(usize::from(z.slot))?.as_deref()?;
        Some(EntryFocusTarget {
            pos: s.fighter().pos,
            cam_offset_y: s.framing().cam_offset_y,
            dist: z.dist,
        })
    });
    let Some(rig) = seats[0].as_deref_mut().and_then(|s| s.rig_mut()) else {
        return;
    };
    if let (Some(z), Some((pos, cam_offset_y))) = (rig.demo_zoom.as_mut(), focus) {
        z.pos = pos;
        z.cam_offset_y = cam_offset_y;
    }
    rig.tick(pos, cam_offset_y, &list[..count], entry, stage);
}

/// Master Hand's last camera request this frame
/// (`gmCameraSetStatusMapZoom`, `gmCameraSetStatusDefault`), taken from
/// every fighter.
pub fn take_boss_camera<S: HasFighter + ?Sized>(seats: &mut Seats<'_, S>) -> Option<CameraRequest> {
    let mut request = None;
    for f in seats.iter_mut().flatten() {
        if let Some(r) = f.fighter_mut().boss.camera.take() {
            request = Some(r);
        }
    }
    request
}

/// Makes every fighter's queued effects ([`crate::fteffect`]), in port
/// order, where the process that asked for them ends.
/// The fighters' queued rumble commands, applied in seat order after each
/// fighter's process, as the original calls `gmRumble*` from it. A KO's
/// rumble for the others reads their stocks here, before a later
/// fighter's process changes them (`ftCommonDeadUpdateRumble`).
/// What [`crate::rumble::Rumble::apply`] reads of the battle's players.
pub fn rumble_others<S: HasFighter + ?Sized>(
    seats: &Seats<'_, S>,
) -> [crate::rumble::Other; crate::rumble::PLAYERS] {
    let mut others = [crate::rumble::Other::default(); crate::rumble::PLAYERS];
    for f in seats.iter().flatten() {
        let f = f.fighter();
        if let Some(o) = others.get_mut(usize::from(f.port)) {
            *o = crate::rumble::Other {
                is_man: f.is_man,
                out_of_stock: f.dead.stock_rule && f.stocks == -1,
            };
        }
    }
    others
}

pub fn flush_rumble<S: HasFighter + ?Sized>(
    seats: &mut Seats<'_, S>,
    mut rumble: Option<&mut crate::rumble::Rumble>,
) {
    let others = rumble_others(seats);
    for f in seats.iter_mut().flatten() {
        for cmd in f.fighter_mut().rumble.take() {
            if let Some(r) = rumble.as_deref_mut() {
                r.apply(cmd, &others);
            }
        }
    }
}

/// The items' rumble commands, in the order their processes made them
/// (`itmball.c:390-400`, `itmsbomb.c:336-345`).
pub fn flush_item_rumble<S: HasFighter + ?Sized>(
    seats: &Seats<'_, S>,
    items: &mut crate::item::ItemPool,
    mut rumble: Option<&mut crate::rumble::Rumble>,
) {
    if items.rumble.is_empty() {
        return;
    }
    let others = rumble_others(seats);
    for cmd in items.rumble.take() {
        if let Some(r) = rumble.as_deref_mut() {
            r.apply(cmd, &others);
        }
    }
}

pub fn flush_fighter_effects<S: HasFighter + ?Sized>(
    seats: &mut Seats<'_, S>,
    effects: &mut dyn HitEffectSink,
) {
    for f in seats.iter_mut().flatten() {
        effects.fighter(f.fighter_mut());
    }
}
