//! Host tests for [`super`] (`gmrumble.c`). The expected motor traces come
//! from a second reading (D-034): a throwaway simulator that parses the US
//! tables out of `gmrumble.c` and runs its pointer algorithm literally, not
//! from these Rust tables. Trace tokens: `{tick}{S|X|I}{port}` for
//! `syControllerStartRumble`, `StopRumble`, `InitRumble`; `*` marks a call
//! made between ticks (a stop or `gmRumbleInitPlayers`), after `tick`.

use super::testing::{Call, Recorder};
use super::*;
use std::format;
use std::string::String;
use std::vec::Vec;

#[derive(Clone, Copy)]
enum Step {
    /// `n` actor ticks.
    T(u32),
    /// `gmRumbleSetPlayerRumbleParams(port, id, len)`.
    M(u8, u8, i32),
    /// `gmRumbleStopRumbleID(port, id)`.
    S(u8, u32),
    /// `gmRumbleInitPlayers`.
    P,
}
use Step::*;

fn token(t: u32, c: Call, star: bool) -> String {
    let (k, p) = match c {
        Call::Init(p) => ('I', p),
        Call::Start(p) => ('S', p),
        Call::Stop(p) => ('X', p),
    };
    format!("{t}{k}{p}{}", if star { "*" } else { "" })
}

fn run(steps: &[Step]) -> (Vec<String>, Rumble) {
    let rec = Recorder::install();
    let mut r = Rumble::new();
    let mut out = Vec::new();
    let mut t = 0;
    for &s in steps {
        match s {
            T(n) => {
                for _ in 0..n {
                    t += 1;
                    r.tick();
                    out.extend(rec.take().into_iter().map(|c| token(t, c, false)));
                }
                continue;
            }
            M(p, id, len) => r.make(p, id, len),
            S(p, id) => r.stop_id(p, id),
            P => init_players(),
        }
        out.extend(rec.take().into_iter().map(|c| token(t, c, true)));
    }
    uninstall();
    (out, r)
}

fn scenario(name: &str) -> Vec<Step> {
    if let Some(rest) = name.strip_prefix("id") {
        let (id, len) = rest.split_once("_len").unwrap();
        return vec![M(0, id.parse().unwrap(), len.parse().unwrap()), T(40)];
    }
    match name {
        "preempt" => vec![M(0, 3, 0), T(5), M(0, 10, 0), T(5), M(0, 0, 20), T(40)],
        "overflow" => vec![M(0, 0, 0), M(0, 1, 0), M(0, 4, 0), M(0, 6, 0), T(3)],
        "evict" => vec![M(0, 6, 0), M(0, 7, 0), M(0, 8, 0), M(0, 9, 0), T(2)],
        "extend" => vec![M(0, 6, 10), T(5), M(0, 6, 20), T(25)],
        "dup" => vec![M(0, 6, 10), T(5), M(0, 6, 3), T(25)],
        "restart" => vec![M(0, 1, 0), T(3), M(0, 1, 0), T(8)],
        "stop_mid" => vec![M(0, 3, 0), T(2), S(0, 3), T(3)],
        "stop_to_next" => vec![M(0, 2, 0), M(0, 0, 30), T(3), S(0, 0), T(6)],
        "pause0" => vec![M(0, 0, 30), T(5), P, T(30)],
        "pause1" => vec![M(0, 1, 0), T(2), P, T(6)],
        "two_ports" => vec![M(1, 9, 0), M(3, 4, 0), T(14)],
        _ => unreachable!("{name}"),
    }
}

