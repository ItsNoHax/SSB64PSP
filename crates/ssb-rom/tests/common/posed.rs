//! One fighter posed as `psp_runtime::scene::FighterScene` runs it, for
//! the host replays that need the skeleton: the catch and hit searches read
//! joints the clip poses (`explain_movement`, `scene_inputs`). The fighter
//! tick itself is `ssb_pose::PosedFighter`, the one copy both run (D-052).

#![allow(dead_code)]

/// One fighter as `psp_runtime::scene::FighterScene` runs it.
pub use ssb_pose::PosedFighter as Posed;

/// `MPGroundData::map_bound_top` for `p`'s stage, when its replay set the
/// blast zones; `None` leaves Pikachu's Thunder bound unset.
pub fn map_bound_top(p: &Posed) -> Option<f32> {
    p.fighter.dead.bounds.map(|b| b.map.top)
}

/// `grab::exchange` from fighter `from` to the other, as the match loop's
/// `exchange_from` runs it.
pub fn exchange(fighters: &mut [Posed], from: usize) {
    let [a, b] = fighters else { unreachable!() };
    if from == 0 {
        ssb_game::grab::exchange(&mut a.fighter, &mut b.fighter);
    } else {
        ssb_game::grab::exchange(&mut b.fighter, &mut a.fighter);
    }
}

/// The match state a two-fighter replay keeps between ticks: each
/// fighter's jump button's last state (its C buttons), the weapon and item
/// pools and the stage controller (none: the replays run on a bare map).
pub struct Pair {
    pub jump_was_held: [bool; 2],
    pub weapons: Box<ssb_game::weapon::WeaponPool>,
    pub items: Box<ssb_game::item::ItemPool>,
    pub stage: ssb_game::stage::Stage,
    /// The stage's moving groups and each map surface's group, as the
    /// PSP's `StageMap` runs them; `None` keeps every line where the pack
    /// put it (group lines at their group's origin).
    pub groups: Option<(ssb_pose::ground::StageGroups, Vec<u16>)>,
    /// The stage's ground objects, for the stage Pokémon's trees (their
    /// root scripts walk them; [`HostProps`]); `None` plays no item tree.
    pub objects: Option<Box<ssb_rom::ground_obj::GroundObjects>>,
    /// Jungle's Barrel root translation this tick, fed from an N64 trace
    /// (the host plays no ground animation); `None` puts it out of reach.
    pub barrel: Option<ssb_engine::math::Vec3>,
    /// Fighter 1 as Training's dummy runs it: a CPU
    /// (`sc1PTrainingModeUpdateDummyBehavior`'s Stand) whose controller
    /// replaces its pad; `None` drives it from the pad.
    pub cpu: Option<HostCpu>,
    /// The control lock this tick (`World::locked`): Training's menu.
    pub locked: bool,
}

/// A CPU seat on the host, as `psp-game`'s `Dummy` runs one: the computer
/// and the stage it reads (`cpu_world`).
pub struct HostCpu {
    pub computer: Box<ssb_game::computer::Computer>,
    pub stage: ssb_game::dead::StageBounds,
    /// `gSCManagerBattleState->gkind`.
    pub gkind: Option<u8>,
}

impl HostCpu {
    /// `ftComputerSetupAll` for Training's dummy `f` on the pack stage
    /// `stage` with map `map`: level 3 (`TRAINING_CPU_LEVEL`), the menu's
    /// Stand, no trait. `groups` (the groups and each surface's group) as
    /// the stage left them at the fighter's creation: the cliff scan skips
    /// the lines of a moving or switched group
    /// (`mpCollisionCheckExistPlatformLineID`).
    pub fn training_dummy(
        f: &ssb_game::fighter::Fighter,
        stage: &ssb_rom::pack::StageDesc,
        bounds: ssb_game::dead::StageBounds,
        map: &[ssb_game::weapon::MapSurface],
        groups: Option<(&[ssb_game::map::MapGroup], &[u16])>,
    ) -> HostCpu {
        let mut computer = Box::new(ssb_game::computer::Computer::setup(f, 3));
        computer.behavior = ssb_game::computer::Behavior::Stand;
        computer.trait_kind = ssb_game::computer::attack::Trait::None;
        let mut cpu = HostCpu {
            computer,
            stage: bounds,
            gkind: ssb_rom::stage::common_ground_kind(stage.source_file),
        };
        let sight = ssb_game::computer::CpuSight::default();
        let (groups, yakumono) = groups.unwrap_or((&[], &[]));
        let surfaces = || GroupedSurfaces {
            map,
            yakumono,
            groups,
            next: 0,
        };
        let world = cpu_world(cpu.stage, cpu.gkind, surfaces, &[], &sight);
        cpu.computer.setup_world(f, &world);
        cpu
    }

