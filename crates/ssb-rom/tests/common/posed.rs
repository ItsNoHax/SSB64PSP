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

/// The match state a two-fighter replay keeps between ticks: the jump
/// button's last state (fighter 0's C buttons), the weapon and item pools
/// and the stage controller (none: the replays run on a bare map).
pub struct Pair {
    pub jump_was_held: bool,
    pub weapons: Box<ssb_game::weapon::WeaponPool>,
    pub items: Box<ssb_game::item::ItemPool>,
    pub stage: ssb_game::stage::Stage,
}

impl Default for Pair {
    fn default() -> Self {
        Pair {
            jump_was_held: false,
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
    use ssb_engine::input::{ControllerState, N64Buttons};
    let pad = |(buttons, stick_x, stick_y): (u16, i8, i8)| ControllerState {
        buttons: N64Buttons(buttons),
        stick_x,
        stick_y,
        connected: true,
    };
    let c0 = pad(pad0(tick));
    let held = c0.buttons.0
        & (N64Buttons::C_UP | N64Buttons::C_DOWN | N64Buttons::C_LEFT | N64Buttons::C_RIGHT)
        != 0;
    let edges = (held && !pair.jump_was_held, !held && pair.jump_was_held);
    pair.jump_was_held = held;
    let c1 = pad(pad1(tick));
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
        input: c0,
        tapped: edges.0,
        released: edges.1,
    };
    let mut s1 = HostSeat {
        posed: b,
        pack,
        input: c1,
        tapped: false,
        released: false,
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
    };
    ssb_game::matchtick::step(&mut arena, &mut seats, &mut world, &mut ());
    sink.queued
}
