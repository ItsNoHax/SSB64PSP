//! Every ledge of the nine VS stages, taken by every fighter (RE-553):
//! the real cliff lines from the pack, the posed fighter and the match
//! loop (`tick_scripted`), so a catch, a climb or a roll ends on the real
//! geometry around the corner. A second fighter hangs from the ledge first
//! in the hog runs (`occupied_cliffs` through the match loop).
//!
//! Each run asserts that no `Cliff*` status outlasts its clip (`CliffWait`
//! its fall wait) and that no status stands still: a status whose status
//! clock and `anim_frame` stay unchanged for [`STUCK`] ticks, hitlag
//! included, is the freeze RE-553 saw (Fox's `CliffCatch` that never ended).

use ssb_engine::input::{ControllerState, N64Buttons};
use ssb_engine::math::Vec3;
use ssb_game::fighter::{Facing, FighterKind};
use ssb_game::status::{self, AnyStatus, FoxStatus, Status, StatusTiming};
use ssb_game::weapon::{MapSurface, MapSurfaceKind};

#[path = "common/posed.rs"]
mod posed;
use posed::{tick_scripted, Pair, Posed};

#[path = "common/dream_land.rs"]
mod dream_land;
use dream_land::{dream_land_map, stage_floors, vs_fighter};

/// Ticks a status may stand still (status clock and `anim_frame`
/// unchanged, hitlag included) before the run counts as frozen.
const STUCK: u32 = 240;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Input {
    /// No input: `CliffWait` until its fall wait drops the fighter.
    Neutral,
    Up,
    /// Stick toward the stage.
    Forward,
    /// Stick up and away from the stage (angle > 50 degrees climbs).
    UpBack,
    /// Stick forward and slightly down (angle > -50 degrees climbs).
    ForwardDown,
    A,
    B,
    Z,
    /// Stick away: falls (`ftCommonCliffClimbOrFallCheckInterruptCommon`).
    Drop,
    /// Stick held up from before the catch: `is_allow_interrupt` stays
    /// false, so the wait runs out.
    HeldUp,
    /// Drop, then up-special back to the ledge.
    DropUpSpecial,
    /// Drop, then side-special (Fox's Illusion) at once.
    DropSideSpecial,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Mod {
    None,
    HighDamage,
    HitlagInWait(u16),
    /// The partner hangs from the ledge first.
    Hogged,
}

fn open_pack() -> Option<Vec<u8>> {
    ssb_tablegen::test_rom(env!("CARGO_MANIFEST_DIR"))?;
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak");
    std::fs::read(path).ok()
}

fn entries(kind: FighterKind) -> Vec<AnyStatus> {
    use ssb_game::status::{
        CaptainStatus, DonkeyStatus, KirbyStatus, LinkStatus, MarioStatus, NessStatus,
        PikachuStatus, PurinStatus, YoshiStatus,
    };
    let mut v: Vec<AnyStatus> = [Status::Fall, Status::JumpAerialF, Status::DamageFall]
        .into_iter()
        .map(AnyStatus::Common)
        .collect();
    v.push(match kind {
        FighterKind::Mario | FighterKind::Luigi => AnyStatus::Mario(MarioStatus::SpecialAirHi),
        FighterKind::Fox => AnyStatus::Fox(FoxStatus::SpecialAirHiEnd),
        FighterKind::Donkey => AnyStatus::Donkey(DonkeyStatus::SpecialAirHi),
        FighterKind::Link => AnyStatus::Link(LinkStatus::SpecialAirHi),
        FighterKind::Yoshi => AnyStatus::Yoshi(YoshiStatus::SpecialAirLwLoop),
        FighterKind::Captain => AnyStatus::Captain(CaptainStatus::SpecialAirHi),
        FighterKind::Kirby => AnyStatus::Kirby(KirbyStatus::SpecialAirHiFall),
        FighterKind::Pikachu => AnyStatus::Pikachu(PikachuStatus::SpecialAirHiEnd),
        FighterKind::Purin => AnyStatus::Purin(PurinStatus::JumpAerialF1),
        FighterKind::Ness => AnyStatus::Ness(NessStatus::SpecialAirHiEnd),
        _ => AnyStatus::Common(Status::FallSpecial),
    });
    v
}

fn is_cliff(s: AnyStatus) -> Option<Status> {
    match s {
        AnyStatus::Common(c)
            if (Status::CliffCatch as u32..=Status::CliffEscapeSlow2 as u32)
                .contains(&(c as u32)) =>
        {
            Some(c)
        }
        _ => None,
    }
}

/// A ledge: its corner and the facing a fighter hangs from it with.
#[derive(Clone, Copy, Debug)]
struct Corner {
    /// The floor line, for the failure message.
    line: u16,
    x: f32,
    y: f32,
    facing: Facing,
}

/// The cliff-flagged ends of every floor line: its lowest-x end is
/// caught facing right, its highest-x end facing left (a line may run
/// either way).
fn corners(map: &[MapSurface]) -> Vec<Corner> {
    /// `x y flags` of one segment end.
    type End = (i16, i16, u16);
    let mut ends: Vec<(u16, End, End)> = Vec::new();
    for s in map {
        let (MapSurfaceKind::Floor, Some(t)) = (s.kind, s.topology) else {
            continue;
        };
        let g = s.segment;
        for (x, y) in [(g.x1, g.y1), (g.x2, g.y2)] {
            let p = (x, y, g.flags);
            match ends.iter_mut().find(|e| e.0 == t.line) {
                None => ends.push((t.line, p, p)),
                Some(e) => {
                    if x < e.1 .0 {
                        e.1 = p;
                    }
                    if x > e.2 .0 {
                        e.2 = p;
                    }
                }
            }
        }
    }
    let mut out = Vec::new();
    for (line, lo, hi) in ends {
        for ((x, y, flags), facing) in [(lo, Facing::Right), (hi, Facing::Left)] {
            if flags & ssb_game::collision::flags::CLIFF != 0 {
                out.push(Corner {
                    line,
                    x: f32::from(x),
                    y: f32::from(y),
                    facing,
                });
            }
        }
    }
    out
}

struct Run<'a> {
    pack: &'a ssb_rom::pack::Pack<'a>,
    stage: &'a ssb_rom::pack::StageDesc,
    floors: &'a [(u16, ssb_game::collision::Segment)],
    map: &'a [MapSurface],
}