    fn world<'a, F, I>(
        &self,
        surfaces: F,
        opponents: &'a [ssb_game::computer::behave::Opponent],
        sight: &'a ssb_game::computer::CpuSight,
    ) -> ssb_game::computer::behave::World<'a, F>
    where
        F: Fn() -> I,
        I: IntoIterator<Item = ssb_game::weapon::MapSurface>,
    {
        cpu_world(self.stage, self.gkind, surfaces, opponents, sight)
    }
}

/// What a CPU reads of the match this frame (`psp-game`'s `cpu_world`).
fn cpu_world<'a, F, I>(
    stage: ssb_game::dead::StageBounds,
    gkind: Option<u8>,
    surfaces: F,
    opponents: &'a [ssb_game::computer::behave::Opponent],
    sight: &'a ssb_game::computer::CpuSight,
) -> ssb_game::computer::behave::World<'a, F>
where
    F: Fn() -> I,
    I: IntoIterator<Item = ssb_game::weapon::MapSurface>,
{
    let geometry = ssb_game::computer::behave::geometry_bounds(surfaces());
    ssb_game::computer::behave::World {
        surfaces,
        geometry,
        stage,
        gkind,
        opponents,
        items: &sight.items,
        weapon_threats: &sight.weapons,
        team_rules: sight.team_rules,
        is_1p_game: false,
        pk_thunder_trail: sight.pk_thunder_trail,
        twister: sight.twister,
        acid: sight.acid,
    }
}

impl Default for Pair {
    fn default() -> Self {
        Pair {
            jump_was_held: [false; 2],
            weapons: Box::default(),
            items: Box::default(),
            stage: ssb_game::stage::Stage::none(),
            groups: None,
            objects: None,
            barrel: None,
            cpu: None,
            locked: false,
        }
    }
}

/// One match-loop tick of a two-fighter Training replay
/// ([`ssb_game::matchtick::step`]): fighter 0 plays `pad0(tick)` (buttons,
/// stick x, stick y), fighter 1 (the dummy) holds neutral. Returns each
/// fighter's effects queued this tick.
pub fn tick_pair(
    pack: &ssb_rom::pack::Pack<'_>,
    map: &[ssb_game::weapon::MapSurface],
    fighters: &mut [Posed],
    pad0: &dyn Fn(u64) -> (u16, i8, i8),
    tick: u64,
    pair: &mut Pair,
) -> [Vec<ssb_game::fteffect::FighterEffect>; 2] {
    tick_pair_pads(pack, map, fighters, pad0, &|_| (0, 0, 0), tick, pair)
}

/// The host's map: the replay's surfaces, and the stage's moving groups
/// when the replay runs them ([`Pair::groups`]).
pub struct HostGround<'a, 'p> {
    pub pack: &'a ssb_rom::pack::Pack<'p>,
    pub map: &'a [ssb_game::weapon::MapSurface],
    pub blast_zone: ssb_game::status::BlastZone,
    /// The groups and each surface's group.
    pub groups: Option<(&'a mut ssb_pose::ground::StageGroups, &'a [u16])>,
}

/// The surfaces whose group exists, each with its group's motion
/// (`psp_runtime::scene::MapSegments`).
pub struct GroupedSurfaces<'s> {
    map: &'s [ssb_game::weapon::MapSurface],
    yakumono: &'s [u16],
    groups: &'s [ssb_game::map::MapGroup],
    next: usize,
}