/// Every scenario's trace, from the second reading.
const EXPECTED: &[(&str, &str)] = &[
        ("id0_len0", "1S0"),
        ("id1_len0", "1S0 3X0 5S0 7X0 9S0 11X0 13S0 15X0 17S0 19X0 21S0 23X0 25S0 27X0 29S0 31X0 33S0 35X0 37S0 39X0"),
        ("id2_len0", "1S0 3X0 7S0 9X0 13S0 15X0 19S0 21X0 25S0 27X0 31S0 33X0 37S0 39X0"),
        ("id3_len0", "1S0 3X0 11S0 13X0 21S0 23X0 31S0 33X0"),
        ("id4_len0", "1S0 13I0 13X0"),
        ("id5_len0", "1S0 3X0 4S0 6X0 7S0 9X0 10S0 12X0 13S0 15X0 16S0 18X0 19S0 21X0 22S0 24X0 25S0 27X0 28S0 30X0 31S0 33X0 34S0 36X0 37S0 39X0 40S0"),
        ("id6_len0", "1S0 4I0 4X0"),
        ("id7_len0", "1S0 3X0 6S0 8X0 11S0 13X0 16S0 18X0 21S0 23X0 26S0 28X0 31S0 33X0 36S0 38X0"),
        ("id8_len0", "1S0 2X0 5S0 6X0 9S0 10X0 13S0 14X0 17S0 18X0 21S0 22X0 25S0 26X0 29S0 30X0 33S0 34X0 37S0 38X0"),
        ("id9_len0", "1S0 9I0 9X0"),
        ("id10_len0", "1S0 17I0 17X0"),
        ("id0_len10", "1S0 11I0 11X0"),
        ("id1_len10", "1S0 3X0 5S0 7X0 9S0 11I0 11X0"),
        ("id2_len10", "1S0 3X0 7S0 9X0 11I0 11X0"),
        ("id3_len10", "1S0 3X0 11I0 11X0"),
        ("id4_len10", "1S0 11I0 11X0"),
        ("id5_len10", "1S0 3X0 4S0 6X0 7S0 9X0 10S0 11I0 11X0"),
        ("id6_len10", "1S0 11I0 11X0"),
        ("id7_len10", "1S0 3X0 6S0 8X0 11I0 11X0"),
        ("id8_len10", "1S0 2X0 5S0 6X0 9S0 10X0 11I0 11X0"),
        ("id9_len10", "1S0 11I0 11X0"),
        ("id10_len10", "1S0 11I0 11X0"),
        ("preempt", "1S0 3X0 11S0 31X0 31S0 33X0 41S0 43X0"),
        ("overflow", "1S0"),
        ("evict", "1S0"),
        ("extend", "1S0 26I0 26X0"),
        ("dup", "1S0 6X0 6S0 11I0 11X0"),
        ("restart", "1S0 3X0 4S0 6X0 8S0 10X0"),
        ("stop_mid", "1S0 2I0* 2X0*"),
        ("stop_to_next", "1S0 3X0* 7S0 9X0"),
        ("pause0", "1S0 5I0* 5X0* 5I1* 5X1* 5I2* 5X2* 5I3* 5X3* 31I0 31X0"),
        ("pause1", "1S0 2I0* 2X0* 2I1* 2X1* 2I2* 2X2* 2I3* 2X3* 3X0 5S0 7X0"),
        ("two_ports", "1S1 1S3 9I1 9X1 13I3 13X3"),
];

#[test]
fn traces_match_the_second_reading() {
    for &(name, want) in EXPECTED {
        let (got, _) = run(&scenario(name));
        assert_eq!(got.join(" "), want, "{name}");
    }
}

#[test]
fn script_0_is_on_until_its_timer_runs_out() {
    // Start on the first tick; the timer reaches 0 after tick 30, and tick
    // 31 frees the script: the empty queue's Init and Stop.
    let (got, r) = run(&[M(0, 0, 30), T(40)]);
    assert_eq!(got, ["1S0", "31I0", "31X0"]);
    assert!(r.players[0].head().event.is_none());
    assert!(!r.players[0].is_active);
}

#[test]
fn a_full_queue_drops_a_lower_priority_make() {
    let (_, r) = run(&scenario("overflow"));
    let p = &r.players[0];
    let ids: Vec<u8> = p
        .order()
        .iter()
        .map(|&l| p.scripts[usize::from(l)].rumble_id)
        .collect();
    assert_eq!(ids, [0, 1, 4]);
}

#[test]
fn equal_priority_goes_first_and_evicts_the_tail() {
    let (_, r) = run(&scenario("evict"));
    let p = &r.players[0];
    let ids: Vec<(u8, bool)> = p
        .order()
        .iter()
        .map(|&l| {
            (
                p.scripts[usize::from(l)].rumble_id,
                p.scripts[usize::from(l)].event.is_some(),
            )
        })
        .collect();
    assert_eq!(ids, [(9, true), (8, true), (7, true)]);
}