/// What one run saw.
#[derive(Default)]
struct Seen {
    caught: bool,
    partner_caught: bool,
}

impl Run<'_> {
    /// A fighter `kind` in `entry` beside `c`, its catch point over the
    /// floor, falling onto it.
    fn fighter(&self, kind: FighterKind, port: u8, entry: AnyStatus, c: Corner) -> Posed {
        let d = self.pack.fighter(kind as u32).unwrap();
        let s = c.facing.sign();
        let x = c.x - s * (d.cliffcatch_width - 20.0);
        let y = c.y - d.cliffcatch_height + 30.0;
        let mut f = vs_fighter(
            self.pack,
            self.stage,
            kind,
            port,
            (x, y, c.facing),
            self.floors,
        );
        status::set_any_status(&mut f, entry, 0.0, StatusTiming::unknown());
        f.physics.vel_air = Vec3::new(0.0, -20.0, 0.0);
        if let AnyStatus::Mario(_) = entry {
            f.motion_script.flags[1] = 1;
        }
        Posed::new(self.pack, f)
    }

    fn sweep(&self, kind: FighterKind, entry: AnyStatus, c: Corner, m: Mod, input: Input) -> Seen {
        let label = format!("{kind:?} {entry:?} {c:?} {m:?} {input:?}");
        let mut seen = Seen::default();
        // The partner: the hog hangs from the same ledge first; otherwise
        // it waits far off the stage's side.
        let partner_kind = FighterKind::Mario;
        let mut partner = if m == Mod::Hogged {
            self.fighter(partner_kind, 1, AnyStatus::Common(Status::Fall), c)
        } else {
            let far = Corner {
                x: c.x - c.facing.sign() * 30_000.0,
                y: c.y + 30_000.0,
                ..c
            };
            self.fighter(partner_kind, 1, AnyStatus::Common(Status::Fall), far)
        };
        partner.fighter.dead.is_ignore_dead = true;
        let mut fighters = vec![self.fighter(kind, 0, entry, c), partner];
        if m == Mod::HighDamage {
            fighters[0].fighter.damage = status::CLIFF_DAMAGE_HIGH;
        }
        let mut pair = Pair::default();
        let delay = if m == Mod::Hogged { 20 } else { 0 };
        let (mut current, mut wait_frames, mut free) = (None::<(Status, u32)>, 0u32, 0u32);
        let (mut hit_wait, mut dropped_at) = (false, None::<u32>);
        let mut still = (AnyStatus::Common(Status::Wait), 0u32, 0u32, 0u32);
        for tick in 0..1700u32 {
            let f = &fighters[0].fighter;
            let in_wait = f.status.status == AnyStatus::Common(Status::CliffWait);
            if in_wait {
                wait_frames += 1;
            }
            let away = -f.facing.sign() as i8;
            let mut pad = ControllerState::default();
            let tap = |b: u16| N64Buttons(b);
            if tick < delay {
                // Held off stage until the partner holds the ledge.
            } else if input == Input::HeldUp {
                pad.stick_y = 80;
            } else if in_wait && wait_frames > 2 {
                match input {
                    Input::Up => pad.stick_y = 80,
                    Input::Forward => pad.stick_x = -away * 80,
                    Input::UpBack => {
                        pad.stick_x = away * 40;
                        pad.stick_y = 80;
                    }
                    Input::ForwardDown => {
                        pad.stick_x = -away * 80;
                        pad.stick_y = -40;
                    }
                    Input::A if wait_frames == 3 => pad.buttons = tap(N64Buttons::A),
                    Input::B if wait_frames == 3 => pad.buttons = tap(N64Buttons::B),
                    Input::Z if wait_frames == 3 => pad.buttons = tap(N64Buttons::Z),
                    Input::Drop | Input::DropUpSpecial | Input::DropSideSpecial => {
                        pad.stick_x = away * 80;
                        dropped_at.get_or_insert(tick);
                    }
                    _ => {}
                }
            } else if let Some(at) = dropped_at {
                // Back toward the ledge with the special.
                if tick == at + 6 {
                    match input {
                        Input::DropUpSpecial => {
                            pad.stick_y = 80;
                            pad.buttons = tap(N64Buttons::B);
                        }
                        Input::DropSideSpecial => {
                            pad.stick_x = -away * 80;
                            pad.buttons = tap(N64Buttons::B);
                        }
                        _ => {}
                    }
                } else if tick > at + 6
                    && matches!(input, Input::DropUpSpecial | Input::DropSideSpecial)
                {
                    pad.stick_x = -away * 60;
                }
            }
            let cp = if tick < delay {
                [ControllerState::default(); 2]
            } else {
                [pad, ControllerState::default()]
            };
            if tick < delay {
                // Only the partner moves: the subject waits in the air.
                let keep = fighters[0].fighter.pos;
                let _ = tick_scripted(self.pack, self.map, &mut fighters, cp, &mut pair);
                fighters[0].fighter.pos = keep;
                fighters[0].fighter.physics.vel_air = Vec3::new(0.0, -20.0, 0.0);
                if is_cliff(fighters[1].fighter.status.status).is_some() {
                    seen.partner_caught = true;
                }
                continue;
            }
            let _ = tick_scripted(self.pack, self.map, &mut fighters, cp, &mut pair);
            let f = &mut fighters[0].fighter;
            let now = is_cliff(f.status.status);
            if now == Some(Status::CliffCatch) {
                seen.caught = true;
            }
            if now == Some(Status::CliffWait) && !hit_wait {
                if let Mod::HitlagInWait(n) = m {
                    hit_wait = true;
                    f.hitlag = n;
                }
            }
            current = match (now, current) {
                (Some(s), Some((c, n))) if s == c => Some((s, n + u32::from(!f.is_in_hitlag()))),
                (Some(s), _) => Some((s, 1)),
                (None, _) => None,
            };
            if let Some((s, n)) = current {
                let bound = if s == Status::CliffWait {
                    (if f.damage < status::CLIFF_DAMAGE_HIGH {
                        status::CLIFF_FALL_WAIT_DAMAGE_LOW
                    } else {
                        status::CLIFF_FALL_WAIT_DAMAGE_HIGH
                    }) as u32
                } else {
                    ssb_game::motion::anim_length(kind, s.into()).map_or(4, |l| l as u32)
                };
                assert!(
                    n <= bound + 2,
                    "{label}: {s:?} for {n} frames (bound {bound}) at tick {tick}: clock {} \
                     anim_frame {} hitlag {} fall_wait {} pos {:?}",
                    f.status.clock,
                    f.status.anim_frame,
                    f.hitlag,
                    f.cliff.fall_wait,
                    f.pos,
                );
            }
            // The stuck-timer detector: any status, clock and frame frozen.
            let key = (
                f.status.status,
                f.status.clock.to_bits(),
                f.status.anim_frame.to_bits(),
            );
            if (still.0, still.1, still.2) == key {
                still.3 += 1;
                assert!(
                    still.3 < STUCK,
                    "{label}: {:?} frozen for {} ticks at tick {tick}: clock {} anim_frame {} \
                     pos {:?}",
                    f.status.status,
                    still.3,
                    f.status.clock,
                    f.status.anim_frame,
                    f.pos,
                );
            } else {
                still = (key.0, key.1, key.2, 0);
            }
            if m == Mod::Hogged
                && is_cliff(fighters[1].fighter.status.status).is_some()
                && now == Some(Status::CliffCatch)
            {
                panic!("{label}: caught a ledge the partner holds at tick {tick}");
            }
            // Out of reach, or off the ledge for two seconds.
            free = if now.is_none() { free + 1 } else { 0 };
            if (!seen.caught && tick > 150 + delay) || (seen.caught && free > 120) {
                break;
            }
        }
        seen
    }
}