impl Iterator for GroupedSurfaces<'_> {
    type Item = ssb_game::weapon::MapSurface;
    fn next(&mut self) -> Option<Self::Item> {
        while let Some(&surface) = self.map.get(self.next) {
            let group = self
                .yakumono
                .get(self.next)
                .and_then(|&y| self.groups.get(usize::from(y)));
            self.next += 1;
            if group.is_none_or(|g| g.exists()) {
                return Some(ssb_game::weapon::MapSurface {
                    motion: group.and_then(|g| g.motion()),
                    ..surface
                });
            }
        }
        None
    }
}

impl ssb_game::matchtick::Ground for HostGround<'_, '_> {
    type Surfaces<'s>
        = GroupedSurfaces<'s>
    where
        Self: 's;

    fn surfaces(&self) -> Self::Surfaces<'_> {
        let (yakumono, groups) = match &self.groups {
            Some((g, y)) => (*y, g.groups.as_slice()),
            None => (&[][..], &[][..]),
        };
        GroupedSurfaces {
            map: self.map,
            yakumono,
            groups,
            next: 0,
        }
    }

    fn groups(&self) -> &[ssb_game::map::MapGroup] {
        self.groups
            .as_ref()
            .map_or(&[], |(g, _)| g.groups.as_slice())
    }

    fn blast_zone(&self) -> ssb_game::status::BlastZone {
        self.blast_zone
    }

    fn quake_translate(&self, magnitude: u8, ticks: u16) -> Option<(f32, f32)> {
        ssb_pose::quake_translate(self.pack, magnitude, ticks)
    }
}

/// No stage objects, no item animations.
pub struct NoProps;
impl ssb_game::stage::StageObjects for NoProps {}
impl ssb_game::item::ItemAnims for NoProps {}

/// The stage items' trees on the host: Saffron's Pokémon play their
/// root scripts through [`ssb_rom::ground_obj::GroundObjects`], as
/// `psp_runtime::scene::ItemAnimsPort` does; no stage object animates.
pub struct HostProps<'a, 'p> {
    pub pack: &'a ssb_rom::pack::Pack<'p>,
    pub objects: Option<&'a mut ssb_rom::ground_obj::GroundObjects>,
    /// [`Pair::barrel`].
    pub barrel: Option<ssb_engine::math::Vec3>,
}

impl HostProps<'_, '_> {
    /// `psp_runtime::scene::item_tree` for the stage Pokémon only.
    fn tree(target: ssb_game::item::ItemAnimTarget) -> Option<(u8, u8)> {
        match target {
            ssb_game::item::ItemAnimTarget::Monster(k) => {
                Some((ssb_rom::ground_obj::MONSTER_FIRST + k as u8, 0))
            }
            _ => None,
        }
    }
}

/// Only Jungle's Barrel has a translation: the trace's ([`Pair::barrel`]).
impl ssb_game::stage::StageObjects for HostProps<'_, '_> {
    fn translate(&self, obj: ssb_game::stage::StageObj) -> ssb_engine::math::Vec3 {
        match (obj, self.barrel) {
            (ssb_game::stage::StageObj::TaruCann, Some(p)) => p,
            (ssb_game::stage::StageObj::TaruCann, None) => {
                ssb_engine::math::Vec3::new(1e9, 1e9, 0.0)
            }
            _ => ssb_engine::math::Vec3::ZERO,
        }
    }
}

impl ssb_game::item::ItemAnims for HostProps<'_, '_> {
    fn root_frame(&self, target: ssb_game::item::ItemAnimTarget) -> f32 {
        match (Self::tree(target), &self.objects) {
            (Some((a, i)), Some(o)) => o.item_root_frame(a, i),
            _ => 0.0,
        }
    }
    fn make(&mut self, target: ssb_game::item::ItemAnimTarget) {
        if let (Some((a, i)), Some(o)) = (Self::tree(target), self.objects.as_mut()) {
            o.item_make(self.pack, a, i);
        }
    }
    fn play(&mut self, target: ssb_game::item::ItemAnimTarget) -> ssb_game::item::RootWrite {
        match (Self::tree(target), self.objects.as_mut()) {
            (Some((a, i)), Some(o)) => ssb_game::item::RootWrite {
                translate: o.item_play(self.pack, a, i),
            },
            _ => ssb_game::item::RootWrite::default(),
        }
    }
    fn root_idle(&self, target: ssb_game::item::ItemAnimTarget) -> bool {
        match (Self::tree(target), &self.objects) {
            (Some((a, i)), Some(o)) => o.item_root_idle(a, i),
            _ => true,
        }
    }
    fn stop_root(&mut self, target: ssb_game::item::ItemAnimTarget) {
        if let (Some((a, i)), Some(o)) = (Self::tree(target), self.objects.as_mut()) {
            o.item_stop_root(a, i);
        }
    }
    fn stop_material(&mut self, target: ssb_game::item::ItemAnimTarget) {
        if let (Some((a, i)), Some(o)) = (Self::tree(target), self.objects.as_mut()) {
            o.item_stop_material(a, i);
        }
    }
}