#[test]
fn a_shorter_make_of_a_running_id_takes_a_second_slot() {
    // `gmRumbleCheckSetEventID` extends only a longer finite time; a
    // shorter one falls through and queues the same ID again.
    let (_, r) = run(&[M(0, 6, 10), T(5), M(0, 6, 3)]);
    let p = &r.players[0];
    let running: Vec<(u8, i32)> = p
        .order()
        .iter()
        .map(|&l| p.scripts[usize::from(l)])
        .filter(|s| s.event.is_some())
        .map(|s| (s.rumble_id, s.rumble_timer))
        .collect();
    assert_eq!(running, [(6, 3), (6, 5)]);
}

#[test]
fn pause_silences_the_motor_but_keeps_is_active() {
    // `gmRumbleInitPlayers` at the pause: Init and Stop on every port, the
    // queue and `is_active` untouched. Script 0 never toggles again, so the
    // motor stays off until the script ends.
    let (got, r) = run(&[M(0, 0, 30), T(5), P]);
    assert_eq!(
        got[1..],
        ["5I0*", "5X0*", "5I1*", "5X1*", "5I2*", "5X2*", "5I3*", "5X3*"]
    );
    assert!(r.players[0].is_active);
    assert_eq!(r.players[0].head().rumble_timer, 25);
}

#[test]
fn a_looped_script_ends_after_its_loop_count() {
    // Script 8: 8000 passes of Start 1, Stop 3 (4 ticks each). The last
    // pass's Stop ends at tick 32000; tick 32001 reads End.
    let (got, _) = run(&[M(2, 8, 0), T(32005)]);
    assert_eq!(got.len(), 16002);
    assert_eq!(
        got[got.len() - 4..],
        ["31997S2", "31998X2", "32001I2", "32001X2"]
    );
}

#[test]
fn no_motor_installed_is_silent() {
    uninstall();
    let mut r = Rumble::new();
    r.make(0, 1, 0);
    for _ in 0..10 {
        r.tick();
    }
    assert!(r.players[0].is_active);
}

#[test]
fn ko_rumbles_the_other_men_still_in() {
    let rec = Recorder::install();
    let mut r = Rumble::new();
    let others = [
        Other {
            is_man: true,
            out_of_stock: false,
        },
        Other {
            is_man: true,
            out_of_stock: false,
        },
        Other {
            is_man: false,
            out_of_stock: false,
        },
        Other {
            is_man: true,
            out_of_stock: true,
        },
    ];
    r.apply(
        Cmd::Make {
            port: 1,
            id: 0,
            len: 30,
        },
        &others,
    );
    r.apply(Cmd::KoOthers { port: 1 }, &others);
    r.tick();
    assert_eq!(rec.take(), [Call::Start(0), Call::Start(1)]);
    assert_eq!(r.players[0].head().rumble_id, 1);
    assert_eq!(r.players[0].head().rumble_timer, 14);
    assert!(r.players[2].head().event.is_none());
    assert!(r.players[3].head().event.is_none());
    uninstall();
}

#[test]
fn winner_pulse_alternates_then_ends_at_tic_120() {
    let rec = Recorder::install();
    let mut w = WinnerPulse::new(2);
    let mut tics = 0;
    while w.tick() {
        tics += 1;
    }
    assert_eq!(tics, 119);
    let calls = rec.take();
    assert_eq!(calls.len(), 121);
    for (i, c) in calls[..119].iter().enumerate() {
        let tic = i + 1;
        assert_eq!(
            *c,
            if tic % 2 == 1 {
                Call::Start(2)
            } else {
                Call::Stop(2)
            },
            "tic {tic}"
        );
    }
    assert_eq!(calls[119..], [Call::Init(2), Call::Stop(2)]);
    uninstall();
}

#[test]
fn hit_lengths_truncate_like_s32_casts() {
    assert_eq!(damage_len(10), 11);
    assert_eq!(damage_len(1), 4);
    assert_eq!(damage_len(3), 6);
    assert_eq!(attack_len(1), 2);
    assert_eq!(attack_len(7), 5);
    assert_eq!(attack_len(0), 2);
}

#[test]
fn script_tables_are_the_us_arms() {
    assert_eq!(
        SCRIPTS[2],
        &[LoopBegin(8000), Start(2), Stop(4), LoopEnd, End]
    );
    assert_eq!(SCRIPTS[6], &[Start(3), End]);
    assert_eq!(SCRIPTS[9], &[Start(8), End]);
    assert_eq!(SCRIPTS[10], &[Start(16), End]);
}