/// The whole matrix: about 14,700 runs, half a minute optimised and too
/// long unoptimised (`cargo test --release -p ssb-rom --test ledge_sweep`).
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[cfg_attr(
    all(debug_assertions, not(ssb64_stub_tables)),
    ignore = "about 10 minutes unoptimised: run with --release"
)]
fn every_vs_stage_ledge_leaves_its_cliff_statuses_in_time() {
    sweep_vs_ledges(true);
}

/// The matrix's quick subset: every ledge and fighter from a fall, climbing
/// and dropping; Fox (RE-553's report) with each ledge action, the long
/// wait at high damage, the hog and hitlag in the wait.
#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn every_vs_stage_ledge_quick_sweep() {
    sweep_vs_ledges(false);
}

fn sweep_vs_ledges(full: bool) {
    let Some(bytes) = open_pack() else {
        return;
    };
    let pack = ssb_rom::pack::Pack::open(&bytes).unwrap();
    let kinds = [
        FighterKind::Mario,
        FighterKind::Fox,
        FighterKind::Donkey,
        FighterKind::Samus,
        FighterKind::Luigi,
        FighterKind::Link,
        FighterKind::Yoshi,
        FighterKind::Captain,
        FighterKind::Kirby,
        FighterKind::Pikachu,
        FighterKind::Purin,
        FighterKind::Ness,
    ];
    let inputs = [
        Input::Neutral,
        Input::Up,
        Input::Forward,
        Input::UpBack,
        Input::ForwardDown,
        Input::A,
        Input::B,
        Input::Z,
        Input::Drop,
        Input::HeldUp,
        Input::DropUpSpecial,
        Input::DropSideSpecial,
    ];
    let (mut runs, mut caught, mut ledges, mut hogs) = (0u32, 0u32, 0u32, 0u32);
    let mut vs = 0;
    for index in 0..pack.stage_count() {
        let stage = pack.stage(index).unwrap();
        let Some(gkind) = ssb_rom::stage::vs_ground_kind(stage.source_file) else {
            continue;
        };
        vs += 1;
        let (stage, floors) = stage_floors(&pack, index);
        let map = dream_land_map(&pack, &stage);
        let run = Run {
            pack: &pack,
            stage: &stage,
            floors: &floors,
            map: &map,
        };
        let corners = corners(&map);
        let mut stage_caught = 0;
        for &c in &corners {
            let mut corner_caught = false;
            for kind in kinds {
                for (e, entry) in entries(kind).into_iter().enumerate() {
                    if !full && e > 0 {
                        break;
                    }
                    for &input in &inputs {
                        // The long waits and the specials once per
                        // fighter and ledge: on the first entry.
                        let first = e == 0;
                        let fox = kind == FighterKind::Fox;
                        let quick = if fox {
                            !matches!(input, Input::UpBack | Input::ForwardDown | Input::HeldUp)
                        } else {
                            matches!(input, Input::Up | Input::Drop)
                        };
                        if !full && !quick {
                            continue;
                        }
                        let mods: &[Mod] = match input {
                            Input::Neutral | Input::HeldUp if !first => continue,
                            Input::Neutral => &[Mod::None, Mod::HighDamage, Mod::Hogged],
                            Input::DropUpSpecial | Input::DropSideSpecial if !first => continue,
                            Input::Up | Input::A | Input::Z if first => &[
                                Mod::None,
                                Mod::HighDamage,
                                Mod::HitlagInWait(1),
                                Mod::HitlagInWait(13),
                            ],
                            _ => &[Mod::None],
                        };
                        for &m in mods {
                            let quick = match (input, m) {
                                (Input::Neutral, m) => fox && m != Mod::None,
                                (Input::Up, Mod::HitlagInWait(13)) => fox,
                                (_, m) => m == Mod::None,
                            };
                            if !full && !quick {
                                continue;
                            }
                            runs += 1;
                            let seen = run.sweep(kind, entry, c, m, input);
                            if m == Mod::Hogged {
                                hogs += u32::from(seen.partner_caught);
                            }
                            if seen.caught {
                                caught += 1;
                                corner_caught = true;
                            }
                        }
                    }
                }
            }
            ledges += 1;
            eprintln!(
                "gkind {gkind} line {} ({}, {}): caught {corner_caught}, hogs so far {hogs}",
                c.line, c.x, c.y
            );
            stage_caught += u32::from(corner_caught);
        }
        let _ = stage_caught;
    }
    eprintln!("real ledge sweep: {vs} stages, {ledges} ledges, {runs} runs, {caught} catches");
    assert_eq!(vs, 9);
    assert!(caught > runs / 3, "{caught} of {runs}");
}