/// The host's stage: [`HostGround`] and [`HostProps`].
pub struct HostArena<'a, 'p>(pub HostGround<'a, 'p>, pub HostProps<'a, 'p>);

impl<'a, 'p> ssb_game::matchtick::Arena for HostArena<'a, 'p> {
    type Ground = HostGround<'a, 'p>;
    type Props = HostProps<'a, 'p>;

    fn ground(&self) -> &Self::Ground {
        &self.0
    }

    fn split(&mut self) -> (&Self::Ground, &mut Self::Props) {
        (&self.0, &mut self.1)
    }

    fn play_yakumono_anim(&mut self) {
        if let Some((groups, _)) = self.0.groups.as_mut() {
            groups.tick(self.0.pack).unwrap();
        }
    }

    fn floor_group(&self, line: u16) -> Option<u16> {
        let (_, yakumono) = self.0.groups.as_ref()?;
        self.0
            .map
            .iter()
            .zip(yakumono.iter())
            .find(|(s, _)| s.topology.is_some_and(|t| t.line == line))
            .map(|(_, &y)| y)
    }

    /// The stage controller (`psp_runtime::arena`'s `tick_stage`): a
    /// replay with a stage hazard (Hyrule's Twister, Saffron's gate) sets
    /// [`Pair::stage`]. With [`Pair::groups`] the controller writes the
    /// moving groups (the gate's group 3) and reads each line's group;
    /// it still queries the static map.
    fn tick_stage(
        &mut self,
        stage: &mut ssb_game::stage::Stage,
        fighters: &mut [&mut ssb_game::fighter::Fighter],
        items: &mut dyn ssb_game::stage::StageItems,
        started: bool,
    ) {
        let map = self.0.map;
        let Some((groups, yakumono)) = self.0.groups.as_mut() else {
            tick_stage_with(stage, map, fighters, items, started, &mut self.1);
            return;
        };
        let yakumono: &[u16] = yakumono;
        // `mpCollisionSetDObjNoID`: a floor's line id to its group.
        let line_group = |line: u16| {
            map.iter()
                .zip(yakumono)
                .find(|(s, _)| s.topology.is_some_and(|t| t.line == line))
                .map(|(_, &y)| y as u8)
        };
        stage.tick(
            fighters,
            ssb_game::stage::TickInput {
                groups: &mut groups.groups,
                objects: &mut self.1,
                items,
                map: ssb_game::stage::MapQuery {
                    surfaces: || map.iter().copied(),
                    line_group: &line_group,
                },
                started,
            },
        );
    }
}

/// [`ssb_game::stage::Stage::tick`] on the static `map`, no objects, no
/// moving groups.
pub fn tick_stage(
    stage: &mut ssb_game::stage::Stage,
    map: &[ssb_game::weapon::MapSurface],
    fighters: &mut [&mut ssb_game::fighter::Fighter],
    items: &mut dyn ssb_game::stage::StageItems,
    started: bool,
) {
    tick_stage_with(stage, map, fighters, items, started, &mut NoProps);
}

/// [`tick_stage`] with the stage's objects.
fn tick_stage_with(
    stage: &mut ssb_game::stage::Stage,
    map: &[ssb_game::weapon::MapSurface],
    fighters: &mut [&mut ssb_game::fighter::Fighter],
    items: &mut dyn ssb_game::stage::StageItems,
    started: bool,
    objects: &mut dyn ssb_game::stage::StageObjects,
) {
    let line_group = |_: u16| None;
    stage.tick(
        fighters,
        ssb_game::stage::TickInput {
            groups: &mut [],
            objects,
            items,
            map: ssb_game::stage::MapQuery {
                surfaces: || map.iter().copied(),
                line_group: &line_group,
            },
            started,
        },
    );
}