// ------------------------------------------------------------ call sites

fn queued(f: &mut crate::fighter::Fighter) -> Vec<Cmd> {
    f.rumble.take().collect()
}

#[test]
fn only_a_man_fighter_queues_makes_and_a_demo_none_of_its_stops() {
    use crate::fighter::{Fighter, FighterKind};
    let mut cpu = Fighter::new(FighterKind::Mario, 1, 3);
    on_damage(&mut cpu, 10);
    make(&mut cpu, 7, 0);
    stop(&mut cpu, 7);
    // `ftParamMakeRumble` is gated on Man; the stops only on non-Demo.
    assert_eq!(queued(&mut cpu), [Cmd::Stop { port: 1, id: 7 }]);
    let mut man = Fighter::new(FighterKind::Mario, 2, 3);
    man.is_man = true;
    on_damage(&mut man, 10);
    on_attack(&mut man, 10);
    on_damage(&mut man, -6);
    assert_eq!(
        queued(&mut man),
        [
            Cmd::Make {
                port: 2,
                id: 0,
                len: 11
            },
            Cmd::Make {
                port: 2,
                id: 5,
                len: 7
            }
        ]
    );
    man.is_demo = true;
    stop(&mut man, 2);
    assert!(queued(&mut man).is_empty());
}

#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn a_status_change_stops_2_and_3_and_7_unless_preserved() {
    use crate::fighter::{Fighter, FighterKind};
    use crate::status::{self, AnyStatus, MarioStatus, Status, StatusTiming};
    let stops =
        |ids: &[u32]| -> Vec<Cmd> { ids.iter().map(|&id| Cmd::Stop { port: 0, id }).collect() };
    let mut f = Fighter::new(FighterKind::Mario, 0, 3);
    status::set_status(&mut f, Status::Wait, 0.0, StatusTiming::unknown());
    assert_eq!(queued(&mut f), stops(&[2, 3, 7]));
    // `ftMarioSpecialLwSwitchStatus...`: the Tornado's ground/air switch
    // carries `FTSTATUS_PRESERVE_RUMBLE`.
    f.status.status = AnyStatus::Mario(MarioStatus::SpecialLw);
    status::set_any_status_preserve(
        &mut f,
        AnyStatus::Mario(MarioStatus::SpecialAirLw),
        0.0,
        StatusTiming::unknown(),
        status::Preserve::NONE,
    );
    assert_eq!(queued(&mut f), stops(&[2, 3]));
    status::set_any_status_preserve(
        &mut f,
        Status::Wait.into(),
        0.0,
        StatusTiming::unknown(),
        status::Preserve {
            rumble: true,
            ..status::Preserve::NONE
        },
    );
    assert_eq!(queued(&mut f), stops(&[2, 3]));
    f.is_demo = true;
    status::set_status(&mut f, Status::Wait, 0.0, StatusTiming::unknown());
    assert!(queued(&mut f).is_empty());
}

#[test]
fn preserve_rumble_rows_cover_every_decomp_user() {
    use crate::fighter::FighterKind;
    use crate::status::{AnyStatus, CaptainStatus, MarioStatus, Status};
    let (n, air) = (
        AnyStatus::Captain(CaptainStatus::SpecialN),
        AnyStatus::Captain(CaptainStatus::SpecialAirN),
    );
    assert!(preserved(FighterKind::Captain, n, air));
    assert!(preserved(FighterKind::Captain, air, n));
    assert!(!preserved(FighterKind::Captain, Status::Wait.into(), air));
    let (lw, air_lw) = (
        AnyStatus::Mario(MarioStatus::SpecialLw),
        AnyStatus::Mario(MarioStatus::SpecialAirLw),
    );
    // Luigi runs Mario's specials.
    assert!(preserved(FighterKind::Luigi, lw, air_lw));
    assert!(!preserved(FighterKind::Fox, lw, air_lw));
}

