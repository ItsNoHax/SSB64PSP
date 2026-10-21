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
}

impl Default for Pair {
    fn default() -> Self {
        Pair {
            jump_was_held: [false; 2],
            weapons: Box::default(),
            items: Box::default(),
            stage: ssb_game::stage::Stage::none(),
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

/// The host's map: the replay's static surfaces, no moving groups.
pub struct HostGround<'a, 'p> {
    pub pack: &'a ssb_rom::pack::Pack<'p>,
    pub map: &'a [ssb_game::weapon::MapSurface],
    pub blast_zone: ssb_game::status::BlastZone,
}

impl ssb_game::matchtick::Ground for HostGround<'_, '_> {
    type Surfaces<'s>
        = core::iter::Copied<core::slice::Iter<'s, ssb_game::weapon::MapSurface>>
    where
        Self: 's;

    fn surfaces(&self) -> Self::Surfaces<'_> {
        self.map.iter().copied()
    }

    fn groups(&self) -> &[ssb_game::map::MapGroup] {
        &[]
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

/// The host's stage: [`HostGround`] and no objects.
pub struct HostArena<'a, 'p>(pub HostGround<'a, 'p>, pub NoProps);

impl<'a, 'p> ssb_game::matchtick::Arena for HostArena<'a, 'p> {
    type Ground = HostGround<'a, 'p>;
    type Props = NoProps;

    fn ground(&self) -> &Self::Ground {
        &self.0
    }

    fn split(&mut self) -> (&Self::Ground, &mut Self::Props) {
        (&self.0, &mut self.1)
    }

    /// The stage controller on the replay's static map
    /// (`psp_runtime::arena`'s `tick_stage` without moving groups): a
    /// replay with a stage hazard (Hyrule's Twister) sets [`Pair::stage`].
    fn tick_stage(
        &mut self,
        stage: &mut ssb_game::stage::Stage,
        fighters: &mut [&mut ssb_game::fighter::Fighter],
        items: &mut dyn ssb_game::stage::StageItems,
        started: bool,
    ) {
        tick_stage(stage, self.0.map, fighters, items, started);
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
    let line_group = |_: u16| None;
    stage.tick(
        fighters,
        ssb_game::stage::TickInput {
            groups: &mut [],
            objects: &mut NoProps,
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
    fn interrupt(&mut self, ground: &HostGround<'_, '_>, _: &ssb_game::matchtick::InputView<'_>) {
        use ssb_game::matchtick::Ground;
        let top = map_bound_top(self.posed);
        let surfaces = || ground.surfaces();
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
/// stick x, stick y; no jump edges), as a human player 2.
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
    tick_seats(
        pack,
        map,
        fighters,
        [c0, c1],
        [edges0, (false, false)],
        pair,
    )
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
    let mut arena = HostArena(
        HostGround {
            pack,
            map,
            blast_zone,
        },
        NoProps,
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
    };
    let mut s1 = HostSeat {
        posed: b,
        pack,
        input: c[1],
        tapped: edges[1].0,
        released: edges[1].1,
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
        locked: false,
        go: false,
    };
    ssb_game::matchtick::step(&mut arena, &mut seats, &mut world, &mut ());
    sink.queued
}