/// One replay fighter with this tick's pad.
pub struct HostSeat<'a, 'p> {
    pub posed: &'a mut Posed,
    pub pack: &'a ssb_rom::pack::Pack<'p>,
    pub input: ssb_engine::input::ControllerState,
    /// The jump button's edges this tick.
    pub tapped: bool,
    pub released: bool,
    /// The CPU that drives this seat instead of `input`.
    pub cpu: Option<&'a mut HostCpu>,
}

impl ssb_game::matchtick::HasFighter for HostSeat<'_, '_> {
    fn fighter(&self) -> &ssb_game::fighter::Fighter {
        &self.posed.fighter
    }
    fn fighter_mut(&mut self) -> &mut ssb_game::fighter::Fighter {
        &mut self.posed.fighter
    }
}

impl ssb_game::matchtick::Seat<HostGround<'_, '_>> for HostSeat<'_, '_> {
    fn interrupt(
        &mut self,
        ground: &HostGround<'_, '_>,
        view: &ssb_game::matchtick::InputView<'_>,
    ) {
        use ssb_game::matchtick::Ground;
        let top = map_bound_top(self.posed);
        let surfaces = || ground.surfaces();
        // `ftComputerProcessAll` sets `fp->input.cp`, which the fighter
        // reads as its controller; a locked fighter runs no CPU.
        if let Some(cpu) = self.cpu.as_deref_mut() {
            self.input = ssb_engine::input::ControllerState::default();
            (self.tapped, self.released) = (false, false);
            if !view.locked {
                let sight = view.sight(self.posed.fighter.port);
                let world = cpu.world(surfaces, view.opponents, &sight);
                cpu.computer.process(&self.posed.fighter, &world);
                self.input = cpu.computer.controller();
            }
        }
        self.posed.tick_interrupt(
            self.pack,
            top,
            self.input,
            self.tapped,
            self.released,
            &surfaces,
        );
    }
    fn physics(&mut self, ground: &HostGround<'_, '_>) {
        use ssb_game::matchtick::Ground;
        self.posed.tick_physics(self.pack, &|| ground.surfaces());
    }
    fn settle_catch(&mut self, ground: &HostGround<'_, '_>) {
        use ssb_game::matchtick::Ground;
        self.posed.settle_catch(self.pack, &|| ground.surfaces());
    }
    fn settle_capture(&mut self, ground: &HostGround<'_, '_>) {
        use ssb_game::matchtick::Ground;
        self.posed.settle_capture(self.pack, &|| ground.surfaces());
    }
    fn settle_status(&mut self, _: &HostGround<'_, '_>) {
        self.posed.settle_status(self.pack);
    }
}

/// No effect runtime: each fighter's queued effects are kept, by seat, as
/// the process that asked for them ends.
struct Collect {
    ports: [u8; 2],
    queued: [Vec<ssb_game::fteffect::FighterEffect>; 2],
}

impl ssb_game::effect::HitEffectSink for Collect {
    fn make(&mut self, _: &ssb_game::effect::HitEffect) {}
    fn fighter(&mut self, f: &mut ssb_game::fighter::Fighter) {
        if let Some(i) = self.ports.iter().position(|&p| p == f.port) {
            self.queued[i].extend(f.effects.iter().copied());
        }
        f.effects.clear();
    }
}

/// A pad's `(buttons, stick x, stick y)` as a connected controller.
fn pad((buttons, stick_x, stick_y): (u16, i8, i8)) -> ssb_engine::input::ControllerState {
    ssb_engine::input::ControllerState {
        buttons: ssb_engine::input::N64Buttons(buttons),
        stick_x,
        stick_y,
        connected: true,
    }
}

/// The jump button's `(tapped, released)` edges on `c` against its last
/// state, which this updates.
fn jump_edges(c: ssb_engine::input::ControllerState, was_held: &mut bool) -> (bool, bool) {
    use ssb_engine::input::N64Buttons;
    let held = c.buttons.0
        & (N64Buttons::C_UP | N64Buttons::C_DOWN | N64Buttons::C_LEFT | N64Buttons::C_RIGHT)
        != 0;
    let edges = (held && !*was_held, !held && *was_held);
    *was_held = held;
    edges
}