#[test]
fn an_items_make_waits_for_its_players_man_gate() {
    let _rec = Recorder::install();
    let mut r = Rumble::new();
    let mut others = [Other::default(); PLAYERS];
    // A Master Ball's opening for a CPU's port: nothing.
    r.apply(
        Cmd::MakeIfMan {
            port: 1,
            id: 8,
            len: 20,
        },
        &others,
    );
    assert!(r.players[1].head().event.is_none());
    others[1].is_man = true;
    r.apply(
        Cmd::MakeIfMan {
            port: 1,
            id: 8,
            len: 20,
        },
        &others,
    );
    assert_eq!(r.players[1].head().rumble_id, 8);
    assert_eq!(r.players[1].head().rumble_timer, 20);
    uninstall();
}

fn vs_battle() -> crate::battle::Battle {
    use crate::battle::{Battle, GameStatus, Player, Rule};
    let mut players = [Player::default(); 4];
    for (i, p) in players.iter_mut().take(2).enumerate() {
        *p = Player {
            present: true,
            is_human: i == 0,
            team: i as u8,
            ..Player::default()
        };
    }
    let mut b = Battle::new(Rule::Stock, 3, 2, players);
    b.status = GameStatus::Go;
    b
}

fn init_all() -> Vec<Call> {
    (0..4)
        .flat_map(|p| [Call::Init(p), Call::Stop(p)])
        .collect()
}

#[test]
fn the_pause_silences_the_motors_and_the_actor_sleeps_through_it() {
    use crate::battle::Frame;
    let rec = Recorder::install();
    let mut b = vs_battle();
    b.pause();
    assert_eq!(rec.take(), init_all());
    assert_eq!(b.begin_frame(), Frame::Frozen);
    assert!(!b.rumble_runs(Frame::Frozen));
    b.unpause(false);
    assert_eq!(b.begin_frame(), Frame::Frozen);
    assert!(!b.rumble_runs(Frame::Frozen));
    // A+B+R+Z: every process pauses, the motors stop, and Set's
    // `gcRunAll` leaves the actor asleep.
    let mut b = vs_battle();
    b.pause();
    rec.take();
    b.reset();
    assert_eq!(rec.take(), init_all());
    assert_eq!(b.begin_frame(), Frame::Frozen);
    assert!(!b.rumble_runs(Frame::Frozen));
    uninstall();
}

#[test]
fn the_battle_end_runs_the_actor_but_the_bonus_end_stops_it() {
    use crate::battle::Frame;
    let rec = Recorder::install();
    let mut b = vs_battle();
    b.announce_end_1p(false);
    let mut frames = 0;
    loop {
        let f = b.begin_frame();
        if f == Frame::Done {
            assert!(b.rumble_runs(f));
            break;
        }
        assert_eq!(f, Frame::Frozen);
        assert!(b.rumble_runs(f), "frame {frames}");
        frames += 1;
    }
    assert!(!rec.take().contains(&Call::Init(0)));
    let mut b = vs_battle();
    b.is_bonus = true;
    b.announce_complete(crate::sound::id::nSYAudioVoiceAnnounceComplete);
    assert_eq!(b.begin_frame(), Frame::Frozen);
    assert_eq!(rec.take(), init_all());
    assert!(!b.rumble_runs(Frame::Frozen));
    uninstall();
}

#[test]
fn the_results_pulse_a_human_winner_only() {
    use crate::battle::Rule;
    use crate::fighter::FighterKind;
    use crate::results::Results;
    let rec = Recorder::install();
    let kinds = [Some(FighterKind::Mario), Some(FighterKind::Fox), None, None];
    let finished = |human_wins: bool| {
        let mut b = vs_battle();
        b.players[0].score = if human_wins { 2 } else { 0 };
        b.players[1].score = if human_wins { 0 } else { 2 };
        b.rule = Rule::Time;
        b
    };
    let mut r = Results::start(&finished(true), kinds);
    assert_eq!(r.winner, Some(0));
    assert_eq!(rec.take(), init_all());
    let mut calls = Vec::new();
    for _ in 0..130 {
        r.tick(false);
        calls.extend(rec.take());
    }
    assert_eq!(calls.len(), 121);
    assert_eq!(calls[..2], [Call::Start(0), Call::Stop(0)]);
    assert_eq!(calls[119..], [Call::Init(0), Call::Stop(0)]);
    let mut r = Results::start(&finished(false), kinds);
    assert_eq!(r.winner, Some(1));
    rec.take();
    for _ in 0..130 {
        r.tick(false);
    }
    assert!(rec.take().is_empty());
    uninstall();
}
