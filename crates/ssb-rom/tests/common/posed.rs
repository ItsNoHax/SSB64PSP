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

/// One match-loop tick of a two-fighter Training replay, in the match
/// loop's process order (`common/posed.rs`, after `explain_movement`):
/// fighter 0 plays `pad0(tick)` (buttons, stick x, stick y), fighter 1
/// (the dummy) holds neutral. Returns each fighter's effects queued this
/// tick, read before `finish_frame` flushes them.
pub fn tick_pair(
    pack: &ssb_rom::pack::Pack<'_>,
    map: &[ssb_game::weapon::MapSurface],
    fighters: &mut [Posed],
    pad0: &dyn Fn(u64) -> (u16, i8, i8),
    tick: u64,
    jump_was_held: &mut bool,
) -> [Vec<ssb_game::fteffect::FighterEffect>; 2] {
    tick_pair_pads(
        pack,
        map,
        fighters,
        pad0,
        &|_| (0, 0, 0),
        tick,
        jump_was_held,
    )
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
    jump_was_held: &mut bool,
) -> [Vec<ssb_game::fteffect::FighterEffect>; 2] {
    let map_fn = || map.iter().copied();
    let map_fn = &map_fn;
    // Priority 5: input, TransN's sample, the interrupt, jostle and the
    // grab exchange.
    for i in 0..2 {
        let (c, tapped, released) = if i == 0 {
            let (buttons, stick_x, stick_y) = pad0(tick);
            let held = buttons
                & (ssb_engine::input::N64Buttons::C_UP
                    | ssb_engine::input::N64Buttons::C_DOWN
                    | ssb_engine::input::N64Buttons::C_LEFT
                    | ssb_engine::input::N64Buttons::C_RIGHT)
                != 0;
            let edges = (held && !*jump_was_held, !held && *jump_was_held);
            *jump_was_held = held;
            (
                ssb_engine::input::ControllerState {
                    buttons: ssb_engine::input::N64Buttons(buttons),
                    stick_x,
                    stick_y,
                    connected: true,
                },
                edges.0,
                edges.1,
            )
        } else {
            let (buttons, stick_x, stick_y) = pad1(tick);
            (
                ssb_engine::input::ControllerState {
                    buttons: ssb_engine::input::N64Buttons(buttons),
                    stick_x,
                    stick_y,
                    connected: true,
                },
                false,
                false,
            )
        };
        let p = &mut fighters[i];
        p.tick_interrupt(pack, map_bound_top(p), c, tapped, released, map_fn);
        if !fighters[i].fighter.is_in_hitlag() {
            let other = ssb_game::fighter::JostleBody::of(&fighters[1 - i].fighter);
            ssb_game::fighter::jostle(&mut fighters[i].fighter, &[(other, i == 0)]);
        }
        exchange(fighters, i);
    }
    // Priority 4: the clip, the physics and map, the joint samples.
    for i in 0..2 {
        fighters[i].tick_physics(pack, map_fn);
        let _ = fighters[i].fighter.take_weapon_spawn();
        exchange(fighters, i);
    }
    // Priorities 1 and 0: the catch search, the hit search, then
    // `ftMainProcParams`.
    let queued;
    for i in 0..2 {
        let [a, b] = &mut fighters[..] else {
            unreachable!()
        };
        let (catcher, other) = if i == 0 { (a, b) } else { (b, a) };
        if ssb_game::grab::nearest_catch(
            &catcher.fighter,
            core::iter::once(&other.fighter),
            ssb_game::team::TeamRules::FREE_FOR_ALL,
        )
        .is_some()
        {
            ssb_game::grab::search_catch(
                &mut catcher.fighter,
                &mut other.fighter,
                ssb_game::team::TeamRules::FREE_FOR_ALL,
            );
        }
    }
    for p in fighters.iter_mut() {
        p.settle_catch(pack, map_fn);
    }
    for i in 0..2 {
        exchange(fighters, i);
    }
    for p in fighters.iter_mut() {
        p.settle_capture(pack, map_fn);
    }
    {
        let [a, b] = &mut fighters[..] else {
            unreachable!()
        };
        let mut both = [&mut a.fighter, &mut b.fighter];
        ssb_game::combat::search_all(&mut both, ssb_game::team::TeamRules::FREE_FOR_ALL);
        queued = [0, 1].map(|i| both[i].effects.iter().copied().collect());
        ssb_game::combat::finish_frame(&mut both);
    }
    for p in fighters.iter_mut() {
        p.fighter.resolve_cliff_release(map_fn);
    }
    for i in 0..2 {
        exchange(fighters, i);
    }
    for p in fighters.iter_mut() {
        p.settle_status(pack);
    }
    queued
}