/// [`tick_pair`] with fighter 1 on its own pad, `pad1(tick)` (buttons,
/// stick x, stick y), as a human player 2 (VS mode's second `Man`
/// player): its C buttons jump as player 1's do.
pub fn tick_pair_pads(
    pack: &ssb_rom::pack::Pack<'_>,
    map: &[ssb_game::weapon::MapSurface],
    fighters: &mut [Posed],
    pad0: &dyn Fn(u64) -> (u16, i8, i8),
    pad1: &dyn Fn(u64) -> (u16, i8, i8),
    tick: u64,
    pair: &mut Pair,
) -> [Vec<ssb_game::fteffect::FighterEffect>; 2] {
    let c0 = pad(pad0(tick));
    let edges0 = jump_edges(c0, &mut pair.jump_was_held[0]);
    let c1 = pad(pad1(tick));
    let edges1 = jump_edges(c1, &mut pair.jump_was_held[1]);
    tick_seats(pack, map, fighters, [c0, c1], [edges0, edges1], pair)
}

/// One match-loop tick of a two-fighter scripted replay (How to Play's
/// `ftKeyProcessKeyEvents`): each fighter reads its own controller `cp`,
/// its C buttons jumping as a pad's do. Returns each fighter's effects
/// queued this tick.
pub fn tick_scripted(
    pack: &ssb_rom::pack::Pack<'_>,
    map: &[ssb_game::weapon::MapSurface],
    fighters: &mut [Posed],
    cp: [ssb_engine::input::ControllerState; 2],
    pair: &mut Pair,
) -> [Vec<ssb_game::fteffect::FighterEffect>; 2] {
    let [held0, held1] = &mut pair.jump_was_held;
    let edges = [jump_edges(cp[0], held0), jump_edges(cp[1], held1)];
    tick_seats(pack, map, fighters, cp, edges, pair)
}

/// [`ssb_game::matchtick::step`] over two seats on controllers `c` with
/// jump `(tapped, released)` edges `edges`.
fn tick_seats(
    pack: &ssb_rom::pack::Pack<'_>,
    map: &[ssb_game::weapon::MapSurface],
    fighters: &mut [Posed],
    c: [ssb_engine::input::ControllerState; 2],
    edges: [(bool, bool); 2],
    pair: &mut Pair,
) -> [Vec<ssb_game::fteffect::FighterEffect>; 2] {
    let blast_zone = fighters[0].fighter.dead.bounds.map_or(
        ssb_game::status::BlastZone {
            top: 1e9,
            bottom: -1e9,
            left: -1e9,
            right: 1e9,
        },
        |b| b.map,
    );
    let groups = pair.groups.as_mut().map(|(g, y)| (g, y.as_slice()));
    let mut arena = HostArena(
        HostGround {
            pack,
            map,
            blast_zone,
            groups,
        },
        HostProps {
            pack,
            objects: pair.objects.as_deref_mut(),
            barrel: pair.barrel,
        },
    );
    let [a, b] = fighters else { unreachable!() };
    let mut sink = Collect {
        ports: [a.fighter.port, b.fighter.port],
        queued: [Vec::new(), Vec::new()],
    };
    let mut s0 = HostSeat {
        posed: a,
        pack,
        input: c[0],
        tapped: edges[0].0,
        released: edges[0].1,
        cpu: None,
    };
    let mut s1 = HostSeat {
        posed: b,
        pack,
        input: c[1],
        tapped: edges[1].0,
        released: edges[1].1,
        cpu: pair.cpu.as_mut(),
    };
    let mut seats: ssb_game::matchtick::Seats<'_, HostSeat<'_, '_>> =
        [Some(&mut s0), Some(&mut s1), None, None];
    let mut world = ssb_game::matchtick::World {
        weapons: &mut pair.weapons,
        items: &mut pair.items,
        stage: &mut pair.stage,
        effects: &mut sink,
        bonus: None,
        battle: None,
        started: true,
        locked: pair.locked,
        go: false,
    };
    ssb_game::matchtick::step(&mut arena, &mut seats, &mut world, &mut ());
    sink.queued
}
