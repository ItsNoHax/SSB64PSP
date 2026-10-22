//! Training hit replays against N64 traces: thrown items, weapons and
//! hit order on the dummy (warp ROMs `re473/warp/warp_*`, Training on
//! Peach's Castle, P1 at the left spawn, the dummy Mario P2 at the right).
//!
//! The rows come from `b30set/train_hit.py` through `hitrows.py`. Per row:
//! `t | p1 | p2 | items | weapons`, each fighter `status anim_frame x y
//! percent hitlag vel_damage_air.x .y` (`fp+0x24`, `g+0x78`, TopN
//! translate, `fp+0x2C`, `fp+0x40`, `fp+0x54/0x58`), each item and weapon
//! `kind x y vel.x vel.y` in link order, `;` apart (a held item's
//! translate is its DObj's under the hand, not compared). The Castle's Bumper (`ITKind` 23) is left
//! out: the replays run on a bare map. N64 row `t` is replay tick `t`; a
//! route step starting at `s` acts on frame `s`.

use ssb_capture::script::button;
use ssb_game::fighter::FighterKind;

#[path = "common/posed.rs"]
mod posed;
use posed::{tick_pair_pads, HostCpu, Pair, Posed};

#[path = "common/dream_land.rs"]
mod dream_land;
use dream_land::{
    camera_stage, dream_land_map, map_groups, stage_bounds, stage_floors, training_fighter,
    vs_fighter,
};

/// Peach's Castle in the pack.
const CASTLE: u32 = 4;

/// Dream Land in the pack.
const DREAM_LAND: u32 = 0;

/// The Training menu's Z button, absent from `ssb_capture::script::button`.
const Z: u16 = 0x2000;

fn open_pack() -> Option<Vec<u8>> {
    ssb_tablegen::test_rom(env!("CARGO_MANIFEST_DIR"))?;
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak"),
    )
    .ok()
}

/// The trace prints two or three decimals. A NaN on the host side is a
/// column the replay does not model.
fn near(a: f32, b: f32) -> bool {
    a.is_nan() || (a - b).abs() < 0.0051
}

/// The camera columns' tolerance: the trace prints four decimals of
/// values near 4000, which the fighters' and weapons' own rounding moves.
const CAMERA_TOLERANCE: f32 = 0.01;

/// Saffron City's `gkind` (`nGRKindYamabuki`).
const SAFFRON_GKIND: u8 = 7;

/// One parsed row: tick, the N64's seed after it and its groups: the two
/// fighters, the items, the weapons and, in a camera replay, the battle
/// camera (`eye at fovy target_dist`: the `CObj`'s eye, at and fovy,
/// `gGMCameraStruct.target_dist`) and, on Saffron, the gate
/// (`gate_status gate_noentry monster_wait gate_wait gate_pos.x
/// gate_pos.y`) and, in a CPU replay, the dummy's computer (`objective
/// item_track_wait`).
struct Row {
    t: u64,
    seed: Option<i32>,
    groups: [Vec<Vec<f32>>; 7],
}

fn parse(table: &str) -> Vec<Row> {
    table
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let mut parts = l.split('|');
            let mut head = parts.next().unwrap().split_whitespace();
            let t = head.next().unwrap().parse().unwrap();
            let seed = head
                .next()
                .map(|s| u32::from_str_radix(s, 16).unwrap() as i32);
            let mut groups: [Vec<Vec<f32>>; 7] = Default::default();
            for (g, part) in groups.iter_mut().zip(parts) {
                *g = part
                    .split(';')
                    .map(|e| e.split_whitespace().map(|v| v.parse().unwrap()).collect())
                    .filter(|e: &Vec<f32>| !e.is_empty())
                    .collect();
            }
            Row { t, seed, groups }
        })
        .collect()
}

/// `ITKind` of a host item, as the trace prints `ip->kind`.
fn it_kind(k: ssb_game::item::ItemKind) -> f32 {
    use ssb_game::item::ItemKind;
    match k {
        ItemKind::NessPKFire => 20.0,
        ItemKind::LinkBomb => 21.0,
        ItemKind::MMonster(m) => f32::from(m.item_kind()),
        // `nITKindGLucky` onward.
        ItemKind::Monster(m) => 27.0 + m as u8 as f32,
        _ => k.common_index().map_or(-1.0, f32::from),
    }
}

/// A `train_hit.py` route (`start:length:keys;...`, keys `a b z start
/// x=.. y=..`) as the replay's pad and the Training menu's items. START
/// opens and closes the menu, which locks the fighters without stopping
/// the game: while it is open, `y=-80` moves to the Item row, each `x=80`
/// steps right one item and A makes it (`ITKind` steps + 3,
/// `sc1PTrainingModeUpdateItemOption`). On the first row (CP) each `x=80`
/// steps the dummy's behaviour (`sc1PTrainingModeUpdateCPOption`); the
/// menu locks both fighters' control from the opening START's tick to
/// the closing one's (`ftParamLockPlayerControl`).
struct Route {
    /// `(first tick, last tick, (buttons, stick x, stick y))`.
    steps: Vec<(u64, u64, (u16, i8, i8))>,
    /// `(tick, ITKind)`.
    menu: Vec<(u64, u8)>,
    /// `(tick, cp_menu_option)`.
    cp: Vec<(u64, u8)>,
    /// `(opening tick, closing tick)`: the ticks in between are locked.
    locks: Vec<(u64, u64)>,
}

/// `dSC1PTrainingModeDummyBehaviors`, by `cp_menu_option`.
const DUMMY_BEHAVIORS: [ssb_game::computer::Behavior; 5] = {
    use ssb_game::computer::Behavior;
    [
        Behavior::Stand,
        Behavior::Walk,
        Behavior::Evade,
        Behavior::Jump,
        Behavior::Default,
    ]
};

impl Route {
    fn parse(route: &str) -> Self {
        let mut out = Route {
            steps: Vec::new(),
            menu: Vec::new(),
            cp: Vec::new(),
            locks: Vec::new(),
        };
        let (mut open, mut option, mut row, mut cp) = (false, 0u8, 0u8, 0u8);
        for step in route.split(';') {
            let mut it = step.split(':');
            let start: u64 = it.next().unwrap().parse().unwrap();
            let len: u64 = it.next().unwrap().parse().unwrap();
            let (mut buttons, mut x, mut y, mut menu) = (0u16, 0i8, 0i8, false);
            for key in it.next().unwrap().split('+').filter(|k| !k.is_empty()) {
                match key.split_once('=') {
                    Some(("x", v)) => x = v.parse().unwrap(),
                    Some(("y", v)) => y = v.parse().unwrap(),
                    _ => match key {
                        "a" => buttons |= button::A,
                        "b" => buttons |= button::B,
                        "z" => buttons |= Z,
                        "start" => menu = true,
                        _ => panic!("route key {key}"),
                    },
                }
            }
            if menu {
                open = !open;
                if open {
                    out.locks.push((start, u64::MAX));
                } else if let Some(lock) = out.locks.last_mut() {
                    lock.1 = start;
                }
                continue;
            }
            if open {
                if y < 0 {
                    row += 1;
                }
                if x > 0 && row == 0 {
                    cp = (cp + 1) % DUMMY_BEHAVIORS.len() as u8;
                    out.cp.push((start, cp));
                } else if x > 0 {
                    option += 1;
                }
                if buttons & button::A != 0 {
                    out.menu.push((start, option + 3));
                }
                continue;
            }
            out.steps.push((start, start + len - 1, (buttons, x, y)));
        }
        out
    }

    /// Whether the menu holds the fighters' control on `tick`.
    fn locked(&self, tick: u64) -> bool {
        self.locks.iter().any(|&(a, b)| (a..b).contains(&tick))
    }

    fn pad(&self, tick: u64) -> (u16, i8, i8) {
        self.steps
            .iter()
            .find(|(a, b, _)| (*a..=*b).contains(&tick))
            .map_or((0, 0, 0), |s| s.2)
    }
}

/// Who drives Training's dummy in a [`Replay`].
#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum Dummy {
    /// Nobody: it stands (the Stand menu option with no CPU model).
    #[default]
    Still,
    /// Training's CPU on its own team, as the warp boots it.
    Cpu,
    /// Training's CPU moved onto P1's team after frame 0, as a poke did.
    CpuAlly,
}

/// A VS two-pad replay (`b36/mkrom_vs2.py`: both players `Man`, no CPU,
/// items off; traced by `b36/vs_p2.py`). The entries are not replayed:
/// each fighter starts on its first Wait frame, where its entry left it.
struct Vs<'a> {
    /// Player 2's fighter (port 1).
    p2: FighterKind,
    /// Player 2's route (`vs_p2.py`'s `P2`), in [`Route`]'s syntax.
    route2: &'a str,
    /// Each fighter's `(x, y, facing, tick)`: where the entry left it and
    /// the N64 row whose Wait shows `anim_frame` 0.
    spawns: [(f32, f32, ssb_game::fighter::Facing, u64); 2],
}

/// What [`Replay::run_stage`] sets up besides the stage.
#[derive(Clone, Copy, Default)]
struct Setup<'a> {
    /// `dITManagerForceMonsterKind`, as the trace's poke at frame 0 wrote it.
    monster: u8,
    /// The dummy's `fp->percent_damage` written after frame `t`.
    dummy_percent: Option<(u64, u16)>,
    /// Saffron's controller with `monster_wait` written after each `(t, wait)`.
    gate: Option<&'a [(u64, u16)]>,
    /// Who drives Training's dummy.
    dummy: Dummy,
    /// A VS two-pad replay instead of Training's.
    vs: Option<&'a Vs<'a>>,
}

/// A Training hit replay on the Castle.
struct Replay<'a> {
    /// P1's fighter; the dummy is Mario.
    p1: FighterKind,
    /// The N64 trace's route.
    route: &'a str,
    /// The live weapons as `kind x y vx vy`, in link order.
    weapons: &'a dyn Fn(&ssb_game::weapon::WeaponPool) -> Vec<Vec<f32>>,
}

/// The replay's first bad tick, its log and the largest position error.
struct Outcome {
    first_bad: Option<u64>,
    log: String,
    max_pos: f32,
    /// The largest camera column error (eye, at, fovy, target distance).
    max_cam: f32,
}

impl Outcome {
    fn check(self, what: &str) {
        if std::env::var_os("SCENE_TRACE").is_some() {
            println!(
                "{what} (max position error {}, camera {}):\n{}",
                self.max_pos, self.max_cam, self.log
            );
        }
        assert_eq!(self.first_bad, None, "{what}\n{}", self.log);
    }
}

fn fighter_cols(f: &ssb_game::fighter::Fighter) -> Vec<f32> {
    vec![
        f32::from(f.status.status.id()),
        f.status.anim_frame,
        f.pos.x,
        f.pos.y,
        f32::from(f.damage),
        f32::from(f.hitlag),
        f.physics.vel_knockback.x,
        f.physics.vel_knockback.y,
    ]
}

impl Replay<'_> {
    fn run(&self, table: &str) -> Option<Outcome> {
        self.run_poked(table, 0, None)
    }

    /// [`Self::run`] with `dITManagerForceMonsterKind` written to
    /// `monster`, as the trace's RDRAM poke did at frame 0.
    fn run_forced(&self, table: &str, monster: u8) -> Option<Outcome> {
        self.run_poked(table, monster, None)
    }

    /// [`Self::run_forced`], and with `dummy_percent = Some((t, percent))`
    /// the dummy's `fp->percent_damage` written after frame `t`, as the
    /// trace's poke did.
    fn run_poked(
        &self,
        table: &str,
        monster: u8,
        dummy_percent: Option<(u64, u16)>,
    ) -> Option<Outcome> {
        self.run_on(CASTLE, table, monster, dummy_percent)
    }

    /// [`Self::run_poked`] on the pack stage `stage_id`. A table whose rows
    /// carry the camera group also replays the battle camera
    /// (`gmCameraUpdateInterests` over both fighters and the
    /// `is_camera_follow` weapons), from the N64's camera on the first row.
    fn run_on(
        &self,
        stage_id: u32,
        table: &str,
        monster: u8,
        dummy_percent: Option<(u64, u16)>,
    ) -> Option<Outcome> {
        let setup = Setup {
            monster,
            dummy_percent,
            ..Setup::default()
        };
        self.run_stage(stage_id, table, setup)
    }

    /// [`Self::run`] with the dummy as Training's CPU
    /// (`sc1PTrainingModeUpdateDummyBehavior`, level 3, menu Stand until
    /// the route's CP row steps it) on the team of P1, as the trace's
    /// poke of the dummy's `fp->team` did after frame 0: its target walks
    /// (`ftComputerCheckFindTarget`) then find nobody.
    fn run_cpu(&self, table: &str) -> Option<Outcome> {
        let setup = Setup {
            dummy: Dummy::CpuAlly,
            ..Setup::default()
        };
        self.run_stage(CASTLE, table, setup)
    }

    /// [`Self::run_forced`] with the dummy as Training's CPU on its own
    /// team (no poke): after a knock-down it gets up (`DownAttack` and the
    /// rest) as the N64's does.
    fn run_forced_cpu(&self, table: &str, monster: u8) -> Option<Outcome> {
        let setup = Setup {
            monster,
            dummy: Dummy::Cpu,
            ..Setup::default()
        };
        self.run_stage(CASTLE, table, setup)
    }

    /// [`Self::run_forced`] on Saffron with its gate controller, and
    /// `monster_wait` written after frame `t` as the trace's poke did.
    fn run_saffron(&self, table: &str, monster: u8, monster_wait: (u64, u16)) -> Option<Outcome> {
        self.run_saffron_waits(table, monster, &[monster_wait])
    }

    /// [`Self::run_saffron`] with `monster_wait` written after each `(t, wait)`.
    fn run_saffron_waits(
        &self,
        table: &str,
        monster: u8,
        monster_waits: &[(u64, u16)],
    ) -> Option<Outcome> {
        let bytes = open_pack()?;
        let pack = ssb_rom::pack::Pack::open(&bytes).unwrap();
        let stage_id = pack
            .stage_of_file(ssb_rom::stage::VS_GROUND_FILES[SAFFRON_GKIND as usize])
            .unwrap();
        let setup = Setup {
            monster,
            gate: Some(monster_waits),
            ..Setup::default()
        };
        self.run_stage(stage_id, table, setup)
    }

    /// The VS two-pad replay `vs` on Dream Land: P1 plays [`Self::route`],
    /// P2 `vs.route2`.
    fn run_vs(&self, table: &str, vs: &Vs<'_>) -> Option<Outcome> {
        let setup = Setup {
            vs: Some(vs),
            ..Setup::default()
        };
        self.run_stage(DREAM_LAND, table, setup)
    }

    /// [`Self::run_on`]; with `gate = Some(&[(t, wait), ..])` the stage's own
    /// controller runs (`StageInit` from the pack, as `hyrule_pair`) and
    /// Saffron's `monster_wait` is written after frame `t`; `dummy` picks
    /// who drives the dummy; `vs` makes it a VS two-pad replay.
    fn run_stage(&self, stage_id: u32, table: &str, setup: Setup<'_>) -> Option<Outcome> {
        let Setup {
            monster,
            dummy_percent,
            gate,
            dummy,
            vs,
        } = setup;
        let bytes = open_pack()?;
        let pack = ssb_rom::pack::Pack::open(&bytes).unwrap();
        let (stage, segments) = stage_floors(&pack, stage_id);
        let map = dream_land_map(&pack, &stage);
        let p2 = vs.map_or(FighterKind::Mario, |v| v.p2);
        let make = |port: u8| {
            let kind = if port == 0 { self.p1 } else { p2 };
            let mut f = match vs {
                Some(v) => {
                    let (x, y, facing, _) = v.spawns[usize::from(port)];
                    vs_fighter(&pack, &stage, kind, port, (x, y, facing), &segments)
                }
                None => training_fighter(&pack, &stage, kind, port, &segments),
            };
            f.dead.bounds = Some(stage_bounds(&pack, &stage));
            Posed::new(&pack, f)
        };
        let mut fighters: Vec<Posed> = (0..2).map(make).collect();
        let want = parse(table);
        let route = Route::parse(self.route);
        let pad = |t: u64| route.pad(t);
        let route2 = vs.map(|v| Route::parse(v.route2));
        let pad1 = |t: u64| route2.as_ref().map_or((0, 0, 0), |r| r.pad(t));
        // A VS replay starts on the first fighter's first Wait frame.
        let first = vs.map_or(0, |v| v.spawns.iter().map(|s| s.3).min().unwrap());
        let last = want.last().unwrap().t;
        let mut pair = Pair {
            groups: Some((
                ssb_pose::ground::StageGroups::new(&pack, stage_id, &stage),
                map_groups(&pack, &stage),
            )),
            ..Pair::default()
        };
        pair.items.force_monster_kind = monster;
        // Training's battle state turns every item off (psp-game's
        // `world.items.normal_switches.toggles = 0`): Chansey lays no egg.
        pair.items.normal_switches.toggles = 0;
        if gate.is_some() {
            use ssb_game::stage::{MapObject, Stage, StageInit, StageKind};
            let kind = StageKind::from_gkind(SAFFRON_GKIND).unwrap();
            let map_objects: Vec<MapObject> = pack
                .stage_points(&stage)
                .map(|p| MapObject {
                    kind: p.kind,
                    pos: ssb_engine::math::Vec3::new(f32::from(p.x), f32::from(p.y), 0.0),
                })
                .collect();
            let (hazard_attack, hazard_throw) = kind.hazard_descs(stage.hazard);
            let init = StageInit {
                kind,
                map_objects: &map_objects,
                bound_bottom: f32::from(stage.bounds.bottom),
                hazard_attack,
                hazard_throw,
                acid_surface_y: stage.hazard_surface_y,
                bonus3_bumpers: &[],
                player: 0,
            };
            pair.objects = Some(Box::new(ssb_rom::ground_obj::GroundObjects::new(
                &pack,
                stage.source_file,
            )));
            let groups = &mut pair.groups.as_mut().unwrap().0.groups;
            pair.stage = Stage::new(&init, groups, &mut posed::NoProps, &mut *pair.items);
        }
        // `ftComputerSetupAll` at the dummy's creation, after the stage's
        // own init has switched its groups.
        if dummy != Dummy::Still {
            pair.cpu = Some(HostCpu::training_dummy(
                &fighters[1].fighter,
                &stage,
                stage_bounds(&pack, &stage),
                &map,
                pair.groups
                    .as_ref()
                    .map(|(g, y)| (g.groups.as_slice(), y.as_slice())),
            ));
        }
        let mut camera = want[0].groups[4].first().map(|c| {
            let mut rig = ssb_game::camera::CameraRig::default();
            rig.camera.eye = ssb_engine::math::Vec3::new(c[0], c[1], c[2]);
            rig.camera.at = ssb_engine::math::Vec3::new(c[3], c[4], c[5]);
            rig.camera.fovy_degrees = c[6];
            rig.camera.target_dist = c[7];
            (rig, camera_stage(&stage))
        });
        let framing = [self.p1, p2].map(|kind| {
            let d = pack.fighter(kind as u32).unwrap();
            ssb_game::camera::Framing {
                cam_offset_y: d.cam_offset_y,
                zoom_frame: d.camera_zoom,
            }
        });
        let mut out = Outcome {
            first_bad: None,
            log: String::new(),
            max_pos: 0.0,
            max_cam: 0.0,
        };
        let mut effect_draws = false;
        // Row 0 follows the battle's first frame: the scene's first
        // `gcRunAll` already played the fighters' Wait and the stage's
        // groups once (both a frame ahead of a replay that starts at 1).
        for tick in first..=last {
            // The replays have no effect runtime, whose particles draw from
            // the one seed too: each tick starts from the N64's seed, the
            // last one the rows give before it (they list every change).
            let before = want
                .iter()
                .rev()
                .find(|r| r.t < tick && r.seed.is_some())
                .and_then(|r| r.seed);
            if let Some(seed) = before {
                ssb_game::rng::set_seed(seed);
            }
            let start = ssb_game::rng::seed();
            // `sc1PTrainingModeUpdateItemOption`, before the frame's item
            // processes: 200 above P1, rising at 30.
            for &(_, kind) in route.menu.iter().filter(|(t, _)| *t == tick) {
                let mut pos = fighters[0].fighter.pos;
                pos.y += 200.0;
                pos.z = 0.0;
                let vel = ssb_engine::math::Vec3::new(0.0, 30.0, 0.0);
                pair.items
                    .make_setup_common(kind, None, pos, vel, &core::iter::empty);
            }
            for &(_, option) in route.cp.iter().filter(|(t, _)| *t == tick) {
                if let Some(cpu) = pair.cpu.as_mut() {
                    // `sc1PTrainingModeUpdateDummyBehavior`.
                    cpu.computer.behavior = DUMMY_BEHAVIORS[usize::from(option)];
                    cpu.computer.trait_kind = ssb_game::computer::attack::Trait::None;
                }
            }
            pair.locked = route.locked(tick);
            if vs.is_none() || tick > first {
                let _ = tick_pair_pads(&pack, &map, &mut fighters, &pad, &pad1, tick, &mut pair);
            }
            // A VS fighter enters its Wait from its entry on this frame.
            for (port, spawn) in vs.iter().flat_map(|v| v.spawns.iter().enumerate()) {
                if spawn.3 == tick && tick > first {
                    fighters[port] = make(port as u8);
                }
            }
            if dummy == Dummy::CpuAlly && tick == 0 {
                fighters[1].fighter.team = fighters[0].fighter.team;
            }
            if let Some((_, percent)) = dummy_percent.filter(|(t, _)| *t == tick) {
                fighters[1].fighter.damage = percent;
            }
            for &(_, wait) in gate.unwrap_or_default().iter().filter(|(t, _)| *t == tick) {
                if let Some(y) = pair.stage.yamabuki_mut() {
                    y.monster_wait = wait;
                }
            }
            // `gmCameraUpdateInterests` after the frame's fighters and
            // weapons (`matchtick::camera_pass`); row 0 is the start state.
            if let Some((rig, cs)) = camera.as_mut().filter(|_| tick > 0) {
                let held = fighters
                    .iter()
                    .filter_map(|p| ssb_game::yoshi::held_egg_position(&p.fighter));
                rig.camera.weapon_interests = ssb_game::camera::WeaponInterests::collect(
                    pair.weapons.camera_follow_positions().chain(held),
                );
                let interests: Vec<_> = fighters
                    .iter()
                    .zip(framing)
                    .filter_map(|(p, fr)| ssb_game::camera::interest_of(&p.fighter, fr, cs))
                    .collect();
                let lead = &fighters[0].fighter;
                rig.tick(lead.pos, framing[0].cam_offset_y, &interests, None, cs);
            }
            let Some(row) = want.iter().find(|r| r.t == tick) else {
                continue;
            };
            // Gameplay draws are a subset of the N64's frame (effects add
            // the rest): drawing more is a divergence.
            let host_draws = draws(start, ssb_game::rng::seed());
            let n64_draws = before.zip(row.seed).map(|(a, b)| draws(a, b));
            let items: Vec<Vec<f32>> = pair
                .items
                .items()
                .map(|it| {
                    // A held item's translate is its DObj's under the hand
                    // joint, which the host does not keep.
                    let (x, y) = if it.is_hold {
                        (f32::NAN, f32::NAN)
                    } else {
                        (it.pos.x, it.pos.y)
                    };
                    vec![it_kind(it.kind), x, y, it.vel_air.x, it.vel_air.y]
                })
                .collect();
            let [c0, c1] = [0, 1].map(|i| fighter_cols(&fighters[i].fighter));
            // Yoshi's egg in hand is already `wpYoshiEggThrowMakeWeapon`'s
            // weapon on the N64, held at the Egg Throw joint.
            let mut weapons: Vec<Vec<f32>> = fighters
                .iter()
                .filter_map(|p| ssb_game::yoshi::held_egg_position(&p.fighter))
                .map(|e| vec![5.0, e.x, e.y, 0.0, 0.0])
                .collect();
            // So is Samus's charging shot, `wpSamusChargeShotMakeWeapon`'s,
            // at joint 16 (`samus.rs` keeps it as a flag).
            weapons.extend(
                fighters
                    .iter()
                    .filter(|p| p.fighter.samus.charge_shot)
                    .map(|p| {
                        let at = ssb_game::samus::charge_shot_position(&p.fighter);
                        vec![2.0, at.x, at.y, 0.0, 0.0]
                    }),
            );
            weapons.extend((self.weapons)(&pair.weapons));
            let cam: Vec<Vec<f32>> = camera
                .iter()
                .filter(|_| !row.groups[4].is_empty())
                .map(|(rig, _)| {
                    let c = &rig.camera;
                    vec![
                        c.eye.x,
                        c.eye.y,
                        c.eye.z,
                        c.at.x,
                        c.at.y,
                        c.at.z,
                        c.fovy_degrees,
                        c.target_dist,
                    ]
                })
                .collect();
            // The effects drew this frame: a gate draw after them (the
            // close's `monster_wait`) reads a seed the host never had.
            effect_draws |= n64_draws.is_some_and(|n| n > host_draws);
            // The gate; `monster_wait` from the poke on (its first draw
            // precedes the rows' seeds) until the effects draw.
            let gate_row: Vec<Vec<f32>> = pair
                .stage
                .yamabuki()
                .filter(|_| !row.groups[5].is_empty())
                .map(|y| {
                    let poked = gate
                        .and_then(|g| g.first())
                        .is_some_and(|&(t, _)| tick >= t);
                    let wait = if poked && !effect_draws {
                        f32::from(y.monster_wait)
                    } else {
                        f32::NAN
                    };
                    vec![
                        y.status as u8 as f32,
                        f32::from(u8::from(y.no_entry)),
                        wait,
                        f32::from(y.gate_wait),
                        y.gate_pos.x,
                        y.gate_pos.y,
                    ]
                })
                .into_iter()
                .collect();
            let com: Vec<Vec<f32>> = pair
                .cpu
                .iter()
                .filter(|_| !row.groups[6].is_empty())
                .map(|c| {
                    vec![
                        c.computer.objective as u8 as f32,
                        f32::from(c.computer.item_track_wait),
                    ]
                })
                .collect();
            let got: [Vec<Vec<f32>>; 7] = [vec![c0], vec![c1], items, weapons, cam, gate_row, com];
            let mut ok = n64_draws.is_none_or(|n| host_draws <= n);
            // A seed-only row (`t seed`) checks the draws alone.
            if row.groups[0].is_empty() {
                if !ok {
                    out.log
                        .push_str(&format!("{tick} X draws {host_draws} > {n64_draws:?}\n"));
                    out.first_bad.get_or_insert(tick);
                }
                continue;
            }
            for (gi, (g, w)) in got.iter().zip(&row.groups).enumerate() {
                ok &= g.len() == w.len();
                if gi == 4 {
                    for (ge, we) in g.iter().zip(w) {
                        let err = ge
                            .iter()
                            .zip(we)
                            .map(|(a, b)| (a - b).abs())
                            .fold(0.0, f32::max);
                        out.max_cam = out.max_cam.max(err);
                        ok &= ge.len() == we.len() && err < CAMERA_TOLERANCE;
                    }
                    continue;
                }
                for (ge, we) in g.iter().zip(w) {
                    ok &= ge.len() == we.len() && ge.iter().zip(we).all(|(a, b)| near(*a, *b));
                    if gi == 6 {
                        continue;
                    }
                    // Position columns: fighters 2-3, items and weapons 1-2.
                    let at = if ge.len() == 8 { 2..4 } else { 1..3 };
                    for i in at.filter(|&i| i < ge.len().min(we.len())) {
                        out.max_pos = out.max_pos.max((ge[i] - we[i]).abs());
                    }
                }
            }
            let mark = if ok { " " } else { "X" };
            out.log.push_str(&format!(
                "{tick} {mark} rust {got:?} draws {host_draws}\n{tick} {mark}  n64 {:?} draws {n64_draws:?}\n",
                row.groups
            ));
            if !ok && out.first_bad.is_none() {
                out.first_bad = Some(tick);
            }
        }
        Some(out)
    }
}

/// The steps of `syUtilsRandUShort`'s generator from `from` to `to`
/// (`usize::MAX` past 4096).
fn draws(from: i32, to: i32) -> usize {
    let mut s = from;
    for n in 0..4096 {
        if s == to {
            return n;
        }
        s = s.wrapping_mul(214013).wrapping_add(2531011);
    }
    usize::MAX
}

fn no_weapons(_: &ssb_game::weapon::WeaponPool) -> Vec<Vec<f32>> {
    Vec::new()
}

/// The fighters' weapons as `WPKind x y vel.x vel.y`, family by family:
/// link order only while one family is out.
fn fighter_weapons(pool: &ssb_game::weapon::WeaponPool) -> Vec<Vec<f32>> {
    use ssb_engine::math::Vec3;
    let row = |k: f32, p: Vec3, v: Vec3| vec![k, p.x, p.y, v.x, v.y];
    let mut out = Vec::new();
    out.extend(pool.fireballs().map(|w| row(0.0, w.position, w.velocity)));
    out.extend(pool.blasters().map(|w| row(1.0, w.position, w.velocity)));
    out.extend(
        pool.charge_shots()
            .map(|w| row(2.0, w.position, w.velocity)),
    );
    out.extend(pool.eggs().map(|w| row(5.0, w.position, w.velocity)));
    out.extend(pool.boomerangs().map(|w| row(7.0, w.position, w.velocity)));
    out.extend(pool.jolts().map(|w| {
        let k = if w.line_type.is_some() { 10.0 } else { 9.0 };
        row(k, w.position, w.velocity)
    }));
    out
}

/// The Pokémon's weapons as `WPKind x y vel.x vel.y`: Onix's rocks and
/// Saffron's Charmander flame and Venusaur razor leaf.
fn monster_weapons(pool: &ssb_game::weapon::WeaponPool) -> Vec<Vec<f32>> {
    use ssb_game::monster_weapon::ShotKind;
    pool.monster_shots()
        .filter_map(|w| {
            let k = match w.kind {
                ShotKind::IwarkRock => 23.0,
                ShotKind::NyarsCoin => 24.0,
                ShotKind::LizardonFlame => 25.0,
                // Clefairy's swarm is `nWPKindSpearSwarm` too (`itspear.c:91`).
                ShotKind::SpearSwarm | ShotKind::PippiSwarm => 26.0,
                ShotKind::KamexHydro => 27.0,
                ShotKind::StarmieSwift => 28.0,
                ShotKind::DogasSmog => 29.0,
                ShotKind::HitokageFlame => 30.0,
                ShotKind::FushigibanaRazor => 31.0,
                _ => return None,
            };
            Some(vec![
                k,
                w.position.x,
                w.position.y,
                w.velocity.x,
                w.velocity.y,
            ])
        })
        .collect()
}

/// `b30set/hit2` (`warp_link`, `itm2`'s route re-recorded with damage):
/// Training's menu makes a Beam Sword above Link (A at 90), he picks it up
/// (A at 150) and smash-throws it right (Z+A+right at 200). It hits the
/// dummy at frame 211: `Damage` 51, 19 percent, 11 frames of hitlag,
/// knockback (71.327, 65.359); the dummy flies right, then falls as
/// `DamageFall` (57) from x 2330.96. Rows 205-300, after the seed's
/// earlier changes.
const LINK_SWORD_HITS_THE_DUMMY: &str = "
90 69521988
91 a6e9c9a6
92 fe87df14
93 c40df552
205 c40df552 | 110 6.00 -210.00 1399.25 0 0 0.000 0.000 | 10 6.00 765.00 1413.09 0 0 0.000 0.000 | 7 0.00 0.00 0.000 0.000
206 c40df552 | 110 7.00 -210.00 1399.25 0 0 0.000 0.000 | 10 7.00 765.00 1413.09 0 0 0.000 0.000 | 7 0.00 0.00 0.000 0.000
207 491c86cd | 110 8.00 -210.00 1399.25 0 0 0.000 0.000 | 10 8.00 765.00 1413.09 0 0 0.000 0.000 | 7 171.03 1833.66 99.172 12.845
208 491c86cd | 110 9.00 -210.00 1399.25 0 0 0.000 0.000 | 10 9.00 765.00 1413.09 0 0 0.000 0.000 | 7 270.20 1845.30 99.172 11.645
209 491c86cd | 110 10.00 -210.00 1399.25 0 0 0.000 0.000 | 10 10.00 765.00 1413.09 0 0 0.000 0.000 | 7 369.37 1855.75 99.172 10.445
210 491c86cd | 110 11.00 -210.00 1399.25 0 0 0.000 0.000 | 10 11.00 765.00 1413.09 0 0 0.000 0.000 | 7 468.55 1864.99 99.172 9.245
211 4db793af | 110 12.00 -210.00 1399.25 0 0 0.000 0.000 | 51 1.00 765.00 1413.09 19 11 71.327 65.359 | 7 567.72 1873.04 -5.950 22.586
212 074d495a | 110 13.00 -210.00 1399.25 0 0 0.000 0.000 | 51 1.00 765.00 1413.09 19 10 71.327 65.359 | 7 561.77 1894.43 -5.950 21.386
213 074d495a | 110 14.00 -210.00 1399.25 0 0 0.000 0.000 | 51 1.00 765.00 1413.09 19 9 71.327 65.359 | 7 555.82 1914.61 -5.950 20.186
214 074d495a | 110 15.00 -210.00 1399.25 0 0 0.000 0.000 | 51 1.00 765.00 1413.09 19 8 71.327 65.359 | 7 549.87 1933.60 -5.950 18.986
215 074d495a | 110 16.00 -210.00 1399.25 0 0 0.000 0.000 | 51 1.00 765.00 1413.09 19 7 71.327 65.359 | 7 543.92 1951.38 -5.950 17.786
216 074d495a | 110 17.00 -210.00 1399.25 0 0 0.000 0.000 | 51 1.00 765.00 1413.09 19 6 71.327 65.359 | 7 537.97 1967.97 -5.950 16.586
217 074d495a | 110 18.00 -210.00 1399.25 0 0 0.000 0.000 | 51 1.00 765.00 1413.09 19 5 71.327 65.359 | 7 532.02 1983.36 -5.950 15.386
218 074d495a | 110 19.00 -210.00 1399.25 0 0 0.000 0.000 | 51 1.00 765.00 1413.09 19 4 71.327 65.359 | 7 526.06 1997.54 -5.950 14.186
219 074d495a | 110 20.00 -210.00 1399.25 0 0 0.000 0.000 | 51 1.00 765.00 1413.09 19 3 71.327 65.359 | 7 520.11 2010.53 -5.950 12.986
220 074d495a | 110 21.00 -210.00 1399.25 0 0 0.000 0.000 | 51 1.00 765.00 1413.09 19 2 71.327 65.359 | 7 514.16 2022.32 -5.950 11.786
221 074d495a | 110 22.00 -210.00 1399.25 0 0 0.000 0.000 | 51 1.00 765.00 1413.09 19 1 71.327 65.359 | 7 508.21 2032.90 -5.950 10.586
222 074d495a | 110 23.00 -210.00 1399.25 0 0 0.000 0.000 | 51 2.00 835.07 1474.90 19 0 70.074 64.211 | 7 502.26 2042.29 -5.950 9.386
223 074d495a | 10 0.00 -210.00 1399.25 0 0 0.000 0.000 | 51 3.00 903.89 1533.16 19 0 68.820 63.062 | 7 496.31 2050.48 -5.950 8.186
224 074d495a | 10 1.00 -210.00 1399.25 0 0 0.000 0.000 | 51 4.00 971.46 1587.88 19 0 67.567 61.914 | 7 490.36 2057.46 -5.950 6.986
225 074d495a | 10 2.00 -210.00 1399.25 0 0 0.000 0.000 | 51 5.00 1037.78 1639.04 19 0 66.314 60.765 | 7 484.41 2063.25 -5.950 5.786
226 074d495a | 10 3.00 -210.00 1399.25 0 0 0.000 0.000 | 51 6.00 1102.84 1686.66 19 0 65.060 59.617 | 7 478.46 2067.84 -5.950 4.586
227 074d495a | 10 4.00 -210.00 1399.25 0 0 0.000 0.000 | 51 7.00 1166.64 1730.73 19 0 63.807 58.468 | 7 472.51 2071.22 -5.950 3.386
228 074d495a | 10 5.00 -210.00 1399.25 0 0 0.000 0.000 | 51 8.00 1229.20 1771.25 19 0 62.554 57.320 | 7 466.56 2073.41 -5.950 2.186
229 074d495a | 10 6.00 -210.00 1399.25 0 0 0.000 0.000 | 51 9.00 1290.50 1808.22 19 0 61.300 56.171 | 7 460.61 2074.40 -5.950 0.986
230 074d495a | 10 7.00 -210.00 1399.25 0 0 0.000 0.000 | 51 10.00 1350.54 1841.64 19 0 60.047 55.023 | 7 454.66 2074.18 -5.950 -0.214
231 074d495a | 10 8.00 -210.00 1399.25 0 0 0.000 0.000 | 51 11.00 1409.34 1871.51 19 0 58.793 53.874 | 7 448.71 2072.77 -5.950 -1.414
232 074d495a | 10 9.00 -210.00 1399.25 0 0 0.000 0.000 | 51 12.00 1466.88 1897.84 19 0 57.540 52.726 | 7 442.76 2070.15 -5.950 -2.614
233 074d495a | 10 10.00 -210.00 1399.25 0 0 0.000 0.000 | 51 13.00 1523.16 1920.62 19 0 56.287 51.577 | 7 436.81 2066.34 -5.950 -3.814
234 074d495a | 10 11.00 -210.00 1399.25 0 0 0.000 0.000 | 51 14.00 1578.20 1939.85 19 0 55.033 50.429 | 7 430.86 2061.33 -5.950 -5.014
235 074d495a | 10 12.00 -210.00 1399.25 0 0 0.000 0.000 | 51 15.00 1631.98 1955.53 19 0 53.780 49.280 | 7 424.91 2055.11 -5.950 -6.214
236 074d495a | 10 13.00 -210.00 1399.25 0 0 0.000 0.000 | 51 16.00 1684.50 1967.66 19 0 52.527 48.132 | 7 418.96 2047.70 -5.950 -7.414
237 074d495a | 10 14.00 -210.00 1399.25 0 0 0.000 0.000 | 51 17.00 1735.78 1976.24 19 0 51.273 46.983 | 7 413.01 2039.09 -5.950 -8.614
238 074d495a | 10 15.00 -210.00 1399.25 0 0 0.000 0.000 | 51 18.00 1785.80 1981.28 19 0 50.020 45.835 | 7 407.06 2029.27 -5.950 -9.814
239 074d495a | 10 16.00 -210.00 1399.25 0 0 0.000 0.000 | 51 19.00 1834.56 1982.76 19 0 48.766 44.686 | 7 401.11 2018.26 -5.950 -11.014
240 074d495a | 10 17.00 -210.00 1399.25 0 0 0.000 0.000 | 51 20.00 1882.08 1982.30 19 0 47.513 43.538 | 7 395.16 2006.05 -5.950 -12.214
241 074d495a | 10 18.00 -210.00 1399.25 0 0 0.000 0.000 | 51 21.00 1928.34 1980.69 19 0 46.260 42.389 | 7 389.21 1992.63 -5.950 -13.414
242 074d495a | 10 19.00 -210.00 1399.25 0 0 0.000 0.000 | 51 22.00 1973.34 1977.93 19 0 45.006 41.241 | 7 383.26 1978.02 -5.950 -14.614
243 074d495a | 10 20.00 -210.00 1399.25 0 0 0.000 0.000 | 51 23.00 2017.09 1974.02 19 0 43.753 40.092 | 7 377.31 1962.21 -5.950 -15.814
244 074d495a | 10 21.00 -210.00 1399.25 0 0 0.000 0.000 | 51 24.00 2059.59 1968.97 19 0 42.500 38.944 | 7 371.36 1945.19 -5.950 -17.014
245 074d495a | 10 22.00 -210.00 1399.25 0 0 0.000 0.000 | 51 25.00 2100.84 1962.76 19 0 41.246 37.795 | 7 365.41 1926.98 -5.950 -18.214
246 074d495a | 10 23.00 -210.00 1399.25 0 0 0.000 0.000 | 51 26.00 2140.83 1955.41 19 0 39.993 36.647 | 7 359.46 1907.56 -5.950 -19.414
247 074d495a | 10 24.00 -210.00 1399.25 0 0 0.000 0.000 | 51 27.00 2179.57 1946.91 19 0 38.739 35.498 | 7 353.51 1886.95 -5.950 -20.614
248 074d495a | 10 25.00 -210.00 1399.25 0 0 0.000 0.000 | 51 28.00 2217.06 1937.26 19 0 37.486 34.350 | 7 347.56 1865.14 -5.950 -21.814
249 074d495a | 10 26.00 -210.00 1399.25 0 0 0.000 0.000 | 51 0.00 2253.29 1926.46 19 0 36.233 33.201 | 7 341.61 1842.12 -5.950 -23.014
250 4dc8aab5 | 10 27.00 -210.00 1399.25 0 0 0.000 0.000 | 51 0.00 2288.27 1914.51 19 0 34.979 32.053 | 7 335.66 1820.40 -1.459 12.291
251 4dc8aab5 | 10 28.00 -210.00 1399.25 0 0 0.000 0.000 | 51 0.00 2322.00 1901.41 19 0 33.726 30.904 | 7 334.20 1831.49 -1.459 11.091
252 4dc8aab5 | 10 29.00 -210.00 1399.25 0 0 0.000 0.000 | 51 0.00 2354.47 1887.17 19 0 32.473 29.756 | 7 332.74 1841.39 -1.459 9.891
253 4dc8aab5 | 10 30.00 -210.00 1399.25 0 0 0.000 0.000 | 51 0.00 2385.69 1871.78 19 0 31.219 28.607 | 7 331.28 1850.08 -1.459 8.691
254 4dc8aab5 | 10 31.00 -210.00 1399.25 0 0 0.000 0.000 | 51 0.00 2415.65 1855.24 19 0 29.966 27.459 | 7 329.82 1857.57 -1.459 7.491
255 4dc8aab5 | 10 32.00 -210.00 1399.25 0 0 0.000 0.000 | 51 0.00 2444.37 1837.55 19 0 28.713 26.310 | 7 328.36 1863.86 -1.459 6.291
256 4dc8aab5 | 10 33.00 -210.00 1399.25 0 0 0.000 0.000 | 56 0.00 2464.97 1818.71 19 0 -18.253 19.404 | 7 326.90 1868.95 -1.459 5.091
257 4dc8aab5 | 10 34.00 -210.00 1399.25 0 0 0.000 0.000 | 56 2.00 2447.89 1834.47 19 0 -17.088 18.166 | 7 325.44 1872.84 -1.459 3.891
258 4dc8aab5 | 10 35.00 -210.00 1399.25 0 0 0.000 0.000 | 56 4.00 2431.96 1846.60 19 0 -15.923 16.927 | 7 323.98 1875.53 -1.459 2.691
259 4dc8aab5 | 10 36.00 -210.00 1399.25 0 0 0.000 0.000 | 56 6.00 2417.20 1855.09 19 0 -14.759 15.689 | 7 322.52 1877.03 -1.459 1.491
260 4dc8aab5 | 10 37.00 -210.00 1399.25 0 0 0.000 0.000 | 56 8.00 2403.61 1859.94 19 0 -13.594 14.451 | 7 321.07 1877.32 -1.459 0.291
261 4dc8aab5 | 10 38.00 -210.00 1399.25 0 0 0.000 0.000 | 56 10.00 2391.18 1861.15 19 0 -12.429 13.213 | 7 319.61 1876.41 -1.459 -0.909
262 4dc8aab5 | 10 39.00 -210.00 1399.25 0 0 0.000 0.000 | 56 12.00 2379.92 1858.73 19 0 -11.264 11.975 | 7 318.15 1874.30 -1.459 -2.109
263 4dc8aab5 | 10 40.00 -210.00 1399.25 0 0 0.000 0.000 | 56 14.00 2369.82 1852.66 19 0 -10.099 10.736 | 7 316.69 1870.99 -1.459 -3.309
264 4dc8aab5 | 10 41.00 -210.00 1399.25 0 0 0.000 0.000 | 56 16.00 2360.88 1842.96 19 0 -8.935 9.498 | 7 315.23 1866.48 -1.459 -4.509
265 4dc8aab5 | 10 42.00 -210.00 1399.25 0 0 0.000 0.000 | 56 18.00 2353.11 1829.62 19 0 -7.770 8.260 | 7 313.77 1860.77 -1.459 -5.709
266 4dc8aab5 | 10 43.00 -210.00 1399.25 0 0 0.000 0.000 | 56 20.00 2346.51 1812.64 19 0 -6.605 7.022 | 7 312.31 1853.87 -1.459 -6.909
267 4dc8aab5 | 10 44.00 -210.00 1399.25 0 0 0.000 0.000 | 56 22.00 2341.07 1792.03 19 0 -5.440 5.783 | 7 310.85 1845.76 -1.459 -8.109
268 4dc8aab5 | 10 45.00 -210.00 1399.25 0 0 0.000 0.000 | 56 24.00 2336.79 1767.77 19 0 -4.275 4.545 | 7 309.39 1836.45 -1.459 -9.309
269 4dc8aab5 | 10 46.00 -210.00 1399.25 0 0 0.000 0.000 | 56 26.00 2333.68 1739.88 19 0 -3.111 3.307 | 7 307.93 1825.94 -1.459 -10.509
270 4dc8aab5 | 10 47.00 -210.00 1399.25 0 0 0.000 0.000 | 57 0.00 2331.74 1708.35 19 0 -1.946 2.069 | 7 306.48 1820.45 0.000 0.000
271 4dc8aab5 | 10 48.00 -210.00 1399.25 0 0 0.000 0.000 | 57 1.00 2330.96 1673.18 19 0 -0.781 0.830 | 7 306.48 1820.45 0.000 0.000
272 4dc8aab5 | 10 49.00 -210.00 1399.25 0 0 0.000 0.000 | 57 2.00 2330.96 1634.78 19 0 0.000 0.000 | 7 306.48 1820.45 0.000 0.000
273 4dc8aab5 | 10 50.00 -210.00 1399.25 0 0 0.000 0.000 | 57 3.00 2330.96 1593.98 19 0 0.000 0.000 | 7 306.48 1820.45 0.000 0.000
274 4dc8aab5 | 10 51.00 -210.00 1399.25 0 0 0.000 0.000 | 57 4.00 2330.96 1550.78 19 0 0.000 0.000 | 7 306.48 1820.45 0.000 0.000
275 4dc8aab5 | 10 52.00 -210.00 1399.25 0 0 0.000 0.000 | 57 5.00 2330.96 1506.78 19 0 0.000 0.000 | 7 306.48 1820.45 0.000 0.000
276 4dc8aab5 | 10 53.00 -210.00 1399.25 0 0 0.000 0.000 | 57 6.00 2330.96 1462.78 19 0 0.000 0.000 | 7 306.48 1820.45 0.000 0.000
277 4dc8aab5 | 10 54.00 -210.00 1399.25 0 0 0.000 0.000 | 57 7.00 2330.96 1418.78 19 0 0.000 0.000 | 7 306.48 1820.45 0.000 0.000
278 4dc8aab5 | 10 55.00 -210.00 1399.25 0 0 0.000 0.000 | 57 8.00 2330.96 1374.78 19 0 0.000 0.000 | 7 306.48 1820.45 0.000 0.000
279 4dc8aab5 | 10 56.00 -210.00 1399.25 0 0 0.000 0.000 | 57 9.00 2330.96 1330.78 19 0 0.000 0.000 | 7 306.48 1820.45 0.000 0.000
280 4dc8aab5 | 10 57.00 -210.00 1399.25 0 0 0.000 0.000 | 57 10.00 2330.96 1286.78 19 0 0.000 0.000 | 7 306.48 1820.45 0.000 0.000
281 4dc8aab5 | 10 58.00 -210.00 1399.25 0 0 0.000 0.000 | 57 11.00 2330.96 1242.78 19 0 0.000 0.000 | 7 306.48 1820.45 0.000 0.000
282 4dc8aab5 | 10 59.00 -210.00 1399.25 0 0 0.000 0.000 | 57 12.00 2330.96 1198.78 19 0 0.000 0.000 | 7 306.48 1820.45 0.000 0.000
283 4dc8aab5 | 10 60.00 -210.00 1399.25 0 0 0.000 0.000 | 57 13.00 2330.96 1154.78 19 0 0.000 0.000 | 7 306.48 1820.45 0.000 0.000
284 4dc8aab5 | 10 61.00 -210.00 1399.25 0 0 0.000 0.000 | 57 14.00 2330.96 1110.78 19 0 0.000 0.000 | 7 306.48 1820.45 0.000 0.000
285 4dc8aab5 | 10 62.00 -210.00 1399.25 0 0 0.000 0.000 | 57 15.00 2330.96 1066.78 19 0 0.000 0.000 | 7 306.48 1820.45 0.000 0.000
286 4dc8aab5 | 10 63.00 -210.00 1399.25 0 0 0.000 0.000 | 57 16.00 2330.96 1022.78 19 0 0.000 0.000 | 7 306.48 1820.45 0.000 0.000
287 4dc8aab5 | 10 64.00 -210.00 1399.25 0 0 0.000 0.000 | 57 17.00 2330.96 978.78 19 0 0.000 0.000 | 7 306.48 1820.45 0.000 0.000
288 4dc8aab5 | 10 65.00 -210.00 1399.25 0 0 0.000 0.000 | 57 18.00 2330.96 934.78 19 0 0.000 0.000 | 7 306.48 1820.45 0.000 0.000
289 4dc8aab5 | 10 66.00 -210.00 1399.25 0 0 0.000 0.000 | 57 19.00 2330.96 890.78 19 0 0.000 0.000 | 7 306.48 1820.45 0.000 0.000
290 4dc8aab5 | 10 67.00 -210.00 1399.25 0 0 0.000 0.000 | 57 20.00 2330.96 846.78 19 0 0.000 0.000 | 7 306.48 1820.45 0.000 0.000
291 4dc8aab5 | 10 68.00 -210.00 1399.25 0 0 0.000 0.000 | 57 21.00 2330.96 802.78 19 0 0.000 0.000 | 7 306.48 1820.45 0.000 0.000
292 4dc8aab5 | 10 69.00 -210.00 1399.25 0 0 0.000 0.000 | 57 22.00 2330.96 758.78 19 0 0.000 0.000 | 7 306.48 1820.45 0.000 0.000
293 4dc8aab5 | 10 70.00 -210.00 1399.25 0 0 0.000 0.000 | 57 23.00 2330.96 714.78 19 0 0.000 0.000 | 7 306.48 1820.45 0.000 0.000
294 4dc8aab5 | 10 71.00 -210.00 1399.25 0 0 0.000 0.000 | 57 24.00 2330.96 670.78 19 0 0.000 0.000 | 7 306.48 1820.45 0.000 0.000
295 4dc8aab5 | 10 72.00 -210.00 1399.25 0 0 0.000 0.000 | 57 25.00 2330.96 626.78 19 0 0.000 0.000 | 7 306.48 1820.45 0.000 0.000
296 4dc8aab5 | 10 73.00 -210.00 1399.25 0 0 0.000 0.000 | 57 26.00 2330.96 582.78 19 0 0.000 0.000 | 7 306.48 1820.45 0.000 0.000
297 4dc8aab5 | 10 74.00 -210.00 1399.25 0 0 0.000 0.000 | 57 27.00 2330.96 538.78 19 0 0.000 0.000 | 7 306.48 1820.45 0.000 0.000
298 4dc8aab5 | 10 75.00 -210.00 1399.25 0 0 0.000 0.000 | 57 28.00 2330.96 494.78 19 0 0.000 0.000 | 7 306.48 1820.45 0.000 0.000
299 4dc8aab5 | 10 76.00 -210.00 1399.25 0 0 0.000 0.000 | 57 0.00 2330.96 450.78 19 0 0.000 0.000 | 7 306.48 1820.45 0.000 0.000
300 4dc8aab5 | 10 77.00 -210.00 1399.25 0 0 0.000 0.000 | 57 1.00 2330.96 406.78 19 0 0.000 0.000 | 7 306.48 1820.45 0.000 0.000
";

#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn a_thrown_beam_sword_launches_the_dummy_as_on_the_n64() {
    let replay = Replay {
        p1: FighterKind::Link,
        route: "30:1:start;40:1:y=-80;50:1:x=80;60:1:x=80;70:1:x=80;80:1:x=80;90:1:a;\
                100:1:start;150:1:a;200:1:z+a+x=127",
        weapons: &no_weapons,
    };
    let Some(out) = replay.run(LINK_SWORD_HITS_THE_DUMMY) else {
        return;
    };
    out.check("Beam Sword smash throw into the dummy");
}

/// `b30set/hfan` (`warp_link`): the menu's Fan hangs above Link, so A at
/// 170 jabs and Z+A at 220 grabs the dummy from 975 units away. The
/// pulled dummy (`CapturePulled` 171) takes the catcher's floor line
/// under its new X each frame: x 409.26 at 244 puts y at 1402.38 on the
/// 3-in-1927 slope (a stale TransN read the floor at x 431.91, 1402.37).
/// Then `CaptureWait` 172, the forward throw (169, `Thrown` 186), 14
/// percent and `DamageFlyHi` (52) from frame 325. Rows 220-400, every
/// 4th past 250.
const LINK_PULLS_THE_DUMMY_DOWN_THE_SLOPE: &str = "
110 69521988
111 a6e9c9a6
112 fe87df14
113 c40df552
179 4db793af
220 4db793af | 166 1.00 -210.00 1399.25 0 0 0.000 0.000 | 10 21.00 765.00 1413.09 0 0 0.000 0.000 | 9 -209.65 1609.25 0.000 0.000
221 4db793af | 166 2.00 -210.00 1399.25 0 0 0.000 0.000 | 10 22.00 765.00 1413.09 0 0 0.000 0.000 | 9 -209.65 1609.25 0.000 0.000
222 4db793af | 166 3.00 -210.00 1399.25 0 0 0.000 0.000 | 10 23.00 765.00 1413.09 0 0 0.000 0.000 | 9 -209.65 1609.25 0.000 0.000
223 4db793af | 166 4.00 -210.00 1399.25 0 0 0.000 0.000 | 10 24.00 765.00 1413.09 0 0 0.000 0.000 | 9 -209.65 1609.25 0.000 0.000
224 4db793af | 166 5.00 -210.00 1399.25 0 0 0.000 0.000 | 10 25.00 765.00 1413.09 0 0 0.000 0.000 | 9 -209.65 1609.25 0.000 0.000
225 4db793af | 166 6.00 -210.00 1399.25 0 0 0.000 0.000 | 10 26.00 765.00 1413.09 0 0 0.000 0.000 | 9 -209.65 1609.25 0.000 0.000
226 4db793af | 166 7.00 -210.00 1399.25 0 0 0.000 0.000 | 10 27.00 765.00 1413.09 0 0 0.000 0.000 | 9 -209.65 1609.25 0.000 0.000
227 4db793af | 166 8.00 -210.00 1399.25 0 0 0.000 0.000 | 10 28.00 765.00 1413.09 0 0 0.000 0.000 | 9 -209.65 1609.25 0.000 0.000
228 4db793af | 166 9.00 -210.00 1399.25 0 0 0.000 0.000 | 10 29.00 765.00 1413.09 0 0 0.000 0.000 | 9 -209.65 1609.25 0.000 0.000
229 4db793af | 166 10.00 -210.00 1399.25 0 0 0.000 0.000 | 10 30.00 765.00 1413.09 0 0 0.000 0.000 | 9 -209.65 1609.25 0.000 0.000
230 4db793af | 166 11.00 -210.00 1399.25 0 0 0.000 0.000 | 10 31.00 765.00 1413.09 0 0 0.000 0.000 | 9 -209.65 1609.25 0.000 0.000
231 4db793af | 166 12.00 -210.00 1399.25 0 0 0.000 0.000 | 10 32.00 765.00 1413.09 0 0 0.000 0.000 | 9 -209.65 1609.25 0.000 0.000
232 4db793af | 166 13.00 -210.00 1399.25 0 0 0.000 0.000 | 10 33.00 765.00 1413.09 0 0 0.000 0.000 | 9 -209.65 1609.25 0.000 0.000
233 4db793af | 166 14.00 -210.00 1399.25 0 0 0.000 0.000 | 10 34.00 765.00 1413.09 0 0 0.000 0.000 | 9 -209.65 1609.25 0.000 0.000
234 4db793af | 166 15.00 -210.00 1399.25 0 0 0.000 0.000 | 10 35.00 765.00 1413.09 0 0 0.000 0.000 | 9 -209.65 1609.25 0.000 0.000
235 4db793af | 166 16.00 -210.00 1399.25 0 0 0.000 0.000 | 10 36.00 765.00 1413.09 0 0 0.000 0.000 | 9 -209.65 1609.25 0.000 0.000
236 b9b3dbe0 | 166 17.00 -210.00 1399.25 0 0 0.000 0.000 | 10 37.00 765.00 1413.09 0 0 0.000 0.000 | 9 -209.65 1609.25 0.000 0.000
237 b9b3dbe0 | 166 18.00 -210.00 1399.25 0 0 0.000 0.000 | 10 38.00 765.00 1413.09 0 0 0.000 0.000 | 9 -209.65 1609.25 0.000 0.000
238 b9b3dbe0 | 166 19.00 -210.00 1399.25 0 0 0.000 0.000 | 10 39.00 765.00 1413.09 0 0 0.000 0.000 | 9 -209.65 1609.25 0.000 0.000
239 4dc8aab5 | 166 20.00 -210.00 1399.25 0 0 0.000 0.000 | 10 40.00 765.00 1413.09 0 0 0.000 0.000 | 9 -209.65 1609.25 0.000 0.000
240 4dc8aab5 | 166 21.00 -210.00 1399.25 0 0 0.000 0.000 | 10 41.00 765.00 1413.09 0 0 0.000 0.000 | 9 -209.65 1609.25 0.000 0.000
241 4dc8aab5 | 166 22.00 -210.00 1399.25 0 0 0.000 0.000 | 10 42.00 765.00 1413.09 0 0 0.000 0.000 | 9 -209.65 1609.25 0.000 0.000
242 428b763e | 166 23.00 -210.00 1399.25 0 0 0.000 0.000 | 10 43.00 765.00 1413.09 0 0 0.000 0.000 | 9 -209.65 1609.25 0.000 0.000
243 428b763e | 167 2.08 -210.00 1399.25 0 0 0.000 0.000 | 171 1.00 615.19 1406.48 0 0 0.000 0.000 | 9 -209.65 1609.25 0.000 0.000
244 428b763e | 167 3.08 -210.00 1399.25 0 0 0.000 0.000 | 171 2.00 409.26 1402.38 0 0 0.000 0.000 | 9 -209.65 1609.25 0.000 0.000
245 428b763e | 167 4.08 -210.00 1399.25 0 0 0.000 0.000 | 171 0.00 226.21 1400.48 0 0 0.000 0.000 | 9 -209.65 1609.25 0.000 0.000
246 428b763e | 167 5.08 -210.00 1399.25 0 0 0.000 0.000 | 171 0.00 66.69 1399.65 0 0 0.000 0.000 | 9 -209.65 1609.25 0.000 0.000
247 428b763e | 168 -0.08 -210.00 1399.25 0 0 0.000 0.000 | 172 0.00 -15.88 1398.95 0 0 0.000 0.000 | 9 -209.65 1609.25 0.000 0.000
248 428b763e | 168 -0.08 -210.00 1399.25 0 0 0.000 0.000 | 172 0.00 -15.88 1398.95 0 0 0.000 0.000 | 9 -209.65 1609.25 0.000 0.000
249 428b763e | 168 -0.08 -210.00 1399.25 0 0 0.000 0.000 | 172 0.00 -15.88 1398.95 0 0 0.000 0.000 | 9 -209.65 1609.25 0.000 0.000
252 428b763e | 168 -0.08 -210.00 1399.25 0 0 0.000 0.000 | 172 0.00 -15.88 1398.95 0 0 0.000 0.000 | 9 -209.65 1609.25 0.000 0.000
256 428b763e | 168 -0.08 -210.00 1399.25 0 0 0.000 0.000 | 172 0.00 -15.88 1398.95 0 0 0.000 0.000 | 9 -209.65 1609.25 0.000 0.000
260 428b763e | 168 -0.08 -210.00 1399.25 0 0 0.000 0.000 | 172 0.00 -15.88 1398.95 0 0 0.000 0.000 | 9 -209.65 1609.25 0.000 0.000
264 428b763e | 168 -0.08 -210.00 1399.25 0 0 0.000 0.000 | 172 0.00 -15.88 1398.95 0 0 0.000 0.000 | 9 -209.65 1609.25 0.000 0.000
268 428b763e | 168 -0.08 -210.00 1399.25 0 0 0.000 0.000 | 172 0.00 -15.88 1398.95 0 0 0.000 0.000 | 9 -209.65 1609.25 0.000 0.000
272 428b763e | 168 -0.08 -210.00 1399.25 0 0 0.000 0.000 | 172 0.00 -15.88 1398.95 0 0 0.000 0.000 | 9 -209.65 1609.25 0.000 0.000
276 428b763e | 168 -0.08 -210.00 1399.25 0 0 0.000 0.000 | 172 0.00 -15.88 1398.95 0 0 0.000 0.000 | 9 -209.65 1609.25 0.000 0.000
280 428b763e | 168 -0.08 -210.00 1399.25 0 0 0.000 0.000 | 172 0.00 -15.88 1398.95 0 0 0.000 0.000 | 9 -209.65 1609.25 0.000 0.000
284 428b763e | 168 -0.08 -210.00 1399.25 0 0 0.000 0.000 | 172 0.00 -15.88 1398.95 0 0 0.000 0.000 | 9 -209.65 1609.25 0.000 0.000
288 428b763e | 168 -0.08 -210.00 1399.25 0 0 0.000 0.000 | 172 0.00 -15.88 1398.95 0 0 0.000 0.000 | 9 -209.65 1609.25 0.000 0.000
292 428b763e | 168 -0.08 -210.00 1399.25 0 0 0.000 0.000 | 172 0.00 -15.88 1398.95 0 0 0.000 0.000 | 9 -209.65 1609.25 0.000 0.000
296 428b763e | 168 -0.08 -210.00 1399.25 0 0 0.000 0.000 | 172 0.00 -15.88 1398.95 0 0 0.000 0.000 | 9 -209.65 1609.25 0.000 0.000
300 428b763e | 168 -0.08 -210.00 1399.25 0 0 0.000 0.000 | 172 0.00 -15.88 1398.95 0 0 0.000 0.000 | 9 -209.65 1609.25 0.000 0.000
304 428b763e | 168 -0.08 -210.00 1399.25 0 0 0.000 0.000 | 172 0.00 -15.88 1398.95 0 0 0.000 0.000 | 9 -209.65 1609.25 0.000 0.000
308 428b763e | 169 3.00 -210.00 1399.25 0 0 0.000 0.000 | 186 4.00 -145.20 1766.94 0 0 0.000 0.000 | 9 -209.65 1609.25 0.000 0.000
312 428b763e | 169 7.00 -210.00 1399.25 0 0 0.000 0.000 | 186 8.00 -228.16 1704.08 0 0 0.000 0.000 | 9 -209.65 1609.25 0.000 0.000
316 428b763e | 169 11.00 -210.00 1399.25 0 0 0.000 0.000 | 186 12.00 117.67 1789.85 0 0 0.000 0.000 | 9 -209.65 1609.25 0.000 0.000
317 a6a6b409
320 a6a6b409 | 169 15.00 -210.00 1399.25 0 0 0.000 0.000 | 186 16.00 117.67 1789.85 0 0 0.000 0.000 | 9 -209.65 1609.25 0.000 0.000
324 a6a6b409 | 169 19.00 -210.00 1399.25 0 0 0.000 0.000 | 186 20.00 165.09 1852.92 0 0 0.000 0.000 | 9 -209.65 1609.25 0.000 0.000
328 a6a6b409 | 169 23.00 -210.00 1399.25 0 0 0.000 0.000 | 52 5.00 498.76 1816.10 14 0 68.431 68.431 | 9 -209.65 1609.25 0.000 0.000
332 a6a6b409 | 169 27.00 -210.00 1399.25 0 0 0.000 0.000 | 52 9.00 760.46 2015.41 14 0 63.623 63.623 | 9 -209.65 1609.25 0.000 0.000
336 a6a6b409 | 169 31.00 -210.00 1399.25 0 0 0.000 0.000 | 52 13.00 1002.93 2157.08 14 0 58.814 58.814 | 9 -209.65 1609.25 0.000 0.000
340 a6a6b409 | 169 35.00 -210.00 1399.25 0 0 0.000 0.000 | 52 17.00 1226.17 2241.11 14 0 54.006 54.006 | 9 -209.65 1609.25 0.000 0.000
344 a6a6b409 | 169 39.00 -210.00 1399.25 0 0 0.000 0.000 | 52 21.00 1430.17 2273.12 14 0 49.198 49.198 | 9 -209.65 1609.25 0.000 0.000
348 a6a6b409 | 10 3.00 -210.00 1399.25 0 0 0.000 0.000 | 52 25.00 1614.94 2281.89 14 0 44.389 44.389 | 9 -209.65 1609.25 0.000 0.000
352 a6a6b409 | 10 7.00 -210.00 1399.25 0 0 0.000 0.000 | 52 0.00 1780.48 2271.42 14 0 39.581 39.581 | 9 -209.65 1609.25 0.000 0.000
356 a6a6b409 | 10 11.00 -210.00 1399.25 0 0 0.000 0.000 | 52 0.00 1926.78 2241.73 14 0 34.773 34.773 | 9 -209.65 1609.25 0.000 0.000
360 a6a6b409 | 10 15.00 -210.00 1399.25 0 0 0.000 0.000 | 52 0.00 2053.85 2192.80 14 0 29.964 29.964 | 9 -209.65 1609.25 0.000 0.000
364 a6a6b409 | 10 19.00 -210.00 1399.25 0 0 0.000 0.000 | 52 0.00 2161.69 2124.63 14 0 25.156 25.156 | 9 -209.65 1609.25 0.000 0.000
368 a6a6b409 | 10 23.00 -210.00 1399.25 0 0 0.000 0.000 | 52 0.00 2250.29 2037.24 14 0 20.348 20.348 | 9 -209.65 1609.25 0.000 0.000
372 a6a6b409 | 10 27.00 -210.00 1399.25 0 0 0.000 0.000 | 52 0.00 2319.66 1930.61 14 0 15.539 15.539 | 9 -209.65 1609.25 0.000 0.000
376 a6a6b409 | 10 31.00 -210.00 1399.25 0 0 0.000 0.000 | 52 0.00 2369.80 1804.74 14 0 10.731 10.731 | 9 -209.65 1609.25 0.000 0.000
380 a6a6b409 | 10 35.00 -210.00 1399.25 0 0 0.000 0.000 | 57 1.00 2400.70 1659.65 14 0 5.923 5.923 | 9 -209.65 1609.25 0.000 0.000
384 a6a6b409 | 10 39.00 -210.00 1399.25 0 0 0.000 0.000 | 57 5.00 2412.37 1495.32 14 0 1.114 1.114 | 9 -209.65 1609.25 0.000 0.000
388 a6a6b409 | 10 43.00 -210.00 1399.25 0 0 0.000 0.000 | 57 9.00 2412.37 1319.32 14 0 0.000 0.000 | 9 -209.65 1609.25 0.000 0.000
392 a6a6b409 | 10 47.00 -210.00 1399.25 0 0 0.000 0.000 | 57 13.00 2412.37 1143.32 14 0 0.000 0.000 | 9 -209.65 1609.25 0.000 0.000
396 a6a6b409 | 10 51.00 -210.00 1399.25 0 0 0.000 0.000 | 57 17.00 2412.37 967.32 14 0 0.000 0.000 | 9 -209.65 1609.25 0.000 0.000
400 a6a6b409 | 10 55.00 -210.00 1399.25 0 0 0.000 0.000 | 57 21.00 2412.37 791.32 14 0 0.000 0.000 | 9 -209.65 1609.25 0.000 0.000
";

/// `b30set/hgun` (`warp_link`): the menu's Ray Gun (A at 130), picked up
/// at 190 and smash-thrown at 240. It hits the dummy at 251 (12 percent,
/// 9 frames of hitlag) and rebounds; its first landing at 290 draws
/// `itMapCheckDestroyDropped`'s despawn roll, which keeps it. Rows
/// 240-400, every 5th past 255.
const LINK_RAY_GUN_HITS_THE_DUMMY: &str = "
130 df90722b
131 81eb59d1
132 b90c5187
133 491c86cd
240 491c86cd | 110 1.00 -210.00 1399.25 0 0 0.000 0.000 | 10 41.00 765.00 1413.09 0 0 0.000 0.000 | 11 0.00 0.00 0.000 0.000
241 491c86cd | 110 2.00 -210.00 1399.25 0 0 0.000 0.000 | 10 42.00 765.00 1413.09 0 0 0.000 0.000 | 11 0.00 0.00 0.000 0.000
242 491c86cd | 110 3.00 -210.00 1399.25 0 0 0.000 0.000 | 10 43.00 765.00 1413.09 0 0 0.000 0.000 | 11 0.00 0.00 0.000 0.000
243 491c86cd | 110 4.00 -210.00 1399.25 0 0 0.000 0.000 | 10 44.00 765.00 1413.09 0 0 0.000 0.000 | 11 0.00 0.00 0.000 0.000
244 491c86cd | 110 5.00 -210.00 1399.25 0 0 0.000 0.000 | 10 45.00 765.00 1413.09 0 0 0.000 0.000 | 11 0.00 0.00 0.000 0.000
245 491c86cd | 110 6.00 -210.00 1399.25 0 0 0.000 0.000 | 10 46.00 765.00 1413.09 0 0 0.000 0.000 | 11 0.00 0.00 0.000 0.000
246 491c86cd | 110 7.00 -210.00 1399.25 0 0 0.000 0.000 | 10 47.00 765.00 1413.09 0 0 0.000 0.000 | 11 0.00 0.00 0.000 0.000
247 4d067e5c | 110 8.00 -210.00 1399.25 0 0 0.000 0.000 | 10 48.00 765.00 1413.09 0 0 0.000 0.000 | 11 180.79 1721.55 108.929 13.809
248 4d067e5c | 110 9.00 -210.00 1399.25 0 0 0.000 0.000 | 10 49.00 765.00 1413.09 0 0 0.000 0.000 | 11 289.72 1733.86 108.929 12.309
249 4d067e5c | 110 10.00 -210.00 1399.25 0 0 0.000 0.000 | 10 0.00 765.00 1413.09 0 0 0.000 0.000 | 11 398.65 1744.67 108.929 10.809
250 4d067e5c | 110 11.00 -210.00 1399.25 0 0 0.000 0.000 | 10 1.00 765.00 1413.09 0 0 0.000 0.000 | 11 507.58 1753.98 108.929 9.309
251 95475fb6 | 110 12.00 -210.00 1399.25 0 0 0.000 0.000 | 38 1.00 765.00 1413.09 12 9 31.508 28.872 | 11 616.51 1761.78 -6.536 22.657
252 4dc8aab5 | 110 13.00 -210.00 1399.25 0 0 0.000 0.000 | 38 1.00 765.00 1413.09 12 8 31.508 28.872 | 11 609.97 1782.94 -6.536 21.157
253 4dc8aab5 | 110 14.00 -210.00 1399.25 0 0 0.000 0.000 | 38 1.00 765.00 1413.09 12 7 31.508 28.872 | 11 603.43 1802.60 -6.536 19.657
254 4dc8aab5 | 110 15.00 -210.00 1399.25 0 0 0.000 0.000 | 38 1.00 765.00 1413.09 12 6 31.508 28.872 | 11 596.90 1820.76 -6.536 18.157
255 4dc8aab5 | 110 16.00 -210.00 1399.25 0 0 0.000 0.000 | 38 1.00 765.00 1413.09 12 5 31.508 28.872 | 11 590.36 1837.41 -6.536 16.657
260 4dc8aab5 | 110 21.00 -210.00 1399.25 0 0 0.000 0.000 | 38 2.00 795.25 1438.41 12 0 30.255 27.724 | 11 557.68 1898.20 -6.536 9.157
265 4dc8aab5 | 10 2.00 -210.00 1399.25 0 0 0.000 0.000 | 38 7.00 927.73 1511.80 12 0 23.988 21.981 | 11 525.01 1921.49 -6.536 1.657
270 4dc8aab5 | 10 7.00 -210.00 1399.25 0 0 0.000 0.000 | 38 12.00 1028.87 1496.48 12 0 17.721 16.238 | 11 492.33 1907.27 -6.536 -5.843
275 4dc8aab5 | 10 12.00 -210.00 1399.25 0 0 0.000 0.000 | 38 17.00 1098.38 1442.50 12 0 11.164 0.985 | 11 459.65 1855.56 -6.536 -13.343
280 4dc8aab5 | 10 17.00 -210.00 1399.25 0 0 0.000 0.000 | 38 22.00 1131.79 1445.45 12 0 3.693 0.326 | 11 426.97 1766.34 -6.536 -20.843
285 4dc8aab5 | 10 22.00 -210.00 1399.25 0 0 0.000 0.000 | 10 3.00 1134.70 1445.71 12 0 0.000 0.000 | 11 394.29 1639.63 -6.536 -28.343
290 6443b2a4 | 10 27.00 -210.00 1399.25 0 0 0.000 0.000 | 10 8.00 1134.70 1445.71 12 0 0.000 0.000 | 11 361.61 1498.36 -0.318 3.615
295 6443b2a4 | 10 32.00 -210.00 1399.25 0 0 0.000 0.000 | 10 13.00 1134.70 1445.71 12 0 0.000 0.000 | 11 360.34 1498.36 0.000 0.000
300 6443b2a4 | 10 37.00 -210.00 1399.25 0 0 0.000 0.000 | 10 18.00 1134.70 1445.71 12 0 0.000 0.000 | 11 360.34 1498.36 0.000 0.000
305 6443b2a4 | 10 42.00 -210.00 1399.25 0 0 0.000 0.000 | 10 23.00 1134.70 1445.71 12 0 0.000 0.000 | 11 360.34 1498.36 0.000 0.000
310 6443b2a4 | 10 47.00 -210.00 1399.25 0 0 0.000 0.000 | 10 28.00 1134.70 1445.71 12 0 0.000 0.000 | 11 360.34 1498.36 0.000 0.000
315 6443b2a4 | 10 52.00 -210.00 1399.25 0 0 0.000 0.000 | 10 33.00 1134.70 1445.71 12 0 0.000 0.000 | 11 360.34 1498.36 0.000 0.000
320 6443b2a4 | 10 57.00 -210.00 1399.25 0 0 0.000 0.000 | 10 38.00 1134.70 1445.71 12 0 0.000 0.000 | 11 360.34 1498.36 0.000 0.000
325 6443b2a4 | 10 62.00 -210.00 1399.25 0 0 0.000 0.000 | 10 43.00 1134.70 1445.71 12 0 0.000 0.000 | 11 360.34 1498.36 0.000 0.000
330 6443b2a4 | 10 67.00 -210.00 1399.25 0 0 0.000 0.000 | 10 48.00 1134.70 1445.71 12 0 0.000 0.000 | 11 360.34 1498.36 0.000 0.000
335 6443b2a4 | 10 72.00 -210.00 1399.25 0 0 0.000 0.000 | 10 3.00 1134.70 1445.71 12 0 0.000 0.000 | 11 360.34 1498.36 0.000 0.000
340 6443b2a4 | 10 77.00 -210.00 1399.25 0 0 0.000 0.000 | 10 8.00 1134.70 1445.71 12 0 0.000 0.000 | 11 360.34 1498.36 0.000 0.000
345 6443b2a4 | 10 2.00 -210.00 1399.25 0 0 0.000 0.000 | 10 13.00 1134.70 1445.71 12 0 0.000 0.000 | 11 360.34 1498.36 0.000 0.000
350 6443b2a4 | 10 7.00 -210.00 1399.25 0 0 0.000 0.000 | 10 18.00 1134.70 1445.71 12 0 0.000 0.000 | 11 360.34 1498.36 0.000 0.000
355 6443b2a4 | 10 12.00 -210.00 1399.25 0 0 0.000 0.000 | 10 23.00 1134.70 1445.71 12 0 0.000 0.000 | 11 360.34 1498.36 0.000 0.000
360 6443b2a4 | 10 17.00 -210.00 1399.25 0 0 0.000 0.000 | 10 28.00 1134.70 1445.71 12 0 0.000 0.000 | 11 360.34 1498.36 0.000 0.000
365 6443b2a4 | 10 22.00 -210.00 1399.25 0 0 0.000 0.000 | 10 33.00 1134.70 1445.71 12 0 0.000 0.000 | 11 360.34 1498.36 0.000 0.000
370 6443b2a4 | 10 27.00 -210.00 1399.25 0 0 0.000 0.000 | 10 38.00 1134.70 1445.71 12 0 0.000 0.000 | 11 360.34 1498.36 0.000 0.000
375 6443b2a4 | 10 32.00 -210.00 1399.25 0 0 0.000 0.000 | 10 43.00 1134.70 1445.71 12 0 0.000 0.000 | 11 360.34 1498.36 0.000 0.000
380 6443b2a4 | 10 37.00 -210.00 1399.25 0 0 0.000 0.000 | 10 48.00 1134.70 1445.71 12 0 0.000 0.000 | 11 360.34 1498.36 0.000 0.000
385 6443b2a4 | 10 42.00 -210.00 1399.25 0 0 0.000 0.000 | 10 3.00 1134.70 1445.71 12 0 0.000 0.000 | 11 360.34 1498.36 0.000 0.000
390 6443b2a4 | 10 47.00 -210.00 1399.25 0 0 0.000 0.000 | 10 8.00 1134.70 1445.71 12 0 0.000 0.000 | 11 360.34 1498.36 0.000 0.000
395 6443b2a4 | 10 52.00 -210.00 1399.25 0 0 0.000 0.000 | 10 13.00 1134.70 1445.71 12 0 0.000 0.000 | 11 360.34 1498.36 0.000 0.000
400 6443b2a4 | 10 57.00 -210.00 1399.25 0 0 0.000 0.000 | 10 18.00 1134.70 1445.71 12 0 0.000 0.000 | 11 360.34 1498.36 0.000 0.000
";

/// `b30set/hbomb` (`warp_link`): the menu's Bob-omb, jabbed at 230: it
/// explodes on Link (30 percent, `DamageFlyHi` 52 at 237), who falls
/// (`DamageFall` 57) out of the bottom blast zone at 383 (`DeadDown` 0).
/// Rows 225-400, every 5th past 240.
const LINK_JABS_A_BOB_OMB: &str = "
170 69521988
171 a6e9c9a6
172 fe87df14
173 c40df552
225 c40df552 | 10 66.00 -210.00 1399.25 0 0 0.000 0.000 | 10 26.00 765.00 1413.09 0 0 0.000 0.000 | 15 -209.89 1578.05 0.022 3.360
226 c40df552 | 10 67.00 -210.00 1399.25 0 0 0.000 0.000 | 10 27.00 765.00 1413.09 0 0 0.000 0.000 | 15 -209.87 1580.21 0.022 2.160
227 c40df552 | 10 68.00 -210.00 1399.25 0 0 0.000 0.000 | 10 28.00 765.00 1413.09 0 0 0.000 0.000 | 15 -209.85 1581.17 0.022 0.960
228 c40df552 | 10 69.00 -210.00 1399.25 0 0 0.000 0.000 | 10 29.00 765.00 1413.09 0 0 0.000 0.000 | 15 -209.83 1580.93 0.022 -0.240
229 c40df552 | 10 70.00 -210.00 1399.25 0 0 0.000 0.000 | 10 30.00 765.00 1413.09 0 0 0.000 0.000 | 15 -209.80 1579.49 0.022 -1.440
230 c40df552 | 190 1.00 -210.00 1399.25 0 0 0.000 0.000 | 10 31.00 765.00 1413.09 0 0 0.000 0.000 | 15 -209.78 1576.85 0.022 -2.640
231 c40df552 | 190 2.00 -210.00 1399.25 0 0 0.000 0.000 | 10 32.00 765.00 1413.09 0 0 0.000 0.000 | 15 -209.76 1573.01 0.022 -3.840
232 c40df552 | 190 3.00 -210.00 1399.25 0 0 0.000 0.000 | 10 33.00 765.00 1413.09 0 0 0.000 0.000 | 15 -209.74 1567.97 0.022 -5.040
233 c40df552 | 190 4.00 -210.00 1399.25 0 0 0.000 0.000 | 10 34.00 765.00 1413.09 0 0 0.000 0.000 | 15 -209.72 1561.73 0.022 -6.240
234 c40df552 | 190 5.00 -210.00 1399.25 0 0 0.000 0.000 | 10 35.00 765.00 1413.09 0 0 0.000 0.000 | 15 -209.69 1554.29 0.022 -7.440
235 c40df552 | 190 6.00 -210.00 1399.25 0 0 0.000 0.000 | 10 36.00 765.00 1413.09 0 0 0.000 0.000 | 15 -209.67 1549.25 0.000 0.000
236 c40df552 | 190 7.00 -210.00 1399.25 0 6 0.000 0.000 | 10 37.00 765.00 1413.09 0 0 0.000 0.000 | 15 -209.67 1549.25 0.000 0.000
237 428b763e | 52 1.00 -210.00 1399.25 30 15 -82.953 76.012 | 10 38.00 765.00 1413.09 0 0 0.000 0.000 | 15 -209.67 1549.25 0.000 0.000
238 428b763e | 52 1.00 -210.00 1399.25 30 14 -82.953 76.012 | 10 39.00 765.00 1413.09 0 0 0.000 0.000 | 15 -209.67 1549.25 0.000 0.000
239 fa5aea62 | 52 1.00 -210.00 1399.25 30 13 -82.953 76.012 | 10 40.00 765.00 1413.09 0 0 0.000 0.000 | 15 -209.67 1549.25 0.000 0.000
240 fa5aea62 | 52 1.00 -210.00 1399.25 30 12 -82.953 76.012 | 10 41.00 765.00 1413.09 0 0 0.000 0.000 | 15 -209.67 1549.25 0.000 0.000
241 bb25e9c6
243 7ff56027
245 b01c0d7c | 52 1.00 -210.00 1399.25 30 7 -82.953 76.012 | 10 46.00 765.00 1413.09 0 0 0.000 0.000 | 15 -209.67 1549.25 0.000 0.000
247 01202500
249 dcfd7fa9
250 dcfd7fa9 | 52 1.00 -210.00 1399.25 30 2 -82.953 76.012 | 10 1.00 765.00 1413.09 0 0 0.000 0.000
251 4cada790
253 c9442554
255 b66b3058 | 52 5.00 -529.28 1657.81 30 0 -77.939 71.418 | 10 6.00 765.00 1413.09 0 0 0.000 0.000
257 6032ec9c
259 bef6be20
260 bef6be20 | 52 10.00 -900.17 1878.68 30 0 -71.672 65.675 | 10 11.00 765.00 1413.09 0 0 0.000 0.000
261 92e148e4
263 58b070e8
265 fbb95a2c | 52 15.00 -1239.73 1985.83 30 0 -65.405 59.933 | 10 16.00 765.00 1413.09 0 0 0.000 0.000
267 692c68b0
269 68994074
270 68994074 | 52 20.00 -1547.96 1979.86 30 0 -59.138 54.190 | 10 21.00 765.00 1413.09 0 0 0.000 0.000
271 5db2c578
273 d7531bbc
275 0fbfa740 | 52 25.00 -1824.85 1913.59 30 0 -52.872 48.448 | 10 26.00 765.00 1413.09 0 0 0.000 0.000
277 c22d0c04
279 f9832e08
280 f9832e08 | 52 0.00 -2070.41 1818.60 30 0 -46.605 42.705 | 10 31.00 765.00 1413.09 0 0 0.000 0.000
281 bc61314c
283 ba6179d0
285 7dd156ca | 52 0.00 -2284.63 1694.90 30 0 -40.338 36.963 | 10 36.00 765.00 1413.09 0 0 0.000 0.000
289 5e9dab94
290 5e9dab94 | 52 0.00 -2467.52 1542.49 30 0 -34.071 31.220 | 10 41.00 765.00 1413.09 0 0 0.000 0.000
293 1916b6ae
295 1916b6ae | 52 0.00 -2619.07 1361.36 30 0 -27.804 25.478 | 10 46.00 765.00 1413.09 0 0 0.000 0.000
297 eb72aa98
300 eb72aa98 | 52 0.00 -2739.29 1151.52 30 0 -21.537 19.735 | 10 1.00 765.00 1413.09 0 0 0.000 0.000
301 401d4dd2
305 83849adc | 52 0.00 -2828.18 912.97 30 0 -15.270 13.993 | 10 6.00 765.00 1413.09 0 0 0.000 0.000
309 494a6036
310 494a6036 | 52 0.00 -2885.73 645.71 30 0 -9.004 8.250 | 10 11.00 765.00 1413.09 0 0 0.000 0.000
312 4402e060
315 4402e060 | 57 4.00 -2911.95 349.73 30 0 -2.737 2.508 | 10 16.00 765.00 1413.09 0 0 0.000 0.000
316 ebe871da
320 d42c1f24 | 57 9.00 -2913.66 31.30 30 0 0.000 0.000 | 10 21.00 765.00 1413.09 0 0 0.000 0.000
324 ebcb46be
325 ebcb46be | 57 14.00 -2913.66 -288.70 30 0 0.000 0.000 | 10 26.00 765.00 1413.09 0 0 0.000 0.000
328 0e123b28
330 0e123b28 | 57 19.00 -2913.66 -608.70 30 0 0.000 0.000 | 10 31.00 765.00 1413.09 0 0 0.000 0.000
332 a833e2e2
335 a833e2e2 | 57 24.00 -2913.66 -928.70 30 0 0.000 0.000 | 10 36.00 765.00 1413.09 0 0 0.000 0.000
336 a49e586c
340 91f48a46 | 57 0.00 -2913.66 -1248.70 30 0 0.000 0.000 | 10 41.00 765.00 1413.09 0 0 0.000 0.000
344 ead4daf0
345 ead4daf0 | 57 5.00 -2913.66 -1568.70 30 0 0.000 0.000 | 10 46.00 765.00 1413.09 0 0 0.000 0.000
348 ccd4c0ea
350 ccd4c0ea | 57 10.00 -2913.66 -1888.70 30 0 0.000 0.000 | 10 1.00 765.00 1413.09 0 0 0.000 0.000
352 205966b4
355 205966b4 | 57 15.00 -2913.66 -2208.70 30 0 0.000 0.000 | 10 6.00 765.00 1413.09 0 0 0.000 0.000
356 80354ace
360 e732dfb8 | 57 20.00 -2913.66 -2528.70 30 0 0.000 0.000 | 10 11.00 765.00 1413.09 0 0 0.000 0.000
365 e732dfb8 | 57 25.00 -2913.66 -2848.70 30 0 0.000 0.000 | 10 16.00 765.00 1413.09 0 0 0.000 0.000
370 e732dfb8 | 57 1.00 -2913.66 -3168.70 30 0 0.000 0.000 | 10 21.00 765.00 1413.09 0 0 0.000 0.000
375 e732dfb8 | 57 6.00 -2913.66 -3488.70 30 0 0.000 0.000 | 10 26.00 765.00 1413.09 0 0 0.000 0.000
380 e732dfb8 | 57 11.00 -2913.66 -3808.70 30 0 0.000 0.000 | 10 31.00 765.00 1413.09 0 0 0.000 0.000
383 c6cf69fc
384 c58f43fa
385 89753ad5 | 0 16.00 -2913.66 -4000.70 30 0 0.000 0.000 | 10 36.00 765.00 1413.09 0 0 0.000 0.000
386 c42babbd
387 fb448791
388 b9ce0efe
389 6a15e95d
390 4ff896ac | 0 21.00 -2913.66 -4000.70 30 0 0.000 0.000 | 10 41.00 765.00 1413.09 0 0 0.000 0.000
391 cad439f8
392 acf7d0db
393 d5794288
394 28522542
395 13d3df4a | 0 26.00 -2913.66 -4000.70 30 0 0.000 0.000 | 10 46.00 765.00 1413.09 0 0 0.000 0.000
396 8a287814
397 d20be72e
398 138a60b6
399 10498fa8
400 e479db62 | 0 2.00 -2913.66 -4000.70 30 0 0.000 0.000 | 10 1.00 765.00 1413.09 0 0 0.000 0.000
";

/// `b30set/hmsb` (`warp_link`): the menu's Motion-Sensor Bomb (A at 160),
/// picked up at 220 and smash-thrown at 270. It hits the dummy at 283
/// (12 percent) and lands. Rows 265-400, every 5th past 290.
const LINK_MOTION_SENSOR_BOMB_HITS_THE_DUMMY: &str = "
160 69521988
161 a6e9c9a6
162 fe87df14
163 c40df552
265 c40df552 | 10 38.00 -210.00 1399.25 0 0 0.000 0.000 | 10 16.00 765.00 1413.09 0 0 0.000 0.000 | 14 0.00 0.00 0.000 0.000
266 c40df552 | 10 39.00 -210.00 1399.25 0 0 0.000 0.000 | 10 17.00 765.00 1413.09 0 0 0.000 0.000 | 14 0.00 0.00 0.000 0.000
267 c40df552 | 10 40.00 -210.00 1399.25 0 0 0.000 0.000 | 10 18.00 765.00 1413.09 0 0 0.000 0.000 | 14 0.00 0.00 0.000 0.000
268 c40df552 | 10 41.00 -210.00 1399.25 0 0 0.000 0.000 | 10 19.00 765.00 1413.09 0 0 0.000 0.000 | 14 0.00 0.00 0.000 0.000
269 c40df552 | 10 42.00 -210.00 1399.25 0 0 0.000 0.000 | 10 20.00 765.00 1413.09 0 0 0.000 0.000 | 14 0.00 0.00 0.000 0.000
270 c40df552 | 110 1.00 -210.00 1399.25 0 0 0.000 0.000 | 10 21.00 765.00 1413.09 0 0 0.000 0.000 | 14 0.00 0.00 0.000 0.000
271 c40df552 | 110 2.00 -210.00 1399.25 0 0 0.000 0.000 | 10 22.00 765.00 1413.09 0 0 0.000 0.000 | 14 0.00 0.00 0.000 0.000
272 c40df552 | 110 3.00 -210.00 1399.25 0 0 0.000 0.000 | 10 23.00 765.00 1413.09 0 0 0.000 0.000 | 14 0.00 0.00 0.000 0.000
273 c40df552 | 110 4.00 -210.00 1399.25 0 0 0.000 0.000 | 10 24.00 765.00 1413.09 0 0 0.000 0.000 | 14 0.00 0.00 0.000 0.000
274 c40df552 | 110 5.00 -210.00 1399.25 0 0 0.000 0.000 | 10 25.00 765.00 1413.09 0 0 0.000 0.000 | 14 0.00 0.00 0.000 0.000
275 c40df552 | 110 6.00 -210.00 1399.25 0 0 0.000 0.000 | 10 26.00 765.00 1413.09 0 0 0.000 0.000 | 14 0.00 0.00 0.000 0.000
276 c40df552 | 110 7.00 -210.00 1399.25 0 0 0.000 0.000 | 10 27.00 765.00 1413.09 0 0 0.000 0.000 | 14 0.00 0.00 0.000 0.000
277 491c86cd | 110 8.00 -210.00 1399.25 0 0 0.000 0.000 | 10 28.00 765.00 1413.09 0 0 0.000 0.000 | 14 151.24 1717.68 79.380 9.942
278 491c86cd | 110 9.00 -210.00 1399.25 0 0 0.000 0.000 | 10 29.00 765.00 1413.09 0 0 0.000 0.000 | 14 230.62 1726.12 79.380 8.442
279 491c86cd | 110 10.00 -210.00 1399.25 0 0 0.000 0.000 | 10 30.00 765.00 1413.09 0 0 0.000 0.000 | 14 310.00 1733.06 79.380 6.942
280 491c86cd | 110 11.00 -210.00 1399.25 0 0 0.000 0.000 | 10 31.00 765.00 1413.09 0 0 0.000 0.000 | 14 389.38 1738.51 79.380 5.442
281 491c86cd | 110 12.00 -210.00 1399.25 0 0 0.000 0.000 | 10 32.00 765.00 1413.09 0 0 0.000 0.000 | 14 468.76 1742.45 79.380 3.942
282 491c86cd | 110 13.00 -210.00 1399.25 0 0 0.000 0.000 | 10 33.00 765.00 1413.09 0 0 0.000 0.000 | 14 548.14 1744.89 79.380 2.442
283 4db793af | 110 14.00 -210.00 1399.25 0 0 0.000 0.000 | 38 1.00 765.00 1413.09 12 9 29.314 26.861 | 14 627.52 1745.83 -4.763 24.718
284 074d495a | 110 15.00 -210.00 1399.25 0 0 0.000 0.000 | 38 1.00 765.00 1413.09 12 8 29.314 26.861 | 14 622.76 1769.05 -4.763 23.218
285 074d495a | 110 16.00 -210.00 1399.25 0 0 0.000 0.000 | 38 1.00 765.00 1413.09 12 7 29.314 26.861 | 14 617.99 1790.76 -4.763 21.718
286 074d495a | 110 17.00 -210.00 1399.25 0 0 0.000 0.000 | 38 1.00 765.00 1413.09 12 6 29.314 26.861 | 14 613.23 1810.98 -4.763 20.218
287 074d495a | 110 18.00 -210.00 1399.25 0 0 0.000 0.000 | 38 1.00 765.00 1413.09 12 5 29.314 26.861 | 14 608.47 1829.70 -4.763 18.718
288 074d495a | 110 19.00 -210.00 1399.25 0 0 0.000 0.000 | 38 1.00 765.00 1413.09 12 4 29.314 26.861 | 14 603.70 1846.92 -4.763 17.218
289 074d495a | 110 20.00 -210.00 1399.25 0 0 0.000 0.000 | 38 1.00 765.00 1413.09 12 3 29.314 26.861 | 14 598.94 1862.64 -4.763 15.718
290 074d495a | 110 21.00 -210.00 1399.25 0 0 0.000 0.000 | 38 1.00 765.00 1413.09 12 2 29.314 26.861 | 14 594.18 1876.85 -4.763 14.218
295 074d495a | 10 2.00 -210.00 1399.25 0 0 0.000 0.000 | 38 5.00 869.72 1485.05 12 0 24.301 22.267 | 14 570.36 1925.44 -4.763 6.718
300 074d495a | 10 7.00 -210.00 1399.25 0 0 0.000 0.000 | 38 10.00 972.43 1495.16 12 0 18.034 16.525 | 14 546.55 1936.53 -4.763 -0.782
305 074d495a | 10 12.00 -210.00 1399.25 0 0 0.000 0.000 | 38 15.00 1043.79 1437.69 12 0 11.767 10.782 | 14 522.74 1910.12 -4.763 -8.282
310 074d495a | 10 17.00 -210.00 1399.25 0 0 0.000 0.000 | 38 20.00 1079.99 1440.88 12 0 4.250 0.375 | 14 498.92 1846.20 -4.763 -15.782
315 074d495a | 10 22.00 -210.00 1399.25 0 0 0.000 0.000 | 10 1.00 1084.01 1441.24 12 0 0.000 0.000 | 14 475.11 1744.79 -4.763 -23.282
320 074d495a | 10 27.00 -210.00 1399.25 0 0 0.000 0.000 | 10 6.00 1084.01 1441.24 12 0 0.000 0.000 | 14 451.29 1605.88 -4.763 -30.782
325 074d495a | 10 32.00 -210.00 1399.25 0 0 0.000 0.000 | 10 11.00 1084.01 1441.24 12 0 0.000 0.000 | 14 427.48 1429.47 -4.763 -38.282
330 074d495a | 10 37.00 -210.00 1399.25 0 0 0.000 0.000 | 10 16.00 1084.01 1441.24 12 0 0.000 0.000 | 14 422.72 1428.27 0.000 0.000
335 074d495a | 10 42.00 -210.00 1399.25 0 0 0.000 0.000 | 10 21.00 1084.01 1441.24 12 0 0.000 0.000 | 14 422.72 1428.27 0.000 0.000
340 074d495a | 10 47.00 -210.00 1399.25 0 0 0.000 0.000 | 10 26.00 1084.01 1441.24 12 0 0.000 0.000 | 14 422.72 1428.27 0.000 0.000
345 074d495a | 10 52.00 -210.00 1399.25 0 0 0.000 0.000 | 10 31.00 1084.01 1441.24 12 0 0.000 0.000 | 14 422.72 1428.27 0.000 0.000
350 074d495a | 10 57.00 -210.00 1399.25 0 0 0.000 0.000 | 10 36.00 1084.01 1441.24 12 0 0.000 0.000 | 14 422.72 1428.27 0.000 0.000
355 074d495a | 10 62.00 -210.00 1399.25 0 0 0.000 0.000 | 10 41.00 1084.01 1441.24 12 0 0.000 0.000 | 14 422.72 1428.27 0.000 0.000
360 074d495a | 10 67.00 -210.00 1399.25 0 0 0.000 0.000 | 10 46.00 1084.01 1441.24 12 0 0.000 0.000 | 14 422.72 1428.27 0.000 0.000
365 074d495a | 10 72.00 -210.00 1399.25 0 0 0.000 0.000 | 10 1.00 1084.01 1441.24 12 0 0.000 0.000 | 14 422.72 1428.27 0.000 0.000
370 074d495a | 10 77.00 -210.00 1399.25 0 0 0.000 0.000 | 10 6.00 1084.01 1441.24 12 0 0.000 0.000 | 14 422.72 1428.27 0.000 0.000
375 074d495a | 10 2.00 -210.00 1399.25 0 0 0.000 0.000 | 10 11.00 1084.01 1441.24 12 0 0.000 0.000 | 14 422.72 1428.27 0.000 0.000
380 074d495a | 10 7.00 -210.00 1399.25 0 0 0.000 0.000 | 10 16.00 1084.01 1441.24 12 0 0.000 0.000 | 14 422.72 1428.27 0.000 0.000
385 074d495a | 10 12.00 -210.00 1399.25 0 0 0.000 0.000 | 10 21.00 1084.01 1441.24 12 0 0.000 0.000 | 14 422.72 1428.27 0.000 0.000
390 074d495a | 10 17.00 -210.00 1399.25 0 0 0.000 0.000 | 10 26.00 1084.01 1441.24 12 0 0.000 0.000 | 14 422.72 1428.27 0.000 0.000
395 074d495a | 10 22.00 -210.00 1399.25 0 0 0.000 0.000 | 10 31.00 1084.01 1441.24 12 0 0.000 0.000 | 14 422.72 1428.27 0.000 0.000
400 074d495a | 10 27.00 -210.00 1399.25 0 0 0.000 0.000 | 10 36.00 1084.01 1441.24 12 0 0.000 0.000 | 14 422.72 1428.27 0.000 0.000
";

#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn a_dummy_pulled_by_link_follows_the_floor_under_his_hand_as_on_the_n64() {
    let replay = Replay {
        p1: FighterKind::Link,
        route: "30:1:start;40:1:y=-80;50:1:x=80;60:1:x=80;70:1:x=80;80:1:x=80;\
                90:1:x=80;100:1:x=80;110:1:a;120:1:start;170:1:a;220:1:z+a+x=127",
        weapons: &no_weapons,
    };
    let Some(out) = replay.run(LINK_PULLS_THE_DUMMY_DOWN_THE_SLOPE) else {
        return;
    };
    out.check("Link's grab pulls the dummy along the Castle's slope");
}

#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn a_thrown_ray_gun_hits_the_dummy_and_stays_on_landing_as_on_the_n64() {
    let replay = Replay {
        p1: FighterKind::Link,
        route: "30:1:start;40:1:y=-80;50:1:x=80;60:1:x=80;70:1:x=80;80:1:x=80;\
                90:1:x=80;100:1:x=80;110:1:x=80;120:1:x=80;130:1:a;140:1:start;\
                190:1:a;240:1:z+a+x=127",
        weapons: &no_weapons,
    };
    let Some(out) = replay.run(LINK_RAY_GUN_HITS_THE_DUMMY) else {
        return;
    };
    out.check("Ray Gun smash throw into the dummy");
}

#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn a_jabbed_bob_omb_blasts_link_out_as_on_the_n64() {
    let replay = Replay {
        p1: FighterKind::Link,
        route: "30:1:start;40:1:y=-80;50:1:x=80;60:1:x=80;70:1:x=80;80:1:x=80;\
                90:1:x=80;100:1:x=80;110:1:x=80;120:1:x=80;130:1:x=80;140:1:x=80;\
                150:1:x=80;160:1:x=80;170:1:a;180:1:start;230:1:a;280:1:z+a+x=127",
        weapons: &no_weapons,
    };
    let Some(out) = replay.run(LINK_JABS_A_BOB_OMB) else {
        return;
    };
    out.check("Bob-omb explosion and bottom blast zone");
}

#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn a_thrown_motion_sensor_bomb_hits_the_dummy_as_on_the_n64() {
    let replay = Replay {
        p1: FighterKind::Link,
        route: "30:1:start;40:1:y=-80;50:1:x=80;60:1:x=80;70:1:x=80;80:1:x=80;\
                90:1:x=80;100:1:x=80;110:1:x=80;120:1:x=80;130:1:x=80;140:1:x=80;\
                150:1:x=80;160:1:a;170:1:start;220:1:a;270:1:z+a+x=127",
        weapons: &no_weapons,
    };
    let Some(out) = replay.run(LINK_MOTION_SENSOR_BOMB_HITS_THE_DUMMY) else {
        return;
    };
    out.check("Motion-Sensor Bomb smash throw into the dummy");
}

/// `hyegg`: Yoshi's Egg Throw held for 30 frames of full right tilt, the
/// egg's blast catching the dummy (`wpYoshiEggHitInitVars`).
const YOSHI_EGG_HITS_THE_DUMMY: &str = "
58 00000001 | 10 59.00 -210.00 1399.25 0 0 0.000 0.000 | 10 9.00 765.00 1413.09 0 0 0.000 0.000
59 00000001 | 10 60.00 -210.00 1399.25 0 0 0.000 0.000 | 10 10.00 765.00 1413.09 0 0 0.000 0.000
60 00000001 | 222 1.00 -210.00 1399.25 0 0 0.000 0.000 | 10 11.00 765.00 1413.09 0 0 0.000 0.000
61 00000001 | 222 2.00 -210.00 1399.25 0 0 0.000 0.000 | 10 12.00 765.00 1413.09 0 0 0.000 0.000
62 00000001 | 222 3.00 -210.00 1399.25 0 0 0.000 0.000 | 10 13.00 765.00 1413.09 0 0 0.000 0.000
63 00000001 | 222 4.00 -210.00 1399.25 0 0 0.000 0.000 | 10 14.00 765.00 1413.09 0 0 0.000 0.000 |  | 5 -311.75 1568.44 0.000 0.000
64 18be873a | 222 5.00 -210.00 1399.25 0 0 0.000 0.000 | 10 15.00 765.00 1413.09 0 0 0.000 0.000 |  | 5 -363.45 1569.48 0.000 0.000
65 18be873a | 222 6.00 -210.00 1399.25 0 0 0.000 0.000 | 10 16.00 765.00 1413.09 0 0 0.000 0.000 |  | 5 -387.58 1574.98 0.000 0.000
66 18be873a | 222 7.00 -210.00 1399.25 0 0 0.000 0.000 | 10 17.00 765.00 1413.09 0 0 0.000 0.000 |  | 5 -395.62 1613.75 0.000 0.000
67 18be873a | 222 8.00 -210.00 1399.25 0 0 0.000 0.000 | 10 18.00 765.00 1413.09 0 0 0.000 0.000 |  | 5 -396.88 1781.23 0.000 0.000
68 18be873a | 222 9.00 -210.00 1399.25 0 0 0.000 0.000 | 10 19.00 765.00 1413.09 0 0 0.000 0.000 |  | 5 -393.10 1644.00 0.000 0.000
69 18be873a | 222 10.00 -210.00 1399.25 0 0 0.000 0.000 | 10 20.00 765.00 1413.09 0 0 0.000 0.000 |  | 5 -388.04 1688.28 0.000 0.000
70 18be873a | 222 11.00 -210.00 1399.25 0 0 0.000 0.000 | 10 21.00 765.00 1413.09 0 0 0.000 0.000 |  | 5 -385.45 1733.38 0.000 0.000
74 18be873a | 222 15.00 -210.00 1399.25 0 0 0.000 0.000 | 10 25.00 765.00 1413.09 0 0 0.000 0.000 |  | 5 -387.34 1679.07 0.000 0.000
78 18be873a | 222 19.00 -210.00 1399.25 0 0 0.000 0.000 | 10 29.00 765.00 1413.09 0 0 0.000 0.000 |  | 5 -386.83 1699.00 0.000 0.000
82 e7847115 | 222 23.00 -210.00 1399.25 0 0 0.000 0.000 | 10 33.00 765.00 1413.09 0 0 0.000 0.000 |  | 5 -270.10 2062.06 30.091 39.932
86 e7847115 | 222 27.00 -210.00 1399.25 0 0 0.000 0.000 | 10 37.00 765.00 1413.09 0 0 0.000 0.000 |  | 5 -149.74 2194.79 30.091 29.132
90 e7847115 | 222 31.00 -210.00 1399.25 0 0 0.000 0.000 | 10 41.00 765.00 1413.09 0 0 0.000 0.000 |  | 5 -29.37 2284.31 30.091 18.332
94 e7847115 | 222 35.00 -210.00 1399.25 0 0 0.000 0.000 | 10 45.00 765.00 1413.09 0 0 0.000 0.000 |  | 5 90.99 2330.64 30.091 7.532
98 e7847115 | 222 39.00 -210.00 1399.25 0 0 0.000 0.000 | 10 49.00 765.00 1413.09 0 0 0.000 0.000 |  | 5 211.35 2333.77 30.091 -3.268
102 e7847115 | 222 43.00 -210.00 1399.25 0 0 0.000 0.000 | 10 3.00 765.00 1413.09 0 0 0.000 0.000 |  | 5 331.71 2293.69 30.091 -14.068
106 e7847115 | 222 47.00 -210.00 1399.25 0 0 0.000 0.000 | 10 7.00 765.00 1413.09 0 0 0.000 0.000 |  | 5 452.08 2210.42 30.091 -24.868
110 e7847115 | 222 51.00 -210.00 1399.25 0 0 0.000 0.000 | 10 11.00 765.00 1413.09 0 0 0.000 0.000 |  | 5 572.44 2083.95 30.091 -35.668
114 e7847115 | 222 55.00 -210.00 1399.25 0 0 0.000 0.000 | 51 1.00 765.00 1413.09 14 9 49.280 45.156 |  | 5 692.80 1914.28 0.000 0.000
115 fe87df14
116 c40df552
118 c40df552 | 222 59.00 -210.00 1399.25 0 0 0.000 0.000 | 51 1.00 765.00 1413.09 14 5 49.280 45.156 |  | 5 692.80 1914.28 0.000 0.000
122 c40df552 | 222 63.00 -210.00 1399.25 0 0 0.000 0.000 | 51 1.00 765.00 1413.09 14 1 49.280 45.156 |  | 5 692.80 1914.28 0.000 0.000
126 c40df552 | 222 67.00 -210.00 1399.25 0 0 0.000 0.000 | 51 5.00 949.58 1558.23 14 0 44.266 40.562
130 c40df552 | 222 71.00 -210.00 1399.25 0 0 0.000 0.000 | 51 9.00 1114.12 1646.59 14 0 39.253 35.968
134 c40df552 | 10 3.00 -210.00 1399.25 0 0 0.000 0.000 | 51 13.00 1258.59 1678.18 14 0 34.239 31.374
138 c40df552 | 10 7.00 -210.00 1399.25 0 0 0.000 0.000 | 51 17.00 1383.02 1652.99 14 0 29.226 26.780
142 c40df552 | 10 11.00 -210.00 1399.25 0 0 0.000 0.000 | 51 21.00 1487.38 1576.63 14 0 24.212 22.186
146 c40df552 | 10 15.00 -210.00 1399.25 0 0 0.000 0.000 | 51 25.00 1571.70 1477.89 14 0 19.199 17.592
150 c40df552 | 10 19.00 -210.00 1399.25 0 0 0.000 0.000 | 51 0.00 1635.96 1360.78 14 0 14.185 12.998
154 c40df552 | 10 23.00 -210.00 1399.25 0 0 0.000 0.000 | 68 1.00 1679.22 1280.05 14 0 8.226 -3.463
158 c40df552 | 10 27.00 -210.00 1399.25 0 0 0.000 0.000 | 68 5.00 1698.30 1272.02 14 0 2.696 -1.135
162 c40df552 | 10 31.00 -210.00 1399.25 0 0 0.000 0.000 | 68 9.00 1699.61 1271.47 14 0 0.000 0.000
166 c40df552 | 10 35.00 -210.00 1399.25 0 0 0.000 0.000 | 68 13.00 1699.61 1271.47 14 0 0.000 0.000
170 c40df552 | 10 39.00 -210.00 1399.25 0 0 0.000 0.000 | 68 17.00 1699.61 1271.47 14 0 0.000 0.000
174 c40df552 | 10 43.00 -210.00 1399.25 0 0 0.000 0.000 | 68 21.00 1699.61 1271.47 14 0 0.000 0.000
178 c40df552 | 10 47.00 -210.00 1399.25 0 0 0.000 0.000 | 68 25.00 1699.61 1271.47 14 0 0.000 0.000
180 c40df552 | 10 49.00 -210.00 1399.25 0 0 0.000 0.000 | 70 0.00 1699.61 1271.47 14 0 0.000 0.000
";

/// `hyegg2`: the same throw with no tilt, the blast's 340 radius reaching
/// the dummy from 400 units away.
const YOSHI_EGG_BLAST_REACHES_THE_DUMMY: &str = "
58 00000001 | 10 59.00 -210.00 1399.25 0 0 0.000 0.000 | 10 9.00 765.00 1413.09 0 0 0.000 0.000
59 00000001 | 10 60.00 -210.00 1399.25 0 0 0.000 0.000 | 10 10.00 765.00 1413.09 0 0 0.000 0.000
60 00000001 | 222 1.00 -210.00 1399.25 0 0 0.000 0.000 | 10 11.00 765.00 1413.09 0 0 0.000 0.000
61 00000001 | 222 2.00 -210.00 1399.25 0 0 0.000 0.000 | 10 12.00 765.00 1413.09 0 0 0.000 0.000
62 00000001 | 222 3.00 -210.00 1399.25 0 0 0.000 0.000 | 10 13.00 765.00 1413.09 0 0 0.000 0.000
63 00000001 | 222 4.00 -210.00 1399.25 0 0 0.000 0.000 | 10 14.00 765.00 1413.09 0 0 0.000 0.000 |  | 5 -311.75 1568.44 0.000 0.000
64 18be873a | 222 5.00 -210.00 1399.25 0 0 0.000 0.000 | 10 15.00 765.00 1413.09 0 0 0.000 0.000 |  | 5 -363.45 1569.48 0.000 0.000
65 18be873a | 222 6.00 -210.00 1399.25 0 0 0.000 0.000 | 10 16.00 765.00 1413.09 0 0 0.000 0.000 |  | 5 -387.58 1574.98 0.000 0.000
66 18be873a | 222 7.00 -210.00 1399.25 0 0 0.000 0.000 | 10 17.00 765.00 1413.09 0 0 0.000 0.000 |  | 5 -395.62 1613.75 0.000 0.000
67 18be873a | 222 8.00 -210.00 1399.25 0 0 0.000 0.000 | 10 18.00 765.00 1413.09 0 0 0.000 0.000 |  | 5 -396.88 1781.23 0.000 0.000
68 18be873a | 222 9.00 -210.00 1399.25 0 0 0.000 0.000 | 10 19.00 765.00 1413.09 0 0 0.000 0.000 |  | 5 -393.10 1644.00 0.000 0.000
69 18be873a | 222 10.00 -210.00 1399.25 0 0 0.000 0.000 | 10 20.00 765.00 1413.09 0 0 0.000 0.000 |  | 5 -388.04 1688.28 0.000 0.000
70 18be873a | 222 11.00 -210.00 1399.25 0 0 0.000 0.000 | 10 21.00 765.00 1413.09 0 0 0.000 0.000 |  | 5 -385.45 1733.38 0.000 0.000
74 18be873a | 222 15.00 -210.00 1399.25 0 0 0.000 0.000 | 10 25.00 765.00 1413.09 0 0 0.000 0.000 |  | 5 -387.34 1679.07 0.000 0.000
78 18be873a | 222 19.00 -210.00 1399.25 0 0 0.000 0.000 | 10 29.00 765.00 1413.09 0 0 0.000 0.000 |  | 5 -386.83 1699.00 0.000 0.000
82 e7847115 | 222 23.00 -210.00 1399.25 0 0 0.000 0.000 | 10 33.00 765.00 1413.09 0 0 0.000 0.000 |  | 5 -285.57 2069.94 14.619 47.815
86 e7847115 | 222 27.00 -210.00 1399.25 0 0 0.000 0.000 | 10 37.00 765.00 1413.09 0 0 0.000 0.000 |  | 5 -227.10 2234.20 14.619 37.015
90 e7847115 | 222 31.00 -210.00 1399.25 0 0 0.000 0.000 | 10 41.00 765.00 1413.09 0 0 0.000 0.000 |  | 5 -168.62 2355.26 14.619 26.215
94 e7847115 | 222 35.00 -210.00 1399.25 0 0 0.000 0.000 | 10 45.00 765.00 1413.09 0 0 0.000 0.000 |  | 5 -110.15 2433.12 14.619 15.415
98 e7847115 | 222 39.00 -210.00 1399.25 0 0 0.000 0.000 | 10 49.00 765.00 1413.09 0 0 0.000 0.000 |  | 5 -51.68 2467.79 14.619 4.615
102 e7847115 | 222 43.00 -210.00 1399.25 0 0 0.000 0.000 | 10 3.00 765.00 1413.09 0 0 0.000 0.000 |  | 5 6.80 2459.25 14.619 -6.185
106 e7847115 | 222 47.00 -210.00 1399.25 0 0 0.000 0.000 | 10 7.00 765.00 1413.09 0 0 0.000 0.000 |  | 5 65.27 2407.51 14.619 -16.985
110 e7847115 | 222 51.00 -210.00 1399.25 0 0 0.000 0.000 | 10 11.00 765.00 1413.09 0 0 0.000 0.000 |  | 5 123.75 2312.57 14.619 -27.785
114 e7847115 | 222 55.00 -210.00 1399.25 0 0 0.000 0.000 | 10 15.00 765.00 1413.09 0 0 0.000 0.000 |  | 5 182.22 2174.43 14.619 -38.585
118 e7847115 | 222 59.00 -210.00 1399.25 0 0 0.000 0.000 | 10 19.00 765.00 1413.09 0 0 0.000 0.000 |  | 5 240.70 1993.09 14.619 -49.385
122 e7847115 | 222 63.00 -210.00 1399.25 0 0 0.000 0.000 | 10 23.00 765.00 1413.09 0 0 0.000 0.000 |  | 5 299.17 1768.55 14.619 -60.185
126 e7847115 | 222 67.00 -210.00 1399.25 0 0 0.000 0.000 | 52 1.00 765.00 1413.09 14 9 49.280 45.156 |  | 5 357.64 1500.81 0.000 0.000
127 fe87df14
128 c40df552
130 c40df552 | 222 71.00 -210.00 1399.25 0 0 0.000 0.000 | 52 1.00 765.00 1413.09 14 5 49.280 45.156 |  | 5 357.64 1500.81 0.000 0.000
134 c40df552 | 10 3.00 -210.00 1399.25 0 0 0.000 0.000 | 52 1.00 765.00 1413.09 14 1 49.280 45.156 |  | 5 357.64 1500.81 0.000 0.000
138 c40df552 | 10 7.00 -210.00 1399.25 0 0 0.000 0.000 | 52 5.00 949.58 1558.23 14 0 44.266 40.562
142 c40df552 | 10 11.00 -210.00 1399.25 0 0 0.000 0.000 | 52 9.00 1114.12 1646.59 14 0 39.253 35.968
146 c40df552 | 10 15.00 -210.00 1399.25 0 0 0.000 0.000 | 52 13.00 1258.59 1678.18 14 0 34.239 31.374
150 c40df552 | 10 19.00 -210.00 1399.25 0 0 0.000 0.000 | 52 17.00 1383.02 1652.99 14 0 29.226 26.780
154 c40df552 | 10 23.00 -210.00 1399.25 0 0 0.000 0.000 | 52 21.00 1487.38 1576.63 14 0 24.212 22.186
158 c40df552 | 10 27.00 -210.00 1399.25 0 0 0.000 0.000 | 52 25.00 1571.70 1477.89 14 0 19.199 17.592
162 c40df552 | 10 31.00 -210.00 1399.25 0 0 0.000 0.000 | 52 0.00 1635.96 1360.78 14 0 14.185 12.998
166 c40df552 | 10 35.00 -210.00 1399.25 0 0 0.000 0.000 | 68 1.00 1679.22 1280.05 14 0 8.226 -3.463
170 c40df552 | 10 39.00 -210.00 1399.25 0 0 0.000 0.000 | 68 5.00 1698.30 1272.02 14 0 2.696 -1.135
174 c40df552 | 10 43.00 -210.00 1399.25 0 0 0.000 0.000 | 68 9.00 1699.61 1271.47 14 0 0.000 0.000
178 c40df552 | 10 47.00 -210.00 1399.25 0 0 0.000 0.000 | 68 13.00 1699.61 1271.47 14 0 0.000 0.000
182 c40df552 | 10 51.00 -210.00 1399.25 0 0 0.000 0.000 | 68 17.00 1699.61 1271.47 14 0 0.000 0.000
186 c40df552 | 10 55.00 -210.00 1399.25 0 0 0.000 0.000 | 68 21.00 1699.61 1271.47 14 0 0.000 0.000
190 c40df552 | 10 59.00 -210.00 1399.25 0 0 0.000 0.000 | 68 25.00 1699.61 1271.47 14 0 0.000 0.000
194 c40df552 | 10 63.00 -210.00 1399.25 0 0 0.000 0.000 | 70 0.00 1699.61 1271.47 14 0 0.000 0.000
198 c40df552 | 10 67.00 -210.00 1399.25 0 0 0.000 0.000 | 70 0.00 1699.61 1271.47 14 0 0.000 0.000
200 c40df552 | 10 69.00 -210.00 1399.25 0 0 0.000 0.000 | 70 0.00 1699.61 1271.47 14 0 0.000 0.000
";

#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn a_tilted_yoshi_egg_hits_the_dummy_as_on_the_n64() {
    let replay = Replay {
        p1: FighterKind::Yoshi,
        route: "60:1:b+y=127;63:30:x=127",
        weapons: &fighter_weapons,
    };
    let Some(out) = replay.run(YOSHI_EGG_HITS_THE_DUMMY) else {
        return;
    };
    out.check("Yoshi's egg, held in his hand, then thrown into the dummy");
}

#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn a_yoshi_egg_blast_reaches_the_dummy_as_on_the_n64() {
    let replay = Replay {
        p1: FighterKind::Yoshi,
        route: "60:1:b+y=127",
        weapons: &fighter_weapons,
    };
    let Some(out) = replay.run(YOSHI_EGG_BLAST_REACHES_THE_DUMMY) else {
        return;
    };
    out.check("Yoshi's egg blast at its unhalved explode size");
}

/// `hcamy`: the `yoshi` golden's moves on Dream Land (`warp_yoshipupupu`):
/// Yoshi jabs, walks left, and throws the egg up and to the left; the
/// battle camera frames both fighters and the egg (`is_camera_follow`
/// from `wpYoshiEggThrowMakeWeapon` on, in hand and in flight), with
/// `eye at fovy target_dist` per row. The rows stop at the blast (102):
/// its `efManagerQuakeMakeEffect(2)` shakes `at` from 104 on, and the
/// replays run no effects (the PSP's `effects_then_camera`, RE-420).
const YOSHI_EGG_MOVES_THE_CAMERA: &str = "
0 c823f683 | 10 1.00 0.00 0.00 0 0 0.000 0.000 | 10 1.00 -1397.00 904.00 0 0 0.000 0.000 |  |  | 0.8740 394.5759 9950.2109 -49.5671 324.2970 0.0000 38.0000 9549.8223
2 c823f683 | 190 2.00 0.00 0.00 0 0 0.000 0.000 | 10 3.00 -1397.00 904.00 0 0 0.000 0.000 |  |  | 3.9340 559.3721 9747.0156 -141.2199 369.2238 0.0000 38.0000 8748.2236
4 c823f683 | 190 4.00 0.00 0.00 0 0 0.000 0.000 | 10 5.00 -1397.00 904.00 0 0 0.000 0.000 |  |  | 5.0450 695.2932 9446.6006 -222.7587 409.1928 0.0000 38.0000 8062.3560
6 c823f683 | 190 6.00 0.00 0.00 0 0 0.000 0.000 | 10 7.00 -1397.00 904.00 0 0 0.000 0.000 |  |  | 3.1388 806.5723 9087.0156 -294.4292 444.3246 0.0000 38.0000 7475.5103
8 c823f683 | 190 8.00 0.00 0.00 0 0 0.000 0.000 | 10 9.00 -1397.00 904.00 0 0 0.000 0.000 |  |  | -3.9945 897.8800 8696.2539 -356.7975 474.8966 0.0000 38.0000 6973.3906
10 c823f683 | 190 10.00 0.00 0.00 0 0 0.000 0.000 | 10 11.00 -1397.00 904.00 0 0 0.000 0.000 |  |  | -14.4792 971.0843 8294.7227 -410.6189 501.2790 0.0000 38.0000 6543.7642
12 c823f683 | 190 12.00 0.00 0.00 0 0 0.000 0.000 | 10 13.00 -1397.00 904.00 0 0 0.000 0.000 |  |  | -27.6957 1029.8605 7896.6904 -456.7397 523.8867 0.0000 38.0000 6176.1650
14 c823f683 | 190 14.00 0.00 0.00 0 0 0.000 0.000 | 10 15.00 -1397.00 904.00 0 0 0.000 0.000 |  |  | -44.6416 1077.1569 7512.0679 -496.0289 543.1456 0.0000 38.0000 5861.6382
16 c823f683 | 190 16.00 0.00 0.00 0 0 0.000 0.000 | 10 17.00 -1397.00 904.00 0 0 0.000 0.000 |  |  | -63.3890 1113.7096 7147.4326 -529.3320 559.4703 0.0000 38.0000 5592.5215
18 c823f683 | 190 18.00 0.00 0.00 0 0 0.000 0.000 | 10 19.00 -1397.00 904.00 0 0 0.000 0.000 |  |  | -83.3306 1142.6257 6806.5596 -557.4422 573.2494 0.0000 38.0000 5362.2583
20 c823f683 | 190 20.00 0.00 0.00 0 0 0.000 0.000 | 10 21.00 -1397.00 904.00 0 0 0.000 0.000 |  |  | -103.9280 1164.8552 6491.5967 -581.0844 584.8384 0.0000 38.0000 5165.2393
22 c823f683 | 190 22.00 0.00 0.00 0 0 0.000 0.000 | 10 23.00 -1397.00 904.00 0 0 0.000 0.000 |  |  | -124.1087 1181.9191 6203.1875 -600.9084 594.5559 0.0000 38.0000 4996.6646
24 c823f683 | 10 0.00 0.00 0.00 0 0 0.000 0.000 | 10 25.00 -1397.00 904.00 0 0 0.000 0.000 |  |  | -143.8657 1195.0220 5941.1064 -617.4882 602.6830 0.0000 38.0000 4852.4277
26 c823f683 | 12 1.00 -11.40 0.00 0 0 0.000 0.000 | 10 27.00 -1397.00 904.00 0 0 0.000 0.000 |  |  | -162.5036 1203.7469 5704.6060 -631.8343 609.5141 0.0000 38.0000 4729.0156
28 c823f683 | 12 3.00 -34.20 0.00 0 0 0.000 0.000 | 10 29.00 -1397.00 904.00 0 0 0.000 0.000 |  |  | -180.6315 1210.5420 5492.1748 -645.7396 615.3912 0.0000 38.0000 4623.4209
30 c823f683 | 12 5.00 -57.00 0.00 0 0 0.000 0.000 | 10 31.00 -1397.00 904.00 0 0 0.000 0.000 |  |  | -198.0663 1214.6458 5302.4009 -659.2668 620.4667 0.0000 38.0000 4533.0718
32 c823f683 | 12 7.00 -79.80 0.00 0 0 0.000 0.000 | 10 33.00 -1397.00 904.00 0 0 0.000 0.000 |  |  | -214.7248 1218.0747 5133.3413 -672.4707 624.8705 0.0000 38.0000 4455.7666
34 c823f683 | 12 9.00 -102.60 0.00 0 0 0.000 0.000 | 10 35.00 -1397.00 904.00 0 0 0.000 0.000 |  |  | -229.9363 1219.8302 4983.3628 -685.3989 628.7127 0.0000 38.0000 4389.6221
36 c823f683 | 12 11.00 -125.40 0.00 0 0 0.000 0.000 | 10 37.00 -1397.00 904.00 0 0 0.000 0.000 |  |  | -244.4135 1220.3936 4850.7539 -698.0924 632.0863 0.0000 38.0000 4333.0273
38 c823f683 | 222 2.00 -130.40 0.00 0 0 0.000 0.000 | 10 39.00 -1397.00 904.00 0 0 0.000 0.000 |  |  | -258.0171 1220.7712 4733.7129 -709.5098 634.9666 0.0000 38.0000 4284.6035
40 c823f683 | 222 4.00 -130.40 0.00 0 0 0.000 0.000 | 10 41.00 -1397.00 904.00 0 0 0.000 0.000 |  | 5 -28.65 169.19 0.000 0.000 | -270.1113 1219.8726 4632.4893 -710.8384 630.0440 0.0000 38.0000 4260.2544
41 cae1df84
42 cae1df84 | 222 6.00 -130.40 0.00 0 0 0.000 0.000 | 10 43.00 -1397.00 904.00 0 0 0.000 0.000 |  | 5 47.18 175.73 0.000 0.000 | -279.1917 1215.6062 4549.7153 -698.9533 619.1721 0.0000 38.0000 4254.4077
44 cae1df84 | 222 8.00 -130.40 0.00 0 0 0.000 0.000 | 10 45.00 -1397.00 904.00 0 0 0.000 0.000 |  | 5 56.48 381.98 0.000 0.000 | -285.8101 1210.2362 4479.4053 -687.3721 617.9283 0.0000 38.0000 4228.5708
46 cae1df84 | 222 10.00 -130.40 0.00 0 0 0.000 0.000 | 10 47.00 -1397.00 904.00 0 0 0.000 0.000 |  | 5 47.64 289.02 0.000 0.000 | -290.3071 1205.1471 4418.0088 -678.3110 616.5531 0.0000 38.0000 4207.4443
48 cae1df84 | 222 12.00 -130.40 0.00 0 0 0.000 0.000 | 10 49.00 -1397.00 904.00 0 0 0.000 0.000 |  | 5 45.17 340.18 0.000 0.000 | -294.2289 1200.8175 4362.8774 -671.2544 620.0566 0.0000 38.0000 4177.1519
50 cae1df84 | 222 14.00 -130.40 0.00 0 0 0.000 0.000 | 10 1.00 -1397.00 904.00 0 0 0.000 0.000 |  | 5 46.28 302.01 0.000 0.000 | -297.2811 1197.1241 4313.5244 -665.3500 621.8790 0.0000 38.0000 4154.0249
52 cae1df84 | 222 16.00 -130.40 0.00 0 0 0.000 0.000 | 10 3.00 -1397.00 904.00 0 0 0.000 0.000 |  | 5 47.45 267.58 0.000 0.000 | -299.5595 1193.5973 4270.9878 -660.3683 620.2631 0.0000 38.0000 4142.4639
54 cae1df84 | 222 18.00 -130.40 0.00 0 0 0.000 0.000 | 10 5.00 -1397.00 904.00 0 0 0.000 0.000 |  | 5 47.35 285.50 0.000 0.000 | -300.7498 1190.2206 4234.5747 -656.2322 619.3647 0.0000 38.0000 4131.4419
56 cae1df84 | 222 20.00 -130.40 0.00 0 0 0.000 0.000 | 10 7.00 -1397.00 904.00 0 0 0.000 0.000 |  | 5 45.05 308.00 0.000 0.000 | -302.4663 1187.2864 4202.5542 -652.9752 620.6373 0.0000 38.0000 4116.7285
58 cae1df84 | 222 22.00 -130.40 0.00 0 0 0.000 0.000 | 10 9.00 -1397.00 904.00 0 0 0.000 0.000 |  | 5 -40.21 622.88 0.000 0.000 | -303.5366 1186.1987 4173.2397 -655.0206 623.8841 0.0000 38.0000 4099.5332
59 3d6c1037
60 3d6c1037 | 222 24.00 -130.40 0.00 0 0 0.000 0.000 | 10 11.00 -1397.00 904.00 0 0 0.000 0.000 |  | 5 -84.41 709.88 -22.099 42.151 | -305.3714 1185.5465 4146.5825 -662.4653 627.1144 0.0000 38.0000 4084.8203
62 3d6c1037 | 222 26.00 -130.40 0.00 0 0 0.000 0.000 | 10 13.00 -1397.00 904.00 0 0 0.000 0.000 |  | 5 -128.61 786.08 -22.099 36.751 | -307.6900 1185.2654 4122.4365 -672.4868 630.1508 0.0000 38.0000 4072.2314
64 3d6c1037 | 222 28.00 -130.40 0.00 0 0 0.000 0.000 | 10 15.00 -1397.00 904.00 0 0 0.000 0.000 |  | 5 -172.80 851.48 -22.099 31.351 | -310.8004 1185.3018 4100.6763 -684.6343 633.0269 0.0000 38.0000 4061.4602
66 3d6c1037 | 222 30.00 -130.40 0.00 0 0 0.000 0.000 | 10 17.00 -1397.00 904.00 0 0 0.000 0.000 |  | 5 -217.00 906.08 -22.099 25.951 | -314.8472 1186.7761 4080.9858 -698.0467 635.7238 0.0000 38.0000 4052.2441
68 3d6c1037 | 222 32.00 -130.40 0.00 0 0 0.000 0.000 | 10 19.00 -1397.00 904.00 0 0 0.000 0.000 |  | 5 -261.20 949.89 -22.099 20.551 | -319.3991 1188.2083 4063.3945 -709.5867 637.9936 0.0000 38.0000 4044.3584
70 3d6c1037 | 222 34.00 -130.40 0.00 0 0 0.000 0.000 | 10 21.00 -1397.00 904.00 0 0 0.000 0.000 |  | 5 -305.40 982.89 -22.099 15.151 | -323.9416 1189.5577 4047.7241 -719.1009 639.8649 0.0000 38.0000 4037.6111
72 3d6c1037 | 222 36.00 -130.40 0.00 0 0 0.000 0.000 | 10 23.00 -1397.00 904.00 0 0 0.000 0.000 |  | 5 -349.60 1005.09 -22.099 9.751 | -328.6785 1190.8011 4033.8499 -726.9442 641.4076 0.0000 38.0000 4031.8379
74 3d6c1037 | 222 38.00 -130.40 0.00 0 0 0.000 0.000 | 10 25.00 -1397.00 904.00 0 0 0.000 0.000 |  | 5 -393.80 1016.49 -22.099 4.351 | -333.3456 1191.9275 4021.5984 -733.4095 642.6792 0.0000 38.0000 4026.8984
76 3d6c1037 | 222 40.00 -130.40 0.00 0 0 0.000 0.000 | 10 27.00 -1397.00 904.00 0 0 0.000 0.000 |  | 5 -438.00 1017.09 -22.099 -1.049 | -337.6573 1192.9337 4010.7937 -738.7385 643.7274 0.0000 38.0000 4022.6721
78 3d6c1037 | 222 42.00 -130.40 0.00 0 0 0.000 0.000 | 10 29.00 -1397.00 904.00 0 0 0.000 0.000 |  | 5 -482.20 1006.90 -22.099 -6.449 | -341.5450 1193.8228 4001.2854 -743.1306 644.5912 0.0000 38.0000 4019.0559
80 3d6c1037 | 222 44.00 -130.40 0.00 0 0 0.000 0.000 | 10 31.00 -1397.00 904.00 0 0 0.000 0.000 |  | 5 -526.40 985.90 -22.099 -11.849 | -345.4731 1194.6006 3992.9822 -746.7503 645.3032 0.0000 38.0000 4015.9617
82 3d6c1037 | 222 46.00 -130.40 0.00 0 0 0.000 0.000 | 10 33.00 -1397.00 904.00 0 0 0.000 0.000 |  | 5 -570.59 954.10 -22.099 -17.249 | -348.1621 1196.4340 3985.4624 -749.7334 645.8900 0.0000 38.0000 4013.3142
84 3d6c1037 | 222 48.00 -130.40 0.00 0 0 0.000 0.000 | 10 35.00 -1397.00 904.00 0 0 0.000 0.000 |  | 5 -614.79 911.50 -22.099 -22.649 | -350.8732 1197.9531 3978.9312 -752.1917 646.3734 0.0000 38.0000 4011.0491
86 3d6c1037 | 222 50.00 -130.40 0.00 0 0 0.000 0.000 | 10 37.00 -1397.00 904.00 0 0 0.000 0.000 |  | 5 -658.99 858.10 -22.099 -28.049 | -353.5096 1199.2097 3973.2642 -754.2175 646.7719 0.0000 38.0000 4009.1108
88 3d6c1037 | 222 52.00 -130.40 0.00 0 0 0.000 0.000 | 10 39.00 -1397.00 904.00 0 0 0.000 0.000 |  | 5 -703.19 793.91 -22.099 -33.449 | -356.0093 1200.2476 3968.3518 -755.8868 647.1002 0.0000 38.0000 4007.4524
90 3d6c1037 | 222 54.00 -130.40 0.00 0 0 0.000 0.000 | 10 41.00 -1397.00 904.00 0 0 0.000 0.000 |  | 5 -747.39 718.91 -22.099 -38.849 | -358.3351 1201.1030 3964.0969 -757.2623 647.3707 0.0000 38.0000 4006.0334
92 3d6c1037 | 222 56.00 -130.40 0.00 0 0 0.000 0.000 | 10 43.00 -1397.00 904.00 0 0 0.000 0.000 |  | 5 -791.59 633.11 -22.099 -44.249 | -360.4680 1201.8071 3960.4148 -758.3957 647.5937 0.0000 38.0000 4004.8196
94 3d6c1037 | 222 58.00 -130.40 0.00 0 0 0.000 0.000 | 10 45.00 -1397.00 904.00 0 0 0.000 0.000 |  | 5 -835.79 536.51 -22.099 -49.649 | -362.4015 1202.3855 3957.2305 -759.3296 647.7774 0.0000 38.0000 4003.7808
96 3d6c1037 | 222 60.00 -130.40 0.00 0 0 0.000 0.000 | 10 47.00 -1397.00 904.00 0 0 0.000 0.000 |  | 5 -879.99 429.11 -22.099 -55.049 | -363.5329 1202.8596 3954.4175 -760.0991 647.9288 0.0000 38.0000 4002.8921
98 3d6c1037 | 222 62.00 -130.40 0.00 0 0 0.000 0.000 | 10 49.00 -1397.00 904.00 0 0 0.000 0.000 |  | 5 -924.18 310.92 -22.099 -60.449 | -363.8743 1203.4255 3953.6499 -760.7329 647.4107 0.0000 38.0000 4019.5349
100 3d6c1037 | 222 64.00 -130.40 0.00 0 0 0.000 0.000 | 10 1.00 -1397.00 904.00 0 0 0.000 0.000 |  | 5 -968.38 181.92 -22.099 -65.849 | -361.4263 1205.4216 3981.2922 -761.2521 638.5170 0.0000 38.0000 4206.9204
102 0f3e862e | 222 66.00 -130.40 0.00 0 0 0.000 0.000 | 10 3.00 -1397.00 904.00 0 0 0.000 0.000 |  | 5 -990.48 113.37 0.000 0.000 | -356.7978 1206.8168 4030.9729 -761.6779 623.1893 0.0000 38.0000 4306.4966
";

#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn the_camera_follows_yoshis_egg_as_on_the_n64() {
    let replay = Replay {
        p1: FighterKind::Yoshi,
        route: "1:1:a;26:11:x=-30;37:1:b+x=-30+y=80;38:44:x=-30",
        weapons: &fighter_weapons,
    };
    let Some(out) = replay.run_on(DREAM_LAND, YOSHI_EGG_MOVES_THE_CAMERA, 0, None) else {
        return;
    };
    out.check("the battle camera over Yoshi's Egg Throw");
}

/// `hlbomb2`: Link pulls a bomb and throws it straight up; it falls back on
/// him and blasts him out (`itLinkBomb`).
const LINK_BOMB_FALLS_ON_LINK: &str = "
116 00000001 | 10 12.00 -210.00 1399.25 0 0 0.000 0.000 | 10 17.00 765.00 1413.09 0 0 0.000 0.000 | 21 -69.79 341.64 0.000 0.000
117 00000001 | 10 13.00 -210.00 1399.25 0 0 0.000 0.000 | 10 18.00 765.00 1413.09 0 0 0.000 0.000 | 21 -69.79 341.64 0.000 0.000
118 00000001 | 10 14.00 -210.00 1399.25 0 0 0.000 0.000 | 10 19.00 765.00 1413.09 0 0 0.000 0.000 | 21 -69.79 341.64 0.000 0.000
119 00000001 | 10 15.00 -210.00 1399.25 0 0 0.000 0.000 | 10 20.00 765.00 1413.09 0 0 0.000 0.000 | 21 -69.79 341.64 0.000 0.000
120 00000001 | 112 1.00 -210.00 1399.25 0 0 0.000 0.000 | 10 21.00 765.00 1413.09 0 0 0.000 0.000 | 21 -69.79 341.64 0.000 0.000
121 00000001 | 112 2.00 -210.00 1399.25 0 0 0.000 0.000 | 10 22.00 765.00 1413.09 0 0 0.000 0.000 | 21 -69.79 341.64 0.000 0.000
122 00000001 | 112 3.00 -210.00 1399.25 0 0 0.000 0.000 | 10 23.00 765.00 1413.09 0 0 0.000 0.000 | 21 -69.79 341.64 0.000 0.000
123 00000001 | 112 4.00 -210.00 1399.25 0 0 0.000 0.000 | 10 24.00 765.00 1413.09 0 0 0.000 0.000 | 21 -69.79 341.64 0.000 0.000
124 00000001 | 112 5.00 -210.00 1399.25 0 0 0.000 0.000 | 10 25.00 765.00 1413.09 0 0 0.000 0.000 | 21 -69.79 341.64 0.000 0.000
125 00000001 | 112 6.00 -210.00 1399.25 0 0 0.000 0.000 | 10 26.00 765.00 1413.09 0 0 0.000 0.000 | 21 -69.79 341.64 0.000 0.000
126 00000001 | 112 7.00 -210.00 1399.25 0 0 0.000 0.000 | 10 27.00 765.00 1413.09 0 0 0.000 0.000 | 21 -69.79 341.64 0.000 0.000
127 00000001 | 112 8.00 -210.00 1399.25 0 0 0.000 0.000 | 10 28.00 765.00 1413.09 0 0 0.000 0.000 | 21 -69.79 341.64 0.000 0.000
128 0029e2c0 | 112 9.00 -210.00 1399.25 0 0 0.000 0.000 | 10 29.00 765.00 1413.09 0 0 0.000 0.000 | 21 -196.87 1962.69 0.000 64.800
129 0029e2c0 | 112 10.00 -210.00 1399.25 0 0 0.000 0.000 | 10 30.00 765.00 1413.09 0 0 0.000 0.000 | 21 -196.87 2026.29 0.000 63.600
130 0029e2c0 | 112 11.00 -210.00 1399.25 0 0 0.000 0.000 | 10 31.00 765.00 1413.09 0 0 0.000 0.000 | 21 -196.87 2088.69 0.000 62.400
131 0029e2c0 | 112 12.00 -210.00 1399.25 0 0 0.000 0.000 | 10 32.00 765.00 1413.09 0 0 0.000 0.000 | 21 -196.87 2149.89 0.000 61.200
132 0029e2c0 | 112 13.00 -210.00 1399.25 0 0 0.000 0.000 | 10 33.00 765.00 1413.09 0 0 0.000 0.000 | 21 -196.87 2209.89 0.000 60.000
136 0029e2c0 | 112 17.00 -210.00 1399.25 0 0 0.000 0.000 | 10 37.00 765.00 1413.09 0 0 0.000 0.000 | 21 -196.87 2437.89 0.000 55.200
140 0029e2c0 | 112 21.00 -210.00 1399.25 0 0 0.000 0.000 | 10 41.00 765.00 1413.09 0 0 0.000 0.000 | 21 -196.87 2646.69 0.000 50.400
144 0029e2c0 | 10 1.00 -210.00 1399.25 0 0 0.000 0.000 | 10 45.00 765.00 1413.09 0 0 0.000 0.000 | 21 -196.87 2836.29 0.000 45.600
148 0029e2c0 | 10 5.00 -210.00 1399.25 0 0 0.000 0.000 | 10 49.00 765.00 1413.09 0 0 0.000 0.000 | 21 -196.87 3006.69 0.000 40.800
152 0029e2c0 | 10 9.00 -210.00 1399.25 0 0 0.000 0.000 | 10 3.00 765.00 1413.09 0 0 0.000 0.000 | 21 -196.87 3157.89 0.000 36.000
156 0029e2c0 | 10 13.00 -210.00 1399.25 0 0 0.000 0.000 | 10 7.00 765.00 1413.09 0 0 0.000 0.000 | 21 -196.87 3289.89 0.000 31.200
160 0029e2c0 | 10 17.00 -210.00 1399.25 0 0 0.000 0.000 | 10 11.00 765.00 1413.09 0 0 0.000 0.000 | 21 -196.87 3402.69 0.000 26.400
164 0029e2c0 | 10 21.00 -210.00 1399.25 0 0 0.000 0.000 | 10 15.00 765.00 1413.09 0 0 0.000 0.000 | 21 -196.87 3496.29 0.000 21.600
168 0029e2c0 | 10 25.00 -210.00 1399.25 0 0 0.000 0.000 | 10 19.00 765.00 1413.09 0 0 0.000 0.000 | 21 -196.87 3570.69 0.000 16.800
172 0029e2c0 | 10 29.00 -210.00 1399.25 0 0 0.000 0.000 | 10 23.00 765.00 1413.09 0 0 0.000 0.000 | 21 -196.87 3625.89 0.000 12.000
176 0029e2c0 | 10 33.00 -210.00 1399.25 0 0 0.000 0.000 | 10 27.00 765.00 1413.09 0 0 0.000 0.000 | 21 -196.87 3661.89 0.000 7.200
180 0029e2c0 | 10 37.00 -210.00 1399.25 0 0 0.000 0.000 | 10 31.00 765.00 1413.09 0 0 0.000 0.000 | 21 -196.87 3678.69 0.000 2.400
184 0029e2c0 | 10 41.00 -210.00 1399.25 0 0 0.000 0.000 | 10 35.00 765.00 1413.09 0 0 0.000 0.000 | 21 -196.87 3676.29 0.000 -2.400
188 0029e2c0 | 10 45.00 -210.00 1399.25 0 0 0.000 0.000 | 10 39.00 765.00 1413.09 0 0 0.000 0.000 | 21 -196.87 3654.69 0.000 -7.200
192 0029e2c0 | 10 49.00 -210.00 1399.25 0 0 0.000 0.000 | 10 43.00 765.00 1413.09 0 0 0.000 0.000 | 21 -196.87 3613.89 0.000 -12.000
196 0029e2c0 | 10 53.00 -210.00 1399.25 0 0 0.000 0.000 | 10 47.00 765.00 1413.09 0 0 0.000 0.000 | 21 -196.87 3553.89 0.000 -16.800
200 0029e2c0 | 10 57.00 -210.00 1399.25 0 0 0.000 0.000 | 10 1.00 765.00 1413.09 0 0 0.000 0.000 | 21 -196.87 3474.69 0.000 -21.600
204 0029e2c0 | 10 61.00 -210.00 1399.25 0 0 0.000 0.000 | 10 5.00 765.00 1413.09 0 0 0.000 0.000 | 21 -196.87 3376.29 0.000 -26.400
208 0029e2c0 | 10 65.00 -210.00 1399.25 0 0 0.000 0.000 | 10 9.00 765.00 1413.09 0 0 0.000 0.000 | 21 -196.87 3258.69 0.000 -31.200
212 0029e2c0 | 10 69.00 -210.00 1399.25 0 0 0.000 0.000 | 10 13.00 765.00 1413.09 0 0 0.000 0.000 | 21 -196.87 3121.89 0.000 -36.000
216 0029e2c0 | 10 73.00 -210.00 1399.25 0 0 0.000 0.000 | 10 17.00 765.00 1413.09 0 0 0.000 0.000 | 21 -196.87 2965.89 0.000 -40.800
220 0029e2c0 | 10 77.00 -210.00 1399.25 0 0 0.000 0.000 | 10 21.00 765.00 1413.09 0 0 0.000 0.000 | 21 -196.87 2790.69 0.000 -45.600
224 0029e2c0 | 10 1.00 -210.00 1399.25 0 0 0.000 0.000 | 10 25.00 765.00 1413.09 0 0 0.000 0.000 | 21 -196.87 2596.29 0.000 -50.400
228 0029e2c0 | 10 5.00 -210.00 1399.25 0 0 0.000 0.000 | 10 29.00 765.00 1413.09 0 0 0.000 0.000 | 21 -196.87 2382.69 0.000 -55.200
232 0029e2c0 | 10 9.00 -210.00 1399.25 0 0 0.000 0.000 | 10 33.00 765.00 1413.09 0 0 0.000 0.000 | 21 -196.87 2149.89 0.000 -60.000
236 0029e2c0 | 10 13.00 -210.00 1399.25 0 0 0.000 0.000 | 10 37.00 765.00 1413.09 0 0 0.000 0.000 | 21 -196.87 1897.89 0.000 -64.800
240 0029e2c0 | 10 17.00 -210.00 1399.25 0 0 0.000 0.000 | 10 41.00 765.00 1413.09 0 0 0.000 0.000 | 21 -196.87 1626.69 0.000 -69.600
242 2cd60a1e
243 daf1b4cc
244 0bb31550 | 52 1.00 -210.00 1399.25 5 4 -47.238 43.285 | 10 45.00 765.00 1413.09 0 0 0.000 0.000 | 21 -196.87 1512.23 0.000 0.000
246 fe87df14
248 8124f618 | 52 2.00 -255.98 1437.99 5 0 -45.984 42.137 | 10 49.00 765.00 1413.09 0 0 0.000 0.000
249 491c86cd
250 54ded7a1
251 074d495a
252 428b763e | 52 6.00 -427.39 1547.45 5 0 -40.971 37.543 | 10 3.00 765.00 1413.09 0 0 0.000 0.000
254 fa5aea62
255 92387aff
256 9ad4bb73 | 52 10.00 -578.74 1584.14 5 0 -35.957 32.949 | 10 7.00 765.00 1413.09 0 0 0.000 0.000
257 ce45da4e
258 eb899372
260 d6ae67d6 | 52 14.00 -710.03 1548.05 5 0 -30.944 28.355 | 10 11.00 765.00 1413.09 0 0 0.000 0.000
262 a350db7a
264 eb36b25e | 52 18.00 -821.27 1439.18 5 0 -25.930 23.761 | 10 15.00 765.00 1413.09 0 0 0.000 0.000
266 5f32f082
267 df49d9e6
268 df49d9e6 | 68 2.00 -910.22 1400.34 5 0 -19.424 0.030 | 10 19.00 765.00 1413.09 0 0 0.000 0.000
269 5e14f28a
271 1366fe6e
272 1366fe6e | 68 6.00 -967.92 1400.43 5 0 -11.424 0.018 | 10 23.00 765.00 1413.09 0 0 0.000 0.000
273 c2300192
275 15a13ff6
276 15a13ff6 | 68 10.00 -993.61 1400.47 5 0 -3.424 0.005 | 10 27.00 765.00 1413.09 0 0 0.000 0.000
277 59913d9a
279 121fbe7e
280 121fbe7e | 68 14.00 -995.03 1400.47 5 0 0.000 0.000 | 10 31.00 765.00 1413.09 0 0 0.000 0.000
281 3699c6a2
283 139d9a06
284 139d9a06 | 68 18.00 -995.03 1400.47 5 0 0.000 0.000 | 10 35.00 765.00 1413.09 0 0 0.000 0.000
285 187ebcaa
287 13e9f28e
288 13e9f28e | 68 22.00 -995.03 1400.47 5 0 0.000 0.000 | 10 39.00 765.00 1413.09 0 0 0.000 0.000
289 5db2c578
292 5db2c578 | 70 0.00 -995.03 1400.47 5 0 0.000 0.000 | 10 43.00 765.00 1413.09 0 0 0.000 0.000
293 a3c93fb2
296 a3c93fb2 | 70 0.00 -995.03 1400.47 5 0 0.000 0.000 | 10 47.00 765.00 1413.09 0 0 0.000 0.000
297 d7531bbc
300 d7531bbc | 70 0.00 -995.03 1400.47 5 0 0.000 0.000 | 10 1.00 765.00 1413.09 0 0 0.000 0.000
301 dc67e816
304 dc67e816 | 70 0.00 -995.03 1400.47 5 0 0.000 0.000 | 10 5.00 765.00 1413.09 0 0 0.000 0.000
305 0fbfa740
308 0fbfa740 | 70 0.00 -995.03 1400.47 5 0 0.000 0.000 | 10 9.00 765.00 1413.09 0 0 0.000 0.000
309 6ad66fba
312 6ad66fba | 70 0.00 -995.03 1400.47 5 0 0.000 0.000 | 10 13.00 765.00 1413.09 0 0 0.000 0.000
313 c22d0c04
316 c22d0c04 | 70 0.00 -995.03 1400.47 5 0 0.000 0.000 | 10 17.00 765.00 1413.09 0 0 0.000 0.000
317 b68e9a9e
320 b68e9a9e | 70 0.00 -995.03 1400.47 5 0 0.000 0.000 | 10 21.00 765.00 1413.09 0 0 0.000 0.000
";

#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn a_link_bomb_thrown_up_blasts_link_as_on_the_n64() {
    let replay = Replay {
        p1: FighterKind::Link,
        route: "60:1:b+y=-80;120:1:z+a+y=127",
        weapons: &no_weapons,
    };
    let Some(out) = replay.run(LINK_BOMB_FALLS_ON_LINK) else {
        return;
    };
    out.check("Link's bomb thrown up and landing on him");
}

/// `hjolt`: Pikachu's ground Thunder Jolt crawls to the dummy; its
/// electric hit holds him in `DamageE1` until hitlag ends.
const PIKACHU_JOLT_HITS_THE_DUMMY: &str = "
60 00000001 | 222 1.00 -210.00 1399.25 0 0 0.000 0.000 | 10 11.00 765.00 1413.09 0 0 0.000 0.000
64 00000001 | 222 5.00 -210.00 1399.25 0 0 0.000 0.000 | 10 15.00 765.00 1413.09 0 0 0.000 0.000
68 00000001 | 222 9.00 -210.00 1399.25 0 0 0.000 0.000 | 10 19.00 765.00 1413.09 0 0 0.000 0.000
72 00000001 | 222 13.00 -210.00 1399.25 0 0 0.000 0.000 | 10 23.00 765.00 1413.09 0 0 0.000 0.000
76 00000001 | 222 17.00 -210.00 1399.25 0 0 0.000 0.000 | 10 27.00 765.00 1413.09 0 0 0.000 0.000
80 00000001 | 222 21.00 -210.00 1399.25 0 0 0.000 0.000 | 10 31.00 765.00 1413.09 0 0 0.000 0.000 |  | 9 -46.10 1475.75 28.284 -28.284
84 00000001 | 222 25.00 -210.00 1399.25 0 0 0.000 0.000 | 10 35.00 765.00 1413.09 0 0 0.000 0.000 |  | 10 202.19 1398.61 55.000 -0.086
88 00000001 | 222 29.00 -210.00 1399.25 0 0 0.000 0.000 | 10 39.00 765.00 1413.09 0 0 0.000 0.000 |  | 10 422.19 1398.27 55.000 -0.086
90 6df109fd
91 aea69ed3
92 aea69ed3 | 222 33.00 -210.00 1399.25 0 0 0.000 0.000 | 49 1.00 765.00 1413.09 7 8 14.426 1.273
95 0f3e862e
96 0f3e862e | 222 37.00 -210.00 1399.25 0 0 0.000 0.000 | 49 1.00 765.00 1413.09 7 4 14.426 1.273
99 491c86cd
100 b9b3dbe0 | 222 41.00 -210.00 1399.25 0 0 0.000 0.000 | 43 1.00 777.93 1414.23 7 0 12.932 1.141
103 e6bb16d7
104 e6bb16d7 | 222 45.00 -210.00 1399.25 0 0 0.000 0.000 | 43 5.00 814.72 1417.47 7 0 6.955 0.614
108 fa5aea62 | 222 49.00 -210.00 1399.25 0 0 0.000 0.000 | 43 9.00 827.59 1418.61 7 0 0.978 0.086
112 fa5aea62 | 222 53.00 -210.00 1399.25 0 0 0.000 0.000 | 10 1.00 827.59 1418.61 7 0 0.000 0.000
116 fa5aea62 | 222 57.00 -210.00 1399.25 0 0 0.000 0.000 | 10 5.00 827.59 1418.61 7 0 0.000 0.000
117 9e1f7971
120 9e1f7971 | 222 61.00 -210.00 1399.25 0 0 0.000 0.000 | 10 9.00 827.59 1418.61 7 0 0.000 0.000
124 9e1f7971 | 10 1.00 -210.00 1399.25 0 0 0.000 0.000 | 10 13.00 827.59 1418.61 7 0 0.000 0.000
128 9e1f7971 | 10 5.00 -210.00 1399.25 0 0 0.000 0.000 | 10 17.00 827.59 1418.61 7 0 0.000 0.000
132 9e1f7971 | 10 9.00 -210.00 1399.25 0 0 0.000 0.000 | 10 21.00 827.59 1418.61 7 0 0.000 0.000
136 9e1f7971 | 10 13.00 -210.00 1399.25 0 0 0.000 0.000 | 10 25.00 827.59 1418.61 7 0 0.000 0.000
140 9e1f7971 | 10 17.00 -210.00 1399.25 0 0 0.000 0.000 | 10 29.00 827.59 1418.61 7 0 0.000 0.000
144 9e1f7971 | 10 21.00 -210.00 1399.25 0 0 0.000 0.000 | 10 33.00 827.59 1418.61 7 0 0.000 0.000
148 9e1f7971 | 10 25.00 -210.00 1399.25 0 0 0.000 0.000 | 10 37.00 827.59 1418.61 7 0 0.000 0.000
152 9e1f7971 | 10 29.00 -210.00 1399.25 0 0 0.000 0.000 | 10 41.00 827.59 1418.61 7 0 0.000 0.000
156 9e1f7971 | 10 33.00 -210.00 1399.25 0 0 0.000 0.000 | 10 45.00 827.59 1418.61 7 0 0.000 0.000
160 9e1f7971 | 10 37.00 -210.00 1399.25 0 0 0.000 0.000 | 10 49.00 827.59 1418.61 7 0 0.000 0.000
164 9e1f7971 | 10 41.00 -210.00 1399.25 0 0 0.000 0.000 | 10 3.00 827.59 1418.61 7 0 0.000 0.000
168 9e1f7971 | 10 45.00 -210.00 1399.25 0 0 0.000 0.000 | 10 7.00 827.59 1418.61 7 0 0.000 0.000
172 9e1f7971 | 10 49.00 -210.00 1399.25 0 0 0.000 0.000 | 10 11.00 827.59 1418.61 7 0 0.000 0.000
176 9e1f7971 | 10 53.00 -210.00 1399.25 0 0 0.000 0.000 | 10 15.00 827.59 1418.61 7 0 0.000 0.000
180 9e1f7971 | 10 57.00 -210.00 1399.25 0 0 0.000 0.000 | 10 19.00 827.59 1418.61 7 0 0.000 0.000
184 9e1f7971 | 10 61.00 -210.00 1399.25 0 0 0.000 0.000 | 10 23.00 827.59 1418.61 7 0 0.000 0.000
188 9e1f7971 | 10 65.00 -210.00 1399.25 0 0 0.000 0.000 | 10 27.00 827.59 1418.61 7 0 0.000 0.000
192 9e1f7971 | 10 69.00 -210.00 1399.25 0 0 0.000 0.000 | 10 31.00 827.59 1418.61 7 0 0.000 0.000
196 9e1f7971 | 10 73.00 -210.00 1399.25 0 0 0.000 0.000 | 10 35.00 827.59 1418.61 7 0 0.000 0.000
200 9e1f7971 | 10 77.00 -210.00 1399.25 0 0 0.000 0.000 | 10 39.00 827.59 1418.61 7 0 0.000 0.000
204 9e1f7971 | 10 81.00 -210.00 1399.25 0 0 0.000 0.000 | 10 43.00 827.59 1418.61 7 0 0.000 0.000
208 9e1f7971 | 10 85.00 -210.00 1399.25 0 0 0.000 0.000 | 10 47.00 827.59 1418.61 7 0 0.000 0.000
212 9e1f7971 | 10 89.00 -210.00 1399.25 0 0 0.000 0.000 | 10 1.00 827.59 1418.61 7 0 0.000 0.000
216 9e1f7971 | 10 93.00 -210.00 1399.25 0 0 0.000 0.000 | 10 5.00 827.59 1418.61 7 0 0.000 0.000
220 9e1f7971 | 10 97.00 -210.00 1399.25 0 0 0.000 0.000 | 10 9.00 827.59 1418.61 7 0 0.000 0.000
224 9e1f7971 | 10 101.00 -210.00 1399.25 0 0 0.000 0.000 | 10 13.00 827.59 1418.61 7 0 0.000 0.000
228 9e1f7971 | 10 105.00 -210.00 1399.25 0 0 0.000 0.000 | 10 17.00 827.59 1418.61 7 0 0.000 0.000
232 9e1f7971 | 10 109.00 -210.00 1399.25 0 0 0.000 0.000 | 10 21.00 827.59 1418.61 7 0 0.000 0.000
236 9e1f7971 | 10 113.00 -210.00 1399.25 0 0 0.000 0.000 | 10 25.00 827.59 1418.61 7 0 0.000 0.000
240 9e1f7971 | 10 117.00 -210.00 1399.25 0 0 0.000 0.000 | 10 29.00 827.59 1418.61 7 0 0.000 0.000
244 9e1f7971 | 10 121.00 -210.00 1399.25 0 0 0.000 0.000 | 10 33.00 827.59 1418.61 7 0 0.000 0.000
248 9e1f7971 | 10 125.00 -210.00 1399.25 0 0 0.000 0.000 | 10 37.00 827.59 1418.61 7 0 0.000 0.000
252 9e1f7971 | 10 129.00 -210.00 1399.25 0 0 0.000 0.000 | 10 41.00 827.59 1418.61 7 0 0.000 0.000
256 9e1f7971 | 10 133.00 -210.00 1399.25 0 0 0.000 0.000 | 10 45.00 827.59 1418.61 7 0 0.000 0.000
260 9e1f7971 | 10 137.00 -210.00 1399.25 0 0 0.000 0.000 | 10 49.00 827.59 1418.61 7 0 0.000 0.000
264 9e1f7971 | 10 141.00 -210.00 1399.25 0 0 0.000 0.000 | 10 3.00 827.59 1418.61 7 0 0.000 0.000
268 9e1f7971 | 10 145.00 -210.00 1399.25 0 0 0.000 0.000 | 10 7.00 827.59 1418.61 7 0 0.000 0.000
272 9e1f7971 | 10 149.00 -210.00 1399.25 0 0 0.000 0.000 | 10 11.00 827.59 1418.61 7 0 0.000 0.000
276 9e1f7971 | 10 153.00 -210.00 1399.25 0 0 0.000 0.000 | 10 15.00 827.59 1418.61 7 0 0.000 0.000
280 9e1f7971 | 10 157.00 -210.00 1399.25 0 0 0.000 0.000 | 10 19.00 827.59 1418.61 7 0 0.000 0.000
284 9e1f7971 | 10 161.00 -210.00 1399.25 0 0 0.000 0.000 | 10 23.00 827.59 1418.61 7 0 0.000 0.000
288 9e1f7971 | 10 165.00 -210.00 1399.25 0 0 0.000 0.000 | 10 27.00 827.59 1418.61 7 0 0.000 0.000
292 9e1f7971 | 10 169.00 -210.00 1399.25 0 0 0.000 0.000 | 10 31.00 827.59 1418.61 7 0 0.000 0.000
296 9e1f7971 | 10 173.00 -210.00 1399.25 0 0 0.000 0.000 | 10 35.00 827.59 1418.61 7 0 0.000 0.000
300 9e1f7971 | 10 177.00 -210.00 1399.25 0 0 0.000 0.000 | 10 39.00 827.59 1418.61 7 0 0.000 0.000
";

#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn a_thunder_jolt_hits_the_dummy_as_on_the_n64() {
    let replay = Replay {
        p1: FighterKind::Pikachu,
        route: "60:1:b",
        weapons: &fighter_weapons,
    };
    let Some(out) = replay.run(PIKACHU_JOLT_HITS_THE_DUMMY) else {
        return;
    };
    out.check("Thunder Jolt along the Castle floor into the dummy");
}

/// `honix` (`dITManagerForceMonsterKind` = 1 at 0x80189450, not the map's
/// 0x80189600): Link smash-throws a Poké Ball at the dummy; Onix comes out
/// where it lands (t 403) and drops his rocks from 544, clamped to the
/// stage. Rows end at 629: the Training CPU's `DownAttackD` at 630 is not
/// modelled.
const ONIX_ROCKS_FALL_ON_THE_CASTLE: &str = "
210 69521988
211 a6e9c9a6
212 fe87df14
213 c40df552
327 491c86cd
332 4db793af
333 074d495a
396 074d495a | 10 53.00 -210.00 1399.25 0 0 0.000 0.000 | 57 5.00 2369.91 1575.09 20 0 0.000 0.000 | 19 385.58 1507.32 0.000 0.000
402 074d495a | 10 59.00 -210.00 1399.25 0 0 0.000 0.000 | 57 11.00 2369.91 1311.09 20 0 0.000 0.000 | 19 385.58 1507.32 0.000 0.000
408 074d495a | 10 65.00 -210.00 1399.25 0 0 0.000 0.000 | 57 17.00 2369.91 1047.09 20 0 0.000 0.000 | 32 385.58 1927.32 0.000 16.000
414 074d495a | 10 71.00 -210.00 1399.25 0 0 0.000 0.000 | 57 23.00 2369.91 783.09 20 0 0.000 0.000 | 32 385.58 2023.32 0.000 16.000
420 074d495a | 10 77.00 -210.00 1399.25 0 0 0.000 0.000 | 57 0.00 2369.91 519.09 20 0 0.000 0.000 | 32 385.58 2119.32 0.000 16.000
426 074d495a | 10 3.00 -210.00 1399.25 0 0 0.000 0.000 | 57 6.00 2369.91 255.09 20 0 0.000 0.000 | 32 385.58 2183.32 0.000 0.000
431 4dc8aab5
432 4dc8aab5 | 10 9.00 -210.00 1399.25 0 0 0.000 0.000 | 57 12.00 2369.91 -8.91 20 0 0.000 0.000 | 32 385.58 2183.32 0.000 0.000
438 4dc8aab5 | 10 15.00 -210.00 1399.25 0 0 0.000 0.000 | 67 3.00 2359.10 -101.00 20 0 0.000 0.000 | 32 385.58 2183.32 0.000 0.000
444 4dc8aab5 | 10 21.00 -210.00 1399.25 0 0 0.000 0.000 | 67 9.00 2337.48 -101.00 20 0 0.000 0.000 | 32 385.58 2183.32 0.000 0.000
450 4dc8aab5 | 10 27.00 -210.00 1399.25 0 0 0.000 0.000 | 67 15.00 2315.86 -101.00 20 0 0.000 0.000 | 32 385.58 2183.32 0.000 0.000
455 6443b2a4
456 6443b2a4 | 10 33.00 -210.00 1399.25 0 0 0.000 0.000 | 67 21.00 2294.23 -101.00 20 0 0.000 0.000 | 32 385.58 2343.32 0.000 80.000
462 6443b2a4 | 10 39.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2272.61 -101.00 20 0 0.000 0.000 | 32 385.58 2823.32 0.000 80.000
468 6443b2a4 | 10 45.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2250.99 -101.00 20 0 0.000 0.000 | 32 385.58 3303.32 0.000 80.000
474 6443b2a4 | 10 51.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2229.37 -101.00 20 0 0.000 0.000 | 32 385.58 3783.32 0.000 80.000
480 6443b2a4 | 10 57.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2207.75 -101.00 20 0 0.000 0.000 | 32 385.58 4263.32 0.000 80.000
486 6443b2a4 | 10 63.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2186.13 -101.00 20 0 0.000 0.000 | 32 385.58 4743.32 0.000 80.000
492 6443b2a4 | 10 69.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2164.51 -101.00 20 0 0.000 0.000 | 32 385.58 5223.32 0.000 80.000
498 6443b2a4 | 10 75.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2142.89 -101.00 20 0 0.000 0.000 | 32 385.58 5703.32 0.000 80.000
504 6443b2a4 | 10 1.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2121.27 -101.00 20 0 0.000 0.000 | 32 385.58 6183.32 0.000 80.000
510 6443b2a4 | 10 7.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2099.65 -101.00 20 0 0.000 0.000 | 32 385.58 6663.32 0.000 80.000
516 6443b2a4 | 10 13.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2078.02 -101.00 20 0 0.000 0.000 | 32 385.58 7143.32 0.000 80.000
522 6443b2a4 | 10 19.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2056.40 -101.00 20 0 0.000 0.000 | 32 385.58 7623.32 0.000 80.000
528 6443b2a4 | 10 25.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2034.78 -101.00 20 0 0.000 0.000 | 32 385.58 8103.32 0.000 80.000
534 6443b2a4 | 10 31.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2013.16 -101.00 20 0 0.000 0.000 | 32 385.58 8583.32 0.000 80.000
540 6443b2a4 | 10 37.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1991.54 -101.00 20 0 0.000 0.000 | 32 385.58 9063.32 0.000 80.000
544 701fe6a8
546 701fe6a8 | 10 43.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1969.92 -101.00 20 0 0.000 0.000 | 32 385.58 9300.00 0.000 0.000 | 23 1188.16 8988.00 0.000 -106.000
552 701fe6a8 | 10 49.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1948.29 -101.00 20 0 0.000 0.000 | 32 385.58 9300.00 0.000 0.000 | 23 1188.16 8310.00 0.000 -118.000
558 701fe6a8 | 10 55.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1926.67 -101.00 20 0 0.000 0.000 | 32 385.58 9300.00 0.000 0.000 | 23 1188.16 7560.00 0.000 -130.000
564 701fe6a8 | 10 61.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1905.05 -101.00 20 0 0.000 0.000 | 32 385.58 9300.00 0.000 0.000 | 23 1188.16 6738.00 0.000 -142.000
570 701fe6a8 | 10 67.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1883.43 -101.00 20 0 0.000 0.000 | 32 385.58 9300.00 0.000 0.000 | 23 1188.16 5844.00 0.000 -154.000
572 45099bec
576 45099bec | 10 73.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1861.81 -101.00 20 0 0.000 0.000 | 32 385.58 9300.00 0.000 0.000 | 23 1188.16 4878.00 0.000 -166.000 ; 23 1112.23 9270.00 0.000 -10.000
582 45099bec | 10 79.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1840.19 -101.00 20 0 0.000 0.000 | 32 385.58 9300.00 0.000 0.000 | 23 1188.16 3840.00 0.000 -178.000 ; 23 1112.23 9168.00 0.000 -22.000
588 45099bec | 10 5.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1818.57 -101.00 20 0 0.000 0.000 | 32 385.58 9300.00 0.000 0.000 | 23 1188.16 2730.00 0.000 -190.000 ; 23 1112.23 8994.00 0.000 -34.000
594 bb25e9c6 | 10 11.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1796.94 -101.00 20 0 0.000 0.000 | 32 385.58 9300.00 0.000 0.000 | 23 1188.16 1550.00 4.642 19.454 ; 23 1112.23 8748.00 0.000 -46.000
595 e3cbf86a
600 e3cbf86a | 10 17.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1775.32 -101.00 20 0 0.000 0.000 | 32 385.58 9300.00 0.000 0.000 | 23 1216.01 1624.72 4.642 7.454 ; 23 1112.23 8430.00 0.000 -58.000 ; 23 620.90 9258.00 0.000 -12.000
606 e3cbf86a | 10 23.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1753.70 -101.00 20 0 0.000 0.000 | 32 385.58 9300.00 0.000 0.000 | 23 1243.86 1627.45 4.642 -4.546 ; 23 1112.23 8040.00 0.000 -70.000 ; 23 620.90 9144.00 0.000 -24.000
612 e3cbf86a | 10 29.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1732.08 -101.00 20 0 0.000 0.000 | 32 385.58 9300.00 0.000 0.000 | 23 1271.71 1558.17 4.642 -16.546 ; 23 1112.23 7578.00 0.000 -82.000 ; 23 620.90 8958.00 0.000 -36.000
613 6bfcdd85
618 6bfcdd85 | 10 35.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1710.46 -101.00 20 0 0.000 0.000 | 32 385.58 9300.00 0.000 0.000 | 23 1299.56 1416.90 4.642 -28.546 ; 23 1112.23 7044.00 0.000 -94.000 ; 23 620.90 8700.00 0.000 -48.000
624 6bfcdd85 | 10 41.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1688.84 -101.00 20 0 0.000 0.000 | 32 385.58 9300.00 0.000 0.000 | 23 1327.41 1203.62 4.642 -40.546 ; 23 1112.23 6438.00 0.000 -106.000 ; 23 620.90 8370.00 0.000 -60.000
629 6bfcdd85 | 10 46.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1670.82 -101.00 20 0 0.000 0.000 | 32 385.58 9300.00 0.000 0.000 | 23 1350.61 970.89 4.642 -50.546 ; 23 1112.23 5878.00 0.000 -116.000 ; 23 620.90 8040.00 0.000 -70.000
";

#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn onix_rocks_fall_as_on_the_n64() {
    let replay = Replay {
        p1: FighterKind::Link,
        route: "30:1:start;40:1:y=-80;50:1:x=80;60:1:x=80;70:1:x=80;80:1:x=80;\
                90:1:x=80;100:1:x=80;110:1:x=80;120:1:x=80;130:1:x=80;140:1:x=80;\
                150:1:x=80;160:1:x=80;170:1:x=80;180:1:x=80;190:1:x=80;200:1:x=80;\
                210:1:a;220:1:start;270:1:a;320:1:z+a+x=127",
        weapons: &monster_weapons,
    };
    let Some(out) = replay.run_forced(ONIX_ROCKS_FALL_ON_THE_CASTLE, 1) else {
        return;
    };
    out.check("Onix from a forced Poké Ball and his rocks");
}

/// `honix`'s route for the forced Poké Ball rows below: Training's menu
/// makes a Poké Ball, Link picks it up and smash-throws it; it opens at
/// 403 and the Pokémon comes out at 405 where it lands.
const FORCED_BALL_ROUTE: &str = "30:1:start;40:1:y=-80;50:1:x=80;60:1:x=80;70:1:x=80;80:1:x=80;\
    90:1:x=80;100:1:x=80;110:1:x=80;120:1:x=80;130:1:x=80;140:1:x=80;\
    150:1:x=80;160:1:x=80;170:1:x=80;180:1:x=80;190:1:x=80;200:1:x=80;\
    210:1:a;220:1:start;270:1:a;320:1:z+a+x=127";

/// Replays a forced Poké Ball (`dITManagerForceMonsterKind` = `monster`,
/// `itMBallOpenProcUpdate`'s forced branch) with Training's CPU dummy.
fn forced_ball(table: &str, monster: u8, what: &str) {
    let replay = Replay {
        p1: FighterKind::Link,
        route: FORCED_BALL_ROUTE,
        weapons: &monster_weapons,
    };
    let Some(out) = replay.run_forced_cpu(table, monster) else {
        return;
    };
    out.check(what);
}

/// `b36poke2` (`warp_link`, [`FORCED_BALL_ROUTE`],
/// `dITManagerForceMonsterKind` = 2 poked at frame 2): `nITKindKabigon`.
/// Snorlax rises off-screen, falls at x 1352 and crushes the dummy at 603 (20 to 31 percent, 8 frames of hitlag, `DamageFlyN`); the Training CPU jumps back at 668 (`func_ovl3_801346D4`'s cliff aim), falls short, is KO'd off the bottom and reborn at 870. Rows 396-1100: every 6th frame, the seed's changes, and
/// every frame round a hit; the dummy is Training's CPU (its getup at
/// 630).
const FORCED_SNORLAX: &str = "
210 69521988
211 a6e9c9a6
212 fe87df14
213 c40df552
327 491c86cd
332 4db793af
333 074d495a
396 074d495a | 10 53.00 -210.00 1399.25 0 0 0.000 0.000 | 57 5.00 2369.91 1575.09 20 0 0.000 0.000 | 19 385.58 1507.32 0.000 0.000
402 074d495a | 10 59.00 -210.00 1399.25 0 0 0.000 0.000 | 57 11.00 2369.91 1311.09 20 0 0.000 0.000 | 19 385.58 1507.32 0.000 0.000
408 074d495a | 10 65.00 -210.00 1399.25 0 0 0.000 0.000 | 57 17.00 2369.91 1047.09 20 0 0.000 0.000 | 33 385.58 2079.32 0.000 16.000
414 074d495a | 10 71.00 -210.00 1399.25 0 0 0.000 0.000 | 57 23.00 2369.91 783.09 20 0 0.000 0.000 | 33 385.58 2175.32 0.000 16.000
420 074d495a | 10 77.00 -210.00 1399.25 0 0 0.000 0.000 | 57 0.00 2369.91 519.09 20 0 0.000 0.000 | 33 385.58 2271.32 0.000 16.000
426 074d495a | 10 3.00 -210.00 1399.25 0 0 0.000 0.000 | 57 6.00 2369.91 255.09 20 0 0.000 0.000 | 33 385.58 2495.32 0.000 80.000
430 6443b2a4
431 e6bb16d7
432 e6bb16d7 | 10 9.00 -210.00 1399.25 0 0 0.000 0.000 | 57 12.00 2369.91 -8.91 20 0 0.000 0.000 | 33 385.58 2975.32 0.000 80.000
434 a6a6b409
438 dd038acb | 10 15.00 -210.00 1399.25 0 0 0.000 0.000 | 67 3.00 2359.10 -101.00 20 0 0.000 0.000 | 33 385.58 3455.32 0.000 80.000
442 f67de79d
444 f67de79d | 10 21.00 -210.00 1399.25 0 0 0.000 0.000 | 67 9.00 2337.48 -101.00 20 0 0.000 0.000 | 33 385.58 3935.32 0.000 80.000
446 92387aff
450 9e1f7971 | 10 27.00 -210.00 1399.25 0 0 0.000 0.000 | 67 15.00 2315.86 -101.00 20 0 0.000 0.000 | 33 385.58 4415.32 0.000 80.000
454 9ad4bb73
456 9ad4bb73 | 10 33.00 -210.00 1399.25 0 0 0.000 0.000 | 67 21.00 2294.23 -101.00 20 0 0.000 0.000 | 33 385.58 4895.32 0.000 80.000
458 6bfcdd85
462 7ff56027 | 10 39.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2272.61 -101.00 20 0 0.000 0.000 | 33 385.58 5375.32 0.000 80.000
466 b23bc7d9
468 b23bc7d9 | 10 45.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2250.99 -101.00 20 0 0.000 0.000 | 33 385.58 5855.32 0.000 80.000
470 a60dbd1b
474 830a2c6d | 10 51.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2229.37 -101.00 20 0 0.000 0.000 | 33 385.58 6335.32 0.000 80.000
478 8bdb664f
480 8bdb664f | 10 57.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2207.75 -101.00 20 0 0.000 0.000 | 33 385.58 6815.32 0.000 80.000
482 07323f41
486 f59a2fc3 | 10 63.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2186.13 -101.00 20 0 0.000 0.000 | 33 385.58 7295.32 0.000 80.000
490 22ee7455
492 22ee7455 | 10 69.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2164.51 -101.00 20 0 0.000 0.000 | 33 385.58 7775.32 0.000 80.000
494 d8782d77
498 dcfd7fa9 | 10 75.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2142.89 -101.00 20 0 0.000 0.000 | 33 385.58 8255.32 0.000 80.000
502 1a49b36b
504 1a49b36b | 10 1.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2121.27 -101.00 20 0 0.000 0.000 | 33 385.58 8735.32 0.000 80.000
506 3bf6553d
510 797d559f | 10 7.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2099.65 -101.00 20 0 0.000 0.000 | 33 385.58 9215.32 0.000 80.000
514 8ddc2911
516 8ddc2911 | 10 13.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2078.02 -101.00 20 0 0.000 0.000 | 33 385.58 9375.32 0.000 0.000
518 314fe813
522 4df26f25 | 10 19.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2056.40 -101.00 20 0 0.000 0.000 | 33 385.58 9375.32 0.000 0.000
526 2e407ec7
528 2e407ec7 | 10 25.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2034.78 -101.00 20 0 0.000 0.000 | 33 385.58 9375.32 0.000 0.000
530 1cd0db79
534 e6c46dbb | 10 31.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2013.16 -101.00 20 0 0.000 0.000 | 33 385.58 9375.32 0.000 0.000
538 7eb7620d
540 7eb7620d | 10 37.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1991.54 -101.00 20 0 0.000 0.000 | 33 385.58 9375.32 0.000 0.000
542 2c3b48ef
546 542236e1 | 10 43.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1969.92 -101.00 20 0 0.000 0.000 | 33 385.58 9375.32 0.000 0.000
550 8822e463
552 8822e463 | 10 49.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1948.29 -101.00 20 0 0.000 0.000 | 33 385.58 9375.32 0.000 0.000
554 409dcdf5
558 f98b5417 | 10 55.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1926.67 -101.00 20 0 0.000 0.000 | 33 385.58 9375.32 0.000 0.000
562 73dadb49
564 73dadb49 | 10 61.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1905.05 -101.00 20 0 0.000 0.000 | 33 385.58 9375.32 0.000 0.000
566 26caec0b
570 890252dd | 10 67.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1883.43 -101.00 20 0 0.000 0.000 | 33 385.58 9375.32 0.000 0.000
571 fbb95a2c
576 fbb95a2c | 10 73.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1861.81 -101.00 20 0 0.000 0.000 | 33 1352.16 8055.32 0.000 -220.000
582 fbb95a2c | 10 79.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1840.19 -101.00 20 0 0.000 0.000 | 33 1352.16 6735.32 0.000 -220.000
588 fbb95a2c | 10 5.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1818.57 -101.00 20 0 0.000 0.000 | 33 1352.16 5415.32 0.000 -220.000
594 fbb95a2c | 10 11.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1796.94 -101.00 20 0 0.000 0.000 | 33 1352.16 4095.32 0.000 -220.000
599 fbb95a2c | 10 16.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1778.93 -101.00 20 0 0.000 0.000 | 33 1352.16 2995.32 0.000 -220.000
600 fbb95a2c | 10 17.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1775.32 -101.00 20 0 0.000 0.000 | 33 1352.16 2775.32 0.000 -220.000
601 fbb95a2c | 10 18.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1771.72 -101.00 20 0 0.000 0.000 | 33 1352.16 2555.32 0.000 -220.000
602 fbb95a2c | 10 19.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1768.11 -101.00 20 0 0.000 0.000 | 33 1352.16 2335.32 0.000 -220.000
603 139d9a06 | 10 20.00 -210.00 1399.25 0 0 0.000 0.000 | 52 1.00 1764.51 -101.00 31 8 80.405 73.677 | 33 1352.16 2115.32 0.000 -220.000
604 96c590c5 | 10 21.00 -210.00 1399.25 0 0 0.000 0.000 | 52 1.00 1764.51 -101.00 31 7 80.405 73.677 | 33 1352.16 1895.32 0.000 -220.000
605 96c590c5 | 10 22.00 -210.00 1399.25 0 0 0.000 0.000 | 52 1.00 1764.51 -101.00 31 6 80.405 73.677 | 33 1352.16 1675.32 0.000 -220.000
606 96c590c5 | 10 23.00 -210.00 1399.25 0 0 0.000 0.000 | 52 1.00 1764.51 -101.00 31 5 80.405 73.677 | 33 1352.16 1455.32 0.000 -220.000
607 96c590c5 | 10 24.00 -210.00 1399.25 0 0 0.000 0.000 | 52 1.00 1764.51 -101.00 31 4 80.405 73.677 | 33 1352.16 1235.32 0.000 -220.000
608 96c590c5 | 10 25.00 -210.00 1399.25 0 0 0.000 0.000 | 52 1.00 1764.51 -101.00 31 3 80.405 73.677 | 33 1352.16 1015.32 0.000 -220.000
609 96c590c5 | 10 26.00 -210.00 1399.25 0 0 0.000 0.000 | 52 1.00 1764.51 -101.00 31 2 80.405 73.677 | 33 1352.16 795.32 0.000 -220.000
610 96c590c5 | 10 27.00 -210.00 1399.25 0 0 0.000 0.000 | 52 1.00 1764.51 -101.00 31 1 80.405 73.677 | 33 1352.16 575.32 0.000 -220.000
611 96c590c5 | 10 28.00 -210.00 1399.25 0 0 0.000 0.000 | 52 2.00 1843.66 -30.87 31 0 79.151 72.529 | 33 1352.16 355.32 0.000 -220.000
612 96c590c5 | 10 29.00 -210.00 1399.25 0 0 0.000 0.000 | 52 3.00 1921.56 35.71 31 0 77.898 71.380 | 33 1352.16 135.32 0.000 -220.000
613 96c590c5 | 10 30.00 -210.00 1399.25 0 0 0.000 0.000 | 52 4.00 1998.20 98.74 31 0 76.644 70.232 | 33 1352.16 -84.68 0.000 -220.000
618 96c590c5 | 10 35.00 -210.00 1399.25 0 0 0.000 0.000 | 52 9.00 2362.63 360.67 31 0 70.378 64.489 | 33 1352.16 -1184.68 0.000 -220.000
624 96c590c5 | 10 41.00 -210.00 1399.25 0 0 0.000 0.000 | 52 15.00 2758.57 557.89 31 0 62.857 57.598 | 33 1352.16 -2504.68 0.000 -220.000
630 96c590c5 | 10 47.00 -210.00 1399.25 0 0 0.000 0.000 | 52 21.00 3109.39 632.96 31 0 55.337 50.707 | 33 1352.16 -3824.68 0.000 -220.000
636 96c590c5 | 10 53.00 -210.00 1399.25 0 0 0.000 0.000 | 52 27.00 3415.09 649.08 31 0 47.817 43.816
642 96c590c5 | 10 59.00 -210.00 1399.25 0 0 0.000 0.000 | 52 0.00 3675.68 623.86 31 0 40.297 36.925
648 96c590c5 | 10 65.00 -210.00 1399.25 0 0 0.000 0.000 | 52 0.00 3891.13 557.29 31 0 32.776 30.034
654 96c590c5 | 10 71.00 -210.00 1399.25 0 0 0.000 0.000 | 52 0.00 4061.47 449.38 31 0 25.256 23.143
660 96c590c5 | 10 77.00 -210.00 1399.25 0 0 0.000 0.000 | 52 0.00 4186.69 300.12 31 0 17.736 16.252
661 96c590c5 | 10 78.00 -210.00 1399.25 0 0 0.000 0.000 | 52 0.00 4203.17 271.22 31 0 16.483 15.103
662 96c590c5 | 10 79.00 -210.00 1399.25 0 0 0.000 0.000 | 52 0.00 4218.40 241.17 31 0 15.229 13.955
663 96c590c5 | 10 0.00 -210.00 1399.25 0 0 0.000 0.000 | 52 0.00 4232.37 209.98 31 0 13.976 12.806
664 96c590c5 | 10 1.00 -210.00 1399.25 0 0 0.000 0.000 | 52 0.00 4245.10 177.64 31 0 12.722 11.658
665 96c590c5 | 10 2.00 -210.00 1399.25 0 0 0.000 0.000 | 52 0.00 4256.57 144.15 31 0 11.469 10.509
666 96c590c5 | 10 3.00 -210.00 1399.25 0 0 0.000 0.000 | 52 0.00 4266.78 109.51 31 0 10.216 9.361
667 96c590c5 | 10 4.00 -210.00 1399.25 0 0 0.000 0.000 | 52 0.00 4275.74 73.72 31 0 8.962 8.212
668 5db2c578 | 10 5.00 -210.00 1399.25 0 0 0.000 0.000 | 24 0.00 4253.65 152.19 31 0 7.709 7.064
669 5db2c578 | 10 6.00 -210.00 1399.25 0 0 0.000 0.000 | 24 1.00 4230.31 227.10 31 0 6.456 5.915
670 5db2c578 | 10 7.00 -210.00 1399.25 0 0 0.000 0.000 | 24 2.00 4205.71 298.47 31 0 5.202 4.767
671 5db2c578 | 10 8.00 -210.00 1399.25 0 0 0.000 0.000 | 24 3.00 4179.86 366.29 31 0 3.949 3.618
672 5db2c578 | 10 9.00 -210.00 1399.25 0 0 0.000 0.000 | 24 4.00 4152.76 430.56 31 0 2.695 2.470
673 5db2c578 | 10 10.00 -210.00 1399.25 0 0 0.000 0.000 | 24 5.00 4124.40 491.28 31 0 1.442 1.321
674 e0bf3b8f | 10 11.00 -210.00 1399.25 0 0 0.000 0.000 | 24 6.00 4094.79 548.45 31 0 0.189 0.173
675 e0bf3b8f | 10 12.00 -210.00 1399.25 0 0 0.000 0.000 | 24 7.00 4064.99 603.05 31 0 0.000 0.000
676 e0bf3b8f | 10 13.00 -210.00 1399.25 0 0 0.000 0.000 | 24 8.00 4035.19 655.25 31 0 0.000 0.000
677 e0bf3b8f | 10 14.00 -210.00 1399.25 0 0 0.000 0.000 | 24 9.00 4005.39 705.05 31 0 0.000 0.000
678 e0bf3b8f | 10 15.00 -210.00 1399.25 0 0 0.000 0.000 | 24 10.00 3975.59 752.45 31 0 0.000 0.000
679 e0bf3b8f | 10 16.00 -210.00 1399.25 0 0 0.000 0.000 | 24 11.00 3945.79 797.45 31 0 0.000 0.000
680 e0bf3b8f | 10 17.00 -210.00 1399.25 0 0 0.000 0.000 | 24 12.00 3915.99 840.05 31 0 0.000 0.000
681 e0bf3b8f | 10 18.00 -210.00 1399.25 0 0 0.000 0.000 | 24 13.00 3886.19 880.25 31 0 0.000 0.000
682 e0bf3b8f | 10 19.00 -210.00 1399.25 0 0 0.000 0.000 | 24 14.00 3856.39 918.05 31 0 0.000 0.000
683 e0bf3b8f | 10 20.00 -210.00 1399.25 0 0 0.000 0.000 | 24 15.00 3826.59 953.45 31 0 0.000 0.000
684 e0bf3b8f | 10 21.00 -210.00 1399.25 0 0 0.000 0.000 | 24 16.00 3796.79 986.45 31 0 0.000 0.000
685 e0bf3b8f | 10 22.00 -210.00 1399.25 0 0 0.000 0.000 | 24 17.00 3766.99 1017.05 31 0 0.000 0.000
686 e0bf3b8f | 10 23.00 -210.00 1399.25 0 0 0.000 0.000 | 24 18.00 3737.19 1045.25 31 0 0.000 0.000
687 e0bf3b8f | 10 24.00 -210.00 1399.25 0 0 0.000 0.000 | 24 19.00 3707.39 1071.05 31 0 0.000 0.000
688 e0bf3b8f | 10 25.00 -210.00 1399.25 0 0 0.000 0.000 | 24 20.00 3677.59 1094.45 31 0 0.000 0.000
689 e0bf3b8f | 10 26.00 -210.00 1399.25 0 0 0.000 0.000 | 24 21.00 3647.79 1115.45 31 0 0.000 0.000
690 e0bf3b8f | 10 27.00 -210.00 1399.25 0 0 0.000 0.000 | 24 22.00 3617.99 1134.05 31 0 0.000 0.000
691 e0bf3b8f | 10 28.00 -210.00 1399.25 0 0 0.000 0.000 | 24 23.00 3588.19 1150.25 31 0 0.000 0.000
692 e0bf3b8f | 10 29.00 -210.00 1399.25 0 0 0.000 0.000 | 24 24.00 3558.39 1164.05 31 0 0.000 0.000
693 e0bf3b8f | 10 30.00 -210.00 1399.25 0 0 0.000 0.000 | 24 25.00 3528.59 1175.45 31 0 0.000 0.000
694 e0bf3b8f | 10 31.00 -210.00 1399.25 0 0 0.000 0.000 | 24 26.00 3498.79 1184.45 31 0 0.000 0.000
695 e0bf3b8f | 10 32.00 -210.00 1399.25 0 0 0.000 0.000 | 24 27.00 3468.99 1191.05 31 0 0.000 0.000
696 e0bf3b8f | 10 33.00 -210.00 1399.25 0 0 0.000 0.000 | 24 28.00 3439.19 1195.25 31 0 0.000 0.000
697 e0bf3b8f | 10 34.00 -210.00 1399.25 0 0 0.000 0.000 | 24 29.00 3409.39 1197.05 31 0 0.000 0.000
698 e0bf3b8f | 10 35.00 -210.00 1399.25 0 0 0.000 0.000 | 24 30.00 3379.59 1196.45 31 0 0.000 0.000
699 e0bf3b8f | 10 36.00 -210.00 1399.25 0 0 0.000 0.000 | 24 31.00 3349.79 1193.45 31 0 0.000 0.000
700 e0bf3b8f | 10 37.00 -210.00 1399.25 0 0 0.000 0.000 | 24 32.00 3319.99 1188.05 31 0 0.000 0.000
702 dc67e816 | 10 39.00 -210.00 1399.25 0 0 0.000 0.000 | 24 34.00 3260.39 1170.05 31 0 0.000 0.000
708 dc67e816 | 10 45.00 -210.00 1399.25 0 0 0.000 0.000 | 24 40.00 3081.59 1058.45 31 0 0.000 0.000
714 dc67e816 | 10 51.00 -210.00 1399.25 0 0 0.000 0.000 | 24 46.00 2902.79 860.45 31 0 0.000 0.000
720 dc67e816 | 10 57.00 -210.00 1399.25 0 0 0.000 0.000 | 24 52.00 2723.98 599.25 31 0 0.000 0.000
726 dc67e816 | 10 63.00 -210.00 1399.25 0 0 0.000 0.000 | 24 58.00 2545.18 335.25 31 0 0.000 0.000
732 dc67e816 | 10 69.00 -210.00 1399.25 0 0 0.000 0.000 | 27 4.00 2366.38 71.25 31 0 0.000 0.000
738 dc67e816 | 10 75.00 -210.00 1399.25 0 0 0.000 0.000 | 27 0.00 2187.58 -192.75 31 0 0.000 0.000
744 dc67e816 | 10 1.00 -210.00 1399.25 0 0 0.000 0.000 | 27 6.00 2008.78 -456.75 31 0 0.000 0.000
750 dc67e816 | 10 7.00 -210.00 1399.25 0 0 0.000 0.000 | 27 2.00 1829.98 -720.75 31 0 0.000 0.000
756 dc67e816 | 10 13.00 -210.00 1399.25 0 0 0.000 0.000 | 27 8.00 1651.18 -984.75 31 0 0.000 0.000
762 dc67e816 | 10 19.00 -210.00 1399.25 0 0 0.000 0.000 | 27 4.00 1472.38 -1248.75 31 0 0.000 0.000
768 dc67e816 | 10 25.00 -210.00 1399.25 0 0 0.000 0.000 | 27 0.00 1293.58 -1512.75 31 0 0.000 0.000
774 dc67e816 | 10 31.00 -210.00 1399.25 0 0 0.000 0.000 | 27 6.00 1114.78 -1776.75 31 0 0.000 0.000
780 dc67e816 | 10 37.00 -210.00 1399.25 0 0 0.000 0.000 | 27 2.00 935.98 -2040.75 31 0 0.000 0.000
786 dc67e816 | 10 43.00 -210.00 1399.25 0 0 0.000 0.000 | 27 8.00 757.18 -2304.75 31 0 0.000 0.000
792 dc67e816 | 10 49.00 -210.00 1399.25 0 0 0.000 0.000 | 27 4.00 578.58 -2568.75 31 0 0.000 0.000
798 dc67e816 | 10 55.00 -210.00 1399.25 0 0 0.000 0.000 | 27 0.00 405.18 -2832.75 31 0 0.000 0.000
804 dc67e816 | 10 61.00 -210.00 1399.25 0 0 0.000 0.000 | 27 6.00 242.98 -3096.75 31 0 0.000 0.000
810 dc67e816 | 10 67.00 -210.00 1399.25 0 0 0.000 0.000 | 27 2.00 104.11 -3360.75 31 0 0.000 0.000
816 dc67e816 | 10 73.00 -210.00 1399.25 0 0 0.000 0.000 | 27 8.00 -2.27 -3624.75 31 0 0.000 0.000
822 dc67e816 | 10 79.00 -210.00 1399.25 0 0 0.000 0.000 | 27 4.00 -69.22 -3888.75 31 0 0.000 0.000
825 6ad66fba
826 f9832e08
827 75ef34ab
828 22cd7153 | 10 5.00 -210.00 1399.25 0 0 0.000 0.000 | 0 0.00 -86.44 -4020.75 31 0 0.000 0.000
829 c89cec07
830 ddd597f1
831 5a9fcdf3
832 ccd4c0ea
833 0e90a856
834 3a2d7dc1 | 10 11.00 -210.00 1399.25 0 0 0.000 0.000 | 0 6.00 -86.44 -4020.75 31 0 0.000 0.000
835 f65f3a93
836 2b0055a5
837 8c7b64d8
838 0c15da12
839 f73b891c
840 d13e6e8b | 10 17.00 -210.00 1399.25 0 0 0.000 0.000 | 0 2.00 -86.44 -4020.75 31 0 0.000 0.000
841 c86aba86
842 f3d9b745
843 182f07e7
844 5968ad99
845 85881569
846 d5794288 | 10 23.00 -210.00 1399.25 0 0 0.000 0.000 | 0 8.00 -86.44 -4020.75 31 0 0.000 0.000
847 8de58aa6
848 ef3c16d1
849 a9d8a8e5
851 dccd8bd7
852 4325149d | 10 29.00 -210.00 1399.25 0 0 0.000 0.000 | 0 4.00 -86.44 -4020.75 31 0 0.000 0.000
853 89402ac6
854 f014b671
855 1d117334
857 54d6d438
858 54d6d438 | 10 35.00 -210.00 1399.25 0 0 0.000 0.000 | 0 0.00 -86.44 -4020.75 31 0 0.000 0.000
859 658c3b4f
860 c12fe8d6
863 b2950155
864 b2950155 | 10 41.00 -210.00 1399.25 0 0 0.000 0.000 | 0 6.00 -86.44 -4020.75 31 0 0.000 0.000
867 ee7e2cc8
870 ee7e2cc8 | 10 47.00 -210.00 1399.25 0 0 0.000 0.000 | 7 -74.00 0.00 9500.00 0 0 0.000 0.000
871 64866a9f
875 969a038a
876 969a038a | 10 53.00 -210.00 1399.25 0 0 0.000 0.000 | 7 -74.00 0.00 8654.49 0 0 0.000 0.000
882 969a038a | 10 59.00 -210.00 1399.25 0 0 0.000 0.000 | 7 -74.00 0.00 7867.29 0 0 0.000 0.000
888 969a038a | 10 65.00 -210.00 1399.25 0 0 0.000 0.000 | 7 -74.00 0.00 7138.40 0 0 0.000 0.000
894 969a038a | 10 71.00 -210.00 1399.25 0 0 0.000 0.000 | 7 -74.00 0.00 6467.82 0 0 0.000 0.000
900 969a038a | 10 77.00 -210.00 1399.25 0 0 0.000 0.000 | 7 -74.00 0.00 5855.56 0 0 0.000 0.000
906 969a038a | 10 3.00 -210.00 1399.25 0 0 0.000 0.000 | 7 -74.00 0.00 5301.60 0 0 0.000 0.000
912 969a038a | 10 9.00 -210.00 1399.25 0 0 0.000 0.000 | 7 -74.00 0.00 4805.96 0 0 0.000 0.000
918 969a038a | 10 15.00 -210.00 1399.25 0 0 0.000 0.000 | 7 -74.00 0.00 4368.62 0 0 0.000 0.000
924 969a038a | 10 21.00 -210.00 1399.25 0 0 0.000 0.000 | 7 -74.00 0.00 3989.60 0 0 0.000 0.000
930 969a038a | 10 27.00 -210.00 1399.25 0 0 0.000 0.000 | 7 -74.00 0.00 3668.89 0 0 0.000 0.000
936 969a038a | 10 33.00 -210.00 1399.25 0 0 0.000 0.000 | 7 -74.00 0.00 3406.49 0 0 0.000 0.000
942 969a038a | 10 39.00 -210.00 1399.25 0 0 0.000 0.000 | 7 -74.00 0.00 3202.40 0 0 0.000 0.000
948 969a038a | 10 45.00 -210.00 1399.25 0 0 0.000 0.000 | 8 4.00 0.00 3056.62 0 0 0.000 0.000
954 969a038a | 10 51.00 -210.00 1399.25 0 0 0.000 0.000 | 8 10.00 0.00 2969.16 0 0 0.000 0.000
960 969a038a | 10 57.00 -210.00 1399.25 0 0 0.000 0.000 | 8 16.00 0.00 2940.00 0 0 0.000 0.000
966 969a038a | 10 63.00 -210.00 1399.25 0 0 0.000 0.000 | 8 22.00 0.00 2940.00 0 0 0.000 0.000
972 969a038a | 10 69.00 -210.00 1399.25 0 0 0.000 0.000 | 8 28.00 0.00 2940.00 0 0 0.000 0.000
978 969a038a | 10 75.00 -210.00 1399.25 0 0 0.000 0.000 | 26 3.00 0.00 2925.60 0 0 0.000 0.000
984 969a038a | 10 1.00 -210.00 1399.25 0 0 0.000 0.000 | 26 9.00 0.00 2832.00 0 0 0.000 0.000
990 969a038a | 10 7.00 -210.00 1399.25 0 0 0.000 0.000 | 26 5.00 0.00 2652.00 0 0 0.000 0.000
996 969a038a | 10 13.00 -210.00 1399.25 0 0 0.000 0.000 | 26 1.00 0.00 2397.60 0 0 0.000 0.000
1002 969a038a | 10 19.00 -210.00 1399.25 0 0 0.000 0.000 | 26 7.00 0.00 2133.60 0 0 0.000 0.000
1008 969a038a | 10 25.00 -210.00 1399.25 0 0 0.000 0.000 | 26 3.00 0.00 1869.60 0 0 0.000 0.000
1014 969a038a | 10 31.00 -210.00 1399.25 0 0 0.000 0.000 | 26 9.00 0.00 1605.60 0 0 0.000 0.000
1019 90d9be54
1020 90d9be54 | 10 37.00 -216.75 1399.26 0 0 0.000 0.000 | 31 1.00 6.75 1398.91 0 0 0.000 0.000
1026 90d9be54 | 10 43.00 -223.50 1399.27 0 0 0.000 0.000 | 10 0.00 13.50 1398.90 0 0 0.000 0.000
1032 90d9be54 | 10 49.00 -223.50 1399.27 0 0 0.000 0.000 | 10 6.00 13.50 1398.90 0 0 0.000 0.000
1038 90d9be54 | 10 55.00 -223.50 1399.27 0 0 0.000 0.000 | 10 12.00 13.50 1398.90 0 0 0.000 0.000
1044 90d9be54 | 10 61.00 -223.50 1399.27 0 0 0.000 0.000 | 10 18.00 13.50 1398.90 0 0 0.000 0.000
1050 90d9be54 | 10 67.00 -223.50 1399.27 0 0 0.000 0.000 | 10 24.00 13.50 1398.90 0 0 0.000 0.000
1056 90d9be54 | 10 73.00 -223.50 1399.27 0 0 0.000 0.000 | 10 30.00 13.50 1398.90 0 0 0.000 0.000
1062 90d9be54 | 10 79.00 -223.50 1399.27 0 0 0.000 0.000 | 10 36.00 13.50 1398.90 0 0 0.000 0.000
1068 90d9be54 | 10 5.00 -223.50 1399.27 0 0 0.000 0.000 | 10 42.00 13.50 1398.90 0 0 0.000 0.000
1074 90d9be54 | 10 11.00 -223.50 1399.27 0 0 0.000 0.000 | 10 48.00 13.50 1398.90 0 0 0.000 0.000
1080 90d9be54 | 10 17.00 -223.50 1399.27 0 0 0.000 0.000 | 10 4.00 13.50 1398.90 0 0 0.000 0.000
1086 90d9be54 | 10 23.00 -223.50 1399.27 0 0 0.000 0.000 | 10 10.00 13.50 1398.90 0 0 0.000 0.000
1092 90d9be54 | 10 29.00 -223.50 1399.27 0 0 0.000 0.000 | 10 16.00 13.50 1398.90 0 0 0.000 0.000
1098 90d9be54 | 10 35.00 -223.50 1399.27 0 0 0.000 0.000 | 10 22.00 13.50 1398.90 0 0 0.000 0.000
1100 90d9be54 | 10 37.00 -223.50 1399.27 0 0 0.000 0.000 | 10 24.00 13.50 1398.90 0 0 0.000 0.000
";

#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn a_forced_snorlax_crushes_the_dummy_as_on_the_n64() {
    forced_ball(FORCED_SNORLAX, 2, "nITKindKabigon from a forced Poké Ball");
}

/// `b36poke3` (`warp_link`, [`FORCED_BALL_ROUTE`],
/// `dITManagerForceMonsterKind` = 3 poked at frame 2): `nITKindTosakinto`.
/// Goldeen flops about the landing point. Rows 396-1100: every 6th frame, the seed's changes, and
/// every frame round a hit; the dummy is Training's CPU (its getup at
/// 630).
const FORCED_GOLDEEN: &str = "
210 69521988
211 a6e9c9a6
212 fe87df14
213 c40df552
327 491c86cd
332 4db793af
333 074d495a
396 074d495a | 10 53.00 -210.00 1399.25 0 0 0.000 0.000 | 57 5.00 2369.91 1575.09 20 0 0.000 0.000 | 19 385.58 1507.32 0.000 0.000
402 074d495a | 10 59.00 -210.00 1399.25 0 0 0.000 0.000 | 57 11.00 2369.91 1311.09 20 0 0.000 0.000 | 19 385.58 1507.32 0.000 0.000
408 074d495a | 10 65.00 -210.00 1399.25 0 0 0.000 0.000 | 57 17.00 2369.91 1047.09 20 0 0.000 0.000 | 34 385.58 1738.32 0.000 16.000
414 074d495a | 10 71.00 -210.00 1399.25 0 0 0.000 0.000 | 57 23.00 2369.91 783.09 20 0 0.000 0.000 | 34 385.58 1834.32 0.000 16.000
420 074d495a | 10 77.00 -210.00 1399.25 0 0 0.000 0.000 | 57 0.00 2369.91 519.09 20 0 0.000 0.000 | 34 385.58 1930.32 0.000 16.000
426 074d495a | 10 3.00 -210.00 1399.25 0 0 0.000 0.000 | 57 6.00 2369.91 255.09 20 0 0.000 0.000 | 34 385.58 1988.32 0.000 -6.000
431 4dc8aab5
432 4dc8aab5 | 10 9.00 -210.00 1399.25 0 0 0.000 0.000 | 57 12.00 2369.91 -8.91 20 0 0.000 0.000 | 34 385.58 1826.32 0.000 -42.000
438 4dc8aab5 | 10 15.00 -210.00 1399.25 0 0 0.000 0.000 | 67 3.00 2359.10 -101.00 20 0 0.000 0.000 | 34 395.58 1580.32 10.000 54.000
444 4dc8aab5 | 10 21.00 -210.00 1399.25 0 0 0.000 0.000 | 67 9.00 2337.48 -101.00 20 0 0.000 0.000 | 34 455.58 1778.32 10.000 18.000
450 4dc8aab5 | 10 27.00 -210.00 1399.25 0 0 0.000 0.000 | 67 15.00 2315.86 -101.00 20 0 0.000 0.000 | 34 515.58 1760.32 10.000 -18.000
456 6443b2a4 | 10 33.00 -210.00 1399.25 0 0 0.000 0.000 | 67 21.00 2294.23 -101.00 20 0 0.000 0.000 | 34 575.58 1526.32 10.000 60.000
462 6443b2a4 | 10 39.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2272.61 -101.00 20 0 0.000 0.000 | 34 635.58 1760.32 10.000 24.000
468 6443b2a4 | 10 45.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2250.99 -101.00 20 0 0.000 0.000 | 34 695.58 1778.32 10.000 -12.000
474 6443b2a4 | 10 51.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2229.37 -101.00 20 0 0.000 0.000 | 34 755.58 1580.32 10.000 -48.000
475 e6bb16d7
480 e6bb16d7 | 10 57.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2207.75 -101.00 20 0 0.000 0.000 | 34 715.58 1736.32 -10.000 30.000
486 e6bb16d7 | 10 63.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2186.13 -101.00 20 0 0.000 0.000 | 34 655.58 1790.32 -10.000 -6.000
492 e6bb16d7 | 10 69.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2164.51 -101.00 20 0 0.000 0.000 | 34 595.58 1628.32 -10.000 -42.000
494 428b763e
498 428b763e | 10 75.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2142.89 -101.00 20 0 0.000 0.000 | 34 535.58 1706.32 -10.000 36.000
504 428b763e | 10 1.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2121.27 -101.00 20 0 0.000 0.000 | 34 475.58 1796.32 -10.000 0.000
510 428b763e | 10 7.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2099.65 -101.00 20 0 0.000 0.000 | 34 415.58 1670.32 -10.000 -36.000
513 a6a6b409
516 a6a6b409 | 10 13.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2078.02 -101.00 20 0 0.000 0.000 | 34 415.58 1670.32 10.000 42.000
522 a6a6b409 | 10 19.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2056.40 -101.00 20 0 0.000 0.000 | 34 475.58 1796.32 10.000 6.000
528 a6a6b409 | 10 25.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2034.78 -101.00 20 0 0.000 0.000 | 34 535.58 1706.32 10.000 -30.000
532 701fe6a8
534 701fe6a8 | 10 31.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2013.16 -101.00 20 0 0.000 0.000 | 34 595.58 1628.32 10.000 48.000
540 701fe6a8 | 10 37.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1991.54 -101.00 20 0 0.000 0.000 | 34 655.58 1790.32 10.000 12.000
546 701fe6a8 | 10 43.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1969.92 -101.00 20 0 0.000 0.000 | 34 715.58 1736.32 10.000 -24.000
551 dd038acb
552 dd038acb | 10 49.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1948.29 -101.00 20 0 0.000 0.000 | 34 755.58 1580.32 -10.000 54.000
558 dd038acb | 10 55.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1926.67 -101.00 20 0 0.000 0.000 | 34 695.58 1778.32 -10.000 18.000
564 dd038acb | 10 61.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1905.05 -101.00 20 0 0.000 0.000 | 34 635.58 1760.32 -10.000 -18.000
570 fa5aea62 | 10 67.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1883.43 -101.00 20 0 0.000 0.000 | 34 575.58 1526.32 10.000 60.000
576 fa5aea62 | 10 73.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1861.81 -101.00 20 0 0.000 0.000 | 34 635.58 1760.32 10.000 24.000
582 fa5aea62 | 10 79.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1840.19 -101.00 20 0 0.000 0.000 | 34 695.58 1778.32 10.000 -12.000
588 fa5aea62 | 10 5.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1818.57 -101.00 20 0 0.000 0.000 | 34 755.58 1580.32 10.000 -48.000
589 f67de79d
594 f67de79d | 10 11.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1796.94 -101.00 20 0 0.000 0.000 | 34 715.58 1736.32 -10.000 30.000
600 f67de79d | 10 17.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1775.32 -101.00 20 0 0.000 0.000 | 34 655.58 1790.32 -10.000 -6.000
606 f67de79d | 10 23.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1753.70 -101.00 20 0 0.000 0.000 | 34 595.58 1628.32 -10.000 -42.000
608 45099bec
612 45099bec | 10 29.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1732.08 -101.00 20 0 0.000 0.000 | 34 535.58 1706.32 -10.000 36.000
613 92387aff
618 92387aff | 10 35.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1710.46 -101.00 20 0 0.000 0.000 | 34 475.58 1796.32 -10.000 0.000
624 92387aff | 10 41.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1688.84 -101.00 20 0 0.000 0.000 | 34 415.58 1670.32 -10.000 -36.000
627 bb25e9c6
630 bb25e9c6 | 10 47.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1667.21 -101.00 20 0 0.000 0.000 | 34 415.58 1670.32 10.000 42.000
636 bb25e9c6 | 10 53.00 -210.00 1399.25 0 0 0.000 0.000 | 79 6.00 1645.59 -101.00 20 0 0.000 0.000 | 34 475.58 1796.32 10.000 6.000
642 bb25e9c6 | 10 59.00 -210.00 1399.25 0 0 0.000 0.000 | 79 12.00 1623.97 -101.00 20 0 0.000 0.000 | 34 535.58 1706.32 10.000 -30.000
646 9e1f7971
648 9e1f7971 | 10 65.00 -210.00 1399.25 0 0 0.000 0.000 | 79 18.00 1602.35 -101.00 20 0 0.000 0.000 | 34 555.58 1628.32 -10.000 48.000
654 9e1f7971 | 10 71.00 -210.00 1399.25 0 0 0.000 0.000 | 79 24.00 1580.73 -101.00 20 0 0.000 0.000 | 34 495.58 1790.32 -10.000 12.000
660 9e1f7971 | 10 77.00 -210.00 1399.25 0 0 0.000 0.000 | 79 30.00 1559.11 -101.00 20 0 0.000 0.000 | 34 435.58 1736.32 -10.000 -24.000
665 ee5d3670
666 ee5d3670 | 10 3.00 -210.00 1399.25 0 0 0.000 0.000 | 79 36.00 1537.48 -101.00 20 0 0.000 0.000 | 34 395.58 1580.32 10.000 54.000
672 ee5d3670 | 10 9.00 -210.00 1399.25 0 0 0.000 0.000 | 79 42.00 1515.86 -101.00 20 0 0.000 0.000 | 34 455.58 1778.32 10.000 18.000
678 ee5d3670 | 10 15.00 -210.00 1399.25 0 0 0.000 0.000 | 79 48.00 1494.24 -101.00 20 0 0.000 0.000 | 34 515.58 1760.32 10.000 -18.000
684 9ad4bb73 | 10 21.00 -210.00 1399.25 0 0 0.000 0.000 | 10 4.00 1472.62 -101.00 20 0 0.000 0.000 | 34 575.58 1526.32 -10.000 60.000
690 9ad4bb73 | 10 27.00 -210.00 1399.25 0 0 0.000 0.000 | 10 10.00 1451.00 -101.00 20 0 0.000 0.000 | 34 515.58 1760.32 -10.000 24.000
696 9ad4bb73 | 10 33.00 -210.00 1399.25 0 0 0.000 0.000 | 10 16.00 1429.38 -101.00 20 0 0.000 0.000 | 34 455.58 1778.32 -10.000 -12.000
702 9ad4bb73 | 10 39.00 -210.00 1399.25 0 0 0.000 0.000 | 10 22.00 1407.75 -101.00 20 0 0.000 0.000 | 34 395.58 1580.32 -10.000 -48.000
703 e3cbf86a
708 e3cbf86a | 10 45.00 -210.00 1399.25 0 0 0.000 0.000 | 10 28.00 1386.13 -101.00 20 0 0.000 0.000 | 34 435.58 1736.32 10.000 30.000
714 e3cbf86a | 10 51.00 -210.00 1399.25 0 0 0.000 0.000 | 10 34.00 1364.51 -101.00 20 0 0.000 0.000 | 34 495.58 1790.32 10.000 -6.000
720 e3cbf86a | 10 57.00 -210.00 1399.25 0 0 0.000 0.000 | 10 40.00 1342.89 -101.00 20 0 0.000 0.000 | 34 555.58 1628.32 10.000 -42.000
722 6bfcdd85
726 6bfcdd85 | 10 63.00 -210.00 1399.25 0 0 0.000 0.000 | 10 46.00 1321.27 -101.00 20 0 0.000 0.000 | 34 615.58 1706.32 10.000 36.000
732 6bfcdd85 | 10 69.00 -210.00 1399.25 0 0 0.000 0.000 | 10 2.00 1320.00 -101.00 20 0 0.000 0.000 | 34 675.58 1796.32 10.000 0.000
738 6bfcdd85 | 10 75.00 -210.00 1399.25 0 0 0.000 0.000 | 10 8.00 1320.00 -101.00 20 0 0.000 0.000 | 34 735.58 1670.32 10.000 -36.000
741 7f965a34
744 7f965a34 | 10 1.00 -210.00 1399.25 0 0 0.000 0.000 | 10 14.00 1320.00 -101.00 20 0 0.000 0.000 | 34 795.58 1670.32 10.000 42.000
750 7f965a34 | 10 7.00 -210.00 1399.25 0 0 0.000 0.000 | 10 20.00 1320.00 -101.00 20 0 0.000 0.000 | 34 855.58 1796.32 10.000 6.000
756 7f965a34 | 10 13.00 -210.00 1399.25 0 0 0.000 0.000 | 10 26.00 1320.00 -101.00 20 0 0.000 0.000 | 34 915.58 1706.32 10.000 -30.000
760 7ff56027
762 7ff56027 | 10 19.00 -210.00 1399.25 0 0 0.000 0.000 | 10 32.00 1320.00 -101.00 20 0 0.000 0.000 | 34 975.58 1628.32 10.000 48.000
768 7ff56027 | 10 25.00 -210.00 1399.25 0 0 0.000 0.000 | 10 38.00 1320.00 -101.00 20 0 0.000 0.000 | 34 1035.58 1790.32 10.000 12.000
774 7ff56027 | 10 31.00 -210.00 1399.25 0 0 0.000 0.000 | 10 44.00 1320.00 -101.00 20 0 0.000 0.000 | 34 1095.58 1736.32 10.000 -24.000
776 7ff56027 | 10 33.00 -210.00 1399.25 0 0 0.000 0.000 | 10 46.00 1320.00 -101.00 20 0 0.000 0.000 | 34 1115.58 1670.32 10.000 -36.000
777 7ff56027 | 10 34.00 -210.00 1399.25 0 0 0.000 0.000 | 10 47.00 1320.00 -101.00 20 0 0.000 0.000 | 34 1125.58 1628.32 10.000 -42.000
778 ce45da4e | 10 35.00 -210.00 1399.25 0 0 0.000 0.000 | 10 48.00 1320.00 -101.00 20 0 0.000 0.000 | 34 1135.58 1580.32 -10.000 60.000
779 ce45da4e | 10 36.00 -210.00 1399.25 0 0 0.000 0.000 | 10 49.00 1320.00 -101.00 20 0 0.000 0.000 | 34 1125.58 1634.32 -10.000 54.000
780 ce45da4e | 10 37.00 -210.00 1399.25 0 0 0.000 0.000 | 10 0.00 1320.00 -101.00 20 0 0.000 0.000 | 34 1115.58 1682.32 -10.000 48.000
781 ce45da4e | 10 38.00 -210.00 1399.25 0 0 0.000 0.000 | 10 1.00 1320.00 -101.00 20 0 0.000 0.000 | 34 1105.58 1724.32 -10.000 42.000
782 ce45da4e | 10 39.00 -210.00 1399.25 0 0 0.000 0.000 | 26 0.00 1320.00 -101.00 20 0 0.000 0.000 | 34 1095.58 1760.32 -10.000 36.000
783 ce45da4e | 10 40.00 -210.00 1399.25 0 0 0.000 0.000 | 26 1.00 1321.80 -103.40 20 0 0.000 0.000 | 34 1085.58 1790.32 -10.000 30.000
784 ce45da4e | 10 41.00 -210.00 1399.25 0 0 0.000 0.000 | 26 2.00 1323.40 -108.20 20 0 0.000 0.000 | 34 1075.58 1814.32 -10.000 24.000
785 830a2c6d | 10 42.00 -210.00 1399.25 0 0 0.000 0.000 | 25 0.00 1320.00 -36.80 20 0 0.000 0.000 | 34 1065.58 1832.32 -10.000 18.000
786 830a2c6d | 10 43.00 -210.00 1399.25 0 0 0.000 0.000 | 25 1.00 1320.00 32.20 20 0 0.000 0.000 | 34 1055.58 1844.32 -10.000 12.000
787 830a2c6d | 10 44.00 -210.00 1399.25 0 0 0.000 0.000 | 25 2.00 1320.00 98.80 20 0 0.000 0.000 | 34 1045.58 1850.32 -10.000 6.000
788 830a2c6d | 10 45.00 -210.00 1399.25 0 0 0.000 0.000 | 25 3.00 1320.00 163.00 20 0 0.000 0.000 | 34 1035.58 1850.32 -10.000 0.000
789 830a2c6d | 10 46.00 -210.00 1399.25 0 0 0.000 0.000 | 25 4.00 1301.40 224.80 20 0 0.000 0.000 | 34 1025.58 1844.32 -10.000 -6.000
790 830a2c6d | 10 47.00 -210.00 1399.25 0 0 0.000 0.000 | 25 5.00 1281.00 284.20 20 0 0.000 0.000 | 34 1015.58 1832.32 -10.000 -12.000
791 01202500 | 10 48.00 -210.00 1399.25 0 0 0.000 0.000 | 25 6.00 1260.80 341.20 20 0 0.000 0.000 | 34 1005.58 1814.32 -10.000 -18.000
792 01202500 | 10 49.00 -210.00 1399.25 0 0 0.000 0.000 | 25 7.00 1240.80 395.80 20 0 0.000 0.000 | 34 995.58 1790.32 -10.000 -24.000
793 01202500 | 10 50.00 -210.00 1399.25 0 0 0.000 0.000 | 25 8.00 1221.00 448.00 20 0 0.000 0.000 | 34 985.58 1760.32 -10.000 -30.000
794 01202500 | 10 51.00 -210.00 1399.25 0 0 0.000 0.000 | 25 9.00 1201.40 497.80 20 0 0.000 0.000 | 34 975.58 1724.32 -10.000 -36.000
795 01202500 | 10 52.00 -210.00 1399.25 0 0 0.000 0.000 | 25 10.00 1182.00 545.20 20 0 0.000 0.000 | 34 965.58 1682.32 -10.000 -42.000
796 01202500 | 10 53.00 -210.00 1399.25 0 0 0.000 0.000 | 25 11.00 1162.80 590.20 20 0 0.000 0.000 | 34 955.58 1634.32 -10.000 -48.000
797 01202500 | 10 54.00 -210.00 1399.25 0 0 0.000 0.000 | 25 12.00 1143.80 632.80 20 0 0.000 0.000
798 01202500 | 10 55.00 -210.00 1399.25 0 0 0.000 0.000 | 25 13.00 1125.00 673.00 20 0 0.000 0.000
799 01202500 | 10 56.00 -210.00 1399.25 0 0 0.000 0.000 | 25 14.00 1106.40 710.80 20 0 0.000 0.000
800 01202500 | 10 57.00 -210.00 1399.25 0 0 0.000 0.000 | 25 15.00 1088.00 746.20 20 0 0.000 0.000
804 01202500 | 10 61.00 -210.00 1399.25 0 0 0.000 0.000 | 25 19.00 1016.40 863.80 20 0 0.000 0.000
810 01202500 | 10 67.00 -210.00 1399.25 0 0 0.000 0.000 | 25 25.00 915.00 968.20 20 0 0.000 0.000
816 01202500 | 10 73.00 -210.00 1399.25 0 0 0.000 0.000 | 25 31.00 820.80 986.20 20 0 0.000 0.000
822 01202500 | 10 79.00 -210.00 1399.25 0 0 0.000 0.000 | 25 37.00 733.80 917.80 20 0 0.000 0.000
828 01202500 | 10 5.00 -210.00 1399.25 0 0 0.000 0.000 | 25 43.00 654.00 763.00 20 0 0.000 0.000
834 01202500 | 10 11.00 -210.00 1399.25 0 0 0.000 0.000 | 25 49.00 581.40 524.00 20 0 0.000 0.000
835 a350db7a
840 a350db7a | 10 17.00 -210.00 1399.25 0 0 0.000 0.000 | 31 5.00 564.60 510.00 20 0 0.000 0.000
846 a350db7a | 10 23.00 -210.00 1399.25 0 0 0.000 0.000 | 10 4.00 564.60 510.00 20 0 0.000 0.000
852 a350db7a | 10 29.00 -210.00 1399.25 0 0 0.000 0.000 | 10 10.00 564.60 510.00 20 0 0.000 0.000
858 a350db7a | 10 35.00 -210.00 1399.25 0 0 0.000 0.000 | 10 16.00 564.60 510.00 20 0 0.000 0.000
864 a350db7a | 10 41.00 -210.00 1399.25 0 0 0.000 0.000 | 10 22.00 564.60 510.00 20 0 0.000 0.000
870 a350db7a | 10 47.00 -210.00 1399.25 0 0 0.000 0.000 | 10 28.00 564.60 510.00 20 0 0.000 0.000
876 a350db7a | 10 53.00 -210.00 1399.25 0 0 0.000 0.000 | 10 34.00 564.60 510.00 20 0 0.000 0.000
882 a350db7a | 10 59.00 -210.00 1399.25 0 0 0.000 0.000 | 10 40.00 564.60 510.00 20 0 0.000 0.000
888 a350db7a | 10 65.00 -210.00 1399.25 0 0 0.000 0.000 | 10 46.00 564.60 510.00 20 0 0.000 0.000
894 a350db7a | 10 71.00 -210.00 1399.25 0 0 0.000 0.000 | 10 2.00 564.60 510.00 20 0 0.000 0.000
900 a350db7a | 10 77.00 -210.00 1399.25 0 0 0.000 0.000 | 10 8.00 564.60 510.00 20 0 0.000 0.000
906 a350db7a | 10 3.00 -210.00 1399.25 0 0 0.000 0.000 | 10 14.00 564.60 510.00 20 0 0.000 0.000
912 a350db7a | 10 9.00 -210.00 1399.25 0 0 0.000 0.000 | 10 20.00 564.60 510.00 20 0 0.000 0.000
918 a350db7a | 10 15.00 -210.00 1399.25 0 0 0.000 0.000 | 10 26.00 564.60 510.00 20 0 0.000 0.000
924 a350db7a | 10 21.00 -210.00 1399.25 0 0 0.000 0.000 | 10 32.00 564.60 510.00 20 0 0.000 0.000
930 a350db7a | 10 27.00 -210.00 1399.25 0 0 0.000 0.000 | 10 38.00 564.60 510.00 20 0 0.000 0.000
936 a350db7a | 10 33.00 -210.00 1399.25 0 0 0.000 0.000 | 10 44.00 564.60 510.00 20 0 0.000 0.000
942 a350db7a | 10 39.00 -210.00 1399.25 0 0 0.000 0.000 | 10 0.00 564.60 510.00 20 0 0.000 0.000
948 a350db7a | 10 45.00 -210.00 1399.25 0 0 0.000 0.000 | 10 6.00 564.60 510.00 20 0 0.000 0.000
954 a350db7a | 10 51.00 -210.00 1399.25 0 0 0.000 0.000 | 10 12.00 564.60 510.00 20 0 0.000 0.000
960 a350db7a | 10 57.00 -210.00 1399.25 0 0 0.000 0.000 | 10 18.00 564.60 510.00 20 0 0.000 0.000
966 a350db7a | 10 63.00 -210.00 1399.25 0 0 0.000 0.000 | 10 24.00 564.60 510.00 20 0 0.000 0.000
972 a350db7a | 10 69.00 -210.00 1399.25 0 0 0.000 0.000 | 10 30.00 564.60 510.00 20 0 0.000 0.000
978 a350db7a | 10 75.00 -210.00 1399.25 0 0 0.000 0.000 | 10 36.00 564.60 510.00 20 0 0.000 0.000
984 a350db7a | 10 1.00 -210.00 1399.25 0 0 0.000 0.000 | 10 42.00 564.60 510.00 20 0 0.000 0.000
990 a350db7a | 10 7.00 -210.00 1399.25 0 0 0.000 0.000 | 10 48.00 564.60 510.00 20 0 0.000 0.000
996 a350db7a | 10 13.00 -210.00 1399.25 0 0 0.000 0.000 | 10 4.00 564.60 510.00 20 0 0.000 0.000
1002 a350db7a | 10 19.00 -210.00 1399.25 0 0 0.000 0.000 | 10 10.00 564.60 510.00 20 0 0.000 0.000
1008 a350db7a | 10 25.00 -210.00 1399.25 0 0 0.000 0.000 | 10 16.00 564.60 510.00 20 0 0.000 0.000
1014 a350db7a | 10 31.00 -210.00 1399.25 0 0 0.000 0.000 | 10 22.00 564.60 510.00 20 0 0.000 0.000
1020 a350db7a | 10 37.00 -210.00 1399.25 0 0 0.000 0.000 | 10 28.00 564.60 510.00 20 0 0.000 0.000
1026 a350db7a | 10 43.00 -210.00 1399.25 0 0 0.000 0.000 | 10 34.00 564.60 510.00 20 0 0.000 0.000
1032 a350db7a | 10 49.00 -210.00 1399.25 0 0 0.000 0.000 | 10 40.00 564.60 510.00 20 0 0.000 0.000
1038 a350db7a | 10 55.00 -210.00 1399.25 0 0 0.000 0.000 | 10 46.00 564.60 510.00 20 0 0.000 0.000
1044 a350db7a | 10 61.00 -210.00 1399.25 0 0 0.000 0.000 | 10 2.00 564.60 510.00 20 0 0.000 0.000
1050 a350db7a | 10 67.00 -210.00 1399.25 0 0 0.000 0.000 | 10 8.00 564.60 510.00 20 0 0.000 0.000
1056 a350db7a | 10 73.00 -210.00 1399.25 0 0 0.000 0.000 | 10 14.00 564.60 510.00 20 0 0.000 0.000
1062 a350db7a | 10 79.00 -210.00 1399.25 0 0 0.000 0.000 | 10 20.00 564.60 510.00 20 0 0.000 0.000
1068 a350db7a | 10 5.00 -210.00 1399.25 0 0 0.000 0.000 | 10 26.00 564.60 510.00 20 0 0.000 0.000
1074 a350db7a | 10 11.00 -210.00 1399.25 0 0 0.000 0.000 | 10 32.00 564.60 510.00 20 0 0.000 0.000
1080 a350db7a | 10 17.00 -210.00 1399.25 0 0 0.000 0.000 | 10 38.00 564.60 510.00 20 0 0.000 0.000
1086 a350db7a | 10 23.00 -210.00 1399.25 0 0 0.000 0.000 | 10 44.00 564.60 510.00 20 0 0.000 0.000
1092 a350db7a | 10 29.00 -210.00 1399.25 0 0 0.000 0.000 | 10 0.00 564.60 510.00 20 0 0.000 0.000
1098 a350db7a | 10 35.00 -210.00 1399.25 0 0 0.000 0.000 | 10 6.00 564.60 510.00 20 0 0.000 0.000
1100 a350db7a | 10 37.00 -210.00 1399.25 0 0 0.000 0.000 | 10 8.00 564.60 510.00 20 0 0.000 0.000
";

#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn a_forced_goldeen_flops_as_on_the_n64() {
    forced_ball(
        FORCED_GOLDEEN,
        3,
        "nITKindTosakinto from a forced Poké Ball",
    );
}

/// `b36poke4` (`warp_link`, [`FORCED_BALL_ROUTE`],
/// `dITManagerForceMonsterKind` = 4 poked at frame 2): `nITKindNyars`.
/// Meowth throws his coins from 429 (`nWPKindNyarsCoin`). Rows 396-1100: every 6th frame, the seed's changes, and
/// every frame round a hit; the dummy is Training's CPU (its getup at
/// 630).
const FORCED_MEOWTH: &str = "
210 69521988
211 a6e9c9a6
212 fe87df14
213 c40df552
327 491c86cd
332 4db793af
333 074d495a
396 074d495a | 10 53.00 -210.00 1399.25 0 0 0.000 0.000 | 57 5.00 2369.91 1575.09 20 0 0.000 0.000 | 19 385.58 1507.32 0.000 0.000
402 074d495a | 10 59.00 -210.00 1399.25 0 0 0.000 0.000 | 57 11.00 2369.91 1311.09 20 0 0.000 0.000 | 19 385.58 1507.32 0.000 0.000
408 074d495a | 10 65.00 -210.00 1399.25 0 0 0.000 0.000 | 57 17.00 2369.91 1047.09 20 0 0.000 0.000 | 35 385.58 1847.32 0.000 16.000
414 074d495a | 10 71.00 -210.00 1399.25 0 0 0.000 0.000 | 57 23.00 2369.91 783.09 20 0 0.000 0.000 | 35 385.58 1943.32 0.000 16.000
420 074d495a | 10 77.00 -210.00 1399.25 0 0 0.000 0.000 | 57 0.00 2369.91 519.09 20 0 0.000 0.000 | 35 385.58 2039.32 0.000 16.000
426 074d495a | 10 3.00 -210.00 1399.25 0 0 0.000 0.000 | 57 6.00 2369.91 255.09 20 0 0.000 0.000 | 35 385.58 2103.32 0.000 0.000
431 4dc8aab5
432 4dc8aab5 | 10 9.00 -210.00 1399.25 0 0 0.000 0.000 | 57 12.00 2369.91 -8.91 20 0 0.000 0.000 | 35 385.58 2103.32 0.000 0.000 | 24 905.58 2103.32 130.000 0.000 ; 24 385.58 2623.32 0.000 130.000 ; 24 -134.42 2103.32 -130.000 0.000 ; 24 385.58 1583.32 0.000 -130.000
438 4dc8aab5 | 10 15.00 -210.00 1399.25 0 0 0.000 0.000 | 67 3.00 2359.10 -101.00 20 0 0.000 0.000 | 35 385.58 2103.32 0.000 0.000 | 24 1685.58 2103.32 130.000 0.000 ; 24 385.58 3403.32 0.000 130.000 ; 24 -914.42 2103.32 -130.000 0.000 ; 24 385.58 803.32 0.000 -130.000 ; 24 638.91 2161.81 126.668 29.244 ; 24 327.09 2356.66 -29.244 126.668 ; 24 132.24 2044.84 -126.668 -29.244 ; 24 444.06 1849.99 29.244 -126.668
444 4dc8aab5 | 10 21.00 -210.00 1399.25 0 0 0.000 0.000 | 67 9.00 2337.48 -101.00 20 0 0.000 0.000 | 35 385.58 2103.32 0.000 0.000 | 24 1398.92 2337.27 126.668 29.244 ; 24 151.63 3116.67 -29.244 126.668 ; 24 -627.77 1869.38 -126.668 -29.244 ; 24 619.53 1089.98 29.244 -126.668
450 4dc8aab5 | 10 27.00 -210.00 1399.25 0 0 0.000 0.000 | 67 15.00 2315.86 -101.00 20 0 0.000 0.000 | 35 385.58 2103.32 0.000 0.000 | 24 1086.64 2445.25 116.843 56.988 ; 24 43.65 2804.38 -56.988 116.843 ; 24 -315.48 1761.39 -116.843 -56.988 ; 24 727.51 1402.26 56.988 -116.843
456 4dc8aab5 | 10 33.00 -210.00 1399.25 0 0 0.000 0.000 | 67 21.00 2294.23 -101.00 20 0 0.000 0.000 | 35 385.58 2103.32 0.000 0.000 | 24 789.69 2430.57 101.029 81.812 ; 24 58.33 2507.44 -81.812 101.029 ; 24 -18.54 1776.08 -101.029 -81.812 ; 24 712.82 1699.21 81.812 -101.029
462 4dc8aab5 | 10 39.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2272.61 -101.00 20 0 0.000 0.000 | 35 385.58 2103.32 0.000 0.000 | 24 1395.87 2921.44 101.029 81.812 ; 24 -432.54 3113.61 -81.812 101.029 ; 24 -624.71 1285.21 -101.029 -81.812 ; 24 1203.69 1093.04 81.812 -101.029 ; 24 545.65 2308.21 80.036 102.441 ; 24 180.69 2263.40 -102.441 80.036 ; 24 225.50 1898.44 -80.036 -102.441 ; 24 590.46 1943.25 102.441 -80.036
468 4dc8aab5 | 10 45.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2250.99 -101.00 20 0 0.000 0.000 | 35 385.58 2103.32 0.000 0.000 | 24 1025.86 2922.86 80.036 102.441 ; 24 -433.96 2743.61 -102.441 80.036 ; 24 -254.71 1283.79 -80.036 -102.441 ; 24 1205.11 1463.04 102.441 -80.036
474 4dc8aab5 | 10 51.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2229.37 -101.00 20 0 0.000 0.000 | 35 385.58 2103.32 0.000 0.000 | 24 715.22 2810.24 54.940 117.820 ; 24 -321.34 2432.97 -117.820 54.940 ; 24 55.93 1396.40 -54.940 -117.820 ; 24 1092.50 1773.68 117.820 -54.940
480 4dc8aab5 | 10 57.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2207.75 -101.00 20 0 0.000 0.000 | 35 385.58 2103.32 0.000 0.000 | 24 493.69 2611.96 27.029 127.159 ; 24 -123.06 2211.44 -127.159 27.029 ; 24 277.46 1594.69 -27.028 -127.159 ; 24 894.21 1995.21 127.159 -27.028
486 4dc8aab5 | 10 63.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2186.13 -101.00 20 0 0.000 0.000 | 35 385.58 2103.32 0.000 0.000 | 24 655.86 3374.92 27.029 127.159 ; 24 -886.02 2373.61 -127.159 27.029 ; 24 115.29 831.73 -27.028 -127.159 ; 24 1657.17 1833.04 127.159 -27.028 ; 24 381.04 2363.28 -2.269 129.980 ; 24 125.62 2098.79 -129.980 -2.269 ; 24 390.11 1843.36 2.269 -129.980 ; 24 645.54 2107.86 129.980 2.269
492 4dc8aab5 | 10 69.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2164.51 -101.00 20 0 0.000 0.000 | 35 385.58 2103.32 0.000 0.000 | 24 367.43 3143.17 -2.269 129.980 ; 24 -654.27 2085.17 -129.980 -2.269 ; 24 403.73 1063.48 2.269 -129.980 ; 24 1425.42 2121.47 129.980 2.269
498 4dc8aab5 | 10 75.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2142.89 -101.00 20 0 0.000 0.000 | 35 385.58 2103.32 0.000 0.000 | 24 196.88 2860.16 -31.450 126.138 ; 24 -371.25 1914.63 -126.138 -31.450 ; 24 574.27 1346.49 31.450 -126.138 ; 24 1142.41 2292.02 126.138 31.450
504 4dc8aab5 | 10 1.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2121.27 -101.00 20 0 0.000 0.000 | 35 385.58 2103.32 0.000 0.000 | 24 149.50 2566.65 -59.019 115.831 ; 24 -77.75 1867.25 -115.831 -59.019 ; 24 621.65 1640.00 59.019 -115.831 ; 24 848.90 2339.40 115.831 59.019
510 4dc8aab5 | 10 7.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2099.65 -101.00 20 0 0.000 0.000 | 35 385.58 2103.32 0.000 0.000 | 24 -204.61 3261.63 -59.019 115.831 ; 24 -772.73 1513.14 -115.831 -59.019 ; 24 975.76 945.02 59.019 -115.831 ; 24 1543.88 2693.51 115.831 59.019 ; 24 218.45 2302.50 -83.562 99.586 ; 24 186.40 1936.20 -99.586 -83.562 ; 24 552.70 1904.15 83.562 -99.586 ; 24 584.75 2270.45 99.586 83.562
516 4dc8aab5 | 10 13.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2078.02 -101.00 20 0 0.000 0.000 | 35 385.58 2103.32 0.000 0.000 | 24 -282.92 2900.01 -83.562 99.586 ; 24 -411.11 1434.83 -99.586 -83.562 ; 24 1054.07 1306.64 83.562 -99.586 ; 24 1182.26 2771.82 99.586 83.562
522 4dc8aab5 | 10 19.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2056.40 -101.00 20 0 0.000 0.000 | 35 385.58 2103.32 0.000 0.000 | 24 -237.36 2572.74 -103.823 78.236 ; 24 -83.84 1480.39 -78.236 -103.823 ; 24 1008.51 1633.91 103.823 -78.236 ; 24 854.99 2726.26 78.236 103.823
528 4dc8aab5 | 10 25.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2034.78 -101.00 20 0 0.000 0.000 | 35 385.58 2103.32 0.000 0.000 | 24 -89.47 2314.83 -118.761 52.876 ; 24 174.07 1628.28 -52.876 -118.761 ; 24 860.62 1891.82 118.761 -52.876 ; 24 597.08 2578.37 52.876 118.761
534 4dc8aab5 | 10 31.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2013.16 -101.00 20 0 0.000 0.000 | 35 385.58 2103.32 0.000 0.000 | 24 -802.03 2632.08 -118.761 52.876 ; 24 -143.18 915.72 -52.876 -118.761 ; 24 1573.18 1574.57 118.761 -52.876 ; 24 914.33 3290.93 52.876 118.761 ; 24 130.35 2152.93 -127.612 24.805 ; 24 335.97 1848.10 -24.805 -127.612 ; 24 640.80 2053.71 127.612 -24.805 ; 24 435.19 2358.55 24.805 127.612
540 4dc8aab5 | 10 37.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1991.54 -101.00 20 0 0.000 0.000 | 35 385.58 2103.32 0.000 0.000 | 24 -635.32 2301.77 -127.612 24.805 ; 24 187.13 1082.43 -24.805 -127.612 ; 24 1406.47 1904.88 127.612 -24.805 ; 24 584.02 3124.22 24.805 127.612
546 4dc8aab5 | 10 43.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1969.92 -101.00 20 0 0.000 0.000 | 35 385.58 2103.32 0.000 0.000 | 24 -393.95 2076.10 -129.921 -4.537 ; 24 412.80 1323.80 4.537 -129.921 ; 24 1165.10 2130.55 129.921 4.537 ; 24 358.35 2882.85 -4.537 129.921
552 4dc8aab5 | 10 49.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1948.29 -101.00 20 0 0.000 0.000 | 35 385.58 2103.32 0.000 0.000 | 24 -116.71 1968.74 -125.570 -33.647 ; 24 520.16 1601.04 33.647 -125.570 ; 24 887.86 2237.91 125.570 33.646 ; 24 250.99 2605.61 -33.646 125.570
558 4dc8aab5 | 10 55.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1926.67 -101.00 20 0 0.000 0.000 | 35 385.58 2103.32 0.000 0.000 | 24 -870.13 1766.86 -125.570 -33.647 ; 24 722.04 847.62 33.647 -125.570 ; 24 1641.28 2439.79 125.570 33.646 ; 24 49.11 3359.03 -33.646 125.570 ; 24 156.01 1981.26 -114.783 -61.031 ; 24 507.64 1873.76 61.031 -114.783 ; 24 615.14 2225.39 114.783 61.031 ; 24 263.51 2332.89 -61.031 114.783
564 4dc8aab5 | 10 61.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1905.05 -101.00 20 0 0.000 0.000 | 35 385.58 2103.32 0.000 0.000 | 24 -532.69 1615.07 -114.783 -61.031 ; 24 873.83 1185.06 61.031 -114.783 ; 24 1303.84 2591.57 114.783 61.031 ; 24 -102.67 3021.59 -61.031 114.783
570 4dc8aab5 | 10 67.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1883.43 -101.00 20 0 0.000 0.000 | 35 385.58 2103.32 0.000 0.000 | 24 -203.10 1591.60 -98.112 -85.288 ; 24 897.30 1514.65 85.288 -98.112 ; 24 974.25 2615.05 98.112 85.288 ; 24 -126.15 2692.00 -85.288 98.112
576 4dc8aab5 | 10 73.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1861.81 -101.00 20 0 0.000 0.000 | 35 385.58 2103.32 0.000 0.000 | 24 79.93 1682.64 -76.412 -105.172 ; 24 806.26 1797.68 105.172 -76.412 ; 24 691.22 2524.01 76.412 105.172 ; 24 -35.11 2408.97 -105.172 76.412
582 4dc8aab5 | 10 79.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1840.19 -101.00 20 0 0.000 0.000 | 35 385.58 2103.32 0.000 0.000 | 24 -378.54 1051.60 -76.412 -105.172 ; 24 1437.30 1339.20 105.172 -76.412 ; 24 1149.70 3155.05 76.412 105.172 ; 24 -666.15 2867.45 -105.172 76.412 ; 24 283.99 1863.99 -50.795 -119.666 ; 24 624.91 2001.73 119.666 -50.795 ; 24 487.17 2342.66 50.795 119.666 ; 24 146.24 2204.91 -119.666 50.795
588 4dc8aab5 | 10 5.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1818.57 -101.00 20 0 0.000 0.000 | 35 385.58 2103.32 0.000 0.000 | 24 -20.78 1146.00 -50.795 -119.666 ; 24 1342.90 1696.97 119.666 -50.795 ; 24 791.94 3060.65 50.795 119.666 ; 24 -571.75 2509.68 -119.666 50.795
594 4dc8aab5 | 10 11.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1796.94 -101.00 20 0 0.000 0.000 | 35 385.58 2103.32 0.000 0.000 | 24 250.13 1335.17 -22.574 -128.025 ; 24 1153.73 1967.88 128.025 -22.574 ; 24 521.02 2871.47 22.574 128.025 ; 24 -382.57 2238.77 -128.025 22.574
600 4dc8aab5 | 10 17.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1775.32 -101.00 20 0 0.000 0.000 | 35 385.58 2103.32 0.000 0.000 | 24 412.79 1584.04 6.804 -129.822 ; 24 904.86 2130.54 129.822 6.804 ; 24 358.36 2622.61 -6.804 129.822 ; 24 -133.71 2076.11 -129.822 -6.804
606 4dc8aab5 | 10 23.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1753.70 -101.00 20 0 0.000 0.000 | 35 385.58 2103.32 0.000 0.000 | 24 453.61 805.11 6.804 -129.822 ; 24 1683.79 2171.36 129.822 6.804 ; 24 317.54 3401.54 -6.804 129.822 ; 24 -912.64 2035.29 -129.822 -6.804 ; 24 457.24 1853.40 35.833 -124.964 ; 24 635.50 2174.99 124.964 35.833 ; 24 313.91 2353.25 -35.833 124.964 ; 24 135.65 2031.66 -124.964 -35.833
612 4dc8aab5 | 10 29.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1732.08 -101.00 20 0 0.000 0.000 | 35 385.58 2103.32 0.000 0.000 | 24 672.24 1103.61 35.833 -124.964 ; 24 1385.29 2389.99 124.964 35.833 ; 24 98.91 3103.04 -35.833 124.964 ; 24 -614.14 1816.66 -124.964 -35.833
613 6443b2a4
618 6443b2a4 | 10 35.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1710.46 -101.00 20 0 0.000 0.000 | 35 385.58 2103.32 0.000 0.000 | 24 763.73 1421.12 63.025 -113.701 ; 24 1067.78 2481.48 113.701 63.025 ; 24 7.42 2785.53 -63.025 113.701 ; 24 -296.63 1725.17 -113.701 -63.025
624 6443b2a4 | 10 41.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1688.84 -101.00 20 0 0.000 0.000 | 35 385.58 2103.32 0.000 0.000 | 24 733.52 1716.89 86.987 -96.609 ; 24 772.01 2451.27 96.609 86.987 ; 24 37.63 2489.76 -86.987 96.609 ; 24 -0.86 1755.38 -96.609 -86.987
630 6443b2a4 | 10 47.00 -210.00 1399.25 0 0 0.000 0.000 | 79 2.00 1667.21 -101.00 20 0 0.000 0.000 | 35 385.58 2103.32 0.000 0.000 | 24 1255.45 1137.24 86.987 -96.609 ; 24 1351.66 2973.20 96.609 86.987 ; 24 -484.29 3069.41 -86.987 96.609 ; 24 -580.51 1233.45 -96.609 -86.987 ; 24 598.56 1954.19 106.490 -74.565 ; 24 534.71 2316.30 74.565 106.490 ; 24 172.60 2252.45 -106.490 74.565 ; 24 236.45 1890.34 -74.565 -106.490
636 6443b2a4 | 10 53.00 -210.00 1399.25 0 0 0.000 0.000 | 79 8.00 1645.59 -101.00 20 0 0.000 0.000 | 35 385.58 2103.32 0.000 0.000 | 24 1237.49 1506.80 106.490 -74.565 ; 24 982.10 2955.24 74.565 106.490 ; 24 -466.34 2699.84 -106.490 74.565 ; 24 -210.94 1251.41 -74.565 -106.490
642 6443b2a4 | 10 59.00 -210.00 1399.25 0 0 0.000 0.000 | 79 14.00 1623.97 -101.00 20 0 0.000 0.000 | 35 385.58 2103.32 0.000 0.000 | 24 1108.78 1811.13 120.534 -48.699 ; 24 677.77 2826.53 48.699 120.534 ; 24 -337.63 2395.52 -120.534 48.699 ; 24 93.38 1380.12 -48.699 -120.534
648 6443b2a4 | 10 65.00 -210.00 1399.25 0 0 0.000 0.000 | 79 20.00 1602.35 -101.00 20 0 0.000 0.000 | 35 385.58 2103.32 0.000 0.000 | 24 899.17 2021.98 128.399 -20.336 ; 24 466.92 2616.92 20.336 128.399 ; 24 -128.02 2184.67 -128.400 20.336 ; 24 304.23 1589.73 -20.336 -128.399
654 6443b2a4 | 10 71.00 -210.00 1399.25 0 0 0.000 0.000 | 79 26.00 1580.73 -101.00 20 0 0.000 0.000 | 35 385.58 2103.32 0.000 0.000 | 24 1669.57 1899.96 128.399 -20.336 ; 24 588.94 3387.32 20.336 128.399 ; 24 -898.42 2306.69 -128.400 20.336 ; 24 182.21 819.33 -20.336 -128.399 ; 24 644.94 2121.46 129.683 9.068 ; 24 367.44 2362.69 -9.068 129.683 ; 24 126.21 2085.19 -129.683 -9.068 ; 24 403.71 1843.96 9.068 -129.683
660 6443b2a4 | 10 77.00 -210.00 1399.25 0 0 0.000 0.000 | 79 32.00 1559.11 -101.00 20 0 0.000 0.000 | 35 385.58 2103.32 0.000 0.000 | 24 1423.04 2175.87 129.683 9.068 ; 24 313.03 3140.79 -9.068 129.683 ; 24 -651.89 2030.78 -129.683 -9.068 ; 24 458.12 1065.86 9.068 -129.683
666 6443b2a4 | 10 3.00 -210.00 1399.25 0 0 0.000 0.000 | 79 38.00 1537.48 -101.00 20 0 0.000 0.000 |  | 24 1131.49 2331.37 124.320 38.008 ; 24 157.53 2849.24 -38.008 124.320 ; 24 -360.34 1875.27 -124.320 -38.008 ; 24 613.63 1357.41 38.008 -124.320
672 6443b2a4 | 10 9.00 -210.00 1399.25 0 0 0.000 0.000 | 79 44.00 1515.86 -101.00 20 0 0.000 0.000
678 6443b2a4 | 10 15.00 -210.00 1399.25 0 0 0.000 0.000 | 10 0.00 1494.24 -101.00 20 0 0.000 0.000
684 6443b2a4 | 10 21.00 -210.00 1399.25 0 0 0.000 0.000 | 10 6.00 1472.62 -101.00 20 0 0.000 0.000
690 6443b2a4 | 10 27.00 -210.00 1399.25 0 0 0.000 0.000 | 10 12.00 1451.00 -101.00 20 0 0.000 0.000
696 6443b2a4 | 10 33.00 -210.00 1399.25 0 0 0.000 0.000 | 10 18.00 1429.38 -101.00 20 0 0.000 0.000
702 6443b2a4 | 10 39.00 -210.00 1399.25 0 0 0.000 0.000 | 10 24.00 1407.75 -101.00 20 0 0.000 0.000
708 6443b2a4 | 10 45.00 -210.00 1399.25 0 0 0.000 0.000 | 10 30.00 1386.13 -101.00 20 0 0.000 0.000
714 6443b2a4 | 10 51.00 -210.00 1399.25 0 0 0.000 0.000 | 10 36.00 1364.51 -101.00 20 0 0.000 0.000
720 6443b2a4 | 10 57.00 -210.00 1399.25 0 0 0.000 0.000 | 10 42.00 1342.89 -101.00 20 0 0.000 0.000
726 6443b2a4 | 10 63.00 -210.00 1399.25 0 0 0.000 0.000 | 10 48.00 1321.27 -101.00 20 0 0.000 0.000
732 6443b2a4 | 10 69.00 -210.00 1399.25 0 0 0.000 0.000 | 10 4.00 1320.00 -101.00 20 0 0.000 0.000
738 6443b2a4 | 10 75.00 -210.00 1399.25 0 0 0.000 0.000 | 10 10.00 1320.00 -101.00 20 0 0.000 0.000
744 6443b2a4 | 10 1.00 -210.00 1399.25 0 0 0.000 0.000 | 10 16.00 1320.00 -101.00 20 0 0.000 0.000
750 6443b2a4 | 10 7.00 -210.00 1399.25 0 0 0.000 0.000 | 10 22.00 1320.00 -101.00 20 0 0.000 0.000
756 6443b2a4 | 10 13.00 -210.00 1399.25 0 0 0.000 0.000 | 10 28.00 1320.00 -101.00 20 0 0.000 0.000
762 6443b2a4 | 10 19.00 -210.00 1399.25 0 0 0.000 0.000 | 10 34.00 1320.00 -101.00 20 0 0.000 0.000
768 6443b2a4 | 10 25.00 -210.00 1399.25 0 0 0.000 0.000 | 10 40.00 1320.00 -101.00 20 0 0.000 0.000
774 6443b2a4 | 10 31.00 -210.00 1399.25 0 0 0.000 0.000 | 10 46.00 1320.00 -101.00 20 0 0.000 0.000
780 6443b2a4 | 10 37.00 -210.00 1399.25 0 0 0.000 0.000 | 10 2.00 1320.00 -101.00 20 0 0.000 0.000
785 dd038acb
786 dd038acb | 10 43.00 -210.00 1399.25 0 0 0.000 0.000 | 25 1.00 1320.00 32.20 20 0 0.000 0.000
791 bb25e9c6
792 bb25e9c6 | 10 49.00 -210.00 1399.25 0 0 0.000 0.000 | 25 7.00 1240.80 395.80 20 0 0.000 0.000
798 bb25e9c6 | 10 55.00 -210.00 1399.25 0 0 0.000 0.000 | 25 13.00 1125.00 673.00 20 0 0.000 0.000
804 bb25e9c6 | 10 61.00 -210.00 1399.25 0 0 0.000 0.000 | 25 19.00 1016.40 863.80 20 0 0.000 0.000
810 bb25e9c6 | 10 67.00 -210.00 1399.25 0 0 0.000 0.000 | 25 25.00 915.00 968.20 20 0 0.000 0.000
816 bb25e9c6 | 10 73.00 -210.00 1399.25 0 0 0.000 0.000 | 25 31.00 820.80 986.20 20 0 0.000 0.000
822 bb25e9c6 | 10 79.00 -210.00 1399.25 0 0 0.000 0.000 | 25 37.00 733.80 917.80 20 0 0.000 0.000
828 bb25e9c6 | 10 5.00 -210.00 1399.25 0 0 0.000 0.000 | 25 43.00 654.00 763.00 20 0 0.000 0.000
834 bb25e9c6 | 10 11.00 -210.00 1399.25 0 0 0.000 0.000 | 25 49.00 581.40 524.00 20 0 0.000 0.000
835 ee5d3670
840 ee5d3670 | 10 17.00 -210.00 1399.25 0 0 0.000 0.000 | 31 5.00 564.60 510.00 20 0 0.000 0.000
846 ee5d3670 | 10 23.00 -210.00 1399.25 0 0 0.000 0.000 | 10 4.00 564.60 510.00 20 0 0.000 0.000
852 ee5d3670 | 10 29.00 -210.00 1399.25 0 0 0.000 0.000 | 10 10.00 564.60 510.00 20 0 0.000 0.000
858 ee5d3670 | 10 35.00 -210.00 1399.25 0 0 0.000 0.000 | 10 16.00 564.60 510.00 20 0 0.000 0.000
864 ee5d3670 | 10 41.00 -210.00 1399.25 0 0 0.000 0.000 | 10 22.00 564.60 510.00 20 0 0.000 0.000
870 ee5d3670 | 10 47.00 -210.00 1399.25 0 0 0.000 0.000 | 10 28.00 564.60 510.00 20 0 0.000 0.000
876 ee5d3670 | 10 53.00 -210.00 1399.25 0 0 0.000 0.000 | 10 34.00 564.60 510.00 20 0 0.000 0.000
882 ee5d3670 | 10 59.00 -210.00 1399.25 0 0 0.000 0.000 | 10 40.00 564.60 510.00 20 0 0.000 0.000
888 ee5d3670 | 10 65.00 -210.00 1399.25 0 0 0.000 0.000 | 10 46.00 564.60 510.00 20 0 0.000 0.000
894 ee5d3670 | 10 71.00 -210.00 1399.25 0 0 0.000 0.000 | 10 2.00 564.60 510.00 20 0 0.000 0.000
900 ee5d3670 | 10 77.00 -210.00 1399.25 0 0 0.000 0.000 | 10 8.00 564.60 510.00 20 0 0.000 0.000
906 ee5d3670 | 10 3.00 -210.00 1399.25 0 0 0.000 0.000 | 10 14.00 564.60 510.00 20 0 0.000 0.000
912 ee5d3670 | 10 9.00 -210.00 1399.25 0 0 0.000 0.000 | 10 20.00 564.60 510.00 20 0 0.000 0.000
918 ee5d3670 | 10 15.00 -210.00 1399.25 0 0 0.000 0.000 | 10 26.00 564.60 510.00 20 0 0.000 0.000
924 ee5d3670 | 10 21.00 -210.00 1399.25 0 0 0.000 0.000 | 10 32.00 564.60 510.00 20 0 0.000 0.000
930 ee5d3670 | 10 27.00 -210.00 1399.25 0 0 0.000 0.000 | 10 38.00 564.60 510.00 20 0 0.000 0.000
936 ee5d3670 | 10 33.00 -210.00 1399.25 0 0 0.000 0.000 | 10 44.00 564.60 510.00 20 0 0.000 0.000
942 ee5d3670 | 10 39.00 -210.00 1399.25 0 0 0.000 0.000 | 10 0.00 564.60 510.00 20 0 0.000 0.000
948 ee5d3670 | 10 45.00 -210.00 1399.25 0 0 0.000 0.000 | 10 6.00 564.60 510.00 20 0 0.000 0.000
954 ee5d3670 | 10 51.00 -210.00 1399.25 0 0 0.000 0.000 | 10 12.00 564.60 510.00 20 0 0.000 0.000
960 ee5d3670 | 10 57.00 -210.00 1399.25 0 0 0.000 0.000 | 10 18.00 564.60 510.00 20 0 0.000 0.000
966 ee5d3670 | 10 63.00 -210.00 1399.25 0 0 0.000 0.000 | 10 24.00 564.60 510.00 20 0 0.000 0.000
972 ee5d3670 | 10 69.00 -210.00 1399.25 0 0 0.000 0.000 | 10 30.00 564.60 510.00 20 0 0.000 0.000
978 ee5d3670 | 10 75.00 -210.00 1399.25 0 0 0.000 0.000 | 10 36.00 564.60 510.00 20 0 0.000 0.000
984 ee5d3670 | 10 1.00 -210.00 1399.25 0 0 0.000 0.000 | 10 42.00 564.60 510.00 20 0 0.000 0.000
990 ee5d3670 | 10 7.00 -210.00 1399.25 0 0 0.000 0.000 | 10 48.00 564.60 510.00 20 0 0.000 0.000
996 ee5d3670 | 10 13.00 -210.00 1399.25 0 0 0.000 0.000 | 10 4.00 564.60 510.00 20 0 0.000 0.000
1002 ee5d3670 | 10 19.00 -210.00 1399.25 0 0 0.000 0.000 | 10 10.00 564.60 510.00 20 0 0.000 0.000
1008 ee5d3670 | 10 25.00 -210.00 1399.25 0 0 0.000 0.000 | 10 16.00 564.60 510.00 20 0 0.000 0.000
1014 ee5d3670 | 10 31.00 -210.00 1399.25 0 0 0.000 0.000 | 10 22.00 564.60 510.00 20 0 0.000 0.000
1020 ee5d3670 | 10 37.00 -210.00 1399.25 0 0 0.000 0.000 | 10 28.00 564.60 510.00 20 0 0.000 0.000
1026 ee5d3670 | 10 43.00 -210.00 1399.25 0 0 0.000 0.000 | 10 34.00 564.60 510.00 20 0 0.000 0.000
1032 ee5d3670 | 10 49.00 -210.00 1399.25 0 0 0.000 0.000 | 10 40.00 564.60 510.00 20 0 0.000 0.000
1038 ee5d3670 | 10 55.00 -210.00 1399.25 0 0 0.000 0.000 | 10 46.00 564.60 510.00 20 0 0.000 0.000
1044 ee5d3670 | 10 61.00 -210.00 1399.25 0 0 0.000 0.000 | 10 2.00 564.60 510.00 20 0 0.000 0.000
1050 ee5d3670 | 10 67.00 -210.00 1399.25 0 0 0.000 0.000 | 10 8.00 564.60 510.00 20 0 0.000 0.000
1056 ee5d3670 | 10 73.00 -210.00 1399.25 0 0 0.000 0.000 | 10 14.00 564.60 510.00 20 0 0.000 0.000
1062 ee5d3670 | 10 79.00 -210.00 1399.25 0 0 0.000 0.000 | 10 20.00 564.60 510.00 20 0 0.000 0.000
1068 ee5d3670 | 10 5.00 -210.00 1399.25 0 0 0.000 0.000 | 10 26.00 564.60 510.00 20 0 0.000 0.000
1074 ee5d3670 | 10 11.00 -210.00 1399.25 0 0 0.000 0.000 | 10 32.00 564.60 510.00 20 0 0.000 0.000
1080 ee5d3670 | 10 17.00 -210.00 1399.25 0 0 0.000 0.000 | 10 38.00 564.60 510.00 20 0 0.000 0.000
1086 ee5d3670 | 10 23.00 -210.00 1399.25 0 0 0.000 0.000 | 10 44.00 564.60 510.00 20 0 0.000 0.000
1092 ee5d3670 | 10 29.00 -210.00 1399.25 0 0 0.000 0.000 | 10 0.00 564.60 510.00 20 0 0.000 0.000
1098 ee5d3670 | 10 35.00 -210.00 1399.25 0 0 0.000 0.000 | 10 6.00 564.60 510.00 20 0 0.000 0.000
1100 ee5d3670 | 10 37.00 -210.00 1399.25 0 0 0.000 0.000 | 10 8.00 564.60 510.00 20 0 0.000 0.000
";

#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn a_forced_meowth_throws_coins_as_on_the_n64() {
    forced_ball(FORCED_MEOWTH, 4, "nITKindNyars from a forced Poké Ball");
}

/// `b36poke5` (`warp_link`, [`FORCED_BALL_ROUTE`],
/// `dITManagerForceMonsterKind` = 5 poked at frame 2): `nITKindLizardon`.
/// Charizard breathes his flames from 456 (`nWPKindLizardonFlame`). Rows 396-1100: every 6th frame, the seed's changes, and
/// every frame round a hit; the dummy is Training's CPU (its getup at
/// 630).
const FORCED_CHARIZARD: &str = "
210 69521988
211 a6e9c9a6
212 fe87df14
213 c40df552
327 491c86cd
332 4db793af
333 074d495a
396 074d495a | 10 53.00 -210.00 1399.25 0 0 0.000 0.000 | 57 5.00 2369.91 1575.09 20 0 0.000 0.000 | 19 385.58 1507.32 0.000 0.000
402 074d495a | 10 59.00 -210.00 1399.25 0 0 0.000 0.000 | 57 11.00 2369.91 1311.09 20 0 0.000 0.000 | 19 385.58 1507.32 0.000 0.000
408 074d495a | 10 65.00 -210.00 1399.25 0 0 0.000 0.000 | 57 17.00 2369.91 1047.09 20 0 0.000 0.000 | 36 385.58 1993.32 0.000 16.000
414 074d495a | 10 71.00 -210.00 1399.25 0 0 0.000 0.000 | 57 23.00 2369.91 783.09 20 0 0.000 0.000 | 36 385.58 2089.32 0.000 16.000
420 074d495a | 10 77.00 -210.00 1399.25 0 0 0.000 0.000 | 57 0.00 2369.91 519.09 20 0 0.000 0.000 | 36 385.58 2185.32 0.000 16.000
426 074d495a | 10 3.00 -210.00 1399.25 0 0 0.000 0.000 | 57 6.00 2369.91 255.09 20 0 0.000 0.000 | 36 385.58 2248.32 0.000 -1.000
431 4dc8aab5
432 4dc8aab5 | 10 9.00 -210.00 1399.25 0 0 0.000 0.000 | 57 12.00 2369.91 -8.91 20 0 0.000 0.000 | 36 385.58 2221.32 0.000 -7.000
438 4dc8aab5 | 10 15.00 -210.00 1399.25 0 0 0.000 0.000 | 67 3.00 2359.10 -101.00 20 0 0.000 0.000 | 36 385.58 2158.32 0.000 -13.000
444 4dc8aab5 | 10 21.00 -210.00 1399.25 0 0 0.000 0.000 | 67 9.00 2337.48 -101.00 20 0 0.000 0.000 | 36 385.58 2059.32 0.000 -19.000
450 4dc8aab5 | 10 27.00 -210.00 1399.25 0 0 0.000 0.000 | 67 15.00 2315.86 -101.00 20 0 0.000 0.000 | 36 385.58 1924.32 0.000 -25.000
456 428b763e | 10 33.00 -210.00 1399.25 0 0 0.000 0.000 | 67 21.00 2294.23 -101.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 157.28 1891.38 -48.296 -12.941
462 428b763e | 10 39.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2272.61 -101.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 -132.50 1813.74 -48.296 -12.941
464 dd038acb
468 dd038acb | 10 45.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2250.99 -101.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 -422.28 1736.09 -48.296 -12.941 ; 25 -35.91 1843.62 -48.296 -12.941
472 45099bec
474 45099bec | 10 51.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2229.37 -101.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 -712.05 1658.45 -48.296 -12.941 ; 25 -325.68 1765.97 -48.296 -12.941 ; 25 60.69 1869.50 -48.296 -12.941
480 9e1f7971 | 10 57.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2207.75 -101.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 -1001.83 1580.80 -48.296 -12.941 ; 25 -615.46 1688.33 -48.296 -12.941 ; 25 -229.09 1791.86 -48.296 -12.941 ; 25 157.28 1895.38 -48.296 -12.941
486 9e1f7971 | 10 63.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2186.13 -101.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 -905.24 1610.68 -48.296 -12.941 ; 25 -518.87 1714.21 -48.296 -12.941 ; 25 -132.50 1817.74 -48.296 -12.941
488 e3cbf86a
492 e3cbf86a | 10 69.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2164.51 -101.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 -1195.02 1533.04 -48.296 -12.941 ; 25 -808.65 1636.57 -48.296 -12.941 ; 25 -422.28 1740.09 -48.296 -12.941 ; 25 807.06 1843.62 48.296 -12.941
496 7ff56027
498 7ff56027 | 10 75.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2142.89 -101.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 -1098.42 1558.92 -48.296 -12.941 ; 25 -712.05 1662.45 -48.296 -12.941 ; 25 1096.83 1765.97 48.296 -12.941 ; 25 710.46 1869.50 48.296 -12.941
504 2213eb38 | 10 1.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2121.27 -101.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 -1001.83 1584.80 -48.296 -12.941 ; 25 1386.61 1688.33 48.296 -12.941 ; 25 1000.24 1791.86 48.296 -12.941 ; 25 613.87 1895.38 48.296 -12.941
510 2213eb38 | 10 7.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2099.65 -101.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 1676.39 1610.68 48.296 -12.941 ; 25 1290.02 1714.21 48.296 -12.941 ; 25 903.65 1817.74 48.296 -12.941
512 830a2c6d
516 830a2c6d | 10 13.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2078.02 -101.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 1966.17 1533.04 48.296 -12.941 ; 25 1579.80 1636.57 48.296 -12.941 ; 25 1193.43 1740.09 48.296 -12.941 ; 25 -35.91 1843.62 -48.296 -12.941
520 d6ae67d6
522 d6ae67d6 | 10 19.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2056.40 -101.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 1869.57 1558.92 48.296 -12.941 ; 25 1483.20 1662.45 48.296 -12.941 ; 25 -325.68 1765.97 -48.296 -12.941 ; 25 60.69 1869.50 -48.296 -12.941
528 f59a2fc3 | 10 25.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2034.78 -101.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 1772.98 1584.80 48.296 -12.941 ; 25 -615.46 1688.33 -48.296 -12.941 ; 25 -229.09 1791.86 -48.296 -12.941 ; 25 157.28 1895.38 -48.296 -12.941
534 f59a2fc3 | 10 31.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2013.16 -101.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 -905.24 1610.68 -48.296 -12.941 ; 25 -518.87 1714.21 -48.296 -12.941 ; 25 -132.50 1817.74 -48.296 -12.941
536 4b40d5c4
540 4b40d5c4 | 10 37.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1991.54 -101.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 -1195.02 1533.04 -48.296 -12.941 ; 25 -808.65 1636.57 -48.296 -12.941 ; 25 -422.28 1740.09 -48.296 -12.941 ; 25 807.06 1843.62 48.296 -12.941
544 dcfd7fa9
546 dcfd7fa9 | 10 43.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1969.92 -101.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 -1098.42 1558.92 -48.296 -12.941 ; 25 -712.05 1662.45 -48.296 -12.941 ; 25 1096.83 1765.97 48.296 -12.941 ; 25 710.46 1869.50 48.296 -12.941
552 5f32f082 | 10 49.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1948.29 -101.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 -1001.83 1584.80 -48.296 -12.941 ; 25 1386.61 1688.33 48.296 -12.941 ; 25 1000.24 1791.86 48.296 -12.941 ; 25 613.87 1895.38 48.296 -12.941
558 5f32f082 | 10 55.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1926.67 -101.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 1676.39 1610.68 48.296 -12.941 ; 25 1290.02 1714.21 48.296 -12.941 ; 25 903.65 1817.74 48.296 -12.941
560 797d559f
564 797d559f | 10 61.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1905.05 -101.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 1966.17 1533.04 48.296 -12.941 ; 25 1579.80 1636.57 48.296 -12.941 ; 25 1193.43 1740.09 48.296 -12.941 ; 25 807.06 1843.62 48.296 -12.941
568 4cada790
570 4cada790 | 10 67.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1883.43 -101.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 1869.57 1558.92 48.296 -12.941 ; 25 1483.20 1662.45 48.296 -12.941 ; 25 1096.83 1765.97 48.296 -12.941 ; 25 60.69 1869.50 -48.296 -12.941
576 4df26f25 | 10 73.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1861.81 -101.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 1772.98 1584.80 48.296 -12.941 ; 25 1386.61 1688.33 48.296 -12.941 ; 25 -229.09 1791.86 -48.296 -12.941 ; 25 157.28 1895.38 -48.296 -12.941
582 4df26f25 | 10 79.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1840.19 -101.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 1676.39 1610.68 48.296 -12.941 ; 25 -518.87 1714.21 -48.296 -12.941 ; 25 -132.50 1817.74 -48.296 -12.941
584 1366fe6e
588 1366fe6e | 10 5.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1818.57 -101.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 1966.17 1533.04 48.296 -12.941 ; 25 -808.65 1636.57 -48.296 -12.941 ; 25 -422.28 1740.09 -48.296 -12.941 ; 25 -35.91 1843.62 -48.296 -12.941
592 e6c46dbb
594 e6c46dbb | 10 11.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1796.94 -101.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 -1098.42 1558.92 -48.296 -12.941 ; 25 -712.05 1662.45 -48.296 -12.941 ; 25 -325.68 1765.97 -48.296 -12.941 ; 25 710.46 1869.50 48.296 -12.941
600 6032ec9c | 10 17.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1775.32 -101.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 -1001.83 1584.80 -48.296 -12.941 ; 25 -615.46 1688.33 -48.296 -12.941 ; 25 1000.24 1791.86 48.296 -12.941 ; 25 613.87 1895.38 48.296 -12.941
606 6032ec9c | 10 23.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1753.70 -101.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 -905.24 1610.68 -48.296 -12.941 ; 25 1290.02 1714.21 48.296 -12.941 ; 25 903.65 1817.74 48.296 -12.941
608 542236e1
612 542236e1 | 10 29.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1732.08 -101.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 -1195.02 1533.04 -48.296 -12.941 ; 25 1579.80 1636.57 48.296 -12.941 ; 25 1193.43 1740.09 48.296 -12.941 ; 25 807.06 1843.62 48.296 -12.941
613 bef6be20
616 409dcdf5
618 409dcdf5 | 10 35.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1710.46 -101.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 1869.57 1558.92 48.296 -12.941 ; 25 1483.20 1662.45 48.296 -12.941 ; 25 1096.83 1765.97 48.296 -12.941 ; 25 60.69 1869.50 -48.296 -12.941
624 121fbe7e | 10 41.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1688.84 -101.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 1772.98 1584.80 48.296 -12.941 ; 25 1386.61 1688.33 48.296 -12.941 ; 25 -229.09 1791.86 -48.296 -12.941 ; 25 157.28 1895.38 -48.296 -12.941
630 121fbe7e | 10 47.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1667.21 -101.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 1676.39 1610.68 48.296 -12.941 ; 25 -518.87 1714.21 -48.296 -12.941 ; 25 -132.50 1817.74 -48.296 -12.941
632 26caec0b
636 26caec0b | 10 53.00 -210.00 1399.25 0 0 0.000 0.000 | 79 4.00 1645.59 -101.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 1966.17 1533.04 48.296 -12.941 ; 25 -808.65 1636.57 -48.296 -12.941 ; 25 -422.28 1740.09 -48.296 -12.941 ; 25 -35.91 1843.62 -48.296 -12.941
640 fbb95a2c
642 fbb95a2c | 10 59.00 -210.00 1399.25 0 0 0.000 0.000 | 79 10.00 1623.97 -101.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 -1098.42 1558.92 -48.296 -12.941 ; 25 -712.05 1662.45 -48.296 -12.941 ; 25 -325.68 1765.97 -48.296 -12.941 ; 25 710.46 1869.50 48.296 -12.941
648 704968b1 | 10 65.00 -210.00 1399.25 0 0 0.000 0.000 | 79 16.00 1602.35 -101.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 -1001.83 1584.80 -48.296 -12.941 ; 25 -615.46 1688.33 -48.296 -12.941 ; 25 1000.24 1791.86 48.296 -12.941 ; 25 613.87 1895.38 48.296 -12.941
654 704968b1 | 10 71.00 -210.00 1399.25 0 0 0.000 0.000 | 79 22.00 1580.73 -101.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 -905.24 1610.68 -48.296 -12.941 ; 25 1290.02 1714.21 48.296 -12.941 ; 25 903.65 1817.74 48.296 -12.941
656 187ebcaa
660 187ebcaa | 10 77.00 -210.00 1399.25 0 0 0.000 0.000 | 79 28.00 1559.11 -101.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 -1195.02 1533.04 -48.296 -12.941 ; 25 1579.80 1636.57 48.296 -12.941 ; 25 1193.43 1740.09 48.296 -12.941 ; 25 807.06 1843.62 48.296 -12.941
664 3cd5ad67
666 3cd5ad67 | 10 3.00 -210.00 1399.25 0 0 0.000 0.000 | 79 34.00 1537.48 -101.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 1869.57 1558.92 48.296 -12.941 ; 25 1483.20 1662.45 48.296 -12.941 ; 25 1096.83 1765.97 48.296 -12.941 ; 25 710.46 1869.50 48.296 -12.941
672 5db2c578 | 10 9.00 -210.00 1399.25 0 0 0.000 0.000 | 79 40.00 1515.86 -101.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 1772.98 1584.80 48.296 -12.941 ; 25 1386.61 1688.33 48.296 -12.941 ; 25 1000.24 1791.86 48.296 -12.941 ; 25 157.28 1895.38 -48.296 -12.941
678 5db2c578 | 10 15.00 -210.00 1399.25 0 0 0.000 0.000 | 79 46.00 1494.24 -101.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 1676.39 1610.68 48.296 -12.941 ; 25 1290.02 1714.21 48.296 -12.941 ; 25 -132.50 1817.74 -48.296 -12.941
680 48cc27ad
684 48cc27ad | 10 21.00 -210.00 1399.25 0 0 0.000 0.000 | 10 2.00 1472.62 -101.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 1966.17 1533.04 48.296 -12.941 ; 25 1579.80 1636.57 48.296 -12.941 ; 25 -422.28 1740.09 -48.296 -12.941 ; 25 -35.91 1843.62 -48.296 -12.941
688 dc67e816
690 dc67e816 | 10 27.00 -210.00 1399.25 0 0 0.000 0.000 | 10 8.00 1451.00 -101.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 1869.57 1558.92 48.296 -12.941 ; 25 -712.05 1662.45 -48.296 -12.941 ; 25 -325.68 1765.97 -48.296 -12.941 ; 25 60.69 1869.50 -48.296 -12.941
696 af14a903 | 10 33.00 -210.00 1399.25 0 0 0.000 0.000 | 10 14.00 1429.38 -101.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 -1001.83 1584.80 -48.296 -12.941 ; 25 -615.46 1688.33 -48.296 -12.941 ; 25 -229.09 1791.86 -48.296 -12.941 ; 25 613.87 1895.38 48.296 -12.941
702 af14a903 | 10 39.00 -210.00 1399.25 0 0 0.000 0.000 | 10 20.00 1407.75 -101.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 -905.24 1610.68 -48.296 -12.941 ; 25 -518.87 1714.21 -48.296 -12.941 ; 25 903.65 1817.74 48.296 -12.941
704 c22d0c04
708 c22d0c04 | 10 45.00 -210.00 1399.25 0 0 0.000 0.000 | 10 26.00 1386.13 -101.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 -1195.02 1533.04 -48.296 -12.941 ; 25 -808.65 1636.57 -48.296 -12.941 ; 25 1193.43 1740.09 48.296 -12.941 ; 25 807.06 1843.62 48.296 -12.941
712 8d66c6e9
714 8d66c6e9 | 10 51.00 -210.00 1399.25 0 0 0.000 0.000 | 10 32.00 1364.51 -101.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 -1098.42 1558.92 -48.296 -12.941 ; 25 1483.20 1662.45 48.296 -12.941 ; 25 1096.83 1765.97 48.296 -12.941 ; 25 710.46 1869.50 48.296 -12.941
720 c6576cc2 | 10 57.00 -210.00 1399.25 0 0 0.000 0.000 | 10 38.00 1342.89 -101.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 1772.98 1584.80 48.296 -12.941 ; 25 1386.61 1688.33 48.296 -12.941 ; 25 1000.24 1791.86 48.296 -12.941 ; 25 157.28 1895.38 -48.296 -12.941
726 c6576cc2 | 10 63.00 -210.00 1399.25 0 0 0.000 0.000 | 10 44.00 1321.27 -101.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 1676.39 1610.68 48.296 -12.941 ; 25 1290.02 1714.21 48.296 -12.941 ; 25 -132.50 1817.74 -48.296 -12.941
728 2fff3adf
732 2fff3adf | 10 69.00 -210.00 1399.25 0 0 0.000 0.000 | 10 0.00 1320.00 -101.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 1966.17 1533.04 48.296 -12.941 ; 25 1579.80 1636.57 48.296 -12.941 ; 25 -422.28 1740.09 -48.296 -12.941 ; 25 -35.91 1843.62 -48.296 -12.941
736 ba6179d0
738 ba6179d0 | 10 75.00 -210.00 1399.25 0 0 0.000 0.000 | 10 6.00 1320.00 -101.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 1869.57 1558.92 48.296 -12.941 ; 25 -712.05 1662.45 -48.296 -12.941 ; 25 -325.68 1765.97 -48.296 -12.941 ; 25 60.69 1869.50 -48.296 -12.941
744 261e4265 | 10 1.00 -210.00 1399.25 0 0 0.000 0.000 | 10 12.00 1320.00 -101.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 -1001.83 1584.80 -48.296 -12.941 ; 25 -615.46 1688.33 -48.296 -12.941 ; 25 -229.09 1791.86 -48.296 -12.941 ; 25 613.87 1895.38 48.296 -12.941
750 261e4265 | 10 7.00 -210.00 1399.25 0 0 0.000 0.000 | 10 18.00 1320.00 -101.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 -905.24 1610.68 -48.296 -12.941 ; 25 -518.87 1714.21 -48.296 -12.941 ; 25 903.65 1817.74 48.296 -12.941
752 1916b6ae
756 1916b6ae | 10 13.00 -210.00 1399.25 0 0 0.000 0.000 | 10 24.00 1320.00 -101.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 -1195.02 1533.04 -48.296 -12.941 ; 25 -808.65 1636.57 -48.296 -12.941 ; 25 1193.43 1740.09 48.296 -12.941 ; 25 807.06 1843.62 48.296 -12.941
760 b2e6fefb
762 b2e6fefb | 10 19.00 -210.00 1399.25 0 0 0.000 0.000 | 10 30.00 1320.00 -101.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 -1098.42 1558.92 -48.296 -12.941 ; 25 1483.20 1662.45 48.296 -12.941 ; 25 1096.83 1765.97 48.296 -12.941 ; 25 710.46 1869.50 48.296 -12.941
768 83849adc | 10 25.00 -210.00 1399.25 0 0 0.000 0.000 | 10 36.00 1320.00 -101.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 1772.98 1584.80 48.296 -12.941 ; 25 1386.61 1688.33 48.296 -12.941 ; 25 1000.24 1791.86 48.296 -12.941 ; 25 613.87 1895.38 48.296 -12.941
774 83849adc | 10 31.00 -210.00 1399.25 0 0 0.000 0.000 | 10 42.00 1320.00 -101.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 1676.39 1610.68 48.296 -12.941 ; 25 1290.02 1714.21 48.296 -12.941 ; 25 903.65 1817.74 48.296 -12.941
776 0677d621
780 0677d621 | 10 37.00 -210.00 1399.25 0 0 0.000 0.000 | 10 48.00 1320.00 -101.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 1966.17 1533.04 48.296 -12.941 ; 25 1579.80 1636.57 48.296 -12.941 ; 25 1193.43 1740.09 48.296 -12.941 ; 25 -35.91 1843.62 -48.296 -12.941
784 ebe871da
785 0fc94289
786 0fc94289 | 10 43.00 -210.00 1399.25 0 0 0.000 0.000 | 25 1.00 1320.00 32.20 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 1869.57 1558.92 48.296 -12.941 ; 25 1483.20 1662.45 48.296 -12.941 ; 25 -325.68 1765.97 -48.296 -12.941 ; 25 60.69 1869.50 -48.296 -12.941
791 a49e586c
792 ddd597f1 | 10 49.00 -210.00 1399.25 0 0 0.000 0.000 | 25 7.00 1240.80 395.80 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 1772.98 1584.80 48.296 -12.941 ; 25 -615.46 1688.33 -48.296 -12.941 ; 25 -229.09 1791.86 -48.296 -12.941 ; 25 157.28 1895.38 -48.296 -12.941
798 ddd597f1 | 10 55.00 -210.00 1399.25 0 0 0.000 0.000 | 25 13.00 1125.00 673.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 -905.24 1610.68 -48.296 -12.941 ; 25 -518.87 1714.21 -48.296 -12.941 ; 25 -132.50 1817.74 -48.296 -12.941
800 ccd4c0ea
804 ccd4c0ea | 10 61.00 -210.00 1399.25 0 0 0.000 0.000 | 25 19.00 1016.40 863.80 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 -1195.02 1533.04 -48.296 -12.941 ; 25 -808.65 1636.57 -48.296 -12.941 ; 25 -422.28 1740.09 -48.296 -12.941 ; 25 807.06 1843.62 48.296 -12.941
808 927e3aa7
810 927e3aa7 | 10 67.00 -210.00 1399.25 0 0 0.000 0.000 | 25 25.00 915.00 968.20 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 -1098.42 1558.92 -48.296 -12.941 ; 25 -712.05 1662.45 -48.296 -12.941 ; 25 1096.83 1765.97 48.296 -12.941 ; 25 710.46 1869.50 48.296 -12.941
816 e732dfb8 | 10 73.00 -210.00 1399.25 0 0 0.000 0.000 | 25 31.00 820.80 986.20 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 -1001.83 1584.80 -48.296 -12.941 ; 25 1386.61 1688.33 48.296 -12.941 ; 25 1000.24 1791.86 48.296 -12.941 ; 25 613.87 1895.38 48.296 -12.941
822 e732dfb8 | 10 79.00 -210.00 1399.25 0 0 0.000 0.000 | 25 37.00 733.80 917.80 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 1676.39 1610.68 48.296 -12.941 ; 25 1290.02 1714.21 48.296 -12.941 ; 25 903.65 1817.74 48.296 -12.941
824 8ecc62ed
828 8ecc62ed | 10 5.00 -210.00 1399.25 0 0 0.000 0.000 | 25 43.00 654.00 763.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 1966.17 1533.04 48.296 -12.941 ; 25 1579.80 1636.57 48.296 -12.941 ; 25 1193.43 1740.09 48.296 -12.941 ; 25 -35.91 1843.62 -48.296 -12.941
832 0e90a856
834 0e90a856 | 10 11.00 -210.00 1399.25 0 0 0.000 0.000 | 25 49.00 581.40 524.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 1869.57 1558.92 48.296 -12.941 ; 25 1483.20 1662.45 48.296 -12.941 ; 25 -325.68 1765.97 -48.296 -12.941 ; 25 60.69 1869.50 -48.296 -12.941
835 e0486980
840 89753ad5 | 10 17.00 -210.00 1399.25 0 0 0.000 0.000 | 31 5.00 564.60 510.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 1772.98 1584.80 48.296 -12.941 ; 25 -615.46 1688.33 -48.296 -12.941 ; 25 -229.09 1791.86 -48.296 -12.941 ; 25 157.28 1895.38 -48.296 -12.941
846 89753ad5 | 10 23.00 -210.00 1399.25 0 0 0.000 0.000 | 10 4.00 564.60 510.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 -905.24 1610.68 -48.296 -12.941 ; 25 -518.87 1714.21 -48.296 -12.941 ; 25 -132.50 1817.74 -48.296 -12.941
848 d91dc2de
852 d91dc2de | 10 29.00 -210.00 1399.25 0 0 0.000 0.000 | 10 10.00 564.60 510.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 -1195.02 1533.04 -48.296 -12.941 ; 25 -808.65 1636.57 -48.296 -12.941 ; 25 -422.28 1740.09 -48.296 -12.941 ; 25 807.06 1843.62 48.296 -12.941
856 1dc0f5eb
858 1dc0f5eb | 10 35.00 -210.00 1399.25 0 0 0.000 0.000 | 10 16.00 564.60 510.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 -1098.42 1558.92 -48.296 -12.941 ; 25 -712.05 1662.45 -48.296 -12.941 ; 25 1096.83 1765.97 48.296 -12.941 ; 25 710.46 1869.50 48.296 -12.941
864 5078cf8c | 10 41.00 -210.00 1399.25 0 0 0.000 0.000 | 10 22.00 564.60 510.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 -1001.83 1584.80 -48.296 -12.941 ; 25 1386.61 1688.33 48.296 -12.941 ; 25 1000.24 1791.86 48.296 -12.941 ; 25 613.87 1895.38 48.296 -12.941
870 5078cf8c | 10 47.00 -210.00 1399.25 0 0 0.000 0.000 | 10 28.00 564.60 510.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 1676.39 1610.68 48.296 -12.941 ; 25 1290.02 1714.21 48.296 -12.941 ; 25 903.65 1817.74 48.296 -12.941
872 fb448791
876 fb448791 | 10 53.00 -210.00 1399.25 0 0 0.000 0.000 | 10 34.00 564.60 510.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 1966.17 1533.04 48.296 -12.941 ; 25 1579.80 1636.57 48.296 -12.941 ; 25 1193.43 1740.09 48.296 -12.941 ; 25 807.06 1843.62 48.296 -12.941
880 1850fb0a
882 1850fb0a | 10 59.00 -210.00 1399.25 0 0 0.000 0.000 | 10 40.00 564.60 510.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 1869.57 1558.92 48.296 -12.941 ; 25 1483.20 1662.45 48.296 -12.941 ; 25 1096.83 1765.97 48.296 -12.941 ; 25 60.69 1869.50 -48.296 -12.941
888 ff619947 | 10 65.00 -210.00 1399.25 0 0 0.000 0.000 | 10 46.00 564.60 510.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 1772.98 1584.80 48.296 -12.941 ; 25 1386.61 1688.33 48.296 -12.941 ; 25 -229.09 1791.86 -48.296 -12.941 ; 25 157.28 1895.38 -48.296 -12.941
894 ff619947 | 10 71.00 -210.00 1399.25 0 0 0.000 0.000 | 10 2.00 564.60 510.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 1676.39 1610.68 48.296 -12.941 ; 25 -518.87 1714.21 -48.296 -12.941 ; 25 -132.50 1817.74 -48.296 -12.941
896 8c7b64d8
900 8c7b64d8 | 10 77.00 -210.00 1399.25 0 0 0.000 0.000 | 10 8.00 564.60 510.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 1966.17 1533.04 48.296 -12.941 ; 25 -808.65 1636.57 -48.296 -12.941 ; 25 -422.28 1740.09 -48.296 -12.941 ; 25 -35.91 1843.62 -48.296 -12.941
904 3807d88d
906 3807d88d | 10 3.00 -210.00 1399.25 0 0 0.000 0.000 | 10 14.00 564.60 510.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 -1098.42 1558.92 -48.296 -12.941 ; 25 -712.05 1662.45 -48.296 -12.941 ; 25 -325.68 1765.97 -48.296 -12.941 ; 25 710.46 1869.50 48.296 -12.941
912 7282c076 | 10 9.00 -210.00 1399.25 0 0 0.000 0.000 | 10 20.00 564.60 510.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 -1001.83 1584.80 -48.296 -12.941 ; 25 -615.46 1688.33 -48.296 -12.941 ; 25 1000.24 1791.86 48.296 -12.941 ; 25 613.87 1895.38 48.296 -12.941
918 7282c076 | 10 15.00 -210.00 1399.25 0 0 0.000 0.000 | 10 26.00 564.60 510.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 -905.24 1610.68 -48.296 -12.941 ; 25 1290.02 1714.21 48.296 -12.941 ; 25 903.65 1817.74 48.296 -12.941
920 627056e3
924 627056e3 | 10 21.00 -210.00 1399.25 0 0 0.000 0.000 | 10 32.00 564.60 510.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 -1195.02 1533.04 -48.296 -12.941 ; 25 1579.80 1636.57 48.296 -12.941 ; 25 1193.43 1740.09 48.296 -12.941 ; 25 807.06 1843.62 48.296 -12.941
928 d0643564
930 d0643564 | 10 27.00 -210.00 1399.25 0 0 0.000 0.000 | 10 38.00 564.60 510.00 20 0 0.000 0.000 | 36 385.58 1788.32 0.000 0.000 | 25 1869.57 1558.92 48.296 -12.941 ; 25 1483.20 1662.45 48.296 -12.941 ; 25 1096.83 1765.97 48.296 -12.941 ; 25 60.69 1869.50 -48.296 -12.941
936 d0643564 | 10 33.00 -210.00 1399.25 0 0 0.000 0.000 | 10 44.00 564.60 510.00 20 0 0.000 0.000 |  | 25 1772.98 1584.80 48.296 -12.941 ; 25 1386.61 1688.33 48.296 -12.941 ; 25 -229.09 1791.86 -48.296 -12.941
942 d0643564 | 10 39.00 -210.00 1399.25 0 0 0.000 0.000 | 10 0.00 564.60 510.00 20 0 0.000 0.000 |  | 25 1676.39 1610.68 48.296 -12.941 ; 25 -518.87 1714.21 -48.296 -12.941
948 d0643564 | 10 45.00 -210.00 1399.25 0 0 0.000 0.000 | 10 6.00 564.60 510.00 20 0 0.000 0.000 |  | 25 1966.17 1533.04 48.296 -12.941 ; 25 -808.65 1636.57 -48.296 -12.941
954 d0643564 | 10 51.00 -210.00 1399.25 0 0 0.000 0.000 | 10 12.00 564.60 510.00 20 0 0.000 0.000 |  | 25 -1098.42 1558.92 -48.296 -12.941
960 d0643564 | 10 57.00 -210.00 1399.25 0 0 0.000 0.000 | 10 18.00 564.60 510.00 20 0 0.000 0.000
966 d0643564 | 10 63.00 -210.00 1399.25 0 0 0.000 0.000 | 10 24.00 564.60 510.00 20 0 0.000 0.000
972 d0643564 | 10 69.00 -210.00 1399.25 0 0 0.000 0.000 | 10 30.00 564.60 510.00 20 0 0.000 0.000
978 d0643564 | 10 75.00 -210.00 1399.25 0 0 0.000 0.000 | 10 36.00 564.60 510.00 20 0 0.000 0.000
984 d0643564 | 10 1.00 -210.00 1399.25 0 0 0.000 0.000 | 10 42.00 564.60 510.00 20 0 0.000 0.000
990 d0643564 | 10 7.00 -210.00 1399.25 0 0 0.000 0.000 | 10 48.00 564.60 510.00 20 0 0.000 0.000
996 d0643564 | 10 13.00 -210.00 1399.25 0 0 0.000 0.000 | 10 4.00 564.60 510.00 20 0 0.000 0.000
1002 d0643564 | 10 19.00 -210.00 1399.25 0 0 0.000 0.000 | 10 10.00 564.60 510.00 20 0 0.000 0.000
1008 d0643564 | 10 25.00 -210.00 1399.25 0 0 0.000 0.000 | 10 16.00 564.60 510.00 20 0 0.000 0.000
1014 d0643564 | 10 31.00 -210.00 1399.25 0 0 0.000 0.000 | 10 22.00 564.60 510.00 20 0 0.000 0.000
1020 d0643564 | 10 37.00 -210.00 1399.25 0 0 0.000 0.000 | 10 28.00 564.60 510.00 20 0 0.000 0.000
1026 d0643564 | 10 43.00 -210.00 1399.25 0 0 0.000 0.000 | 10 34.00 564.60 510.00 20 0 0.000 0.000
1032 d0643564 | 10 49.00 -210.00 1399.25 0 0 0.000 0.000 | 10 40.00 564.60 510.00 20 0 0.000 0.000
1038 d0643564 | 10 55.00 -210.00 1399.25 0 0 0.000 0.000 | 10 46.00 564.60 510.00 20 0 0.000 0.000
1044 d0643564 | 10 61.00 -210.00 1399.25 0 0 0.000 0.000 | 10 2.00 564.60 510.00 20 0 0.000 0.000
1050 d0643564 | 10 67.00 -210.00 1399.25 0 0 0.000 0.000 | 10 8.00 564.60 510.00 20 0 0.000 0.000
1056 d0643564 | 10 73.00 -210.00 1399.25 0 0 0.000 0.000 | 10 14.00 564.60 510.00 20 0 0.000 0.000
1062 d0643564 | 10 79.00 -210.00 1399.25 0 0 0.000 0.000 | 10 20.00 564.60 510.00 20 0 0.000 0.000
1068 d0643564 | 10 5.00 -210.00 1399.25 0 0 0.000 0.000 | 10 26.00 564.60 510.00 20 0 0.000 0.000
1074 d0643564 | 10 11.00 -210.00 1399.25 0 0 0.000 0.000 | 10 32.00 564.60 510.00 20 0 0.000 0.000
1080 d0643564 | 10 17.00 -210.00 1399.25 0 0 0.000 0.000 | 10 38.00 564.60 510.00 20 0 0.000 0.000
1086 d0643564 | 10 23.00 -210.00 1399.25 0 0 0.000 0.000 | 10 44.00 564.60 510.00 20 0 0.000 0.000
1092 d0643564 | 10 29.00 -210.00 1399.25 0 0 0.000 0.000 | 10 0.00 564.60 510.00 20 0 0.000 0.000
1098 d0643564 | 10 35.00 -210.00 1399.25 0 0 0.000 0.000 | 10 6.00 564.60 510.00 20 0 0.000 0.000
1100 d0643564 | 10 37.00 -210.00 1399.25 0 0 0.000 0.000 | 10 8.00 564.60 510.00 20 0 0.000 0.000
";

#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn a_forced_charizard_breathes_fire_as_on_the_n64() {
    forced_ball(
        FORCED_CHARIZARD,
        5,
        "nITKindLizardon from a forced Poké Ball",
    );
}

/// `b36poke6` (`warp_link`, [`FORCED_BALL_ROUTE`],
/// `dITManagerForceMonsterKind` = 6 poked at frame 2): `nITKindSpear`.
/// Beedrill flies off and its swarm crosses the stage from 579 (`nWPKindSpearSwarm`). Rows 396-1100: every 6th frame, the seed's changes, and
/// every frame round a hit; the dummy is Training's CPU (its getup at
/// 630).
const FORCED_BEEDRILL: &str = "
210 69521988
211 a6e9c9a6
212 fe87df14
213 c40df552
327 491c86cd
332 4db793af
333 074d495a
396 074d495a | 10 53.00 -210.00 1399.25 0 0 0.000 0.000 | 57 5.00 2369.91 1575.09 20 0 0.000 0.000 | 19 385.58 1507.32 0.000 0.000
402 074d495a | 10 59.00 -210.00 1399.25 0 0 0.000 0.000 | 57 11.00 2369.91 1311.09 20 0 0.000 0.000 | 19 385.58 1507.32 0.000 0.000
403 4dc8aab5
408 4dc8aab5 | 10 65.00 -210.00 1399.25 0 0 0.000 0.000 | 57 17.00 2369.91 1047.09 20 0 0.000 0.000 | 37 385.58 1847.32 0.000 16.000
414 4dc8aab5 | 10 71.00 -210.00 1399.25 0 0 0.000 0.000 | 57 23.00 2369.91 783.09 20 0 0.000 0.000 | 37 385.58 1943.32 0.000 16.000
420 4dc8aab5 | 10 77.00 -210.00 1399.25 0 0 0.000 0.000 | 57 0.00 2369.91 519.09 20 0 0.000 0.000 | 37 385.58 2039.32 0.000 16.000
426 4dc8aab5 | 10 3.00 -210.00 1399.25 0 0 0.000 0.000 | 57 6.00 2369.91 255.09 20 0 0.000 0.000 | 37 385.58 2103.32 0.000 0.000
431 6443b2a4
432 6443b2a4 | 10 9.00 -210.00 1399.25 0 0 0.000 0.000 | 57 12.00 2369.91 -8.91 20 0 0.000 0.000 | 37 385.58 2103.32 0.000 0.000
438 6443b2a4 | 10 15.00 -210.00 1399.25 0 0 0.000 0.000 | 67 3.00 2359.10 -101.00 20 0 0.000 0.000 | 37 385.58 2103.32 0.000 0.000
444 6443b2a4 | 10 21.00 -210.00 1399.25 0 0 0.000 0.000 | 67 9.00 2337.48 -101.00 20 0 0.000 0.000 | 37 385.58 2103.32 0.000 0.000
450 6443b2a4 | 10 27.00 -210.00 1399.25 0 0 0.000 0.000 | 67 15.00 2315.86 -101.00 20 0 0.000 0.000 | 37 385.58 2103.32 0.000 0.000
456 6443b2a4 | 10 33.00 -210.00 1399.25 0 0 0.000 0.000 | 67 21.00 2294.23 -101.00 20 0 0.000 0.000 | 37 385.58 2103.32 0.000 0.000
462 6443b2a4 | 10 39.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2272.61 -101.00 20 0 0.000 0.000 | 37 385.58 2103.32 0.000 0.000
468 6443b2a4 | 10 45.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2250.99 -101.00 20 0 0.000 0.000 | 37 385.58 2103.32 0.000 0.000
474 6443b2a4 | 10 51.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2229.37 -101.00 20 0 0.000 0.000 | 37 385.58 2103.32 0.000 0.000
480 6443b2a4 | 10 57.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2207.75 -101.00 20 0 0.000 0.000 | 37 325.58 2393.32 -24.000 56.000
486 6443b2a4 | 10 63.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2186.13 -101.00 20 0 0.000 0.000 | 37 55.58 2708.32 -60.000 50.000
492 6443b2a4 | 10 69.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2164.51 -101.00 20 0 0.000 0.000 | 37 -416.24 2979.87 -86.945 39.343
498 6443b2a4 | 10 75.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2142.89 -101.00 20 0 0.000 0.000 | 37 -961.68 3156.60 -93.008 23.014
504 6443b2a4 | 10 1.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2121.27 -101.00 20 0 0.000 0.000 | 37 -1529.01 3250.84 -95.318 11.057
510 6443b2a4 | 10 7.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2099.65 -101.00 20 0 0.000 0.000 | 37 -2103.65 3286.49 -95.959 2.722
516 6443b2a4 | 10 13.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2078.02 -101.00 20 0 0.000 0.000 | 37 -2679.55 3281.85 -95.951 -2.966
522 6443b2a4 | 10 19.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2056.40 -101.00 20 0 0.000 0.000 | 37 -3254.56 3249.85 -95.742 -6.816
528 6443b2a4 | 10 25.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2034.78 -101.00 20 0 0.000 0.000 | 37 -3828.18 3199.36 -95.506 -9.413
534 6443b2a4 | 10 31.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2013.16 -101.00 20 0 0.000 0.000 | 37 -4400.49 3136.42 -95.305 -11.164
540 6443b2a4 | 10 37.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1991.54 -101.00 20 0 0.000 0.000 | 37 -4971.76 3065.08 -95.149 -12.345
546 6443b2a4 | 10 43.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1969.92 -101.00 20 0 0.000 0.000 | 37 -5542.23 2988.07 -95.035 -13.142
552 6443b2a4 | 10 49.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1948.29 -101.00 20 0 0.000 0.000 | 37 -6112.15 2907.23 -94.954 -13.679
558 6443b2a4 | 10 55.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1926.67 -101.00 20 0 0.000 0.000 | 37 -6681.67 2823.82 -94.898 -14.042
564 6443b2a4 | 10 61.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1905.05 -101.00 20 0 0.000 0.000 | 37 -7250.91 2738.67 -94.859 -14.287
570 6443b2a4 | 10 67.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1883.43 -101.00 20 0 0.000 0.000 | 37 -7819.96 2652.34 -94.832 -14.452
576 6443b2a4 | 10 73.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1861.81 -101.00 20 0 0.000 0.000 | 37 -8388.89 2565.21 -94.814 -14.564
579 428b763e
582 428b763e | 10 79.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1840.19 -101.00 20 0 0.000 0.000 | 37 -8578.51 2536.04 0.000 0.000 | 26 -8058.51 2925.65 130.000 0.000
588 428b763e | 10 5.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1818.57 -101.00 20 0 0.000 0.000 | 37 -8578.51 2536.04 0.000 0.000 | 26 -7278.51 2925.65 130.000 0.000
593 701fe6a8
594 701fe6a8 | 10 11.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1796.94 -101.00 20 0 0.000 0.000 | 37 -8578.51 2536.04 0.000 0.000 | 26 -6498.51 2925.65 130.000 0.000 ; 26 -8318.51 2475.07 130.000 0.000
600 701fe6a8 | 10 17.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1775.32 -101.00 20 0 0.000 0.000 | 37 -8578.51 2536.04 0.000 0.000 | 26 -5718.51 2925.65 130.000 0.000 ; 26 -7538.51 2475.07 130.000 0.000
606 701fe6a8 | 10 23.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1753.70 -101.00 20 0 0.000 0.000 | 37 -8578.51 2536.04 0.000 0.000 | 26 -4938.51 2925.65 130.000 0.000 ; 26 -6758.51 2475.07 130.000 0.000
608 fa5aea62
612 fa5aea62 | 10 29.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1732.08 -101.00 20 0 0.000 0.000 | 37 -8578.51 2536.04 0.000 0.000 | 26 -4158.51 2925.65 130.000 0.000 ; 26 -5978.51 2475.07 130.000 0.000 ; 26 -7928.51 2857.31 130.000 0.000
613 f67de79d
618 f67de79d | 10 35.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1710.46 -101.00 20 0 0.000 0.000 | 37 -8578.51 2536.04 0.000 0.000 | 26 -3378.51 2925.65 130.000 0.000 ; 26 -5198.51 2475.07 130.000 0.000 ; 26 -7148.51 2857.31 130.000 0.000
624 f67de79d | 10 41.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1688.84 -101.00 20 0 0.000 0.000 | 37 -8578.51 2536.04 0.000 0.000 | 26 -2598.51 2925.65 130.000 0.000 ; 26 -4418.51 2475.07 130.000 0.000 ; 26 -6368.51 2857.31 130.000 0.000
628 92387aff
630 92387aff | 10 47.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1667.21 -101.00 20 0 0.000 0.000 | 37 -8578.51 2536.04 0.000 0.000 | 26 -1818.51 2925.65 130.000 0.000 ; 26 -3638.51 2475.07 130.000 0.000 ; 26 -5588.51 2857.31 130.000 0.000 ; 26 -8188.51 1788.73 130.000 0.000
636 92387aff | 10 53.00 -210.00 1399.25 0 0 0.000 0.000 | 79 1.00 1645.59 -101.00 20 0 0.000 0.000 | 37 -8578.51 2536.04 0.000 0.000 | 26 -1038.51 2925.65 130.000 0.000 ; 26 -2858.51 2475.07 130.000 0.000 ; 26 -4808.51 2857.31 130.000 0.000 ; 26 -7408.51 1788.73 130.000 0.000
642 92387aff | 10 59.00 -210.00 1399.25 0 0 0.000 0.000 | 79 7.00 1623.97 -101.00 20 0 0.000 0.000 | 37 -8578.51 2536.04 0.000 0.000 | 26 -258.51 2925.65 130.000 0.000 ; 26 -2078.51 2475.07 130.000 0.000 ; 26 -4028.51 2857.31 130.000 0.000 ; 26 -6628.51 1788.73 130.000 0.000
645 9e1f7971
648 9e1f7971 | 10 65.00 -210.00 1399.25 0 0 0.000 0.000 | 79 13.00 1602.35 -101.00 20 0 0.000 0.000 | 37 -8578.51 2536.04 0.000 0.000 | 26 521.49 2925.65 130.000 0.000 ; 26 -1298.51 2475.07 130.000 0.000 ; 26 -3248.51 2857.31 130.000 0.000 ; 26 -5848.51 1788.73 130.000 0.000 ; 26 -8058.51 2619.18 130.000 0.000
654 9e1f7971 | 10 71.00 -210.00 1399.25 0 0 0.000 0.000 | 79 19.00 1580.73 -101.00 20 0 0.000 0.000 | 37 -8578.51 2536.04 0.000 0.000 | 26 1301.49 2925.65 130.000 0.000 ; 26 -518.51 2475.07 130.000 0.000 ; 26 -2468.51 2857.31 130.000 0.000 ; 26 -5068.51 1788.73 130.000 0.000 ; 26 -7278.51 2619.18 130.000 0.000
660 9e1f7971 | 10 77.00 -210.00 1399.25 0 0 0.000 0.000 | 79 25.00 1559.11 -101.00 20 0 0.000 0.000 | 37 -8578.51 2536.04 0.000 0.000 | 26 2081.49 2925.65 130.000 0.000 ; 26 261.49 2475.07 130.000 0.000 ; 26 -1688.51 2857.31 130.000 0.000 ; 26 -4288.51 1788.73 130.000 0.000 ; 26 -6498.51 2619.18 130.000 0.000
662 9ad4bb73
666 9ad4bb73 | 10 3.00 -210.00 1399.25 0 0 0.000 0.000 | 79 31.00 1537.48 -101.00 20 0 0.000 0.000 | 37 -8578.51 2536.04 0.000 0.000 | 26 2861.49 2925.65 130.000 0.000 ; 26 1041.49 2475.07 130.000 0.000 ; 26 -908.51 2857.31 130.000 0.000 ; 26 -3508.51 1788.73 130.000 0.000 ; 26 -5718.51 2619.18 130.000 0.000 ; 26 -7928.51 2979.32 130.000 0.000
672 9ad4bb73 | 10 9.00 -210.00 1399.25 0 0 0.000 0.000 | 79 37.00 1515.86 -101.00 20 0 0.000 0.000 | 37 -8578.51 2536.04 0.000 0.000 | 26 3641.49 2925.65 130.000 0.000 ; 26 1821.49 2475.07 130.000 0.000 ; 26 -128.51 2857.31 130.000 0.000 ; 26 -2728.51 1788.73 130.000 0.000 ; 26 -4938.51 2619.18 130.000 0.000 ; 26 -7148.51 2979.32 130.000 0.000
678 9ad4bb73 | 10 15.00 -210.00 1399.25 0 0 0.000 0.000 | 79 43.00 1494.24 -101.00 20 0 0.000 0.000 | 37 -8578.51 2536.04 0.000 0.000 | 26 4421.49 2925.65 130.000 0.000 ; 26 2601.49 2475.07 130.000 0.000 ; 26 651.49 2857.31 130.000 0.000 ; 26 -1948.51 1788.73 130.000 0.000 ; 26 -4158.51 2619.18 130.000 0.000 ; 26 -6368.51 2979.32 130.000 0.000
679 6bfcdd85
684 6bfcdd85 | 10 21.00 -210.00 1399.25 0 0 0.000 0.000 | 79 49.00 1472.62 -101.00 20 0 0.000 0.000 | 37 -8578.51 2536.04 0.000 0.000 | 26 5201.49 2925.65 130.000 0.000 ; 26 3381.49 2475.07 130.000 0.000 ; 26 1431.49 2857.31 130.000 0.000 ; 26 -1168.51 1788.73 130.000 0.000 ; 26 -3378.51 2619.18 130.000 0.000 ; 26 -5588.51 2979.32 130.000 0.000 ; 26 -7798.51 2904.99 130.000 0.000
690 6bfcdd85 | 10 27.00 -210.00 1399.25 0 0 0.000 0.000 | 10 5.00 1451.00 -101.00 20 0 0.000 0.000 | 37 -8578.51 2536.04 0.000 0.000 | 26 5981.49 2925.65 130.000 0.000 ; 26 4161.49 2475.07 130.000 0.000 ; 26 2211.49 2857.31 130.000 0.000 ; 26 -388.51 1788.73 130.000 0.000 ; 26 -2598.51 2619.18 130.000 0.000 ; 26 -4808.51 2979.32 130.000 0.000 ; 26 -7018.51 2904.99 130.000 0.000
694 7ff56027
696 7ff56027 | 10 33.00 -210.00 1399.25 0 0 0.000 0.000 | 10 11.00 1429.38 -101.00 20 0 0.000 0.000 | 37 -8578.51 2536.04 0.000 0.000 | 26 6761.49 2925.65 130.000 0.000 ; 26 4941.49 2475.07 130.000 0.000 ; 26 2991.49 2857.31 130.000 0.000 ; 26 391.49 1788.73 130.000 0.000 ; 26 -1818.51 2619.18 130.000 0.000 ; 26 -4028.51 2979.32 130.000 0.000 ; 26 -6238.51 2904.99 130.000 0.000 ; 26 -8188.51 2200.41 130.000 0.000
702 7ff56027 | 10 39.00 -210.00 1399.25 0 0 0.000 0.000 | 10 17.00 1407.75 -101.00 20 0 0.000 0.000 | 37 -8578.51 2536.04 0.000 0.000 | 26 7541.49 2925.65 130.000 0.000 ; 26 5721.49 2475.07 130.000 0.000 ; 26 3771.49 2857.31 130.000 0.000 ; 26 1171.49 1788.73 130.000 0.000 ; 26 -1038.51 2619.18 130.000 0.000 ; 26 -3248.51 2979.32 130.000 0.000 ; 26 -5458.51 2904.99 130.000 0.000 ; 26 -7408.51 2200.41 130.000 0.000
708 7ff56027 | 10 45.00 -210.00 1399.25 0 0 0.000 0.000 | 10 23.00 1386.13 -101.00 20 0 0.000 0.000 | 37 -8578.51 2536.04 0.000 0.000 | 26 8321.49 2925.65 130.000 0.000 ; 26 6501.49 2475.07 130.000 0.000 ; 26 4551.49 2857.31 130.000 0.000 ; 26 1951.49 1788.73 130.000 0.000 ; 26 -258.51 2619.18 130.000 0.000 ; 26 -2468.51 2979.32 130.000 0.000 ; 26 -4678.51 2904.99 130.000 0.000 ; 26 -6628.51 2200.41 130.000 0.000
710 b23bc7d9
714 b23bc7d9 | 10 51.00 -210.00 1399.25 0 0 0.000 0.000 | 10 29.00 1364.51 -101.00 20 0 0.000 0.000 | 37 -8578.51 2536.04 0.000 0.000 | 26 7281.49 2475.07 130.000 0.000 ; 26 5331.49 2857.31 130.000 0.000 ; 26 2731.49 1788.73 130.000 0.000 ; 26 521.49 2619.18 130.000 0.000 ; 26 -1688.51 2979.32 130.000 0.000 ; 26 -3898.51 2904.99 130.000 0.000 ; 26 -5848.51 2200.41 130.000 0.000 ; 26 -7928.51 2753.66 130.000 0.000
720 b23bc7d9 | 10 57.00 -210.00 1399.25 0 0 0.000 0.000 | 10 35.00 1342.89 -101.00 20 0 0.000 0.000 | 37 -8578.51 2536.04 0.000 0.000 | 26 8061.49 2475.07 130.000 0.000 ; 26 6111.49 2857.31 130.000 0.000 ; 26 3511.49 1788.73 130.000 0.000 ; 26 1301.49 2619.18 130.000 0.000 ; 26 -908.51 2979.32 130.000 0.000 ; 26 -3118.51 2904.99 130.000 0.000 ; 26 -5068.51 2200.41 130.000 0.000 ; 26 -7148.51 2753.66 130.000 0.000
726 b23bc7d9 | 10 63.00 -210.00 1399.25 0 0 0.000 0.000 | 10 41.00 1321.27 -101.00 20 0 0.000 0.000 | 37 -8578.51 2536.04 0.000 0.000 | 26 6891.49 2857.31 130.000 0.000 ; 26 4291.49 1788.73 130.000 0.000 ; 26 2081.49 2619.18 130.000 0.000 ; 26 -128.51 2979.32 130.000 0.000 ; 26 -2338.51 2904.99 130.000 0.000 ; 26 -4288.51 2200.41 130.000 0.000 ; 26 -6368.51 2753.66 130.000 0.000
728 a60dbd1b
732 a60dbd1b | 10 69.00 -210.00 1399.25 0 0 0.000 0.000 | 10 47.00 1320.00 -101.00 20 0 0.000 0.000 | 37 -8578.51 2536.04 0.000 0.000 | 26 7671.49 2857.31 130.000 0.000 ; 26 5071.49 1788.73 130.000 0.000 ; 26 2861.49 2619.18 130.000 0.000 ; 26 651.49 2979.32 130.000 0.000 ; 26 -1558.51 2904.99 130.000 0.000 ; 26 -3508.51 2200.41 130.000 0.000 ; 26 -5588.51 2753.66 130.000 0.000 ; 26 -7928.51 1542.91 130.000 0.000
738 a60dbd1b | 10 75.00 -210.00 1399.25 0 0 0.000 0.000 | 10 3.00 1320.00 -101.00 20 0 0.000 0.000 | 37 -8578.51 2536.04 0.000 0.000 | 26 8451.49 2857.31 130.000 0.000 ; 26 5851.49 1788.73 130.000 0.000 ; 26 3641.49 2619.18 130.000 0.000 ; 26 1431.49 2979.32 130.000 0.000 ; 26 -778.51 2904.99 130.000 0.000 ; 26 -2728.51 2200.41 130.000 0.000 ; 26 -4808.51 2753.66 130.000 0.000 ; 26 -7148.51 1542.91 130.000 0.000
744 a60dbd1b | 10 1.00 -210.00 1399.25 0 0 0.000 0.000 | 10 9.00 1320.00 -101.00 20 0 0.000 0.000 | 37 -8578.51 2536.04 0.000 0.000 | 26 6631.49 1788.73 130.000 0.000 ; 26 4421.49 2619.18 130.000 0.000 ; 26 2211.49 2979.32 130.000 0.000 ; 26 1.49 2904.99 130.000 0.000 ; 26 -1948.51 2200.41 130.000 0.000 ; 26 -4028.51 2753.66 130.000 0.000 ; 26 -6368.51 1542.91 130.000 0.000
745 830a2c6d
750 830a2c6d | 10 7.00 -210.00 1399.25 0 0 0.000 0.000 | 10 15.00 1320.00 -101.00 20 0 0.000 0.000 | 37 -8578.51 2536.04 0.000 0.000 | 26 7411.49 1788.73 130.000 0.000 ; 26 5201.49 2619.18 130.000 0.000 ; 26 2991.49 2979.32 130.000 0.000 ; 26 781.49 2904.99 130.000 0.000 ; 26 -1168.51 2200.41 130.000 0.000 ; 26 -3248.51 2753.66 130.000 0.000 ; 26 -5588.51 1542.91 130.000 0.000 ; 26 -7798.51 2959.43 130.000 0.000
756 830a2c6d | 10 13.00 -210.00 1399.25 0 0 0.000 0.000 | 10 21.00 1320.00 -101.00 20 0 0.000 0.000 | 37 -8578.51 2536.04 0.000 0.000 | 26 8191.49 1788.73 130.000 0.000 ; 26 5981.49 2619.18 130.000 0.000 ; 26 3771.49 2979.32 130.000 0.000 ; 26 1561.49 2904.99 130.000 0.000 ; 26 -388.51 2200.41 130.000 0.000 ; 26 -2468.51 2753.66 130.000 0.000 ; 26 -4808.51 1542.91 130.000 0.000 ; 26 -7018.51 2959.43 130.000 0.000
761 8bdb664f
762 8bdb664f | 10 19.00 -210.00 1399.25 0 0 0.000 0.000 | 10 27.00 1320.00 -101.00 20 0 0.000 0.000 | 37 -8578.51 2536.04 0.000 0.000 | 26 6761.49 2619.18 130.000 0.000 ; 26 4551.49 2979.32 130.000 0.000 ; 26 2341.49 2904.99 130.000 0.000 ; 26 391.49 2200.41 130.000 0.000 ; 26 -1688.51 2753.66 130.000 0.000 ; 26 -4028.51 1542.91 130.000 0.000 ; 26 -6238.51 2959.43 130.000 0.000 ; 26 -8318.51 2541.59 130.000 0.000
768 8bdb664f | 10 25.00 -210.00 1399.25 0 0 0.000 0.000 | 10 33.00 1320.00 -101.00 20 0 0.000 0.000 | 37 -8578.51 2536.04 0.000 0.000 | 26 7541.49 2619.18 130.000 0.000 ; 26 5331.49 2979.32 130.000 0.000 ; 26 3121.49 2904.99 130.000 0.000 ; 26 1171.49 2200.41 130.000 0.000 ; 26 -908.51 2753.66 130.000 0.000 ; 26 -3248.51 1542.91 130.000 0.000 ; 26 -5458.51 2959.43 130.000 0.000 ; 26 -7538.51 2541.59 130.000 0.000
774 8bdb664f | 10 31.00 -210.00 1399.25 0 0 0.000 0.000 | 10 39.00 1320.00 -101.00 20 0 0.000 0.000 | 37 -8578.51 2536.04 0.000 0.000 | 26 8321.49 2619.18 130.000 0.000 ; 26 6111.49 2979.32 130.000 0.000 ; 26 3901.49 2904.99 130.000 0.000 ; 26 1951.49 2200.41 130.000 0.000 ; 26 -128.51 2753.66 130.000 0.000 ; 26 -2468.51 1542.91 130.000 0.000 ; 26 -4678.51 2959.43 130.000 0.000 ; 26 -6758.51 2541.59 130.000 0.000
777 07323f41
780 07323f41 | 10 37.00 -210.00 1399.25 0 0 0.000 0.000 | 10 45.00 1320.00 -101.00 20 0 0.000 0.000 | 37 -8578.51 2536.04 0.000 0.000 | 26 6891.49 2979.32 130.000 0.000 ; 26 4681.49 2904.99 130.000 0.000 ; 26 2731.49 2200.41 130.000 0.000 ; 26 651.49 2753.66 130.000 0.000 ; 26 -1688.51 1542.91 130.000 0.000 ; 26 -3898.51 2959.43 130.000 0.000 ; 26 -5978.51 2541.59 130.000 0.000 ; 26 -8058.51 2812.79 130.000 0.000
785 4b40d5c4
786 4b40d5c4 | 10 43.00 -210.00 1399.25 0 0 0.000 0.000 | 25 1.00 1320.00 32.20 20 0 0.000 0.000 | 37 -8578.51 2536.04 0.000 0.000 | 26 7671.49 2979.32 130.000 0.000 ; 26 5461.49 2904.99 130.000 0.000 ; 26 3511.49 2200.41 130.000 0.000 ; 26 1431.49 2753.66 130.000 0.000 ; 26 -908.51 1542.91 130.000 0.000 ; 26 -3118.51 2959.43 130.000 0.000 ; 26 -5198.51 2541.59 130.000 0.000 ; 26 -7278.51 2812.79 130.000 0.000
789 eb36b25e
791 3bf6553d
792 3bf6553d | 10 49.00 -210.00 1399.25 0 0 0.000 0.000 | 25 7.00 1240.80 395.80 20 0 0.000 0.000 | 37 -8578.51 2536.04 0.000 0.000 | 26 8451.49 2979.32 130.000 0.000 ; 26 6241.49 2904.99 130.000 0.000 ; 26 4291.49 2200.41 130.000 0.000 ; 26 2211.49 2753.66 130.000 0.000 ; 26 -128.51 1542.91 130.000 0.000 ; 26 -2338.51 2959.43 130.000 0.000 ; 26 -4418.51 2541.59 130.000 0.000 ; 26 -6498.51 2812.79 130.000 0.000 ; 26 -8058.51 2825.37 130.000 0.000
798 3bf6553d | 10 55.00 -210.00 1399.25 0 0 0.000 0.000 | 25 13.00 1125.00 673.00 20 0 0.000 0.000 | 37 -8578.51 2536.04 0.000 0.000 | 26 7021.49 2904.99 130.000 0.000 ; 26 5071.49 2200.41 130.000 0.000 ; 26 2991.49 2753.66 130.000 0.000 ; 26 651.49 1542.91 130.000 0.000 ; 26 -1558.51 2959.43 130.000 0.000 ; 26 -3638.51 2541.59 130.000 0.000 ; 26 -5718.51 2812.79 130.000 0.000 ; 26 -7278.51 2825.37 130.000 0.000
804 3bf6553d | 10 61.00 -210.00 1399.25 0 0 0.000 0.000 | 25 19.00 1016.40 863.80 20 0 0.000 0.000 | 37 -8578.51 2536.04 0.000 0.000 | 26 7801.49 2904.99 130.000 0.000 ; 26 5851.49 2200.41 130.000 0.000 ; 26 3771.49 2753.66 130.000 0.000 ; 26 1431.49 1542.91 130.000 0.000 ; 26 -778.51 2959.43 130.000 0.000 ; 26 -2858.51 2541.59 130.000 0.000 ; 26 -4938.51 2812.79 130.000 0.000 ; 26 -6498.51 2825.37 130.000 0.000
809 797d559f
810 797d559f | 10 67.00 -210.00 1399.25 0 0 0.000 0.000 | 25 25.00 915.00 968.20 20 0 0.000 0.000 | 37 -8578.51 2536.04 0.000 0.000 | 26 8581.49 2904.99 130.000 0.000 ; 26 6631.49 2200.41 130.000 0.000 ; 26 4551.49 2753.66 130.000 0.000 ; 26 2211.49 1542.91 130.000 0.000 ; 26 1.49 2959.43 130.000 0.000 ; 26 -2078.51 2541.59 130.000 0.000 ; 26 -4158.51 2812.79 130.000 0.000 ; 26 -5718.51 2825.37 130.000 0.000 ; 26 -8318.51 2615.48 130.000 0.000
816 797d559f | 10 73.00 -210.00 1399.25 0 0 0.000 0.000 | 25 31.00 820.80 986.20 20 0 0.000 0.000 | 37 -8578.51 2536.04 0.000 0.000 | 26 7411.49 2200.41 130.000 0.000 ; 26 5331.49 2753.66 130.000 0.000 ; 26 2991.49 1542.91 130.000 0.000 ; 26 781.49 2959.43 130.000 0.000 ; 26 -1298.51 2541.59 130.000 0.000 ; 26 -3378.51 2812.79 130.000 0.000 ; 26 -4938.51 2825.37 130.000 0.000 ; 26 -7538.51 2615.48 130.000 0.000
822 797d559f | 10 79.00 -210.00 1399.25 0 0 0.000 0.000 | 25 37.00 733.80 917.80 20 0 0.000 0.000 | 37 -8578.51 2536.04 0.000 0.000 | 26 8191.49 2200.41 130.000 0.000 ; 26 6111.49 2753.66 130.000 0.000 ; 26 3771.49 1542.91 130.000 0.000 ; 26 1561.49 2959.43 130.000 0.000 ; 26 -518.51 2541.59 130.000 0.000 ; 26 -2598.51 2812.79 130.000 0.000 ; 26 -4158.51 2825.37 130.000 0.000 ; 26 -6758.51 2615.48 130.000 0.000
825 8ddc2911
828 8ddc2911 | 10 5.00 -210.00 1399.25 0 0 0.000 0.000 | 25 43.00 654.00 763.00 20 0 0.000 0.000 |  | 26 6891.49 2753.66 130.000 0.000 ; 26 4551.49 1542.91 130.000 0.000 ; 26 2341.49 2959.43 130.000 0.000 ; 26 261.49 2541.59 130.000 0.000 ; 26 -1818.51 2812.79 130.000 0.000 ; 26 -3378.51 2825.37 130.000 0.000 ; 26 -5978.51 2615.48 130.000 0.000 ; 26 -8058.51 2873.30 130.000 0.000
834 8ddc2911 | 10 11.00 -210.00 1399.25 0 0 0.000 0.000 | 25 49.00 581.40 524.00 20 0 0.000 0.000 |  | 26 7671.49 2753.66 130.000 0.000 ; 26 5331.49 1542.91 130.000 0.000 ; 26 3121.49 2959.43 130.000 0.000 ; 26 1041.49 2541.59 130.000 0.000 ; 26 -1038.51 2812.79 130.000 0.000 ; 26 -2598.51 2825.37 130.000 0.000 ; 26 -5198.51 2615.48 130.000 0.000 ; 26 -7278.51 2873.30 130.000 0.000
835 314fe813
840 314fe813 | 10 17.00 -210.00 1399.25 0 0 0.000 0.000 | 31 5.00 564.60 510.00 20 0 0.000 0.000 |  | 26 8451.49 2753.66 130.000 0.000 ; 26 6111.49 1542.91 130.000 0.000 ; 26 3901.49 2959.43 130.000 0.000 ; 26 1821.49 2541.59 130.000 0.000 ; 26 -258.51 2812.79 130.000 0.000 ; 26 -1818.51 2825.37 130.000 0.000 ; 26 -4418.51 2615.48 130.000 0.000 ; 26 -6498.51 2873.30 130.000 0.000
846 314fe813 | 10 23.00 -210.00 1399.25 0 0 0.000 0.000 | 10 4.00 564.60 510.00 20 0 0.000 0.000 |  | 26 6891.49 1542.91 130.000 0.000 ; 26 4681.49 2959.43 130.000 0.000 ; 26 2601.49 2541.59 130.000 0.000 ; 26 521.49 2812.79 130.000 0.000 ; 26 -1038.51 2825.37 130.000 0.000 ; 26 -3638.51 2615.48 130.000 0.000 ; 26 -5718.51 2873.30 130.000 0.000
852 314fe813 | 10 29.00 -210.00 1399.25 0 0 0.000 0.000 | 10 10.00 564.60 510.00 20 0 0.000 0.000 |  | 26 7671.49 1542.91 130.000 0.000 ; 26 5461.49 2959.43 130.000 0.000 ; 26 3381.49 2541.59 130.000 0.000 ; 26 1301.49 2812.79 130.000 0.000 ; 26 -258.51 2825.37 130.000 0.000 ; 26 -2858.51 2615.48 130.000 0.000 ; 26 -4938.51 2873.30 130.000 0.000
858 314fe813 | 10 35.00 -210.00 1399.25 0 0 0.000 0.000 | 10 16.00 564.60 510.00 20 0 0.000 0.000 |  | 26 8451.49 1542.91 130.000 0.000 ; 26 6241.49 2959.43 130.000 0.000 ; 26 4161.49 2541.59 130.000 0.000 ; 26 2081.49 2812.79 130.000 0.000 ; 26 521.49 2825.37 130.000 0.000 ; 26 -2078.51 2615.48 130.000 0.000 ; 26 -4158.51 2873.30 130.000 0.000
864 314fe813 | 10 41.00 -210.00 1399.25 0 0 0.000 0.000 | 10 22.00 564.60 510.00 20 0 0.000 0.000 |  | 26 7021.49 2959.43 130.000 0.000 ; 26 4941.49 2541.59 130.000 0.000 ; 26 2861.49 2812.79 130.000 0.000 ; 26 1301.49 2825.37 130.000 0.000 ; 26 -1298.51 2615.48 130.000 0.000 ; 26 -3378.51 2873.30 130.000 0.000
870 314fe813 | 10 47.00 -210.00 1399.25 0 0 0.000 0.000 | 10 28.00 564.60 510.00 20 0 0.000 0.000 |  | 26 7801.49 2959.43 130.000 0.000 ; 26 5721.49 2541.59 130.000 0.000 ; 26 3641.49 2812.79 130.000 0.000 ; 26 2081.49 2825.37 130.000 0.000 ; 26 -518.51 2615.48 130.000 0.000 ; 26 -2598.51 2873.30 130.000 0.000
876 314fe813 | 10 53.00 -210.00 1399.25 0 0 0.000 0.000 | 10 34.00 564.60 510.00 20 0 0.000 0.000 |  | 26 8581.49 2959.43 130.000 0.000 ; 26 6501.49 2541.59 130.000 0.000 ; 26 4421.49 2812.79 130.000 0.000 ; 26 2861.49 2825.37 130.000 0.000 ; 26 261.49 2615.48 130.000 0.000 ; 26 -1818.51 2873.30 130.000 0.000
882 314fe813 | 10 59.00 -210.00 1399.25 0 0 0.000 0.000 | 10 40.00 564.60 510.00 20 0 0.000 0.000 |  | 26 7281.49 2541.59 130.000 0.000 ; 26 5201.49 2812.79 130.000 0.000 ; 26 3641.49 2825.37 130.000 0.000 ; 26 1041.49 2615.48 130.000 0.000 ; 26 -1038.51 2873.30 130.000 0.000
888 314fe813 | 10 65.00 -210.00 1399.25 0 0 0.000 0.000 | 10 46.00 564.60 510.00 20 0 0.000 0.000 |  | 26 8061.49 2541.59 130.000 0.000 ; 26 5981.49 2812.79 130.000 0.000 ; 26 4421.49 2825.37 130.000 0.000 ; 26 1821.49 2615.48 130.000 0.000 ; 26 -258.51 2873.30 130.000 0.000
894 314fe813 | 10 71.00 -210.00 1399.25 0 0 0.000 0.000 | 10 2.00 564.60 510.00 20 0 0.000 0.000 |  | 26 6761.49 2812.79 130.000 0.000 ; 26 5201.49 2825.37 130.000 0.000 ; 26 2601.49 2615.48 130.000 0.000 ; 26 521.49 2873.30 130.000 0.000
900 314fe813 | 10 77.00 -210.00 1399.25 0 0 0.000 0.000 | 10 8.00 564.60 510.00 20 0 0.000 0.000 |  | 26 7541.49 2812.79 130.000 0.000 ; 26 5981.49 2825.37 130.000 0.000 ; 26 3381.49 2615.48 130.000 0.000 ; 26 1301.49 2873.30 130.000 0.000
906 314fe813 | 10 3.00 -210.00 1399.25 0 0 0.000 0.000 | 10 14.00 564.60 510.00 20 0 0.000 0.000 |  | 26 8321.49 2812.79 130.000 0.000 ; 26 6761.49 2825.37 130.000 0.000 ; 26 4161.49 2615.48 130.000 0.000 ; 26 2081.49 2873.30 130.000 0.000
912 314fe813 | 10 9.00 -210.00 1399.25 0 0 0.000 0.000 | 10 20.00 564.60 510.00 20 0 0.000 0.000 |  | 26 7541.49 2825.37 130.000 0.000 ; 26 4941.49 2615.48 130.000 0.000 ; 26 2861.49 2873.30 130.000 0.000
918 314fe813 | 10 15.00 -210.00 1399.25 0 0 0.000 0.000 | 10 26.00 564.60 510.00 20 0 0.000 0.000 |  | 26 8321.49 2825.37 130.000 0.000 ; 26 5721.49 2615.48 130.000 0.000 ; 26 3641.49 2873.30 130.000 0.000
924 314fe813 | 10 21.00 -210.00 1399.25 0 0 0.000 0.000 | 10 32.00 564.60 510.00 20 0 0.000 0.000 |  | 26 6501.49 2615.48 130.000 0.000 ; 26 4421.49 2873.30 130.000 0.000
930 314fe813 | 10 27.00 -210.00 1399.25 0 0 0.000 0.000 | 10 38.00 564.60 510.00 20 0 0.000 0.000 |  | 26 7281.49 2615.48 130.000 0.000 ; 26 5201.49 2873.30 130.000 0.000
936 314fe813 | 10 33.00 -210.00 1399.25 0 0 0.000 0.000 | 10 44.00 564.60 510.00 20 0 0.000 0.000 |  | 26 8061.49 2615.48 130.000 0.000 ; 26 5981.49 2873.30 130.000 0.000
942 314fe813 | 10 39.00 -210.00 1399.25 0 0 0.000 0.000 | 10 0.00 564.60 510.00 20 0 0.000 0.000 |  | 26 6761.49 2873.30 130.000 0.000
948 314fe813 | 10 45.00 -210.00 1399.25 0 0 0.000 0.000 | 10 6.00 564.60 510.00 20 0 0.000 0.000 |  | 26 7541.49 2873.30 130.000 0.000
954 314fe813 | 10 51.00 -210.00 1399.25 0 0 0.000 0.000 | 10 12.00 564.60 510.00 20 0 0.000 0.000 |  | 26 8321.49 2873.30 130.000 0.000
960 314fe813 | 10 57.00 -210.00 1399.25 0 0 0.000 0.000 | 10 18.00 564.60 510.00 20 0 0.000 0.000
966 314fe813 | 10 63.00 -210.00 1399.25 0 0 0.000 0.000 | 10 24.00 564.60 510.00 20 0 0.000 0.000
972 314fe813 | 10 69.00 -210.00 1399.25 0 0 0.000 0.000 | 10 30.00 564.60 510.00 20 0 0.000 0.000
978 314fe813 | 10 75.00 -210.00 1399.25 0 0 0.000 0.000 | 10 36.00 564.60 510.00 20 0 0.000 0.000
984 314fe813 | 10 1.00 -210.00 1399.25 0 0 0.000 0.000 | 10 42.00 564.60 510.00 20 0 0.000 0.000
990 314fe813 | 10 7.00 -210.00 1399.25 0 0 0.000 0.000 | 10 48.00 564.60 510.00 20 0 0.000 0.000
996 314fe813 | 10 13.00 -210.00 1399.25 0 0 0.000 0.000 | 10 4.00 564.60 510.00 20 0 0.000 0.000
1002 314fe813 | 10 19.00 -210.00 1399.25 0 0 0.000 0.000 | 10 10.00 564.60 510.00 20 0 0.000 0.000
1008 314fe813 | 10 25.00 -210.00 1399.25 0 0 0.000 0.000 | 10 16.00 564.60 510.00 20 0 0.000 0.000
1014 314fe813 | 10 31.00 -210.00 1399.25 0 0 0.000 0.000 | 10 22.00 564.60 510.00 20 0 0.000 0.000
1020 314fe813 | 10 37.00 -210.00 1399.25 0 0 0.000 0.000 | 10 28.00 564.60 510.00 20 0 0.000 0.000
1026 314fe813 | 10 43.00 -210.00 1399.25 0 0 0.000 0.000 | 10 34.00 564.60 510.00 20 0 0.000 0.000
1032 314fe813 | 10 49.00 -210.00 1399.25 0 0 0.000 0.000 | 10 40.00 564.60 510.00 20 0 0.000 0.000
1038 314fe813 | 10 55.00 -210.00 1399.25 0 0 0.000 0.000 | 10 46.00 564.60 510.00 20 0 0.000 0.000
1044 314fe813 | 10 61.00 -210.00 1399.25 0 0 0.000 0.000 | 10 2.00 564.60 510.00 20 0 0.000 0.000
1050 314fe813 | 10 67.00 -210.00 1399.25 0 0 0.000 0.000 | 10 8.00 564.60 510.00 20 0 0.000 0.000
1056 314fe813 | 10 73.00 -210.00 1399.25 0 0 0.000 0.000 | 10 14.00 564.60 510.00 20 0 0.000 0.000
1062 314fe813 | 10 79.00 -210.00 1399.25 0 0 0.000 0.000 | 10 20.00 564.60 510.00 20 0 0.000 0.000
1068 314fe813 | 10 5.00 -210.00 1399.25 0 0 0.000 0.000 | 10 26.00 564.60 510.00 20 0 0.000 0.000
1074 314fe813 | 10 11.00 -210.00 1399.25 0 0 0.000 0.000 | 10 32.00 564.60 510.00 20 0 0.000 0.000
1080 314fe813 | 10 17.00 -210.00 1399.25 0 0 0.000 0.000 | 10 38.00 564.60 510.00 20 0 0.000 0.000
1086 314fe813 | 10 23.00 -210.00 1399.25 0 0 0.000 0.000 | 10 44.00 564.60 510.00 20 0 0.000 0.000
1092 314fe813 | 10 29.00 -210.00 1399.25 0 0 0.000 0.000 | 10 0.00 564.60 510.00 20 0 0.000 0.000
1098 314fe813 | 10 35.00 -210.00 1399.25 0 0 0.000 0.000 | 10 6.00 564.60 510.00 20 0 0.000 0.000
1100 314fe813 | 10 37.00 -210.00 1399.25 0 0 0.000 0.000 | 10 8.00 564.60 510.00 20 0 0.000 0.000
";

#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn a_forced_beedrill_swarm_crosses_as_on_the_n64() {
    forced_ball(FORCED_BEEDRILL, 6, "nITKindSpear from a forced Poké Ball");
}

/// `b36poke7` (`warp_link`, [`FORCED_BALL_ROUTE`],
/// `dITManagerForceMonsterKind` = 7 poked at frame 2): `nITKindKamex`.
/// Blastoise sprays Hydro Pump from 456 (`nWPKindKamexHydro`) and slides back. Rows 396-1100: every 6th frame, the seed's changes, and
/// every frame round a hit; the dummy is Training's CPU (its getup at
/// 630).
const FORCED_BLASTOISE: &str = "
210 69521988
211 a6e9c9a6
212 fe87df14
213 c40df552
327 491c86cd
332 4db793af
333 074d495a
396 074d495a | 10 53.00 -210.00 1399.25 0 0 0.000 0.000 | 57 5.00 2369.91 1575.09 20 0 0.000 0.000 | 19 385.58 1507.32 0.000 0.000
402 074d495a | 10 59.00 -210.00 1399.25 0 0 0.000 0.000 | 57 11.00 2369.91 1311.09 20 0 0.000 0.000 | 19 385.58 1507.32 0.000 0.000
408 074d495a | 10 65.00 -210.00 1399.25 0 0 0.000 0.000 | 57 17.00 2369.91 1047.09 20 0 0.000 0.000 | 38 385.58 1903.32 0.000 16.000
414 074d495a | 10 71.00 -210.00 1399.25 0 0 0.000 0.000 | 57 23.00 2369.91 783.09 20 0 0.000 0.000 | 38 385.58 1999.32 0.000 16.000
420 074d495a | 10 77.00 -210.00 1399.25 0 0 0.000 0.000 | 57 0.00 2369.91 519.09 20 0 0.000 0.000 | 38 385.58 2095.32 0.000 16.000
426 074d495a | 10 3.00 -210.00 1399.25 0 0 0.000 0.000 | 57 6.00 2369.91 255.09 20 0 0.000 0.000 | 38 385.58 2158.32 0.000 -1.000
431 4dc8aab5
432 4dc8aab5 | 10 9.00 -210.00 1399.25 0 0 0.000 0.000 | 57 12.00 2369.91 -8.91 20 0 0.000 0.000 | 38 385.58 2131.32 0.000 -7.000
438 4dc8aab5 | 10 15.00 -210.00 1399.25 0 0 0.000 0.000 | 67 3.00 2359.10 -101.00 20 0 0.000 0.000 | 38 385.58 2068.32 0.000 -13.000
444 4dc8aab5 | 10 21.00 -210.00 1399.25 0 0 0.000 0.000 | 67 9.00 2337.48 -101.00 20 0 0.000 0.000 | 38 385.58 1969.32 0.000 -19.000
450 4dc8aab5 | 10 27.00 -210.00 1399.25 0 0 0.000 0.000 | 67 15.00 2315.86 -101.00 20 0 0.000 0.000 | 38 385.58 1834.32 0.000 -25.000
456 6443b2a4 | 10 33.00 -210.00 1399.25 0 0 0.000 0.000 | 67 21.00 2294.23 -101.00 20 0 0.000 0.000 | 38 349.88 1739.38 -35.700 0.000 | 27 745.58 1798.32 0.000 0.000
457 e6bb16d7
461 428b763e
462 428b763e | 10 39.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2272.61 -101.00 20 0 0.000 0.000 | 38 183.98 1739.64 -21.900 0.000 | 27 745.58 1798.32 0.000 0.000
465 a6a6b409
468 a6a6b409 | 10 45.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2250.99 -101.00 20 0 0.000 0.000 | 38 100.88 1739.77 -8.100 0.000 | 27 745.58 1798.32 0.000 0.000
474 a6a6b409 | 10 51.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2229.37 -101.00 20 0 0.000 0.000 | 38 100.58 1739.77 5.700 0.000 | 27 745.58 1798.32 0.000 0.000
480 a6a6b409 | 10 57.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2207.75 -101.00 20 0 0.000 0.000 | 38 183.08 1739.64 19.500 0.000
486 701fe6a8 | 10 63.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2186.13 -101.00 20 0 0.000 0.000 | 38 279.38 1739.49 -35.700 0.000 | 27 675.08 1839.43 0.000 0.000
487 dd038acb
491 fa5aea62
492 fa5aea62 | 10 69.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2164.51 -101.00 20 0 0.000 0.000 | 38 113.48 1739.75 -21.900 0.000 | 27 675.08 1839.43 0.000 0.000
495 f67de79d
498 f67de79d | 10 75.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2142.89 -101.00 20 0 0.000 0.000 | 38 30.38 1739.88 -8.100 0.000 | 27 675.08 1839.43 0.000 0.000
504 f67de79d | 10 1.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2121.27 -101.00 20 0 0.000 0.000 | 38 30.08 1739.88 5.700 0.000 | 27 675.08 1839.43 0.000 0.000
510 f67de79d | 10 7.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2099.65 -101.00 20 0 0.000 0.000 | 38 112.58 1739.75 19.500 0.000
516 45099bec | 10 13.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2078.02 -101.00 20 0 0.000 0.000 | 38 208.88 1739.60 -35.700 0.000 | 27 604.58 1839.54 0.000 0.000
517 92387aff
521 bb25e9c6
522 bb25e9c6 | 10 19.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2056.40 -101.00 20 0 0.000 0.000 | 38 42.98 1739.86 -21.900 0.000 | 27 604.58 1839.54 0.000 0.000
525 9e1f7971
528 9e1f7971 | 10 25.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2034.78 -101.00 20 0 0.000 0.000 | 38 -40.12 1739.99 -8.100 0.000 | 27 604.58 1839.54 0.000 0.000
534 9e1f7971 | 10 31.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2013.16 -101.00 20 0 0.000 0.000 | 38 -40.42 1739.99 5.700 0.000 | 27 604.58 1839.54 0.000 0.000
540 9e1f7971 | 10 37.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1991.54 -101.00 20 0 0.000 0.000 | 38 42.08 1739.86 19.500 0.000
546 ee5d3670 | 10 43.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1969.92 -101.00 20 0 0.000 0.000 | 38 138.38 1739.71 -35.700 0.000 | 27 534.08 1839.65 0.000 0.000
547 9ad4bb73
551 e3cbf86a
552 e3cbf86a | 10 49.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1948.29 -101.00 20 0 0.000 0.000 | 38 -27.52 1739.97 -21.900 0.000 | 27 534.08 1839.65 0.000 0.000
555 6bfcdd85
558 6bfcdd85 | 10 55.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1926.67 -101.00 20 0 0.000 0.000 | 38 -110.62 1740.10 -8.100 0.000 | 27 534.08 1839.65 0.000 0.000
564 6bfcdd85 | 10 61.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1905.05 -101.00 20 0 0.000 0.000 | 38 -110.92 1740.10 5.700 0.000 | 27 534.08 1839.65 0.000 0.000
570 6bfcdd85 | 10 67.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1883.43 -101.00 20 0 0.000 0.000 | 38 -28.42 1739.97 19.500 0.000
576 7f965a34 | 10 73.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1861.81 -101.00 20 0 0.000 0.000 | 38 67.88 1739.82 -35.700 0.000 | 27 463.58 1839.76 0.000 0.000
577 7ff56027
581 ce45da4e
582 ce45da4e | 10 79.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1840.19 -101.00 20 0 0.000 0.000 | 38 -98.02 1740.08 -21.900 0.000 | 27 463.58 1839.76 0.000 0.000
585 b23bc7d9
588 b23bc7d9 | 10 5.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1818.57 -101.00 20 0 0.000 0.000 | 38 -181.12 1740.21 -8.100 0.000 | 27 463.58 1839.76 0.000 0.000
594 b23bc7d9 | 10 11.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1796.94 -101.00 20 0 0.000 0.000 | 38 -181.42 1740.21 5.700 0.000 | 27 463.58 1839.76 0.000 0.000
600 b23bc7d9 | 10 17.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1775.32 -101.00 20 0 0.000 0.000 | 38 -98.92 1740.08 19.500 0.000
606 2213eb38 | 10 23.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1753.70 -101.00 20 0 0.000 0.000 | 38 -2.63 1739.93 -35.700 0.000 | 27 393.08 1839.87 0.000 0.000
607 a60dbd1b
611 eb899372
612 eb899372 | 10 29.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1732.08 -101.00 20 0 0.000 0.000 | 38 -168.53 1740.19 -21.900 0.000 | 27 393.08 1839.87 0.000 0.000
613 830a2c6d
615 b01c0d7c
618 b01c0d7c | 10 35.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1710.46 -101.00 20 0 0.000 0.000 | 38 -251.63 1740.32 -8.100 0.000 | 27 393.08 1839.87 0.000 0.000
624 b01c0d7c | 10 41.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1688.84 -101.00 20 0 0.000 0.000 | 38 -251.93 1740.32 5.700 0.000 | 27 393.08 1839.87 0.000 0.000
630 b01c0d7c | 10 47.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1667.21 -101.00 20 0 0.000 0.000 | 38 -169.43 1740.19 19.500 0.000
636 8bdb664f | 10 53.00 -210.00 1399.25 0 0 0.000 0.000 | 79 6.00 1645.59 -101.00 20 0 0.000 0.000 | 38 -73.13 1740.04 -35.700 0.000 | 27 322.57 1839.98 0.000 0.000
637 d6ae67d6
641 07323f41
642 07323f41 | 10 59.00 -210.00 1399.25 0 0 0.000 0.000 | 79 12.00 1623.97 -101.00 20 0 0.000 0.000 | 38 -239.03 1740.30 -21.900 0.000 | 27 322.57 1839.98 0.000 0.000
645 01202500
648 01202500 | 10 65.00 -210.00 1399.25 0 0 0.000 0.000 | 79 18.00 1602.35 -101.00 20 0 0.000 0.000 | 38 -322.13 1740.43 -8.100 0.000 | 27 322.57 1839.98 0.000 0.000
654 01202500 | 10 71.00 -210.00 1399.25 0 0 0.000 0.000 | 79 24.00 1580.73 -101.00 20 0 0.000 0.000 | 38 -322.43 1740.43 5.700 0.000 | 27 322.57 1839.98 0.000 0.000
660 01202500 | 10 77.00 -210.00 1399.25 0 0 0.000 0.000 | 79 30.00 1559.11 -101.00 20 0 0.000 0.000 | 38 -239.93 1740.30 19.500 0.000
666 f59a2fc3 | 10 3.00 -210.00 1399.25 0 0 0.000 0.000 | 79 36.00 1537.48 -101.00 20 0 0.000 0.000 | 38 -143.63 1740.15 -35.700 0.000 | 27 252.07 1840.09 0.000 0.000
667 a350db7a
671 22ee7455
672 22ee7455 | 10 9.00 -210.00 1399.25 0 0 0.000 0.000 | 79 42.00 1515.86 -101.00 20 0 0.000 0.000 | 38 -309.53 1740.41 -21.900 0.000 | 27 252.07 1840.09 0.000 0.000
675 4b40d5c4
678 4b40d5c4 | 10 15.00 -210.00 1399.25 0 0 0.000 0.000 | 79 48.00 1494.24 -101.00 20 0 0.000 0.000 | 38 -392.63 1740.54 -8.100 0.000 | 27 252.07 1840.09 0.000 0.000
684 4b40d5c4 | 10 21.00 -210.00 1399.25 0 0 0.000 0.000 | 10 4.00 1472.62 -101.00 20 0 0.000 0.000 | 38 -392.93 1740.54 5.700 0.000 | 27 252.07 1840.09 0.000 0.000
690 4b40d5c4 | 10 27.00 -210.00 1399.25 0 0 0.000 0.000 | 10 10.00 1451.00 -101.00 20 0 0.000 0.000 | 38 -310.43 1740.41 19.500 0.000
696 d8782d77 | 10 33.00 -210.00 1399.25 0 0 0.000 0.000 | 10 16.00 1429.38 -101.00 20 0 0.000 0.000 | 38 -214.13 1740.26 -35.700 0.000 | 27 181.57 1840.20 0.000 0.000
697 eb36b25e
701 dcfd7fa9
702 dcfd7fa9 | 10 39.00 -210.00 1399.25 0 0 0.000 0.000 | 10 22.00 1407.75 -101.00 20 0 0.000 0.000 | 38 -380.03 1740.52 -21.900 0.000 | 27 181.57 1840.20 0.000 0.000
705 3e1203c8
708 3e1203c8 | 10 45.00 -210.00 1399.25 0 0 0.000 0.000 | 10 28.00 1386.13 -101.00 20 0 0.000 0.000 | 38 -463.13 1740.65 -8.100 0.000 | 27 181.57 1840.20 0.000 0.000
714 3e1203c8 | 10 51.00 -210.00 1399.25 0 0 0.000 0.000 | 10 34.00 1364.51 -101.00 20 0 0.000 0.000 | 38 -463.43 1740.65 5.700 0.000 | 27 181.57 1840.20 0.000 0.000
720 3e1203c8 | 10 57.00 -210.00 1399.25 0 0 0.000 0.000 | 10 40.00 1342.89 -101.00 20 0 0.000 0.000 | 38 -380.93 1740.52 19.500 0.000
726 1a49b36b | 10 63.00 -210.00 1399.25 0 0 0.000 0.000 | 10 46.00 1321.27 -101.00 20 0 0.000 0.000 | 38 -284.63 1740.37 -35.700 0.000 | 27 111.07 1840.31 0.000 0.000
727 5f32f082
731 3bf6553d
732 3bf6553d | 10 69.00 -210.00 1399.25 0 0 0.000 0.000 | 10 2.00 1320.00 -101.00 20 0 0.000 0.000 | 38 -450.53 1740.63 -21.900 0.000 | 27 111.07 1840.31 0.000 0.000
735 ba9ed30c
738 ba9ed30c | 10 75.00 -210.00 1399.25 0 0 0.000 0.000 | 10 8.00 1320.00 -101.00 20 0 0.000 0.000 | 38 -533.63 1740.76 -8.100 0.000 | 27 111.07 1840.31 0.000 0.000
744 ba9ed30c | 10 1.00 -210.00 1399.25 0 0 0.000 0.000 | 10 14.00 1320.00 -101.00 20 0 0.000 0.000 | 38 -533.93 1740.76 5.700 0.000 | 27 111.07 1840.31 0.000 0.000
750 ba9ed30c | 10 7.00 -210.00 1399.25 0 0 0.000 0.000 | 10 20.00 1320.00 -101.00 20 0 0.000 0.000 | 38 -451.43 1740.63 19.500 0.000
756 797d559f | 10 13.00 -210.00 1399.25 0 0 0.000 0.000 | 10 26.00 1320.00 -101.00 20 0 0.000 0.000 | 38 -355.13 1740.48 -35.700 0.000 | 27 40.57 1840.42 0.000 0.000
757 df49d9e6
761 8ddc2911
762 8ddc2911 | 10 19.00 -210.00 1399.25 0 0 0.000 0.000 | 10 32.00 1320.00 -101.00 20 0 0.000 0.000 | 38 -521.03 1740.74 -21.900 0.000 | 27 40.57 1840.42 0.000 0.000
765 4cada790
768 4cada790 | 10 25.00 -210.00 1399.25 0 0 0.000 0.000 | 10 38.00 1320.00 -101.00 20 0 0.000 0.000 | 38 -604.13 1740.87 -8.100 0.000 | 27 40.57 1840.42 0.000 0.000
774 4cada790 | 10 31.00 -210.00 1399.25 0 0 0.000 0.000 | 10 44.00 1320.00 -101.00 20 0 0.000 0.000 | 38 -604.43 1740.87 5.700 0.000 | 27 40.57 1840.42 0.000 0.000
780 4cada790 | 10 37.00 -210.00 1399.25 0 0 0.000 0.000 | 10 0.00 1320.00 -101.00 20 0 0.000 0.000 | 38 -521.93 1740.74 19.500 0.000
785 2e407ec7
786 1366fe6e | 10 43.00 -210.00 1399.25 0 0 0.000 0.000 | 25 1.00 1320.00 32.20 20 0 0.000 0.000 | 38 -425.63 1740.59 -35.700 0.000 | 27 -29.93 1840.53 0.000 0.000
787 1cd0db79
791 2c3b48ef
792 2c3b48ef | 10 49.00 -210.00 1399.25 0 0 0.000 0.000 | 25 7.00 1240.80 395.80 20 0 0.000 0.000 | 38 -591.53 1740.85 -21.900 0.000 | 27 -29.93 1840.53 0.000 0.000
795 15a13ff6
798 15a13ff6 | 10 55.00 -210.00 1399.25 0 0 0.000 0.000 | 25 13.00 1125.00 673.00 20 0 0.000 0.000 | 38 -674.63 1740.97 -8.100 0.000 | 27 -29.93 1840.53 0.000 0.000
804 15a13ff6 | 10 61.00 -210.00 1399.25 0 0 0.000 0.000 | 25 19.00 1016.40 863.80 20 0 0.000 0.000 | 38 -674.93 1740.98 5.700 0.000 | 27 -29.93 1840.53 0.000 0.000
810 15a13ff6 | 10 67.00 -210.00 1399.25 0 0 0.000 0.000 | 25 25.00 915.00 968.20 20 0 0.000 0.000 | 38 -592.43 1740.85 19.500 0.000
816 15a13ff6 | 10 73.00 -210.00 1399.25 0 0 0.000 0.000 | 25 31.00 820.80 986.20 20 0 0.000 0.000
822 15a13ff6 | 10 79.00 -210.00 1399.25 0 0 0.000 0.000 | 25 37.00 733.80 917.80 20 0 0.000 0.000
828 15a13ff6 | 10 5.00 -210.00 1399.25 0 0 0.000 0.000 | 25 43.00 654.00 763.00 20 0 0.000 0.000
834 15a13ff6 | 10 11.00 -210.00 1399.25 0 0 0.000 0.000 | 25 49.00 581.40 524.00 20 0 0.000 0.000
835 bef6be20
840 bef6be20 | 10 17.00 -210.00 1399.25 0 0 0.000 0.000 | 31 5.00 564.60 510.00 20 0 0.000 0.000
846 bef6be20 | 10 23.00 -210.00 1399.25 0 0 0.000 0.000 | 10 4.00 564.60 510.00 20 0 0.000 0.000
852 bef6be20 | 10 29.00 -210.00 1399.25 0 0 0.000 0.000 | 10 10.00 564.60 510.00 20 0 0.000 0.000
858 bef6be20 | 10 35.00 -210.00 1399.25 0 0 0.000 0.000 | 10 16.00 564.60 510.00 20 0 0.000 0.000
864 bef6be20 | 10 41.00 -210.00 1399.25 0 0 0.000 0.000 | 10 22.00 564.60 510.00 20 0 0.000 0.000
870 bef6be20 | 10 47.00 -210.00 1399.25 0 0 0.000 0.000 | 10 28.00 564.60 510.00 20 0 0.000 0.000
876 bef6be20 | 10 53.00 -210.00 1399.25 0 0 0.000 0.000 | 10 34.00 564.60 510.00 20 0 0.000 0.000
882 bef6be20 | 10 59.00 -210.00 1399.25 0 0 0.000 0.000 | 10 40.00 564.60 510.00 20 0 0.000 0.000
888 bef6be20 | 10 65.00 -210.00 1399.25 0 0 0.000 0.000 | 10 46.00 564.60 510.00 20 0 0.000 0.000
894 bef6be20 | 10 71.00 -210.00 1399.25 0 0 0.000 0.000 | 10 2.00 564.60 510.00 20 0 0.000 0.000
900 bef6be20 | 10 77.00 -210.00 1399.25 0 0 0.000 0.000 | 10 8.00 564.60 510.00 20 0 0.000 0.000
906 bef6be20 | 10 3.00 -210.00 1399.25 0 0 0.000 0.000 | 10 14.00 564.60 510.00 20 0 0.000 0.000
912 bef6be20 | 10 9.00 -210.00 1399.25 0 0 0.000 0.000 | 10 20.00 564.60 510.00 20 0 0.000 0.000
918 bef6be20 | 10 15.00 -210.00 1399.25 0 0 0.000 0.000 | 10 26.00 564.60 510.00 20 0 0.000 0.000
924 bef6be20 | 10 21.00 -210.00 1399.25 0 0 0.000 0.000 | 10 32.00 564.60 510.00 20 0 0.000 0.000
930 bef6be20 | 10 27.00 -210.00 1399.25 0 0 0.000 0.000 | 10 38.00 564.60 510.00 20 0 0.000 0.000
936 bef6be20 | 10 33.00 -210.00 1399.25 0 0 0.000 0.000 | 10 44.00 564.60 510.00 20 0 0.000 0.000
942 bef6be20 | 10 39.00 -210.00 1399.25 0 0 0.000 0.000 | 10 0.00 564.60 510.00 20 0 0.000 0.000
948 bef6be20 | 10 45.00 -210.00 1399.25 0 0 0.000 0.000 | 10 6.00 564.60 510.00 20 0 0.000 0.000
954 bef6be20 | 10 51.00 -210.00 1399.25 0 0 0.000 0.000 | 10 12.00 564.60 510.00 20 0 0.000 0.000
960 bef6be20 | 10 57.00 -210.00 1399.25 0 0 0.000 0.000 | 10 18.00 564.60 510.00 20 0 0.000 0.000
966 bef6be20 | 10 63.00 -210.00 1399.25 0 0 0.000 0.000 | 10 24.00 564.60 510.00 20 0 0.000 0.000
972 bef6be20 | 10 69.00 -210.00 1399.25 0 0 0.000 0.000 | 10 30.00 564.60 510.00 20 0 0.000 0.000
978 bef6be20 | 10 75.00 -210.00 1399.25 0 0 0.000 0.000 | 10 36.00 564.60 510.00 20 0 0.000 0.000
984 bef6be20 | 10 1.00 -210.00 1399.25 0 0 0.000 0.000 | 10 42.00 564.60 510.00 20 0 0.000 0.000
990 bef6be20 | 10 7.00 -210.00 1399.25 0 0 0.000 0.000 | 10 48.00 564.60 510.00 20 0 0.000 0.000
996 bef6be20 | 10 13.00 -210.00 1399.25 0 0 0.000 0.000 | 10 4.00 564.60 510.00 20 0 0.000 0.000
1002 bef6be20 | 10 19.00 -210.00 1399.25 0 0 0.000 0.000 | 10 10.00 564.60 510.00 20 0 0.000 0.000
1008 bef6be20 | 10 25.00 -210.00 1399.25 0 0 0.000 0.000 | 10 16.00 564.60 510.00 20 0 0.000 0.000
1014 bef6be20 | 10 31.00 -210.00 1399.25 0 0 0.000 0.000 | 10 22.00 564.60 510.00 20 0 0.000 0.000
1020 bef6be20 | 10 37.00 -210.00 1399.25 0 0 0.000 0.000 | 10 28.00 564.60 510.00 20 0 0.000 0.000
1026 bef6be20 | 10 43.00 -210.00 1399.25 0 0 0.000 0.000 | 10 34.00 564.60 510.00 20 0 0.000 0.000
1032 bef6be20 | 10 49.00 -210.00 1399.25 0 0 0.000 0.000 | 10 40.00 564.60 510.00 20 0 0.000 0.000
1038 bef6be20 | 10 55.00 -210.00 1399.25 0 0 0.000 0.000 | 10 46.00 564.60 510.00 20 0 0.000 0.000
1044 bef6be20 | 10 61.00 -210.00 1399.25 0 0 0.000 0.000 | 10 2.00 564.60 510.00 20 0 0.000 0.000
1050 bef6be20 | 10 67.00 -210.00 1399.25 0 0 0.000 0.000 | 10 8.00 564.60 510.00 20 0 0.000 0.000
1056 bef6be20 | 10 73.00 -210.00 1399.25 0 0 0.000 0.000 | 10 14.00 564.60 510.00 20 0 0.000 0.000
1062 bef6be20 | 10 79.00 -210.00 1399.25 0 0 0.000 0.000 | 10 20.00 564.60 510.00 20 0 0.000 0.000
1068 bef6be20 | 10 5.00 -210.00 1399.25 0 0 0.000 0.000 | 10 26.00 564.60 510.00 20 0 0.000 0.000
1074 bef6be20 | 10 11.00 -210.00 1399.25 0 0 0.000 0.000 | 10 32.00 564.60 510.00 20 0 0.000 0.000
1080 bef6be20 | 10 17.00 -210.00 1399.25 0 0 0.000 0.000 | 10 38.00 564.60 510.00 20 0 0.000 0.000
1086 bef6be20 | 10 23.00 -210.00 1399.25 0 0 0.000 0.000 | 10 44.00 564.60 510.00 20 0 0.000 0.000
1092 bef6be20 | 10 29.00 -210.00 1399.25 0 0 0.000 0.000 | 10 0.00 564.60 510.00 20 0 0.000 0.000
1098 bef6be20 | 10 35.00 -210.00 1399.25 0 0 0.000 0.000 | 10 6.00 564.60 510.00 20 0 0.000 0.000
1100 bef6be20 | 10 37.00 -210.00 1399.25 0 0 0.000 0.000 | 10 8.00 564.60 510.00 20 0 0.000 0.000
";

#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn a_forced_blastoise_sprays_as_on_the_n64() {
    forced_ball(FORCED_BLASTOISE, 7, "nITKindKamex from a forced Poké Ball");
}

/// `b36poke8` (`warp_link`, [`FORCED_BALL_ROUTE`],
/// `dITManagerForceMonsterKind` = 8 poked at frame 2): `nITKindMLucky`.
/// Chansey (no eggs: Training turns every item off). Rows 396-1100: every 6th frame, the seed's changes, and
/// every frame round a hit; the dummy is Training's CPU (its getup at
/// 630).
const FORCED_CHANSEY: &str = "
210 69521988
211 a6e9c9a6
212 fe87df14
213 c40df552
327 491c86cd
332 4db793af
333 074d495a
396 074d495a | 10 53.00 -210.00 1399.25 0 0 0.000 0.000 | 57 5.00 2369.91 1575.09 20 0 0.000 0.000 | 19 385.58 1507.32 0.000 0.000
402 074d495a | 10 59.00 -210.00 1399.25 0 0 0.000 0.000 | 57 11.00 2369.91 1311.09 20 0 0.000 0.000 | 19 385.58 1507.32 0.000 0.000
408 074d495a | 10 65.00 -210.00 1399.25 0 0 0.000 0.000 | 57 17.00 2369.91 1047.09 20 0 0.000 0.000 | 39 385.58 1803.32 0.000 16.000
414 074d495a | 10 71.00 -210.00 1399.25 0 0 0.000 0.000 | 57 23.00 2369.91 783.09 20 0 0.000 0.000 | 39 385.58 1899.32 0.000 16.000
420 074d495a | 10 77.00 -210.00 1399.25 0 0 0.000 0.000 | 57 0.00 2369.91 519.09 20 0 0.000 0.000 | 39 385.58 1995.32 0.000 16.000
426 074d495a | 10 3.00 -210.00 1399.25 0 0 0.000 0.000 | 57 6.00 2369.91 255.09 20 0 0.000 0.000 | 39 385.58 2058.32 0.000 -1.000
431 4dc8aab5
432 4dc8aab5 | 10 9.00 -210.00 1399.25 0 0 0.000 0.000 | 57 12.00 2369.91 -8.91 20 0 0.000 0.000 | 39 385.58 2031.32 0.000 -7.000
438 4dc8aab5 | 10 15.00 -210.00 1399.25 0 0 0.000 0.000 | 67 3.00 2359.10 -101.00 20 0 0.000 0.000 | 39 385.58 1968.32 0.000 -13.000
444 4dc8aab5 | 10 21.00 -210.00 1399.25 0 0 0.000 0.000 | 67 9.00 2337.48 -101.00 20 0 0.000 0.000 | 39 385.58 1869.32 0.000 -19.000
450 4dc8aab5 | 10 27.00 -210.00 1399.25 0 0 0.000 0.000 | 67 15.00 2315.86 -101.00 20 0 0.000 0.000 | 39 385.58 1734.32 0.000 -25.000
456 4dc8aab5 | 10 33.00 -210.00 1399.25 0 0 0.000 0.000 | 67 21.00 2294.23 -101.00 20 0 0.000 0.000 | 39 385.58 1598.32 0.000 0.000
462 4dc8aab5 | 10 39.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2272.61 -101.00 20 0 0.000 0.000 | 39 385.58 1598.32 0.000 0.000
468 4dc8aab5 | 10 45.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2250.99 -101.00 20 0 0.000 0.000 | 39 385.58 1598.32 0.000 0.000
474 4dc8aab5 | 10 51.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2229.37 -101.00 20 0 0.000 0.000 | 39 385.58 1598.32 0.000 0.000
480 4dc8aab5 | 10 57.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2207.75 -101.00 20 0 0.000 0.000 | 39 385.58 1598.32 0.000 0.000
486 4dc8aab5 | 10 63.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2186.13 -101.00 20 0 0.000 0.000 | 39 385.58 1598.32 0.000 0.000
492 4dc8aab5 | 10 69.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2164.51 -101.00 20 0 0.000 0.000 | 39 385.58 1598.32 0.000 0.000
498 4dc8aab5 | 10 75.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2142.89 -101.00 20 0 0.000 0.000 | 39 385.58 1598.32 0.000 0.000
504 4dc8aab5 | 10 1.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2121.27 -101.00 20 0 0.000 0.000 | 39 385.58 1598.32 0.000 0.000
510 4dc8aab5 | 10 7.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2099.65 -101.00 20 0 0.000 0.000 | 39 385.58 1598.32 0.000 0.000
516 4dc8aab5 | 10 13.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2078.02 -101.00 20 0 0.000 0.000 | 39 385.58 1598.32 0.000 0.000
522 4dc8aab5 | 10 19.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2056.40 -101.00 20 0 0.000 0.000 | 39 385.58 1598.32 0.000 0.000
528 4dc8aab5 | 10 25.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2034.78 -101.00 20 0 0.000 0.000 | 39 385.58 1598.32 0.000 0.000
534 4dc8aab5 | 10 31.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2013.16 -101.00 20 0 0.000 0.000 | 39 385.58 1598.32 0.000 0.000
540 4dc8aab5 | 10 37.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1991.54 -101.00 20 0 0.000 0.000 | 39 385.58 1598.32 0.000 0.000
546 4dc8aab5 | 10 43.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1969.92 -101.00 20 0 0.000 0.000 | 39 385.58 1598.32 0.000 0.000
552 4dc8aab5 | 10 49.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1948.29 -101.00 20 0 0.000 0.000 | 39 385.58 1598.32 0.000 0.000
558 4dc8aab5 | 10 55.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1926.67 -101.00 20 0 0.000 0.000 | 39 385.58 1598.32 0.000 0.000
564 4dc8aab5 | 10 61.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1905.05 -101.00 20 0 0.000 0.000 | 39 385.58 1598.32 0.000 0.000
570 4dc8aab5 | 10 67.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1883.43 -101.00 20 0 0.000 0.000 | 39 385.58 1598.32 0.000 0.000
576 4dc8aab5 | 10 73.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1861.81 -101.00 20 0 0.000 0.000 | 39 385.58 1598.32 0.000 0.000
582 4dc8aab5 | 10 79.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1840.19 -101.00 20 0 0.000 0.000 | 39 385.58 1598.32 0.000 0.000
588 4dc8aab5 | 10 5.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1818.57 -101.00 20 0 0.000 0.000 | 39 385.58 1598.32 0.000 0.000
594 4dc8aab5 | 10 11.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1796.94 -101.00 20 0 0.000 0.000 | 39 385.58 1598.32 0.000 0.000
600 4dc8aab5 | 10 17.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1775.32 -101.00 20 0 0.000 0.000 | 39 385.58 1598.32 0.000 0.000
606 4dc8aab5 | 10 23.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1753.70 -101.00 20 0 0.000 0.000 | 39 385.58 1598.32 0.000 0.000
612 4dc8aab5 | 10 29.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1732.08 -101.00 20 0 0.000 0.000 | 39 385.58 1598.32 0.000 0.000
613 6443b2a4
618 6443b2a4 | 10 35.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1710.46 -101.00 20 0 0.000 0.000 | 39 385.58 1598.32 0.000 0.000
624 6443b2a4 | 10 41.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1688.84 -101.00 20 0 0.000 0.000 | 39 385.58 1598.32 0.000 0.000
630 6443b2a4 | 10 47.00 -210.00 1399.25 0 0 0.000 0.000 | 79 2.00 1667.21 -101.00 20 0 0.000 0.000 | 39 385.58 1598.32 0.000 0.000
636 6443b2a4 | 10 53.00 -210.00 1399.25 0 0 0.000 0.000 | 79 8.00 1645.59 -101.00 20 0 0.000 0.000 | 39 385.58 1598.32 0.000 0.000
642 6443b2a4 | 10 59.00 -210.00 1399.25 0 0 0.000 0.000 | 79 14.00 1623.97 -101.00 20 0 0.000 0.000
648 6443b2a4 | 10 65.00 -210.00 1399.25 0 0 0.000 0.000 | 79 20.00 1602.35 -101.00 20 0 0.000 0.000
654 6443b2a4 | 10 71.00 -210.00 1399.25 0 0 0.000 0.000 | 79 26.00 1580.73 -101.00 20 0 0.000 0.000
660 6443b2a4 | 10 77.00 -210.00 1399.25 0 0 0.000 0.000 | 79 32.00 1559.11 -101.00 20 0 0.000 0.000
666 6443b2a4 | 10 3.00 -210.00 1399.25 0 0 0.000 0.000 | 79 38.00 1537.48 -101.00 20 0 0.000 0.000
672 6443b2a4 | 10 9.00 -210.00 1399.25 0 0 0.000 0.000 | 79 44.00 1515.86 -101.00 20 0 0.000 0.000
678 6443b2a4 | 10 15.00 -210.00 1399.25 0 0 0.000 0.000 | 10 0.00 1494.24 -101.00 20 0 0.000 0.000
684 6443b2a4 | 10 21.00 -210.00 1399.25 0 0 0.000 0.000 | 10 6.00 1472.62 -101.00 20 0 0.000 0.000
690 6443b2a4 | 10 27.00 -210.00 1399.25 0 0 0.000 0.000 | 10 12.00 1451.00 -101.00 20 0 0.000 0.000
696 6443b2a4 | 10 33.00 -210.00 1399.25 0 0 0.000 0.000 | 10 18.00 1429.38 -101.00 20 0 0.000 0.000
702 6443b2a4 | 10 39.00 -210.00 1399.25 0 0 0.000 0.000 | 10 24.00 1407.75 -101.00 20 0 0.000 0.000
708 6443b2a4 | 10 45.00 -210.00 1399.25 0 0 0.000 0.000 | 10 30.00 1386.13 -101.00 20 0 0.000 0.000
714 6443b2a4 | 10 51.00 -210.00 1399.25 0 0 0.000 0.000 | 10 36.00 1364.51 -101.00 20 0 0.000 0.000
720 6443b2a4 | 10 57.00 -210.00 1399.25 0 0 0.000 0.000 | 10 42.00 1342.89 -101.00 20 0 0.000 0.000
726 6443b2a4 | 10 63.00 -210.00 1399.25 0 0 0.000 0.000 | 10 48.00 1321.27 -101.00 20 0 0.000 0.000
732 6443b2a4 | 10 69.00 -210.00 1399.25 0 0 0.000 0.000 | 10 4.00 1320.00 -101.00 20 0 0.000 0.000
738 6443b2a4 | 10 75.00 -210.00 1399.25 0 0 0.000 0.000 | 10 10.00 1320.00 -101.00 20 0 0.000 0.000
744 6443b2a4 | 10 1.00 -210.00 1399.25 0 0 0.000 0.000 | 10 16.00 1320.00 -101.00 20 0 0.000 0.000
750 6443b2a4 | 10 7.00 -210.00 1399.25 0 0 0.000 0.000 | 10 22.00 1320.00 -101.00 20 0 0.000 0.000
756 6443b2a4 | 10 13.00 -210.00 1399.25 0 0 0.000 0.000 | 10 28.00 1320.00 -101.00 20 0 0.000 0.000
762 6443b2a4 | 10 19.00 -210.00 1399.25 0 0 0.000 0.000 | 10 34.00 1320.00 -101.00 20 0 0.000 0.000
768 6443b2a4 | 10 25.00 -210.00 1399.25 0 0 0.000 0.000 | 10 40.00 1320.00 -101.00 20 0 0.000 0.000
774 6443b2a4 | 10 31.00 -210.00 1399.25 0 0 0.000 0.000 | 10 46.00 1320.00 -101.00 20 0 0.000 0.000
780 6443b2a4 | 10 37.00 -210.00 1399.25 0 0 0.000 0.000 | 10 2.00 1320.00 -101.00 20 0 0.000 0.000
785 dd038acb
786 dd038acb | 10 43.00 -210.00 1399.25 0 0 0.000 0.000 | 25 1.00 1320.00 32.20 20 0 0.000 0.000
791 bb25e9c6
792 bb25e9c6 | 10 49.00 -210.00 1399.25 0 0 0.000 0.000 | 25 7.00 1240.80 395.80 20 0 0.000 0.000
798 bb25e9c6 | 10 55.00 -210.00 1399.25 0 0 0.000 0.000 | 25 13.00 1125.00 673.00 20 0 0.000 0.000
804 bb25e9c6 | 10 61.00 -210.00 1399.25 0 0 0.000 0.000 | 25 19.00 1016.40 863.80 20 0 0.000 0.000
810 bb25e9c6 | 10 67.00 -210.00 1399.25 0 0 0.000 0.000 | 25 25.00 915.00 968.20 20 0 0.000 0.000
816 bb25e9c6 | 10 73.00 -210.00 1399.25 0 0 0.000 0.000 | 25 31.00 820.80 986.20 20 0 0.000 0.000
822 bb25e9c6 | 10 79.00 -210.00 1399.25 0 0 0.000 0.000 | 25 37.00 733.80 917.80 20 0 0.000 0.000
828 bb25e9c6 | 10 5.00 -210.00 1399.25 0 0 0.000 0.000 | 25 43.00 654.00 763.00 20 0 0.000 0.000
834 bb25e9c6 | 10 11.00 -210.00 1399.25 0 0 0.000 0.000 | 25 49.00 581.40 524.00 20 0 0.000 0.000
835 ee5d3670
840 ee5d3670 | 10 17.00 -210.00 1399.25 0 0 0.000 0.000 | 31 5.00 564.60 510.00 20 0 0.000 0.000
846 ee5d3670 | 10 23.00 -210.00 1399.25 0 0 0.000 0.000 | 10 4.00 564.60 510.00 20 0 0.000 0.000
852 ee5d3670 | 10 29.00 -210.00 1399.25 0 0 0.000 0.000 | 10 10.00 564.60 510.00 20 0 0.000 0.000
858 ee5d3670 | 10 35.00 -210.00 1399.25 0 0 0.000 0.000 | 10 16.00 564.60 510.00 20 0 0.000 0.000
864 ee5d3670 | 10 41.00 -210.00 1399.25 0 0 0.000 0.000 | 10 22.00 564.60 510.00 20 0 0.000 0.000
870 ee5d3670 | 10 47.00 -210.00 1399.25 0 0 0.000 0.000 | 10 28.00 564.60 510.00 20 0 0.000 0.000
876 ee5d3670 | 10 53.00 -210.00 1399.25 0 0 0.000 0.000 | 10 34.00 564.60 510.00 20 0 0.000 0.000
882 ee5d3670 | 10 59.00 -210.00 1399.25 0 0 0.000 0.000 | 10 40.00 564.60 510.00 20 0 0.000 0.000
888 ee5d3670 | 10 65.00 -210.00 1399.25 0 0 0.000 0.000 | 10 46.00 564.60 510.00 20 0 0.000 0.000
894 ee5d3670 | 10 71.00 -210.00 1399.25 0 0 0.000 0.000 | 10 2.00 564.60 510.00 20 0 0.000 0.000
900 ee5d3670 | 10 77.00 -210.00 1399.25 0 0 0.000 0.000 | 10 8.00 564.60 510.00 20 0 0.000 0.000
906 ee5d3670 | 10 3.00 -210.00 1399.25 0 0 0.000 0.000 | 10 14.00 564.60 510.00 20 0 0.000 0.000
912 ee5d3670 | 10 9.00 -210.00 1399.25 0 0 0.000 0.000 | 10 20.00 564.60 510.00 20 0 0.000 0.000
918 ee5d3670 | 10 15.00 -210.00 1399.25 0 0 0.000 0.000 | 10 26.00 564.60 510.00 20 0 0.000 0.000
924 ee5d3670 | 10 21.00 -210.00 1399.25 0 0 0.000 0.000 | 10 32.00 564.60 510.00 20 0 0.000 0.000
930 ee5d3670 | 10 27.00 -210.00 1399.25 0 0 0.000 0.000 | 10 38.00 564.60 510.00 20 0 0.000 0.000
936 ee5d3670 | 10 33.00 -210.00 1399.25 0 0 0.000 0.000 | 10 44.00 564.60 510.00 20 0 0.000 0.000
942 ee5d3670 | 10 39.00 -210.00 1399.25 0 0 0.000 0.000 | 10 0.00 564.60 510.00 20 0 0.000 0.000
948 ee5d3670 | 10 45.00 -210.00 1399.25 0 0 0.000 0.000 | 10 6.00 564.60 510.00 20 0 0.000 0.000
954 ee5d3670 | 10 51.00 -210.00 1399.25 0 0 0.000 0.000 | 10 12.00 564.60 510.00 20 0 0.000 0.000
960 ee5d3670 | 10 57.00 -210.00 1399.25 0 0 0.000 0.000 | 10 18.00 564.60 510.00 20 0 0.000 0.000
966 ee5d3670 | 10 63.00 -210.00 1399.25 0 0 0.000 0.000 | 10 24.00 564.60 510.00 20 0 0.000 0.000
972 ee5d3670 | 10 69.00 -210.00 1399.25 0 0 0.000 0.000 | 10 30.00 564.60 510.00 20 0 0.000 0.000
978 ee5d3670 | 10 75.00 -210.00 1399.25 0 0 0.000 0.000 | 10 36.00 564.60 510.00 20 0 0.000 0.000
984 ee5d3670 | 10 1.00 -210.00 1399.25 0 0 0.000 0.000 | 10 42.00 564.60 510.00 20 0 0.000 0.000
990 ee5d3670 | 10 7.00 -210.00 1399.25 0 0 0.000 0.000 | 10 48.00 564.60 510.00 20 0 0.000 0.000
996 ee5d3670 | 10 13.00 -210.00 1399.25 0 0 0.000 0.000 | 10 4.00 564.60 510.00 20 0 0.000 0.000
1002 ee5d3670 | 10 19.00 -210.00 1399.25 0 0 0.000 0.000 | 10 10.00 564.60 510.00 20 0 0.000 0.000
1008 ee5d3670 | 10 25.00 -210.00 1399.25 0 0 0.000 0.000 | 10 16.00 564.60 510.00 20 0 0.000 0.000
1014 ee5d3670 | 10 31.00 -210.00 1399.25 0 0 0.000 0.000 | 10 22.00 564.60 510.00 20 0 0.000 0.000
1020 ee5d3670 | 10 37.00 -210.00 1399.25 0 0 0.000 0.000 | 10 28.00 564.60 510.00 20 0 0.000 0.000
1026 ee5d3670 | 10 43.00 -210.00 1399.25 0 0 0.000 0.000 | 10 34.00 564.60 510.00 20 0 0.000 0.000
1032 ee5d3670 | 10 49.00 -210.00 1399.25 0 0 0.000 0.000 | 10 40.00 564.60 510.00 20 0 0.000 0.000
1038 ee5d3670 | 10 55.00 -210.00 1399.25 0 0 0.000 0.000 | 10 46.00 564.60 510.00 20 0 0.000 0.000
1044 ee5d3670 | 10 61.00 -210.00 1399.25 0 0 0.000 0.000 | 10 2.00 564.60 510.00 20 0 0.000 0.000
1050 ee5d3670 | 10 67.00 -210.00 1399.25 0 0 0.000 0.000 | 10 8.00 564.60 510.00 20 0 0.000 0.000
1056 ee5d3670 | 10 73.00 -210.00 1399.25 0 0 0.000 0.000 | 10 14.00 564.60 510.00 20 0 0.000 0.000
1062 ee5d3670 | 10 79.00 -210.00 1399.25 0 0 0.000 0.000 | 10 20.00 564.60 510.00 20 0 0.000 0.000
1068 ee5d3670 | 10 5.00 -210.00 1399.25 0 0 0.000 0.000 | 10 26.00 564.60 510.00 20 0 0.000 0.000
1074 ee5d3670 | 10 11.00 -210.00 1399.25 0 0 0.000 0.000 | 10 32.00 564.60 510.00 20 0 0.000 0.000
1080 ee5d3670 | 10 17.00 -210.00 1399.25 0 0 0.000 0.000 | 10 38.00 564.60 510.00 20 0 0.000 0.000
1086 ee5d3670 | 10 23.00 -210.00 1399.25 0 0 0.000 0.000 | 10 44.00 564.60 510.00 20 0 0.000 0.000
1092 ee5d3670 | 10 29.00 -210.00 1399.25 0 0 0.000 0.000 | 10 0.00 564.60 510.00 20 0 0.000 0.000
1098 ee5d3670 | 10 35.00 -210.00 1399.25 0 0 0.000 0.000 | 10 6.00 564.60 510.00 20 0 0.000 0.000
1100 ee5d3670 | 10 37.00 -210.00 1399.25 0 0 0.000 0.000 | 10 8.00 564.60 510.00 20 0 0.000 0.000
";

#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn a_forced_chansey_comes_and_goes_as_on_the_n64() {
    forced_ball(FORCED_CHANSEY, 8, "nITKindMLucky from a forced Poké Ball");
}

/// `b36poke9` (`warp_link`, [`FORCED_BALL_ROUTE`],
/// `dITManagerForceMonsterKind` = 9 poked at frame 2): `nITKindStarmie`.
/// Starmie fires Swift from 531 (`nWPKindStarmieSwift`). Rows 396-1100: every 6th frame, the seed's changes, and
/// every frame round a hit; the dummy is Training's CPU (its getup at
/// 630).
const FORCED_STARMIE: &str = "
210 69521988
211 a6e9c9a6
212 fe87df14
213 c40df552
327 491c86cd
332 4db793af
333 074d495a
396 074d495a | 10 53.00 -210.00 1399.25 0 0 0.000 0.000 | 57 5.00 2369.91 1575.09 20 0 0.000 0.000 | 19 385.58 1507.32 0.000 0.000
402 074d495a | 10 59.00 -210.00 1399.25 0 0 0.000 0.000 | 57 11.00 2369.91 1311.09 20 0 0.000 0.000 | 19 385.58 1507.32 0.000 0.000
408 074d495a | 10 65.00 -210.00 1399.25 0 0 0.000 0.000 | 57 17.00 2369.91 1047.09 20 0 0.000 0.000 | 40 385.58 1823.32 0.000 16.000
414 074d495a | 10 71.00 -210.00 1399.25 0 0 0.000 0.000 | 57 23.00 2369.91 783.09 20 0 0.000 0.000 | 40 385.58 1919.32 0.000 16.000
420 074d495a | 10 77.00 -210.00 1399.25 0 0 0.000 0.000 | 57 0.00 2369.91 519.09 20 0 0.000 0.000 | 40 385.58 2015.32 0.000 16.000
426 074d495a | 10 3.00 -210.00 1399.25 0 0 0.000 0.000 | 57 6.00 2369.91 255.09 20 0 0.000 0.000 | 40 412.93 2050.14 13.677 -14.592
431 4dc8aab5
432 4dc8aab5 | 10 9.00 -210.00 1399.25 0 0 0.000 0.000 | 57 12.00 2369.91 -8.91 20 0 0.000 0.000 | 40 495.00 1962.59 13.677 -14.592
438 4dc8aab5 | 10 15.00 -210.00 1399.25 0 0 0.000 0.000 | 67 3.00 2359.10 -101.00 20 0 0.000 0.000 | 40 577.06 1875.04 13.677 -14.592
444 4dc8aab5 | 10 21.00 -210.00 1399.25 0 0 0.000 0.000 | 67 9.00 2337.48 -101.00 20 0 0.000 0.000 | 40 659.13 1787.48 13.677 -14.592
450 4dc8aab5 | 10 27.00 -210.00 1399.25 0 0 0.000 0.000 | 67 15.00 2315.86 -101.00 20 0 0.000 0.000 | 40 741.19 1699.93 13.677 -14.592
456 4dc8aab5 | 10 33.00 -210.00 1399.25 0 0 0.000 0.000 | 67 21.00 2294.23 -101.00 20 0 0.000 0.000 | 40 823.26 1612.38 13.677 -14.592
462 4dc8aab5 | 10 39.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2272.61 -101.00 20 0 0.000 0.000 | 40 905.32 1524.83 13.677 -14.592
468 4dc8aab5 | 10 45.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2250.99 -101.00 20 0 0.000 0.000 | 40 987.39 1437.27 13.677 -14.592
474 4dc8aab5 | 10 51.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2229.37 -101.00 20 0 0.000 0.000 | 40 1069.45 1349.72 13.677 -14.592
480 4dc8aab5 | 10 57.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2207.75 -101.00 20 0 0.000 0.000 | 40 1151.52 1262.17 13.677 -14.592
486 4dc8aab5 | 10 63.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2186.13 -101.00 20 0 0.000 0.000 | 40 1233.58 1174.62 13.677 -14.592
492 4dc8aab5 | 10 69.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2164.51 -101.00 20 0 0.000 0.000 | 40 1315.65 1087.07 13.677 -14.592
498 4dc8aab5 | 10 75.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2142.89 -101.00 20 0 0.000 0.000 | 40 1397.71 999.51 13.677 -14.592
504 4dc8aab5 | 10 1.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2121.27 -101.00 20 0 0.000 0.000 | 40 1479.78 911.96 13.677 -14.592
510 4dc8aab5 | 10 7.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2099.65 -101.00 20 0 0.000 0.000 | 40 1561.84 824.41 13.677 -14.592
516 4dc8aab5 | 10 13.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2078.02 -101.00 20 0 0.000 0.000 | 40 1643.90 736.86 13.677 -14.592
522 4dc8aab5 | 10 19.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2056.40 -101.00 20 0 0.000 0.000 | 40 1725.97 649.31 13.677 -14.592
528 4dc8aab5 | 10 25.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2034.78 -101.00 20 0 0.000 0.000 | 40 1808.03 561.75 13.677 -14.592
531 6443b2a4
534 6443b2a4 | 10 31.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2013.16 -101.00 20 0 0.000 0.000 | 40 1641.71 547.16 -30.000 0.000 | 28 2621.71 647.16 150.000 0.000
540 6443b2a4 | 10 37.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1991.54 -101.00 20 0 0.000 0.000 | 40 1671.71 547.16 30.000 0.000 | 28 3521.71 647.16 150.000 0.000
543 e6bb16d7
546 e6bb16d7 | 10 43.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1969.92 -101.00 20 0 0.000 0.000 | 40 1581.71 547.16 -30.000 0.000 | 28 4421.71 647.16 150.000 0.000 ; 28 2561.71 647.16 150.000 0.000
552 e6bb16d7 | 10 49.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1948.29 -101.00 20 0 0.000 0.000 | 40 1611.71 547.16 30.000 0.000 | 28 5321.71 647.16 150.000 0.000 ; 28 3461.71 647.16 150.000 0.000
555 428b763e
558 428b763e | 10 55.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1926.67 -101.00 20 0 0.000 0.000 | 40 1521.71 547.16 -30.000 0.000 | 28 6221.71 647.16 150.000 0.000 ; 28 4361.71 647.16 150.000 0.000 ; 28 2501.71 647.16 150.000 0.000
564 428b763e | 10 61.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1905.05 -101.00 20 0 0.000 0.000 | 40 1551.71 547.16 30.000 0.000 | 28 5261.71 647.16 150.000 0.000 ; 28 3401.71 647.16 150.000 0.000
567 a6a6b409
570 a6a6b409 | 10 67.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1883.43 -101.00 20 0 0.000 0.000 | 40 1461.71 547.16 -30.000 0.000 | 28 6161.71 647.16 150.000 0.000 ; 28 4301.71 647.16 150.000 0.000 ; 28 2441.71 647.16 150.000 0.000
576 a6a6b409 | 10 73.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1861.81 -101.00 20 0 0.000 0.000 | 40 1491.71 547.16 30.000 0.000 | 28 5201.71 647.16 150.000 0.000 ; 28 3341.71 647.16 150.000 0.000
579 701fe6a8
582 701fe6a8 | 10 79.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1840.19 -101.00 20 0 0.000 0.000 | 40 1401.71 547.16 -30.000 0.000 | 28 6101.71 647.16 150.000 0.000 ; 28 4241.71 647.16 150.000 0.000 ; 28 2381.71 647.16 150.000 0.000
588 701fe6a8 | 10 5.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1818.57 -101.00 20 0 0.000 0.000 | 40 1431.71 547.16 30.000 0.000 | 28 5141.71 647.16 150.000 0.000 ; 28 3281.71 647.16 150.000 0.000
591 dd038acb
594 dd038acb | 10 11.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1796.94 -101.00 20 0 0.000 0.000 | 40 1341.71 547.16 -30.000 0.000 | 28 6041.71 647.16 150.000 0.000 ; 28 4181.71 647.16 150.000 0.000 ; 28 2321.71 647.16 150.000 0.000
600 dd038acb | 10 17.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1775.32 -101.00 20 0 0.000 0.000 | 40 1371.71 547.16 30.000 0.000 | 28 5081.71 647.16 150.000 0.000 ; 28 3221.71 647.16 150.000 0.000
603 fa5aea62
606 fa5aea62 | 10 23.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1753.70 -101.00 20 0 0.000 0.000 | 40 1281.71 547.16 -30.000 0.000 | 28 5981.71 647.16 150.000 0.000 ; 28 4121.71 647.16 150.000 0.000 ; 28 2261.71 647.16 150.000 0.000
612 fa5aea62 | 10 29.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1732.08 -101.00 20 0 0.000 0.000 | 40 1311.71 547.16 30.000 0.000 | 28 5021.71 647.16 150.000 0.000 ; 28 3161.71 647.16 150.000 0.000
613 f67de79d
615 45099bec
618 45099bec | 10 35.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1710.46 -101.00 20 0 0.000 0.000 | 40 1221.71 547.16 -30.000 0.000 | 28 5921.71 647.16 150.000 0.000 ; 28 4061.71 647.16 150.000 0.000 ; 28 2201.71 647.16 150.000 0.000
624 45099bec | 10 41.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1688.84 -101.00 20 0 0.000 0.000 | 40 1251.71 547.16 30.000 0.000 | 28 4961.71 647.16 150.000 0.000 ; 28 3101.71 647.16 150.000 0.000
627 92387aff
630 92387aff | 10 47.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1667.21 -101.00 20 0 0.000 0.000 | 40 1161.71 547.16 -30.000 0.000 | 28 5861.71 647.16 150.000 0.000 ; 28 4001.71 647.16 150.000 0.000 ; 28 2141.71 647.16 150.000 0.000
636 92387aff | 10 53.00 -210.00 1399.25 0 0 0.000 0.000 | 79 1.00 1645.59 -101.00 20 0 0.000 0.000 | 40 1191.71 547.16 30.000 0.000 | 28 4901.71 647.16 150.000 0.000 ; 28 3041.71 647.16 150.000 0.000
639 bb25e9c6
642 bb25e9c6 | 10 59.00 -210.00 1399.25 0 0 0.000 0.000 | 79 7.00 1623.97 -101.00 20 0 0.000 0.000 | 40 1101.71 547.16 -30.000 0.000 | 28 5801.71 647.16 150.000 0.000 ; 28 3941.71 647.16 150.000 0.000 ; 28 2081.71 647.16 150.000 0.000
648 bb25e9c6 | 10 65.00 -210.00 1399.25 0 0 0.000 0.000 | 79 13.00 1602.35 -101.00 20 0 0.000 0.000 | 40 1131.71 547.16 30.000 0.000 | 28 4841.71 647.16 150.000 0.000 ; 28 2981.71 647.16 150.000 0.000
651 9e1f7971
654 9e1f7971 | 10 71.00 -210.00 1399.25 0 0 0.000 0.000 | 79 19.00 1580.73 -101.00 20 0 0.000 0.000 | 40 1041.71 547.16 -30.000 0.000 | 28 5741.71 647.16 150.000 0.000 ; 28 3881.71 647.16 150.000 0.000 ; 28 2021.71 647.16 150.000 0.000
660 9e1f7971 | 10 77.00 -210.00 1399.25 0 0 0.000 0.000 | 79 25.00 1559.11 -101.00 20 0 0.000 0.000 | 40 1071.71 547.16 30.000 0.000 | 28 4781.71 647.16 150.000 0.000 ; 28 2921.71 647.16 150.000 0.000
663 ee5d3670
666 ee5d3670 | 10 3.00 -210.00 1399.25 0 0 0.000 0.000 | 79 31.00 1537.48 -101.00 20 0 0.000 0.000 | 40 981.71 547.16 -30.000 0.000 | 28 5681.71 647.16 150.000 0.000 ; 28 3821.71 647.16 150.000 0.000 ; 28 1961.71 647.16 150.000 0.000
672 ee5d3670 | 10 9.00 -210.00 1399.25 0 0 0.000 0.000 | 79 37.00 1515.86 -101.00 20 0 0.000 0.000 | 40 1011.71 547.16 30.000 0.000 | 28 4721.71 647.16 150.000 0.000 ; 28 2861.71 647.16 150.000 0.000
675 9ad4bb73
678 9ad4bb73 | 10 15.00 -210.00 1399.25 0 0 0.000 0.000 | 79 43.00 1494.24 -101.00 20 0 0.000 0.000 | 40 921.71 547.16 -30.000 0.000 | 28 5621.71 647.16 150.000 0.000 ; 28 3761.71 647.16 150.000 0.000 ; 28 1901.71 647.16 150.000 0.000
684 9ad4bb73 | 10 21.00 -210.00 1399.25 0 0 0.000 0.000 | 79 49.00 1472.62 -101.00 20 0 0.000 0.000 | 40 951.71 547.16 30.000 0.000 | 28 4661.71 647.16 150.000 0.000 ; 28 2801.71 647.16 150.000 0.000
687 e3cbf86a
690 e3cbf86a | 10 27.00 -210.00 1399.25 0 0 0.000 0.000 | 10 5.00 1451.00 -101.00 20 0 0.000 0.000 | 40 861.71 547.16 -30.000 0.000 | 28 5561.71 647.16 150.000 0.000 ; 28 3701.71 647.16 150.000 0.000 ; 28 1841.71 647.16 150.000 0.000
696 e3cbf86a | 10 33.00 -210.00 1399.25 0 0 0.000 0.000 | 10 11.00 1429.38 -101.00 20 0 0.000 0.000 | 40 891.71 547.16 30.000 0.000 | 28 4601.71 647.16 150.000 0.000 ; 28 2741.71 647.16 150.000 0.000
699 6bfcdd85
702 6bfcdd85 | 10 39.00 -210.00 1399.25 0 0 0.000 0.000 | 10 17.00 1407.75 -101.00 20 0 0.000 0.000 | 40 801.71 547.16 -30.000 0.000 | 28 5501.71 647.16 150.000 0.000 ; 28 3641.71 647.16 150.000 0.000 ; 28 1781.71 647.16 150.000 0.000
708 6bfcdd85 | 10 45.00 -210.00 1399.25 0 0 0.000 0.000 | 10 23.00 1386.13 -101.00 20 0 0.000 0.000 | 40 831.71 547.16 30.000 0.000 | 28 4541.71 647.16 150.000 0.000 ; 28 2681.71 647.16 150.000 0.000
711 7f965a34
714 7f965a34 | 10 51.00 -210.00 1399.25 0 0 0.000 0.000 | 10 29.00 1364.51 -101.00 20 0 0.000 0.000 | 40 741.71 547.16 -30.000 0.000 | 28 5441.71 647.16 150.000 0.000 ; 28 3581.71 647.16 150.000 0.000 ; 28 1721.71 647.16 150.000 0.000
720 7f965a34 | 10 57.00 -210.00 1399.25 0 0 0.000 0.000 | 10 35.00 1342.89 -101.00 20 0 0.000 0.000 | 40 771.71 547.16 30.000 0.000 | 28 4481.71 647.16 150.000 0.000 ; 28 2621.71 647.16 150.000 0.000
723 7ff56027
726 7ff56027 | 10 63.00 -210.00 1399.25 0 0 0.000 0.000 | 10 41.00 1321.27 -101.00 20 0 0.000 0.000 | 40 681.71 547.16 -30.000 0.000 | 28 5381.71 647.16 150.000 0.000 ; 28 3521.71 647.16 150.000 0.000 ; 28 1661.71 647.16 150.000 0.000
732 7ff56027 | 10 69.00 -210.00 1399.25 0 0 0.000 0.000 | 10 47.00 1320.00 -101.00 20 0 0.000 0.000 | 40 711.71 547.16 30.000 0.000 | 28 4421.71 647.16 150.000 0.000 ; 28 2561.71 647.16 150.000 0.000
735 ce45da4e
738 ce45da4e | 10 75.00 -210.00 1399.25 0 0 0.000 0.000 | 10 3.00 1320.00 -101.00 20 0 0.000 0.000 | 40 621.71 547.16 -30.000 0.000 | 28 5321.71 647.16 150.000 0.000 ; 28 3461.71 647.16 150.000 0.000 ; 28 1601.71 647.16 150.000 0.000
744 ce45da4e | 10 1.00 -210.00 1399.25 0 0 0.000 0.000 | 10 9.00 1320.00 -101.00 20 0 0.000 0.000 | 40 651.71 547.16 30.000 0.000 | 28 4361.71 647.16 150.000 0.000 ; 28 2501.71 647.16 150.000 0.000
747 b23bc7d9
750 b23bc7d9 | 10 7.00 -210.00 1399.25 0 0 0.000 0.000 | 10 15.00 1320.00 -101.00 20 0 0.000 0.000 | 40 561.71 547.16 -30.000 0.000 | 28 5261.71 647.16 150.000 0.000 ; 28 3401.71 647.16 150.000 0.000 ; 28 1541.71 647.16 150.000 0.000
756 b23bc7d9 | 10 13.00 -210.00 1399.25 0 0 0.000 0.000 | 10 21.00 1320.00 -101.00 20 0 0.000 0.000 | 40 591.71 547.16 30.000 0.000 | 28 4301.71 647.16 150.000 0.000 ; 28 2441.71 647.16 150.000 0.000
759 2213eb38
762 2213eb38 | 10 19.00 -210.00 1399.25 0 0 0.000 0.000 | 10 27.00 1320.00 -101.00 20 0 0.000 0.000 | 40 501.71 547.16 -30.000 0.000 | 28 5201.71 647.16 150.000 0.000 ; 28 3341.71 647.16 150.000 0.000 ; 28 1481.71 647.16 150.000 0.000
768 2213eb38 | 10 25.00 -210.00 1399.25 0 0 0.000 0.000 | 10 33.00 1320.00 -101.00 20 0 0.000 0.000 | 40 531.71 547.16 30.000 0.000 | 28 4241.71 647.16 150.000 0.000 ; 28 2381.71 647.16 150.000 0.000
774 2213eb38 | 10 31.00 -210.00 1399.25 0 0 0.000 0.000 | 10 39.00 1320.00 -101.00 20 0 0.000 0.000 |  | 28 5141.71 647.16 150.000 0.000 ; 28 3281.71 647.16 150.000 0.000
780 2213eb38 | 10 37.00 -210.00 1399.25 0 0 0.000 0.000 | 10 45.00 1320.00 -101.00 20 0 0.000 0.000 |  | 28 4181.71 647.16 150.000 0.000
785 8bdb664f
786 8bdb664f | 10 43.00 -210.00 1399.25 0 0 0.000 0.000 | 25 1.00 1320.00 32.20 20 0 0.000 0.000 |  | 28 5081.71 647.16 150.000 0.000
791 a350db7a
792 a350db7a | 10 49.00 -210.00 1399.25 0 0 0.000 0.000 | 25 7.00 1240.80 395.80 20 0 0.000 0.000
798 a350db7a | 10 55.00 -210.00 1399.25 0 0 0.000 0.000 | 25 13.00 1125.00 673.00 20 0 0.000 0.000
804 a350db7a | 10 61.00 -210.00 1399.25 0 0 0.000 0.000 | 25 19.00 1016.40 863.80 20 0 0.000 0.000
810 a350db7a | 10 67.00 -210.00 1399.25 0 0 0.000 0.000 | 25 25.00 915.00 968.20 20 0 0.000 0.000
816 a350db7a | 10 73.00 -210.00 1399.25 0 0 0.000 0.000 | 25 31.00 820.80 986.20 20 0 0.000 0.000
822 a350db7a | 10 79.00 -210.00 1399.25 0 0 0.000 0.000 | 25 37.00 733.80 917.80 20 0 0.000 0.000
828 a350db7a | 10 5.00 -210.00 1399.25 0 0 0.000 0.000 | 25 43.00 654.00 763.00 20 0 0.000 0.000
834 a350db7a | 10 11.00 -210.00 1399.25 0 0 0.000 0.000 | 25 49.00 581.40 524.00 20 0 0.000 0.000
835 4b40d5c4
840 4b40d5c4 | 10 17.00 -210.00 1399.25 0 0 0.000 0.000 | 31 5.00 564.60 510.00 20 0 0.000 0.000
846 4b40d5c4 | 10 23.00 -210.00 1399.25 0 0 0.000 0.000 | 10 4.00 564.60 510.00 20 0 0.000 0.000
852 4b40d5c4 | 10 29.00 -210.00 1399.25 0 0 0.000 0.000 | 10 10.00 564.60 510.00 20 0 0.000 0.000
858 4b40d5c4 | 10 35.00 -210.00 1399.25 0 0 0.000 0.000 | 10 16.00 564.60 510.00 20 0 0.000 0.000
864 4b40d5c4 | 10 41.00 -210.00 1399.25 0 0 0.000 0.000 | 10 22.00 564.60 510.00 20 0 0.000 0.000
870 4b40d5c4 | 10 47.00 -210.00 1399.25 0 0 0.000 0.000 | 10 28.00 564.60 510.00 20 0 0.000 0.000
876 4b40d5c4 | 10 53.00 -210.00 1399.25 0 0 0.000 0.000 | 10 34.00 564.60 510.00 20 0 0.000 0.000
882 4b40d5c4 | 10 59.00 -210.00 1399.25 0 0 0.000 0.000 | 10 40.00 564.60 510.00 20 0 0.000 0.000
888 4b40d5c4 | 10 65.00 -210.00 1399.25 0 0 0.000 0.000 | 10 46.00 564.60 510.00 20 0 0.000 0.000
894 4b40d5c4 | 10 71.00 -210.00 1399.25 0 0 0.000 0.000 | 10 2.00 564.60 510.00 20 0 0.000 0.000
900 4b40d5c4 | 10 77.00 -210.00 1399.25 0 0 0.000 0.000 | 10 8.00 564.60 510.00 20 0 0.000 0.000
906 4b40d5c4 | 10 3.00 -210.00 1399.25 0 0 0.000 0.000 | 10 14.00 564.60 510.00 20 0 0.000 0.000
912 4b40d5c4 | 10 9.00 -210.00 1399.25 0 0 0.000 0.000 | 10 20.00 564.60 510.00 20 0 0.000 0.000
918 4b40d5c4 | 10 15.00 -210.00 1399.25 0 0 0.000 0.000 | 10 26.00 564.60 510.00 20 0 0.000 0.000
924 4b40d5c4 | 10 21.00 -210.00 1399.25 0 0 0.000 0.000 | 10 32.00 564.60 510.00 20 0 0.000 0.000
930 4b40d5c4 | 10 27.00 -210.00 1399.25 0 0 0.000 0.000 | 10 38.00 564.60 510.00 20 0 0.000 0.000
936 4b40d5c4 | 10 33.00 -210.00 1399.25 0 0 0.000 0.000 | 10 44.00 564.60 510.00 20 0 0.000 0.000
942 4b40d5c4 | 10 39.00 -210.00 1399.25 0 0 0.000 0.000 | 10 0.00 564.60 510.00 20 0 0.000 0.000
948 4b40d5c4 | 10 45.00 -210.00 1399.25 0 0 0.000 0.000 | 10 6.00 564.60 510.00 20 0 0.000 0.000
954 4b40d5c4 | 10 51.00 -210.00 1399.25 0 0 0.000 0.000 | 10 12.00 564.60 510.00 20 0 0.000 0.000
960 4b40d5c4 | 10 57.00 -210.00 1399.25 0 0 0.000 0.000 | 10 18.00 564.60 510.00 20 0 0.000 0.000
966 4b40d5c4 | 10 63.00 -210.00 1399.25 0 0 0.000 0.000 | 10 24.00 564.60 510.00 20 0 0.000 0.000
972 4b40d5c4 | 10 69.00 -210.00 1399.25 0 0 0.000 0.000 | 10 30.00 564.60 510.00 20 0 0.000 0.000
978 4b40d5c4 | 10 75.00 -210.00 1399.25 0 0 0.000 0.000 | 10 36.00 564.60 510.00 20 0 0.000 0.000
984 4b40d5c4 | 10 1.00 -210.00 1399.25 0 0 0.000 0.000 | 10 42.00 564.60 510.00 20 0 0.000 0.000
990 4b40d5c4 | 10 7.00 -210.00 1399.25 0 0 0.000 0.000 | 10 48.00 564.60 510.00 20 0 0.000 0.000
996 4b40d5c4 | 10 13.00 -210.00 1399.25 0 0 0.000 0.000 | 10 4.00 564.60 510.00 20 0 0.000 0.000
1002 4b40d5c4 | 10 19.00 -210.00 1399.25 0 0 0.000 0.000 | 10 10.00 564.60 510.00 20 0 0.000 0.000
1008 4b40d5c4 | 10 25.00 -210.00 1399.25 0 0 0.000 0.000 | 10 16.00 564.60 510.00 20 0 0.000 0.000
1014 4b40d5c4 | 10 31.00 -210.00 1399.25 0 0 0.000 0.000 | 10 22.00 564.60 510.00 20 0 0.000 0.000
1020 4b40d5c4 | 10 37.00 -210.00 1399.25 0 0 0.000 0.000 | 10 28.00 564.60 510.00 20 0 0.000 0.000
1026 4b40d5c4 | 10 43.00 -210.00 1399.25 0 0 0.000 0.000 | 10 34.00 564.60 510.00 20 0 0.000 0.000
1032 4b40d5c4 | 10 49.00 -210.00 1399.25 0 0 0.000 0.000 | 10 40.00 564.60 510.00 20 0 0.000 0.000
1038 4b40d5c4 | 10 55.00 -210.00 1399.25 0 0 0.000 0.000 | 10 46.00 564.60 510.00 20 0 0.000 0.000
1044 4b40d5c4 | 10 61.00 -210.00 1399.25 0 0 0.000 0.000 | 10 2.00 564.60 510.00 20 0 0.000 0.000
1050 4b40d5c4 | 10 67.00 -210.00 1399.25 0 0 0.000 0.000 | 10 8.00 564.60 510.00 20 0 0.000 0.000
1056 4b40d5c4 | 10 73.00 -210.00 1399.25 0 0 0.000 0.000 | 10 14.00 564.60 510.00 20 0 0.000 0.000
1062 4b40d5c4 | 10 79.00 -210.00 1399.25 0 0 0.000 0.000 | 10 20.00 564.60 510.00 20 0 0.000 0.000
1068 4b40d5c4 | 10 5.00 -210.00 1399.25 0 0 0.000 0.000 | 10 26.00 564.60 510.00 20 0 0.000 0.000
1074 4b40d5c4 | 10 11.00 -210.00 1399.25 0 0 0.000 0.000 | 10 32.00 564.60 510.00 20 0 0.000 0.000
1080 4b40d5c4 | 10 17.00 -210.00 1399.25 0 0 0.000 0.000 | 10 38.00 564.60 510.00 20 0 0.000 0.000
1086 4b40d5c4 | 10 23.00 -210.00 1399.25 0 0 0.000 0.000 | 10 44.00 564.60 510.00 20 0 0.000 0.000
1092 4b40d5c4 | 10 29.00 -210.00 1399.25 0 0 0.000 0.000 | 10 0.00 564.60 510.00 20 0 0.000 0.000
1098 4b40d5c4 | 10 35.00 -210.00 1399.25 0 0 0.000 0.000 | 10 6.00 564.60 510.00 20 0 0.000 0.000
1100 4b40d5c4 | 10 37.00 -210.00 1399.25 0 0 0.000 0.000 | 10 8.00 564.60 510.00 20 0 0.000 0.000
";

#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn a_forced_starmie_fires_swift_as_on_the_n64() {
    forced_ball(FORCED_STARMIE, 9, "nITKindStarmie from a forced Poké Ball");
}

/// `b36poke10` (`warp_link`, [`FORCED_BALL_ROUTE`],
/// `dITManagerForceMonsterKind` = 10 poked at frame 2): `nITKindSawamura`.
/// Hitmonlee jumps and kicks the dummy at 500 (20 to 32 percent, 9 frames of hitlag). Rows 396-1100: every 6th frame, the seed's changes, and
/// every frame round a hit; the dummy is Training's CPU (its getup at
/// 630).
const FORCED_HITMONLEE: &str = "
210 69521988
211 a6e9c9a6
212 fe87df14
213 c40df552
327 491c86cd
332 4db793af
333 074d495a
396 074d495a | 10 53.00 -210.00 1399.25 0 0 0.000 0.000 | 57 5.00 2369.91 1575.09 20 0 0.000 0.000 | 19 385.58 1507.32 0.000 0.000
402 074d495a | 10 59.00 -210.00 1399.25 0 0 0.000 0.000 | 57 11.00 2369.91 1311.09 20 0 0.000 0.000 | 19 385.58 1507.32 0.000 0.000
408 074d495a | 10 65.00 -210.00 1399.25 0 0 0.000 0.000 | 57 17.00 2369.91 1047.09 20 0 0.000 0.000 | 41 385.58 1920.32 0.000 16.000
414 074d495a | 10 71.00 -210.00 1399.25 0 0 0.000 0.000 | 57 23.00 2369.91 783.09 20 0 0.000 0.000 | 41 385.58 2016.32 0.000 16.000
420 074d495a | 10 77.00 -210.00 1399.25 0 0 0.000 0.000 | 57 0.00 2369.91 519.09 20 0 0.000 0.000 | 41 385.58 2112.32 0.000 16.000
426 074d495a | 10 3.00 -210.00 1399.25 0 0 0.000 0.000 | 57 6.00 2369.91 255.09 20 0 0.000 0.000 | 41 385.58 2173.92 0.000 -2.400
431 4dc8aab5
432 4dc8aab5 | 10 9.00 -210.00 1399.25 0 0 0.000 0.000 | 57 12.00 2369.91 -8.91 20 0 0.000 0.000 | 41 385.58 2109.12 0.000 -16.800
438 4dc8aab5 | 10 15.00 -210.00 1399.25 0 0 0.000 0.000 | 67 3.00 2359.10 -101.00 20 0 0.000 0.000 | 41 385.58 1957.92 0.000 -31.200
444 4dc8aab5 | 10 21.00 -210.00 1399.25 0 0 0.000 0.000 | 67 9.00 2337.48 -101.00 20 0 0.000 0.000 | 41 385.58 1720.32 0.000 -45.600
450 4dc8aab5 | 10 27.00 -210.00 1399.25 0 0 0.000 0.000 | 67 15.00 2315.86 -101.00 20 0 0.000 0.000 | 41 385.58 1715.32 0.000 0.000
456 4dc8aab5 | 10 33.00 -210.00 1399.25 0 0 0.000 0.000 | 67 21.00 2294.23 -101.00 20 0 0.000 0.000 | 41 385.58 1715.32 0.000 0.000
462 4dc8aab5 | 10 39.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2272.61 -101.00 20 0 0.000 0.000 | 41 385.58 1715.32 0.000 0.000
468 4dc8aab5 | 10 45.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2250.99 -101.00 20 0 0.000 0.000 | 41 385.58 1715.32 0.000 0.000
474 4dc8aab5 | 10 51.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2229.37 -101.00 20 0 0.000 0.000 | 41 385.58 1715.32 0.000 0.000
480 4dc8aab5 | 10 57.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2207.75 -101.00 20 0 0.000 0.000 | 41 385.58 1715.32 0.000 0.000
486 4dc8aab5 | 10 63.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2186.13 -101.00 20 0 0.000 0.000 | 41 789.21 1420.23 80.498 -59.330
492 4dc8aab5 | 10 69.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2164.51 -101.00 20 0 0.000 0.000 | 41 1247.82 1033.81 73.494 -67.814
496 4dc8aab5 | 10 73.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2150.09 -101.00 20 0 0.000 0.000 | 41 1529.91 750.31 68.737 -72.630
497 4dc8aab5 | 10 74.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2146.49 -101.00 20 0 0.000 0.000 | 41 1597.46 676.57 67.551 -73.735
498 4dc8aab5 | 10 75.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2142.89 -101.00 20 0 0.000 0.000 | 41 1663.83 601.77 66.368 -74.802
499 4dc8aab5 | 10 76.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2139.28 -101.00 20 0 0.000 0.000 | 41 1729.02 525.94 65.189 -75.831
500 e6bb16d7 | 10 77.00 -210.00 1399.25 0 0 0.000 0.000 | 54 1.00 2135.68 -101.00 32 9 0.000 96.240 | 41 1793.03 449.12 64.017 -76.824
501 fa5aea62 | 10 78.00 -210.00 1399.25 0 0 0.000 0.000 | 54 1.00 2135.68 -101.00 32 8 0.000 96.240 | 41 1855.88 371.34 62.851 -77.780
502 fa5aea62 | 10 79.00 -210.00 1399.25 0 0 0.000 0.000 | 54 1.00 2135.68 -101.00 32 7 0.000 96.240 | 41 1917.58 292.63 61.692 -78.703
503 fa5aea62 | 10 0.00 -210.00 1399.25 0 0 0.000 0.000 | 54 1.00 2135.68 -101.00 32 6 0.000 96.240 | 41 1978.12 213.04 60.542 -79.591
504 fa5aea62 | 10 1.00 -210.00 1399.25 0 0 0.000 0.000 | 54 1.00 2135.68 -101.00 32 5 0.000 96.240 | 41 2037.52 132.60 59.401 -80.446
505 fa5aea62 | 10 2.00 -210.00 1399.25 0 0 0.000 0.000 | 54 1.00 2135.68 -101.00 32 4 0.000 96.240 | 41 2095.79 51.33 58.270 -81.268
506 fa5aea62 | 10 3.00 -210.00 1399.25 0 0 0.000 0.000 | 54 1.00 2135.68 -101.00 32 3 0.000 96.240 | 41 2152.94 -30.73 57.150 -82.060
507 fa5aea62 | 10 4.00 -210.00 1399.25 0 0 0.000 0.000 | 54 1.00 2135.68 -101.00 32 2 0.000 96.240 | 41 2208.98 -113.55 56.041 -82.821
508 fa5aea62 | 10 5.00 -210.00 1399.25 0 0 0.000 0.000 | 54 1.00 2135.68 -101.00 32 1 0.000 96.240 | 41 2263.93 -197.11 54.944 -83.553
509 fa5aea62 | 10 6.00 -210.00 1399.25 0 0 0.000 0.000 | 54 2.00 2135.68 -8.86 32 0 0.000 94.540 | 41 2317.78 -281.36 53.860 -84.256
510 fa5aea62 | 10 7.00 -210.00 1399.25 0 0 0.000 0.000 | 54 3.00 2135.68 79.18 32 0 0.000 92.840 | 41 2370.57 -366.29 52.788 -84.932
516 fa5aea62 | 10 13.00 -210.00 1399.25 0 0 0.000 0.000 | 54 9.00 2135.68 521.32 32 0 0.000 82.640 | 41 2665.56 -888.63 46.648 -88.453
522 fa5aea62 | 10 19.00 -210.00 1399.25 0 0 0.000 0.000 | 54 15.00 2135.68 815.86 32 0 0.000 72.440 | 41 2925.56 -1429.27 41.041 -91.190
528 fa5aea62 | 10 25.00 -210.00 1399.25 0 0 0.000 0.000 | 54 21.00 2135.68 968.40 32 0 0.000 62.240 | 41 3153.84 -1984.07 35.986 -93.301
534 fa5aea62 | 10 31.00 -210.00 1399.25 0 0 0.000 0.000 | 54 27.00 2135.68 1042.14 32 0 0.000 52.040 | 41 3353.69 -2549.75 31.471 -94.919
540 fa5aea62 | 10 37.00 -210.00 1399.25 0 0 0.000 0.000 | 54 33.00 2135.68 1054.68 32 0 0.000 41.840 | 41 3528.27 -3123.75 27.467 -96.154
546 fa5aea62 | 10 43.00 -210.00 1399.25 0 0 0.000 0.000 | 54 39.00 2135.68 1006.02 32 0 0.000 31.640 | 41 3680.50 -3704.08 23.937 -97.093
552 fa5aea62 | 10 49.00 -210.00 1399.25 0 0 0.000 0.000 | 54 45.00 2135.68 896.16 32 0 0.000 21.440
558 fa5aea62 | 10 55.00 -210.00 1399.25 0 0 0.000 0.000 | 54 51.00 2135.68 725.10 32 0 0.000 11.240
564 fa5aea62 | 10 61.00 -210.00 1399.25 0 0 0.000 0.000 | 54 57.00 2097.88 492.84 32 0 0.000 1.040
570 fa5aea62 | 10 67.00 -210.00 1399.25 0 0 0.000 0.000 | 57 3.00 2037.28 228.84 32 0 0.000 0.000
574 f67de79d
576 f67de79d | 10 73.00 -210.00 1399.25 0 0 0.000 0.000 | 57 9.00 1983.88 -35.16 32 0 0.000 0.000
582 f67de79d | 10 79.00 -210.00 1399.25 0 0 0.000 0.000 | 67 4.00 1951.27 -101.00 32 0 0.000 0.000
588 f67de79d | 10 5.00 -210.00 1399.25 0 0 0.000 0.000 | 67 10.00 1929.64 -101.00 32 0 0.000 0.000
594 f67de79d | 10 11.00 -210.00 1399.25 0 0 0.000 0.000 | 67 16.00 1908.02 -101.00 32 0 0.000 0.000
600 f67de79d | 10 17.00 -210.00 1399.25 0 0 0.000 0.000 | 67 22.00 1886.40 -101.00 32 0 0.000 0.000
606 f67de79d | 10 23.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1864.78 -101.00 32 0 0.000 0.000
612 f67de79d | 10 29.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1843.16 -101.00 32 0 0.000 0.000
618 f67de79d | 10 35.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1821.54 -101.00 32 0 0.000 0.000
624 f67de79d | 10 41.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1799.91 -101.00 32 0 0.000 0.000
630 f67de79d | 10 47.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1778.29 -101.00 32 0 0.000 0.000
636 f67de79d | 10 53.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1756.67 -101.00 32 0 0.000 0.000
642 f67de79d | 10 59.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1735.05 -101.00 32 0 0.000 0.000
648 f67de79d | 10 65.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1713.43 -101.00 32 0 0.000 0.000
654 f67de79d | 10 71.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1691.81 -101.00 32 0 0.000 0.000
660 f67de79d | 10 77.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1670.18 -101.00 32 0 0.000 0.000
666 f67de79d | 10 3.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1648.56 -101.00 32 0 0.000 0.000
672 f67de79d | 10 9.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1626.94 -101.00 32 0 0.000 0.000
678 f67de79d | 10 15.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1605.32 -101.00 32 0 0.000 0.000
684 f67de79d | 10 21.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1583.70 -101.00 32 0 0.000 0.000
690 f67de79d | 10 27.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1562.08 -101.00 32 0 0.000 0.000
696 f67de79d | 10 33.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1540.46 -101.00 32 0 0.000 0.000
702 f67de79d | 10 39.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1518.83 -101.00 32 0 0.000 0.000
708 f67de79d | 10 45.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1497.21 -101.00 32 0 0.000 0.000
714 f67de79d | 10 51.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1475.59 -101.00 32 0 0.000 0.000
720 f67de79d | 10 57.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1453.97 -101.00 32 0 0.000 0.000
726 f67de79d | 10 63.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1432.35 -101.00 32 0 0.000 0.000
732 f67de79d | 10 69.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1410.73 -101.00 32 0 0.000 0.000
738 f67de79d | 10 75.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1389.10 -101.00 32 0 0.000 0.000
744 f67de79d | 10 1.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1367.48 -101.00 32 0 0.000 0.000
750 f67de79d | 10 7.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1345.86 -101.00 32 0 0.000 0.000
756 45099bec | 10 13.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1324.24 -101.00 32 0 0.000 0.000
762 45099bec | 10 19.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1320.00 -101.00 32 0 0.000 0.000
768 45099bec | 10 25.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1320.00 -101.00 32 0 0.000 0.000
774 45099bec | 10 31.00 -210.00 1399.25 0 0 0.000 0.000 | 79 4.00 1320.00 -101.00 32 0 0.000 0.000
780 45099bec | 10 37.00 -210.00 1399.25 0 0 0.000 0.000 | 79 10.00 1320.00 -101.00 32 0 0.000 0.000
785 9ad4bb73
786 9ad4bb73 | 10 43.00 -210.00 1399.25 0 0 0.000 0.000 | 24 1.00 1320.00 32.20 32 0 0.000 0.000
791 ce45da4e
792 ce45da4e | 10 49.00 -210.00 1399.25 0 0 0.000 0.000 | 24 7.00 1240.80 395.80 32 0 0.000 0.000
798 ce45da4e | 10 55.00 -210.00 1399.25 0 0 0.000 0.000 | 24 13.00 1125.00 673.00 32 0 0.000 0.000
804 ce45da4e | 10 61.00 -210.00 1399.25 0 0 0.000 0.000 | 24 19.00 1016.40 863.80 32 0 0.000 0.000
810 ce45da4e | 10 67.00 -210.00 1399.25 0 0 0.000 0.000 | 24 25.00 915.00 968.20 32 0 0.000 0.000
816 ce45da4e | 10 73.00 -210.00 1399.25 0 0 0.000 0.000 | 24 31.00 820.80 986.20 32 0 0.000 0.000
822 ce45da4e | 10 79.00 -210.00 1399.25 0 0 0.000 0.000 | 24 37.00 733.80 917.80 32 0 0.000 0.000
828 ce45da4e | 10 5.00 -210.00 1399.25 0 0 0.000 0.000 | 24 43.00 654.00 763.00 32 0 0.000 0.000
834 ce45da4e | 10 11.00 -210.00 1399.25 0 0 0.000 0.000 | 24 49.00 581.40 524.00 32 0 0.000 0.000
835 2213eb38
840 2213eb38 | 10 17.00 -210.00 1399.25 0 0 0.000 0.000 | 31 5.00 564.60 510.00 32 0 0.000 0.000
846 2213eb38 | 10 23.00 -210.00 1399.25 0 0 0.000 0.000 | 10 4.00 564.60 510.00 32 0 0.000 0.000
852 2213eb38 | 10 29.00 -210.00 1399.25 0 0 0.000 0.000 | 10 10.00 564.60 510.00 32 0 0.000 0.000
858 2213eb38 | 10 35.00 -210.00 1399.25 0 0 0.000 0.000 | 10 16.00 564.60 510.00 32 0 0.000 0.000
864 2213eb38 | 10 41.00 -210.00 1399.25 0 0 0.000 0.000 | 10 22.00 564.60 510.00 32 0 0.000 0.000
870 2213eb38 | 10 47.00 -210.00 1399.25 0 0 0.000 0.000 | 10 28.00 564.60 510.00 32 0 0.000 0.000
876 2213eb38 | 10 53.00 -210.00 1399.25 0 0 0.000 0.000 | 10 34.00 564.60 510.00 32 0 0.000 0.000
882 2213eb38 | 10 59.00 -210.00 1399.25 0 0 0.000 0.000 | 10 40.00 564.60 510.00 32 0 0.000 0.000
888 2213eb38 | 10 65.00 -210.00 1399.25 0 0 0.000 0.000 | 10 46.00 564.60 510.00 32 0 0.000 0.000
894 2213eb38 | 10 71.00 -210.00 1399.25 0 0 0.000 0.000 | 10 2.00 564.60 510.00 32 0 0.000 0.000
900 2213eb38 | 10 77.00 -210.00 1399.25 0 0 0.000 0.000 | 10 8.00 564.60 510.00 32 0 0.000 0.000
906 2213eb38 | 10 3.00 -210.00 1399.25 0 0 0.000 0.000 | 10 14.00 564.60 510.00 32 0 0.000 0.000
912 2213eb38 | 10 9.00 -210.00 1399.25 0 0 0.000 0.000 | 10 20.00 564.60 510.00 32 0 0.000 0.000
918 2213eb38 | 10 15.00 -210.00 1399.25 0 0 0.000 0.000 | 10 26.00 564.60 510.00 32 0 0.000 0.000
924 2213eb38 | 10 21.00 -210.00 1399.25 0 0 0.000 0.000 | 10 32.00 564.60 510.00 32 0 0.000 0.000
930 2213eb38 | 10 27.00 -210.00 1399.25 0 0 0.000 0.000 | 10 38.00 564.60 510.00 32 0 0.000 0.000
936 2213eb38 | 10 33.00 -210.00 1399.25 0 0 0.000 0.000 | 10 44.00 564.60 510.00 32 0 0.000 0.000
942 2213eb38 | 10 39.00 -210.00 1399.25 0 0 0.000 0.000 | 10 0.00 564.60 510.00 32 0 0.000 0.000
948 2213eb38 | 10 45.00 -210.00 1399.25 0 0 0.000 0.000 | 10 6.00 564.60 510.00 32 0 0.000 0.000
954 2213eb38 | 10 51.00 -210.00 1399.25 0 0 0.000 0.000 | 10 12.00 564.60 510.00 32 0 0.000 0.000
960 2213eb38 | 10 57.00 -210.00 1399.25 0 0 0.000 0.000 | 10 18.00 564.60 510.00 32 0 0.000 0.000
966 2213eb38 | 10 63.00 -210.00 1399.25 0 0 0.000 0.000 | 10 24.00 564.60 510.00 32 0 0.000 0.000
972 2213eb38 | 10 69.00 -210.00 1399.25 0 0 0.000 0.000 | 10 30.00 564.60 510.00 32 0 0.000 0.000
978 2213eb38 | 10 75.00 -210.00 1399.25 0 0 0.000 0.000 | 10 36.00 564.60 510.00 32 0 0.000 0.000
984 2213eb38 | 10 1.00 -210.00 1399.25 0 0 0.000 0.000 | 10 42.00 564.60 510.00 32 0 0.000 0.000
990 2213eb38 | 10 7.00 -210.00 1399.25 0 0 0.000 0.000 | 10 48.00 564.60 510.00 32 0 0.000 0.000
996 2213eb38 | 10 13.00 -210.00 1399.25 0 0 0.000 0.000 | 10 4.00 564.60 510.00 32 0 0.000 0.000
1002 2213eb38 | 10 19.00 -210.00 1399.25 0 0 0.000 0.000 | 10 10.00 564.60 510.00 32 0 0.000 0.000
1008 2213eb38 | 10 25.00 -210.00 1399.25 0 0 0.000 0.000 | 10 16.00 564.60 510.00 32 0 0.000 0.000
1014 2213eb38 | 10 31.00 -210.00 1399.25 0 0 0.000 0.000 | 10 22.00 564.60 510.00 32 0 0.000 0.000
1020 2213eb38 | 10 37.00 -210.00 1399.25 0 0 0.000 0.000 | 10 28.00 564.60 510.00 32 0 0.000 0.000
1026 2213eb38 | 10 43.00 -210.00 1399.25 0 0 0.000 0.000 | 10 34.00 564.60 510.00 32 0 0.000 0.000
1032 2213eb38 | 10 49.00 -210.00 1399.25 0 0 0.000 0.000 | 10 40.00 564.60 510.00 32 0 0.000 0.000
1038 2213eb38 | 10 55.00 -210.00 1399.25 0 0 0.000 0.000 | 10 46.00 564.60 510.00 32 0 0.000 0.000
1044 2213eb38 | 10 61.00 -210.00 1399.25 0 0 0.000 0.000 | 10 2.00 564.60 510.00 32 0 0.000 0.000
1050 2213eb38 | 10 67.00 -210.00 1399.25 0 0 0.000 0.000 | 10 8.00 564.60 510.00 32 0 0.000 0.000
1056 2213eb38 | 10 73.00 -210.00 1399.25 0 0 0.000 0.000 | 10 14.00 564.60 510.00 32 0 0.000 0.000
1062 2213eb38 | 10 79.00 -210.00 1399.25 0 0 0.000 0.000 | 10 20.00 564.60 510.00 32 0 0.000 0.000
1068 2213eb38 | 10 5.00 -210.00 1399.25 0 0 0.000 0.000 | 10 26.00 564.60 510.00 32 0 0.000 0.000
1074 2213eb38 | 10 11.00 -210.00 1399.25 0 0 0.000 0.000 | 10 32.00 564.60 510.00 32 0 0.000 0.000
1080 2213eb38 | 10 17.00 -210.00 1399.25 0 0 0.000 0.000 | 10 38.00 564.60 510.00 32 0 0.000 0.000
1086 2213eb38 | 10 23.00 -210.00 1399.25 0 0 0.000 0.000 | 10 44.00 564.60 510.00 32 0 0.000 0.000
1092 2213eb38 | 10 29.00 -210.00 1399.25 0 0 0.000 0.000 | 10 0.00 564.60 510.00 32 0 0.000 0.000
1098 2213eb38 | 10 35.00 -210.00 1399.25 0 0 0.000 0.000 | 10 6.00 564.60 510.00 32 0 0.000 0.000
1100 2213eb38 | 10 37.00 -210.00 1399.25 0 0 0.000 0.000 | 10 8.00 564.60 510.00 32 0 0.000 0.000
";

#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn a_forced_hitmonlee_kicks_the_dummy_as_on_the_n64() {
    forced_ball(
        FORCED_HITMONLEE,
        10,
        "nITKindSawamura from a forced Poké Ball",
    );
}

/// `b36poke11` (`warp_link`, [`FORCED_BALL_ROUTE`],
/// `dITManagerForceMonsterKind` = 11 poked at frame 2): `nITKindDogas`.
/// Koffing spreads Smog from 426 (`nWPKindDogasSmog`). Rows 396-1100: every 6th frame, the seed's changes, and
/// every frame round a hit; the dummy is Training's CPU (its getup at
/// 630).
const FORCED_KOFFING: &str = "
210 69521988
211 a6e9c9a6
212 fe87df14
213 c40df552
327 491c86cd
332 4db793af
333 074d495a
396 074d495a | 10 53.00 -210.00 1399.25 0 0 0.000 0.000 | 57 5.00 2369.91 1575.09 20 0 0.000 0.000 | 19 385.58 1507.32 0.000 0.000
402 074d495a | 10 59.00 -210.00 1399.25 0 0 0.000 0.000 | 57 11.00 2369.91 1311.09 20 0 0.000 0.000 | 19 385.58 1507.32 0.000 0.000
408 074d495a | 10 65.00 -210.00 1399.25 0 0 0.000 0.000 | 57 17.00 2369.91 1047.09 20 0 0.000 0.000 | 42 385.58 1843.32 0.000 16.000
414 074d495a | 10 71.00 -210.00 1399.25 0 0 0.000 0.000 | 57 23.00 2369.91 783.09 20 0 0.000 0.000 | 42 385.58 1939.32 0.000 16.000
420 074d495a | 10 77.00 -210.00 1399.25 0 0 0.000 0.000 | 57 0.00 2369.91 519.09 20 0 0.000 0.000 | 42 385.58 2035.32 0.000 16.000
426 6443b2a4 | 10 3.00 -210.00 1399.25 0 0 0.000 0.000 | 57 6.00 2369.91 255.09 20 0 0.000 0.000 | 42 385.58 2099.32 0.000 0.000 | 29 289.11 1994.64 -18.000 -18.000
431 e6bb16d7
432 e6bb16d7 | 10 9.00 -210.00 1399.25 0 0 0.000 0.000 | 57 12.00 2369.91 -8.91 20 0 0.000 0.000 | 42 385.58 2099.32 0.000 0.000 | 29 181.11 1886.64 -18.000 -18.000
434 a6a6b409
438 a6a6b409 | 10 15.00 -210.00 1399.25 0 0 0.000 0.000 | 67 3.00 2359.10 -101.00 20 0 0.000 0.000 | 42 385.58 2099.32 0.000 0.000 | 29 73.11 1778.64 -18.000 -18.000 ; 29 199.55 2310.10 -18.000 18.000
442 dd038acb
444 dd038acb | 10 21.00 -210.00 1399.25 0 0 0.000 0.000 | 67 9.00 2337.48 -101.00 20 0 0.000 0.000 | 42 385.58 2099.32 0.000 0.000 | 29 -34.89 1670.64 -18.000 -18.000 ; 29 91.55 2418.10 -18.000 18.000 ; 29 306.77 2443.99 -18.000 18.000
450 f67de79d | 10 27.00 -210.00 1399.25 0 0 0.000 0.000 | 67 15.00 2315.86 -101.00 20 0 0.000 0.000 | 42 385.58 2099.32 0.000 0.000 | 29 -142.89 1562.64 -18.000 -18.000 ; 29 -16.45 2526.10 -18.000 18.000 ; 29 198.77 2551.99 -18.000 18.000 ; 29 594.75 2487.60 18.000 18.000
456 f67de79d | 10 33.00 -210.00 1399.25 0 0 0.000 0.000 | 67 21.00 2294.23 -101.00 20 0 0.000 0.000 | 42 385.58 2099.32 0.000 0.000 | 29 -124.45 2634.10 -18.000 18.000 ; 29 90.77 2659.99 -18.000 18.000 ; 29 702.75 2595.60 18.000 18.000
458 92387aff
462 92387aff | 10 39.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2272.61 -101.00 20 0 0.000 0.000 | 42 385.58 2099.32 0.000 0.000 | 29 -232.45 2742.10 -18.000 18.000 ; 29 -17.23 2767.99 -18.000 18.000 ; 29 810.75 2703.60 18.000 18.000 ; 29 203.44 2246.26 -18.000 18.000
466 9e1f7971
468 9e1f7971 | 10 45.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2250.99 -101.00 20 0 0.000 0.000 | 42 385.58 2099.32 0.000 0.000 | 29 -125.23 2875.99 -18.000 18.000 ; 29 918.75 2811.60 18.000 18.000 ; 29 95.44 2354.26 -18.000 18.000 ; 29 531.99 2247.45 18.000 18.000
474 9ad4bb73 | 10 51.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2229.37 -101.00 20 0 0.000 0.000 | 42 385.58 2099.32 0.000 0.000 | 29 1026.75 2919.60 18.000 18.000 ; 29 -12.56 2462.26 -18.000 18.000 ; 29 639.99 2355.45 18.000 18.000 ; 29 576.02 2201.16 18.000 18.000
480 9ad4bb73 | 10 57.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2207.75 -101.00 20 0 0.000 0.000 | 42 385.58 2099.32 0.000 0.000 | 29 -120.56 2570.26 -18.000 18.000 ; 29 747.99 2463.45 18.000 18.000 ; 29 684.02 2309.16 18.000 18.000
482 6bfcdd85
486 6bfcdd85 | 10 63.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2186.13 -101.00 20 0 0.000 0.000 | 42 385.58 2099.32 0.000 0.000 | 29 -228.56 2678.26 -18.000 18.000 ; 29 855.99 2571.45 18.000 18.000 ; 29 792.02 2417.16 18.000 18.000 ; 29 631.50 1946.78 18.000 -18.000
490 7ff56027
492 7ff56027 | 10 69.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2164.51 -101.00 20 0 0.000 0.000 | 42 385.58 2099.32 0.000 0.000 | 29 963.99 2679.45 18.000 18.000 ; 29 900.02 2525.16 18.000 18.000 ; 29 739.50 1838.78 18.000 -18.000 ; 29 330.93 2045.19 -18.000 -18.000
498 b23bc7d9 | 10 75.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2142.89 -101.00 20 0 0.000 0.000 | 42 385.58 2099.32 0.000 0.000 | 29 1008.02 2633.16 18.000 18.000 ; 29 847.50 1730.78 18.000 -18.000 ; 29 222.93 1937.19 -18.000 -18.000 ; 29 525.87 2274.29 18.000 18.000
504 b23bc7d9 | 10 1.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2121.27 -101.00 20 0 0.000 0.000 | 42 385.58 2099.32 0.000 0.000 | 29 955.50 1622.78 18.000 -18.000 ; 29 114.93 1829.19 -18.000 -18.000 ; 29 633.87 2382.29 18.000 18.000
506 a60dbd1b
510 a60dbd1b | 10 7.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2099.65 -101.00 20 0 0.000 0.000 | 42 385.58 2099.32 0.000 0.000 | 29 1063.50 1514.78 18.000 -18.000 ; 29 6.93 1721.19 -18.000 -18.000 ; 29 741.87 2490.29 18.000 18.000 ; 29 148.82 2308.23 -18.000 18.000
514 830a2c6d
516 830a2c6d | 10 13.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2078.02 -101.00 20 0 0.000 0.000 | 42 385.58 2099.32 0.000 0.000 | 29 -101.07 1613.19 -18.000 -18.000 ; 29 849.87 2598.29 18.000 18.000 ; 29 40.82 2416.23 -18.000 18.000 ; 29 607.60 2162.82 18.000 18.000
522 8bdb664f | 10 19.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2056.40 -101.00 20 0 0.000 0.000 | 42 385.58 2099.32 0.000 0.000 | 29 957.87 2706.29 18.000 18.000 ; 29 -67.18 2524.23 -18.000 18.000 ; 29 715.60 2270.82 18.000 18.000 ; 29 478.75 2154.37 18.000 18.000
528 8bdb664f | 10 25.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2034.78 -101.00 20 0 0.000 0.000 | 42 385.58 2099.32 0.000 0.000 | 29 -175.18 2632.23 -18.000 18.000 ; 29 823.60 2378.82 18.000 18.000 ; 29 586.75 2262.37 18.000 18.000
530 07323f41
534 07323f41 | 10 31.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2013.16 -101.00 20 0 0.000 0.000 | 42 385.58 2099.32 0.000 0.000 | 29 -283.18 2740.23 -18.000 18.000 ; 29 931.60 2486.82 18.000 18.000 ; 29 694.75 2370.37 18.000 18.000 ; 29 611.01 1631.81 18.000 -18.000
538 f59a2fc3
540 f59a2fc3 | 10 37.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1991.54 -101.00 20 0 0.000 0.000 | 42 385.58 2099.32 0.000 0.000 | 29 1039.60 2594.82 18.000 18.000 ; 29 802.75 2478.37 18.000 18.000 ; 29 719.01 1523.81 18.000 -18.000 ; 29 133.33 2520.83 -18.000 18.000
546 22ee7455 | 10 43.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1969.92 -101.00 20 0 0.000 0.000 | 42 385.58 2099.32 0.000 0.000 | 29 910.75 2586.37 18.000 18.000 ; 29 827.01 1415.81 18.000 -18.000 ; 29 25.33 2628.83 -18.000 18.000 ; 29 458.75 1790.48 18.000 -18.000
552 22ee7455 | 10 49.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1948.29 -101.00 20 0 0.000 0.000 | 42 385.58 2099.32 0.000 0.000 | 29 935.01 1307.81 18.000 -18.000 ; 29 -82.67 2736.83 -18.000 18.000 ; 29 566.75 1682.48 18.000 -18.000
554 d8782d77
558 d8782d77 | 10 55.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1926.67 -101.00 20 0 0.000 0.000 | 42 385.58 2099.32 0.000 0.000 | 29 1043.01 1199.81 18.000 -18.000 ; 29 -190.67 2844.83 -18.000 18.000 ; 29 674.75 1574.48 18.000 -18.000 ; 29 213.15 2465.79 -18.000 18.000
562 dcfd7fa9
564 dcfd7fa9 | 10 61.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1905.05 -101.00 20 0 0.000 0.000 | 42 385.58 2099.32 0.000 0.000 | 29 -298.67 2952.83 -18.000 18.000 ; 29 782.75 1466.48 18.000 -18.000 ; 29 105.15 2573.79 -18.000 18.000 ; 29 607.09 2443.91 18.000 18.000
570 1a49b36b | 10 67.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1883.43 -101.00 20 0 0.000 0.000 | 42 385.58 2099.32 0.000 0.000 | 29 890.75 1358.48 18.000 -18.000 ; 29 -2.85 2681.79 -18.000 18.000 ; 29 715.09 2551.91 18.000 18.000 ; 29 264.56 1763.47 -18.000 -18.000
576 1a49b36b | 10 73.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1861.81 -101.00 20 0 0.000 0.000 | 42 385.58 2099.32 0.000 0.000 | 29 -110.85 2789.79 -18.000 18.000 ; 29 823.09 2659.91 18.000 18.000 ; 29 156.56 1655.47 -18.000 -18.000
578 3bf6553d
582 3bf6553d | 10 79.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1840.19 -101.00 20 0 0.000 0.000 | 42 385.58 2099.32 0.000 0.000 | 29 -218.85 2897.79 -18.000 18.000 ; 29 931.09 2767.91 18.000 18.000 ; 29 48.56 1547.47 -18.000 -18.000 ; 29 244.32 1796.70 -18.000 -18.000
586 797d559f
588 797d559f | 10 5.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1818.57 -101.00 20 0 0.000 0.000 | 42 385.58 2099.32 0.000 0.000 | 29 1039.09 2875.91 18.000 18.000 ; 29 -59.44 1439.47 -18.000 -18.000 ; 29 136.32 1688.70 -18.000 -18.000 ; 29 531.17 2024.98 18.000 -18.000
594 8ddc2911 | 10 11.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1796.94 -101.00 20 0 0.000 0.000 | 42 385.58 2099.32 0.000 0.000 | 29 -167.44 1331.47 -18.000 -18.000 ; 29 28.32 1580.70 -18.000 -18.000 ; 29 639.17 1916.98 18.000 -18.000 ; 29 552.46 2160.64 18.000 18.000
600 8ddc2911 | 10 17.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1775.32 -101.00 20 0 0.000 0.000 | 42 385.58 2099.32 0.000 0.000 | 29 -79.68 1472.70 -18.000 -18.000 ; 29 747.17 1808.98 18.000 -18.000 ; 29 660.46 2268.64 18.000 18.000
602 314fe813
606 314fe813 | 10 23.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1753.70 -101.00 20 0 0.000 0.000 | 42 385.58 2099.32 0.000 0.000 | 29 -187.68 1364.70 -18.000 -18.000 ; 29 855.17 1700.98 18.000 -18.000 ; 29 768.46 2376.64 18.000 18.000 ; 29 215.38 1763.41 -18.000 -18.000
610 4df26f25
612 4df26f25 | 10 29.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1732.08 -101.00 20 0 0.000 0.000 | 42 385.58 2099.32 0.000 0.000 | 29 963.17 1592.98 18.000 -18.000 ; 29 876.46 2484.64 18.000 18.000 ; 29 107.38 1655.41 -18.000 -18.000 ; 29 278.57 1888.90 -18.000 -18.000
613 c9442554
618 1366fe6e | 10 35.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1710.46 -101.00 20 0 0.000 0.000 | 42 385.58 2099.32 0.000 0.000 | 29 984.46 2592.64 18.000 18.000 ; 29 -0.62 1547.41 -18.000 -18.000 ; 29 170.57 1780.90 -18.000 -18.000 ; 29 239.84 1741.94 -18.000 -18.000
624 1366fe6e | 10 41.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1688.84 -101.00 20 0 0.000 0.000 | 42 385.58 2099.32 0.000 0.000 | 29 -108.62 1439.41 -18.000 -18.000 ; 29 62.57 1672.90 -18.000 -18.000 ; 29 131.84 1633.94 -18.000 -18.000
626 b66b3058
630 b66b3058 | 10 47.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1667.21 -101.00 20 0 0.000 0.000 | 42 385.58 2099.32 0.000 0.000 | 29 -216.62 1331.41 -18.000 -18.000 ; 29 -45.43 1564.90 -18.000 -18.000 ; 29 23.84 1525.94 -18.000 -18.000 ; 29 140.60 2359.38 -18.000 18.000
634 c2300192
636 c2300192 | 10 53.00 -210.00 1399.25 0 0 0.000 0.000 | 79 3.00 1645.59 -101.00 20 0 0.000 0.000 | 42 385.58 2099.32 0.000 0.000 | 29 -153.43 1456.90 -18.000 -18.000 ; 29 -84.16 1417.94 -18.000 -18.000 ; 29 32.60 2467.38 -18.000 18.000 ; 29 600.15 2360.16 18.000 18.000
642 6032ec9c | 10 59.00 -210.00 1399.25 0 0 0.000 0.000 | 79 9.00 1623.97 -101.00 20 0 0.000 0.000 | 42 385.58 2099.32 0.000 0.000 | 29 -192.16 1309.94 -18.000 -18.000 ; 29 -75.40 2575.38 -18.000 18.000 ; 29 708.15 2468.16 18.000 18.000 ; 29 365.57 1981.93 -18.000 -18.000
648 6032ec9c | 10 65.00 -210.00 1399.25 0 0 0.000 0.000 | 79 15.00 1602.35 -101.00 20 0 0.000 0.000 | 42 385.58 2099.32 0.000 0.000 | 29 -183.40 2683.38 -18.000 18.000 ; 29 816.15 2576.16 18.000 18.000 ; 29 257.57 1873.93 -18.000 -18.000
650 15a13ff6
654 15a13ff6 | 10 71.00 -210.00 1399.25 0 0 0.000 0.000 | 79 21.00 1580.73 -101.00 20 0 0.000 0.000 | 42 385.58 2099.32 0.000 0.000 | 29 -291.40 2791.38 -18.000 18.000 ; 29 924.15 2684.16 18.000 18.000 ; 29 149.57 1765.93 -18.000 -18.000 ; 29 164.69 1676.91 -18.000 -18.000
658 bef6be20
660 bef6be20 | 10 77.00 -210.00 1399.25 0 0 0.000 0.000 | 79 27.00 1559.11 -101.00 20 0 0.000 0.000 | 42 385.58 2099.32 0.000 0.000 | 29 1032.15 2792.16 18.000 18.000 ; 29 41.57 1657.93 -18.000 -18.000 ; 29 56.69 1568.91 -18.000 -18.000 ; 29 263.03 2350.08 -18.000 18.000
666 59913d9a | 10 3.00 -210.00 1399.25 0 0 0.000 0.000 | 79 33.00 1537.48 -101.00 20 0 0.000 0.000 | 42 385.58 2099.32 0.000 0.000 | 29 -66.43 1549.93 -18.000 -18.000 ; 29 -51.31 1460.91 -18.000 -18.000 ; 29 155.03 2458.08 -18.000 18.000 ; 29 416.28 1961.22 18.000 -18.000
672 59913d9a | 10 9.00 -210.00 1399.25 0 0 0.000 0.000 | 79 39.00 1515.86 -101.00 20 0 0.000 0.000 | 42 385.58 2099.32 0.000 0.000 | 29 -159.31 1352.91 -18.000 -18.000 ; 29 47.03 2566.08 -18.000 18.000 ; 29 524.28 1853.22 18.000 -18.000
678 59913d9a | 10 15.00 -210.00 1399.25 0 0 0.000 0.000 | 79 45.00 1494.24 -101.00 20 0 0.000 0.000 | 42 385.58 2099.32 0.000 0.000 | 29 -267.31 1244.91 -18.000 -18.000 ; 29 -60.97 2674.08 -18.000 18.000 ; 29 632.28 1745.22 18.000 -18.000
684 59913d9a | 10 21.00 -210.00 1399.25 0 0 0.000 0.000 | 10 1.00 1472.62 -101.00 20 0 0.000 0.000 | 42 385.58 2099.32 0.000 0.000 | 29 -168.97 2782.08 -18.000 18.000 ; 29 740.28 1637.22 18.000 -18.000
690 59913d9a | 10 27.00 -210.00 1399.25 0 0 0.000 0.000 | 10 7.00 1451.00 -101.00 20 0 0.000 0.000 | 42 385.58 2099.32 0.000 0.000 | 29 848.28 1529.22 18.000 -18.000
696 59913d9a | 10 33.00 -210.00 1399.25 0 0 0.000 0.000 | 10 13.00 1429.38 -101.00 20 0 0.000 0.000 | 42 385.58 2099.32 0.000 0.000
702 59913d9a | 10 39.00 -210.00 1399.25 0 0 0.000 0.000 | 10 19.00 1407.75 -101.00 20 0 0.000 0.000 | 42 385.58 2099.32 0.000 0.000
708 59913d9a | 10 45.00 -210.00 1399.25 0 0 0.000 0.000 | 10 25.00 1386.13 -101.00 20 0 0.000 0.000 | 42 385.58 2099.32 0.000 0.000
714 59913d9a | 10 51.00 -210.00 1399.25 0 0 0.000 0.000 | 10 31.00 1364.51 -101.00 20 0 0.000 0.000 | 42 385.58 2099.32 0.000 0.000
720 59913d9a | 10 57.00 -210.00 1399.25 0 0 0.000 0.000 | 10 37.00 1342.89 -101.00 20 0 0.000 0.000 | 42 385.58 2099.32 0.000 0.000
726 59913d9a | 10 63.00 -210.00 1399.25 0 0 0.000 0.000 | 10 43.00 1321.27 -101.00 20 0 0.000 0.000 | 42 385.58 2099.32 0.000 0.000
732 59913d9a | 10 69.00 -210.00 1399.25 0 0 0.000 0.000 | 10 49.00 1320.00 -101.00 20 0 0.000 0.000 | 42 385.58 2099.32 0.000 0.000
738 59913d9a | 10 75.00 -210.00 1399.25 0 0 0.000 0.000 | 10 5.00 1320.00 -101.00 20 0 0.000 0.000 | 42 385.58 2099.32 0.000 0.000
744 59913d9a | 10 1.00 -210.00 1399.25 0 0 0.000 0.000 | 10 11.00 1320.00 -101.00 20 0 0.000 0.000 | 42 385.58 2099.32 0.000 0.000
750 59913d9a | 10 7.00 -210.00 1399.25 0 0 0.000 0.000 | 10 17.00 1320.00 -101.00 20 0 0.000 0.000 | 42 385.58 2099.32 0.000 0.000
756 59913d9a | 10 13.00 -210.00 1399.25 0 0 0.000 0.000 | 10 23.00 1320.00 -101.00 20 0 0.000 0.000 | 42 385.58 2099.32 0.000 0.000
762 59913d9a | 10 19.00 -210.00 1399.25 0 0 0.000 0.000 | 10 29.00 1320.00 -101.00 20 0 0.000 0.000
768 59913d9a | 10 25.00 -210.00 1399.25 0 0 0.000 0.000 | 10 35.00 1320.00 -101.00 20 0 0.000 0.000
774 59913d9a | 10 31.00 -210.00 1399.25 0 0 0.000 0.000 | 10 41.00 1320.00 -101.00 20 0 0.000 0.000
780 59913d9a | 10 37.00 -210.00 1399.25 0 0 0.000 0.000 | 10 47.00 1320.00 -101.00 20 0 0.000 0.000
785 73dadb49
786 73dadb49 | 10 43.00 -210.00 1399.25 0 0 0.000 0.000 | 25 1.00 1320.00 32.20 20 0 0.000 0.000
791 fbb95a2c
792 fbb95a2c | 10 49.00 -210.00 1399.25 0 0 0.000 0.000 | 25 7.00 1240.80 395.80 20 0 0.000 0.000
798 fbb95a2c | 10 55.00 -210.00 1399.25 0 0 0.000 0.000 | 25 13.00 1125.00 673.00 20 0 0.000 0.000
804 fbb95a2c | 10 61.00 -210.00 1399.25 0 0 0.000 0.000 | 25 19.00 1016.40 863.80 20 0 0.000 0.000
810 fbb95a2c | 10 67.00 -210.00 1399.25 0 0 0.000 0.000 | 25 25.00 915.00 968.20 20 0 0.000 0.000
816 fbb95a2c | 10 73.00 -210.00 1399.25 0 0 0.000 0.000 | 25 31.00 820.80 986.20 20 0 0.000 0.000
822 fbb95a2c | 10 79.00 -210.00 1399.25 0 0 0.000 0.000 | 25 37.00 733.80 917.80 20 0 0.000 0.000
828 fbb95a2c | 10 5.00 -210.00 1399.25 0 0 0.000 0.000 | 25 43.00 654.00 763.00 20 0 0.000 0.000
834 fbb95a2c | 10 11.00 -210.00 1399.25 0 0 0.000 0.000 | 25 49.00 581.40 524.00 20 0 0.000 0.000
835 139d9a06
840 139d9a06 | 10 17.00 -210.00 1399.25 0 0 0.000 0.000 | 31 5.00 564.60 510.00 20 0 0.000 0.000
846 139d9a06 | 10 23.00 -210.00 1399.25 0 0 0.000 0.000 | 10 4.00 564.60 510.00 20 0 0.000 0.000
852 139d9a06 | 10 29.00 -210.00 1399.25 0 0 0.000 0.000 | 10 10.00 564.60 510.00 20 0 0.000 0.000
858 139d9a06 | 10 35.00 -210.00 1399.25 0 0 0.000 0.000 | 10 16.00 564.60 510.00 20 0 0.000 0.000
864 139d9a06 | 10 41.00 -210.00 1399.25 0 0 0.000 0.000 | 10 22.00 564.60 510.00 20 0 0.000 0.000
870 139d9a06 | 10 47.00 -210.00 1399.25 0 0 0.000 0.000 | 10 28.00 564.60 510.00 20 0 0.000 0.000
876 139d9a06 | 10 53.00 -210.00 1399.25 0 0 0.000 0.000 | 10 34.00 564.60 510.00 20 0 0.000 0.000
882 139d9a06 | 10 59.00 -210.00 1399.25 0 0 0.000 0.000 | 10 40.00 564.60 510.00 20 0 0.000 0.000
888 139d9a06 | 10 65.00 -210.00 1399.25 0 0 0.000 0.000 | 10 46.00 564.60 510.00 20 0 0.000 0.000
894 139d9a06 | 10 71.00 -210.00 1399.25 0 0 0.000 0.000 | 10 2.00 564.60 510.00 20 0 0.000 0.000
900 139d9a06 | 10 77.00 -210.00 1399.25 0 0 0.000 0.000 | 10 8.00 564.60 510.00 20 0 0.000 0.000
906 139d9a06 | 10 3.00 -210.00 1399.25 0 0 0.000 0.000 | 10 14.00 564.60 510.00 20 0 0.000 0.000
912 139d9a06 | 10 9.00 -210.00 1399.25 0 0 0.000 0.000 | 10 20.00 564.60 510.00 20 0 0.000 0.000
918 139d9a06 | 10 15.00 -210.00 1399.25 0 0 0.000 0.000 | 10 26.00 564.60 510.00 20 0 0.000 0.000
924 139d9a06 | 10 21.00 -210.00 1399.25 0 0 0.000 0.000 | 10 32.00 564.60 510.00 20 0 0.000 0.000
930 139d9a06 | 10 27.00 -210.00 1399.25 0 0 0.000 0.000 | 10 38.00 564.60 510.00 20 0 0.000 0.000
936 139d9a06 | 10 33.00 -210.00 1399.25 0 0 0.000 0.000 | 10 44.00 564.60 510.00 20 0 0.000 0.000
942 139d9a06 | 10 39.00 -210.00 1399.25 0 0 0.000 0.000 | 10 0.00 564.60 510.00 20 0 0.000 0.000
948 139d9a06 | 10 45.00 -210.00 1399.25 0 0 0.000 0.000 | 10 6.00 564.60 510.00 20 0 0.000 0.000
954 139d9a06 | 10 51.00 -210.00 1399.25 0 0 0.000 0.000 | 10 12.00 564.60 510.00 20 0 0.000 0.000
960 139d9a06 | 10 57.00 -210.00 1399.25 0 0 0.000 0.000 | 10 18.00 564.60 510.00 20 0 0.000 0.000
966 139d9a06 | 10 63.00 -210.00 1399.25 0 0 0.000 0.000 | 10 24.00 564.60 510.00 20 0 0.000 0.000
972 139d9a06 | 10 69.00 -210.00 1399.25 0 0 0.000 0.000 | 10 30.00 564.60 510.00 20 0 0.000 0.000
978 139d9a06 | 10 75.00 -210.00 1399.25 0 0 0.000 0.000 | 10 36.00 564.60 510.00 20 0 0.000 0.000
984 139d9a06 | 10 1.00 -210.00 1399.25 0 0 0.000 0.000 | 10 42.00 564.60 510.00 20 0 0.000 0.000
990 139d9a06 | 10 7.00 -210.00 1399.25 0 0 0.000 0.000 | 10 48.00 564.60 510.00 20 0 0.000 0.000
996 139d9a06 | 10 13.00 -210.00 1399.25 0 0 0.000 0.000 | 10 4.00 564.60 510.00 20 0 0.000 0.000
1002 139d9a06 | 10 19.00 -210.00 1399.25 0 0 0.000 0.000 | 10 10.00 564.60 510.00 20 0 0.000 0.000
1008 139d9a06 | 10 25.00 -210.00 1399.25 0 0 0.000 0.000 | 10 16.00 564.60 510.00 20 0 0.000 0.000
1014 139d9a06 | 10 31.00 -210.00 1399.25 0 0 0.000 0.000 | 10 22.00 564.60 510.00 20 0 0.000 0.000
1020 139d9a06 | 10 37.00 -210.00 1399.25 0 0 0.000 0.000 | 10 28.00 564.60 510.00 20 0 0.000 0.000
1026 139d9a06 | 10 43.00 -210.00 1399.25 0 0 0.000 0.000 | 10 34.00 564.60 510.00 20 0 0.000 0.000
1032 139d9a06 | 10 49.00 -210.00 1399.25 0 0 0.000 0.000 | 10 40.00 564.60 510.00 20 0 0.000 0.000
1038 139d9a06 | 10 55.00 -210.00 1399.25 0 0 0.000 0.000 | 10 46.00 564.60 510.00 20 0 0.000 0.000
1044 139d9a06 | 10 61.00 -210.00 1399.25 0 0 0.000 0.000 | 10 2.00 564.60 510.00 20 0 0.000 0.000
1050 139d9a06 | 10 67.00 -210.00 1399.25 0 0 0.000 0.000 | 10 8.00 564.60 510.00 20 0 0.000 0.000
1056 139d9a06 | 10 73.00 -210.00 1399.25 0 0 0.000 0.000 | 10 14.00 564.60 510.00 20 0 0.000 0.000
1062 139d9a06 | 10 79.00 -210.00 1399.25 0 0 0.000 0.000 | 10 20.00 564.60 510.00 20 0 0.000 0.000
1068 139d9a06 | 10 5.00 -210.00 1399.25 0 0 0.000 0.000 | 10 26.00 564.60 510.00 20 0 0.000 0.000
1074 139d9a06 | 10 11.00 -210.00 1399.25 0 0 0.000 0.000 | 10 32.00 564.60 510.00 20 0 0.000 0.000
1080 139d9a06 | 10 17.00 -210.00 1399.25 0 0 0.000 0.000 | 10 38.00 564.60 510.00 20 0 0.000 0.000
1086 139d9a06 | 10 23.00 -210.00 1399.25 0 0 0.000 0.000 | 10 44.00 564.60 510.00 20 0 0.000 0.000
1092 139d9a06 | 10 29.00 -210.00 1399.25 0 0 0.000 0.000 | 10 0.00 564.60 510.00 20 0 0.000 0.000
1098 139d9a06 | 10 35.00 -210.00 1399.25 0 0 0.000 0.000 | 10 6.00 564.60 510.00 20 0 0.000 0.000
1100 139d9a06 | 10 37.00 -210.00 1399.25 0 0 0.000 0.000 | 10 8.00 564.60 510.00 20 0 0.000 0.000
";

#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn a_forced_koffing_spreads_smog_as_on_the_n64() {
    forced_ball(FORCED_KOFFING, 11, "nITKindDogas from a forced Poké Ball");
}

/// `b36poke12` (`warp_link`, [`FORCED_BALL_ROUTE`],
/// `dITManagerForceMonsterKind` = 12 poked at frame 2): `nITKindPippi`.
/// Clefairy's Metronome picks Meowth's coins from 429 (`nWPKindNyarsCoin`). Rows 396-1100: every 6th frame, the seed's changes, and
/// every frame round a hit; the dummy is Training's CPU (its getup at
/// 630).
const FORCED_CLEFAIRY: &str = "
210 69521988
211 a6e9c9a6
212 fe87df14
213 c40df552
327 491c86cd
332 4db793af
333 074d495a
396 074d495a | 10 53.00 -210.00 1399.25 0 0 0.000 0.000 | 57 5.00 2369.91 1575.09 20 0 0.000 0.000 | 19 385.58 1507.32 0.000 0.000
402 074d495a | 10 59.00 -210.00 1399.25 0 0 0.000 0.000 | 57 11.00 2369.91 1311.09 20 0 0.000 0.000 | 19 385.58 1507.32 0.000 0.000
408 074d495a | 10 65.00 -210.00 1399.25 0 0 0.000 0.000 | 57 17.00 2369.91 1047.09 20 0 0.000 0.000 | 43 385.58 1795.32 0.000 16.000
414 074d495a | 10 71.00 -210.00 1399.25 0 0 0.000 0.000 | 57 23.00 2369.91 783.09 20 0 0.000 0.000 | 43 385.58 1891.32 0.000 16.000
420 074d495a | 10 77.00 -210.00 1399.25 0 0 0.000 0.000 | 57 0.00 2369.91 519.09 20 0 0.000 0.000 | 43 385.58 1987.32 0.000 16.000
425 4dc8aab5
426 4dc8aab5 | 10 3.00 -210.00 1399.25 0 0 0.000 0.000 | 57 6.00 2369.91 255.09 20 0 0.000 0.000 | 43 385.58 2051.32 0.000 0.000
431 6443b2a4
432 6443b2a4 | 10 9.00 -210.00 1399.25 0 0 0.000 0.000 | 57 12.00 2369.91 -8.91 20 0 0.000 0.000 | 43 385.58 2051.32 0.000 0.000 | 24 905.58 2051.32 130.000 0.000 ; 24 385.58 2571.32 0.000 130.000 ; 24 -134.42 2051.32 -130.000 0.000 ; 24 385.58 1531.32 0.000 -130.000
438 6443b2a4 | 10 15.00 -210.00 1399.25 0 0 0.000 0.000 | 67 3.00 2359.10 -101.00 20 0 0.000 0.000 | 43 385.58 2051.32 0.000 0.000 | 24 1685.58 2051.32 130.000 0.000 ; 24 385.58 3351.32 0.000 130.000 ; 24 -914.42 2051.32 -130.000 0.000 ; 24 385.58 751.32 0.000 -130.000 ; 24 638.91 2109.81 126.668 29.244 ; 24 327.09 2304.66 -29.244 126.668 ; 24 132.24 1992.84 -126.668 -29.244 ; 24 444.06 1797.99 29.244 -126.668
444 6443b2a4 | 10 21.00 -210.00 1399.25 0 0 0.000 0.000 | 67 9.00 2337.48 -101.00 20 0 0.000 0.000 | 43 385.58 2051.32 0.000 0.000 | 24 1398.92 2285.27 126.668 29.244 ; 24 151.63 3064.67 -29.244 126.668 ; 24 -627.77 1817.38 -126.668 -29.244 ; 24 619.53 1037.98 29.244 -126.668
450 6443b2a4 | 10 27.00 -210.00 1399.25 0 0 0.000 0.000 | 67 15.00 2315.86 -101.00 20 0 0.000 0.000 | 43 385.58 2051.32 0.000 0.000 | 24 1086.64 2393.25 116.843 56.988 ; 24 43.65 2752.38 -56.988 116.843 ; 24 -315.48 1709.39 -116.843 -56.988 ; 24 727.51 1350.26 56.988 -116.843
456 6443b2a4 | 10 33.00 -210.00 1399.25 0 0 0.000 0.000 | 67 21.00 2294.23 -101.00 20 0 0.000 0.000 | 43 385.58 2051.32 0.000 0.000 | 24 789.69 2378.57 101.029 81.812 ; 24 58.33 2455.44 -81.812 101.029 ; 24 -18.54 1724.08 -101.029 -81.812 ; 24 712.82 1647.21 81.812 -101.029
462 6443b2a4 | 10 39.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2272.61 -101.00 20 0 0.000 0.000 | 43 385.58 2051.32 0.000 0.000 | 24 1395.87 2869.44 101.029 81.812 ; 24 -432.54 3061.61 -81.812 101.029 ; 24 -624.71 1233.21 -101.029 -81.812 ; 24 1203.69 1041.04 81.812 -101.029 ; 24 545.65 2256.21 80.036 102.441 ; 24 180.69 2211.40 -102.441 80.036 ; 24 225.50 1846.44 -80.036 -102.441 ; 24 590.46 1891.25 102.441 -80.036
468 6443b2a4 | 10 45.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2250.99 -101.00 20 0 0.000 0.000 | 43 385.58 2051.32 0.000 0.000 | 24 1025.86 2870.86 80.036 102.441 ; 24 -433.96 2691.61 -102.441 80.036 ; 24 -254.71 1231.79 -80.036 -102.441 ; 24 1205.11 1411.04 102.441 -80.036
474 6443b2a4 | 10 51.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2229.37 -101.00 20 0 0.000 0.000 | 43 385.58 2051.32 0.000 0.000 | 24 715.22 2758.24 54.940 117.820 ; 24 -321.34 2380.97 -117.820 54.940 ; 24 55.93 1344.40 -54.940 -117.820 ; 24 1092.50 1721.68 117.820 -54.940
480 6443b2a4 | 10 57.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2207.75 -101.00 20 0 0.000 0.000 | 43 385.58 2051.32 0.000 0.000 | 24 493.69 2559.96 27.029 127.159 ; 24 -123.06 2159.44 -127.159 27.029 ; 24 277.46 1542.69 -27.028 -127.159 ; 24 894.21 1943.21 127.159 -27.028
486 6443b2a4 | 10 63.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2186.13 -101.00 20 0 0.000 0.000 | 43 385.58 2051.32 0.000 0.000 | 24 655.86 3322.92 27.029 127.159 ; 24 -886.02 2321.61 -127.159 27.029 ; 24 115.29 779.73 -27.028 -127.159 ; 24 1657.17 1781.04 127.159 -27.028 ; 24 381.04 2311.28 -2.269 129.980 ; 24 125.62 2046.79 -129.980 -2.269 ; 24 390.11 1791.36 2.269 -129.980 ; 24 645.54 2055.86 129.980 2.269
492 6443b2a4 | 10 69.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2164.51 -101.00 20 0 0.000 0.000 | 43 385.58 2051.32 0.000 0.000 | 24 367.43 3091.17 -2.269 129.980 ; 24 -654.27 2033.17 -129.980 -2.269 ; 24 403.73 1011.48 2.269 -129.980 ; 24 1425.42 2069.47 129.980 2.269
498 6443b2a4 | 10 75.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2142.89 -101.00 20 0 0.000 0.000 | 43 385.58 2051.32 0.000 0.000 | 24 196.88 2808.16 -31.450 126.138 ; 24 -371.25 1862.63 -126.138 -31.450 ; 24 574.27 1294.49 31.450 -126.138 ; 24 1142.41 2240.02 126.138 31.450
504 6443b2a4 | 10 1.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2121.27 -101.00 20 0 0.000 0.000 | 43 385.58 2051.32 0.000 0.000 | 24 149.50 2514.65 -59.019 115.831 ; 24 -77.75 1815.25 -115.831 -59.019 ; 24 621.65 1588.00 59.019 -115.831 ; 24 848.90 2287.40 115.831 59.019
510 6443b2a4 | 10 7.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2099.65 -101.00 20 0 0.000 0.000 | 43 385.58 2051.32 0.000 0.000 | 24 -204.61 3209.63 -59.019 115.831 ; 24 -772.73 1461.14 -115.831 -59.019 ; 24 975.76 893.02 59.019 -115.831 ; 24 1543.88 2641.51 115.831 59.019 ; 24 218.45 2250.50 -83.562 99.586 ; 24 186.40 1884.20 -99.586 -83.562 ; 24 552.70 1852.15 83.562 -99.586 ; 24 584.75 2218.45 99.586 83.562
516 6443b2a4 | 10 13.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2078.02 -101.00 20 0 0.000 0.000 | 43 385.58 2051.32 0.000 0.000 | 24 -282.92 2848.01 -83.562 99.586 ; 24 -411.11 1382.83 -99.586 -83.562 ; 24 1054.07 1254.64 83.562 -99.586 ; 24 1182.26 2719.82 99.586 83.562
522 6443b2a4 | 10 19.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2056.40 -101.00 20 0 0.000 0.000 | 43 385.58 2051.32 0.000 0.000 | 24 -237.36 2520.74 -103.823 78.236 ; 24 -83.84 1428.39 -78.236 -103.823 ; 24 1008.51 1581.91 103.823 -78.236 ; 24 854.99 2674.26 78.236 103.823
528 6443b2a4 | 10 25.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2034.78 -101.00 20 0 0.000 0.000 | 43 385.58 2051.32 0.000 0.000 | 24 -89.47 2262.83 -118.761 52.876 ; 24 174.07 1576.28 -52.876 -118.761 ; 24 860.62 1839.82 118.761 -52.876 ; 24 597.08 2526.37 52.876 118.761
534 6443b2a4 | 10 31.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2013.16 -101.00 20 0 0.000 0.000 | 43 385.58 2051.32 0.000 0.000 | 24 -802.03 2580.08 -118.761 52.876 ; 24 -143.18 863.72 -52.876 -118.761 ; 24 1573.18 1522.57 118.761 -52.876 ; 24 914.33 3238.93 52.876 118.761 ; 24 130.35 2100.93 -127.612 24.805 ; 24 335.97 1796.10 -24.805 -127.612 ; 24 640.80 2001.71 127.612 -24.805 ; 24 435.19 2306.55 24.805 127.612
540 6443b2a4 | 10 37.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1991.54 -101.00 20 0 0.000 0.000 | 43 385.58 2051.32 0.000 0.000 | 24 -635.32 2249.77 -127.612 24.805 ; 24 187.13 1030.43 -24.805 -127.612 ; 24 1406.47 1852.88 127.612 -24.805 ; 24 584.02 3072.22 24.805 127.612
546 6443b2a4 | 10 43.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1969.92 -101.00 20 0 0.000 0.000 | 43 385.58 2051.32 0.000 0.000 | 24 -393.95 2024.10 -129.921 -4.537 ; 24 412.80 1271.80 4.537 -129.921 ; 24 1165.10 2078.55 129.921 4.537 ; 24 358.35 2830.85 -4.537 129.921
552 6443b2a4 | 10 49.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1948.29 -101.00 20 0 0.000 0.000 | 43 385.58 2051.32 0.000 0.000 | 24 -116.71 1916.74 -125.570 -33.647 ; 24 520.16 1549.04 33.647 -125.570 ; 24 887.86 2185.91 125.570 33.646 ; 24 250.99 2553.61 -33.646 125.570
558 6443b2a4 | 10 55.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1926.67 -101.00 20 0 0.000 0.000 | 43 385.58 2051.32 0.000 0.000 | 24 -870.13 1714.86 -125.570 -33.647 ; 24 722.04 795.62 33.647 -125.570 ; 24 1641.28 2387.79 125.570 33.646 ; 24 49.11 3307.03 -33.646 125.570 ; 24 156.01 1929.26 -114.783 -61.031 ; 24 507.64 1821.76 61.031 -114.783 ; 24 615.14 2173.39 114.783 61.031 ; 24 263.51 2280.89 -61.031 114.783
564 6443b2a4 | 10 61.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1905.05 -101.00 20 0 0.000 0.000 | 43 385.58 2051.32 0.000 0.000 | 24 -532.69 1563.07 -114.783 -61.031 ; 24 873.83 1133.06 61.031 -114.783 ; 24 1303.84 2539.57 114.783 61.031 ; 24 -102.67 2969.59 -61.031 114.783
570 6443b2a4 | 10 67.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1883.43 -101.00 20 0 0.000 0.000 | 43 385.58 2051.32 0.000 0.000 | 24 -203.10 1539.60 -98.112 -85.288 ; 24 897.30 1462.65 85.288 -98.112 ; 24 974.25 2563.05 98.112 85.288 ; 24 -126.15 2640.00 -85.288 98.112
576 6443b2a4 | 10 73.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1861.81 -101.00 20 0 0.000 0.000 | 43 385.58 2051.32 0.000 0.000 | 24 79.93 1630.64 -76.412 -105.172 ; 24 806.26 1745.68 105.172 -76.412 ; 24 691.22 2472.01 76.412 105.172 ; 24 -35.11 2356.97 -105.172 76.412
582 6443b2a4 | 10 79.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1840.19 -101.00 20 0 0.000 0.000 | 43 385.58 2051.32 0.000 0.000 | 24 -378.54 999.60 -76.412 -105.172 ; 24 1437.30 1287.20 105.172 -76.412 ; 24 1149.70 3103.05 76.412 105.172 ; 24 -666.15 2815.45 -105.172 76.412 ; 24 283.99 1811.99 -50.795 -119.666 ; 24 624.91 1949.73 119.666 -50.795 ; 24 487.17 2290.66 50.795 119.666 ; 24 146.24 2152.91 -119.666 50.795
588 6443b2a4 | 10 5.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1818.57 -101.00 20 0 0.000 0.000 | 43 385.58 2051.32 0.000 0.000 | 24 -20.78 1094.00 -50.795 -119.666 ; 24 1342.90 1644.97 119.666 -50.795 ; 24 791.94 3008.65 50.795 119.666 ; 24 -571.75 2457.68 -119.666 50.795
594 6443b2a4 | 10 11.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1796.94 -101.00 20 0 0.000 0.000 | 43 385.58 2051.32 0.000 0.000 | 24 250.13 1283.17 -22.574 -128.025 ; 24 1153.73 1915.88 128.025 -22.574 ; 24 521.02 2819.47 22.574 128.025 ; 24 -382.57 2186.77 -128.025 22.574
600 6443b2a4 | 10 17.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1775.32 -101.00 20 0 0.000 0.000 | 43 385.58 2051.32 0.000 0.000 | 24 412.79 1532.04 6.804 -129.822 ; 24 904.86 2078.54 129.822 6.804 ; 24 358.36 2570.61 -6.804 129.822 ; 24 -133.71 2024.11 -129.822 -6.804
606 6443b2a4 | 10 23.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1753.70 -101.00 20 0 0.000 0.000 | 43 385.58 2051.32 0.000 0.000 | 24 453.61 753.11 6.804 -129.822 ; 24 1683.79 2119.36 129.822 6.804 ; 24 317.54 3349.54 -6.804 129.822 ; 24 -912.64 1983.29 -129.822 -6.804 ; 24 457.24 1801.40 35.833 -124.964 ; 24 635.50 2122.99 124.964 35.833 ; 24 313.91 2301.25 -35.833 124.964 ; 24 135.65 1979.66 -124.964 -35.833
612 6443b2a4 | 10 29.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1732.08 -101.00 20 0 0.000 0.000 | 43 385.58 2051.32 0.000 0.000 | 24 672.24 1051.61 35.833 -124.964 ; 24 1385.29 2337.99 124.964 35.833 ; 24 98.91 3051.04 -35.833 124.964 ; 24 -614.14 1764.66 -124.964 -35.833
613 e6bb16d7
618 e6bb16d7 | 10 35.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1710.46 -101.00 20 0 0.000 0.000 | 43 385.58 2051.32 0.000 0.000 | 24 763.73 1369.12 63.025 -113.701 ; 24 1067.78 2429.48 113.701 63.025 ; 24 7.42 2733.53 -63.025 113.701 ; 24 -296.63 1673.17 -113.701 -63.025
624 e6bb16d7 | 10 41.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1688.84 -101.00 20 0 0.000 0.000 | 43 385.58 2051.32 0.000 0.000 | 24 733.52 1664.89 86.987 -96.609 ; 24 772.01 2399.27 96.609 86.987 ; 24 37.63 2437.76 -86.987 96.609 ; 24 -0.86 1703.38 -96.609 -86.987
630 e6bb16d7 | 10 47.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1667.21 -101.00 20 0 0.000 0.000 | 43 385.58 2051.32 0.000 0.000 | 24 1255.45 1085.24 86.987 -96.609 ; 24 1351.66 2921.20 96.609 86.987 ; 24 -484.29 3017.41 -86.987 96.609 ; 24 -580.51 1181.45 -96.609 -86.987 ; 24 598.56 1902.19 106.490 -74.565 ; 24 534.71 2264.30 74.565 106.490 ; 24 172.60 2200.45 -106.490 74.565 ; 24 236.45 1838.34 -74.565 -106.490
636 e6bb16d7 | 10 53.00 -210.00 1399.25 0 0 0.000 0.000 | 79 2.00 1645.59 -101.00 20 0 0.000 0.000 | 43 385.58 2051.32 0.000 0.000 | 24 1237.49 1454.80 106.490 -74.565 ; 24 982.10 2903.24 74.565 106.490 ; 24 -466.34 2647.84 -106.490 74.565 ; 24 -210.94 1199.41 -74.565 -106.490
642 e6bb16d7 | 10 59.00 -210.00 1399.25 0 0 0.000 0.000 | 79 8.00 1623.97 -101.00 20 0 0.000 0.000 | 43 385.58 2051.32 0.000 0.000 | 24 1108.78 1759.13 120.534 -48.699 ; 24 677.77 2774.53 48.699 120.534 ; 24 -337.63 2343.52 -120.534 48.699 ; 24 93.38 1328.12 -48.699 -120.534
648 e6bb16d7 | 10 65.00 -210.00 1399.25 0 0 0.000 0.000 | 79 14.00 1602.35 -101.00 20 0 0.000 0.000 | 43 385.58 2051.32 0.000 0.000 | 24 899.17 1969.98 128.399 -20.336 ; 24 466.92 2564.92 20.336 128.399 ; 24 -128.02 2132.67 -128.400 20.336 ; 24 304.23 1537.73 -20.336 -128.399
654 e6bb16d7 | 10 71.00 -210.00 1399.25 0 0 0.000 0.000 | 79 20.00 1580.73 -101.00 20 0 0.000 0.000 | 43 385.58 2051.32 0.000 0.000 | 24 1669.57 1847.96 128.399 -20.336 ; 24 588.94 3335.32 20.336 128.399 ; 24 -898.42 2254.69 -128.400 20.336 ; 24 182.21 767.33 -20.336 -128.399 ; 24 644.94 2069.46 129.683 9.068 ; 24 367.44 2310.69 -9.068 129.683 ; 24 126.21 2033.19 -129.683 -9.068 ; 24 403.71 1791.96 9.068 -129.683
660 e6bb16d7 | 10 77.00 -210.00 1399.25 0 0 0.000 0.000 | 79 26.00 1559.11 -101.00 20 0 0.000 0.000 | 43 385.58 2051.32 0.000 0.000 | 24 1423.04 2123.87 129.683 9.068 ; 24 313.03 3088.79 -9.068 129.683 ; 24 -651.89 1978.78 -129.683 -9.068 ; 24 458.12 1013.86 9.068 -129.683
666 e6bb16d7 | 10 3.00 -210.00 1399.25 0 0 0.000 0.000 | 79 32.00 1537.48 -101.00 20 0 0.000 0.000 |  | 24 1131.49 2279.37 124.320 38.008 ; 24 157.53 2797.24 -38.008 124.320 ; 24 -360.34 1823.27 -124.320 -38.008 ; 24 613.63 1305.41 38.008 -124.320
672 e6bb16d7 | 10 9.00 -210.00 1399.25 0 0 0.000 0.000 | 79 38.00 1515.86 -101.00 20 0 0.000 0.000
678 e6bb16d7 | 10 15.00 -210.00 1399.25 0 0 0.000 0.000 | 79 44.00 1494.24 -101.00 20 0 0.000 0.000
684 e6bb16d7 | 10 21.00 -210.00 1399.25 0 0 0.000 0.000 | 10 0.00 1472.62 -101.00 20 0 0.000 0.000
690 e6bb16d7 | 10 27.00 -210.00 1399.25 0 0 0.000 0.000 | 10 6.00 1451.00 -101.00 20 0 0.000 0.000
696 e6bb16d7 | 10 33.00 -210.00 1399.25 0 0 0.000 0.000 | 10 12.00 1429.38 -101.00 20 0 0.000 0.000
702 e6bb16d7 | 10 39.00 -210.00 1399.25 0 0 0.000 0.000 | 10 18.00 1407.75 -101.00 20 0 0.000 0.000
708 e6bb16d7 | 10 45.00 -210.00 1399.25 0 0 0.000 0.000 | 10 24.00 1386.13 -101.00 20 0 0.000 0.000
714 e6bb16d7 | 10 51.00 -210.00 1399.25 0 0 0.000 0.000 | 10 30.00 1364.51 -101.00 20 0 0.000 0.000
720 e6bb16d7 | 10 57.00 -210.00 1399.25 0 0 0.000 0.000 | 10 36.00 1342.89 -101.00 20 0 0.000 0.000
726 e6bb16d7 | 10 63.00 -210.00 1399.25 0 0 0.000 0.000 | 10 42.00 1321.27 -101.00 20 0 0.000 0.000
732 e6bb16d7 | 10 69.00 -210.00 1399.25 0 0 0.000 0.000 | 10 48.00 1320.00 -101.00 20 0 0.000 0.000
738 e6bb16d7 | 10 75.00 -210.00 1399.25 0 0 0.000 0.000 | 10 4.00 1320.00 -101.00 20 0 0.000 0.000
744 e6bb16d7 | 10 1.00 -210.00 1399.25 0 0 0.000 0.000 | 10 10.00 1320.00 -101.00 20 0 0.000 0.000
750 e6bb16d7 | 10 7.00 -210.00 1399.25 0 0 0.000 0.000 | 10 16.00 1320.00 -101.00 20 0 0.000 0.000
756 e6bb16d7 | 10 13.00 -210.00 1399.25 0 0 0.000 0.000 | 10 22.00 1320.00 -101.00 20 0 0.000 0.000
762 e6bb16d7 | 10 19.00 -210.00 1399.25 0 0 0.000 0.000 | 10 28.00 1320.00 -101.00 20 0 0.000 0.000
768 e6bb16d7 | 10 25.00 -210.00 1399.25 0 0 0.000 0.000 | 10 34.00 1320.00 -101.00 20 0 0.000 0.000
774 e6bb16d7 | 10 31.00 -210.00 1399.25 0 0 0.000 0.000 | 10 40.00 1320.00 -101.00 20 0 0.000 0.000
780 e6bb16d7 | 10 37.00 -210.00 1399.25 0 0 0.000 0.000 | 10 46.00 1320.00 -101.00 20 0 0.000 0.000
785 fa5aea62
786 fa5aea62 | 10 43.00 -210.00 1399.25 0 0 0.000 0.000 | 25 1.00 1320.00 32.20 20 0 0.000 0.000
791 9e1f7971
792 9e1f7971 | 10 49.00 -210.00 1399.25 0 0 0.000 0.000 | 25 7.00 1240.80 395.80 20 0 0.000 0.000
798 9e1f7971 | 10 55.00 -210.00 1399.25 0 0 0.000 0.000 | 25 13.00 1125.00 673.00 20 0 0.000 0.000
804 9e1f7971 | 10 61.00 -210.00 1399.25 0 0 0.000 0.000 | 25 19.00 1016.40 863.80 20 0 0.000 0.000
810 9e1f7971 | 10 67.00 -210.00 1399.25 0 0 0.000 0.000 | 25 25.00 915.00 968.20 20 0 0.000 0.000
816 9e1f7971 | 10 73.00 -210.00 1399.25 0 0 0.000 0.000 | 25 31.00 820.80 986.20 20 0 0.000 0.000
822 9e1f7971 | 10 79.00 -210.00 1399.25 0 0 0.000 0.000 | 25 37.00 733.80 917.80 20 0 0.000 0.000
828 9e1f7971 | 10 5.00 -210.00 1399.25 0 0 0.000 0.000 | 25 43.00 654.00 763.00 20 0 0.000 0.000
834 9e1f7971 | 10 11.00 -210.00 1399.25 0 0 0.000 0.000 | 25 49.00 581.40 524.00 20 0 0.000 0.000
835 9ad4bb73
840 9ad4bb73 | 10 17.00 -210.00 1399.25 0 0 0.000 0.000 | 31 5.00 564.60 510.00 20 0 0.000 0.000
846 9ad4bb73 | 10 23.00 -210.00 1399.25 0 0 0.000 0.000 | 10 4.00 564.60 510.00 20 0 0.000 0.000
852 9ad4bb73 | 10 29.00 -210.00 1399.25 0 0 0.000 0.000 | 10 10.00 564.60 510.00 20 0 0.000 0.000
858 9ad4bb73 | 10 35.00 -210.00 1399.25 0 0 0.000 0.000 | 10 16.00 564.60 510.00 20 0 0.000 0.000
864 9ad4bb73 | 10 41.00 -210.00 1399.25 0 0 0.000 0.000 | 10 22.00 564.60 510.00 20 0 0.000 0.000
870 9ad4bb73 | 10 47.00 -210.00 1399.25 0 0 0.000 0.000 | 10 28.00 564.60 510.00 20 0 0.000 0.000
876 9ad4bb73 | 10 53.00 -210.00 1399.25 0 0 0.000 0.000 | 10 34.00 564.60 510.00 20 0 0.000 0.000
882 9ad4bb73 | 10 59.00 -210.00 1399.25 0 0 0.000 0.000 | 10 40.00 564.60 510.00 20 0 0.000 0.000
888 9ad4bb73 | 10 65.00 -210.00 1399.25 0 0 0.000 0.000 | 10 46.00 564.60 510.00 20 0 0.000 0.000
894 9ad4bb73 | 10 71.00 -210.00 1399.25 0 0 0.000 0.000 | 10 2.00 564.60 510.00 20 0 0.000 0.000
900 9ad4bb73 | 10 77.00 -210.00 1399.25 0 0 0.000 0.000 | 10 8.00 564.60 510.00 20 0 0.000 0.000
906 9ad4bb73 | 10 3.00 -210.00 1399.25 0 0 0.000 0.000 | 10 14.00 564.60 510.00 20 0 0.000 0.000
912 9ad4bb73 | 10 9.00 -210.00 1399.25 0 0 0.000 0.000 | 10 20.00 564.60 510.00 20 0 0.000 0.000
918 9ad4bb73 | 10 15.00 -210.00 1399.25 0 0 0.000 0.000 | 10 26.00 564.60 510.00 20 0 0.000 0.000
924 9ad4bb73 | 10 21.00 -210.00 1399.25 0 0 0.000 0.000 | 10 32.00 564.60 510.00 20 0 0.000 0.000
930 9ad4bb73 | 10 27.00 -210.00 1399.25 0 0 0.000 0.000 | 10 38.00 564.60 510.00 20 0 0.000 0.000
936 9ad4bb73 | 10 33.00 -210.00 1399.25 0 0 0.000 0.000 | 10 44.00 564.60 510.00 20 0 0.000 0.000
942 9ad4bb73 | 10 39.00 -210.00 1399.25 0 0 0.000 0.000 | 10 0.00 564.60 510.00 20 0 0.000 0.000
948 9ad4bb73 | 10 45.00 -210.00 1399.25 0 0 0.000 0.000 | 10 6.00 564.60 510.00 20 0 0.000 0.000
954 9ad4bb73 | 10 51.00 -210.00 1399.25 0 0 0.000 0.000 | 10 12.00 564.60 510.00 20 0 0.000 0.000
960 9ad4bb73 | 10 57.00 -210.00 1399.25 0 0 0.000 0.000 | 10 18.00 564.60 510.00 20 0 0.000 0.000
966 9ad4bb73 | 10 63.00 -210.00 1399.25 0 0 0.000 0.000 | 10 24.00 564.60 510.00 20 0 0.000 0.000
972 9ad4bb73 | 10 69.00 -210.00 1399.25 0 0 0.000 0.000 | 10 30.00 564.60 510.00 20 0 0.000 0.000
978 9ad4bb73 | 10 75.00 -210.00 1399.25 0 0 0.000 0.000 | 10 36.00 564.60 510.00 20 0 0.000 0.000
984 9ad4bb73 | 10 1.00 -210.00 1399.25 0 0 0.000 0.000 | 10 42.00 564.60 510.00 20 0 0.000 0.000
990 9ad4bb73 | 10 7.00 -210.00 1399.25 0 0 0.000 0.000 | 10 48.00 564.60 510.00 20 0 0.000 0.000
996 9ad4bb73 | 10 13.00 -210.00 1399.25 0 0 0.000 0.000 | 10 4.00 564.60 510.00 20 0 0.000 0.000
1002 9ad4bb73 | 10 19.00 -210.00 1399.25 0 0 0.000 0.000 | 10 10.00 564.60 510.00 20 0 0.000 0.000
1008 9ad4bb73 | 10 25.00 -210.00 1399.25 0 0 0.000 0.000 | 10 16.00 564.60 510.00 20 0 0.000 0.000
1014 9ad4bb73 | 10 31.00 -210.00 1399.25 0 0 0.000 0.000 | 10 22.00 564.60 510.00 20 0 0.000 0.000
1020 9ad4bb73 | 10 37.00 -210.00 1399.25 0 0 0.000 0.000 | 10 28.00 564.60 510.00 20 0 0.000 0.000
1026 9ad4bb73 | 10 43.00 -210.00 1399.25 0 0 0.000 0.000 | 10 34.00 564.60 510.00 20 0 0.000 0.000
1032 9ad4bb73 | 10 49.00 -210.00 1399.25 0 0 0.000 0.000 | 10 40.00 564.60 510.00 20 0 0.000 0.000
1038 9ad4bb73 | 10 55.00 -210.00 1399.25 0 0 0.000 0.000 | 10 46.00 564.60 510.00 20 0 0.000 0.000
1044 9ad4bb73 | 10 61.00 -210.00 1399.25 0 0 0.000 0.000 | 10 2.00 564.60 510.00 20 0 0.000 0.000
1050 9ad4bb73 | 10 67.00 -210.00 1399.25 0 0 0.000 0.000 | 10 8.00 564.60 510.00 20 0 0.000 0.000
1056 9ad4bb73 | 10 73.00 -210.00 1399.25 0 0 0.000 0.000 | 10 14.00 564.60 510.00 20 0 0.000 0.000
1062 9ad4bb73 | 10 79.00 -210.00 1399.25 0 0 0.000 0.000 | 10 20.00 564.60 510.00 20 0 0.000 0.000
1068 9ad4bb73 | 10 5.00 -210.00 1399.25 0 0 0.000 0.000 | 10 26.00 564.60 510.00 20 0 0.000 0.000
1074 9ad4bb73 | 10 11.00 -210.00 1399.25 0 0 0.000 0.000 | 10 32.00 564.60 510.00 20 0 0.000 0.000
1080 9ad4bb73 | 10 17.00 -210.00 1399.25 0 0 0.000 0.000 | 10 38.00 564.60 510.00 20 0 0.000 0.000
1086 9ad4bb73 | 10 23.00 -210.00 1399.25 0 0 0.000 0.000 | 10 44.00 564.60 510.00 20 0 0.000 0.000
1092 9ad4bb73 | 10 29.00 -210.00 1399.25 0 0 0.000 0.000 | 10 0.00 564.60 510.00 20 0 0.000 0.000
1098 9ad4bb73 | 10 35.00 -210.00 1399.25 0 0 0.000 0.000 | 10 6.00 564.60 510.00 20 0 0.000 0.000
1100 9ad4bb73 | 10 37.00 -210.00 1399.25 0 0 0.000 0.000 | 10 8.00 564.60 510.00 20 0 0.000 0.000
";

#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn a_forced_clefairy_metronome_as_on_the_n64() {
    forced_ball(FORCED_CLEFAIRY, 12, "nITKindPippi from a forced Poké Ball");
}

/// `b36poke13` (`warp_link`, [`FORCED_BALL_ROUTE`],
/// `dITManagerForceMonsterKind` = 13 poked at frame 2): `nITKindMew`.
/// Mew flies off (no 1P bonus on the forced path). Rows 396-1100: every 6th frame, the seed's changes, and
/// every frame round a hit; the dummy is Training's CPU (its getup at
/// 630).
const FORCED_MEW: &str = "
210 69521988
211 a6e9c9a6
212 fe87df14
213 c40df552
327 491c86cd
332 4db793af
333 074d495a
396 074d495a | 10 53.00 -210.00 1399.25 0 0 0.000 0.000 | 57 5.00 2369.91 1575.09 20 0 0.000 0.000 | 19 385.58 1507.32 0.000 0.000
402 074d495a | 10 59.00 -210.00 1399.25 0 0 0.000 0.000 | 57 11.00 2369.91 1311.09 20 0 0.000 0.000 | 19 385.58 1507.32 0.000 0.000
408 074d495a | 10 65.00 -210.00 1399.25 0 0 0.000 0.000 | 57 17.00 2369.91 1047.09 20 0 0.000 0.000 | 44 385.58 1930.32 0.000 16.000
414 074d495a | 10 71.00 -210.00 1399.25 0 0 0.000 0.000 | 57 23.00 2369.91 783.09 20 0 0.000 0.000 | 44 385.58 2026.32 0.000 16.000
420 074d495a | 10 77.00 -210.00 1399.25 0 0 0.000 0.000 | 57 0.00 2369.91 519.09 20 0 0.000 0.000 | 44 385.58 2122.32 0.000 16.000
425 4dc8aab5
426 428b763e | 10 3.00 -210.00 1399.25 0 0 0.000 0.000 | 57 6.00 2369.91 255.09 20 0 0.000 0.000 | 44 369.58 2147.12 -8.000 -19.200
427 6bfcdd85
430 01202500
431 f59a2fc3
432 f59a2fc3 | 10 9.00 -210.00 1399.25 0 0 0.000 0.000 | 57 12.00 2369.91 -8.91 20 0 0.000 0.000 | 44 321.58 2048.72 -8.000 -14.400
433 3e1203c8
436 2e407ec7
438 2e407ec7 | 10 15.00 -210.00 1399.25 0 0 0.000 0.000 | 67 3.00 2359.10 -101.00 20 0 0.000 0.000 | 44 273.58 1979.12 -8.000 -9.600
439 6032ec9c
442 409dcdf5
444 409dcdf5 | 10 21.00 -210.00 1399.25 0 0 0.000 0.000 | 67 9.00 2337.48 -101.00 20 0 0.000 0.000 | 44 225.58 1938.32 -8.000 -4.800
445 3699c6a2
448 c0807f19
450 c0807f19 | 10 27.00 -210.00 1399.25 0 0 0.000 0.000 | 67 15.00 2315.86 -101.00 20 0 0.000 0.000 | 44 177.58 1926.32 -8.000 0.000
451 c22d0c04
454 2c49e07d
456 2c49e07d | 10 33.00 -210.00 1399.25 0 0 0.000 0.000 | 67 21.00 2294.23 -101.00 20 0 0.000 0.000 | 44 129.58 1943.12 -8.000 4.800
457 7dd156ca
460 0677d621
462 0677d621 | 10 39.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2272.61 -101.00 20 0 0.000 0.000 | 44 81.58 1988.72 -8.000 9.600
463 a49e586c
466 5fa48405
468 5fa48405 | 10 45.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2250.99 -101.00 20 0 0.000 0.000 | 44 33.58 2063.12 -8.000 14.400
469 e0486980
472 9481601f
474 9481601f | 10 51.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2229.37 -101.00 20 0 0.000 0.000 | 44 -14.42 2166.32 -8.000 19.200
475 0c15da12
478 627056e3
480 627056e3 | 10 57.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2207.75 -101.00 20 0 0.000 0.000 | 44 -62.42 2298.32 -8.000 24.000
481 c86aba86
484 182f07e7
486 182f07e7 | 10 63.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2186.13 -101.00 20 0 0.000 0.000 | 44 -110.42 2459.12 -8.000 28.800
487 5ed0f83c
490 7cfef72b
492 7cfef72b | 10 69.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2164.51 -101.00 20 0 0.000 0.000 | 44 -158.42 2648.72 -8.000 33.600
493 6cf4de50
496 68f52139
498 68f52139 | 10 75.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2142.89 -101.00 20 0 0.000 0.000 | 44 -206.42 2867.12 -8.000 38.400
499 138a60b6
502 4325149d
504 4325149d | 10 1.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2121.27 -101.00 20 0 0.000 0.000 | 44 -254.42 3114.32 -8.000 43.200
505 0d6a896a
508 30f1bc41
510 30f1bc41 | 10 7.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2099.65 -101.00 20 0 0.000 0.000 | 44 -302.42 3390.32 -8.000 48.000
511 30a7cc0c
514 2fe73c25
516 2fe73c25 | 10 13.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2078.02 -101.00 20 0 0.000 0.000 | 44 -350.42 3695.12 -8.000 52.800
517 0728b292
520 6d4ec963
522 6d4ec963 | 10 19.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2056.40 -101.00 20 0 0.000 0.000 | 44 -398.42 4028.72 -8.000 57.600
523 e6b4db06
526 50a96267
528 50a96267 | 10 25.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2034.78 -101.00 20 0 0.000 0.000 | 44 -446.42 4391.12 -8.000 62.400
529 4963d4bc
532 4ebfb9ab
534 4ebfb9ab | 10 31.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 2013.16 -101.00 20 0 0.000 0.000 | 44 -494.42 4782.32 -8.000 67.200
535 456d42d0
538 21208fb9
540 21208fb9 | 10 37.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1991.54 -101.00 20 0 0.000 0.000 | 44 -542.42 5202.32 -8.000 72.000
541 06076136
544 808e2b1d
546 808e2b1d | 10 43.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1969.92 -101.00 20 0 0.000 0.000 | 44 -590.42 5651.12 -8.000 76.800
547 ccffc8b8
550 d4be1cf7
552 d4be1cf7 | 10 49.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1948.29 -101.00 20 0 0.000 0.000 | 44 -638.42 6128.72 -8.000 81.600
553 0af00c0a
556 4f5b953b
558 4f5b953b | 10 55.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1926.67 -101.00 20 0 0.000 0.000 | 44 -686.42 6635.12 -8.000 86.400
559 8a41affe
562 3a721fbf
564 3a721fbf | 10 61.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1905.05 -101.00 20 0 0.000 0.000 | 44 -734.42 7170.32 -8.000 91.200
565 04f0e5f4
568 ba4cc083
570 ba4cc083 | 10 67.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1883.43 -101.00 20 0 0.000 0.000 | 44 -782.42 7734.32 -8.000 96.000
571 2cd54ba6
574 0914bb87
576 0914bb87 | 10 73.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1861.81 -101.00 20 0 0.000 0.000 | 44 -830.42 8327.12 -8.000 100.800
577 387cf05c
580 389594cb
582 389594cb | 10 79.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1840.19 -101.00 20 0 0.000 0.000 | 44 -878.42 8948.73 -8.000 105.600
583 73b19c4e
586 f49f8e55
588 f49f8e55 | 10 5.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1818.57 -101.00 20 0 0.000 0.000
589 838f3990
594 838f3990 | 10 11.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1796.94 -101.00 20 0 0.000 0.000
600 838f3990 | 10 17.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1775.32 -101.00 20 0 0.000 0.000
606 838f3990 | 10 23.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1753.70 -101.00 20 0 0.000 0.000
612 838f3990 | 10 29.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1732.08 -101.00 20 0 0.000 0.000
613 2d733213
618 2d733213 | 10 35.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1710.46 -101.00 20 0 0.000 0.000
624 2d733213 | 10 41.00 -210.00 1399.25 0 0 0.000 0.000 | 69 0.00 1688.84 -101.00 20 0 0.000 0.000
630 2d733213 | 10 47.00 -210.00 1399.25 0 0 0.000 0.000 | 79 4.00 1667.21 -101.00 20 0 0.000 0.000
636 2d733213 | 10 53.00 -210.00 1399.25 0 0 0.000 0.000 | 79 10.00 1645.59 -101.00 20 0 0.000 0.000
642 2d733213 | 10 59.00 -210.00 1399.25 0 0 0.000 0.000 | 79 16.00 1623.97 -101.00 20 0 0.000 0.000
648 2d733213 | 10 65.00 -210.00 1399.25 0 0 0.000 0.000 | 79 22.00 1602.35 -101.00 20 0 0.000 0.000
654 2d733213 | 10 71.00 -210.00 1399.25 0 0 0.000 0.000 | 79 28.00 1580.73 -101.00 20 0 0.000 0.000
660 2d733213 | 10 77.00 -210.00 1399.25 0 0 0.000 0.000 | 79 34.00 1559.11 -101.00 20 0 0.000 0.000
666 2d733213 | 10 3.00 -210.00 1399.25 0 0 0.000 0.000 | 79 40.00 1537.48 -101.00 20 0 0.000 0.000
672 2d733213 | 10 9.00 -210.00 1399.25 0 0 0.000 0.000 | 79 46.00 1515.86 -101.00 20 0 0.000 0.000
678 2d733213 | 10 15.00 -210.00 1399.25 0 0 0.000 0.000 | 10 2.00 1494.24 -101.00 20 0 0.000 0.000
684 2d733213 | 10 21.00 -210.00 1399.25 0 0 0.000 0.000 | 10 8.00 1472.62 -101.00 20 0 0.000 0.000
690 2d733213 | 10 27.00 -210.00 1399.25 0 0 0.000 0.000 | 10 14.00 1451.00 -101.00 20 0 0.000 0.000
696 2d733213 | 10 33.00 -210.00 1399.25 0 0 0.000 0.000 | 10 20.00 1429.38 -101.00 20 0 0.000 0.000
702 2d733213 | 10 39.00 -210.00 1399.25 0 0 0.000 0.000 | 10 26.00 1407.75 -101.00 20 0 0.000 0.000
708 2d733213 | 10 45.00 -210.00 1399.25 0 0 0.000 0.000 | 10 32.00 1386.13 -101.00 20 0 0.000 0.000
714 2d733213 | 10 51.00 -210.00 1399.25 0 0 0.000 0.000 | 10 38.00 1364.51 -101.00 20 0 0.000 0.000
720 2d733213 | 10 57.00 -210.00 1399.25 0 0 0.000 0.000 | 10 44.00 1342.89 -101.00 20 0 0.000 0.000
726 2d733213 | 10 63.00 -210.00 1399.25 0 0 0.000 0.000 | 10 0.00 1321.27 -101.00 20 0 0.000 0.000
732 2d733213 | 10 69.00 -210.00 1399.25 0 0 0.000 0.000 | 10 6.00 1320.00 -101.00 20 0 0.000 0.000
738 2d733213 | 10 75.00 -210.00 1399.25 0 0 0.000 0.000 | 10 12.00 1320.00 -101.00 20 0 0.000 0.000
744 2d733213 | 10 1.00 -210.00 1399.25 0 0 0.000 0.000 | 10 18.00 1320.00 -101.00 20 0 0.000 0.000
750 2d733213 | 10 7.00 -210.00 1399.25 0 0 0.000 0.000 | 10 24.00 1320.00 -101.00 20 0 0.000 0.000
756 2d733213 | 10 13.00 -210.00 1399.25 0 0 0.000 0.000 | 10 30.00 1320.00 -101.00 20 0 0.000 0.000
762 2d733213 | 10 19.00 -210.00 1399.25 0 0 0.000 0.000 | 10 36.00 1320.00 -101.00 20 0 0.000 0.000
768 2d733213 | 10 25.00 -210.00 1399.25 0 0 0.000 0.000 | 10 42.00 1320.00 -101.00 20 0 0.000 0.000
774 2d733213 | 10 31.00 -210.00 1399.25 0 0 0.000 0.000 | 10 48.00 1320.00 -101.00 20 0 0.000 0.000
780 2d733213 | 10 37.00 -210.00 1399.25 0 0 0.000 0.000 | 10 4.00 1320.00 -101.00 20 0 0.000 0.000
785 88afc06e
786 88afc06e | 10 43.00 -210.00 1399.25 0 0 0.000 0.000 | 25 1.00 1320.00 32.20 20 0 0.000 0.000
791 4ecf3c0d
792 4ecf3c0d | 10 49.00 -210.00 1399.25 0 0 0.000 0.000 | 25 7.00 1240.80 395.80 20 0 0.000 0.000
798 4ecf3c0d | 10 55.00 -210.00 1399.25 0 0 0.000 0.000 | 25 13.00 1125.00 673.00 20 0 0.000 0.000
804 4ecf3c0d | 10 61.00 -210.00 1399.25 0 0 0.000 0.000 | 25 19.00 1016.40 863.80 20 0 0.000 0.000
810 4ecf3c0d | 10 67.00 -210.00 1399.25 0 0 0.000 0.000 | 25 25.00 915.00 968.20 20 0 0.000 0.000
816 4ecf3c0d | 10 73.00 -210.00 1399.25 0 0 0.000 0.000 | 25 31.00 820.80 986.20 20 0 0.000 0.000
822 4ecf3c0d | 10 79.00 -210.00 1399.25 0 0 0.000 0.000 | 25 37.00 733.80 917.80 20 0 0.000 0.000
828 4ecf3c0d | 10 5.00 -210.00 1399.25 0 0 0.000 0.000 | 25 43.00 654.00 763.00 20 0 0.000 0.000
834 4ecf3c0d | 10 11.00 -210.00 1399.25 0 0 0.000 0.000 | 25 49.00 581.40 524.00 20 0 0.000 0.000
835 c5a1f2ef
840 c5a1f2ef | 10 17.00 -210.00 1399.25 0 0 0.000 0.000 | 31 5.00 564.60 510.00 20 0 0.000 0.000
846 c5a1f2ef | 10 23.00 -210.00 1399.25 0 0 0.000 0.000 | 10 4.00 564.60 510.00 20 0 0.000 0.000
852 c5a1f2ef | 10 29.00 -210.00 1399.25 0 0 0.000 0.000 | 10 10.00 564.60 510.00 20 0 0.000 0.000
858 c5a1f2ef | 10 35.00 -210.00 1399.25 0 0 0.000 0.000 | 10 16.00 564.60 510.00 20 0 0.000 0.000
864 c5a1f2ef | 10 41.00 -210.00 1399.25 0 0 0.000 0.000 | 10 22.00 564.60 510.00 20 0 0.000 0.000
870 c5a1f2ef | 10 47.00 -210.00 1399.25 0 0 0.000 0.000 | 10 28.00 564.60 510.00 20 0 0.000 0.000
876 c5a1f2ef | 10 53.00 -210.00 1399.25 0 0 0.000 0.000 | 10 34.00 564.60 510.00 20 0 0.000 0.000
882 c5a1f2ef | 10 59.00 -210.00 1399.25 0 0 0.000 0.000 | 10 40.00 564.60 510.00 20 0 0.000 0.000
888 c5a1f2ef | 10 65.00 -210.00 1399.25 0 0 0.000 0.000 | 10 46.00 564.60 510.00 20 0 0.000 0.000
894 c5a1f2ef | 10 71.00 -210.00 1399.25 0 0 0.000 0.000 | 10 2.00 564.60 510.00 20 0 0.000 0.000
900 c5a1f2ef | 10 77.00 -210.00 1399.25 0 0 0.000 0.000 | 10 8.00 564.60 510.00 20 0 0.000 0.000
906 c5a1f2ef | 10 3.00 -210.00 1399.25 0 0 0.000 0.000 | 10 14.00 564.60 510.00 20 0 0.000 0.000
912 c5a1f2ef | 10 9.00 -210.00 1399.25 0 0 0.000 0.000 | 10 20.00 564.60 510.00 20 0 0.000 0.000
918 c5a1f2ef | 10 15.00 -210.00 1399.25 0 0 0.000 0.000 | 10 26.00 564.60 510.00 20 0 0.000 0.000
924 c5a1f2ef | 10 21.00 -210.00 1399.25 0 0 0.000 0.000 | 10 32.00 564.60 510.00 20 0 0.000 0.000
930 c5a1f2ef | 10 27.00 -210.00 1399.25 0 0 0.000 0.000 | 10 38.00 564.60 510.00 20 0 0.000 0.000
936 c5a1f2ef | 10 33.00 -210.00 1399.25 0 0 0.000 0.000 | 10 44.00 564.60 510.00 20 0 0.000 0.000
942 c5a1f2ef | 10 39.00 -210.00 1399.25 0 0 0.000 0.000 | 10 0.00 564.60 510.00 20 0 0.000 0.000
948 c5a1f2ef | 10 45.00 -210.00 1399.25 0 0 0.000 0.000 | 10 6.00 564.60 510.00 20 0 0.000 0.000
954 c5a1f2ef | 10 51.00 -210.00 1399.25 0 0 0.000 0.000 | 10 12.00 564.60 510.00 20 0 0.000 0.000
960 c5a1f2ef | 10 57.00 -210.00 1399.25 0 0 0.000 0.000 | 10 18.00 564.60 510.00 20 0 0.000 0.000
966 c5a1f2ef | 10 63.00 -210.00 1399.25 0 0 0.000 0.000 | 10 24.00 564.60 510.00 20 0 0.000 0.000
972 c5a1f2ef | 10 69.00 -210.00 1399.25 0 0 0.000 0.000 | 10 30.00 564.60 510.00 20 0 0.000 0.000
978 c5a1f2ef | 10 75.00 -210.00 1399.25 0 0 0.000 0.000 | 10 36.00 564.60 510.00 20 0 0.000 0.000
984 c5a1f2ef | 10 1.00 -210.00 1399.25 0 0 0.000 0.000 | 10 42.00 564.60 510.00 20 0 0.000 0.000
990 c5a1f2ef | 10 7.00 -210.00 1399.25 0 0 0.000 0.000 | 10 48.00 564.60 510.00 20 0 0.000 0.000
996 c5a1f2ef | 10 13.00 -210.00 1399.25 0 0 0.000 0.000 | 10 4.00 564.60 510.00 20 0 0.000 0.000
1002 c5a1f2ef | 10 19.00 -210.00 1399.25 0 0 0.000 0.000 | 10 10.00 564.60 510.00 20 0 0.000 0.000
1008 c5a1f2ef | 10 25.00 -210.00 1399.25 0 0 0.000 0.000 | 10 16.00 564.60 510.00 20 0 0.000 0.000
1014 c5a1f2ef | 10 31.00 -210.00 1399.25 0 0 0.000 0.000 | 10 22.00 564.60 510.00 20 0 0.000 0.000
1020 c5a1f2ef | 10 37.00 -210.00 1399.25 0 0 0.000 0.000 | 10 28.00 564.60 510.00 20 0 0.000 0.000
1026 c5a1f2ef | 10 43.00 -210.00 1399.25 0 0 0.000 0.000 | 10 34.00 564.60 510.00 20 0 0.000 0.000
1032 c5a1f2ef | 10 49.00 -210.00 1399.25 0 0 0.000 0.000 | 10 40.00 564.60 510.00 20 0 0.000 0.000
1038 c5a1f2ef | 10 55.00 -210.00 1399.25 0 0 0.000 0.000 | 10 46.00 564.60 510.00 20 0 0.000 0.000
1044 c5a1f2ef | 10 61.00 -210.00 1399.25 0 0 0.000 0.000 | 10 2.00 564.60 510.00 20 0 0.000 0.000
1050 c5a1f2ef | 10 67.00 -210.00 1399.25 0 0 0.000 0.000 | 10 8.00 564.60 510.00 20 0 0.000 0.000
1056 c5a1f2ef | 10 73.00 -210.00 1399.25 0 0 0.000 0.000 | 10 14.00 564.60 510.00 20 0 0.000 0.000
1062 c5a1f2ef | 10 79.00 -210.00 1399.25 0 0 0.000 0.000 | 10 20.00 564.60 510.00 20 0 0.000 0.000
1068 c5a1f2ef | 10 5.00 -210.00 1399.25 0 0 0.000 0.000 | 10 26.00 564.60 510.00 20 0 0.000 0.000
1074 c5a1f2ef | 10 11.00 -210.00 1399.25 0 0 0.000 0.000 | 10 32.00 564.60 510.00 20 0 0.000 0.000
1080 c5a1f2ef | 10 17.00 -210.00 1399.25 0 0 0.000 0.000 | 10 38.00 564.60 510.00 20 0 0.000 0.000
1086 c5a1f2ef | 10 23.00 -210.00 1399.25 0 0 0.000 0.000 | 10 44.00 564.60 510.00 20 0 0.000 0.000
1092 c5a1f2ef | 10 29.00 -210.00 1399.25 0 0 0.000 0.000 | 10 0.00 564.60 510.00 20 0 0.000 0.000
1098 c5a1f2ef | 10 35.00 -210.00 1399.25 0 0 0.000 0.000 | 10 6.00 564.60 510.00 20 0 0.000 0.000
1100 c5a1f2ef | 10 37.00 -210.00 1399.25 0 0 0.000 0.000 | 10 8.00 564.60 510.00 20 0 0.000 0.000
";

#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn a_forced_mew_flies_off_as_on_the_n64() {
    forced_ball(FORCED_MEW, 13, "nITKindMew from a forced Poké Ball");
}

/// `hfire`: Mario's Fireball bounces along the floor into the dummy (t 86, 7 percent).
const MARIO_FIREBALL_HITS_THE_DUMMY: &str = "
60 00000001 | 223 1.00 -210.00 1399.25 0 0 0.000 0.000 | 10 11.00 765.00 1413.09 0 0 0.000 0.000
61 00000001 | 223 2.00 -210.00 1399.25 0 0 0.000 0.000 | 10 12.00 765.00 1413.09 0 0 0.000 0.000
62 00000001 | 223 3.00 -210.00 1399.25 0 0 0.000 0.000 | 10 13.00 765.00 1413.09 0 0 0.000 0.000
63 00000001 | 223 4.00 -210.00 1399.25 0 0 0.000 0.000 | 10 14.00 765.00 1413.09 0 0 0.000 0.000
64 00000001 | 223 5.00 -210.00 1399.25 0 0 0.000 0.000 | 10 15.00 765.00 1413.09 0 0 0.000 0.000
65 00000001 | 223 6.00 -210.00 1399.25 0 0 0.000 0.000 | 10 16.00 765.00 1413.09 0 0 0.000 0.000
66 00000001 | 223 7.00 -210.00 1399.25 0 0 0.000 0.000 | 10 17.00 765.00 1413.09 0 0 0.000 0.000
67 00000001 | 223 8.00 -210.00 1399.25 0 0 0.000 0.000 | 10 18.00 765.00 1413.09 0 0 0.000 0.000
68 00000001 | 223 9.00 -210.00 1399.25 0 0 0.000 0.000 | 10 19.00 765.00 1413.09 0 0 0.000 0.000
69 00000001 | 223 10.00 -210.00 1399.25 0 0 0.000 0.000 | 10 20.00 765.00 1413.09 0 0 0.000 0.000
70 00000001 | 223 11.00 -210.00 1399.25 0 0 0.000 0.000 | 10 21.00 765.00 1413.09 0 0 0.000 0.000
71 00000001 | 223 12.00 -210.00 1399.25 0 0 0.000 0.000 | 10 22.00 765.00 1413.09 0 0 0.000 0.000
72 00000001 | 223 13.00 -210.00 1399.25 0 0 0.000 0.000 | 10 23.00 765.00 1413.09 0 0 0.000 0.000
73 00000001 | 223 14.00 -210.00 1399.25 0 0 0.000 0.000 | 10 24.00 765.00 1413.09 0 0 0.000 0.000
74 00000001 | 223 15.00 -210.00 1399.25 0 0 0.000 0.000 | 10 25.00 765.00 1413.09 0 0 0.000 0.000
75 00000001 | 223 16.00 -210.00 1399.25 0 0 0.000 0.000 | 10 26.00 765.00 1413.09 0 0 0.000 0.000 |  | 0 22.14 1611.90 49.810 -5.558
76 00000001 | 223 17.00 -210.00 1399.25 0 0 0.000 0.000 | 10 27.00 765.00 1413.09 0 0 0.000 0.000 |  | 0 71.95 1605.14 49.810 -6.758
77 00000001 | 223 18.00 -210.00 1399.25 0 0 0.000 0.000 | 10 28.00 765.00 1413.09 0 0 0.000 0.000 |  | 0 121.76 1597.18 49.810 -7.958
78 00000001 | 223 19.00 -210.00 1399.25 0 0 0.000 0.000 | 10 29.00 765.00 1413.09 0 0 0.000 0.000 |  | 0 171.57 1588.03 49.810 -9.158
79 00000001 | 223 20.00 -210.00 1399.25 0 0 0.000 0.000 | 10 30.00 765.00 1413.09 0 0 0.000 0.000 |  | 0 221.38 1577.67 49.810 -10.358
80 00000001 | 223 21.00 -210.00 1399.25 0 0 0.000 0.000 | 10 31.00 765.00 1413.09 0 0 0.000 0.000 |  | 0 271.19 1566.11 49.810 -11.558
81 00000001 | 223 22.00 -210.00 1399.25 0 0 0.000 0.000 | 10 32.00 765.00 1413.09 0 0 0.000 0.000 |  | 0 321.00 1553.35 49.810 -12.758
82 00000001 | 223 23.00 -210.00 1399.25 0 0 0.000 0.000 | 10 33.00 765.00 1413.09 0 0 0.000 0.000 |  | 0 370.81 1539.39 49.810 -13.958
83 00000001 | 223 24.00 -210.00 1399.25 0 0 0.000 0.000 | 10 34.00 765.00 1413.09 0 0 0.000 0.000 |  | 0 420.62 1524.24 49.810 -15.158
84 00000001 | 223 25.00 -210.00 1399.25 0 0 0.000 0.000 | 10 35.00 765.00 1413.09 0 0 0.000 0.000 |  | 0 470.43 1507.88 49.810 -16.358
85 00000001 | 223 26.00 -210.00 1399.25 0 0 0.000 0.000 | 10 36.00 765.00 1413.09 0 0 0.000 0.000 |  | 0 520.24 1490.32 49.810 -17.558
86 e7847115 | 223 27.00 -210.00 1399.25 0 0 0.000 0.000 | 40 1.00 765.00 1413.09 7 7 15.542 1.371
87 e7847115 | 223 28.00 -210.00 1399.25 0 0 0.000 0.000 | 40 1.00 765.00 1413.09 7 6 15.542 1.371
88 72ae7869 | 223 29.00 -210.00 1399.25 0 0 0.000 0.000 | 40 1.00 765.00 1413.09 7 5 15.542 1.371
89 72ae7869 | 223 30.00 -210.00 1399.25 0 0 0.000 0.000 | 40 1.00 765.00 1413.09 7 4 15.542 1.371
90 6df109fd | 223 31.00 -210.00 1399.25 0 0 0.000 0.000 | 40 1.00 765.00 1413.09 7 3 15.542 1.371
91 6df109fd | 223 32.00 -210.00 1399.25 0 0 0.000 0.000 | 40 1.00 765.00 1413.09 7 2 15.542 1.371
92 81eb59d1 | 223 33.00 -210.00 1399.25 0 0 0.000 0.000 | 40 1.00 765.00 1413.09 7 1 15.542 1.371
93 81eb59d1 | 223 34.00 -210.00 1399.25 0 0 0.000 0.000 | 40 2.00 779.05 1414.33 7 0 14.048 1.240
94 aea69ed3 | 223 35.00 -210.00 1399.25 0 0 0.000 0.000 | 40 3.00 791.60 1415.44 7 0 12.554 1.108
95 aea69ed3 | 223 36.00 -210.00 1399.25 0 0 0.000 0.000 | 40 4.00 802.66 1416.41 7 0 11.060 0.976
96 aea69ed3 | 223 37.00 -210.00 1399.25 0 0 0.000 0.000 | 40 5.00 812.23 1417.26 7 0 9.565 0.844
98 953cdbe5
102 b90c5187 | 223 43.00 -210.00 1399.25 0 0 0.000 0.000 | 40 11.00 838.24 1419.55 7 0 0.600 0.053
105 00994439
108 00994439 | 10 3.00 -210.00 1399.25 0 0 0.000 0.000 | 10 5.00 838.24 1419.55 7 0 0.000 0.000
114 00994439 | 10 9.00 -210.00 1399.25 0 0 0.000 0.000 | 10 11.00 838.24 1419.55 7 0 0.000 0.000
120 00994439 | 10 15.00 -210.00 1399.25 0 0 0.000 0.000 | 10 17.00 838.24 1419.55 7 0 0.000 0.000
126 00994439 | 10 21.00 -210.00 1399.25 0 0 0.000 0.000 | 10 23.00 838.24 1419.55 7 0 0.000 0.000
132 00994439 | 10 27.00 -210.00 1399.25 0 0 0.000 0.000 | 10 29.00 838.24 1419.55 7 0 0.000 0.000
138 00994439 | 10 33.00 -210.00 1399.25 0 0 0.000 0.000 | 10 35.00 838.24 1419.55 7 0 0.000 0.000
144 00994439 | 10 39.00 -210.00 1399.25 0 0 0.000 0.000 | 10 41.00 838.24 1419.55 7 0 0.000 0.000
150 00994439 | 10 45.00 -210.00 1399.25 0 0 0.000 0.000 | 10 47.00 838.24 1419.55 7 0 0.000 0.000
156 00994439 | 10 1.00 -210.00 1399.25 0 0 0.000 0.000 | 10 3.00 838.24 1419.55 7 0 0.000 0.000
162 00994439 | 10 7.00 -210.00 1399.25 0 0 0.000 0.000 | 10 9.00 838.24 1419.55 7 0 0.000 0.000
168 00994439 | 10 13.00 -210.00 1399.25 0 0 0.000 0.000 | 10 15.00 838.24 1419.55 7 0 0.000 0.000
174 00994439 | 10 19.00 -210.00 1399.25 0 0 0.000 0.000 | 10 21.00 838.24 1419.55 7 0 0.000 0.000
180 00994439 | 10 25.00 -210.00 1399.25 0 0 0.000 0.000 | 10 27.00 838.24 1419.55 7 0 0.000 0.000
186 00994439 | 10 31.00 -210.00 1399.25 0 0 0.000 0.000 | 10 33.00 838.24 1419.55 7 0 0.000 0.000
192 00994439 | 10 37.00 -210.00 1399.25 0 0 0.000 0.000 | 10 39.00 838.24 1419.55 7 0 0.000 0.000
198 00994439 | 10 43.00 -210.00 1399.25 0 0 0.000 0.000 | 10 45.00 838.24 1419.55 7 0 0.000 0.000
204 00994439 | 10 49.00 -210.00 1399.25 0 0 0.000 0.000 | 10 1.00 838.24 1419.55 7 0 0.000 0.000
210 00994439 | 10 5.00 -210.00 1399.25 0 0 0.000 0.000 | 10 7.00 838.24 1419.55 7 0 0.000 0.000
216 00994439 | 10 11.00 -210.00 1399.25 0 0 0.000 0.000 | 10 13.00 838.24 1419.55 7 0 0.000 0.000
220 00994439 | 10 15.00 -210.00 1399.25 0 0 0.000 0.000 | 10 17.00 838.24 1419.55 7 0 0.000 0.000
";

#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn a_fireball_hits_the_dummy_as_on_the_n64() {
    let replay = Replay {
        p1: FighterKind::Mario,
        route: "60:1:b",
        weapons: &fighter_weapons,
    };
    let Some(out) = replay.run(MARIO_FIREBALL_HITS_THE_DUMMY) else {
        return;
    };
    out.check("Fireball into the dummy");
}

/// `hblast`: Fox's Blaster shot crosses to the dummy (t 88, 6 percent).
const FOX_BLASTER_HITS_THE_DUMMY: &str = "
60 00000001 | 225 1.00 -210.00 1399.25 0 0 0.000 0.000 | 10 11.00 765.00 1413.09 0 0 0.000 0.000
61 00000001 | 225 2.00 -210.00 1399.25 0 0 0.000 0.000 | 10 12.00 765.00 1413.09 0 0 0.000 0.000
62 00000001 | 225 3.00 -210.00 1399.25 0 0 0.000 0.000 | 10 13.00 765.00 1413.09 0 0 0.000 0.000
63 00000001 | 225 4.00 -210.00 1399.25 0 0 0.000 0.000 | 10 14.00 765.00 1413.09 0 0 0.000 0.000
64 00000001 | 225 5.00 -210.00 1399.25 0 0 0.000 0.000 | 10 15.00 765.00 1413.09 0 0 0.000 0.000
65 00000001 | 225 6.00 -210.00 1399.25 0 0 0.000 0.000 | 10 16.00 765.00 1413.09 0 0 0.000 0.000
66 00000001 | 225 7.00 -210.00 1399.25 0 0 0.000 0.000 | 10 17.00 765.00 1413.09 0 0 0.000 0.000
67 00000001 | 225 8.00 -210.00 1399.25 0 0 0.000 0.000 | 10 18.00 765.00 1413.09 0 0 0.000 0.000
68 00000001 | 225 9.00 -210.00 1399.25 0 0 0.000 0.000 | 10 19.00 765.00 1413.09 0 0 0.000 0.000
69 00000001 | 225 10.00 -210.00 1399.25 0 0 0.000 0.000 | 10 20.00 765.00 1413.09 0 0 0.000 0.000
70 00000001 | 225 11.00 -210.00 1399.25 0 0 0.000 0.000 | 10 21.00 765.00 1413.09 0 0 0.000 0.000
71 00000001 | 225 12.00 -210.00 1399.25 0 0 0.000 0.000 | 10 22.00 765.00 1413.09 0 0 0.000 0.000
72 00000001 | 225 13.00 -210.00 1399.25 0 0 0.000 0.000 | 10 23.00 765.00 1413.09 0 0 0.000 0.000
73 00000001 | 225 14.00 -210.00 1399.25 0 0 0.000 0.000 | 10 24.00 765.00 1413.09 0 0 0.000 0.000
74 00000001 | 225 15.00 -210.00 1399.25 0 0 0.000 0.000 | 10 25.00 765.00 1413.09 0 0 0.000 0.000
75 00000001 | 225 16.00 -210.00 1399.25 0 0 0.000 0.000 | 10 26.00 765.00 1413.09 0 0 0.000 0.000
76 00000001 | 225 17.00 -210.00 1399.25 0 0 0.000 0.000 | 10 27.00 765.00 1413.09 0 0 0.000 0.000
77 00000001 | 225 18.00 -210.00 1399.25 0 0 0.000 0.000 | 10 28.00 765.00 1413.09 0 0 0.000 0.000
78 00000001 | 225 19.00 -210.00 1399.25 0 0 0.000 0.000 | 10 29.00 765.00 1413.09 0 0 0.000 0.000
79 00000001 | 225 20.00 -210.00 1399.25 0 0 0.000 0.000 | 10 30.00 765.00 1413.09 0 0 0.000 0.000
80 00000001 | 225 21.00 -210.00 1399.25 0 0 0.000 0.000 | 10 31.00 765.00 1413.09 0 0 0.000 0.000
81 00000001 | 225 22.00 -210.00 1399.25 0 0 0.000 0.000 | 10 32.00 765.00 1413.09 0 0 0.000 0.000
82 00000001 | 225 23.00 -210.00 1399.25 0 0 0.000 0.000 | 10 33.00 765.00 1413.09 0 0 0.000 0.000
83 00000001 | 225 24.00 -210.00 1399.25 0 0 0.000 0.000 | 10 34.00 765.00 1413.09 0 0 0.000 0.000
84 00000001 | 225 25.00 -210.00 1399.25 0 0 0.000 0.000 | 10 35.00 765.00 1413.09 0 0 0.000 0.000 |  | 1 41.14 1713.12 160.000 0.000
85 00000001 | 225 26.00 -210.00 1399.25 0 0 0.000 0.000 | 10 36.00 765.00 1413.09 0 0 0.000 0.000 |  | 1 201.14 1713.12 160.000 0.000
86 00000001 | 225 27.00 -210.00 1399.25 0 0 0.000 0.000 | 10 37.00 765.00 1413.09 0 0 0.000 0.000 |  | 1 361.14 1713.12 160.000 0.000
87 00000001 | 225 28.00 -210.00 1399.25 0 0 0.000 0.000 | 10 38.00 765.00 1413.09 0 0 0.000 0.000 |  | 1 521.14 1713.12 160.000 0.000
88 00000001 | 225 29.00 -210.00 1399.25 0 0 0.000 0.000 | 37 1.00 765.00 1413.09 6 7 19.795 3.490
89 00000001 | 225 30.00 -210.00 1399.25 0 0 0.000 0.000 | 37 1.00 765.00 1413.09 6 6 19.795 3.490
90 00000001 | 225 31.00 -210.00 1399.25 0 0 0.000 0.000 | 37 1.00 765.00 1413.09 6 5 19.795 3.490
91 00000001 | 225 32.00 -210.00 1399.25 0 0 0.000 0.000 | 37 1.00 765.00 1413.09 6 4 19.795 3.490
92 00000001 | 225 33.00 -210.00 1399.25 0 0 0.000 0.000 | 37 1.00 765.00 1413.09 6 3 19.795 3.490
93 00000001 | 225 34.00 -210.00 1399.25 0 0 0.000 0.000 | 37 1.00 765.00 1413.09 6 2 19.795 3.490
94 00000001 | 225 35.00 -210.00 1399.25 0 0 0.000 0.000 | 37 1.00 765.00 1413.09 6 1 19.795 3.490
95 00000001 | 225 36.00 -210.00 1399.25 0 0 0.000 0.000 | 37 2.00 783.12 1414.69 6 0 18.120 3.195
96 00000001 | 225 37.00 -210.00 1399.25 0 0 0.000 0.000 | 37 3.00 799.68 1416.15 6 0 16.556 1.461
102 00000001 | 225 43.00 -210.00 1399.25 0 0 0.000 0.000 | 37 9.00 867.64 1422.14 6 0 7.591 0.670
108 00000001 | 225 49.00 -210.00 1399.25 0 0 0.000 0.000 | 10 3.00 883.18 1423.52 6 0 0.000 0.000
114 00000001 | 10 0.00 -210.00 1399.25 0 0 0.000 0.000 | 10 9.00 883.18 1423.52 6 0 0.000 0.000
120 00000001 | 10 6.00 -210.00 1399.25 0 0 0.000 0.000 | 10 15.00 883.18 1423.52 6 0 0.000 0.000
126 00000001 | 10 12.00 -210.00 1399.25 0 0 0.000 0.000 | 10 21.00 883.18 1423.52 6 0 0.000 0.000
132 00000001 | 10 18.00 -210.00 1399.25 0 0 0.000 0.000 | 10 27.00 883.18 1423.52 6 0 0.000 0.000
138 00000001 | 10 24.00 -210.00 1399.25 0 0 0.000 0.000 | 10 33.00 883.18 1423.52 6 0 0.000 0.000
144 00000001 | 10 30.00 -210.00 1399.25 0 0 0.000 0.000 | 10 39.00 883.18 1423.52 6 0 0.000 0.000
150 00000001 | 10 36.00 -210.00 1399.25 0 0 0.000 0.000 | 10 45.00 883.18 1423.52 6 0 0.000 0.000
156 00000001 | 10 42.00 -210.00 1399.25 0 0 0.000 0.000 | 10 1.00 883.18 1423.52 6 0 0.000 0.000
162 00000001 | 10 48.00 -210.00 1399.25 0 0 0.000 0.000 | 10 7.00 883.18 1423.52 6 0 0.000 0.000
168 00000001 | 10 54.00 -210.00 1399.25 0 0 0.000 0.000 | 10 13.00 883.18 1423.52 6 0 0.000 0.000
174 00000001 | 10 60.00 -210.00 1399.25 0 0 0.000 0.000 | 10 19.00 883.18 1423.52 6 0 0.000 0.000
180 00000001 | 10 66.00 -210.00 1399.25 0 0 0.000 0.000 | 10 25.00 883.18 1423.52 6 0 0.000 0.000
186 00000001 | 10 2.00 -210.00 1399.25 0 0 0.000 0.000 | 10 31.00 883.18 1423.52 6 0 0.000 0.000
192 00000001 | 10 8.00 -210.00 1399.25 0 0 0.000 0.000 | 10 37.00 883.18 1423.52 6 0 0.000 0.000
198 00000001 | 10 14.00 -210.00 1399.25 0 0 0.000 0.000 | 10 43.00 883.18 1423.52 6 0 0.000 0.000
204 00000001 | 10 20.00 -210.00 1399.25 0 0 0.000 0.000 | 10 49.00 883.18 1423.52 6 0 0.000 0.000
210 00000001 | 10 26.00 -210.00 1399.25 0 0 0.000 0.000 | 10 5.00 883.18 1423.52 6 0 0.000 0.000
216 00000001 | 10 32.00 -210.00 1399.25 0 0 0.000 0.000 | 10 11.00 883.18 1423.52 6 0 0.000 0.000
220 00000001 | 10 36.00 -210.00 1399.25 0 0 0.000 0.000 | 10 15.00 883.18 1423.52 6 0 0.000 0.000
";

#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn a_blaster_shot_hits_the_dummy_as_on_the_n64() {
    let replay = Replay {
        p1: FighterKind::Fox,
        route: "60:1:b",
        weapons: &fighter_weapons,
    };
    let Some(out) = replay.run(FOX_BLASTER_HITS_THE_DUMMY) else {
        return;
    };
    out.check("Blaster shot into the dummy");
}

/// `hcharge`: Samus charges from 60 and fires at 100: the charging shot rides joint 16, the released one hits the dummy (t 112, 6 percent).
const SAMUS_CHARGE_SHOT_HITS_THE_DUMMY: &str = "
60 00000001 | 222 1.00 -210.00 1399.25 0 0 0.000 0.000 | 10 11.00 765.00 1413.09 0 0 0.000 0.000
61 00000001 | 222 2.00 -210.00 1399.25 0 0 0.000 0.000 | 10 12.00 765.00 1413.09 0 0 0.000 0.000
62 00000001 | 222 3.00 -210.00 1399.25 0 0 0.000 0.000 | 10 13.00 765.00 1413.09 0 0 0.000 0.000
63 00000001 | 222 4.00 -210.00 1399.25 0 0 0.000 0.000 | 10 14.00 765.00 1413.09 0 0 0.000 0.000
64 00000001 | 222 5.00 -210.00 1399.25 0 0 0.000 0.000 | 10 15.00 765.00 1413.09 0 0 0.000 0.000
65 00000001 | 222 6.00 -210.00 1399.25 0 0 0.000 0.000 | 10 16.00 765.00 1413.09 0 0 0.000 0.000
66 00000001 | 222 7.00 -210.00 1399.25 0 0 0.000 0.000 | 10 17.00 765.00 1413.09 0 0 0.000 0.000
67 00000001 | 222 8.00 -210.00 1399.25 0 0 0.000 0.000 | 10 18.00 765.00 1413.09 0 0 0.000 0.000
68 00000001 | 222 9.00 -210.00 1399.25 0 0 0.000 0.000 | 10 19.00 765.00 1413.09 0 0 0.000 0.000
69 00000001 | 222 10.00 -210.00 1399.25 0 0 0.000 0.000 | 10 20.00 765.00 1413.09 0 0 0.000 0.000
70 00000001 | 222 11.00 -210.00 1399.25 0 0 0.000 0.000 | 10 21.00 765.00 1413.09 0 0 0.000 0.000
71 00000001 | 222 12.00 -210.00 1399.25 0 0 0.000 0.000 | 10 22.00 765.00 1413.09 0 0 0.000 0.000
72 00000001 | 222 13.00 -210.00 1399.25 0 0 0.000 0.000 | 10 23.00 765.00 1413.09 0 0 0.000 0.000
73 00000001 | 222 14.00 -210.00 1399.25 0 0 0.000 0.000 | 10 24.00 765.00 1413.09 0 0 0.000 0.000
74 00000001 | 222 15.00 -210.00 1399.25 0 0 0.000 0.000 | 10 25.00 765.00 1413.09 0 0 0.000 0.000
75 00000001 | 223 0.00 -210.00 1399.25 0 0 0.000 0.000 | 10 26.00 765.00 1413.09 0 0 0.000 0.000 |  | 2 -107.44 1709.04 0.000 0.000
76 00000001 | 223 1.00 -210.00 1399.25 0 0 0.000 0.000 | 10 27.00 765.00 1413.09 0 0 0.000 0.000 |  | 2 -108.75 1704.96 0.000 0.000
77 00000001 | 223 2.00 -210.00 1399.25 0 0 0.000 0.000 | 10 28.00 765.00 1413.09 0 0 0.000 0.000 |  | 2 -109.91 1702.71 0.000 0.000
78 00000001 | 223 3.00 -210.00 1399.25 0 0 0.000 0.000 | 10 29.00 765.00 1413.09 0 0 0.000 0.000 |  | 2 -110.82 1702.01 0.000 0.000
79 00000001 | 223 4.00 -210.00 1399.25 0 0 0.000 0.000 | 10 30.00 765.00 1413.09 0 0 0.000 0.000 |  | 2 -111.18 1701.76 0.000 0.000
80 00000001 | 223 5.00 -210.00 1399.25 0 0 0.000 0.000 | 10 31.00 765.00 1413.09 0 0 0.000 0.000 |  | 2 -110.95 1701.64 0.000 0.000
81 00000001 | 223 6.00 -210.00 1399.25 0 0 0.000 0.000 | 10 32.00 765.00 1413.09 0 0 0.000 0.000 |  | 2 -109.99 1700.19 0.000 0.000
82 00000001 | 223 7.00 -210.00 1399.25 0 0 0.000 0.000 | 10 33.00 765.00 1413.09 0 0 0.000 0.000 |  | 2 -108.89 1704.38 0.000 0.000
83 00000001 | 223 0.00 -210.00 1399.25 0 0 0.000 0.000 | 10 34.00 765.00 1413.09 0 0 0.000 0.000 |  | 2 -107.44 1709.04 0.000 0.000
84 00000001 | 223 1.00 -210.00 1399.25 0 0 0.000 0.000 | 10 35.00 765.00 1413.09 0 0 0.000 0.000 |  | 2 -108.75 1704.96 0.000 0.000
85 00000001 | 223 2.00 -210.00 1399.25 0 0 0.000 0.000 | 10 36.00 765.00 1413.09 0 0 0.000 0.000 |  | 2 -109.91 1702.71 0.000 0.000
86 00000001 | 223 3.00 -210.00 1399.25 0 0 0.000 0.000 | 10 37.00 765.00 1413.09 0 0 0.000 0.000 |  | 2 -110.82 1702.01 0.000 0.000
87 00000001 | 223 4.00 -210.00 1399.25 0 0 0.000 0.000 | 10 38.00 765.00 1413.09 0 0 0.000 0.000 |  | 2 -111.18 1701.76 0.000 0.000
88 00000001 | 223 5.00 -210.00 1399.25 0 0 0.000 0.000 | 10 39.00 765.00 1413.09 0 0 0.000 0.000 |  | 2 -110.95 1701.64 0.000 0.000
89 00000001 | 223 6.00 -210.00 1399.25 0 0 0.000 0.000 | 10 40.00 765.00 1413.09 0 0 0.000 0.000 |  | 2 -109.99 1700.19 0.000 0.000
90 00000001 | 223 7.00 -210.00 1399.25 0 0 0.000 0.000 | 10 41.00 765.00 1413.09 0 0 0.000 0.000 |  | 2 -108.89 1704.38 0.000 0.000
91 00000001 | 223 0.00 -210.00 1399.25 0 0 0.000 0.000 | 10 42.00 765.00 1413.09 0 0 0.000 0.000 |  | 2 -107.44 1709.04 0.000 0.000
92 00000001 | 223 1.00 -210.00 1399.25 0 0 0.000 0.000 | 10 43.00 765.00 1413.09 0 0 0.000 0.000 |  | 2 -108.75 1704.96 0.000 0.000
93 00000001 | 223 2.00 -210.00 1399.25 0 0 0.000 0.000 | 10 44.00 765.00 1413.09 0 0 0.000 0.000 |  | 2 -109.91 1702.71 0.000 0.000
94 00000001 | 223 3.00 -210.00 1399.25 0 0 0.000 0.000 | 10 45.00 765.00 1413.09 0 0 0.000 0.000 |  | 2 -110.82 1702.01 0.000 0.000
95 00000001 | 223 4.00 -210.00 1399.25 0 0 0.000 0.000 | 10 46.00 765.00 1413.09 0 0 0.000 0.000 |  | 2 -111.18 1701.76 0.000 0.000
96 00000001 | 223 5.00 -210.00 1399.25 0 0 0.000 0.000 | 10 47.00 765.00 1413.09 0 0 0.000 0.000 |  | 2 -110.95 1701.64 0.000 0.000
97 00000001 | 223 6.00 -210.00 1399.25 0 0 0.000 0.000 | 10 48.00 765.00 1413.09 0 0 0.000 0.000 |  | 2 -109.99 1700.19 0.000 0.000
98 00000001 | 223 7.00 -210.00 1399.25 0 0 0.000 0.000 | 10 49.00 765.00 1413.09 0 0 0.000 0.000 |  | 2 -108.89 1704.38 0.000 0.000
99 00000001 | 223 0.00 -210.00 1399.25 0 0 0.000 0.000 | 10 0.00 765.00 1413.09 0 0 0.000 0.000 |  | 2 -107.44 1709.04 0.000 0.000
100 00000001 | 224 0.00 -210.00 1399.25 0 0 0.000 0.000 | 10 1.00 765.00 1413.09 0 0 0.000 0.000 |  | 2 -107.44 1709.04 0.000 0.000
101 00000001 | 224 1.00 -216.00 1399.26 0 0 0.000 0.000 | 10 2.00 765.00 1413.09 0 0 0.000 0.000 |  | 2 -42.80 1699.50 62.000 0.000
102 00000001 | 224 2.00 -216.00 1399.26 0 0 0.000 0.000 | 10 3.00 765.00 1413.09 0 0 0.000 0.000 |  | 2 19.20 1699.50 62.000 0.000
103 00000001 | 224 3.00 -216.00 1399.26 0 0 0.000 0.000 | 10 4.00 765.00 1413.09 0 0 0.000 0.000 |  | 2 81.20 1699.50 62.000 0.000
104 00000001 | 224 4.00 -216.00 1399.26 0 0 0.000 0.000 | 10 5.00 765.00 1413.09 0 0 0.000 0.000 |  | 2 143.20 1699.50 62.000 0.000
105 00000001 | 224 5.00 -216.00 1399.26 0 0 0.000 0.000 | 10 6.00 765.00 1413.09 0 0 0.000 0.000 |  | 2 205.20 1699.50 62.000 0.000
106 00000001 | 224 6.00 -216.00 1399.26 0 0 0.000 0.000 | 10 7.00 765.00 1413.09 0 0 0.000 0.000 |  | 2 267.20 1699.50 62.000 0.000
107 00000001 | 224 7.00 -216.00 1399.26 0 0 0.000 0.000 | 10 8.00 765.00 1413.09 0 0 0.000 0.000 |  | 2 329.20 1699.50 62.000 0.000
108 00000001 | 224 8.00 -216.00 1399.26 0 0 0.000 0.000 | 10 9.00 765.00 1413.09 0 0 0.000 0.000 |  | 2 391.20 1699.50 62.000 0.000
109 00000001 | 224 9.00 -216.00 1399.26 0 0 0.000 0.000 | 10 10.00 765.00 1413.09 0 0 0.000 0.000 |  | 2 453.20 1699.50 62.000 0.000
110 00000001 | 224 10.00 -216.00 1399.26 0 0 0.000 0.000 | 10 11.00 765.00 1413.09 0 0 0.000 0.000 |  | 2 515.20 1699.50 62.000 0.000
111 00000001 | 224 11.00 -216.00 1399.26 0 0 0.000 0.000 | 10 12.00 765.00 1413.09 0 0 0.000 0.000 |  | 2 577.20 1699.50 62.000 0.000
112 2cd60a1e | 224 12.00 -216.00 1399.26 0 0 0.000 0.000 | 49 1.00 765.00 1413.09 6 10 21.277 1.877
113 daf1b4cc | 224 13.00 -216.00 1399.26 0 0 0.000 0.000 | 49 1.00 765.00 1413.09 6 9 21.277 1.877
114 daf1b4cc | 224 14.00 -216.00 1399.26 0 0 0.000 0.000 | 49 1.00 765.00 1413.09 6 8 21.277 1.877
115 daf1b4cc | 224 15.00 -216.00 1399.26 0 0 0.000 0.000 | 49 1.00 765.00 1413.09 6 7 21.277 1.877
116 daf1b4cc | 224 16.00 -216.00 1399.26 0 0 0.000 0.000 | 49 1.00 765.00 1413.09 6 6 21.277 1.877
117 aea69ed3 | 224 17.00 -216.00 1399.26 0 0 0.000 0.000 | 49 1.00 765.00 1413.09 6 5 21.277 1.877
118 aea69ed3 | 224 18.00 -216.00 1399.26 0 0 0.000 0.000 | 49 1.00 765.00 1413.09 6 4 21.277 1.877
119 aea69ed3 | 224 19.00 -216.00 1399.26 0 0 0.000 0.000 | 49 1.00 765.00 1413.09 6 3 21.277 1.877
120 aea69ed3 | 224 20.00 -216.00 1399.26 0 0 0.000 0.000 | 49 1.00 765.00 1413.09 6 2 21.277 1.877
122 0f3e862e
125 491c86cd
126 491c86cd | 224 26.00 -216.00 1399.26 0 0 0.000 0.000 | 37 5.00 848.97 1420.50 6 0 13.806 1.218
132 491c86cd | 10 2.00 -216.00 1399.26 0 0 0.000 0.000 | 37 11.00 900.43 1425.04 6 0 4.841 0.427
138 491c86cd | 10 8.00 -216.00 1399.26 0 0 0.000 0.000 | 10 5.00 905.99 1425.53 6 0 0.000 0.000
144 491c86cd | 10 14.00 -216.00 1399.26 0 0 0.000 0.000 | 10 11.00 905.99 1425.53 6 0 0.000 0.000
150 491c86cd | 10 20.00 -216.00 1399.26 0 0 0.000 0.000 | 10 17.00 905.99 1425.53 6 0 0.000 0.000
156 491c86cd | 10 26.00 -216.00 1399.26 0 0 0.000 0.000 | 10 23.00 905.99 1425.53 6 0 0.000 0.000
162 491c86cd | 10 32.00 -216.00 1399.26 0 0 0.000 0.000 | 10 29.00 905.99 1425.53 6 0 0.000 0.000
168 491c86cd | 10 38.00 -216.00 1399.26 0 0 0.000 0.000 | 10 35.00 905.99 1425.53 6 0 0.000 0.000
174 491c86cd | 10 44.00 -216.00 1399.26 0 0 0.000 0.000 | 10 41.00 905.99 1425.53 6 0 0.000 0.000
180 491c86cd | 10 50.00 -216.00 1399.26 0 0 0.000 0.000 | 10 47.00 905.99 1425.53 6 0 0.000 0.000
186 491c86cd | 10 56.00 -216.00 1399.26 0 0 0.000 0.000 | 10 3.00 905.99 1425.53 6 0 0.000 0.000
192 491c86cd | 10 62.00 -216.00 1399.26 0 0 0.000 0.000 | 10 9.00 905.99 1425.53 6 0 0.000 0.000
198 491c86cd | 10 68.00 -216.00 1399.26 0 0 0.000 0.000 | 10 15.00 905.99 1425.53 6 0 0.000 0.000
204 491c86cd | 10 74.00 -216.00 1399.26 0 0 0.000 0.000 | 10 21.00 905.99 1425.53 6 0 0.000 0.000
210 491c86cd | 10 80.00 -216.00 1399.26 0 0 0.000 0.000 | 10 27.00 905.99 1425.53 6 0 0.000 0.000
216 491c86cd | 10 4.00 -216.00 1399.26 0 0 0.000 0.000 | 10 33.00 905.99 1425.53 6 0 0.000 0.000
220 491c86cd | 10 8.00 -216.00 1399.26 0 0 0.000 0.000 | 10 37.00 905.99 1425.53 6 0 0.000 0.000
";

#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn a_charge_shot_hits_the_dummy_as_on_the_n64() {
    let replay = Replay {
        p1: FighterKind::Samus,
        route: "60:1:b;100:1:b",
        weapons: &fighter_weapons,
    };
    let Some(out) = replay.run(SAMUS_CHARGE_SHOT_HITS_THE_DUMMY) else {
        return;
    };
    out.check("Charge Shot, charging then released into the dummy");
}

/// `hboom`: Link's Boomerang hits the dummy (t 93, 9 percent) and flies on.
const LINK_BOOMERANG_HITS_THE_DUMMY: &str = "
60 00000001 | 229 1.00 -210.00 1399.25 0 0 0.000 0.000 | 10 11.00 765.00 1413.09 0 0 0.000 0.000
61 00000001 | 229 2.00 -210.00 1399.25 0 0 0.000 0.000 | 10 12.00 765.00 1413.09 0 0 0.000 0.000
62 00000001 | 229 3.00 -210.00 1399.25 0 0 0.000 0.000 | 10 13.00 765.00 1413.09 0 0 0.000 0.000
63 00000001 | 229 4.00 -210.00 1399.25 0 0 0.000 0.000 | 10 14.00 765.00 1413.09 0 0 0.000 0.000
64 00000001 | 229 5.00 -210.00 1399.25 0 0 0.000 0.000 | 10 15.00 765.00 1413.09 0 0 0.000 0.000
65 00000001 | 229 6.00 -210.00 1399.25 0 0 0.000 0.000 | 10 16.00 765.00 1413.09 0 0 0.000 0.000
66 00000001 | 229 7.00 -210.00 1399.25 0 0 0.000 0.000 | 10 17.00 765.00 1413.09 0 0 0.000 0.000
67 00000001 | 229 8.00 -210.00 1399.25 0 0 0.000 0.000 | 10 18.00 765.00 1413.09 0 0 0.000 0.000
68 00000001 | 229 9.00 -210.00 1399.25 0 0 0.000 0.000 | 10 19.00 765.00 1413.09 0 0 0.000 0.000
69 00000001 | 229 10.00 -210.00 1399.25 0 0 0.000 0.000 | 10 20.00 765.00 1413.09 0 0 0.000 0.000
70 00000001 | 229 11.00 -210.00 1399.25 0 0 0.000 0.000 | 10 21.00 765.00 1413.09 0 0 0.000 0.000
71 00000001 | 229 12.00 -210.00 1399.25 0 0 0.000 0.000 | 10 22.00 765.00 1413.09 0 0 0.000 0.000
72 00000001 | 229 13.00 -210.00 1399.25 0 0 0.000 0.000 | 10 23.00 765.00 1413.09 0 0 0.000 0.000
73 00000001 | 229 14.00 -210.00 1399.25 0 0 0.000 0.000 | 10 24.00 765.00 1413.09 0 0 0.000 0.000
74 00000001 | 229 15.00 -210.00 1399.25 0 0 0.000 0.000 | 10 25.00 765.00 1413.09 0 0 0.000 0.000
75 00000001 | 229 16.00 -210.00 1399.25 0 0 0.000 0.000 | 10 26.00 765.00 1413.09 0 0 0.000 0.000
76 00000001 | 229 17.00 -210.00 1399.25 0 0 0.000 0.000 | 10 27.00 765.00 1413.09 0 0 0.000 0.000
77 00000001 | 229 18.00 -210.00 1399.25 0 0 0.000 0.000 | 10 28.00 765.00 1413.09 0 0 0.000 0.000
78 00000001 | 229 19.00 -210.00 1399.25 0 0 0.000 0.000 | 10 29.00 765.00 1413.09 0 0 0.000 0.000
79 00000001 | 229 20.00 -210.00 1399.25 0 0 0.000 0.000 | 10 30.00 765.00 1413.09 0 0 0.000 0.000
80 00000001 | 229 21.00 -210.00 1399.25 0 0 0.000 0.000 | 10 31.00 765.00 1413.09 0 0 0.000 0.000
81 00000001 | 229 22.00 -210.00 1399.25 0 0 0.000 0.000 | 10 32.00 765.00 1413.09 0 0 0.000 0.000
82 00000001 | 229 23.00 -210.00 1399.25 0 0 0.000 0.000 | 10 33.00 765.00 1413.09 0 0 0.000 0.000
83 00000001 | 229 24.00 -210.00 1399.25 0 0 0.000 0.000 | 10 34.00 765.00 1413.09 0 0 0.000 0.000
84 00000001 | 229 25.00 -210.00 1399.25 0 0 0.000 0.000 | 10 35.00 765.00 1413.09 0 0 0.000 0.000
85 00000001 | 229 26.00 -210.00 1399.25 0 0 0.000 0.000 | 10 36.00 765.00 1413.09 0 0 0.000 0.000 |  | 7 23.60 1689.25 83.600 0.000
86 00000001 | 229 27.00 -210.00 1399.25 0 0 0.000 0.000 | 10 37.00 765.00 1413.09 0 0 0.000 0.000 |  | 7 105.80 1689.25 82.200 0.000
87 00000001 | 229 28.00 -210.00 1399.25 0 0 0.000 0.000 | 10 38.00 765.00 1413.09 0 0 0.000 0.000 |  | 7 186.60 1689.25 80.800 0.000
88 00000001 | 229 29.00 -210.00 1399.25 0 0 0.000 0.000 | 10 39.00 765.00 1413.09 0 0 0.000 0.000 |  | 7 266.00 1689.25 79.400 0.000
89 00000001 | 229 30.00 -210.00 1399.25 0 0 0.000 0.000 | 10 40.00 765.00 1413.09 0 0 0.000 0.000 |  | 7 344.00 1689.25 78.000 0.000
90 00000001 | 229 31.00 -210.00 1399.25 0 0 0.000 0.000 | 10 41.00 765.00 1413.09 0 0 0.000 0.000 |  | 7 420.60 1689.25 76.600 0.000
91 00000001 | 229 32.00 -210.00 1399.25 0 0 0.000 0.000 | 10 42.00 765.00 1413.09 0 0 0.000 0.000 |  | 7 495.80 1689.25 75.200 0.000
92 00000001 | 229 33.00 -210.00 1399.25 0 0 0.000 0.000 | 10 43.00 765.00 1413.09 0 0 0.000 0.000 |  | 7 569.60 1689.25 73.800 0.000
93 c823f683 | 229 34.00 -210.00 1399.25 0 0 0.000 0.000 | 52 1.00 765.00 1413.09 9 8 21.369 58.711 |  | 7 642.00 1689.25 67.400 0.000
94 2cd60a1e | 229 35.00 -210.00 1399.25 0 0 0.000 0.000 | 52 1.00 765.00 1413.09 9 7 21.369 58.711 |  | 7 573.60 1689.25 -68.400 0.000
95 2cd60a1e | 229 36.00 -210.00 1399.25 0 0 0.000 0.000 | 52 1.00 765.00 1413.09 9 6 21.369 58.711 |  | 7 504.20 1689.25 -69.400 0.000
96 2cd60a1e | 229 37.00 -210.00 1399.25 0 0 0.000 0.000 | 52 1.00 765.00 1413.09 9 5 21.369 58.711 |  | 7 433.80 1689.25 -70.400 0.000
97 2cd60a1e | 229 38.00 -210.00 1399.25 0 0 0.000 0.000 | 52 1.00 765.00 1413.09 9 4 21.369 58.711 |  | 7 362.40 1689.25 -71.400 0.000
98 2cd60a1e | 229 39.00 -210.00 1399.25 0 0 0.000 0.000 | 52 1.00 765.00 1413.09 9 3 21.369 58.711 |  | 7 290.00 1689.25 -72.400 0.000
99 2cd60a1e | 229 40.00 -210.00 1399.25 0 0 0.000 0.000 | 52 1.00 765.00 1413.09 9 2 21.369 58.711 |  | 7 216.60 1689.25 -73.400 0.000
100 2cd60a1e | 229 41.00 -210.00 1399.25 0 0 0.000 0.000 | 52 1.00 765.00 1413.09 9 1 21.369 58.711 |  | 7 142.20 1689.25 -74.400 0.000
101 2cd60a1e | 229 42.00 -210.00 1399.25 0 0 0.000 0.000 | 52 2.00 785.79 1467.80 9 0 20.788 57.114 |  | 7 66.80 1689.25 -75.400 0.000
102 2cd60a1e | 229 43.00 -210.00 1399.25 0 0 0.000 0.000 | 52 3.00 805.99 1518.52 9 0 20.206 55.516 |  | 7 -9.60 1689.25 -76.400 0.000
103 2cd60a1e | 229 44.00 -210.00 1399.25 0 0 0.000 0.000 | 52 4.00 825.62 1565.24 9 0 19.625 53.919 |  | 7 -87.00 1689.25 -77.400 0.000
104 2cd60a1e | 229 45.00 -210.00 1399.25 0 0 0.000 0.000 | 52 5.00 844.66 1607.96 9 0 19.043 52.321
105 2cd60a1e | 10 0.00 -210.00 1399.25 0 0 0.000 0.000 | 52 6.00 863.12 1646.68 9 0 18.462 50.724
106 2cd60a1e | 10 1.00 -210.00 1399.25 0 0 0.000 0.000 | 52 7.00 881.00 1681.41 9 0 17.880 49.126
107 2cd60a1e | 10 2.00 -210.00 1399.25 0 0 0.000 0.000 | 52 8.00 898.30 1712.14 9 0 17.299 47.529
108 2cd60a1e | 10 3.00 -210.00 1399.25 0 0 0.000 0.000 | 52 9.00 915.02 1738.87 9 0 16.718 45.931
109 2cd60a1e | 10 4.00 -210.00 1399.25 0 0 0.000 0.000 | 52 10.00 931.16 1761.60 9 0 16.136 44.334
114 2cd60a1e | 10 9.00 -210.00 1399.25 0 0 0.000 0.000 | 52 15.00 1003.12 1815.31 9 0 13.229 36.346
120 2cd60a1e | 10 15.00 -210.00 1399.25 0 0 0.000 0.000 | 52 21.00 1070.28 1753.44 9 0 9.740 26.762
126 2cd60a1e | 10 21.00 -210.00 1399.25 0 0 0.000 0.000 | 52 27.00 1116.51 1616.46 9 0 6.252 17.177
132 2cd60a1e | 10 27.00 -210.00 1399.25 0 0 0.000 0.000 | 68 0.00 1141.81 1445.55 9 0 2.744 -0.323
138 2cd60a1e | 10 33.00 -210.00 1399.25 0 0 0.000 0.000 | 68 6.00 1143.07 1445.40 9 0 0.000 0.000
144 2cd60a1e | 10 39.00 -210.00 1399.25 0 0 0.000 0.000 | 68 12.00 1143.07 1445.40 9 0 0.000 0.000
150 2cd60a1e | 10 45.00 -210.00 1399.25 0 0 0.000 0.000 | 68 18.00 1143.07 1445.40 9 0 0.000 0.000
156 2cd60a1e | 10 51.00 -210.00 1399.25 0 0 0.000 0.000 | 68 24.00 1143.07 1445.40 9 0 0.000 0.000
162 2cd60a1e | 10 57.00 -210.00 1399.25 0 0 0.000 0.000 | 70 0.00 1143.07 1445.40 9 0 0.000 0.000
168 2cd60a1e | 10 63.00 -210.00 1399.25 0 0 0.000 0.000 | 70 0.00 1143.07 1445.40 9 0 0.000 0.000
174 2cd60a1e | 10 69.00 -210.00 1399.25 0 0 0.000 0.000 | 70 0.00 1143.07 1445.40 9 0 0.000 0.000
180 2cd60a1e | 10 75.00 -210.00 1399.25 0 0 0.000 0.000 | 70 0.00 1143.07 1445.40 9 0 0.000 0.000
186 2cd60a1e | 10 1.00 -210.00 1399.25 0 0 0.000 0.000 | 70 0.00 1143.07 1445.40 9 0 0.000 0.000
192 2cd60a1e | 10 7.00 -210.00 1399.25 0 0 0.000 0.000 | 70 0.00 1143.07 1445.40 9 0 0.000 0.000
198 2cd60a1e | 10 13.00 -210.00 1399.25 0 0 0.000 0.000 | 70 0.00 1143.07 1445.40 9 0 0.000 0.000
204 2cd60a1e | 10 19.00 -210.00 1399.25 0 0 0.000 0.000 | 70 0.00 1143.07 1445.40 9 0 0.000 0.000
210 2cd60a1e | 10 25.00 -210.00 1399.25 0 0 0.000 0.000 | 70 0.00 1143.07 1445.40 9 0 0.000 0.000
216 2cd60a1e | 10 31.00 -210.00 1399.25 0 0 0.000 0.000 | 70 0.00 1143.07 1445.40 9 0 0.000 0.000
220 2cd60a1e | 10 35.00 -210.00 1399.25 0 0 0.000 0.000 | 70 0.00 1143.07 1445.40 9 0 0.000 0.000
";

#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn a_boomerang_hits_the_dummy_as_on_the_n64() {
    let replay = Replay {
        p1: FighterKind::Link,
        route: "60:1:b",
        weapons: &fighter_weapons,
    };
    let Some(out) = replay.run(LINK_BOOMERANG_HITS_THE_DUMMY) else {
        return;
    };
    out.check("Boomerang into the dummy");
}

/// N64 trace `hw3f` (warp ROM, Training at Peach's Castle): Link runs in,
/// pulls a bomb, throws it straight up and sends the Boomerang into the
/// dummy. On frame 300 the falling bomb and the Boomerang hit the dummy
/// together (8 + 9). On that frame the Boomerang also hits the bomb with
/// its outbound 9, not the return damage its dummy hit switches to, and
/// the hitlag keeps the explosion to frame 312. The dummy lands face down
/// on frame 343 (`DownBounceD`).
const LINK_BOMB_AND_BOOMERANG_HIT_THE_DUMMY: &str = "
60 18be873a
66 3d6c1037
188 2cd60a1e
290 2cd60a1e | 229 16.00 565.80 1398.04 0 0 0.000 0.000 | 10 41.00 792.00 1415.47 0 0 0.000 0.000 | 21 578.93 2267.48 0.000 -57.600
291 2cd60a1e | 229 17.00 565.80 1398.04 0 0 0.000 0.000 | 10 42.00 792.00 1415.47 0 0 0.000 0.000 | 21 578.93 2208.68 0.000 -58.800
292 2cd60a1e | 229 18.00 565.80 1398.04 0 0 0.000 0.000 | 10 43.00 792.00 1415.47 0 0 0.000 0.000 | 21 578.93 2148.68 0.000 -60.000
293 2cd60a1e | 229 19.00 565.80 1398.04 0 0 0.000 0.000 | 10 44.00 792.00 1415.47 0 0 0.000 0.000 | 21 578.93 2087.48 0.000 -61.200
294 2cd60a1e | 229 20.00 565.80 1398.04 0 0 0.000 0.000 | 10 45.00 792.00 1415.47 0 0 0.000 0.000 | 21 578.93 2025.08 0.000 -62.400
295 2cd60a1e | 229 21.00 565.80 1398.04 0 0 0.000 0.000 | 10 46.00 792.00 1415.47 0 0 0.000 0.000 | 21 578.93 1961.48 0.000 -63.600
296 2cd60a1e | 229 22.00 565.80 1398.04 0 0 0.000 0.000 | 10 47.00 792.00 1415.47 0 0 0.000 0.000 | 21 578.93 1896.68 0.000 -64.800
297 2cd60a1e | 229 23.00 565.80 1398.04 0 0 0.000 0.000 | 10 48.00 792.00 1415.47 0 0 0.000 0.000 | 21 578.93 1830.68 0.000 -66.000
298 2cd60a1e | 229 24.00 565.80 1398.04 0 0 0.000 0.000 | 10 49.00 792.00 1415.47 0 0 0.000 0.000 | 21 578.93 1763.48 0.000 -67.200
299 2cd60a1e | 229 25.00 565.80 1398.04 0 0 0.000 0.000 | 10 0.00 792.00 1415.47 0 0 0.000 0.000 | 21 578.93 1695.08 0.000 -68.400
300 daf1b4cc | 229 26.00 565.80 1398.04 0 0 0.000 0.000 | 54 1.00 792.00 1415.47 17 8 11.457 64.978 | 21 578.93 1625.48 0.000 0.000 | 7 799.40 1688.04 78.600 0.000
301 92387aff | 52 1.00 565.80 1398.04 5 6 -47.238 43.285 | 54 1.00 792.00 1415.47 22 6 11.457 64.978 | 21 578.93 1625.48 0.000 0.000 | 7 719.80 1688.04 -79.600 0.000
302 92387aff | 52 1.00 565.80 1398.04 5 5 -47.238 43.285 | 54 1.00 792.00 1415.47 22 5 11.457 64.978 | 21 578.93 1625.48 0.000 0.000
303 7ff56027 | 52 1.00 565.80 1398.04 5 4 -47.238 43.285 | 54 1.00 792.00 1415.47 22 4 11.457 64.978 | 21 578.93 1625.48 0.000 0.000
304 7ff56027 | 52 1.00 565.80 1398.04 5 3 -47.238 43.285 | 54 1.00 792.00 1415.47 22 3 11.457 64.978 | 21 578.93 1625.48 0.000 0.000
305 8bdb664f | 52 1.00 565.80 1398.04 5 2 -47.238 43.285 | 54 1.00 792.00 1415.47 22 2 11.457 64.978 | 21 578.93 1625.48 0.000 0.000
306 8bdb664f | 52 1.00 565.80 1398.04 5 1 -47.238 43.285 | 54 1.00 792.00 1415.47 22 1 11.457 64.978 | 21 578.93 1625.48 0.000 0.000
307 3e1203c8 | 52 2.00 519.81 1436.78 5 0 -45.984 42.137 | 54 2.00 803.16 1476.37 22 0 11.162 63.303 | 21 578.93 1625.48 0.000 0.000
308 3e1203c8 | 52 3.00 475.08 1470.97 5 0 -44.731 40.988 | 54 3.00 814.03 1533.20 22 0 10.867 61.629 | 21 578.93 1625.48 0.000 0.000
309 4df26f25 | 52 4.00 431.61 1500.61 5 0 -43.478 39.840 | 54 4.00 824.60 1585.96 22 0 10.572 59.955 | 21 578.93 1625.48 0.000 0.000
310 4df26f25 | 52 5.00 389.38 1525.70 5 0 -42.224 38.691 | 54 5.00 834.88 1634.64 22 0 10.276 58.281 | 21 578.93 1625.48 0.000 0.000
311 7eb7620d | 52 6.00 348.41 1546.24 5 0 -40.971 37.543 | 54 6.00 844.86 1679.25 22 0 9.981 56.607 | 21 578.93 1625.48 0.000 0.000
312 7eb7620d | 52 7.00 308.69 1562.24 5 0 -39.717 36.394 | 54 7.00 854.54 1719.78 22 0 9.686 54.933 | 21 578.93 1625.48 0.000 0.000
313 121fbe7e | 52 8.00 270.23 1573.68 5 0 -38.464 35.246 | 54 8.00 863.94 1756.24 22 0 9.391 53.258
314 121fbe7e | 52 9.00 233.02 1580.58 5 0 -37.211 34.097 | 54 9.00 873.03 1788.62 22 0 9.096 51.584
315 96c590c5 | 52 10.00 197.06 1582.93 5 0 -35.957 32.949 | 54 10.00 881.83 1816.93 22 0 8.800 49.910
316 96c590c5 | 52 11.00 162.36 1580.73 5 0 -34.704 31.800 | 54 11.00 890.34 1841.17 22 0 8.505 48.236
317 48cc27ad | 52 12.00 128.91 1573.98 5 0 -33.451 30.652 | 54 12.00 898.55 1861.33 22 0 8.210 46.562
318 48cc27ad | 52 13.00 96.71 1562.68 5 0 -32.197 29.503 | 54 13.00 906.46 1877.42 22 0 7.915 44.888
319 047eb795 | 52 14.00 65.77 1546.84 5 0 -30.944 28.355 | 54 14.00 914.08 1889.43 22 0 7.620 43.213
320 047eb795 | 52 15.00 36.08 1526.45 5 0 -29.690 27.206 | 54 15.00 921.41 1897.37 22 0 7.324 41.539
321 2c49e07d | 52 16.00 7.64 1501.50 5 0 -28.437 26.058 | 54 16.00 928.44 1901.23 22 0 7.029 39.865
322 2c49e07d | 52 17.00 -19.54 1472.01 5 0 -27.184 24.909 | 54 17.00 935.17 1901.03 22 0 6.734 38.191
323 261e4265 | 52 18.00 -45.48 1437.97 5 0 -25.930 23.761 | 54 18.00 941.61 1896.74 22 0 6.439 36.517
324 261e4265 | 52 19.00 -70.15 1399.39 5 0 -24.677 22.612 | 54 19.00 947.75 1888.38 22 0 6.144 34.843
325 f1f07d4d | 68 0.00 -93.58 1399.07 5 0 -23.424 0.036 | 54 20.00 953.60 1877.55 22 0 5.848 33.168
326 0677d621 | 68 1.00 -115.00 1399.10 5 0 -21.424 0.033 | 54 21.00 959.15 1865.05 22 0 5.553 31.494
327 50393135 | 68 2.00 -134.42 1399.13 5 0 -19.424 0.030 | 54 22.00 964.41 1850.87 22 0 5.258 29.820
328 0fc94289 | 68 3.00 -151.85 1399.16 5 0 -17.424 0.027 | 54 23.00 969.37 1835.01 22 0 4.963 28.146
329 7874fe1d | 68 4.00 -167.27 1399.19 5 0 -15.424 0.024 | 54 24.00 974.04 1817.48 22 0 4.668 26.472
330 ddd597f1 | 68 5.00 -180.69 1399.21 5 0 -13.424 0.021 | 54 25.00 978.41 1798.28 22 0 4.372 24.797
331 5fa48405 | 68 6.00 -192.12 1399.22 5 0 -11.424 0.018 | 54 26.00 982.49 1777.41 22 0 4.077 23.123
332 87cf7659 | 68 7.00 -201.54 1399.24 5 0 -9.424 0.015 | 54 27.00 986.27 1754.85 22 0 3.782 21.449
333 8ecc62ed | 68 8.00 -208.96 1399.25 5 0 -7.424 0.012 | 54 28.00 989.76 1730.63 22 0 3.487 19.775
334 3a2d7dc1 | 68 9.00 -214.39 1399.26 5 0 -5.424 0.008 | 54 29.00 992.95 1704.73 22 0 3.192 18.101
335 89753ad5 | 68 10.00 -217.81 1399.26 5 0 -3.424 0.005 | 54 30.00 995.85 1677.16 22 0 2.896 16.427
336 a52a4e29 | 68 11.00 -219.23 1399.27 5 0 -1.424 0.002 | 54 31.00 998.45 1647.91 22 0 2.601 14.752
337 c42babbd | 68 12.00 -219.23 1399.27 5 0 0.000 0.000 | 54 32.00 1000.76 1616.99 22 0 2.306 13.078
338 fb448791 | 68 13.00 -219.23 1399.27 5 0 0.000 0.000 | 54 33.00 1002.77 1584.39 22 0 2.011 11.404
339 2b0055a5 | 68 14.00 -219.23 1399.27 5 0 0.000 0.000 | 54 34.00 1004.48 1550.12 22 0 1.716 9.730
340 7fbec9f9 | 68 15.00 -219.23 1399.27 5 0 0.000 0.000 | 54 35.00 1005.90 1514.18 22 0 1.420 8.056
341 3807d88d | 68 16.00 -219.23 1399.27 5 0 0.000 0.000 | 54 36.00 1007.03 1476.56 22 0 1.125 6.382
342 a51fb561 | 68 17.00 -219.23 1399.27 5 0 0.000 0.000 | 54 37.00 1007.86 1437.27 22 0 0.830 4.707
343 99dad475 | 68 18.00 -219.23 1399.27 5 0 0.000 0.000 | 67 0.00 1008.39 1434.56 22 0 0.533 0.047
344 6a15e95d | 68 19.00 -219.23 1399.27 5 0 0.000 0.000 | 67 1.00 1008.39 1434.56 22 0 0.000 0.000
345 6a15e95d | 68 20.00 -219.23 1399.27 5 0 0.000 0.000 | 67 2.00 1008.39 1434.56 22 0 0.000 0.000
346 f3d9b745 | 68 21.00 -219.23 1399.27 5 0 0.000 0.000 | 67 3.00 1008.39 1434.56 22 0 0.000 0.000
347 f3d9b745 | 68 22.00 -219.23 1399.27 5 0 0.000 0.000 | 67 4.00 1008.39 1434.56 22 0 0.000 0.000
348 5968ad99 | 68 23.00 -219.23 1399.27 5 0 0.000 0.000 | 67 5.00 1008.39 1434.56 22 0 0.000 0.000
349 5968ad99 | 68 24.00 -219.23 1399.27 5 0 0.000 0.000 | 67 6.00 1008.39 1434.56 22 0 0.000 0.000
350 5968ad99 | 68 25.00 -219.23 1399.27 5 0 0.000 0.000 | 67 7.00 1008.39 1434.56 22 0 0.000 0.000
352 0a4ade2d
354 0a4ade2d | 70 0.00 -219.23 1399.27 5 0 0.000 0.000 | 67 11.00 1008.39 1434.56 22 0 0.000 0.000
356 d8767d01
358 d8767d01 | 70 0.00 -219.23 1399.27 5 0 0.000 0.000 | 67 15.00 1008.39 1434.56 22 0 0.000 0.000
360 ef11fe15
362 ef11fe15 | 70 0.00 -219.23 1399.27 5 0 0.000 0.000 | 67 19.00 1008.39 1434.56 22 0 0.000 0.000
364 85881569
366 85881569 | 70 0.00 -219.23 1399.27 5 0 0.000 0.000 | 67 23.00 1008.39 1434.56 22 0 0.000 0.000
368 c8dbb6fd
370 c8dbb6fd | 70 0.00 -219.23 1399.27 5 0 0.000 0.000 | 69 0.00 1008.39 1434.56 22 0 0.000 0.000
372 ef3c16d1
374 ef3c16d1 | 70 0.00 -219.23 1399.27 5 0 0.000 0.000 | 69 0.00 1008.39 1434.56 22 0 0.000 0.000
376 a9d8a8e5
378 a9d8a8e5 | 70 0.00 -219.23 1399.27 5 0 0.000 0.000 | 69 0.00 1008.39 1434.56 22 0 0.000 0.000
380 68f52139
382 68f52139 | 70 0.00 -219.23 1399.27 5 0 0.000 0.000 | 69 0.00 1008.39 1434.56 22 0 0.000 0.000
384 263d73cd
386 263d73cd | 70 0.00 -219.23 1399.27 5 0 0.000 0.000 | 69 0.00 1008.39 1434.56 22 0 0.000 0.000
388 a959d4a1
390 a959d4a1 | 70 0.00 -219.23 1399.27 5 0 0.000 0.000 | 69 0.00 1008.39 1434.56 22 0 0.000 0.000
392 7ac2b7b5
394 7ac2b7b5 | 70 0.00 -219.23 1399.27 5 0 0.000 0.000 | 69 0.00 1008.39 1434.56 22 0 0.000 0.000
396 f8d4d109
398 f8d4d109 | 70 0.00 -219.23 1399.27 5 0 0.000 0.000 | 69 0.00 1008.39 1434.56 22 0 0.000 0.000
400 4325149d
402 4325149d | 70 0.00 -219.23 1399.27 5 0 0.000 0.000 | 69 0.00 1008.39 1434.56 22 0 0.000 0.000
404 f014b671
406 f014b671 | 70 0.00 -219.23 1399.27 5 0 0.000 0.000 | 69 0.00 1008.39 1434.56 22 0 0.000 0.000
408 c0a52a85
410 c0a52a85 | 70 0.00 -219.23 1399.27 5 0 0.000 0.000 | 69 0.00 1008.39 1434.56 22 0 0.000 0.000
412 c68c24d9
414 c68c24d9 | 70 0.00 -219.23 1399.27 5 0 0.000 0.000 | 69 0.00 1008.39 1434.56 22 0 0.000 0.000
416 b087996d
418 b087996d | 70 0.00 -219.23 1399.27 5 0 0.000 0.000 | 69 0.00 1008.39 1434.56 22 0 0.000 0.000
420 30f1bc41
422 30f1bc41 | 70 0.00 -219.23 1399.27 5 0 0.000 0.000 | 69 0.00 1008.39 1434.56 22 0 0.000 0.000
424 b2950155
426 b2950155 | 70 0.00 -219.23 1399.27 5 0 0.000 0.000 | 69 0.00 1008.39 1434.56 22 0 0.000 0.000
430 b2950155 | 70 0.00 -219.23 1399.27 5 0 0.000 0.000 | 69 0.00 1008.39 1434.56 22 0 0.000 0.000
434 b2950155 | 70 0.00 -219.23 1399.27 5 0 0.000 0.000 | 69 0.00 1008.39 1434.56 22 0 0.000 0.000
438 b2950155 | 70 0.00 -219.23 1399.27 5 0 0.000 0.000 | 69 0.00 1008.39 1434.56 22 0 0.000 0.000
442 b2950155 | 70 0.00 -219.23 1399.27 5 0 0.000 0.000 | 69 0.00 1008.39 1434.56 22 0 0.000 0.000
446 b2950155 | 70 0.00 -219.23 1399.27 5 0 0.000 0.000 | 69 0.00 1008.39 1434.56 22 0 0.000 0.000
450 b2950155 | 70 0.00 -219.23 1399.27 5 0 0.000 0.000 | 69 0.00 1008.39 1434.56 22 0 0.000 0.000
454 b2950155 | 70 0.00 -219.23 1399.27 5 0 0.000 0.000 | 69 0.00 1008.39 1434.56 22 0 0.000 0.000
458 b2950155 | 70 0.00 -219.23 1399.27 5 0 0.000 0.000 | 69 0.00 1008.39 1434.56 22 0 0.000 0.000
460 b2950155 | 70 0.00 -219.23 1399.27 5 0 0.000 0.000 | 69 0.00 1008.39 1434.56 22 0 0.000 0.000
";

#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn a_bomb_and_a_boomerang_hit_the_dummy_together_as_on_the_n64() {
    let replay = Replay {
        p1: FighterKind::Link,
        route: "60:14:x=127;130:1:b+y=-80;180:1:z+a+y=127;275:1:b",
        weapons: &fighter_weapons,
    };
    let Some(out) = replay.run(LINK_BOMB_AND_BOOMERANG_HIT_THE_DUMMY) else {
        return;
    };
    out.check("bomb and Boomerang into the dummy");
}

/// N64 trace `hroll`: `hw3d`'s route (Link's smash sets off his bomb next
/// to the dummy) with the dummy's percent poked to 100 after frame 200. The
/// explosion sends it into `DamageFlyRoll`, whose physics pitches joint 4
/// each frame, and it lands face up on frame 341 (`DownBounceU`).
const LINK_BOMB_TUMBLES_THE_DUMMY: &str = "
60 18be873a
66 3d6c1037
188 2cd60a1e
283 72ae7869
296 72ae7869 | 204 14.00 565.80 1398.04 0 0 0.000 0.000 | 10 47.00 792.00 1415.47 100 0 0.000 0.000 | 21 578.93 1896.68 0.000 -64.800
297 72ae7869 | 204 15.00 565.80 1398.04 0 0 0.000 0.000 | 10 48.00 792.00 1415.47 100 0 0.000 0.000 | 21 578.93 1830.68 0.000 -66.000
298 72ae7869 | 204 16.00 565.80 1398.04 0 11 0.000 0.000 | 10 49.00 792.00 1415.47 100 0 0.000 0.000 | 21 578.93 1763.48 0.000 0.000
299 c40df552 | 52 1.00 565.80 1398.04 5 6 -47.238 43.285 | 55 1.00 792.00 1415.47 105 6 54.477 49.919 | 21 578.93 1763.48 0.000 0.000
300 c40df552 | 52 1.00 565.80 1398.04 5 5 -47.238 43.285 | 55 1.00 792.00 1415.47 105 5 54.477 49.919 | 21 578.93 1763.48 0.000 0.000
301 074d495a | 52 1.00 565.80 1398.04 5 4 -47.238 43.285 | 55 1.00 792.00 1415.47 105 4 54.477 49.919 | 21 578.93 1763.48 0.000 0.000
302 074d495a | 52 1.00 565.80 1398.04 5 3 -47.238 43.285 | 55 1.00 792.00 1415.47 105 3 54.477 49.919 | 21 578.93 1763.48 0.000 0.000
303 fa5aea62 | 52 1.00 565.80 1398.04 5 2 -47.238 43.285 | 55 1.00 792.00 1415.47 105 2 54.477 49.919 | 21 578.93 1763.48 0.000 0.000
304 fa5aea62 | 52 1.00 565.80 1398.04 5 1 -47.238 43.285 | 55 1.00 792.00 1415.47 105 1 54.477 49.919 | 21 578.93 1763.48 0.000 0.000
305 7ff56027 | 52 2.00 519.81 1436.78 5 0 -45.984 42.137 | 55 2.00 845.22 1461.84 105 0 53.224 48.771 | 21 578.93 1763.48 0.000 0.000
306 7ff56027 | 52 3.00 475.08 1470.97 5 0 -44.731 40.988 | 55 3.00 897.19 1504.66 105 0 51.971 47.622 | 21 578.93 1763.48 0.000 0.000
307 01202500 | 52 4.00 431.61 1500.61 5 0 -43.478 39.840 | 55 4.00 947.91 1543.94 105 0 50.717 46.474 | 21 578.93 1763.48 0.000 0.000
308 01202500 | 52 5.00 389.38 1525.70 5 0 -42.224 38.691 | 55 5.00 997.38 1579.66 105 0 49.464 45.325 | 21 578.93 1763.48 0.000 0.000
309 3e1203c8 | 52 6.00 348.41 1546.24 5 0 -40.971 37.543 | 55 6.00 1045.59 1611.84 105 0 48.211 44.177 | 21 578.93 1763.48 0.000 0.000
310 3e1203c8 | 52 7.00 308.69 1562.24 5 0 -39.717 36.394 | 55 7.00 1092.54 1640.47 105 0 46.957 43.028 | 21 578.93 1763.48 0.000 0.000
311 4df26f25 | 52 8.00 270.23 1573.68 5 0 -38.464 35.246 | 55 8.00 1138.25 1665.55 105 0 45.704 41.880 | 21 578.93 1763.48 0.000 0.000
312 4df26f25 | 52 9.00 233.02 1580.58 5 0 -37.211 34.097 | 55 9.00 1182.70 1687.08 105 0 44.450 40.731 | 21 578.93 1763.48 0.000 0.000
313 bef6be20 | 52 10.00 197.06 1582.93 5 0 -35.957 32.949 | 55 10.00 1225.90 1705.06 105 0 43.197 39.583 | 21 578.93 1763.48 0.000 0.000
314 bef6be20 | 52 11.00 162.36 1580.73 5 0 -34.704 31.800 | 55 11.00 1267.84 1719.50 105 0 41.944 38.434
315 58b070e8 | 52 12.00 128.91 1573.98 5 0 -33.451 30.652 | 55 12.00 1308.53 1730.38 105 0 40.690 37.286
316 58b070e8 | 52 13.00 96.71 1562.68 5 0 -32.197 29.503 | 55 13.00 1347.97 1737.72 105 0 39.437 36.137
317 692c68b0 | 52 14.00 65.77 1546.84 5 0 -30.944 28.355 | 55 14.00 1386.15 1741.51 105 0 38.184 34.989
318 692c68b0 | 52 15.00 36.08 1526.45 5 0 -29.690 27.206 | 55 15.00 1423.08 1741.75 105 0 36.930 33.840
319 5db2c578 | 52 16.00 7.64 1501.50 5 0 -28.437 26.058 | 55 0.00 1458.76 1738.44 105 0 35.677 32.692
320 5db2c578 | 52 17.00 -19.54 1472.01 5 0 -27.184 24.909 | 55 1.00 1493.18 1731.58 105 0 34.423 31.543
321 0fbfa740 | 52 18.00 -45.48 1437.97 5 0 -25.930 23.761 | 55 2.00 1526.35 1721.18 105 0 33.170 30.395
322 0fbfa740 | 52 19.00 -70.15 1399.39 5 0 -24.677 22.612 | 55 3.00 1558.27 1707.23 105 0 31.917 29.246
323 f9832e08 | 68 0.00 -93.58 1399.07 5 0 -23.424 0.036 | 55 4.00 1588.93 1691.32 105 0 30.663 28.098
324 bc61314c | 68 1.00 -115.00 1399.10 5 0 -21.424 0.033 | 55 5.00 1618.34 1674.27 105 0 29.410 26.949
325 ba6179d0 | 68 2.00 -134.42 1399.13 5 0 -19.424 0.030 | 55 6.00 1646.50 1656.07 105 0 28.157 25.801
326 5e9dab94 | 68 3.00 -151.85 1399.16 5 0 -17.424 0.027 | 55 7.00 1673.40 1636.73 105 0 26.903 24.652
327 eb72aa98 | 68 4.00 -167.27 1399.19 5 0 -15.424 0.024 | 55 8.00 1699.05 1616.23 105 0 25.650 23.504
328 83849adc | 68 5.00 -180.69 1399.21 5 0 -13.424 0.021 | 55 9.00 1723.45 1594.58 105 0 24.396 22.355
329 4402e060 | 68 6.00 -192.12 1399.22 5 0 -11.424 0.018 | 55 10.00 1746.59 1571.79 105 0 23.143 21.207
330 d42c1f24 | 68 7.00 -201.54 1399.24 5 0 -9.424 0.015 | 55 11.00 1768.48 1547.85 105 0 21.890 20.058
331 0e123b28 | 68 8.00 -208.96 1399.25 5 0 -7.424 0.012 | 55 12.00 1789.12 1522.76 105 0 20.636 18.910
332 a49e586c | 68 9.00 -214.39 1399.26 5 0 -5.424 0.008 | 55 13.00 1808.50 1496.52 105 0 19.383 17.761
333 ead4daf0 | 68 10.00 -217.81 1399.26 5 0 -3.424 0.005 | 55 14.00 1826.63 1469.13 105 0 18.130 16.613
334 205966b4 | 68 11.00 -219.23 1399.27 5 0 -1.424 0.002 | 55 15.00 1843.50 1440.60 105 0 16.876 15.464
335 e732dfb8 | 68 12.00 -219.23 1399.27 5 0 0.000 0.000 | 55 0.00 1859.13 1410.91 105 0 15.623 14.316
336 c6cf69fc | 68 13.00 -219.23 1399.27 5 0 0.000 0.000 | 55 1.00 1873.50 1380.08 105 0 14.370 13.167
337 e0486980 | 68 14.00 -219.23 1399.27 5 0 0.000 0.000 | 55 2.00 1886.61 1348.10 105 0 13.116 12.019
338 37e68244 | 68 15.00 -219.23 1399.27 5 0 0.000 0.000 | 55 3.00 1898.48 1314.97 105 0 11.863 10.870
339 37e59848 | 68 16.00 -219.23 1399.27 5 0 0.000 0.000 | 55 4.00 1909.09 1280.69 105 0 10.609 9.722
340 5078cf8c | 68 17.00 -219.23 1399.27 5 0 0.000 0.000 | 55 5.00 1918.44 1245.26 105 0 9.356 8.573
341 d90e8c10 | 68 18.00 -219.23 1399.27 5 0 0.000 0.000 | 68 0.00 1926.54 1226.42 105 0 8.102 0.099
342 8c7b64d8 | 68 19.00 -219.23 1399.27 5 0 0.000 0.000 | 68 1.00 1933.15 1226.50 105 0 6.602 0.081
343 8c7b64d8 | 68 20.00 -219.23 1399.27 5 0 0.000 0.000 | 68 2.00 1938.25 1226.56 105 0 5.102 0.062
344 9d1842a0 | 68 21.00 -219.23 1399.27 5 0 0.000 0.000 | 68 3.00 1941.85 1226.61 105 0 3.602 0.044
345 9d1842a0 | 68 22.00 -219.23 1399.27 5 0 0.000 0.000 | 68 4.00 1943.95 1226.63 105 0 2.103 0.026
346 d0643564 | 68 23.00 -219.23 1399.27 5 0 0.000 0.000 | 68 5.00 1944.56 1226.64 105 0 0.603 0.007
347 d0643564 | 68 24.00 -219.23 1399.27 5 0 0.000 0.000 | 68 6.00 1944.56 1226.64 105 0 0.000 0.000
348 d0643564 | 68 25.00 -219.23 1399.27 5 0 0.000 0.000 | 68 7.00 1944.56 1226.64 105 0 0.000 0.000
350 cc854568
352 cc854568 | 70 0.00 -219.23 1399.27 5 0 0.000 0.000 | 68 11.00 1944.56 1226.64 105 0 0.000 0.000
354 4ff896ac
356 4ff896ac | 70 0.00 -219.23 1399.27 5 0 0.000 0.000 | 68 15.00 1944.56 1226.64 105 0 0.000 0.000
358 97968d30
360 97968d30 | 70 0.00 -219.23 1399.27 5 0 0.000 0.000 | 68 19.00 1944.56 1226.64 105 0 0.000 0.000
362 1f16ccf4
364 1f16ccf4 | 70 0.00 -219.23 1399.27 5 0 0.000 0.000 | 68 23.00 1944.56 1226.64 105 0 0.000 0.000
366 cad439f8
368 cad439f8 | 70 0.00 -219.23 1399.27 5 0 0.000 0.000 | 70 0.00 1944.56 1226.64 105 0 0.000 0.000
370 5ed0f83c
372 5ed0f83c | 70 0.00 -219.23 1399.27 5 0 0.000 0.000 | 70 0.00 1944.56 1226.64 105 0 0.000 0.000
374 66fa6bc0
376 66fa6bc0 | 70 0.00 -219.23 1399.27 5 0 0.000 0.000 | 70 0.00 1944.56 1226.64 105 0 0.000 0.000
378 f4ad3884
380 f4ad3884 | 70 0.00 -219.23 1399.27 5 0 0.000 0.000 | 70 0.00 1944.56 1226.64 105 0 0.000 0.000
382 d5794288
384 d5794288 | 70 0.00 -219.23 1399.27 5 0 0.000 0.000 | 70 0.00 1944.56 1226.64 105 0 0.000 0.000
386 2725adcc
388 2725adcc | 70 0.00 -219.23 1399.27 5 0 0.000 0.000 | 70 0.00 1944.56 1226.64 105 0 0.000 0.000
390 6cf4de50
392 6cf4de50 | 70 0.00 -219.23 1399.27 5 0 0.000 0.000 | 70 0.00 1944.56 1226.64 105 0 0.000 0.000
394 8a287814
396 8a287814 | 70 0.00 -219.23 1399.27 5 0 0.000 0.000 | 70 0.00 1944.56 1226.64 105 0 0.000 0.000
398 45c55f18
400 45c55f18 | 70 0.00 -219.23 1399.27 5 0 0.000 0.000 | 70 0.00 1944.56 1226.64 105 0 0.000 0.000
402 3b97b75c
404 3b97b75c | 70 0.00 -219.23 1399.27 5 0 0.000 0.000 | 70 0.00 1944.56 1226.64 105 0 0.000 0.000
406 5e76e4e0
408 5e76e4e0 | 70 0.00 -219.23 1399.27 5 0 0.000 0.000 | 70 0.00 1944.56 1226.64 105 0 0.000 0.000
410 6fc98ba4
412 6fc98ba4 | 70 0.00 -219.23 1399.27 5 0 0.000 0.000 | 70 0.00 1944.56 1226.64 105 0 0.000 0.000
414 10498fa8
416 10498fa8 | 70 0.00 -219.23 1399.27 5 0 0.000 0.000 | 70 0.00 1944.56 1226.64 105 0 0.000 0.000
418 4e0814ec
420 4e0814ec | 70 0.00 -219.23 1399.27 5 0 0.000 0.000 | 70 0.00 1944.56 1226.64 105 0 0.000 0.000
422 d3b17f70
424 d3b17f70 | 70 0.00 -219.23 1399.27 5 0 0.000 0.000 | 70 0.00 1944.56 1226.64 105 0 0.000 0.000
428 d3b17f70 | 70 0.00 -219.23 1399.27 5 0 0.000 0.000 | 70 0.00 1944.56 1226.64 105 0 0.000 0.000
432 d3b17f70 | 70 0.00 -219.23 1399.27 5 0 0.000 0.000 | 70 0.00 1944.56 1226.64 105 0 0.000 0.000
436 d3b17f70 | 70 0.00 -219.23 1399.27 5 0 0.000 0.000 | 70 0.00 1944.56 1226.64 105 0 0.000 0.000
440 d3b17f70 | 70 0.00 -219.23 1399.27 5 0 0.000 0.000 | 70 0.00 1944.56 1226.64 105 0 0.000 0.000
";

#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn a_bomb_tumbles_a_damaged_dummy_as_on_the_n64() {
    let replay = Replay {
        p1: FighterKind::Link,
        route: "60:14:x=127;130:1:b+y=-80;180:1:z+a+y=127;283:1:a+x=127",
        weapons: &fighter_weapons,
    };
    let Some(out) = replay.run_poked(LINK_BOMB_TUMBLES_THE_DUMMY, 0, Some((200, 100))) else {
        return;
    };
    out.check("bomb tumbles the dummy");
}

/// `b30set/ys1` (`warp_yamabuki_mario`, `dITManagerForceMonsterKind` 1,
/// `monster_wait` 1 after frame 60): Saffron's gate opens at frame 1
/// (`gate_wait` 1), Chansey comes out at 61, walks out to the near gate
/// (`gate_noentry` at 72), back in, and the gate closes at 220 with a
/// fresh `monster_wait` draw.
const SAFFRON_CHANSEY_COMES_OUT: &str = "
0 0029e2c0 | 10 1.00 1200.00 810.00 0 0 0.000 0.000 | 10 1.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 1000 1 960.00 360.00
6 0029e2c0 | 10 7.00 1200.00 810.00 0 0 0.000 0.000 | 10 7.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 994 0 1600.00 360.00
12 0029e2c0 | 10 13.00 1200.00 810.00 0 0 0.000 0.000 | 10 13.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 988 0 1600.00 360.00
18 0029e2c0 | 10 19.00 1200.00 810.00 0 0 0.000 0.000 | 10 19.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 982 0 1600.00 360.00
24 0029e2c0 | 10 25.00 1200.00 810.00 0 0 0.000 0.000 | 10 25.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 976 0 1600.00 360.00
30 0029e2c0 | 10 31.00 1200.00 810.00 0 0 0.000 0.000 | 10 31.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 970 0 1600.00 360.00
36 0029e2c0 | 10 37.00 1200.00 810.00 0 0 0.000 0.000 | 10 37.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 964 0 1600.00 360.00
42 0029e2c0 | 10 43.00 1200.00 810.00 0 0 0.000 0.000 | 10 43.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 958 0 1600.00 360.00
48 0029e2c0 | 10 49.00 1200.00 810.00 0 0 0.000 0.000 | 10 49.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 952 0 1600.00 360.00
54 0029e2c0 | 10 5.00 1200.00 810.00 0 0 0.000 0.000 | 10 5.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 946 0 1600.00 360.00
60 0029e2c0 | 10 11.00 1200.00 810.00 0 0 0.000 0.000 | 10 11.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 1 0 1600.00 360.00
66 0029e2c0 | 10 17.00 1200.00 810.00 0 0 0.000 0.000 | 10 17.00 -660.00 -368.00 0 0 0.000 0.000 | 27 1169.97 270.00 0.000 0.000 |  |  | 2 0 0 0 999.97 360.00
72 0029e2c0 | 10 23.00 1200.00 810.00 0 0 0.000 0.000 | 10 23.00 -660.00 -368.00 0 0 0.000 0.000 | 27 1019.94 270.00 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
78 0029e2c0 | 10 29.00 1200.00 810.00 0 0 0.000 0.000 | 10 29.00 -660.00 -368.00 0 0 0.000 0.000 | 27 869.90 270.00 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
84 0029e2c0 | 10 35.00 1200.00 810.00 0 0 0.000 0.000 | 10 35.00 -660.00 -368.00 0 0 0.000 0.000 | 27 868.79 270.00 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
90 0029e2c0 | 10 41.00 1200.00 810.00 0 0 0.000 0.000 | 10 41.00 -660.00 -368.00 0 0 0.000 0.000 | 27 867.68 270.00 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
96 0029e2c0 | 10 47.00 1200.00 810.00 0 0 0.000 0.000 | 10 47.00 -660.00 -368.00 0 0 0.000 0.000 | 27 866.57 270.00 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
102 0029e2c0 | 10 3.00 1200.00 810.00 0 0 0.000 0.000 | 10 3.00 -660.00 -368.00 0 0 0.000 0.000 | 27 865.46 270.00 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
108 0029e2c0 | 10 9.00 1200.00 810.00 0 0 0.000 0.000 | 10 9.00 -660.00 -368.00 0 0 0.000 0.000 | 27 864.35 270.00 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
114 0029e2c0 | 10 15.00 1200.00 810.00 0 0 0.000 0.000 | 10 15.00 -660.00 -368.00 0 0 0.000 0.000 | 27 863.23 270.00 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
120 0029e2c0 | 10 21.00 1200.00 810.00 0 0 0.000 0.000 | 10 21.00 -660.00 -368.00 0 0 0.000 0.000 | 27 862.12 270.00 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
126 0029e2c0 | 10 27.00 1200.00 810.00 0 0 0.000 0.000 | 10 27.00 -660.00 -368.00 0 0 0.000 0.000 | 27 861.01 270.00 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
132 0029e2c0 | 10 33.00 1200.00 810.00 0 0 0.000 0.000 | 10 33.00 -660.00 -368.00 0 0 0.000 0.000 | 27 865.27 270.00 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
138 0029e2c0 | 10 39.00 1200.00 810.00 0 0 0.000 0.000 | 10 39.00 -660.00 -368.00 0 0 0.000 0.000 | 27 896.37 270.00 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
144 0029e2c0 | 10 45.00 1200.00 810.00 0 0 0.000 0.000 | 10 45.00 -660.00 -368.00 0 0 0.000 0.000 | 27 927.48 270.00 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
150 0029e2c0 | 10 1.00 1200.00 810.00 0 0 0.000 0.000 | 10 1.00 -660.00 -368.00 0 0 0.000 0.000 | 27 958.58 270.00 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
156 0029e2c0 | 10 7.00 1200.00 810.00 0 0 0.000 0.000 | 10 7.00 -660.00 -368.00 0 0 0.000 0.000 | 27 989.69 270.00 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
162 0029e2c0 | 10 13.00 1200.00 810.00 0 0 0.000 0.000 | 10 13.00 -660.00 -368.00 0 0 0.000 0.000 | 27 1020.79 270.00 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
168 0029e2c0 | 10 19.00 1200.00 810.00 0 0 0.000 0.000 | 10 19.00 -660.00 -368.00 0 0 0.000 0.000 | 27 1051.90 270.00 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
174 0029e2c0 | 10 25.00 1200.00 810.00 0 0 0.000 0.000 | 10 25.00 -660.00 -368.00 0 0 0.000 0.000 | 27 1083.00 270.00 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
180 0029e2c0 | 10 31.00 1200.00 810.00 0 0 0.000 0.000 | 10 31.00 -660.00 -368.00 0 0 0.000 0.000 | 27 1114.11 270.00 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
186 0029e2c0 | 10 37.00 1200.00 810.00 0 0 0.000 0.000 | 10 37.00 -660.00 -368.00 0 0 0.000 0.000 | 27 1145.21 270.00 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
192 0029e2c0 | 10 43.00 1200.00 810.00 0 0 0.000 0.000 | 10 43.00 -660.00 -368.00 0 0 0.000 0.000 | 27 1176.32 270.00 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
198 0029e2c0 | 10 49.00 1200.00 810.00 0 0 0.000 0.000 | 10 49.00 -660.00 -368.00 0 0 0.000 0.000 | 27 1207.42 270.00 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
204 0029e2c0 | 10 5.00 1200.00 810.00 0 0 0.000 0.000 | 10 5.00 -660.00 -368.00 0 0 0.000 0.000 | 27 1238.53 270.00 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
210 0029e2c0 | 10 11.00 1200.00 810.00 0 0 0.000 0.000 | 10 11.00 -660.00 -368.00 0 0 0.000 0.000 | 27 1269.63 270.00 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
216 0029e2c0 | 10 17.00 1200.00 810.00 0 0 0.000 0.000 | 10 17.00 -660.00 -368.00 0 0 0.000 0.000 | 27 1300.73 270.00 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
220 c823f683
222 c823f683 | 10 23.00 1200.00 810.00 0 0 0.000 0.000 | 10 23.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 1 1779 998 960.00 360.00
228 c823f683 | 10 29.00 1200.00 810.00 0 0 0.000 0.000 | 10 29.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 1 1773 992 960.00 360.00
234 c823f683 | 10 35.00 1200.00 810.00 0 0 0.000 0.000 | 10 35.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 1 1767 986 960.00 360.00
240 c823f683 | 10 41.00 1200.00 810.00 0 0 0.000 0.000 | 10 41.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 1 1761 980 960.00 360.00
";

#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn saffrons_gate_lets_chansey_out_as_on_the_n64() {
    let replay = Replay {
        p1: FighterKind::Mario,
        route: "999:1:",
        weapons: &monster_weapons,
    };
    let Some(out) = replay.run_saffron(SAFFRON_CHANSEY_COMES_OUT, 1, (60, 1)) else {
        return;
    };
    out.check("Saffron's gate and a forced Chansey");
}

/// `b30set/b36ys_unforced` (`warp_yamabuki_mario`, no input, no forced
/// kind; `monster_wait` 1 after frames 60, 210, .. 2160): twelve gate
/// openings, each the frame after a write that found the gate waiting
/// (61, 211, 511, 661, 811, 961, 1111, 1411, 1561, 1711, 1861, 2011).
/// `grYamabukiGateMakeMonster`'s `syUtilsRandIntRange(5)` is the frame's
/// first draw; ids 3 0 2 1 2 3 0 2 4 2 3 0. At 961 and 1861 the draw is
/// 2, `monster_id_prev`, so the gate takes 3. Rows every 30 frames, each
/// opening's frames, and the seed's changes.
const SAFFRON_UNFORCED_PICKS: &str = "
1 0029e2c0
30 0029e2c0 | 10 31.00 1200.00 810.00 0 0 0.000 0.000 | 10 31.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 970 0 1600.00 360.00
60 0029e2c0 | 10 11.00 1200.00 810.00 0 0 0.000 0.000 | 10 11.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 1 0 1600.00 360.00
61 18be873a | 10 12.00 1200.00 810.00 0 0 0.000 0.000 | 10 12.00 -660.00 -368.00 0 0 0.000 0.000 | 30 1200.81 367.24 0.000 0.000 |  |  | 2 0 0 0 1600.00 360.00
62 18be873a | 10 13.00 1200.00 810.00 0 0 0.000 0.000 | 10 13.00 -660.00 -368.00 0 0 0.000 0.000 | 30 1081.63 372.81 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
90 18be873a | 10 41.00 1200.00 810.00 0 0 0.000 0.000 | 10 41.00 -660.00 -368.00 0 0 0.000 0.000 | 30 655.49 270.00 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
120 18be873a | 10 21.00 1200.00 810.00 0 0 0.000 0.000 | 10 21.00 -660.00 -368.00 0 0 0.000 0.000 | 30 648.43 270.00 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
150 18be873a | 10 1.00 1200.00 810.00 0 0 0.000 0.000 | 10 1.00 -660.00 -368.00 0 0 0.000 0.000 | 30 746.80 270.00 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
180 18be873a | 10 31.00 1200.00 810.00 0 0 0.000 0.000 | 10 31.00 -660.00 -368.00 0 0 0.000 0.000 | 30 1321.41 270.00 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
188 3d6c1037
210 3d6c1037 | 10 11.00 1200.00 810.00 0 0 0.000 0.000 | 10 11.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 1 1 978 960.00 360.00
211 2cd60a1e | 10 12.00 1200.00 810.00 0 0 0.000 0.000 | 10 12.00 -660.00 -368.00 0 0 0.000 0.000 | 27 1294.99 270.00 0.000 0.000 |  |  | 2 0 0 977 960.00 360.00
212 2cd60a1e | 10 13.00 1200.00 810.00 0 0 0.000 0.000 | 10 13.00 -660.00 -368.00 0 0 0.000 0.000 | 27 1269.99 270.00 0.000 0.000 |  |  | 2 0 0 977 1099.99 360.00
240 2cd60a1e | 10 41.00 1200.00 810.00 0 0 0.000 0.000 | 10 41.00 -660.00 -368.00 0 0 0.000 0.000 | 27 867.68 270.00 0.000 0.000 |  |  | 2 1 0 977 960.00 360.00
270 2cd60a1e | 10 21.00 1200.00 810.00 0 0 0.000 0.000 | 10 21.00 -660.00 -368.00 0 0 0.000 0.000 | 27 862.12 270.00 0.000 0.000 |  |  | 2 1 0 977 960.00 360.00
300 2cd60a1e | 10 1.00 1200.00 810.00 0 0 0.000 0.000 | 10 1.00 -660.00 -368.00 0 0 0.000 0.000 | 27 958.58 270.00 0.000 0.000 |  |  | 2 1 0 977 960.00 360.00
330 2cd60a1e | 10 31.00 1200.00 810.00 0 0 0.000 0.000 | 10 31.00 -660.00 -368.00 0 0 0.000 0.000 | 27 1114.11 270.00 0.000 0.000 |  |  | 2 1 0 977 960.00 360.00
360 2cd60a1e | 10 11.00 1200.00 810.00 0 0 0.000 0.000 | 10 11.00 -660.00 -368.00 0 0 0.000 0.000 | 27 1269.63 270.00 0.000 0.000 |  |  | 2 1 1 977 960.00 360.00
370 72ae7869
390 72ae7869 | 10 41.00 1200.00 810.00 0 0 0.000 0.000 | 10 41.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 1 1427 980 960.00 360.00
420 72ae7869 | 10 21.00 1200.00 810.00 0 0 0.000 0.000 | 10 21.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 1 1397 950 960.00 360.00
450 72ae7869 | 10 1.00 1200.00 810.00 0 0 0.000 0.000 | 10 1.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 1 1367 920 960.00 360.00
480 72ae7869 | 10 31.00 1200.00 810.00 0 0 0.000 0.000 | 10 31.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 1 1337 890 960.00 360.00
510 72ae7869 | 10 11.00 1200.00 810.00 0 0 0.000 0.000 | 10 11.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 1 1 860 960.00 360.00
511 df90722b | 10 12.00 1200.00 810.00 0 0 0.000 0.000 | 10 12.00 -660.00 -368.00 0 0 0.000 0.000 | 29 1235.22 273.01 0.000 0.000 |  |  | 2 0 0 859 960.00 360.00
512 df90722b | 10 13.00 1200.00 810.00 0 0 0.000 0.000 | 10 13.00 -660.00 -368.00 0 0 0.000 0.000 | 29 1157.48 276.15 0.000 0.000 |  |  | 2 0 0 859 962.22 360.00
540 df90722b | 10 41.00 1200.00 810.00 0 0 0.000 0.000 | 10 41.00 -660.00 -368.00 0 0 0.000 0.000 | 29 712.29 263.13 0.000 0.000 |  |  | 2 1 0 859 960.00 360.00
550 daf1b4cc
559 81eb59d1
568 12dbce4a
570 12dbce4a | 10 21.00 1200.00 810.00 0 0 0.000 0.000 | 10 21.00 -660.00 -368.00 0 0 0.000 0.000 | 29 707.44 270.00 0.000 0.000 | 30 -71.69 189.21 -44.017 -9.356 ; 30 324.99 249.96 -44.017 -9.356 |  | 2 1 0 859 960.00 360.00
577 b90c5187
586 8124f618
595 491c86cd
600 491c86cd | 10 1.00 1200.00 810.00 0 0 0.000 0.000 | 10 1.00 -660.00 -368.00 0 0 0.000 0.000 | 29 724.30 270.00 0.000 0.000 | 30 -196.30 163.03 -44.017 -9.356 ; 30 206.03 230.52 -44.017 -9.356 |  | 2 1 0 859 960.00 360.00
604 95475fb6
613 2d128b23
622 6443b2a4
630 e6bb16d7 | 10 31.00 1200.00 810.00 0 0 0.000 0.000 | 10 31.00 -660.00 -368.00 0 0 0.000 0.000 |  | 30 -147.17 134.37 -44.017 -9.356 |  | 1 1 1901 1000 960.00 360.00
660 e6bb16d7 | 10 11.00 1200.00 810.00 0 0 0.000 0.000 | 10 11.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 1 1 970 960.00 360.00
661 428b763e | 10 12.00 1200.00 810.00 0 0 0.000 0.000 | 10 12.00 -660.00 -368.00 0 0 0.000 0.000 | 28 1299.85 343.79 0.000 0.000 |  |  | 2 0 0 969 960.00 360.00
662 428b763e | 10 13.00 1200.00 810.00 0 0 0.000 0.000 | 10 13.00 -660.00 -368.00 0 0 0.000 0.000 | 28 1280.15 405.89 0.000 0.000 |  |  | 2 0 0 969 1051.85 360.00
690 428b763e | 10 41.00 1200.00 810.00 0 0 0.000 0.000 | 10 41.00 -660.00 -368.00 0 0 0.000 0.000 | 28 890.17 304.79 0.000 0.000 |  |  | 2 1 0 969 960.00 360.00
720 428b763e | 10 21.00 1200.00 810.00 0 0 0.000 0.000 | 10 21.00 -660.00 -368.00 0 0 0.000 0.000 | 28 733.16 270.00 0.000 0.000 |  |  | 2 1 0 969 960.00 360.00
750 428b763e | 10 1.00 1200.00 810.00 0 0 0.000 0.000 | 10 1.00 -660.00 -368.00 0 0 0.000 0.000 | 28 709.94 270.00 0.000 0.000 |  |  | 2 1 0 969 960.00 360.00
780 45099bec | 52 1.00 1200.00 810.00 30 15 92.307 84.584 | 10 31.00 -660.00 -368.00 0 0 0.000 0.000 | 28 736.06 270.00 0.000 0.000 |  |  | 2 1 0 969 960.00 360.00
781 e3cbf86a
782 ce45da4e
784 eb899372
786 07323f41
787 a350db7a
788 eb36b25e
789 1a49b36b
790 797d559f
792 314fe813
793 c9442554
794 b66b3058
795 542236e1
796 409dcdf5
798 73dadb49
800 890252dd
802 704968b1
803 ca8024b3
804 3cd5ad67
806 b3ea2e5b
808 e0bf3b8f
810 af14a903 | 52 17.00 2506.46 1680.75 30 0 72.253 66.208 | 10 11.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 1 1 976 960.00 360.00
811 047eb795 | 52 18.00 2577.46 1705.00 30 0 71.000 65.059 | 10 12.00 -660.00 -368.00 0 0 0.000 0.000 | 29 1235.22 273.01 0.000 0.000 |  |  | 2 0 0 975 960.00 360.00
812 8d66c6e9 | 52 19.00 2647.20 1725.72 30 0 69.746 63.911 | 10 13.00 -660.00 -368.00 0 0 0.000 0.000 | 29 1157.48 276.15 0.000 0.000 |  |  | 2 0 0 975 962.22 360.00
814 2c49e07d
816 a88f3851
818 261e4265
820 e172b2b9
822 f1f07d4d
824 0677d621
826 50393135
828 9953d157
832 0fc94289
836 5f1e8d4b
840 7874fe1d | 52 0.00 4091.23 1816.93 30 0 34.652 31.753 | 10 41.00 -660.00 -368.00 0 0 0.000 0.000 | 29 712.29 263.13 0.000 0.000 |  |  | 2 1 0 975 960.00 360.00
844 2b0c457f
848 ddd597f1
852 5a9fcdf3
856 5fa48405
860 927e3aa7
863 87cf7659
867 6d22df9b
870 6d22df9b | 57 10.00 4553.06 920.11 30 0 0.000 0.000 | 10 21.00 -660.00 -368.00 0 0 0.000 0.000 | 29 707.44 270.00 0.000 0.000 |  |  | 2 1 0 975 960.00 360.00
871 8ecc62ed
875 01d350cf
879 3a2d7dc1
883 d7d36243
887 89753ad5
891 19d927f7
895 a52a4e29
899 1dc0f5eb
900 1dc0f5eb | 57 11.00 4553.06 -399.89 30 0 0.000 0.000 | 10 1.00 -660.00 -368.00 0 0 0.000 0.000 | 29 724.30 270.00 0.000 0.000 |  |  | 2 1 0 975 960.00 360.00
903 c42babbd
930 5078cf8c | 57 12.00 4553.06 -1719.89 30 0 0.000 0.000 | 10 31.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 1 1314 1000 960.00 360.00
960 5078cf8c | 57 13.00 4553.06 -3039.89 30 0 0.000 0.000 | 10 11.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 1 1 970 960.00 360.00
961 c087ba66 | 57 14.00 4553.06 -3083.89 30 0 0.000 0.000 | 10 12.00 -660.00 -368.00 0 0 0.000 0.000 | 30 1200.81 367.24 0.000 0.000 |  |  | 2 0 0 969 960.00 360.00
962 c087ba66 | 57 15.00 4553.06 -3127.89 30 0 0.000 0.000 | 10 13.00 -660.00 -368.00 0 0 0.000 0.000 | 30 1081.63 372.81 0.000 0.000 |  |  | 2 1 0 969 960.00 360.00
990 c087ba66 | 57 14.00 4553.06 -4359.89 30 0 0.000 0.000 | 10 41.00 -660.00 -368.00 0 0 0.000 0.000 | 30 655.49 270.00 0.000 0.000 |  |  | 2 1 0 969 960.00 360.00
1000 1850fb0a
1016 ba8daeee
1020 ba8daeee | 57 15.00 4553.06 -5679.89 30 0 0.000 0.000 | 10 21.00 -660.00 -368.00 0 0 0.000 0.000 | 30 648.43 270.00 0.000 0.000 | 31 -3087.55 270.00 -205.000 0.000 ; 31 -446.83 270.00 -125.000 0.000 |  | 2 1 0 969 960.00 360.00
1028 0c15da12
1029 9d1842a0
1030 627056e3
1031 d13e6e8b
1032 de73b733
1033 d5794288
1034 9643c55f
1035 8de58aa6
1036 3459a652
1037 263d73cd
1038 e479db62
1039 4e0814ec
1040 293be073
1041 f5c1bb4e
1042 54d6d438
1043 d815ae00
1044 dfa86182
1045 30a7cc0c
1046 50bf8d13
1047 2fe73c25
1048 01e18e9a
1049 9030daf5
1050 cfe2f10b | 0 16.00 4553.06 -6031.89 30 0 0.000 0.000 | 10 1.00 -660.00 -368.00 0 0 0.000 0.000 | 30 746.80 270.00 0.000 0.000 | 31 -2763.49 270.00 -195.000 0.000 ; 31 -145.24 270.00 -115.000 0.000 |  | 2 1 0 969 960.00 360.00
1051 e747a5b1
1052 7346ddc5
1054 64e07fb7
1055 f296bb9e
1056 1d5eeb26
1057 9ff1f551
1058 43284494
1060 f21d1398
1062 0a41932f
1063 06076136
1064 b305c2da
1066 6bc95f89
1070 c346d16c
1074 a0a8f2f3
1078 00eb2bce
1080 49d05cf2 | 7 -74.00 0.00 7826.38 0 0 0.000 0.000 | 10 31.00 -660.00 -368.00 0 0 0.000 0.000 | 30 1321.41 270.00 0.000 0.000 | 31 -2045.76 270.00 -185.000 0.000 ; 31 676.41 270.00 -105.000 0.000 |  | 2 1 0 969 960.00 360.00
1088 b10625cf
1110 b10625cf | 7 -74.00 0.00 3872.30 0 0 0.000 0.000 | 10 11.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 1 1 978 960.00 360.00
1111 008c2956 | 7 -74.00 0.00 3770.54 0 0 0.000 0.000 | 10 12.00 -660.00 -368.00 0 0 0.000 0.000 | 27 1294.99 270.00 0.000 0.000 |  |  | 2 0 0 977 960.00 360.00
1112 008c2956 | 7 -74.00 0.00 3670.72 0 0 0.000 0.000 | 10 13.00 -660.00 -368.00 0 0 0.000 0.000 | 27 1269.99 270.00 0.000 0.000 |  |  | 2 0 0 977 1099.99 360.00
1140 008c2956 | 7 -74.00 0.00 1662.67 0 0 0.000 0.000 | 10 41.00 -660.00 -368.00 0 0 0.000 0.000 | 27 867.68 270.00 0.000 0.000 |  |  | 2 1 0 977 960.00 360.00
1170 008c2956 | 8 23.00 0.00 1150.00 0 0 0.000 0.000 | 10 21.00 -660.00 -368.00 0 0 0.000 0.000 | 27 862.12 270.00 0.000 0.000 |  |  | 2 1 0 977 960.00 360.00
1200 008c2956 | 9 23.00 0.00 1150.00 0 0 0.000 0.000 | 10 1.00 -660.00 -368.00 0 0 0.000 0.000 | 27 958.58 270.00 0.000 0.000 |  |  | 2 1 0 977 960.00 360.00
1230 008c2956 | 9 3.00 0.00 1150.00 0 0 0.000 0.000 | 10 31.00 -660.00 -368.00 0 0 0.000 0.000 | 27 1114.11 270.00 0.000 0.000 |  |  | 2 1 0 977 960.00 360.00
1260 008c2956 | 9 33.00 0.00 1150.00 0 0 0.000 0.000 | 10 11.00 -660.00 -368.00 0 0 0.000 0.000 | 27 1269.63 270.00 0.000 0.000 |  |  | 2 1 1 977 960.00 360.00
1270 b57efac1
1290 b57efac1 | 9 13.00 0.00 1150.00 0 0 0.000 0.000 | 10 41.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 1 1688 980 960.00 360.00
1320 b57efac1 | 9 43.00 0.00 1150.00 0 0 0.000 0.000 | 10 21.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 1 1658 950 960.00 360.00
1350 b57efac1 | 9 23.00 0.00 1150.00 0 0 0.000 0.000 | 10 1.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 1 1628 920 960.00 360.00
1380 b57efac1 | 9 3.00 0.00 1150.00 0 0 0.000 0.000 | 10 31.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 1 1598 890 960.00 360.00
1410 b57efac1 | 9 33.00 0.00 1150.00 0 0 0.000 0.000 | 10 11.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 1 1 860 960.00 360.00
1411 67d0db44 | 9 34.00 0.00 1150.00 0 0 0.000 0.000 | 10 12.00 -660.00 -368.00 0 0 0.000 0.000 | 29 1235.22 273.01 0.000 0.000 |  |  | 2 0 0 859 960.00 360.00
1412 67d0db44 | 9 35.00 0.00 1150.00 0 0 0.000 0.000 | 10 13.00 -660.00 -368.00 0 0 0.000 0.000 | 29 1157.48 276.15 0.000 0.000 |  |  | 2 0 0 859 962.22 360.00
1420 66beeb29
1429 92469a02
1438 638c751f
1440 638c751f | 9 13.00 0.00 1150.00 0 0 0.000 0.000 | 10 41.00 -660.00 -368.00 0 0 0.000 0.000 | 29 712.29 263.13 0.000 0.000 | 30 -57.99 187.65 -44.017 -9.356 ; 30 331.42 243.12 -44.017 -9.356 |  | 2 1 0 859 960.00 360.00
1447 527f5510
1456 c6a722a5
1465 23320fee
1470 23320fee | 26 7.00 0.00 1063.60 0 0 0.000 0.000 | 10 21.00 -660.00 -368.00 0 0 0.000 0.000 | 29 707.44 270.00 0.000 0.000 | 30 -203.47 163.27 -44.017 -9.356 ; 30 192.54 232.21 -44.017 -9.356 |  | 2 1 0 859 960.00 360.00
1474 4f5b953b
1483 af0bc21c
1492 93f5b261
1500 93f5b261 | 26 7.00 0.00 -140.40 0 0 0.000 0.000 | 10 1.00 -660.00 -368.00 0 0 0.000 0.000 | 29 724.30 270.00 0.000 0.000 | 30 -330.02 134.18 -44.017 -9.356 ; 30 71.73 210.49 -44.017 -9.356 |  | 2 1 0 859 960.00 360.00
1501 f37d371a
1502 f3890e64
1510 e41b06c9
1519 e34f3022
1528 3a721fbf
1530 007bfb86 | 10 21.00 0.00 -185.54 0 0 0.000 0.000 | 10 31.00 -660.00 -368.00 0 0 0.000 0.000 |  | 30 362.59 188.42 -44.017 -9.356 |  | 1 1 1001 1000 960.00 360.00
1560 007bfb86 | 10 1.00 0.00 -185.54 0 0 0.000 0.000 | 10 11.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 1 1 970 960.00 360.00
1561 e0144431 | 10 2.00 0.00 -185.54 0 0 0.000 0.000 | 10 12.00 -660.00 -368.00 0 0 0.000 0.000 | 31 1237.57 270.12 0.000 0.000 |  |  | 2 0 0 969 960.00 360.00
1562 e0144431 | 10 3.00 0.00 -185.54 0 0 0.000 0.000 | 10 13.00 -660.00 -368.00 0 0 0.000 0.000 | 31 1155.13 270.24 0.000 0.000 |  |  | 2 1 0 969 960.00 360.00
1590 e0144431 | 10 31.00 0.00 -185.54 0 0 0.000 0.000 | 10 41.00 -660.00 -368.00 0 0 0.000 0.000 | 31 660.31 280.00 0.000 0.000 |  |  | 2 1 0 969 960.00 360.00
1592 b3cddc33
1620 b3cddc33 | 10 11.00 0.00 -185.54 0 0 0.000 0.000 | 10 21.00 -660.00 -368.00 0 0 0.000 0.000 | 31 660.01 270.07 0.000 0.000 |  |  | 2 1 0 969 960.00 360.00
1650 b3cddc33 | 10 41.00 0.00 -185.54 0 0 0.000 0.000 | 10 1.00 -660.00 -368.00 0 0 0.000 0.000 | 31 990.00 270.03 0.000 0.000 |  |  | 2 1 0 969 960.00 360.00
1679 27d3962a
1680 27d3962a | 10 21.00 0.00 -185.54 0 0 0.000 0.000 | 10 31.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 1 1154 999 960.00 360.00
1710 27d3962a | 10 1.00 0.00 -185.54 0 0 0.000 0.000 | 10 11.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 1 1 969 960.00 360.00
1711 04f0e5f4 | 10 2.00 0.00 -185.54 0 0 0.000 0.000 | 10 12.00 -660.00 -368.00 0 0 0.000 0.000 | 29 1235.22 273.01 0.000 0.000 |  |  | 2 0 0 968 960.00 360.00
1712 04f0e5f4 | 10 3.00 0.00 -185.54 0 0 0.000 0.000 | 10 13.00 -660.00 -368.00 0 0 0.000 0.000 | 29 1157.48 276.15 0.000 0.000 |  |  | 2 0 0 968 962.22 360.00
1740 04f0e5f4 | 10 31.00 0.00 -185.54 0 0 0.000 0.000 | 10 41.00 -660.00 -368.00 0 0 0.000 0.000 | 29 712.29 263.13 0.000 0.000 |  |  | 2 1 0 968 960.00 360.00
1770 04f0e5f4 | 10 11.00 0.00 -185.54 0 0 0.000 0.000 | 10 21.00 -660.00 -368.00 0 0 0.000 0.000 | 29 707.44 270.00 0.000 0.000 |  |  | 2 1 0 968 960.00 360.00
1800 04f0e5f4 | 10 41.00 0.00 -185.54 0 0 0.000 0.000 | 10 1.00 -660.00 -368.00 0 0 0.000 0.000 | 29 724.30 270.00 0.000 0.000 |  |  | 2 1 0 968 960.00 360.00
1830 a044bce7 | 10 21.00 0.00 -185.54 0 0 0.000 0.000 | 10 31.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 1 1626 1000 960.00 360.00
1860 a044bce7 | 10 1.00 0.00 -185.54 0 0 0.000 0.000 | 10 11.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 1 1 970 960.00 360.00
1861 1fb40a99 | 10 2.00 0.00 -185.54 0 0 0.000 0.000 | 10 12.00 -660.00 -368.00 0 0 0.000 0.000 | 30 1200.81 367.24 0.000 0.000 |  |  | 2 0 0 969 960.00 360.00
1862 1fb40a99 | 10 3.00 0.00 -185.54 0 0 0.000 0.000 | 10 13.00 -660.00 -368.00 0 0 0.000 0.000 | 30 1081.63 372.81 0.000 0.000 |  |  | 2 1 0 969 960.00 360.00
1890 1fb40a99 | 10 31.00 0.00 -185.54 0 0 0.000 0.000 | 10 41.00 -660.00 -368.00 0 0 0.000 0.000 | 30 655.49 270.00 0.000 0.000 |  |  | 2 1 0 969 960.00 360.00
1900 f8334b2d
1916 5410fa01
1920 5410fa01 | 47 1.00 0.00 -0.41 6 2 0.000 42.060 | 10 21.00 -660.00 -368.00 0 0 0.000 0.000 | 30 648.43 270.00 0.000 0.000 |  |  | 2 1 0 969 960.00 360.00
1932 d5038b15
1948 3605b269
1950 3605b269 | 47 1.00 0.00 373.52 12 6 0.000 43.320 | 10 1.00 -660.00 -368.00 0 0 0.000 0.000 | 30 746.80 270.00 0.000 0.000 |  |  | 2 1 0 969 960.00 360.00
1964 44aa63fd
1980 04b0d3d1 | 26 2.00 0.00 185.62 12 0 0.000 0.820 | 10 31.00 -660.00 -368.00 0 0 0.000 0.000 | 30 1321.41 270.00 0.000 0.000 | 31 -2045.76 270.00 -185.000 0.000 ; 31 676.41 270.00 -105.000 0.000 |  | 2 1 0 969 960.00 360.00
1988 88fff04a
2010 88fff04a | 47 21.00 0.00 -10.78 15 0 0.000 9.950 | 10 11.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 1 1 978 960.00 360.00
2011 31d875e5 | 47 22.00 0.00 -46.53 15 0 0.000 8.250 | 10 12.00 -660.00 -368.00 0 0 0.000 0.000 | 27 1294.99 270.00 0.000 0.000 |  |  | 2 0 0 977 960.00 360.00
2012 31d875e5 | 47 23.00 0.00 -83.98 15 0 0.000 6.550 | 10 13.00 -660.00 -368.00 0 0 0.000 0.000 | 27 1269.99 270.00 0.000 0.000 |  |  | 2 0 0 977 1099.99 360.00
2015 0914bb87
2040 0914bb87 | 10 18.00 0.00 -185.54 15 0 0.000 0.000 | 10 41.00 -660.00 -368.00 0 0 0.000 0.000 | 27 867.68 270.00 0.000 0.000 |  |  | 2 1 0 977 960.00 360.00
2070 0914bb87 | 10 48.00 0.00 -185.54 15 0 0.000 0.000 | 10 21.00 -660.00 -368.00 0 0 0.000 0.000 | 27 862.12 270.00 0.000 0.000 |  |  | 2 1 0 977 960.00 360.00
2100 0914bb87 | 10 28.00 0.00 -185.54 15 0 0.000 0.000 | 10 1.00 -660.00 -368.00 0 0 0.000 0.000 | 27 958.58 270.00 0.000 0.000 |  |  | 2 1 0 977 960.00 360.00
2130 0914bb87 | 10 8.00 0.00 -185.54 15 0 0.000 0.000 | 10 31.00 -660.00 -368.00 0 0 0.000 0.000 | 27 1114.11 270.00 0.000 0.000 |  |  | 2 1 0 977 960.00 360.00
2160 0914bb87 | 10 38.00 0.00 -185.54 15 0 0.000 0.000 | 10 11.00 -660.00 -368.00 0 0 0.000 0.000 | 27 1269.63 270.00 0.000 0.000 |  |  | 2 1 1 977 960.00 360.00
2170 994d482e
2190 994d482e | 10 18.00 0.00 -185.54 15 0 0.000 0.000 | 10 41.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 1 1578 980 960.00 360.00
2200 994d482e | 10 28.00 0.00 -185.54 15 0 0.000 0.000 | 10 1.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 1 1568 970 960.00 360.00
";

#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn saffrons_gate_picks_its_pokemon_as_on_the_n64() {
    let replay = Replay {
        p1: FighterKind::Mario,
        route: "999:1:",
        weapons: &monster_weapons,
    };
    let waits: Vec<(u64, u16)> = (60..=2160).step_by(150).map(|t| (t, 1)).collect();
    let Some(out) = replay.run_saffron_waits(SAFFRON_UNFORCED_PICKS, 0, &waits) else {
        return;
    };
    out.check("Saffron's gate picking its Pokémon, twice past the previous one");
}

/// `b30set/ys2` ([`SAFFRON_CHANSEY_COMES_OUT`]'s run with
/// `dITManagerForceMonsterKind` 2): Electrode comes out at 61 and explodes
/// in the gate (30 percent to P1 above it); the gate closes at 186. Rows
/// every 4 frames to the close.
const SAFFRON_ELECTRODE: &str = "
0 0029e2c0 | 10 1.00 1200.00 810.00 0 0 0.000 0.000 | 10 1.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 1000 1 960.00 360.00
4 0029e2c0 | 10 5.00 1200.00 810.00 0 0 0.000 0.000 | 10 5.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 996 0 1600.00 360.00
8 0029e2c0 | 10 9.00 1200.00 810.00 0 0 0.000 0.000 | 10 9.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 992 0 1600.00 360.00
12 0029e2c0 | 10 13.00 1200.00 810.00 0 0 0.000 0.000 | 10 13.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 988 0 1600.00 360.00
16 0029e2c0 | 10 17.00 1200.00 810.00 0 0 0.000 0.000 | 10 17.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 984 0 1600.00 360.00
20 0029e2c0 | 10 21.00 1200.00 810.00 0 0 0.000 0.000 | 10 21.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 980 0 1600.00 360.00
24 0029e2c0 | 10 25.00 1200.00 810.00 0 0 0.000 0.000 | 10 25.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 976 0 1600.00 360.00
28 0029e2c0 | 10 29.00 1200.00 810.00 0 0 0.000 0.000 | 10 29.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 972 0 1600.00 360.00
32 0029e2c0 | 10 33.00 1200.00 810.00 0 0 0.000 0.000 | 10 33.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 968 0 1600.00 360.00
36 0029e2c0 | 10 37.00 1200.00 810.00 0 0 0.000 0.000 | 10 37.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 964 0 1600.00 360.00
40 0029e2c0 | 10 41.00 1200.00 810.00 0 0 0.000 0.000 | 10 41.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 960 0 1600.00 360.00
44 0029e2c0 | 10 45.00 1200.00 810.00 0 0 0.000 0.000 | 10 45.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 956 0 1600.00 360.00
48 0029e2c0 | 10 49.00 1200.00 810.00 0 0 0.000 0.000 | 10 49.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 952 0 1600.00 360.00
52 0029e2c0 | 10 3.00 1200.00 810.00 0 0 0.000 0.000 | 10 3.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 948 0 1600.00 360.00
56 0029e2c0 | 10 7.00 1200.00 810.00 0 0 0.000 0.000 | 10 7.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 944 0 1600.00 360.00
60 0029e2c0 | 10 11.00 1200.00 810.00 0 0 0.000 0.000 | 10 11.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 1 0 1600.00 360.00
64 0029e2c0 | 10 15.00 1200.00 810.00 0 0 0.000 0.000 | 10 15.00 -660.00 -368.00 0 0 0.000 0.000 | 28 1242.08 498.74 0.000 0.000 |  |  | 2 0 0 0 1012.90 360.00
68 0029e2c0 | 10 19.00 1200.00 810.00 0 0 0.000 0.000 | 10 19.00 -660.00 -368.00 0 0 0.000 0.000 | 28 1171.09 584.83 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
72 0029e2c0 | 10 23.00 1200.00 810.00 0 0 0.000 0.000 | 10 23.00 -660.00 -368.00 0 0 0.000 0.000 | 28 1106.70 592.69 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
76 0029e2c0 | 10 27.00 1200.00 810.00 0 0 0.000 0.000 | 10 27.00 -660.00 -368.00 0 0 0.000 0.000 | 28 1048.61 584.33 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
80 0029e2c0 | 10 31.00 1200.00 810.00 0 0 0.000 0.000 | 10 31.00 -660.00 -368.00 0 0 0.000 0.000 | 28 996.48 548.97 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
84 0029e2c0 | 10 35.00 1200.00 810.00 0 0 0.000 0.000 | 10 35.00 -660.00 -368.00 0 0 0.000 0.000 | 28 950.00 467.17 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
88 0029e2c0 | 10 39.00 1200.00 810.00 0 0 0.000 0.000 | 10 39.00 -660.00 -368.00 0 0 0.000 0.000 | 28 908.85 319.49 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
92 0029e2c0 | 10 43.00 1200.00 810.00 0 0 0.000 0.000 | 10 43.00 -660.00 -368.00 0 0 0.000 0.000 | 28 872.71 359.71 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
96 0029e2c0 | 10 47.00 1200.00 810.00 0 0 0.000 0.000 | 10 47.00 -660.00 -368.00 0 0 0.000 0.000 | 28 841.26 420.89 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
100 0029e2c0 | 10 1.00 1200.00 810.00 0 0 0.000 0.000 | 10 1.00 -660.00 -368.00 0 0 0.000 0.000 | 28 814.18 436.22 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
104 0029e2c0 | 10 5.00 1200.00 810.00 0 0 0.000 0.000 | 10 5.00 -660.00 -368.00 0 0 0.000 0.000 | 28 791.15 433.08 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
108 0029e2c0 | 10 9.00 1200.00 810.00 0 0 0.000 0.000 | 10 9.00 -660.00 -368.00 0 0 0.000 0.000 | 28 771.85 420.16 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
112 0029e2c0 | 10 13.00 1200.00 810.00 0 0 0.000 0.000 | 10 13.00 -660.00 -368.00 0 0 0.000 0.000 | 28 755.97 392.18 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
116 0029e2c0 | 10 17.00 1200.00 810.00 0 0 0.000 0.000 | 10 17.00 -660.00 -368.00 0 0 0.000 0.000 | 28 743.18 343.88 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
120 0029e2c0 | 10 21.00 1200.00 810.00 0 0 0.000 0.000 | 10 21.00 -660.00 -368.00 0 0 0.000 0.000 | 28 733.16 270.00 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
124 0029e2c0 | 10 25.00 1200.00 810.00 0 0 0.000 0.000 | 10 25.00 -660.00 -368.00 0 0 0.000 0.000 | 28 725.60 319.96 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
128 0029e2c0 | 10 29.00 1200.00 810.00 0 0 0.000 0.000 | 10 29.00 -660.00 -368.00 0 0 0.000 0.000 | 28 720.17 348.93 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
132 0029e2c0 | 10 33.00 1200.00 810.00 0 0 0.000 0.000 | 10 33.00 -660.00 -368.00 0 0 0.000 0.000 | 28 729.33 355.52 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
136 0029e2c0 | 10 37.00 1200.00 810.00 0 0 0.000 0.000 | 10 37.00 -660.00 -368.00 0 0 0.000 0.000 | 28 730.41 351.35 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
140 0029e2c0 | 10 41.00 1200.00 810.00 0 0 0.000 0.000 | 10 41.00 -660.00 -368.00 0 0 0.000 0.000 | 28 734.72 340.16 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
144 0029e2c0 | 10 45.00 1200.00 810.00 0 0 0.000 0.000 | 10 45.00 -660.00 -368.00 0 0 0.000 0.000 | 28 736.87 320.21 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
148 0029e2c0 | 10 49.00 1200.00 810.00 0 0 0.000 0.000 | 10 49.00 -660.00 -368.00 0 0 0.000 0.000 | 28 742.26 289.73 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
152 0029e2c0 | 10 3.00 1200.00 810.00 0 0 0.000 0.000 | 10 3.00 -660.00 -368.00 0 0 0.000 0.000 | 28 745.49 282.16 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
156 0029e2c0 | 10 7.00 1200.00 810.00 0 0 0.000 0.000 | 10 7.00 -660.00 -368.00 0 0 0.000 0.000 | 28 748.03 298.95 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
160 0029e2c0 | 10 11.00 1200.00 810.00 0 0 0.000 0.000 | 10 11.00 -660.00 -368.00 0 0 0.000 0.000 | 28 756.05 289.72 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
164 0029e2c0 | 10 15.00 1200.00 810.00 0 0 0.000 0.000 | 10 15.00 -660.00 -368.00 0 0 0.000 0.000 | 28 761.06 286.76 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
168 0029e2c0 | 10 19.00 1200.00 810.00 0 0 0.000 0.000 | 10 19.00 -660.00 -368.00 0 0 0.000 0.000 | 28 763.07 282.32 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
172 0029e2c0 | 10 23.00 1200.00 810.00 0 0 0.000 0.000 | 10 23.00 -660.00 -368.00 0 0 0.000 0.000 | 28 761.06 284.54 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
176 0029e2c0 | 10 27.00 1200.00 810.00 0 0 0.000 0.000 | 10 27.00 -660.00 -368.00 0 0 0.000 0.000 | 28 761.06 286.76 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
180 2cd60a1e | 52 1.00 1200.00 810.00 30 15 92.307 84.584 | 10 31.00 -660.00 -368.00 0 0 0.000 0.000 | 28 736.06 270.00 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
181 daf1b4cc
182 0bb31550
184 fe87df14 | 52 1.00 1200.00 810.00 30 11 92.307 84.584 | 10 35.00 -660.00 -368.00 0 0 0.000 0.000 | 28 736.06 270.00 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
186 305e1c7b | 52 1.00 1200.00 810.00 30 9 92.307 84.584 | 10 37.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 1 1188 1000 960.00 360.00
";

#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn saffrons_electrode_explodes_as_on_the_n64() {
    let replay = Replay {
        p1: FighterKind::Mario,
        route: "999:1:",
        weapons: &monster_weapons,
    };
    let Some(out) = replay.run_saffron(SAFFRON_ELECTRODE, 2, (60, 1)) else {
        return;
    };
    out.check("Saffron's gate and a forced Electrode");
}

/// `b30set/ys3` ([`SAFFRON_CHANSEY_COMES_OUT`]'s run with
/// `dITManagerForceMonsterKind` 3): Charmander comes out at 61 and breathes his flames;
/// the gate closes at 180. Rows every 4 frames to 200.
const SAFFRON_CHARMANDER: &str = "
0 0029e2c0 | 10 1.00 1200.00 810.00 0 0 0.000 0.000 | 10 1.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 1000 1 960.00 360.00
4 0029e2c0 | 10 5.00 1200.00 810.00 0 0 0.000 0.000 | 10 5.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 996 0 1600.00 360.00
8 0029e2c0 | 10 9.00 1200.00 810.00 0 0 0.000 0.000 | 10 9.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 992 0 1600.00 360.00
12 0029e2c0 | 10 13.00 1200.00 810.00 0 0 0.000 0.000 | 10 13.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 988 0 1600.00 360.00
16 0029e2c0 | 10 17.00 1200.00 810.00 0 0 0.000 0.000 | 10 17.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 984 0 1600.00 360.00
20 0029e2c0 | 10 21.00 1200.00 810.00 0 0 0.000 0.000 | 10 21.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 980 0 1600.00 360.00
24 0029e2c0 | 10 25.00 1200.00 810.00 0 0 0.000 0.000 | 10 25.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 976 0 1600.00 360.00
28 0029e2c0 | 10 29.00 1200.00 810.00 0 0 0.000 0.000 | 10 29.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 972 0 1600.00 360.00
32 0029e2c0 | 10 33.00 1200.00 810.00 0 0 0.000 0.000 | 10 33.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 968 0 1600.00 360.00
36 0029e2c0 | 10 37.00 1200.00 810.00 0 0 0.000 0.000 | 10 37.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 964 0 1600.00 360.00
40 0029e2c0 | 10 41.00 1200.00 810.00 0 0 0.000 0.000 | 10 41.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 960 0 1600.00 360.00
44 0029e2c0 | 10 45.00 1200.00 810.00 0 0 0.000 0.000 | 10 45.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 956 0 1600.00 360.00
48 0029e2c0 | 10 49.00 1200.00 810.00 0 0 0.000 0.000 | 10 49.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 952 0 1600.00 360.00
52 0029e2c0 | 10 3.00 1200.00 810.00 0 0 0.000 0.000 | 10 3.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 948 0 1600.00 360.00
56 0029e2c0 | 10 7.00 1200.00 810.00 0 0 0.000 0.000 | 10 7.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 944 0 1600.00 360.00
60 0029e2c0 | 10 11.00 1200.00 810.00 0 0 0.000 0.000 | 10 11.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 1 0 1600.00 360.00
61 c823f683
64 c823f683 | 10 15.00 1200.00 810.00 0 0 0.000 0.000 | 10 15.00 -660.00 -368.00 0 0 0.000 0.000 | 29 1022.58 282.55 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
68 c823f683 | 10 19.00 1200.00 810.00 0 0 0.000 0.000 | 10 19.00 -660.00 -368.00 0 0 0.000 0.000 | 29 831.37 294.55 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
72 c823f683 | 10 23.00 1200.00 810.00 0 0 0.000 0.000 | 10 23.00 -660.00 -368.00 0 0 0.000 0.000 | 29 737.81 302.76 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
76 c823f683 | 10 27.00 1200.00 810.00 0 0 0.000 0.000 | 10 27.00 -660.00 -368.00 0 0 0.000 0.000 | 29 722.98 304.02 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
80 c823f683 | 10 31.00 1200.00 810.00 0 0 0.000 0.000 | 10 31.00 -660.00 -368.00 0 0 0.000 0.000 | 29 719.34 297.78 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
84 c823f683 | 10 35.00 1200.00 810.00 0 0 0.000 0.000 | 10 35.00 -660.00 -368.00 0 0 0.000 0.000 | 29 716.18 286.14 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
88 c823f683 | 10 39.00 1200.00 810.00 0 0 0.000 0.000 | 10 39.00 -660.00 -368.00 0 0 0.000 0.000 | 29 713.47 271.19 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
92 c823f683 | 10 43.00 1200.00 810.00 0 0 0.000 0.000 | 10 43.00 -660.00 -368.00 0 0 0.000 0.000 | 29 711.22 272.24 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
96 c823f683 | 10 47.00 1200.00 810.00 0 0 0.000 0.000 | 10 47.00 -660.00 -368.00 0 0 0.000 0.000 | 29 709.41 287.39 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
100 cae1df84 | 10 1.00 1200.00 810.00 0 0 0.000 0.000 | 10 1.00 -660.00 -368.00 0 0 0.000 0.000 | 29 708.04 297.91 0.000 0.000 | 30 414.02 288.55 -44.017 -9.356 |  | 2 1 0 0 960.00 360.00
104 cae1df84 | 10 5.00 1200.00 810.00 0 0 0.000 0.000 | 10 5.00 -660.00 -368.00 0 0 0.000 0.000 | 29 707.10 303.16 0.000 0.000 | 30 237.96 251.13 -44.017 -9.356 |  | 2 1 0 0 960.00 360.00
108 cae1df84 | 10 9.00 1200.00 810.00 0 0 0.000 0.000 | 10 9.00 -660.00 -368.00 0 0 0.000 0.000 | 29 706.57 302.55 0.000 0.000 | 30 61.89 213.70 -44.017 -9.356 |  | 2 1 0 0 960.00 360.00
109 72ae7869
112 72ae7869 | 10 13.00 1200.00 810.00 0 0 0.000 0.000 | 10 13.00 -660.00 -368.00 0 0 0.000 0.000 | 29 706.46 296.28 0.000 0.000 | 30 -114.18 176.28 -44.017 -9.356 ; 30 280.44 264.06 -44.017 -9.356 |  | 2 1 0 0 960.00 360.00
116 72ae7869 | 10 17.00 1200.00 810.00 0 0 0.000 0.000 | 10 17.00 -660.00 -368.00 0 0 0.000 0.000 | 29 706.75 285.16 0.000 0.000 | 30 -290.24 138.85 -44.017 -9.356 ; 30 104.37 226.64 -44.017 -9.356 |  | 2 1 0 0 960.00 360.00
118 1649b442
120 1649b442 | 10 21.00 1200.00 810.00 0 0 0.000 0.000 | 10 21.00 -660.00 -368.00 0 0 0.000 0.000 | 29 707.44 270.00 0.000 0.000 | 30 -71.69 189.21 -44.017 -9.356 ; 30 324.99 249.96 -44.017 -9.356 |  | 2 1 0 0 960.00 360.00
124 1649b442 | 10 25.00 1200.00 810.00 0 0 0.000 0.000 | 10 25.00 -660.00 -368.00 0 0 0.000 0.000 | 29 708.51 281.92 0.000 0.000 | 30 -247.76 151.79 -44.017 -9.356 ; 30 148.93 212.54 -44.017 -9.356 |  | 2 1 0 0 960.00 360.00
127 c1bbb05f
128 c1bbb05f | 10 29.00 1200.00 810.00 0 0 0.000 0.000 | 10 29.00 -660.00 -368.00 0 0 0.000 0.000 | 29 709.95 293.35 0.000 0.000 | 30 -27.14 175.11 -44.017 -9.356 ; 30 371.52 271.98 -44.017 -9.356 |  | 2 1 0 0 960.00 360.00
132 c1bbb05f | 10 33.00 1200.00 810.00 0 0 0.000 0.000 | 10 33.00 -660.00 -368.00 0 0 0.000 0.000 | 29 711.77 301.43 0.000 0.000 | 30 -203.21 137.69 -44.017 -9.356 ; 30 195.46 234.55 -44.017 -9.356 |  | 2 1 0 0 960.00 360.00
136 0bb31550 | 10 37.00 1200.00 810.00 0 0 0.000 0.000 | 10 37.00 -660.00 -368.00 0 0 0.000 0.000 | 29 713.95 303.37 0.000 0.000 | 30 -379.27 100.27 -44.017 -9.356 ; 30 19.39 197.13 -44.017 -9.356 ; 30 419.93 294.02 -44.017 -9.356 |  | 2 1 0 0 960.00 360.00
140 0bb31550 | 10 41.00 1200.00 810.00 0 0 0.000 0.000 | 10 41.00 -660.00 -368.00 0 0 0.000 0.000 | 29 716.48 298.85 0.000 0.000 | 30 -156.68 159.71 -44.017 -9.356 ; 30 243.87 256.59 -44.017 -9.356 |  | 2 1 0 0 960.00 360.00
144 0bb31550 | 10 45.00 1200.00 810.00 0 0 0.000 0.000 | 10 45.00 -660.00 -368.00 0 0 0.000 0.000 | 29 719.36 289.55 0.000 0.000 | 30 -332.74 122.28 -44.017 -9.356 ; 30 67.80 219.17 -44.017 -9.356 |  | 2 1 0 0 960.00 360.00
145 953cdbe5
148 953cdbe5 | 10 49.00 1200.00 810.00 0 0 0.000 0.000 | 10 49.00 -660.00 -368.00 0 0 0.000 0.000 | 29 722.57 277.01 0.000 0.000 | 30 -108.27 181.74 -44.017 -9.356 ; 30 294.06 249.23 -44.017 -9.356 |  | 2 1 0 0 960.00 360.00
152 953cdbe5 | 10 3.00 1200.00 810.00 0 0 0.000 0.000 | 10 3.00 -660.00 -368.00 0 0 0.000 0.000 | 29 726.11 277.22 0.000 0.000 | 30 -284.33 144.32 -44.017 -9.356 ; 30 118.00 211.81 -44.017 -9.356 |  | 2 1 0 0 960.00 360.00
154 0f3e862e
156 0f3e862e | 10 7.00 1200.00 810.00 0 0 0.000 0.000 | 10 7.00 -660.00 -368.00 0 0 0.000 0.000 | 29 729.97 289.83 0.000 0.000 | 30 -58.07 174.38 -44.017 -9.356 ; 30 345.95 255.81 -44.017 -9.356 |  | 2 1 0 0 960.00 360.00
160 0f3e862e | 10 11.00 1200.00 810.00 0 0 0.000 0.000 | 10 11.00 -660.00 -368.00 0 0 0.000 0.000 | 29 734.14 298.97 0.000 0.000 | 30 -234.14 136.96 -44.017 -9.356 ; 30 169.88 218.39 -44.017 -9.356 |  | 2 1 0 0 960.00 360.00
163 305e1c7b
164 305e1c7b | 10 15.00 1200.00 810.00 0 0 0.000 0.000 | 10 15.00 -660.00 -368.00 0 0 0.000 0.000 | 29 943.18 303.37 0.000 0.000 | 30 -6.18 180.96 -44.017 -9.356 ; 30 557.10 284.07 -44.017 -9.356 |  | 2 1 0 0 960.00 360.00
168 305e1c7b | 10 19.00 1200.00 810.00 0 0 0.000 0.000 | 10 19.00 -660.00 -368.00 0 0 0.000 0.000 | 29 1106.99 301.94 0.000 0.000 | 30 -182.25 143.54 -44.017 -9.356 ; 30 381.03 246.65 -44.017 -9.356 |  | 2 1 0 0 960.00 360.00
172 4d067e5c | 10 23.00 1200.00 810.00 0 0 0.000 0.000 | 10 23.00 -660.00 -368.00 0 0 0.000 0.000 | 29 1224.86 295.14 0.000 0.000 | 30 -358.32 106.12 -44.017 -9.356 ; 30 204.96 209.22 -44.017 -9.356 |  | 2 1 0 0 960.00 360.00
176 4d067e5c | 10 27.00 1200.00 810.00 0 0 0.000 0.000 | 10 27.00 -660.00 -368.00 0 0 0.000 0.000 | 29 1296.10 284.23 0.000 0.000 | 30 28.90 171.80 -44.017 -9.356 |  | 2 1 0 0 960.00 360.00
180 4db793af | 10 31.00 1200.00 810.00 0 0 0.000 0.000 | 10 31.00 -660.00 -368.00 0 0 0.000 0.000 |  | 30 -147.17 134.37 -44.017 -9.356 |  | 1 1 1303 1000 960.00 360.00
184 4db793af | 10 35.00 1200.00 810.00 0 0 0.000 0.000 | 10 35.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 1 1299 996 960.00 360.00
188 4db793af | 10 39.00 1200.00 810.00 0 0 0.000 0.000 | 10 39.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 1 1295 992 960.00 360.00
192 4db793af | 10 43.00 1200.00 810.00 0 0 0.000 0.000 | 10 43.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 1 1291 988 960.00 360.00
196 4db793af | 10 47.00 1200.00 810.00 0 0 0.000 0.000 | 10 47.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 1 1287 984 960.00 360.00
200 4db793af | 10 1.00 1200.00 810.00 0 0 0.000 0.000 | 10 1.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 1 1283 980 960.00 360.00
";

#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn saffrons_charmander_breathes_fire_as_on_the_n64() {
    let replay = Replay {
        p1: FighterKind::Mario,
        route: "999:1:",
        weapons: &monster_weapons,
    };
    let Some(out) = replay.run_saffron(SAFFRON_CHARMANDER, 3, (60, 1)) else {
        return;
    };
    out.check("Saffron's gate and a forced Charmander");
}

/// `b30set/ys4` ([`SAFFRON_CHANSEY_COMES_OUT`]'s run with
/// `dITManagerForceMonsterKind` 4): Venusaur comes out at 61 and shoots his razor leaves;
/// the gate closes at 188. Rows every 4 frames to 200.
const SAFFRON_VENUSAUR: &str = "
0 0029e2c0 | 10 1.00 1200.00 810.00 0 0 0.000 0.000 | 10 1.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 1000 1 960.00 360.00
4 0029e2c0 | 10 5.00 1200.00 810.00 0 0 0.000 0.000 | 10 5.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 996 0 1600.00 360.00
8 0029e2c0 | 10 9.00 1200.00 810.00 0 0 0.000 0.000 | 10 9.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 992 0 1600.00 360.00
12 0029e2c0 | 10 13.00 1200.00 810.00 0 0 0.000 0.000 | 10 13.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 988 0 1600.00 360.00
16 0029e2c0 | 10 17.00 1200.00 810.00 0 0 0.000 0.000 | 10 17.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 984 0 1600.00 360.00
20 0029e2c0 | 10 21.00 1200.00 810.00 0 0 0.000 0.000 | 10 21.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 980 0 1600.00 360.00
24 0029e2c0 | 10 25.00 1200.00 810.00 0 0 0.000 0.000 | 10 25.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 976 0 1600.00 360.00
28 0029e2c0 | 10 29.00 1200.00 810.00 0 0 0.000 0.000 | 10 29.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 972 0 1600.00 360.00
32 0029e2c0 | 10 33.00 1200.00 810.00 0 0 0.000 0.000 | 10 33.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 968 0 1600.00 360.00
36 0029e2c0 | 10 37.00 1200.00 810.00 0 0 0.000 0.000 | 10 37.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 964 0 1600.00 360.00
40 0029e2c0 | 10 41.00 1200.00 810.00 0 0 0.000 0.000 | 10 41.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 960 0 1600.00 360.00
44 0029e2c0 | 10 45.00 1200.00 810.00 0 0 0.000 0.000 | 10 45.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 956 0 1600.00 360.00
48 0029e2c0 | 10 49.00 1200.00 810.00 0 0 0.000 0.000 | 10 49.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 952 0 1600.00 360.00
52 0029e2c0 | 10 3.00 1200.00 810.00 0 0 0.000 0.000 | 10 3.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 948 0 1600.00 360.00
56 0029e2c0 | 10 7.00 1200.00 810.00 0 0 0.000 0.000 | 10 7.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 944 0 1600.00 360.00
60 0029e2c0 | 10 11.00 1200.00 810.00 0 0 0.000 0.000 | 10 11.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 1 0 1600.00 360.00
61 c823f683
64 c823f683 | 10 15.00 1200.00 810.00 0 0 0.000 0.000 | 10 15.00 -660.00 -368.00 0 0 0.000 0.000 | 30 843.26 381.22 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
68 c823f683 | 10 19.00 1200.00 810.00 0 0 0.000 0.000 | 10 19.00 -660.00 -368.00 0 0 0.000 0.000 | 30 703.08 385.03 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
72 c823f683 | 10 23.00 1200.00 810.00 0 0 0.000 0.000 | 10 23.00 -660.00 -368.00 0 0 0.000 0.000 | 30 675.09 367.38 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
76 c823f683 | 10 27.00 1200.00 810.00 0 0 0.000 0.000 | 10 27.00 -660.00 -368.00 0 0 0.000 0.000 | 30 647.09 323.33 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
80 c823f683 | 10 31.00 1200.00 810.00 0 0 0.000 0.000 | 10 31.00 -660.00 -368.00 0 0 0.000 0.000 | 30 619.10 270.00 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
84 c823f683 | 10 35.00 1200.00 810.00 0 0 0.000 0.000 | 10 35.00 -660.00 -368.00 0 0 0.000 0.000 | 30 625.59 270.00 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
88 c823f683 | 10 39.00 1200.00 810.00 0 0 0.000 0.000 | 10 39.00 -660.00 -368.00 0 0 0.000 0.000 | 30 643.87 270.00 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
92 c823f683 | 10 43.00 1200.00 810.00 0 0 0.000 0.000 | 10 43.00 -660.00 -368.00 0 0 0.000 0.000 | 30 667.73 270.00 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
96 c823f683 | 10 47.00 1200.00 810.00 0 0 0.000 0.000 | 10 47.00 -660.00 -368.00 0 0 0.000 0.000 | 30 690.99 270.00 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
100 3d6c1037 | 10 1.00 1200.00 810.00 0 0 0.000 0.000 | 10 1.00 -660.00 -368.00 0 0 0.000 0.000 | 30 707.45 270.00 0.000 0.000 | 31 62.45 270.00 -105.000 0.000 |  | 2 1 0 0 960.00 360.00
104 3d6c1037 | 10 5.00 1200.00 810.00 0 0 0.000 0.000 | 10 5.00 -660.00 -368.00 0 0 0.000 0.000 | 30 711.43 270.00 0.000 0.000 | 31 -407.55 270.00 -125.000 0.000 |  | 2 1 0 0 960.00 360.00
108 3d6c1037 | 10 9.00 1200.00 810.00 0 0 0.000 0.000 | 10 9.00 -660.00 -368.00 0 0 0.000 0.000 | 30 703.57 270.00 0.000 0.000 | 31 -957.55 270.00 -145.000 0.000 |  | 2 1 0 0 960.00 360.00
112 3d6c1037 | 10 13.00 1200.00 810.00 0 0 0.000 0.000 | 10 13.00 -660.00 -368.00 0 0 0.000 0.000 | 30 687.85 270.00 0.000 0.000 | 31 -1587.55 270.00 -165.000 0.000 |  | 2 1 0 0 960.00 360.00
116 df90722b | 10 17.00 1200.00 810.00 0 0 0.000 0.000 | 10 17.00 -660.00 -368.00 0 0 0.000 0.000 | 30 668.17 270.00 0.000 0.000 | 31 -2297.55 270.00 -185.000 0.000 ; 31 23.17 270.00 -105.000 0.000 |  | 2 1 0 0 960.00 360.00
120 df90722b | 10 21.00 1200.00 810.00 0 0 0.000 0.000 | 10 21.00 -660.00 -368.00 0 0 0.000 0.000 | 30 648.43 270.00 0.000 0.000 | 31 -3087.55 270.00 -205.000 0.000 ; 31 -446.83 270.00 -125.000 0.000 |  | 2 1 0 0 960.00 360.00
124 df90722b | 10 25.00 1200.00 810.00 0 0 0.000 0.000 | 10 25.00 -660.00 -368.00 0 0 0.000 0.000 | 30 632.52 270.00 0.000 0.000 | 31 -996.83 270.00 -145.000 0.000 |  | 2 1 0 0 960.00 360.00
128 df90722b | 10 29.00 1200.00 810.00 0 0 0.000 0.000 | 10 29.00 -660.00 -368.00 0 0 0.000 0.000 | 30 624.33 270.00 0.000 0.000 | 31 -1626.83 270.00 -165.000 0.000 |  | 2 1 0 0 960.00 360.00
132 c1bbb05f | 10 33.00 1200.00 810.00 0 0 0.000 0.000 | 10 33.00 -660.00 -368.00 0 0 0.000 0.000 | 30 626.51 270.00 0.000 0.000 | 31 -2336.83 270.00 -185.000 0.000 ; 31 -18.49 270.00 -105.000 0.000 |  | 2 1 0 0 960.00 360.00
136 c1bbb05f | 10 37.00 1200.00 810.00 0 0 0.000 0.000 | 10 37.00 -660.00 -368.00 0 0 0.000 0.000 | 30 637.84 270.00 0.000 0.000 | 31 -3126.83 270.00 -205.000 0.000 ; 31 -488.49 270.00 -125.000 0.000 |  | 2 1 0 0 960.00 360.00
140 c1bbb05f | 10 41.00 1200.00 810.00 0 0 0.000 0.000 | 10 41.00 -660.00 -368.00 0 0 0.000 0.000 | 30 658.08 270.00 0.000 0.000 | 31 -1038.49 270.00 -145.000 0.000 |  | 2 1 0 0 960.00 360.00
144 c1bbb05f | 10 45.00 1200.00 810.00 0 0 0.000 0.000 | 10 45.00 -660.00 -368.00 0 0 0.000 0.000 | 30 687.09 270.00 0.000 0.000 | 31 -1668.49 270.00 -165.000 0.000 |  | 2 1 0 0 960.00 360.00
148 aea69ed3 | 10 49.00 1200.00 810.00 0 0 0.000 0.000 | 10 49.00 -660.00 -368.00 0 0 0.000 0.000 | 30 724.76 270.00 0.000 0.000 | 31 -2378.49 270.00 -185.000 0.000 ; 31 79.76 270.00 -105.000 0.000 |  | 2 1 0 0 960.00 360.00
152 aea69ed3 | 10 3.00 1200.00 810.00 0 0 0.000 0.000 | 10 3.00 -660.00 -368.00 0 0 0.000 0.000 | 30 770.94 270.00 0.000 0.000 | 31 -3168.49 270.00 -205.000 0.000 ; 31 -390.24 270.00 -125.000 0.000 |  | 2 1 0 0 960.00 360.00
156 aea69ed3 | 10 7.00 1200.00 810.00 0 0 0.000 0.000 | 10 7.00 -660.00 -368.00 0 0 0.000 0.000 | 30 825.51 270.00 0.000 0.000 | 31 -940.24 270.00 -145.000 0.000 |  | 2 1 0 0 960.00 360.00
160 aea69ed3 | 10 11.00 1200.00 810.00 0 0 0.000 0.000 | 10 11.00 -660.00 -368.00 0 0 0.000 0.000 | 30 888.32 270.00 0.000 0.000 | 31 -1570.24 270.00 -165.000 0.000 |  | 2 1 0 0 960.00 360.00
164 b90c5187 | 10 15.00 1200.00 810.00 0 0 0.000 0.000 | 10 15.00 -660.00 -368.00 0 0 0.000 0.000 | 30 959.24 270.00 0.000 0.000 | 31 -2280.24 270.00 -185.000 0.000 ; 31 314.24 270.00 -105.000 0.000 |  | 2 1 0 0 960.00 360.00
168 b90c5187 | 10 19.00 1200.00 810.00 0 0 0.000 0.000 | 10 19.00 -660.00 -368.00 0 0 0.000 0.000 | 30 1038.14 270.00 0.000 0.000 | 31 -3070.24 270.00 -205.000 0.000 ; 31 -155.76 270.00 -125.000 0.000 |  | 2 1 0 0 960.00 360.00
172 b90c5187 | 10 23.00 1200.00 810.00 0 0 0.000 0.000 | 10 23.00 -660.00 -368.00 0 0 0.000 0.000 | 30 1124.90 270.00 0.000 0.000 | 31 -705.76 270.00 -145.000 0.000 |  | 2 1 0 0 960.00 360.00
176 b90c5187 | 10 27.00 1200.00 810.00 0 0 0.000 0.000 | 10 27.00 -660.00 -368.00 0 0 0.000 0.000 | 30 1219.36 270.00 0.000 0.000 | 31 -1335.76 270.00 -165.000 0.000 |  | 2 1 0 0 960.00 360.00
180 305e1c7b | 10 31.00 1200.00 810.00 0 0 0.000 0.000 | 10 31.00 -660.00 -368.00 0 0 0.000 0.000 | 30 1321.41 270.00 0.000 0.000 | 31 -2045.76 270.00 -185.000 0.000 ; 31 676.41 270.00 -105.000 0.000 |  | 2 1 0 0 960.00 360.00
184 305e1c7b | 10 35.00 1200.00 810.00 0 0 0.000 0.000 | 10 35.00 -660.00 -368.00 0 0 0.000 0.000 | 30 1321.41 270.00 0.000 0.000 | 31 -2835.76 270.00 -205.000 0.000 ; 31 206.41 270.00 -125.000 0.000 |  | 2 1 0 0 960.00 360.00
188 4d067e5c | 10 39.00 1200.00 810.00 0 0 0.000 0.000 | 10 39.00 -660.00 -368.00 0 0 0.000 0.000 |  | 31 -343.59 270.00 -145.000 0.000 |  | 1 1 1300 1000 960.00 360.00
192 4d067e5c | 10 43.00 1200.00 810.00 0 0 0.000 0.000 | 10 43.00 -660.00 -368.00 0 0 0.000 0.000 |  | 31 -973.59 270.00 -165.000 0.000 |  | 1 1 1296 996 960.00 360.00
196 4d067e5c | 10 47.00 1200.00 810.00 0 0 0.000 0.000 | 10 47.00 -660.00 -368.00 0 0 0.000 0.000 |  | 31 -1683.59 270.00 -185.000 0.000 |  | 1 1 1292 992 960.00 360.00
200 4d067e5c | 10 1.00 1200.00 810.00 0 0 0.000 0.000 | 10 1.00 -660.00 -368.00 0 0 0.000 0.000 |  | 31 -2473.59 270.00 -205.000 0.000 |  | 1 1 1288 988 960.00 360.00
";

#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn saffrons_venusaur_shoots_razor_leaves_as_on_the_n64() {
    let replay = Replay {
        p1: FighterKind::Mario,
        route: "999:1:",
        weapons: &monster_weapons,
    };
    let Some(out) = replay.run_saffron(SAFFRON_VENUSAUR, 4, (60, 1)) else {
        return;
    };
    out.check("Saffron's gate and a forced Venusaur");
}

/// `b30set/ys5` ([`SAFFRON_CHANSEY_COMES_OUT`]'s run with
/// `dITManagerForceMonsterKind` 5): Porygon comes out at 61 and tackles out of the gate;
/// the gate closes at 179. Rows every 4 frames to 200.
const SAFFRON_PORYGON: &str = "
0 0029e2c0 | 10 1.00 1200.00 810.00 0 0 0.000 0.000 | 10 1.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 1000 1 960.00 360.00
4 0029e2c0 | 10 5.00 1200.00 810.00 0 0 0.000 0.000 | 10 5.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 996 0 1600.00 360.00
8 0029e2c0 | 10 9.00 1200.00 810.00 0 0 0.000 0.000 | 10 9.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 992 0 1600.00 360.00
12 0029e2c0 | 10 13.00 1200.00 810.00 0 0 0.000 0.000 | 10 13.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 988 0 1600.00 360.00
16 0029e2c0 | 10 17.00 1200.00 810.00 0 0 0.000 0.000 | 10 17.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 984 0 1600.00 360.00
20 0029e2c0 | 10 21.00 1200.00 810.00 0 0 0.000 0.000 | 10 21.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 980 0 1600.00 360.00
24 0029e2c0 | 10 25.00 1200.00 810.00 0 0 0.000 0.000 | 10 25.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 976 0 1600.00 360.00
28 0029e2c0 | 10 29.00 1200.00 810.00 0 0 0.000 0.000 | 10 29.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 972 0 1600.00 360.00
32 0029e2c0 | 10 33.00 1200.00 810.00 0 0 0.000 0.000 | 10 33.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 968 0 1600.00 360.00
36 0029e2c0 | 10 37.00 1200.00 810.00 0 0 0.000 0.000 | 10 37.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 964 0 1600.00 360.00
40 0029e2c0 | 10 41.00 1200.00 810.00 0 0 0.000 0.000 | 10 41.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 960 0 1600.00 360.00
44 0029e2c0 | 10 45.00 1200.00 810.00 0 0 0.000 0.000 | 10 45.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 956 0 1600.00 360.00
48 0029e2c0 | 10 49.00 1200.00 810.00 0 0 0.000 0.000 | 10 49.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 952 0 1600.00 360.00
52 0029e2c0 | 10 3.00 1200.00 810.00 0 0 0.000 0.000 | 10 3.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 948 0 1600.00 360.00
56 0029e2c0 | 10 7.00 1200.00 810.00 0 0 0.000 0.000 | 10 7.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 944 0 1600.00 360.00
60 0029e2c0 | 10 11.00 1200.00 810.00 0 0 0.000 0.000 | 10 11.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 0 1 0 1600.00 360.00
64 0029e2c0 | 10 15.00 1200.00 810.00 0 0 0.000 0.000 | 10 15.00 -660.00 -368.00 0 0 0.000 0.000 | 31 990.27 270.48 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
68 0029e2c0 | 10 19.00 1200.00 810.00 0 0 0.000 0.000 | 10 19.00 -660.00 -368.00 0 0 0.000 0.000 | 31 660.54 290.26 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
72 0029e2c0 | 10 23.00 1200.00 810.00 0 0 0.000 0.000 | 10 23.00 -660.00 -368.00 0 0 0.000 0.000 | 31 660.50 273.82 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
76 0029e2c0 | 10 27.00 1200.00 810.00 0 0 0.000 0.000 | 10 27.00 -660.00 -368.00 0 0 0.000 0.000 | 31 660.46 271.03 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
80 0029e2c0 | 10 31.00 1200.00 810.00 0 0 0.000 0.000 | 10 31.00 -660.00 -368.00 0 0 0.000 0.000 | 31 660.42 269.18 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
84 0029e2c0 | 10 35.00 1200.00 810.00 0 0 0.000 0.000 | 10 35.00 -660.00 -368.00 0 0 0.000 0.000 | 31 660.37 271.03 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
88 0029e2c0 | 10 39.00 1200.00 810.00 0 0 0.000 0.000 | 10 39.00 -660.00 -368.00 0 0 0.000 0.000 | 31 660.33 270.41 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
92 18be873a | 10 43.00 1200.00 810.00 0 0 0.000 0.000 | 10 43.00 -660.00 -368.00 0 0 0.000 0.000 | 31 660.29 270.10 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
96 18be873a | 10 47.00 1200.00 810.00 0 0 0.000 0.000 | 10 47.00 -660.00 -368.00 0 0 0.000 0.000 | 31 660.25 270.10 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
100 18be873a | 10 1.00 1200.00 810.00 0 0 0.000 0.000 | 10 1.00 -660.00 -368.00 0 0 0.000 0.000 | 31 660.21 270.09 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
104 18be873a | 10 5.00 1200.00 810.00 0 0 0.000 0.000 | 10 5.00 -660.00 -368.00 0 0 0.000 0.000 | 31 660.17 270.09 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
108 18be873a | 10 9.00 1200.00 810.00 0 0 0.000 0.000 | 10 9.00 -660.00 -368.00 0 0 0.000 0.000 | 31 660.13 270.08 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
112 18be873a | 10 13.00 1200.00 810.00 0 0 0.000 0.000 | 10 13.00 -660.00 -368.00 0 0 0.000 0.000 | 31 660.09 270.08 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
116 18be873a | 10 17.00 1200.00 810.00 0 0 0.000 0.000 | 10 17.00 -660.00 -368.00 0 0 0.000 0.000 | 31 660.05 270.08 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
120 18be873a | 10 21.00 1200.00 810.00 0 0 0.000 0.000 | 10 21.00 -660.00 -368.00 0 0 0.000 0.000 | 31 660.01 270.07 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
124 18be873a | 10 25.00 1200.00 810.00 0 0 0.000 0.000 | 10 25.00 -660.00 -368.00 0 0 0.000 0.000 | 31 694.14 270.07 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
128 18be873a | 10 29.00 1200.00 810.00 0 0 0.000 0.000 | 10 29.00 -660.00 -368.00 0 0 0.000 0.000 | 31 739.66 270.06 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
132 18be873a | 10 33.00 1200.00 810.00 0 0 0.000 0.000 | 10 33.00 -660.00 -368.00 0 0 0.000 0.000 | 31 785.17 270.06 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
136 18be873a | 10 37.00 1200.00 810.00 0 0 0.000 0.000 | 10 37.00 -660.00 -368.00 0 0 0.000 0.000 | 31 830.69 270.05 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
140 18be873a | 10 41.00 1200.00 810.00 0 0 0.000 0.000 | 10 41.00 -660.00 -368.00 0 0 0.000 0.000 | 31 876.21 270.05 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
144 18be873a | 10 45.00 1200.00 810.00 0 0 0.000 0.000 | 10 45.00 -660.00 -368.00 0 0 0.000 0.000 | 31 921.72 270.04 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
148 18be873a | 10 49.00 1200.00 810.00 0 0 0.000 0.000 | 10 49.00 -660.00 -368.00 0 0 0.000 0.000 | 31 967.24 270.04 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
152 18be873a | 10 3.00 1200.00 810.00 0 0 0.000 0.000 | 10 3.00 -660.00 -368.00 0 0 0.000 0.000 | 31 1012.76 270.03 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
156 18be873a | 10 7.00 1200.00 810.00 0 0 0.000 0.000 | 10 7.00 -660.00 -368.00 0 0 0.000 0.000 | 31 1058.28 270.03 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
160 18be873a | 10 11.00 1200.00 810.00 0 0 0.000 0.000 | 10 11.00 -660.00 -368.00 0 0 0.000 0.000 | 31 1103.79 270.02 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
164 18be873a | 10 15.00 1200.00 810.00 0 0 0.000 0.000 | 10 15.00 -660.00 -368.00 0 0 0.000 0.000 | 31 1149.31 270.02 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
168 18be873a | 10 19.00 1200.00 810.00 0 0 0.000 0.000 | 10 19.00 -660.00 -368.00 0 0 0.000 0.000 | 31 1194.83 270.01 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
172 18be873a | 10 23.00 1200.00 810.00 0 0 0.000 0.000 | 10 23.00 -660.00 -368.00 0 0 0.000 0.000 | 31 1240.34 270.01 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
176 18be873a | 10 27.00 1200.00 810.00 0 0 0.000 0.000 | 10 27.00 -660.00 -368.00 0 0 0.000 0.000 | 31 1285.86 270.00 0.000 0.000 |  |  | 2 1 0 0 960.00 360.00
179 e7847115
180 e7847115 | 10 31.00 1200.00 810.00 0 0 0.000 0.000 | 10 31.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 1 1903 999 960.00 360.00
184 e7847115 | 10 35.00 1200.00 810.00 0 0 0.000 0.000 | 10 35.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 1 1899 995 960.00 360.00
188 e7847115 | 10 39.00 1200.00 810.00 0 0 0.000 0.000 | 10 39.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 1 1895 991 960.00 360.00
192 e7847115 | 10 43.00 1200.00 810.00 0 0 0.000 0.000 | 10 43.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 1 1891 987 960.00 360.00
196 e7847115 | 10 47.00 1200.00 810.00 0 0 0.000 0.000 | 10 47.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 1 1887 983 960.00 360.00
200 e7847115 | 10 1.00 1200.00 810.00 0 0 0.000 0.000 | 10 1.00 -660.00 -368.00 0 0 0.000 0.000 |  |  |  | 1 1 1883 979 960.00 360.00
";

#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn saffrons_porygon_tackles_out_as_on_the_n64() {
    let replay = Replay {
        p1: FighterKind::Mario,
        route: "999:1:",
        weapons: &monster_weapons,
    };
    let Some(out) = replay.run_saffron(SAFFRON_PORYGON, 5, (60, 1)) else {
        return;
    };
    out.check("Saffron's gate and a forced Porygon");
}

/// `b34cpu/cpu34a` (`warp_link`, route below): Link dashes left to x
/// -1441, then the menu (70-180) sets the dummy's CP to the CPU (Default,
/// trait None, level 3) and makes a Beam Sword above Link (A at 170). The
/// trace pokes the dummy's `fp->team` to Link's after frame 0, so
/// `ftComputerProcDefault` finds no target and falls to its item branch:
/// the sword lands on the tower ledge (x -1391) at 224, `item_track_wait`
/// counts from 225 and passes `225 - 25 * 3` at 375 (`TrackItem`); the CPU
/// walks left and picks the sword up (`LightGet` 463), then attacks with
/// it (`objective_base`). Rows every 10th tick and 220-228, 372-380,
/// 460-466; the last group is the dummy's `objective item_track_wait`.
const CPU_TRACKS_A_DROPPED_SWORD: &str = "
0 00000001 | 10 1.00 -210.00 1399.25 0 0 0.000 0.000 | 10 1.00 765.00 1413.09 0 0 0.000 0.000 |  |  |  |  | 0 0
7 18be873a
10 18be873a | 15 4.00 -482.00 1399.68 0 0 0.000 0.000 | 10 11.00 765.00 1413.09 0 0 0.000 0.000 |  |  |  |  | 0 0
13 3d6c1037
20 3d6c1037 | 15 14.00 -967.60 1400.43 0 0 0.000 0.000 | 10 21.00 765.00 1413.09 0 0 0.000 0.000 |  |  |  |  | 0 0
24 72ae7869
25 df90722b
30 df90722b | 17 5.00 -1312.50 1400.97 0 0 0.000 0.000 | 10 31.00 765.00 1413.09 0 0 0.000 0.000 |  |  |  |  | 0 0
40 df90722b | 17 15.00 -1441.45 1429.50 0 0 0.000 0.000 | 10 41.00 765.00 1413.09 0 0 0.000 0.000 |  |  |  |  | 0 0
50 df90722b | 10 0.00 -1441.45 1429.50 0 0 0.000 0.000 | 10 1.00 765.00 1413.09 0 0 0.000 0.000 |  |  |  |  | 0 0
60 df90722b | 10 10.00 -1441.45 1429.50 0 0 0.000 0.000 | 10 11.00 765.00 1413.09 0 0 0.000 0.000 |  |  |  |  | 0 0
70 df90722b | 10 20.00 -1441.45 1429.50 0 0 0.000 0.000 | 10 21.00 765.00 1413.09 0 0 0.000 0.000 |  |  |  |  | 0 0
80 df90722b | 10 30.00 -1441.45 1429.50 0 0 0.000 0.000 | 10 31.00 765.00 1413.09 0 0 0.000 0.000 |  |  |  |  | 0 0
90 df90722b | 10 40.00 -1441.45 1429.50 0 0 0.000 0.000 | 10 41.00 765.00 1413.09 0 0 0.000 0.000 |  |  |  |  | 0 0
100 df90722b | 10 50.00 -1441.45 1429.50 0 0 0.000 0.000 | 10 1.00 765.00 1413.09 0 0 0.000 0.000 |  |  |  |  | 0 0
110 df90722b | 10 60.00 -1441.45 1429.50 0 0 0.000 0.000 | 10 11.00 765.00 1413.09 0 0 0.000 0.000 |  |  |  |  | 0 0
120 df90722b | 10 70.00 -1441.45 1429.50 0 0 0.000 0.000 | 10 21.00 765.00 1413.09 0 0 0.000 0.000 |  |  |  |  | 0 0
130 df90722b | 10 0.00 -1441.45 1429.50 0 0 0.000 0.000 | 10 31.00 765.00 1413.09 0 0 0.000 0.000 |  |  |  |  | 0 0
140 df90722b | 10 10.00 -1441.45 1429.50 0 0 0.000 0.000 | 10 41.00 765.00 1413.09 0 0 0.000 0.000 |  |  |  |  | 0 0
150 df90722b | 10 20.00 -1441.45 1429.50 0 0 0.000 0.000 | 10 1.00 765.00 1413.09 0 0 0.000 0.000 |  |  |  |  | 0 0
160 df90722b | 10 30.00 -1441.45 1429.50 0 0 0.000 0.000 | 10 11.00 765.00 1413.09 0 0 0.000 0.000 |  |  |  |  | 0 0
170 12dbce4a | 10 40.00 -1441.45 1429.50 0 0 0.000 0.000 | 10 21.00 765.00 1413.09 0 0 0.000 0.000 | 7 -1441.45 1658.30 0.000 28.800 |  |  |  | 0 0
171 8124f618
172 95475fb6
173 6443b2a4
180 428b763e | 10 50.00 -1441.45 1429.50 0 0 0.000 0.000 | 10 31.00 765.00 1413.09 0 0 0.000 0.000 | 7 -1441.45 1880.30 0.000 16.800 |  |  |  | 2 0
181 701fe6a8
182 fa5aea62
183 45099bec
184 bb25e9c6
185 ee5d3670
186 e3cbf86a
187 7f965a34
188 ce45da4e
189 2213eb38
190 eb899372 | 10 60.00 -1441.45 1429.50 0 0 0.000 0.000 | 10 41.00 765.00 1413.09 0 0 0.000 0.000 | 7 -1441.45 1982.30 0.000 4.800 |  |  |  | 2 0
191 b01c0d7c
192 d6ae67d6
193 01202500
194 a350db7a
195 4b40d5c4
196 eb36b25e
197 3e1203c8
198 5f32f082
199 ba9ed30c
200 df49d9e6 | 10 70.00 -1441.45 1429.50 0 0 0.000 0.000 | 10 1.00 765.00 1413.09 0 0 0.000 0.000 | 7 -1441.45 1964.30 0.000 -7.200 |  |  |  | 2 0
201 4cada790
202 5e14f28a
203 c9442554
204 1366fe6e
205 b66b3058
206 c2300192
207 6032ec9c
208 15a13ff6
209 8822e463
210 409dcdf5 | 10 0.00 -1441.45 1429.50 0 0 0.000 0.000 | 18 2.00 765.00 1413.09 0 0 0.000 0.000 | 7 -1438.10 1858.57 3.356 7.071 |  |  |  | 2 0
211 f98b5417
212 73dadb49
213 26caec0b
214 890252dd
215 5772403f
216 704968b1
217 ca8024b3
218 96c590c5
219 3cd5ad67
220 c0807f19 | 10 10.00 -1441.45 1429.50 0 0 0.000 0.000 | 12 1.00 776.95 1414.14 0 0 0.000 0.000 | 7 -1404.54 1863.28 3.356 -4.929 |  |  |  | 2 0
221 b3ea2e5b | 10 11.00 -1441.45 1429.50 0 0 0.000 0.000 | 12 2.00 788.91 1415.20 0 0 0.000 0.000 | 7 -1401.19 1857.15 3.356 -6.129 |  |  |  | 2 0
222 48cc27ad | 10 12.00 -1441.45 1429.50 0 0 0.000 0.000 | 12 3.00 800.86 1416.25 0 0 0.000 0.000 | 7 -1397.83 1849.82 3.356 -7.329 |  |  |  | 2 0
223 e0bf3b8f | 10 13.00 -1441.45 1429.50 0 0 0.000 0.000 | 12 4.00 812.81 1417.31 0 0 0.000 0.000 | 7 -1394.48 1841.29 3.356 -8.529 |  |  |  | 2 0
224 bcd6be81 | 10 14.00 -1441.45 1429.50 0 0 0.000 0.000 | 12 5.00 824.77 1418.36 0 0 0.000 0.000 | 7 -1391.12 1838.27 0.000 0.000 |  |  |  | 2 0
225 6ad66fba | 10 15.00 -1441.45 1429.50 0 0 0.000 0.000 | 10 0.00 830.74 1418.89 0 0 0.000 0.000 | 7 -1391.12 1838.27 0.000 0.000 |  |  |  | 2 1
226 c22d0c04 | 10 16.00 -1441.45 1429.50 0 0 0.000 0.000 | 18 1.00 830.74 1418.89 0 0 0.000 0.000 | 7 -1391.12 1838.27 0.000 0.000 |  |  |  | 2 2
227 b68e9a9e | 10 17.00 -1441.45 1429.50 0 0 0.000 0.000 | 18 2.00 830.74 1418.89 0 0 0.000 0.000 | 7 -1391.12 1838.27 0.000 0.000 |  |  |  | 2 3
228 f9832e08 | 10 18.00 -1441.45 1429.50 0 0 0.000 0.000 | 18 3.00 830.74 1418.89 0 0 0.000 0.000 | 7 -1391.12 1838.27 0.000 0.000 |  |  |  | 2 4
229 c6576cc2
230 bc61314c | 10 20.00 -1441.45 1429.50 0 0 0.000 0.000 | 18 5.00 830.74 1418.89 0 0 0.000 0.000 | 7 -1391.12 1838.27 0.000 0.000 |  |  |  | 2 6
231 ec692a26
232 ba6179d0
233 7dd156ca
234 5e9dab94
235 1916b6ae
236 eb72aa98
237 401d4dd2
238 83849adc
239 494a6036
240 4402e060 | 10 30.00 -1441.45 1429.50 0 0 0.000 0.000 | 12 4.00 782.93 1414.67 0 0 0.000 0.000 | 7 -1391.12 1838.27 0.000 0.000 |  |  |  | 2 16
241 ebe871da
242 d42c1f24
243 ebcb46be
244 0e123b28
245 a833e2e2
246 a49e586c
247 91f48a46
248 ead4daf0
249 ccd4c0ea
250 205966b4 | 10 40.00 -1441.45 1429.50 0 0 0.000 0.000 | 12 14.00 663.39 1404.12 0 0 0.000 0.000 | 7 -1391.12 1838.27 0.000 0.000 |  |  |  | 2 26
251 80354ace
252 e732dfb8
253 9af42bf2
254 c6cf69fc
255 0e90a856
256 e0486980
257 c58f43fa
258 37e68244
259 d91dc2de
260 37e59848 | 10 50.00 -1441.45 1429.50 0 0 0.000 0.000 | 12 24.00 543.67 1398.08 0 0 0.000 0.000 | 7 -1391.12 1838.27 0.000 0.000 |  |  |  | 2 36
261 c9f72902
262 5078cf8c
263 c087ba66
264 d90e8c10
265 1850fb0a
266 96d471d4
267 ba8daeee
268 8c7b64d8
269 3807d88d
270 8633736f | 10 60.00 -1441.45 1429.50 0 0 0.000 0.000 | 12 34.00 423.67 1398.27 0 0 0.000 0.000 | 7 -1391.12 1838.27 0.000 0.000 |  |  |  | 2 46
271 a51fb561
272 627056e3
273 99dad475
274 cd548e97
275 bbb1e9c9
276 d13e6e8b
277 6a15e95d
278 dc468abf
279 30040731
280 de73b733 | 10 70.00 -1441.45 1429.50 0 0 0.000 0.000 | 12 44.00 303.67 1398.45 0 0 0.000 0.000 | 7 -1391.12 1838.27 0.000 0.000 |  |  |  | 2 56
281 f3d9b745
282 182f07e7
283 5968ad99
284 acf7d0db
285 0a4ade2d
286 4e57a60f
287 d8767d01
288 93165b83
289 ef11fe15
290 ceae0537 | 10 0.00 -1441.45 1429.50 0 0 0.000 0.000 | 12 54.00 183.67 1398.64 0 0 0.000 0.000 | 7 -1391.12 1838.27 0.000 0.000 |  |  |  | 2 66
291 85881569
292 7cfef72b
293 c8dbb6fd
294 9643c55f
295 ef3c16d1
296 5f4543d3
297 a9d8a8e5
298 89ce8687
299 68f52139
300 b960e17b | 10 10.00 -1441.45 1429.50 0 0 0.000 0.000 | 12 4.00 63.67 1398.83 0 0 0.000 0.000 | 7 -1391.12 1838.27 0.000 0.000 |  |  |  | 2 76
301 263d73cd
302 c027e8af
303 a959d4a1
304 282d7023
305 7ac2b7b5
306 dccd8bd7
307 f8d4d109
308 086a8fcb
309 4325149d
310 7a610fff | 10 20.00 -1441.45 1429.50 0 0 0.000 0.000 | 12 14.00 -56.33 1399.01 0 0 0.000 0.000 | 7 -1391.12 1838.27 0.000 0.000 |  |  |  | 2 86
311 f014b671
312 293be073
313 c0a52a85
314 a5281527
315 c68c24d9
316 8ea9021b
317 b087996d
318 658c3b4f
319 30f1bc41
320 441d94c3 | 10 30.00 -1441.45 1429.50 0 0 0.000 0.000 | 12 24.00 -176.33 1399.20 0 0 0.000 0.000 | 7 -1391.12 1838.27 0.000 0.000 |  |  |  | 2 96
321 b2950155
322 5a9b2277
323 cfc01ca9
324 3ee9386b
325 3f9a023d
326 64866a9f
327 2db5e611
328 50bf8d13
329 2fe73c25
330 5f23b3c7 | 10 40.00 -1441.45 1429.50 0 0 0.000 0.000 | 12 34.00 -296.33 1399.39 0 0 0.000 0.000 | 7 -1391.12 1838.27 0.000 0.000 |  |  |  | 2 106
331 4e55b879
332 2a3832bb
333 d1d14f0d
334 ec6c9def
335 cc6633e1
336 6d4ec963
337 9030daf5
338 4efec917
339 8871f849
340 cfe2f10b | 10 50.00 -1441.45 1429.50 0 0 0.000 0.000 | 10 9.00 -302.33 1399.40 0 0 0.000 0.000 | 7 -1391.12 1838.27 0.000 0.000 |  |  |  | 2 116
341 28e27fdd
342 549bd53f
343 e747a5b1
344 4e3849b3
345 7346ddc5
346 50a96267
347 a079dc19
348 6d76735b
349 b6c294ad
350 26b1108f | 10 60.00 -1441.45 1429.50 0 0 0.000 0.000 | 10 19.00 -302.33 1399.40 0 0 0.000 0.000 | 7 -1391.12 1838.27 0.000 0.000 |  |  |  | 2 126
351 1cdf3b81
352 8e290e03
353 913e4495
354 64e07fb7
355 651263e9
356 4ebfb9ab
357 6da68d7d
358 6e894fdf
359 9ff1f551
360 06e367ca | 10 70.00 -1441.45 1429.50 0 0 0.000 0.000 | 12 1.00 -314.33 1399.41 0 0 0.000 0.000 | 7 -1391.12 1838.27 0.000 0.000 |  |  |  | 2 136
361 43284494
362 0c1e17ae
363 f21d1398
364 12c2fed2
365 773fd3dc
366 06076136
367 ab0fe960
368 b305c2da
369 791bf824
370 c461e7be | 10 0.00 -1441.45 1429.50 0 0 0.000 0.000 | 12 11.00 -434.33 1399.60 0 0 0.000 0.000 | 7 -1391.12 1838.27 0.000 0.000 |  |  |  | 2 146
371 58c5e428
372 a12cd3e2 | 10 2.00 -1441.45 1429.50 0 0 0.000 0.000 | 12 13.00 -458.33 1399.64 0 0 0.000 0.000 | 7 -1391.12 1838.27 0.000 0.000 |  |  |  | 2 148
373 c346d16c | 10 3.00 -1441.45 1429.50 0 0 0.000 0.000 | 12 14.00 -470.33 1399.66 0 0 0.000 0.000 | 7 -1391.12 1838.27 0.000 0.000 |  |  |  | 2 149
374 3308cb46 | 10 4.00 -1441.45 1429.50 0 0 0.000 0.000 | 12 15.00 -482.33 1399.68 0 0 0.000 0.000 | 7 -1391.12 1838.27 0.000 0.000 |  |  |  | 2 150
375 3308cb46 | 10 5.00 -1441.45 1429.50 0 0 0.000 0.000 | 12 16.00 -494.33 1399.69 0 0 0.000 0.000 | 7 -1391.12 1838.27 0.000 0.000 |  |  |  | 5 151
376 3308cb46 | 10 6.00 -1441.45 1429.50 0 0 0.000 0.000 | 12 17.00 -506.33 1399.71 0 0 0.000 0.000 | 7 -1391.12 1838.27 0.000 0.000 |  |  |  | 5 152
377 3308cb46 | 10 7.00 -1441.45 1429.50 0 0 0.000 0.000 | 12 18.00 -518.33 1399.73 0 0 0.000 0.000 | 7 -1391.12 1838.27 0.000 0.000 |  |  |  | 5 153
378 3308cb46 | 10 8.00 -1441.45 1429.50 0 0 0.000 0.000 | 12 19.00 -530.33 1399.75 0 0 0.000 0.000 | 7 -1391.12 1838.27 0.000 0.000 |  |  |  | 5 154
379 3308cb46 | 10 9.00 -1441.45 1429.50 0 0 0.000 0.000 | 12 20.00 -542.33 1399.77 0 0 0.000 0.000 | 7 -1391.12 1838.27 0.000 0.000 |  |  |  | 5 155
380 3308cb46 | 10 10.00 -1441.45 1429.50 0 0 0.000 0.000 | 12 21.00 -554.33 1399.79 0 0 0.000 0.000 | 7 -1391.12 1838.27 0.000 0.000 |  |  |  | 5 156
390 3308cb46 | 10 20.00 -1441.45 1429.50 0 0 0.000 0.000 | 12 31.00 -674.33 1399.97 0 0 0.000 0.000 | 7 -1391.12 1838.27 0.000 0.000 |  |  |  | 5 166
400 3308cb46 | 10 30.00 -1441.45 1429.50 0 0 0.000 0.000 | 12 41.00 -794.33 1400.16 0 0 0.000 0.000 | 7 -1391.12 1838.27 0.000 0.000 |  |  |  | 5 176
410 3308cb46 | 10 40.00 -1441.45 1429.50 0 0 0.000 0.000 | 12 51.00 -914.33 1400.35 0 0 0.000 0.000 | 7 -1391.12 1838.27 0.000 0.000 |  |  |  | 5 186
420 3308cb46 | 10 50.00 -1441.45 1429.50 0 0 0.000 0.000 | 12 1.00 -1034.33 1400.54 0 0 0.000 0.000 | 7 -1391.12 1838.27 0.000 0.000 |  |  |  | 5 196
430 3308cb46 | 10 60.00 -1441.45 1429.50 0 0 0.000 0.000 | 12 11.00 -1154.33 1400.72 0 0 0.000 0.000 | 7 -1391.12 1838.27 0.000 0.000 |  |  |  | 5 206
440 3308cb46 | 10 70.00 -1468.45 1436.59 0 0 0.000 0.000 | 12 21.00 -1247.33 1400.87 0 0 0.000 0.000 | 7 -1391.12 1838.27 0.000 0.000 |  |  |  | 5 216
450 1edcd4f1 | 10 0.00 -1522.45 1450.78 0 0 0.000 0.000 | 10 1.00 -1295.33 1400.94 0 0 0.000 0.000 | 7 -1391.12 1838.27 0.000 0.000 |  |  |  | 5 226
460 1edcd4f1 | 10 10.00 -1522.45 1450.78 0 0 0.000 0.000 | 10 11.00 -1295.33 1400.94 0 0 0.000 0.000 | 7 -1391.12 1838.27 0.000 0.000 |  |  |  | 5 226
461 1edcd4f1 | 10 11.00 -1522.45 1450.78 0 0 0.000 0.000 | 10 12.00 -1295.33 1400.94 0 0 0.000 0.000 | 7 -1391.12 1838.27 0.000 0.000 |  |  |  | 5 226
462 1edcd4f1 | 10 12.00 -1522.45 1450.78 0 0 0.000 0.000 | 10 13.00 -1295.33 1400.94 0 0 0.000 0.000 | 7 -1391.12 1838.27 0.000 0.000 |  |  |  | 5 226
463 1edcd4f1 | 10 13.00 -1522.45 1450.78 0 0 0.000 0.000 | 100 1.00 -1295.33 1400.94 0 0 0.000 0.000 | 7 -1391.12 1838.27 0.000 0.000 |  |  |  | 5 226
464 1edcd4f1 | 10 14.00 -1522.45 1450.78 0 0 0.000 0.000 | 100 2.00 -1295.33 1400.94 0 0 0.000 0.000 | 7 0.00 0.00 0.000 0.000 |  |  |  | 5 226
465 1edcd4f1 | 10 15.00 -1522.45 1450.78 0 0 0.000 0.000 | 100 3.00 -1295.33 1400.94 0 0 0.000 0.000 | 7 0.00 0.00 0.000 0.000 |  |  |  | 5 226
466 a0a8f2f3 | 10 16.00 -1522.45 1450.78 0 0 0.000 0.000 | 100 4.00 -1295.33 1400.94 0 0 0.000 0.000 | 7 0.00 0.00 0.000 0.000 |  |  |  | 2 0
467 f8fed105
468 71f2efa7
469 f871d359
470 e4a0249b | 10 20.00 -1522.45 1450.78 0 0 0.000 0.000 | 12 1.00 -1307.33 1400.96 0 0 0.000 0.000 | 7 0.00 0.00 0.000 0.000 |  |  |  | 2 0
471 923bcfed
472 b10625cf
473 b57efac1
474 5478c743
475 884dc7d5
476 d4be1cf7
477 66beeb29
478 d7c27aeb
479 d84158bd
480 638c751f | 10 30.00 -1583.20 1466.74 0 0 0.000 0.000 | 12 11.00 -1364.61 1409.30 0 0 0.000 0.000 | 7 0.00 0.00 0.000 0.000 |  |  |  | 2 0
481 6f304491
482 da70df93
483 c6a722a5
484 0786ce47
485 1295a6f9
486 4f5b953b
487 f613c58d
488 58e6c86f
489 93f5b261
490 c2be3be3 | 10 40.00 -1643.95 1482.70 0 0 0.000 0.000 | 12 21.00 -1419.92 1423.84 0 0 0.000 0.000 | 7 0.00 0.00 0.000 0.000 |  |  |  | 2 0
491 0d9fe175
492 b88a0397
493 e41b06c9
494 7cb8738b
495 ef68165d
496 3a721fbf
497 e0144431
498 b3cddc33
499 7f0d0445
500 a044bce7 | 10 50.00 -1697.95 1496.89 0 0 0.000 0.000 | 12 31.00 -1481.98 1440.14 0 0 0.000 0.000 | 7 0.00 0.00 0.000 0.000 |  |  |  | 2 0
501 1fb40a99
502 cf6615db
503 f8334b2d
504 63cb7b0f
505 5410fa01
506 ba4cc083
507 d5038b15
508 d173fa37
509 3605b269
510 45317c2b | 10 60.00 -1758.70 1512.85 0 0 0.000 0.000 | 12 41.00 -1537.29 1454.68 0 0 0.000 0.000 | 7 0.00 0.00 0.000 0.000 |  |  |  | 2 0
511 44aa63fd
512 32cfda5f
513 04b0d3d1
514 9927e8d3
515 31d875e5
516 0914bb87
517 93f4fe39
518 ba27a67b
519 d94260cd
520 579c3daf | 10 70.00 -1819.45 1528.82 0 0 0.000 0.000 | 12 51.00 -1592.60 1469.21 0 0 0.000 0.000 | 7 0.00 0.00 0.000 0.000 |  |  |  | 2 0
521 eaf8d1a1
522 198c5523
523 f020c4b5
524 fe6400d7
525 72a6ee09
526 389594cb
527 5ab0419d
528 248da4ff
529 b42df371
530 5ae70573 | 10 0.00 -1873.45 1543.00 0 0 0.000 0.000 | 12 1.00 -1654.66 1485.52 0 0 0.000 0.000 | 7 0.00 0.00 0.000 0.000 |  |  |  | 2 0
531 72b17785
532 b2deca27
533 a78081d9
534 4908471b
535 dde9066d
536 de41104f
537 91d53941
538 22e4f9c3
539 f49f8e55
540 c2421777 | 10 10.00 -1934.20 1558.97 0 0 0.000 0.000 | 12 11.00 -1709.97 1500.05 0 0 0.000 0.000 | 7 0.00 0.00 0.000 0.000 |  |  |  | 2 0
541 7426b9a9
542 424cbd6b
543 b821af3d
544 0b937f9f
545 09b3a311
546 2d733213
547 59400925
548 b28ae8c7
549 567e9579
550 996ff7bb | 36 2.00 -1940.00 1560.49 0 0 0.000 0.000 | 12 21.00 -1765.28 1514.58 0 0 0.000 0.000 | 7 0.00 0.00 0.000 0.000 |  |  |  | 2 0
551 4ecf3c0d
552 c5a1f2ef
553 c5ce30e1
554 7cbeae63
555 fc27e7f5
556 43f63e17
557 d8ad1549
558 31bef60b
559 e7a6acdd
560 07c96a3f | 35 2.00 -1940.00 1560.49 0 0 0.000 0.000 | 12 31.00 -1813.84 1527.34 0 0 0.000 0.000 | 7 0.00 0.00 0.000 0.000 |  |  |  | 2 0
561 6469e2b1
562 a9346eb3
563 812c2ac5
564 c1011767
565 61173919
566 acc6b85b
567 789d01ad
568 ffa6e58f
569 480bb881
570 31817303 | 35 12.00 -1940.00 1560.49 0 0 0.000 0.000 | 12 41.00 -1862.40 1540.10 0 0 0.000 0.000 | 7 0.00 0.00 0.000 0.000 |  |  |  | 2 0
571 a461d195
572 4e6874b7
573 026200e9
574 ba543eab
575 77e73a7d
576 5d1764df
577 6778b251
578 ca92bb53
579 0a1ddc65
580 3b295607 | 35 22.00 -1940.00 1560.49 0 0 0.000 0.000 | 10 7.00 -1823.92 1529.99 0 0 0.000 0.000 | 7 0.00 0.00 0.000 0.000 |  |  |  | 2 0
581 4b726cb9
582 687488fb
583 abfa574d
584 a237e82f
585 1db5d021
586 af9547a3
587 0ef54b35
588 5080bb57
589 176d7c89
590 7374974b | 35 32.00 -1940.00 1560.49 0 0 0.000 0.000 | 10 17.00 -1756.42 1512.25 0 0 0.000 0.000 | 7 0.00 0.00 0.000 0.000 |  |  |  | 2 0
591 fb8b581d
592 73656f7f
593 fa0811f1
594 f1f617f3
595 97bd1e05
596 21eba4a7
597 5db83059
598 95e1699b
599 3d8f3ced
600 e73cfacf | 35 42.00 -1940.00 1560.49 0 0 0.000 0.000 | 10 27.00 -1709.17 1499.84 0 0 0.000 0.000 | 7 0.00 0.00 0.000 0.000 |  |  |  | 2 0
";

#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn a_cpu_tracks_a_dropped_item_as_on_the_n64() {
    let replay = Replay {
        p1: FighterKind::Link,
        route: "2:23:x=-80;70:1:start;80:1:x=80;90:1:x=80;100:1:x=80;110:1:x=80;\
                120:1:y=-80;130:1:x=80;140:1:x=80;150:1:x=80;160:1:x=80;170:1:a;180:1:start",
        weapons: &no_weapons,
    };
    let Some(out) = replay.run_cpu(CPU_TRACKS_A_DROPPED_SWORD) else {
        return;
    };
    out.check("a CPU tracks a dropped Beam Sword");
}

/// Ness's weapons as `WPKind x y vel.x vel.y`, in link order: PK Fire's
/// spark (13), PK Thunder's head (14) and its trails (15).
fn ness_weapons(pool: &ssb_game::weapon::WeaponPool) -> Vec<Vec<f32>> {
    use ssb_engine::math::Vec3;
    use ssb_game::weapon::NessShot;
    let row = |k: f32, p: Vec3, v: Vec3| vec![k, p.x, p.y, v.x, v.y];
    pool.ness_shots()
        .map(|w| match w {
            NessShot::Fire(w) => row(13.0, w.position, w.velocity),
            NessShot::Head(w) => row(14.0, w.position, w.velocity),
            NessShot::Trail(w) => row(15.0, w.position, w.velocity),
        })
        .collect()
}

/// Ness (P1) on Dream Land's floor, facing left, and Fox (P2) on the left
/// platform, facing right: where `vsness`'s entries leave them, and the
/// frames their Wait starts (Ness 270, Fox 330; Go at 390).
const VS_NESS_FOX: [(f32, f32, ssb_game::fighter::Facing, u64); 2] = [
    (0.0, 0.0, ssb_game::fighter::Facing::Left, 270),
    (-1397.0, 904.0, ssb_game::fighter::Facing::Right, 330),
];

/// `b36/vsr1` (`vsness.z64`, VS Dream Land, Ness P1 vs Fox P2, both
/// human): Fox drops through the platform, walks right and holds his
/// reflector (B + down) at x -1016; Ness's PK Fire spark (t489) reaches
/// it at t495 (Fox st 237): the spark moves on by its old velocity, its
/// velocity mirrored in x only. It hits Ness at t500 (8%, hitlag 7) and
/// its pillar (`ITKind` 20) burns him on (11% at 501, 14 at 517, 17 at
/// 535). Every frame from Fox's first Wait frame (330).
const VS_PK_FIRE_REFLECTED: &str = "
330 45099bec | 10 60.00 0.00 0.00 0 0 0.000 0.000 | 10 0.00 -1397.00 904.00 0 0 0.000 0.000
331 45099bec | 10 61.00 0.00 0.00 0 0 0.000 0.000 | 10 1.00 -1397.00 904.00 0 0 0.000 0.000
332 45099bec | 10 62.00 0.00 0.00 0 0 0.000 0.000 | 10 2.00 -1397.00 904.00 0 0 0.000 0.000
333 45099bec | 10 63.00 0.00 0.00 0 0 0.000 0.000 | 10 3.00 -1397.00 904.00 0 0 0.000 0.000
334 45099bec | 10 64.00 0.00 0.00 0 0 0.000 0.000 | 10 4.00 -1397.00 904.00 0 0 0.000 0.000
335 45099bec | 10 65.00 0.00 0.00 0 0 0.000 0.000 | 10 5.00 -1397.00 904.00 0 0 0.000 0.000
336 45099bec | 10 66.00 0.00 0.00 0 0 0.000 0.000 | 10 6.00 -1397.00 904.00 0 0 0.000 0.000
337 45099bec | 10 67.00 0.00 0.00 0 0 0.000 0.000 | 10 7.00 -1397.00 904.00 0 0 0.000 0.000
338 45099bec | 10 68.00 0.00 0.00 0 0 0.000 0.000 | 10 8.00 -1397.00 904.00 0 0 0.000 0.000
339 45099bec | 10 69.00 0.00 0.00 0 0 0.000 0.000 | 10 9.00 -1397.00 904.00 0 0 0.000 0.000
340 45099bec | 10 70.00 0.00 0.00 0 0 0.000 0.000 | 10 10.00 -1397.00 904.00 0 0 0.000 0.000
341 45099bec | 10 71.00 0.00 0.00 0 0 0.000 0.000 | 10 11.00 -1397.00 904.00 0 0 0.000 0.000
342 45099bec | 10 72.00 0.00 0.00 0 0 0.000 0.000 | 10 12.00 -1397.00 904.00 0 0 0.000 0.000
343 45099bec | 10 73.00 0.00 0.00 0 0 0.000 0.000 | 10 13.00 -1397.00 904.00 0 0 0.000 0.000
344 45099bec | 10 74.00 0.00 0.00 0 0 0.000 0.000 | 10 14.00 -1397.00 904.00 0 0 0.000 0.000
345 45099bec | 10 75.00 0.00 0.00 0 0 0.000 0.000 | 10 15.00 -1397.00 904.00 0 0 0.000 0.000
346 45099bec | 10 76.00 0.00 0.00 0 0 0.000 0.000 | 10 16.00 -1397.00 904.00 0 0 0.000 0.000
347 45099bec | 10 77.00 0.00 0.00 0 0 0.000 0.000 | 10 17.00 -1397.00 904.00 0 0 0.000 0.000
348 45099bec | 10 78.00 0.00 0.00 0 0 0.000 0.000 | 10 18.00 -1397.00 904.00 0 0 0.000 0.000
349 45099bec | 10 79.00 0.00 0.00 0 0 0.000 0.000 | 10 19.00 -1397.00 904.00 0 0 0.000 0.000
350 45099bec | 10 80.00 0.00 0.00 0 0 0.000 0.000 | 10 20.00 -1397.00 904.00 0 0 0.000 0.000
351 45099bec | 10 81.00 0.00 0.00 0 0 0.000 0.000 | 10 21.00 -1397.00 904.00 0 0 0.000 0.000
352 45099bec | 10 82.00 0.00 0.00 0 0 0.000 0.000 | 10 22.00 -1397.00 904.00 0 0 0.000 0.000
353 45099bec | 10 83.00 0.00 0.00 0 0 0.000 0.000 | 10 23.00 -1397.00 904.00 0 0 0.000 0.000
354 45099bec | 10 84.00 0.00 0.00 0 0 0.000 0.000 | 10 24.00 -1397.00 904.00 0 0 0.000 0.000
355 45099bec | 10 85.00 0.00 0.00 0 0 0.000 0.000 | 10 25.00 -1397.00 904.00 0 0 0.000 0.000
356 45099bec | 10 86.00 0.00 0.00 0 0 0.000 0.000 | 10 26.00 -1397.00 904.00 0 0 0.000 0.000
357 45099bec | 10 87.00 0.00 0.00 0 0 0.000 0.000 | 10 27.00 -1397.00 904.00 0 0 0.000 0.000
358 45099bec | 10 88.00 0.00 0.00 0 0 0.000 0.000 | 10 28.00 -1397.00 904.00 0 0 0.000 0.000
359 45099bec | 10 89.00 0.00 0.00 0 0 0.000 0.000 | 10 29.00 -1397.00 904.00 0 0 0.000 0.000
360 45099bec | 10 90.00 0.00 0.00 0 0 0.000 0.000 | 10 30.00 -1397.00 904.00 0 0 0.000 0.000
361 45099bec | 10 91.00 0.00 0.00 0 0 0.000 0.000 | 10 31.00 -1397.00 904.00 0 0 0.000 0.000
362 45099bec | 10 92.00 0.00 0.00 0 0 0.000 0.000 | 10 32.00 -1397.00 904.00 0 0 0.000 0.000
363 45099bec | 10 93.00 0.00 0.00 0 0 0.000 0.000 | 10 33.00 -1397.00 904.00 0 0 0.000 0.000
364 45099bec | 10 94.00 0.00 0.00 0 0 0.000 0.000 | 10 34.00 -1397.00 904.00 0 0 0.000 0.000
365 45099bec | 10 95.00 0.00 0.00 0 0 0.000 0.000 | 10 35.00 -1397.00 904.00 0 0 0.000 0.000
366 45099bec | 10 96.00 0.00 0.00 0 0 0.000 0.000 | 10 36.00 -1397.00 904.00 0 0 0.000 0.000
367 45099bec | 10 97.00 0.00 0.00 0 0 0.000 0.000 | 10 37.00 -1397.00 904.00 0 0 0.000 0.000
368 45099bec | 10 98.00 0.00 0.00 0 0 0.000 0.000 | 10 38.00 -1397.00 904.00 0 0 0.000 0.000
369 45099bec | 10 99.00 0.00 0.00 0 0 0.000 0.000 | 10 39.00 -1397.00 904.00 0 0 0.000 0.000
370 45099bec | 10 100.00 0.00 0.00 0 0 0.000 0.000 | 10 40.00 -1397.00 904.00 0 0 0.000 0.000
371 45099bec | 10 101.00 0.00 0.00 0 0 0.000 0.000 | 10 41.00 -1397.00 904.00 0 0 0.000 0.000
372 45099bec | 10 102.00 0.00 0.00 0 0 0.000 0.000 | 10 42.00 -1397.00 904.00 0 0 0.000 0.000
373 45099bec | 10 103.00 0.00 0.00 0 0 0.000 0.000 | 10 43.00 -1397.00 904.00 0 0 0.000 0.000
374 45099bec | 10 104.00 0.00 0.00 0 0 0.000 0.000 | 10 44.00 -1397.00 904.00 0 0 0.000 0.000
375 45099bec | 10 105.00 0.00 0.00 0 0 0.000 0.000 | 10 45.00 -1397.00 904.00 0 0 0.000 0.000
376 45099bec | 10 106.00 0.00 0.00 0 0 0.000 0.000 | 10 46.00 -1397.00 904.00 0 0 0.000 0.000
377 45099bec | 10 107.00 0.00 0.00 0 0 0.000 0.000 | 10 47.00 -1397.00 904.00 0 0 0.000 0.000
378 45099bec | 10 108.00 0.00 0.00 0 0 0.000 0.000 | 10 48.00 -1397.00 904.00 0 0 0.000 0.000
379 45099bec | 10 109.00 0.00 0.00 0 0 0.000 0.000 | 10 49.00 -1397.00 904.00 0 0 0.000 0.000
380 45099bec | 10 110.00 0.00 0.00 0 0 0.000 0.000 | 10 50.00 -1397.00 904.00 0 0 0.000 0.000
381 45099bec | 10 111.00 0.00 0.00 0 0 0.000 0.000 | 10 51.00 -1397.00 904.00 0 0 0.000 0.000
382 45099bec | 10 112.00 0.00 0.00 0 0 0.000 0.000 | 10 52.00 -1397.00 904.00 0 0 0.000 0.000
383 45099bec | 10 113.00 0.00 0.00 0 0 0.000 0.000 | 10 53.00 -1397.00 904.00 0 0 0.000 0.000
384 45099bec | 10 114.00 0.00 0.00 0 0 0.000 0.000 | 10 54.00 -1397.00 904.00 0 0 0.000 0.000
385 45099bec | 10 115.00 0.00 0.00 0 0 0.000 0.000 | 10 55.00 -1397.00 904.00 0 0 0.000 0.000
386 45099bec | 10 116.00 0.00 0.00 0 0 0.000 0.000 | 10 56.00 -1397.00 904.00 0 0 0.000 0.000
387 45099bec | 10 117.00 0.00 0.00 0 0 0.000 0.000 | 10 57.00 -1397.00 904.00 0 0 0.000 0.000
388 45099bec | 10 118.00 0.00 0.00 0 0 0.000 0.000 | 10 58.00 -1397.00 904.00 0 0 0.000 0.000
389 45099bec | 10 119.00 0.00 0.00 0 0 0.000 0.000 | 10 59.00 -1397.00 904.00 0 0 0.000 0.000
390 45099bec | 10 0.00 0.00 0.00 0 0 0.000 0.000 | 10 60.00 -1397.00 904.00 0 0 0.000 0.000
391 45099bec | 10 1.00 0.00 0.00 0 0 0.000 0.000 | 10 61.00 -1397.00 904.00 0 0 0.000 0.000
392 45099bec | 10 2.00 0.00 0.00 0 0 0.000 0.000 | 10 62.00 -1397.00 904.00 0 0 0.000 0.000
393 45099bec | 10 3.00 0.00 0.00 0 0 0.000 0.000 | 10 63.00 -1397.00 904.00 0 0 0.000 0.000
394 45099bec | 10 4.00 0.00 0.00 0 0 0.000 0.000 | 10 64.00 -1397.00 904.00 0 0 0.000 0.000
395 92387aff | 10 5.00 0.00 0.00 0 0 0.000 0.000 | 10 65.00 -1397.00 904.00 0 0 0.000 0.000
396 92387aff | 10 6.00 0.00 0.00 0 0 0.000 0.000 | 10 66.00 -1397.00 904.00 0 0 0.000 0.000
397 92387aff | 10 7.00 0.00 0.00 0 0 0.000 0.000 | 10 67.00 -1397.00 904.00 0 0 0.000 0.000
398 92387aff | 10 8.00 0.00 0.00 0 0 0.000 0.000 | 10 68.00 -1397.00 904.00 0 0 0.000 0.000
399 92387aff | 10 9.00 0.00 0.00 0 0 0.000 0.000 | 10 69.00 -1397.00 904.00 0 0 0.000 0.000
400 92387aff | 10 10.00 0.00 0.00 0 0 0.000 0.000 | 10 0.00 -1397.00 904.00 0 0 0.000 0.000
401 92387aff | 10 11.00 0.00 0.00 0 0 0.000 0.000 | 10 1.00 -1397.00 904.00 0 0 0.000 0.000
402 92387aff | 10 12.00 0.00 0.00 0 0 0.000 0.000 | 10 2.00 -1397.00 904.00 0 0 0.000 0.000
403 92387aff | 10 13.00 0.00 0.00 0 0 0.000 0.000 | 10 3.00 -1397.00 904.00 0 0 0.000 0.000
404 92387aff | 10 14.00 0.00 0.00 0 0 0.000 0.000 | 10 4.00 -1397.00 904.00 0 0 0.000 0.000
405 92387aff | 10 15.00 0.00 0.00 0 0 0.000 0.000 | 28 1.00 -1397.00 904.00 0 0 0.000 0.000
406 92387aff | 10 16.00 0.00 0.00 0 0 0.000 0.000 | 28 2.00 -1397.00 904.00 0 0 0.000 0.000
407 92387aff | 10 17.00 0.00 0.00 0 0 0.000 0.000 | 28 3.00 -1397.00 904.00 0 0 0.000 0.000
408 92387aff | 10 18.00 0.00 0.00 0 0 0.000 0.000 | 33 1.00 -1397.00 900.00 0 0 0.000 0.000
409 92387aff | 10 19.00 0.00 0.00 0 0 0.000 0.000 | 33 2.00 -1397.00 892.00 0 0 0.000 0.000
410 92387aff | 10 20.00 0.00 0.00 0 0 0.000 0.000 | 33 3.00 -1397.00 880.00 0 0 0.000 0.000
411 92387aff | 10 21.00 0.00 0.00 0 0 0.000 0.000 | 33 4.00 -1397.00 864.00 0 0 0.000 0.000
412 92387aff | 10 22.00 0.00 0.00 0 0 0.000 0.000 | 33 5.00 -1397.00 844.00 0 0 0.000 0.000
413 92387aff | 10 23.00 0.00 0.00 0 0 0.000 0.000 | 33 6.00 -1397.00 820.00 0 0 0.000 0.000
414 92387aff | 10 24.00 0.00 0.00 0 0 0.000 0.000 | 33 7.00 -1397.00 792.00 0 0 0.000 0.000
415 92387aff | 10 25.00 0.00 0.00 0 0 0.000 0.000 | 33 8.00 -1397.00 760.00 0 0 0.000 0.000
416 92387aff | 10 26.00 0.00 0.00 0 0 0.000 0.000 | 33 9.00 -1397.00 724.00 0 0 0.000 0.000
417 92387aff | 10 27.00 0.00 0.00 0 0 0.000 0.000 | 33 10.00 -1397.00 684.00 0 0 0.000 0.000
418 92387aff | 10 28.00 0.00 0.00 0 0 0.000 0.000 | 33 11.00 -1397.00 640.00 0 0 0.000 0.000
419 92387aff | 10 29.00 0.00 0.00 0 0 0.000 0.000 | 33 12.00 -1397.00 592.00 0 0 0.000 0.000
420 92387aff | 10 30.00 0.00 0.00 0 0 0.000 0.000 | 33 13.00 -1397.00 540.00 0 0 0.000 0.000
421 92387aff | 10 31.00 0.00 0.00 0 0 0.000 0.000 | 33 14.00 -1397.00 484.00 0 0 0.000 0.000
422 92387aff | 10 32.00 0.00 0.00 0 0 0.000 0.000 | 33 15.00 -1397.00 424.00 0 0 0.000 0.000
423 92387aff | 10 33.00 0.00 0.00 0 0 0.000 0.000 | 33 16.00 -1397.00 364.00 0 0 0.000 0.000
424 92387aff | 10 34.00 0.00 0.00 0 0 0.000 0.000 | 33 17.00 -1397.00 304.00 0 0 0.000 0.000
425 92387aff | 10 35.00 0.00 0.00 0 0 0.000 0.000 | 33 18.00 -1397.00 244.00 0 0 0.000 0.000
426 92387aff | 10 36.00 0.00 0.00 0 0 0.000 0.000 | 33 19.00 -1397.00 184.00 0 0 0.000 0.000
427 92387aff | 10 37.00 0.00 0.00 0 0 0.000 0.000 | 33 20.00 -1397.00 124.00 0 0 0.000 0.000
428 92387aff | 10 38.00 0.00 0.00 0 0 0.000 0.000 | 33 21.00 -1397.00 64.00 0 0 0.000 0.000
429 92387aff | 10 39.00 0.00 0.00 0 0 0.000 0.000 | 33 22.00 -1397.00 4.00 0 0 0.000 0.000
430 9e1f7971 | 10 40.00 0.00 0.00 0 0 0.000 0.000 | 31 0.00 -1397.00 0.00 0 0 0.000 0.000
431 9e1f7971 | 10 41.00 0.00 0.00 0 0 0.000 0.000 | 31 1.00 -1397.00 0.00 0 0 0.000 0.000
432 9e1f7971 | 10 42.00 0.00 0.00 0 0 0.000 0.000 | 31 2.00 -1397.00 0.00 0 0 0.000 0.000
433 9e1f7971 | 10 43.00 0.00 0.00 0 0 0.000 0.000 | 31 3.00 -1397.00 0.00 0 0 0.000 0.000
434 9e1f7971 | 10 44.00 0.00 0.00 0 0 0.000 0.000 | 31 4.00 -1397.00 0.00 0 0 0.000 0.000
435 9e1f7971 | 10 45.00 0.00 0.00 0 0 0.000 0.000 | 31 5.00 -1397.00 0.00 0 0 0.000 0.000
436 9e1f7971 | 10 46.00 0.00 0.00 0 0 0.000 0.000 | 31 6.00 -1397.00 0.00 0 0 0.000 0.000
437 9e1f7971 | 10 47.00 0.00 0.00 0 0 0.000 0.000 | 31 7.00 -1397.00 0.00 0 0 0.000 0.000
438 9e1f7971 | 10 48.00 0.00 0.00 0 0 0.000 0.000 | 10 0.00 -1397.00 0.00 0 0 0.000 0.000
439 9e1f7971 | 10 49.00 0.00 0.00 0 0 0.000 0.000 | 10 1.00 -1397.00 0.00 0 0 0.000 0.000
440 9e1f7971 | 10 50.00 0.00 0.00 0 0 0.000 0.000 | 12 1.00 -1374.50 0.00 0 0 0.000 0.000
441 9e1f7971 | 10 51.00 0.00 0.00 0 0 0.000 0.000 | 12 2.00 -1352.00 0.00 0 0 0.000 0.000
442 9e1f7971 | 10 52.00 0.00 0.00 0 0 0.000 0.000 | 12 3.00 -1329.50 0.00 0 0 0.000 0.000
443 9e1f7971 | 10 53.00 0.00 0.00 0 0 0.000 0.000 | 12 4.00 -1307.00 0.00 0 0 0.000 0.000
444 9e1f7971 | 10 54.00 0.00 0.00 0 0 0.000 0.000 | 12 5.00 -1284.50 0.00 0 0 0.000 0.000
445 9e1f7971 | 10 55.00 0.00 0.00 0 0 0.000 0.000 | 12 6.00 -1262.00 0.00 0 0 0.000 0.000
446 9e1f7971 | 10 56.00 0.00 0.00 0 0 0.000 0.000 | 12 7.00 -1239.50 0.00 0 0 0.000 0.000
447 9e1f7971 | 10 57.00 0.00 0.00 0 0 0.000 0.000 | 12 8.00 -1217.00 0.00 0 0 0.000 0.000
448 9e1f7971 | 10 58.00 0.00 0.00 0 0 0.000 0.000 | 12 9.00 -1194.50 0.00 0 0 0.000 0.000
449 9e1f7971 | 10 59.00 0.00 0.00 0 0 0.000 0.000 | 12 10.00 -1172.00 0.00 0 0 0.000 0.000
450 9e1f7971 | 10 60.00 0.00 0.00 0 0 0.000 0.000 | 12 11.00 -1149.50 0.00 0 0 0.000 0.000
451 9e1f7971 | 10 61.00 0.00 0.00 0 0 0.000 0.000 | 12 12.00 -1127.00 0.00 0 0 0.000 0.000
452 9e1f7971 | 10 62.00 0.00 0.00 0 0 0.000 0.000 | 12 13.00 -1104.50 0.00 0 0 0.000 0.000
453 9e1f7971 | 10 63.00 0.00 0.00 0 0 0.000 0.000 | 12 14.00 -1082.00 0.00 0 0 0.000 0.000
454 9e1f7971 | 10 64.00 0.00 0.00 0 0 0.000 0.000 | 12 15.00 -1059.50 0.00 0 0 0.000 0.000
455 9e1f7971 | 10 65.00 0.00 0.00 0 0 0.000 0.000 | 12 16.00 -1037.00 0.00 0 0 0.000 0.000
456 9e1f7971 | 10 66.00 0.00 0.00 0 0 0.000 0.000 | 10 0.00 -1022.50 0.00 0 0 0.000 0.000
457 9e1f7971 | 10 67.00 0.00 0.00 0 0 0.000 0.000 | 10 1.00 -1016.00 0.00 0 0 0.000 0.000
458 9e1f7971 | 10 68.00 0.00 0.00 0 0 0.000 0.000 | 10 2.00 -1016.00 0.00 0 0 0.000 0.000
459 9e1f7971 | 10 69.00 0.00 0.00 0 0 0.000 0.000 | 10 3.00 -1016.00 0.00 0 0 0.000 0.000
460 9e1f7971 | 10 70.00 0.00 0.00 0 0 0.000 0.000 | 10 4.00 -1016.00 0.00 0 0 0.000 0.000
461 9e1f7971 | 10 71.00 0.00 0.00 0 0 0.000 0.000 | 10 5.00 -1016.00 0.00 0 0 0.000 0.000
462 9e1f7971 | 10 72.00 0.00 0.00 0 0 0.000 0.000 | 10 6.00 -1016.00 0.00 0 0 0.000 0.000
463 9e1f7971 | 10 73.00 0.00 0.00 0 0 0.000 0.000 | 10 7.00 -1016.00 0.00 0 0 0.000 0.000
464 9e1f7971 | 10 74.00 0.00 0.00 0 0 0.000 0.000 | 10 8.00 -1016.00 0.00 0 0 0.000 0.000
465 9e1f7971 | 10 75.00 0.00 0.00 0 0 0.000 0.000 | 10 9.00 -1016.00 0.00 0 0 0.000 0.000
466 9e1f7971 | 10 76.00 0.00 0.00 0 0 0.000 0.000 | 10 10.00 -1016.00 0.00 0 0 0.000 0.000
467 9e1f7971 | 10 77.00 0.00 0.00 0 0 0.000 0.000 | 10 11.00 -1016.00 0.00 0 0 0.000 0.000
468 9e1f7971 | 10 78.00 0.00 0.00 0 0 0.000 0.000 | 236 1.00 -1016.00 0.00 0 0 0.000 0.000
469 9e1f7971 | 10 79.00 0.00 0.00 0 0 0.000 0.000 | 236 2.00 -1016.00 0.00 0 0 0.000 0.000
470 9e1f7971 | 226 1.00 0.00 0.00 0 0 0.000 0.000 | 236 3.00 -1016.00 0.00 0 0 0.000 0.000
471 9e1f7971 | 226 2.00 0.00 0.00 0 0 0.000 0.000 | 239 0.00 -1016.00 0.00 0 0 0.000 0.000
472 9e1f7971 | 226 3.00 0.00 0.00 0 0 0.000 0.000 | 239 1.00 -1016.00 0.00 0 0 0.000 0.000
473 9e1f7971 | 226 4.00 0.00 0.00 0 0 0.000 0.000 | 239 2.00 -1016.00 0.00 0 0 0.000 0.000
474 9e1f7971 | 226 5.00 0.00 0.00 0 0 0.000 0.000 | 239 3.00 -1016.00 0.00 0 0 0.000 0.000
475 9e1f7971 | 226 6.00 0.00 0.00 0 0 0.000 0.000 | 239 4.00 -1016.00 0.00 0 0 0.000 0.000
476 9e1f7971 | 226 7.00 0.00 0.00 0 0 0.000 0.000 | 239 0.00 -1016.00 0.00 0 0 0.000 0.000
477 9e1f7971 | 226 8.00 0.00 0.00 0 0 0.000 0.000 | 239 1.00 -1016.00 0.00 0 0 0.000 0.000
478 9e1f7971 | 226 9.00 0.00 0.00 0 0 0.000 0.000 | 239 2.00 -1016.00 0.00 0 0 0.000 0.000
479 9e1f7971 | 226 10.00 0.00 0.00 0 0 0.000 0.000 | 239 3.00 -1016.00 0.00 0 0 0.000 0.000
480 9e1f7971 | 226 11.00 0.00 0.00 0 0 0.000 0.000 | 239 4.00 -1016.00 0.00 0 0 0.000 0.000
481 9e1f7971 | 226 12.00 0.00 0.00 0 0 0.000 0.000 | 239 0.00 -1016.00 0.00 0 0 0.000 0.000
482 9e1f7971 | 226 13.00 0.00 0.00 0 0 0.000 0.000 | 239 1.00 -1016.00 0.00 0 0 0.000 0.000
483 9e1f7971 | 226 14.00 0.00 0.00 0 0 0.000 0.000 | 239 2.00 -1016.00 0.00 0 0 0.000 0.000
484 9e1f7971 | 226 15.00 0.00 0.00 0 0 0.000 0.000 | 239 3.00 -1016.00 0.00 0 0 0.000 0.000
485 9e1f7971 | 226 16.00 0.00 0.00 0 0 0.000 0.000 | 239 4.00 -1016.00 0.00 0 0 0.000 0.000
486 9e1f7971 | 226 17.00 0.00 0.00 0 0 0.000 0.000 | 239 0.00 -1016.00 0.00 0 0 0.000 0.000
487 9e1f7971 | 226 18.00 0.00 0.00 0 0 0.000 0.000 | 239 1.00 -1016.00 0.00 0 0 0.000 0.000
488 9e1f7971 | 226 19.00 0.00 0.00 0 0 0.000 0.000 | 239 2.00 -1016.00 0.00 0 0 0.000 0.000
489 9e1f7971 | 226 20.00 0.00 0.00 0 0 0.000 0.000 | 239 3.00 -1016.00 0.00 0 0 0.000 0.000 |  | 13 -172.86 175.42 -72.856 -4.584
490 9e1f7971 | 226 21.00 0.00 0.00 0 0 0.000 0.000 | 239 4.00 -1016.00 0.00 0 0 0.000 0.000 |  | 13 -245.71 170.83 -72.856 -4.584
491 9e1f7971 | 226 22.00 0.00 0.00 0 0 0.000 0.000 | 239 0.00 -1016.00 0.00 0 0 0.000 0.000 |  | 13 -318.57 166.25 -72.856 -4.584
492 9e1f7971 | 226 23.00 0.00 0.00 0 0 0.000 0.000 | 239 1.00 -1016.00 0.00 0 0 0.000 0.000 |  | 13 -391.42 161.67 -72.856 -4.584
493 9e1f7971 | 226 24.00 0.00 0.00 0 0 0.000 0.000 | 239 2.00 -1016.00 0.00 0 0 0.000 0.000 |  | 13 -464.28 157.08 -72.856 -4.584
494 9e1f7971 | 226 25.00 0.00 0.00 0 0 0.000 0.000 | 239 3.00 -1016.00 0.00 0 0 0.000 0.000 |  | 13 -537.14 152.50 -72.856 -4.584
495 9e1f7971 | 226 26.00 0.00 0.00 0 0 0.000 0.000 | 237 0.00 -1016.00 0.00 0 0 0.000 0.000 |  | 13 -609.99 147.91 72.856 -4.584
496 ee5d3670 | 226 27.00 0.00 0.00 0 0 0.000 0.000 | 237 1.00 -1016.00 0.00 0 0 0.000 0.000 |  | 13 -537.14 143.33 72.856 -4.584
497 ee5d3670 | 226 28.00 0.00 0.00 0 0 0.000 0.000 | 237 2.00 -1016.00 0.00 0 0 0.000 0.000 |  | 13 -464.28 138.75 72.856 -4.584
498 ee5d3670 | 226 29.00 0.00 0.00 0 0 0.000 0.000 | 237 3.00 -1016.00 0.00 0 0 0.000 0.000 |  | 13 -391.42 134.16 72.856 -4.584
499 ee5d3670 | 226 30.00 0.00 0.00 0 0 0.000 0.000 | 237 4.00 -1016.00 0.00 0 0 0.000 0.000 |  | 13 -318.57 129.58 72.856 -4.584
500 7ff56027 | 39 1.00 0.00 0.00 8 7 9.044 51.289 | 237 5.00 -1016.00 0.00 0 0 0.000 0.000 | 20 -86.03 114.95 0.000 0.000
501 b01c0d7c | 39 1.00 0.00 0.00 11 6 9.044 51.289 | 239 0.00 -1016.00 0.00 0 0 0.000 0.000 | 20 -86.03 114.95 0.000 0.000
502 8bdb664f | 39 1.00 0.00 0.00 11 5 9.044 51.289 | 239 1.00 -1016.00 0.00 0 0 0.000 0.000 | 20 -86.03 114.50 0.000 -0.450
503 a350db7a | 39 1.00 0.00 0.00 11 4 9.044 51.289 | 239 2.00 -1016.00 0.00 0 0 0.000 0.000 | 20 -86.03 113.60 0.000 -0.900
504 4b40d5c4 | 39 1.00 0.00 0.00 11 3 9.044 51.289 | 239 3.00 -1016.00 0.00 0 0 0.000 0.000 | 20 -86.03 112.25 0.000 -1.350
505 8ddc2911 | 39 1.00 0.00 0.00 11 2 9.044 51.289 | 239 4.00 -1016.00 0.00 0 0 0.000 0.000 | 20 -86.03 110.45 0.000 -1.800
506 4cada790 | 39 1.00 0.00 0.00 11 1 9.044 51.289 | 239 0.00 -1016.00 0.00 0 0 0.000 0.000 | 20 -86.03 108.20 0.000 -2.250
507 2e407ec7 | 39 2.00 8.75 46.91 11 0 8.748 49.615 | 239 1.00 -1016.00 0.00 0 0 0.000 0.000 | 20 -86.03 105.50 0.000 -2.700
508 1366fe6e | 39 3.00 17.20 89.46 11 0 8.453 47.940 | 238 0.00 -1016.00 0.00 0 0 0.000 0.000 | 20 -86.03 102.35 0.000 -3.150
509 e6c46dbb | 39 4.00 25.36 127.62 11 0 8.158 46.266 | 238 1.00 -1016.00 0.00 0 0 0.000 0.000 | 20 -86.03 98.75 0.000 -3.600
510 bef6be20 | 39 5.00 33.22 161.41 11 0 7.863 44.592 | 238 2.00 -1016.00 0.00 0 0 0.000 0.000 | 20 -86.03 94.70 0.000 -4.050
511 73dadb49 | 39 6.00 40.79 190.83 11 0 7.568 42.918 | 238 3.00 -1016.00 0.00 0 0 0.000 0.000 | 20 -86.03 90.20 0.000 -4.500
512 58b070e8 | 39 7.00 48.06 215.88 11 0 7.272 41.244 | 238 4.00 -1016.00 0.00 0 0 0.000 0.000 | 20 -86.03 85.25 0.000 -4.950
513 890252dd | 39 8.00 55.04 236.54 11 0 6.977 39.570 | 238 5.00 -1016.00 0.00 0 0 0.000 0.000 | 20 -86.03 79.85 0.000 -5.400
514 fbb95a2c | 39 9.00 61.72 252.84 11 0 6.682 37.895 | 238 6.00 -1016.00 0.00 0 0 0.000 0.000 | 20 -86.03 74.00 0.000 -5.850
515 96c590c5 | 39 10.00 68.11 264.76 11 0 6.387 36.221 | 238 7.00 -1016.00 0.00 0 0 0.000 0.000 | 20 -86.03 67.70 0.000 -6.300
516 68994074 | 39 11.00 74.20 272.31 11 0 6.092 34.547 | 238 8.00 -1016.00 0.00 0 0 0.000 0.000 | 20 -86.03 60.95 0.000 -6.750
517 e0bf3b8f | 46 1.00 80.00 275.48 14 6 2.168 5.957 | 238 9.00 -1016.00 0.00 0 0 0.000 0.000 | 20 -86.03 53.75 0.000 -7.200
518 dc67e816 | 46 1.00 80.00 275.48 14 5 2.168 5.957 | 238 10.00 -1016.00 0.00 0 0 0.000 0.000 | 20 -86.03 46.10 0.000 -7.650
519 047eb795 | 46 1.00 80.00 275.48 14 4 2.168 5.957 | 238 11.00 -1016.00 0.00 0 0 0.000 0.000 | 20 -86.03 38.00 0.000 -8.100
520 c6576cc2 | 46 1.00 80.00 275.48 14 3 2.168 5.957 | 238 12.00 -1016.00 0.00 0 0 0.000 0.000 | 20 -86.03 29.45 0.000 -8.550
521 c89cec07 | 46 1.00 80.00 275.48 14 2 2.168 5.957 | 238 13.00 -1016.00 0.00 0 0 0.000 0.000 | 20 -86.03 20.45 0.000 -9.000
522 1916b6ae | 46 1.00 80.00 275.48 14 1 2.168 5.957 | 238 14.00 -1016.00 0.00 0 0 0.000 0.000 | 20 -86.03 11.00 0.000 -9.450
523 f1f07d4d | 46 2.00 81.58 277.14 14 0 1.587 4.359 | 238 15.00 -1016.00 0.00 0 0 0.000 0.000 | 20 -86.03 1.10 0.000 -9.900
524 83849adc | 46 3.00 82.59 274.50 14 0 1.005 2.762 | 238 16.00 -1016.00 0.00 0 0 0.000 0.000 | 20 -86.03 0.00 0.000 0.000
525 9953d157 | 46 4.00 83.01 267.57 14 0 0.424 1.164 | 238 17.00 -1016.00 0.00 0 0 0.000 0.000 | 20 -86.03 0.00 0.000 0.000
526 ebcb46be | 46 5.00 83.01 256.77 14 0 0.000 0.000 | 10 0.00 -1016.00 0.00 0 0 0.000 0.000 | 20 -86.03 0.00 0.000 0.000
527 0fc94289 | 46 6.00 83.01 243.27 14 0 0.000 0.000 | 10 1.00 -1016.00 0.00 0 0 0.000 0.000 | 20 -86.03 0.00 0.000 0.000
528 0e123b28 | 46 7.00 83.01 227.07 14 0 0.000 0.000 | 10 2.00 -1016.00 0.00 0 0 0.000 0.000 | 20 -86.03 0.00 0.000 0.000
529 7874fe1d | 46 8.00 83.01 208.17 14 0 0.000 0.000 | 10 3.00 -1016.00 0.00 0 0 0.000 0.000 | 20 -86.03 0.00 0.000 0.000
530 ccd4c0ea | 46 9.00 83.01 186.57 14 0 0.000 0.000 | 10 4.00 -1016.00 0.00 0 0 0.000 0.000 | 20 -86.03 0.00 0.000 0.000
531 5fa48405 | 46 10.00 83.01 162.27 14 0 0.000 0.000 | 10 5.00 -1016.00 0.00 0 0 0.000 0.000 | 20 -86.03 0.00 0.000 0.000
532 205966b4 | 46 11.00 83.01 135.27 14 0 0.000 0.000 | 10 6.00 -1016.00 0.00 0 0 0.000 0.000 | 20 -86.03 0.00 0.000 0.000
533 89753ad5 | 46 1.00 83.01 105.57 17 6 2.208 6.065 | 10 7.00 -1016.00 0.00 0 0 0.000 0.000 | 20 -86.03 0.00 0.000 0.000
534 37e68244 | 46 1.00 83.01 105.57 17 5 2.208 6.065 | 10 8.00 -1016.00 0.00 0 0 0.000 0.000 | 20 -86.03 0.00 0.000 0.000
535 fb448791 | 46 1.00 83.01 105.57 17 4 2.208 6.065 | 10 9.00 -1016.00 0.00 0 0 0.000 0.000 | 20 -86.03 0.00 0.000 0.000
536 d90e8c10 | 46 1.00 83.01 105.57 17 3 2.208 6.065 | 10 10.00 -1016.00 0.00 0 0 0.000 0.000 | 20 -86.03 0.00 0.000 0.000
537 ff619947 | 46 1.00 83.01 105.57 17 2 2.208 6.065 | 10 11.00 -1016.00 0.00 0 0 0.000 0.000 | 20 -86.03 0.00 0.000 0.000
538 ba8daeee | 46 1.00 83.01 105.57 17 1 2.208 6.065 | 10 12.00 -1016.00 0.00 0 0 0.000 0.000 | 20 -86.03 0.00 0.000 0.000
539 3807d88d | 46 2.00 84.64 107.33 17 0 1.626 4.468 | 10 13.00 -1016.00 0.00 0 0 0.000 0.000 | 20 -86.03 0.00 0.000 0.000
540 b492e61a | 46 3.00 85.68 104.80 17 0 1.045 2.870 | 10 14.00 -1016.00 0.00 0 0 0.000 0.000 | 20 -86.03 0.00 0.000 0.000
";

/// `vsr1`'s routes: P1 Ness's PK Fire, P2 Fox's drop, walk and reflector.
const VSR1_P1: &str = "470:3:b";
const VSR1_P2: &str = "405:3:y=-80;440:16:x=50;468:40:b+y=-80";

#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn a_reflected_pk_fire_burns_ness_as_on_the_n64() {
    let replay = Replay {
        p1: FighterKind::Ness,
        route: VSR1_P1,
        weapons: &ness_weapons,
    };
    let vs = Vs {
        p2: FighterKind::Fox,
        route2: VSR1_P2,
        spawns: VS_NESS_FOX,
    };
    let Some(out) = replay.run_vs(VS_PK_FIRE_REFLECTED, &vs) else {
        return;
    };
    out.check("Fox reflects Ness's PK Fire in VS");
    // The rows tell a reflect from none: with Fox's B released at 490
    // (no reflector when the spark arrives) the replay leaves them.
    let vs = Vs {
        route2: "405:3:y=-80;440:16:x=50;468:22:b+y=-80",
        ..vs
    };
    let out = replay.run_vs(VS_PK_FIRE_REFLECTED, &vs).unwrap();
    assert!(
        out.first_bad.is_some_and(|t| (480..=500).contains(&t)),
        "a replay without the reflector should leave the rows by 500, left at {:?}",
        out.first_bad
    );
}

/// `b36/vst1` (`vsness.z64`): Ness (P1) fires PK Thunder at t470 and
/// steers it left and down into Fox's reflector (P2, held from t468 at
/// x -1016). The head reaches it at t515 (Fox st 237): a new head, last
/// in the link, at the old one's place with velocity
/// `60 x norm(pos - (Ness + (0, 250, 0)))`; the old trails, marked, stay
/// listed that frame and end at their next update. The reflected head's
/// trails carry its velocity (`wpNessPKReflectTrailMakeWeapon`) and step
/// by it after their update. Every frame from Fox's first Wait frame.
const VS_PK_THUNDER_REFLECTED: &str = "
330 45099bec | 10 60.00 0.00 0.00 0 0 0.000 0.000 | 10 0.00 -1397.00 904.00 0 0 0.000 0.000
331 45099bec | 10 61.00 0.00 0.00 0 0 0.000 0.000 | 10 1.00 -1397.00 904.00 0 0 0.000 0.000
332 45099bec | 10 62.00 0.00 0.00 0 0 0.000 0.000 | 10 2.00 -1397.00 904.00 0 0 0.000 0.000
333 45099bec | 10 63.00 0.00 0.00 0 0 0.000 0.000 | 10 3.00 -1397.00 904.00 0 0 0.000 0.000
334 45099bec | 10 64.00 0.00 0.00 0 0 0.000 0.000 | 10 4.00 -1397.00 904.00 0 0 0.000 0.000
335 45099bec | 10 65.00 0.00 0.00 0 0 0.000 0.000 | 10 5.00 -1397.00 904.00 0 0 0.000 0.000
336 45099bec | 10 66.00 0.00 0.00 0 0 0.000 0.000 | 10 6.00 -1397.00 904.00 0 0 0.000 0.000
337 45099bec | 10 67.00 0.00 0.00 0 0 0.000 0.000 | 10 7.00 -1397.00 904.00 0 0 0.000 0.000
338 45099bec | 10 68.00 0.00 0.00 0 0 0.000 0.000 | 10 8.00 -1397.00 904.00 0 0 0.000 0.000
339 45099bec | 10 69.00 0.00 0.00 0 0 0.000 0.000 | 10 9.00 -1397.00 904.00 0 0 0.000 0.000
340 45099bec | 10 70.00 0.00 0.00 0 0 0.000 0.000 | 10 10.00 -1397.00 904.00 0 0 0.000 0.000
341 45099bec | 10 71.00 0.00 0.00 0 0 0.000 0.000 | 10 11.00 -1397.00 904.00 0 0 0.000 0.000
342 45099bec | 10 72.00 0.00 0.00 0 0 0.000 0.000 | 10 12.00 -1397.00 904.00 0 0 0.000 0.000
343 45099bec | 10 73.00 0.00 0.00 0 0 0.000 0.000 | 10 13.00 -1397.00 904.00 0 0 0.000 0.000
344 45099bec | 10 74.00 0.00 0.00 0 0 0.000 0.000 | 10 14.00 -1397.00 904.00 0 0 0.000 0.000
345 45099bec | 10 75.00 0.00 0.00 0 0 0.000 0.000 | 10 15.00 -1397.00 904.00 0 0 0.000 0.000
346 45099bec | 10 76.00 0.00 0.00 0 0 0.000 0.000 | 10 16.00 -1397.00 904.00 0 0 0.000 0.000
347 45099bec | 10 77.00 0.00 0.00 0 0 0.000 0.000 | 10 17.00 -1397.00 904.00 0 0 0.000 0.000
348 45099bec | 10 78.00 0.00 0.00 0 0 0.000 0.000 | 10 18.00 -1397.00 904.00 0 0 0.000 0.000
349 45099bec | 10 79.00 0.00 0.00 0 0 0.000 0.000 | 10 19.00 -1397.00 904.00 0 0 0.000 0.000
350 45099bec | 10 80.00 0.00 0.00 0 0 0.000 0.000 | 10 20.00 -1397.00 904.00 0 0 0.000 0.000
351 45099bec | 10 81.00 0.00 0.00 0 0 0.000 0.000 | 10 21.00 -1397.00 904.00 0 0 0.000 0.000
352 45099bec | 10 82.00 0.00 0.00 0 0 0.000 0.000 | 10 22.00 -1397.00 904.00 0 0 0.000 0.000
353 45099bec | 10 83.00 0.00 0.00 0 0 0.000 0.000 | 10 23.00 -1397.00 904.00 0 0 0.000 0.000
354 45099bec | 10 84.00 0.00 0.00 0 0 0.000 0.000 | 10 24.00 -1397.00 904.00 0 0 0.000 0.000
355 45099bec | 10 85.00 0.00 0.00 0 0 0.000 0.000 | 10 25.00 -1397.00 904.00 0 0 0.000 0.000
356 45099bec | 10 86.00 0.00 0.00 0 0 0.000 0.000 | 10 26.00 -1397.00 904.00 0 0 0.000 0.000
357 45099bec | 10 87.00 0.00 0.00 0 0 0.000 0.000 | 10 27.00 -1397.00 904.00 0 0 0.000 0.000
358 45099bec | 10 88.00 0.00 0.00 0 0 0.000 0.000 | 10 28.00 -1397.00 904.00 0 0 0.000 0.000
359 45099bec | 10 89.00 0.00 0.00 0 0 0.000 0.000 | 10 29.00 -1397.00 904.00 0 0 0.000 0.000
360 45099bec | 10 90.00 0.00 0.00 0 0 0.000 0.000 | 10 30.00 -1397.00 904.00 0 0 0.000 0.000
361 45099bec | 10 91.00 0.00 0.00 0 0 0.000 0.000 | 10 31.00 -1397.00 904.00 0 0 0.000 0.000
362 45099bec | 10 92.00 0.00 0.00 0 0 0.000 0.000 | 10 32.00 -1397.00 904.00 0 0 0.000 0.000
363 45099bec | 10 93.00 0.00 0.00 0 0 0.000 0.000 | 10 33.00 -1397.00 904.00 0 0 0.000 0.000
364 45099bec | 10 94.00 0.00 0.00 0 0 0.000 0.000 | 10 34.00 -1397.00 904.00 0 0 0.000 0.000
365 45099bec | 10 95.00 0.00 0.00 0 0 0.000 0.000 | 10 35.00 -1397.00 904.00 0 0 0.000 0.000
366 45099bec | 10 96.00 0.00 0.00 0 0 0.000 0.000 | 10 36.00 -1397.00 904.00 0 0 0.000 0.000
367 45099bec | 10 97.00 0.00 0.00 0 0 0.000 0.000 | 10 37.00 -1397.00 904.00 0 0 0.000 0.000
368 45099bec | 10 98.00 0.00 0.00 0 0 0.000 0.000 | 10 38.00 -1397.00 904.00 0 0 0.000 0.000
369 45099bec | 10 99.00 0.00 0.00 0 0 0.000 0.000 | 10 39.00 -1397.00 904.00 0 0 0.000 0.000
370 45099bec | 10 100.00 0.00 0.00 0 0 0.000 0.000 | 10 40.00 -1397.00 904.00 0 0 0.000 0.000
371 45099bec | 10 101.00 0.00 0.00 0 0 0.000 0.000 | 10 41.00 -1397.00 904.00 0 0 0.000 0.000
372 45099bec | 10 102.00 0.00 0.00 0 0 0.000 0.000 | 10 42.00 -1397.00 904.00 0 0 0.000 0.000
373 45099bec | 10 103.00 0.00 0.00 0 0 0.000 0.000 | 10 43.00 -1397.00 904.00 0 0 0.000 0.000
374 45099bec | 10 104.00 0.00 0.00 0 0 0.000 0.000 | 10 44.00 -1397.00 904.00 0 0 0.000 0.000
375 45099bec | 10 105.00 0.00 0.00 0 0 0.000 0.000 | 10 45.00 -1397.00 904.00 0 0 0.000 0.000
376 45099bec | 10 106.00 0.00 0.00 0 0 0.000 0.000 | 10 46.00 -1397.00 904.00 0 0 0.000 0.000
377 45099bec | 10 107.00 0.00 0.00 0 0 0.000 0.000 | 10 47.00 -1397.00 904.00 0 0 0.000 0.000
378 45099bec | 10 108.00 0.00 0.00 0 0 0.000 0.000 | 10 48.00 -1397.00 904.00 0 0 0.000 0.000
379 45099bec | 10 109.00 0.00 0.00 0 0 0.000 0.000 | 10 49.00 -1397.00 904.00 0 0 0.000 0.000
380 45099bec | 10 110.00 0.00 0.00 0 0 0.000 0.000 | 10 50.00 -1397.00 904.00 0 0 0.000 0.000
381 45099bec | 10 111.00 0.00 0.00 0 0 0.000 0.000 | 10 51.00 -1397.00 904.00 0 0 0.000 0.000
382 45099bec | 10 112.00 0.00 0.00 0 0 0.000 0.000 | 10 52.00 -1397.00 904.00 0 0 0.000 0.000
383 45099bec | 10 113.00 0.00 0.00 0 0 0.000 0.000 | 10 53.00 -1397.00 904.00 0 0 0.000 0.000
384 45099bec | 10 114.00 0.00 0.00 0 0 0.000 0.000 | 10 54.00 -1397.00 904.00 0 0 0.000 0.000
385 45099bec | 10 115.00 0.00 0.00 0 0 0.000 0.000 | 10 55.00 -1397.00 904.00 0 0 0.000 0.000
386 45099bec | 10 116.00 0.00 0.00 0 0 0.000 0.000 | 10 56.00 -1397.00 904.00 0 0 0.000 0.000
387 45099bec | 10 117.00 0.00 0.00 0 0 0.000 0.000 | 10 57.00 -1397.00 904.00 0 0 0.000 0.000
388 45099bec | 10 118.00 0.00 0.00 0 0 0.000 0.000 | 10 58.00 -1397.00 904.00 0 0 0.000 0.000
389 45099bec | 10 119.00 0.00 0.00 0 0 0.000 0.000 | 10 59.00 -1397.00 904.00 0 0 0.000 0.000
390 45099bec | 10 0.00 0.00 0.00 0 0 0.000 0.000 | 10 60.00 -1397.00 904.00 0 0 0.000 0.000
391 45099bec | 10 1.00 0.00 0.00 0 0 0.000 0.000 | 10 61.00 -1397.00 904.00 0 0 0.000 0.000
392 45099bec | 10 2.00 0.00 0.00 0 0 0.000 0.000 | 10 62.00 -1397.00 904.00 0 0 0.000 0.000
393 45099bec | 10 3.00 0.00 0.00 0 0 0.000 0.000 | 10 63.00 -1397.00 904.00 0 0 0.000 0.000
394 45099bec | 10 4.00 0.00 0.00 0 0 0.000 0.000 | 10 64.00 -1397.00 904.00 0 0 0.000 0.000
395 92387aff | 10 5.00 0.00 0.00 0 0 0.000 0.000 | 10 65.00 -1397.00 904.00 0 0 0.000 0.000
396 92387aff | 10 6.00 0.00 0.00 0 0 0.000 0.000 | 10 66.00 -1397.00 904.00 0 0 0.000 0.000
397 92387aff | 10 7.00 0.00 0.00 0 0 0.000 0.000 | 10 67.00 -1397.00 904.00 0 0 0.000 0.000
398 92387aff | 10 8.00 0.00 0.00 0 0 0.000 0.000 | 10 68.00 -1397.00 904.00 0 0 0.000 0.000
399 92387aff | 10 9.00 0.00 0.00 0 0 0.000 0.000 | 10 69.00 -1397.00 904.00 0 0 0.000 0.000
400 92387aff | 10 10.00 0.00 0.00 0 0 0.000 0.000 | 10 0.00 -1397.00 904.00 0 0 0.000 0.000
401 92387aff | 10 11.00 0.00 0.00 0 0 0.000 0.000 | 10 1.00 -1397.00 904.00 0 0 0.000 0.000
402 92387aff | 10 12.00 0.00 0.00 0 0 0.000 0.000 | 10 2.00 -1397.00 904.00 0 0 0.000 0.000
403 92387aff | 10 13.00 0.00 0.00 0 0 0.000 0.000 | 10 3.00 -1397.00 904.00 0 0 0.000 0.000
404 92387aff | 10 14.00 0.00 0.00 0 0 0.000 0.000 | 10 4.00 -1397.00 904.00 0 0 0.000 0.000
405 92387aff | 10 15.00 0.00 0.00 0 0 0.000 0.000 | 28 1.00 -1397.00 904.00 0 0 0.000 0.000
406 92387aff | 10 16.00 0.00 0.00 0 0 0.000 0.000 | 28 2.00 -1397.00 904.00 0 0 0.000 0.000
407 92387aff | 10 17.00 0.00 0.00 0 0 0.000 0.000 | 28 3.00 -1397.00 904.00 0 0 0.000 0.000
408 92387aff | 10 18.00 0.00 0.00 0 0 0.000 0.000 | 33 1.00 -1397.00 900.00 0 0 0.000 0.000
409 92387aff | 10 19.00 0.00 0.00 0 0 0.000 0.000 | 33 2.00 -1397.00 892.00 0 0 0.000 0.000
410 92387aff | 10 20.00 0.00 0.00 0 0 0.000 0.000 | 33 3.00 -1397.00 880.00 0 0 0.000 0.000
411 92387aff | 10 21.00 0.00 0.00 0 0 0.000 0.000 | 33 4.00 -1397.00 864.00 0 0 0.000 0.000
412 92387aff | 10 22.00 0.00 0.00 0 0 0.000 0.000 | 33 5.00 -1397.00 844.00 0 0 0.000 0.000
413 92387aff | 10 23.00 0.00 0.00 0 0 0.000 0.000 | 33 6.00 -1397.00 820.00 0 0 0.000 0.000
414 92387aff | 10 24.00 0.00 0.00 0 0 0.000 0.000 | 33 7.00 -1397.00 792.00 0 0 0.000 0.000
415 92387aff | 10 25.00 0.00 0.00 0 0 0.000 0.000 | 33 8.00 -1397.00 760.00 0 0 0.000 0.000
416 92387aff | 10 26.00 0.00 0.00 0 0 0.000 0.000 | 33 9.00 -1397.00 724.00 0 0 0.000 0.000
417 92387aff | 10 27.00 0.00 0.00 0 0 0.000 0.000 | 33 10.00 -1397.00 684.00 0 0 0.000 0.000
418 92387aff | 10 28.00 0.00 0.00 0 0 0.000 0.000 | 33 11.00 -1397.00 640.00 0 0 0.000 0.000
419 92387aff | 10 29.00 0.00 0.00 0 0 0.000 0.000 | 33 12.00 -1397.00 592.00 0 0 0.000 0.000
420 92387aff | 10 30.00 0.00 0.00 0 0 0.000 0.000 | 33 13.00 -1397.00 540.00 0 0 0.000 0.000
421 92387aff | 10 31.00 0.00 0.00 0 0 0.000 0.000 | 33 14.00 -1397.00 484.00 0 0 0.000 0.000
422 92387aff | 10 32.00 0.00 0.00 0 0 0.000 0.000 | 33 15.00 -1397.00 424.00 0 0 0.000 0.000
423 92387aff | 10 33.00 0.00 0.00 0 0 0.000 0.000 | 33 16.00 -1397.00 364.00 0 0 0.000 0.000
424 92387aff | 10 34.00 0.00 0.00 0 0 0.000 0.000 | 33 17.00 -1397.00 304.00 0 0 0.000 0.000
425 92387aff | 10 35.00 0.00 0.00 0 0 0.000 0.000 | 33 18.00 -1397.00 244.00 0 0 0.000 0.000
426 92387aff | 10 36.00 0.00 0.00 0 0 0.000 0.000 | 33 19.00 -1397.00 184.00 0 0 0.000 0.000
427 92387aff | 10 37.00 0.00 0.00 0 0 0.000 0.000 | 33 20.00 -1397.00 124.00 0 0 0.000 0.000
428 92387aff | 10 38.00 0.00 0.00 0 0 0.000 0.000 | 33 21.00 -1397.00 64.00 0 0 0.000 0.000
429 92387aff | 10 39.00 0.00 0.00 0 0 0.000 0.000 | 33 22.00 -1397.00 4.00 0 0 0.000 0.000
430 9e1f7971 | 10 40.00 0.00 0.00 0 0 0.000 0.000 | 31 0.00 -1397.00 0.00 0 0 0.000 0.000
431 9e1f7971 | 10 41.00 0.00 0.00 0 0 0.000 0.000 | 31 1.00 -1397.00 0.00 0 0 0.000 0.000
432 9e1f7971 | 10 42.00 0.00 0.00 0 0 0.000 0.000 | 31 2.00 -1397.00 0.00 0 0 0.000 0.000
433 9e1f7971 | 10 43.00 0.00 0.00 0 0 0.000 0.000 | 31 3.00 -1397.00 0.00 0 0 0.000 0.000
434 9e1f7971 | 10 44.00 0.00 0.00 0 0 0.000 0.000 | 31 4.00 -1397.00 0.00 0 0 0.000 0.000
435 9e1f7971 | 10 45.00 0.00 0.00 0 0 0.000 0.000 | 31 5.00 -1397.00 0.00 0 0 0.000 0.000
436 9e1f7971 | 10 46.00 0.00 0.00 0 0 0.000 0.000 | 31 6.00 -1397.00 0.00 0 0 0.000 0.000
437 9e1f7971 | 10 47.00 0.00 0.00 0 0 0.000 0.000 | 31 7.00 -1397.00 0.00 0 0 0.000 0.000
438 9e1f7971 | 10 48.00 0.00 0.00 0 0 0.000 0.000 | 10 0.00 -1397.00 0.00 0 0 0.000 0.000
439 9e1f7971 | 10 49.00 0.00 0.00 0 0 0.000 0.000 | 10 1.00 -1397.00 0.00 0 0 0.000 0.000
440 9e1f7971 | 10 50.00 0.00 0.00 0 0 0.000 0.000 | 12 1.00 -1374.50 0.00 0 0 0.000 0.000
441 9e1f7971 | 10 51.00 0.00 0.00 0 0 0.000 0.000 | 12 2.00 -1352.00 0.00 0 0 0.000 0.000
442 9e1f7971 | 10 52.00 0.00 0.00 0 0 0.000 0.000 | 12 3.00 -1329.50 0.00 0 0 0.000 0.000
443 9e1f7971 | 10 53.00 0.00 0.00 0 0 0.000 0.000 | 12 4.00 -1307.00 0.00 0 0 0.000 0.000
444 9e1f7971 | 10 54.00 0.00 0.00 0 0 0.000 0.000 | 12 5.00 -1284.50 0.00 0 0 0.000 0.000
445 9e1f7971 | 10 55.00 0.00 0.00 0 0 0.000 0.000 | 12 6.00 -1262.00 0.00 0 0 0.000 0.000
446 9e1f7971 | 10 56.00 0.00 0.00 0 0 0.000 0.000 | 12 7.00 -1239.50 0.00 0 0 0.000 0.000
447 9e1f7971 | 10 57.00 0.00 0.00 0 0 0.000 0.000 | 12 8.00 -1217.00 0.00 0 0 0.000 0.000
448 9e1f7971 | 10 58.00 0.00 0.00 0 0 0.000 0.000 | 12 9.00 -1194.50 0.00 0 0 0.000 0.000
449 9e1f7971 | 10 59.00 0.00 0.00 0 0 0.000 0.000 | 12 10.00 -1172.00 0.00 0 0 0.000 0.000
450 9e1f7971 | 10 60.00 0.00 0.00 0 0 0.000 0.000 | 12 11.00 -1149.50 0.00 0 0 0.000 0.000
451 9e1f7971 | 10 61.00 0.00 0.00 0 0 0.000 0.000 | 12 12.00 -1127.00 0.00 0 0 0.000 0.000
452 9e1f7971 | 10 62.00 0.00 0.00 0 0 0.000 0.000 | 12 13.00 -1104.50 0.00 0 0 0.000 0.000
453 9e1f7971 | 10 63.00 0.00 0.00 0 0 0.000 0.000 | 12 14.00 -1082.00 0.00 0 0 0.000 0.000
454 9e1f7971 | 10 64.00 0.00 0.00 0 0 0.000 0.000 | 12 15.00 -1059.50 0.00 0 0 0.000 0.000
455 9e1f7971 | 10 65.00 0.00 0.00 0 0 0.000 0.000 | 12 16.00 -1037.00 0.00 0 0 0.000 0.000
456 9e1f7971 | 10 66.00 0.00 0.00 0 0 0.000 0.000 | 10 0.00 -1022.50 0.00 0 0 0.000 0.000
457 9e1f7971 | 10 67.00 0.00 0.00 0 0 0.000 0.000 | 10 1.00 -1016.00 0.00 0 0 0.000 0.000
458 9e1f7971 | 10 68.00 0.00 0.00 0 0 0.000 0.000 | 10 2.00 -1016.00 0.00 0 0 0.000 0.000
459 9e1f7971 | 10 69.00 0.00 0.00 0 0 0.000 0.000 | 10 3.00 -1016.00 0.00 0 0 0.000 0.000
460 9e1f7971 | 10 70.00 0.00 0.00 0 0 0.000 0.000 | 10 4.00 -1016.00 0.00 0 0 0.000 0.000
461 9e1f7971 | 10 71.00 0.00 0.00 0 0 0.000 0.000 | 10 5.00 -1016.00 0.00 0 0 0.000 0.000
462 9e1f7971 | 10 72.00 0.00 0.00 0 0 0.000 0.000 | 10 6.00 -1016.00 0.00 0 0 0.000 0.000
463 9e1f7971 | 10 73.00 0.00 0.00 0 0 0.000 0.000 | 10 7.00 -1016.00 0.00 0 0 0.000 0.000
464 9e1f7971 | 10 74.00 0.00 0.00 0 0 0.000 0.000 | 10 8.00 -1016.00 0.00 0 0 0.000 0.000
465 9e1f7971 | 10 75.00 0.00 0.00 0 0 0.000 0.000 | 10 9.00 -1016.00 0.00 0 0 0.000 0.000
466 9e1f7971 | 10 76.00 0.00 0.00 0 0 0.000 0.000 | 10 10.00 -1016.00 0.00 0 0 0.000 0.000
467 9e1f7971 | 10 77.00 0.00 0.00 0 0 0.000 0.000 | 10 11.00 -1016.00 0.00 0 0 0.000 0.000
468 9e1f7971 | 10 78.00 0.00 0.00 0 0 0.000 0.000 | 236 1.00 -1016.00 0.00 0 0 0.000 0.000
469 9e1f7971 | 10 79.00 0.00 0.00 0 0 0.000 0.000 | 236 2.00 -1016.00 0.00 0 0 0.000 0.000
470 9e1f7971 | 228 1.00 0.00 0.00 0 0 0.000 0.000 | 236 3.00 -1016.00 0.00 0 0 0.000 0.000
471 9e1f7971 | 228 2.00 0.00 0.00 0 0 0.000 0.000 | 239 0.00 -1016.00 0.00 0 0 0.000 0.000
472 9e1f7971 | 228 3.00 0.00 0.00 0 0 0.000 0.000 | 239 1.00 -1016.00 0.00 0 0 0.000 0.000
473 9e1f7971 | 228 4.00 0.00 0.00 0 0 0.000 0.000 | 239 2.00 -1016.00 0.00 0 0 0.000 0.000
474 9e1f7971 | 228 5.00 0.00 0.00 0 0 0.000 0.000 | 239 3.00 -1016.00 0.00 0 0 0.000 0.000
475 9e1f7971 | 228 6.00 0.00 0.00 0 0 0.000 0.000 | 239 4.00 -1016.00 0.00 0 0 0.000 0.000
476 9e1f7971 | 228 7.00 0.00 0.00 0 0 0.000 0.000 | 239 0.00 -1016.00 0.00 0 0 0.000 0.000
477 9e1f7971 | 228 8.00 0.00 0.00 0 0 0.000 0.000 | 239 1.00 -1016.00 0.00 0 0 0.000 0.000
478 9e1f7971 | 228 9.00 0.00 0.00 0 0 0.000 0.000 | 239 2.00 -1016.00 0.00 0 0 0.000 0.000
479 9e1f7971 | 228 10.00 0.00 0.00 0 0 0.000 0.000 | 239 3.00 -1016.00 0.00 0 0 0.000 0.000
480 9e1f7971 | 228 11.00 0.00 0.00 0 0 0.000 0.000 | 239 4.00 -1016.00 0.00 0 0 0.000 0.000
481 9e1f7971 | 228 12.00 0.00 0.00 0 0 0.000 0.000 | 239 0.00 -1016.00 0.00 0 0 0.000 0.000
482 9e1f7971 | 228 13.00 0.00 0.00 0 0 0.000 0.000 | 239 1.00 -1016.00 0.00 0 0 0.000 0.000
483 9e1f7971 | 228 14.00 0.00 0.00 0 0 0.000 0.000 | 239 2.00 -1016.00 0.00 0 0 0.000 0.000
484 9e1f7971 | 228 15.00 0.00 0.00 0 0 0.000 0.000 | 239 3.00 -1016.00 0.00 0 0 0.000 0.000
485 9e1f7971 | 228 16.00 0.00 0.00 0 0 0.000 0.000 | 239 4.00 -1016.00 0.00 0 0 0.000 0.000
486 9e1f7971 | 228 17.00 0.00 0.00 0 0 0.000 0.000 | 239 0.00 -1016.00 0.00 0 0 0.000 0.000
487 9e1f7971 | 228 18.00 0.00 0.00 0 0 0.000 0.000 | 239 1.00 -1016.00 0.00 0 0 0.000 0.000
488 9e1f7971 | 228 19.00 0.00 0.00 0 0 0.000 0.000 | 239 2.00 -1016.00 0.00 0 0 0.000 0.000
489 9e1f7971 | 228 20.00 0.00 0.00 0 0 0.000 0.000 | 239 3.00 -1016.00 0.00 0 0 0.000 0.000
490 9e1f7971 | 228 21.00 0.00 0.00 0 0 0.000 0.000 | 239 4.00 -1016.00 0.00 0 0 0.000 0.000
491 9e1f7971 | 228 22.00 0.00 0.00 0 0 0.000 0.000 | 239 0.00 -1016.00 0.00 0 0 0.000 0.000
492 9e1f7971 | 228 23.00 0.00 0.00 0 0 0.000 0.000 | 239 1.00 -1016.00 0.00 0 0 0.000 0.000
493 9e1f7971 | 229 1.00 0.00 0.00 0 0 0.000 0.000 | 239 2.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -23.34 250.22 -6.272 59.671
494 9e1f7971 | 229 2.00 0.00 0.00 0 0 0.000 0.000 | 239 3.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -35.82 308.91 -12.475 58.689
495 ee5d3670 | 229 3.00 0.00 0.00 0 0 0.000 0.000 | 239 4.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -54.36 365.98 -18.541 57.063 ; 15 -23.00 250.00 0.000 0.000
496 9ad4bb73 | 229 4.00 0.00 0.00 0 0 0.000 0.000 | 239 0.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -78.76 420.79 -24.404 54.813 ; 15 -35.00 308.00 0.000 0.000
497 6bfcdd85 | 229 5.00 0.00 0.00 0 0 0.000 0.000 | 239 1.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -108.76 472.75 -30.000 51.962 ; 15 -54.00 365.00 0.000 0.000 ; 15 -23.00 250.00 0.000 0.000
498 7ff56027 | 229 6.00 0.00 0.00 0 0 0.000 0.000 | 239 2.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -144.03 521.29 -35.267 48.541 ; 15 -78.00 420.00 0.000 0.000 ; 15 -35.00 308.00 0.000 0.000
499 2213eb38 | 229 7.00 0.00 0.00 0 0 0.000 0.000 | 239 3.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -184.18 565.88 -40.148 44.589 ; 15 -108.00 472.00 0.000 0.000 ; 15 -54.00 365.00 0.000 0.000 ; 15 -23.00 250.00 0.000 0.000
500 830a2c6d | 229 8.00 0.00 0.00 0 0 0.000 0.000 | 239 4.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -228.77 606.03 -44.589 40.148 ; 15 -144.00 521.00 0.000 0.000 ; 15 -78.00 420.00 0.000 0.000 ; 15 -35.00 308.00 0.000 0.000
501 4b40d5c4 | 229 9.00 0.00 0.00 0 0 0.000 0.000 | 239 0.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -277.31 641.30 -48.541 35.267 ; 15 -184.00 565.00 0.000 0.000 ; 15 -108.00 472.00 0.000 0.000 ; 15 -54.00 365.00 0.000 0.000 ; 15 -23.00 250.00 0.000 0.000
502 3e1203c8 | 229 0.00 0.00 0.00 0 0 0.000 0.000 | 239 1.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -329.27 671.30 -51.962 30.000 ; 15 -228.00 606.00 0.000 0.000 ; 15 -144.00 521.00 0.000 0.000 ; 15 -78.00 420.00 0.000 0.000 ; 15 -35.00 308.00 0.000 0.000
503 ba9ed30c | 229 1.00 0.00 0.00 0 0 0.000 0.000 | 239 2.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -384.08 695.70 -54.813 24.404 ; 15 -277.00 641.00 0.000 0.000 ; 15 -184.00 565.00 0.000 0.000 ; 15 -108.00 472.00 0.000 0.000 ; 15 -54.00 365.00 0.000 0.000
504 4cada790 | 229 2.00 0.00 0.00 0 0 0.000 0.000 | 239 3.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -441.15 714.24 -57.063 18.541 ; 15 -329.00 671.00 0.000 0.000 ; 15 -228.00 606.00 0.000 0.000 ; 15 -144.00 521.00 0.000 0.000 ; 15 -78.00 420.00 0.000 0.000
505 c9442554 | 229 3.00 0.00 0.00 0 0 0.000 0.000 | 239 4.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -499.82 726.78 -58.676 12.534 ; 15 -384.00 695.00 0.000 0.000 ; 15 -277.00 641.00 0.000 0.000 ; 15 -184.00 565.00 0.000 0.000 ; 15 -108.00 472.00 0.000 0.000
506 b66b3058 | 229 4.00 0.00 0.00 0 0 0.000 0.000 | 239 0.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -559.39 733.99 -59.564 7.217 ; 15 -441.00 714.00 0.000 0.000 ; 15 -329.00 671.00 0.000 0.000 ; 15 -228.00 606.00 0.000 0.000 ; 15 -144.00 521.00 0.000 0.000
507 6032ec9c | 229 5.00 0.00 0.00 0 0 0.000 0.000 | 239 1.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -619.33 736.55 -59.945 2.560 ; 15 -499.00 726.00 0.000 0.000 ; 15 -384.00 695.00 0.000 0.000 ; 15 -277.00 641.00 0.000 0.000 ; 15 -184.00 565.00 0.000 0.000
508 bef6be20 | 229 6.00 0.00 0.00 0 0 0.000 0.000 | 239 2.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -679.31 735.06 -59.981 -1.490 ; 15 -559.00 733.00 0.000 0.000 ; 15 -441.00 714.00 0.000 0.000 ; 15 -329.00 671.00 0.000 0.000 ; 15 -228.00 606.00 0.000 0.000
509 26caec0b | 229 7.00 0.00 0.00 0 0 0.000 0.000 | 239 3.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -739.10 730.07 -59.792 -4.995 ; 15 -619.00 736.00 0.000 0.000 ; 15 -499.00 726.00 0.000 0.000 ; 15 -384.00 695.00 0.000 0.000 ; 15 -277.00 641.00 0.000 0.000
510 5772403f | 229 8.00 0.00 0.00 0 0 0.000 0.000 | 239 4.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -798.57 722.05 -59.462 -8.019 ; 15 -679.00 735.00 0.000 0.000 ; 15 -559.00 733.00 0.000 0.000 ; 15 -441.00 714.00 0.000 0.000 ; 15 -329.00 671.00 0.000 0.000
511 ca8024b3 | 229 9.00 0.00 0.00 0 0 0.000 0.000 | 239 0.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -857.62 711.43 -59.052 -10.623 ; 15 -739.00 730.00 0.000 0.000 ; 15 -619.00 736.00 0.000 0.000 ; 15 -499.00 726.00 0.000 0.000 ; 15 -384.00 695.00 0.000 0.000
512 3cd5ad67 | 229 0.00 0.00 0.00 0 0 0.000 0.000 | 239 1.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -916.22 698.56 -58.605 -12.864 ; 15 -798.00 722.00 0.000 0.000 ; 15 -679.00 735.00 0.000 0.000 ; 15 -559.00 733.00 0.000 0.000 ; 15 -441.00 714.00 0.000 0.000
513 b3ea2e5b | 229 1.00 0.00 0.00 0 0 0.000 0.000 | 239 2.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -974.37 683.77 -58.148 -14.791 ; 15 -857.00 711.00 0.000 0.000 ; 15 -739.00 730.00 0.000 0.000 ; 15 -619.00 736.00 0.000 0.000 ; 15 -499.00 726.00 0.000 0.000
514 e0bf3b8f | 229 2.00 0.00 0.00 0 0 0.000 0.000 | 239 3.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1032.07 667.32 -57.701 -16.449 ; 15 -916.00 698.00 0.000 0.000 ; 15 -798.00 722.00 0.000 0.000 ; 15 -679.00 735.00 0.000 0.000 ; 15 -559.00 733.00 0.000 0.000
515 af14a903 | 229 3.00 0.00 0.00 0 0 0.000 0.000 | 237 0.00 -1016.00 0.00 0 0 0.000 0.000 |  | 15 -974.00 683.00 0.000 0.000 ; 15 -857.00 711.00 0.000 0.000 ; 15 -739.00 730.00 0.000 0.000 ; 15 -619.00 736.00 0.000 0.000 ; 14 -1089.35 649.45 -10.836 59.013
516 6ad66fba | 229 4.00 0.00 0.00 0 0 0.000 0.000 | 237 1.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1100.18 708.46 -10.836 59.013
517 8d66c6e9 | 229 5.00 0.00 0.00 0 0 0.000 0.000 | 237 2.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1111.02 767.47 -10.836 59.013
518 f9832e08 | 229 6.00 0.00 0.00 0 0 0.000 0.000 | 237 3.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1121.86 826.49 -10.836 59.013 ; 15 -1100.18 708.46 -10.836 59.013
519 75ef34ab | 229 7.00 0.00 0.00 0 0 0.000 0.000 | 237 4.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1132.69 885.50 -10.836 59.013 ; 15 -1111.02 767.47 -10.836 59.013
520 bc61314c | 229 8.00 0.00 0.00 0 0 0.000 0.000 | 237 5.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1143.53 944.51 -10.836 59.013 ; 15 -1121.86 826.49 -10.836 59.013 ; 15 -1100.18 708.46 -10.836 59.013
521 ec692a26 | 229 9.00 0.00 0.00 0 0 0.000 0.000 | 239 0.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1154.37 1003.53 -10.836 59.013 ; 15 -1132.69 885.50 -10.836 59.013 ; 15 -1111.02 767.47 -10.836 59.013
522 22cd7153 | 229 0.00 0.00 0.00 0 0 0.000 0.000 | 239 1.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1165.20 1062.54 -10.836 59.013 ; 15 -1143.53 944.51 -10.836 59.013 ; 15 -1121.86 826.49 -10.836 59.013 ; 15 -1100.18 708.46 -10.836 59.013
523 5e9dab94 | 229 1.00 0.00 0.00 0 0 0.000 0.000 | 239 2.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1176.04 1121.55 -10.836 59.013 ; 15 -1154.37 1003.53 -10.836 59.013 ; 15 -1132.69 885.50 -10.836 59.013 ; 15 -1111.02 767.47 -10.836 59.013
524 b2e6fefb | 229 2.00 0.00 0.00 0 0 0.000 0.000 | 239 3.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1186.88 1180.57 -10.836 59.013 ; 15 -1165.20 1062.54 -10.836 59.013 ; 15 -1143.53 944.51 -10.836 59.013 ; 15 -1121.86 826.49 -10.836 59.013 ; 15 -1100.18 708.46 -10.836 59.013
525 ebe871da | 229 3.00 0.00 0.00 0 0 0.000 0.000 | 239 4.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1197.71 1239.58 -10.836 59.013 ; 15 -1176.04 1121.55 -10.836 59.013 ; 15 -1154.37 1003.53 -10.836 59.013 ; 15 -1132.69 885.50 -10.836 59.013 ; 15 -1111.02 767.47 -10.836 59.013
526 ebcb46be | 229 4.00 0.00 0.00 0 0 0.000 0.000 | 239 0.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1208.55 1298.59 -10.836 59.013 ; 15 -1186.88 1180.57 -10.836 59.013 ; 15 -1165.20 1062.54 -10.836 59.013 ; 15 -1143.53 944.51 -10.836 59.013 ; 15 -1121.86 826.49 -10.836 59.013
527 a833e2e2 | 229 5.00 0.00 0.00 0 0 0.000 0.000 | 239 1.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1219.38 1357.61 -10.836 59.013 ; 15 -1197.71 1239.58 -10.836 59.013 ; 15 -1176.04 1121.55 -10.836 59.013 ; 15 -1154.37 1003.53 -10.836 59.013 ; 15 -1132.69 885.50 -10.836 59.013
528 91f48a46 | 229 6.00 0.00 0.00 0 0 0.000 0.000 | 239 2.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1230.22 1416.62 -10.836 59.013 ; 15 -1208.55 1298.59 -10.836 59.013 ; 15 -1186.88 1180.57 -10.836 59.013 ; 15 -1165.20 1062.54 -10.836 59.013 ; 15 -1143.53 944.51 -10.836 59.013
529 ccd4c0ea | 229 7.00 0.00 0.00 0 0 0.000 0.000 | 239 3.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1241.06 1475.63 -10.836 59.013 ; 15 -1219.38 1357.61 -10.836 59.013 ; 15 -1197.71 1239.58 -10.836 59.013 ; 15 -1176.04 1121.55 -10.836 59.013 ; 15 -1154.37 1003.53 -10.836 59.013
530 80354ace | 229 8.00 0.00 0.00 0 0 0.000 0.000 | 239 4.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1251.89 1534.65 -10.836 59.013 ; 15 -1230.22 1416.62 -10.836 59.013 ; 15 -1208.55 1298.59 -10.836 59.013 ; 15 -1186.88 1180.57 -10.836 59.013 ; 15 -1165.20 1062.54 -10.836 59.013
531 9af42bf2 | 229 9.00 0.00 0.00 0 0 0.000 0.000 | 239 0.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1262.73 1593.66 -10.836 59.013 ; 15 -1241.06 1475.63 -10.836 59.013 ; 15 -1219.38 1357.61 -10.836 59.013 ; 15 -1197.71 1239.58 -10.836 59.013 ; 15 -1176.04 1121.55 -10.836 59.013
532 0e90a856 | 229 0.00 0.00 0.00 0 0 0.000 0.000 | 239 1.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1273.57 1652.67 -10.836 59.013 ; 15 -1251.89 1534.65 -10.836 59.013 ; 15 -1230.22 1416.62 -10.836 59.013 ; 15 -1208.55 1298.59 -10.836 59.013 ; 15 -1186.88 1180.57 -10.836 59.013
533 a52a4e29 | 229 1.00 0.00 0.00 0 0 0.000 0.000 | 239 2.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1284.40 1711.69 -10.836 59.013 ; 15 -1262.73 1593.66 -10.836 59.013 ; 15 -1241.06 1475.63 -10.836 59.013 ; 15 -1219.38 1357.61 -10.836 59.013 ; 15 -1197.71 1239.58 -10.836 59.013
534 c42babbd | 229 2.00 0.00 0.00 0 0 0.000 0.000 | 239 3.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1295.24 1770.70 -10.836 59.013 ; 15 -1273.57 1652.67 -10.836 59.013 ; 15 -1251.89 1534.65 -10.836 59.013 ; 15 -1230.22 1416.62 -10.836 59.013 ; 15 -1208.55 1298.59 -10.836 59.013
535 fb448791 | 229 3.00 0.00 0.00 0 0 0.000 0.000 | 239 4.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1306.07 1829.71 -10.836 59.013 ; 15 -1284.40 1711.69 -10.836 59.013 ; 15 -1262.73 1593.66 -10.836 59.013 ; 15 -1241.06 1475.63 -10.836 59.013 ; 15 -1219.38 1357.61 -10.836 59.013
536 2b0055a5 | 229 4.00 0.00 0.00 0 0 0.000 0.000 | 239 0.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1316.91 1888.73 -10.836 59.013 ; 15 -1295.24 1770.70 -10.836 59.013 ; 15 -1273.57 1652.67 -10.836 59.013 ; 15 -1251.89 1534.65 -10.836 59.013 ; 15 -1230.22 1416.62 -10.836 59.013
537 7fbec9f9 | 229 5.00 0.00 0.00 0 0 0.000 0.000 | 239 1.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1327.75 1947.74 -10.836 59.013 ; 15 -1306.07 1829.71 -10.836 59.013 ; 15 -1284.40 1711.69 -10.836 59.013 ; 15 -1262.73 1593.66 -10.836 59.013 ; 15 -1241.06 1475.63 -10.836 59.013
538 3807d88d | 229 6.00 0.00 0.00 0 0 0.000 0.000 | 239 2.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1338.58 2006.75 -10.836 59.013 ; 15 -1316.91 1888.73 -10.836 59.013 ; 15 -1295.24 1770.70 -10.836 59.013 ; 15 -1273.57 1652.67 -10.836 59.013 ; 15 -1251.89 1534.65 -10.836 59.013
539 a51fb561 | 229 7.00 0.00 0.00 0 0 0.000 0.000 | 239 3.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1349.42 2065.77 -10.836 59.013 ; 15 -1327.75 1947.74 -10.836 59.013 ; 15 -1306.07 1829.71 -10.836 59.013 ; 15 -1284.40 1711.69 -10.836 59.013 ; 15 -1262.73 1593.66 -10.836 59.013
540 99dad475 | 229 8.00 0.00 0.00 0 0 0.000 0.000 | 239 4.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1360.26 2124.78 -10.836 59.013 ; 15 -1338.58 2006.75 -10.836 59.013 ; 15 -1316.91 1888.73 -10.836 59.013 ; 15 -1295.24 1770.70 -10.836 59.013 ; 15 -1273.57 1652.67 -10.836 59.013
541 4ff896ac | 229 9.00 0.00 0.00 0 0 0.000 0.000 | 239 0.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1371.09 2183.79 -10.836 59.013 ; 15 -1349.42 2065.77 -10.836 59.013 ; 15 -1327.75 1947.74 -10.836 59.013 ; 15 -1306.07 1829.71 -10.836 59.013 ; 15 -1284.40 1711.69 -10.836 59.013
542 97968d30 | 229 0.00 0.00 0.00 0 0 0.000 0.000 | 239 1.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1381.93 2242.81 -10.836 59.013 ; 15 -1360.26 2124.78 -10.836 59.013 ; 15 -1338.58 2006.75 -10.836 59.013 ; 15 -1316.91 1888.73 -10.836 59.013 ; 15 -1295.24 1770.70 -10.836 59.013
543 1f16ccf4 | 229 1.00 0.00 0.00 0 0 0.000 0.000 | 239 2.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1392.76 2301.82 -10.836 59.013 ; 15 -1371.09 2183.79 -10.836 59.013 ; 15 -1349.42 2065.77 -10.836 59.013 ; 15 -1327.75 1947.74 -10.836 59.013 ; 15 -1306.07 1829.71 -10.836 59.013
544 cad439f8 | 229 2.00 0.00 0.00 0 0 0.000 0.000 | 239 3.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1403.60 2360.83 -10.836 59.013 ; 15 -1381.93 2242.81 -10.836 59.013 ; 15 -1360.26 2124.78 -10.836 59.013 ; 15 -1338.58 2006.75 -10.836 59.013 ; 15 -1316.91 1888.73 -10.836 59.013
545 5ed0f83c | 230 1.00 0.00 0.00 0 0 0.000 0.000 | 239 4.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1414.44 2419.85 -10.836 59.013 ; 15 -1392.76 2301.82 -10.836 59.013 ; 15 -1371.09 2183.79 -10.836 59.013 ; 15 -1349.42 2065.77 -10.836 59.013 ; 15 -1327.75 1947.74 -10.836 59.013
546 66fa6bc0 | 230 2.00 0.00 0.00 0 0 0.000 0.000 | 239 0.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1425.27 2478.86 -10.836 59.013 ; 15 -1403.60 2360.83 -10.836 59.013 ; 15 -1381.93 2242.81 -10.836 59.013 ; 15 -1360.26 2124.78 -10.836 59.013 ; 15 -1338.58 2006.75 -10.836 59.013
547 f4ad3884 | 230 3.00 0.00 0.00 0 0 0.000 0.000 | 239 1.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1436.11 2537.87 -10.836 59.013 ; 15 -1414.44 2419.85 -10.836 59.013 ; 15 -1392.76 2301.82 -10.836 59.013 ; 15 -1371.09 2183.79 -10.836 59.013 ; 15 -1349.42 2065.77 -10.836 59.013
548 d5794288 | 230 4.00 0.00 0.00 0 0 0.000 0.000 | 239 2.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1446.95 2596.89 -10.836 59.013 ; 15 -1425.27 2478.86 -10.836 59.013 ; 15 -1403.60 2360.83 -10.836 59.013 ; 15 -1381.93 2242.81 -10.836 59.013 ; 15 -1360.26 2124.78 -10.836 59.013
549 2725adcc | 230 5.00 0.00 0.00 0 0 0.000 0.000 | 239 3.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1457.78 2655.90 -10.836 59.013 ; 15 -1436.11 2537.87 -10.836 59.013 ; 15 -1414.44 2419.85 -10.836 59.013 ; 15 -1392.76 2301.82 -10.836 59.013 ; 15 -1371.09 2183.79 -10.836 59.013
550 6cf4de50 | 230 6.00 0.00 0.00 0 0 0.000 0.000 | 239 4.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1468.62 2714.91 -10.836 59.013 ; 15 -1446.95 2596.89 -10.836 59.013 ; 15 -1425.27 2478.86 -10.836 59.013 ; 15 -1403.60 2360.83 -10.836 59.013 ; 15 -1381.93 2242.81 -10.836 59.013
551 8a287814 | 230 7.00 0.00 0.00 0 0 0.000 0.000 | 239 0.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1479.46 2773.93 -10.836 59.013 ; 15 -1457.78 2655.90 -10.836 59.013 ; 15 -1436.11 2537.87 -10.836 59.013 ; 15 -1414.44 2419.85 -10.836 59.013 ; 15 -1392.76 2301.82 -10.836 59.013
552 45c55f18 | 230 8.00 0.00 0.00 0 0 0.000 0.000 | 239 1.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1490.29 2832.94 -10.836 59.013 ; 15 -1468.62 2714.91 -10.836 59.013 ; 15 -1446.95 2596.89 -10.836 59.013 ; 15 -1425.27 2478.86 -10.836 59.013 ; 15 -1403.60 2360.83 -10.836 59.013
553 3b97b75c | 230 9.00 0.00 0.00 0 0 0.000 0.000 | 239 2.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1501.13 2891.95 -10.836 59.013 ; 15 -1479.46 2773.93 -10.836 59.013 ; 15 -1457.78 2655.90 -10.836 59.013 ; 15 -1436.11 2537.87 -10.836 59.013 ; 15 -1414.44 2419.85 -10.836 59.013
554 5e76e4e0 | 230 10.00 0.00 0.00 0 0 0.000 0.000 | 239 3.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1511.96 2950.97 -10.836 59.013 ; 15 -1490.29 2832.94 -10.836 59.013 ; 15 -1468.62 2714.91 -10.836 59.013 ; 15 -1446.95 2596.89 -10.836 59.013 ; 15 -1425.27 2478.86 -10.836 59.013
555 6fc98ba4 | 230 11.00 0.00 0.00 0 0 0.000 0.000 | 239 4.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1522.80 3009.98 -10.836 59.013 ; 15 -1501.13 2891.95 -10.836 59.013 ; 15 -1479.46 2773.93 -10.836 59.013 ; 15 -1457.78 2655.90 -10.836 59.013 ; 15 -1436.11 2537.87 -10.836 59.013
556 10498fa8 | 230 12.00 0.00 0.00 0 0 0.000 0.000 | 239 0.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1533.64 3068.99 -10.836 59.013 ; 15 -1511.96 2950.97 -10.836 59.013 ; 15 -1490.29 2832.94 -10.836 59.013 ; 15 -1468.62 2714.91 -10.836 59.013 ; 15 -1446.95 2596.89 -10.836 59.013
557 4e0814ec | 230 13.00 0.00 0.00 0 0 0.000 0.000 | 239 1.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1544.47 3128.01 -10.836 59.013 ; 15 -1522.80 3009.98 -10.836 59.013 ; 15 -1501.13 2891.95 -10.836 59.013 ; 15 -1479.46 2773.93 -10.836 59.013 ; 15 -1457.78 2655.90 -10.836 59.013
558 d3b17f70 | 230 14.00 0.00 0.00 0 0 0.000 0.000 | 239 2.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1555.31 3187.02 -10.836 59.013 ; 15 -1533.64 3068.99 -10.836 59.013 ; 15 -1511.96 2950.97 -10.836 59.013 ; 15 -1490.29 2832.94 -10.836 59.013 ; 15 -1468.62 2714.91 -10.836 59.013
559 1d117334 | 230 15.00 0.00 0.00 0 0 0.000 0.000 | 239 3.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1566.15 3246.03 -10.836 59.013 ; 15 -1544.47 3128.01 -10.836 59.013 ; 15 -1522.80 3009.98 -10.836 59.013 ; 15 -1501.13 2891.95 -10.836 59.013 ; 15 -1479.46 2773.93 -10.836 59.013
560 54d6d438 | 10 0.00 0.00 0.00 0 0 0.000 0.000 | 239 4.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1576.98 3305.05 -10.836 59.013 ; 15 -1555.31 3187.02 -10.836 59.013 ; 15 -1533.64 3068.99 -10.836 59.013 ; 15 -1511.96 2950.97 -10.836 59.013 ; 15 -1490.29 2832.94 -10.836 59.013
561 bf97c67c | 10 1.00 0.00 0.00 0 0 0.000 0.000 | 239 0.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1587.82 3364.06 -10.836 59.013 ; 15 -1566.15 3246.03 -10.836 59.013 ; 15 -1544.47 3128.01 -10.836 59.013 ; 15 -1522.80 3009.98 -10.836 59.013 ; 15 -1501.13 2891.95 -10.836 59.013
562 d815ae00 | 10 2.00 0.00 0.00 0 0 0.000 0.000 | 239 1.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1598.65 3423.07 -10.836 59.013 ; 15 -1576.98 3305.05 -10.836 59.013 ; 15 -1555.31 3187.02 -10.836 59.013 ; 15 -1533.64 3069.00 -10.836 59.013 ; 15 -1511.96 2950.97 -10.836 59.013
563 80c12ec4 | 10 3.00 0.00 0.00 0 0 0.000 0.000 | 239 2.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1609.49 3482.09 -10.836 59.013 ; 15 -1587.82 3364.06 -10.836 59.013 ; 15 -1566.15 3246.04 -10.836 59.013 ; 15 -1544.47 3128.01 -10.836 59.013 ; 15 -1522.80 3009.98 -10.836 59.013
564 ee7e2cc8 | 10 4.00 0.00 0.00 0 0 0.000 0.000 | 239 3.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1620.33 3541.10 -10.836 59.013 ; 15 -1598.65 3423.08 -10.836 59.013 ; 15 -1576.98 3305.05 -10.836 59.013 ; 15 -1555.31 3187.02 -10.836 59.013 ; 15 -1533.64 3069.00 -10.836 59.013
565 30a7cc0c | 10 5.00 0.00 0.00 0 0 0.000 0.000 | 239 4.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1631.16 3600.12 -10.836 59.013 ; 15 -1609.49 3482.09 -10.836 59.013 ; 15 -1587.82 3364.06 -10.836 59.013 ; 15 -1566.15 3246.04 -10.836 59.013 ; 15 -1544.47 3128.01 -10.836 59.013
566 7a547090 | 10 6.00 0.00 0.00 0 0 0.000 0.000 | 239 0.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1642.00 3659.13 -10.836 59.013 ; 15 -1620.33 3541.10 -10.836 59.013 ; 15 -1598.65 3423.08 -10.836 59.013 ; 15 -1576.98 3305.05 -10.836 59.013 ; 15 -1555.31 3187.02 -10.836 59.013
567 90d9be54 | 10 7.00 0.00 0.00 0 0 0.000 0.000 | 239 1.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1652.84 3718.14 -10.836 59.013 ; 15 -1631.16 3600.12 -10.836 59.013 ; 15 -1609.49 3482.09 -10.836 59.013 ; 15 -1587.82 3364.06 -10.836 59.013 ; 15 -1566.15 3246.04 -10.836 59.013
568 03909958 | 10 8.00 0.00 0.00 0 0 0.000 0.000 | 239 2.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1663.67 3777.16 -10.836 59.013 ; 15 -1642.00 3659.13 -10.836 59.013 ; 15 -1620.33 3541.10 -10.836 59.013 ; 15 -1598.65 3423.08 -10.836 59.013 ; 15 -1576.98 3305.05 -10.836 59.013
569 10d9259c | 10 9.00 0.00 0.00 0 0 0.000 0.000 | 239 3.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1674.51 3836.17 -10.836 59.013 ; 15 -1652.84 3718.14 -10.836 59.013 ; 15 -1631.16 3600.12 -10.836 59.013 ; 15 -1609.49 3482.09 -10.836 59.013 ; 15 -1587.82 3364.06 -10.836 59.013
570 5c5ec720 | 10 10.00 0.00 0.00 0 0 0.000 0.000 | 239 4.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1685.35 3895.18 -10.836 59.013 ; 15 -1663.67 3777.16 -10.836 59.013 ; 15 -1642.00 3659.13 -10.836 59.013 ; 15 -1620.33 3541.10 -10.836 59.013 ; 15 -1598.65 3423.08 -10.836 59.013
571 da9c21e4 | 10 11.00 0.00 0.00 0 0 0.000 0.000 | 239 0.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1696.18 3954.20 -10.836 59.013 ; 15 -1674.51 3836.17 -10.836 59.013 ; 15 -1652.84 3718.14 -10.836 59.013 ; 15 -1631.16 3600.12 -10.836 59.013 ; 15 -1609.49 3482.09 -10.836 59.013
572 959f19e8 | 10 12.00 0.00 0.00 0 0 0.000 0.000 | 239 1.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1707.02 4013.21 -10.836 59.013 ; 15 -1685.35 3895.18 -10.836 59.013 ; 15 -1663.67 3777.16 -10.836 59.013 ; 15 -1642.00 3659.13 -10.836 59.013 ; 15 -1620.33 3541.10 -10.836 59.013
573 2f0cd32c | 10 13.00 0.00 0.00 0 0 0.000 0.000 | 239 2.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1717.85 4072.22 -10.836 59.013 ; 15 -1696.18 3954.20 -10.836 59.013 ; 15 -1674.51 3836.17 -10.836 59.013 ; 15 -1652.84 3718.14 -10.836 59.013 ; 15 -1631.16 3600.12 -10.836 59.013
574 4365b1b0 | 10 14.00 0.00 0.00 0 0 0.000 0.000 | 239 3.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1728.69 4131.24 -10.836 59.013 ; 15 -1707.02 4013.21 -10.836 59.013 ; 15 -1685.34 3895.18 -10.836 59.013 ; 15 -1663.67 3777.16 -10.836 59.013 ; 15 -1642.00 3659.13 -10.836 59.013
575 12895974 | 10 15.00 0.00 0.00 0 0 0.000 0.000 | 239 4.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1739.53 4190.25 -10.836 59.013 ; 15 -1717.85 4072.22 -10.836 59.013 ; 15 -1696.18 3954.20 -10.836 59.013 ; 15 -1674.51 3836.17 -10.836 59.013 ; 15 -1652.84 3718.14 -10.836 59.013
576 117aae78 | 10 16.00 0.00 0.00 0 0 0.000 0.000 | 239 0.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1750.36 4249.26 -10.836 59.013 ; 15 -1728.69 4131.24 -10.836 59.013 ; 15 -1707.02 4013.21 -10.836 59.013 ; 15 -1685.34 3895.18 -10.836 59.013 ; 15 -1663.67 3777.16 -10.836 59.013
577 4963d4bc | 10 17.00 0.00 0.00 0 0 0.000 0.000 | 239 1.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1761.20 4308.28 -10.836 59.013 ; 15 -1739.53 4190.25 -10.836 59.013 ; 15 -1717.85 4072.22 -10.836 59.013 ; 15 -1696.18 3954.20 -10.836 59.013 ; 15 -1674.51 3836.17 -10.836 59.013
578 a7da3040 | 10 18.00 0.00 0.00 0 0 0.000 0.000 | 239 2.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1772.04 4367.29 -10.836 59.013 ; 15 -1750.36 4249.26 -10.836 59.013 ; 15 -1728.69 4131.24 -10.836 59.013 ; 15 -1707.02 4013.21 -10.836 59.013 ; 15 -1685.34 3895.18 -10.836 59.013
579 a4626504 | 10 19.00 0.00 0.00 0 0 0.000 0.000 | 239 3.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1782.87 4426.30 -10.836 59.013 ; 15 -1761.20 4308.27 -10.836 59.013 ; 15 -1739.53 4190.25 -10.836 59.013 ; 15 -1717.85 4072.22 -10.836 59.013 ; 15 -1696.18 3954.20 -10.836 59.013
580 df345708 | 10 20.00 0.00 0.00 0 0 0.000 0.000 | 239 4.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1793.71 4485.31 -10.836 59.013 ; 15 -1772.04 4367.29 -10.836 59.013 ; 15 -1750.36 4249.26 -10.836 59.013 ; 15 -1728.69 4131.23 -10.836 59.013 ; 15 -1707.02 4013.21 -10.836 59.013
581 9d3f2a4c | 10 21.00 0.00 0.00 0 0 0.000 0.000 | 239 0.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1804.54 4544.33 -10.836 59.013 ; 15 -1782.87 4426.30 -10.836 59.013 ; 15 -1761.20 4308.27 -10.836 59.013 ; 15 -1739.53 4190.25 -10.836 59.013 ; 15 -1717.85 4072.22 -10.836 59.013
582 456d42d0 | 10 22.00 0.00 0.00 0 0 0.000 0.000 | 239 1.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1815.38 4603.34 -10.836 59.013 ; 15 -1793.71 4485.31 -10.836 59.013 ; 15 -1772.04 4367.29 -10.836 59.013 ; 15 -1750.36 4249.26 -10.836 59.013 ; 15 -1728.69 4131.23 -10.836 59.013
583 43284494 | 10 23.00 0.00 0.00 0 0 0.000 0.000 | 239 2.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1826.22 4662.35 -10.836 59.013 ; 15 -1804.54 4544.33 -10.836 59.013 ; 15 -1782.87 4426.30 -10.836 59.013 ; 15 -1761.20 4308.27 -10.836 59.013 ; 15 -1739.53 4190.25 -10.836 59.013
584 f21d1398 | 10 24.00 0.00 0.00 0 0 0.000 0.000 | 239 3.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1837.05 4721.37 -10.836 59.013 ; 15 -1815.38 4603.34 -10.836 59.013 ; 15 -1793.71 4485.31 -10.836 59.013 ; 15 -1772.04 4367.29 -10.836 59.013 ; 15 -1750.36 4249.26 -10.836 59.013
585 773fd3dc | 10 25.00 0.00 0.00 0 0 0.000 0.000 | 239 4.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1847.89 4780.38 -10.836 59.013 ; 15 -1826.22 4662.35 -10.836 59.013 ; 15 -1804.54 4544.33 -10.836 59.013 ; 15 -1782.87 4426.30 -10.836 59.013 ; 15 -1761.20 4308.27 -10.836 59.013
586 ab0fe960 | 10 26.00 0.00 0.00 0 0 0.000 0.000 | 239 0.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1858.73 4839.39 -10.836 59.013 ; 15 -1837.05 4721.37 -10.836 59.013 ; 15 -1815.38 4603.34 -10.836 59.013 ; 15 -1793.71 4485.31 -10.836 59.013 ; 15 -1772.04 4367.29 -10.836 59.013
587 791bf824 | 10 27.00 0.00 0.00 0 0 0.000 0.000 | 239 1.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1869.56 4898.41 -10.836 59.013 ; 15 -1847.89 4780.38 -10.836 59.013 ; 15 -1826.22 4662.35 -10.836 59.013 ; 15 -1804.54 4544.33 -10.836 59.013 ; 15 -1782.87 4426.30 -10.836 59.013
588 58c5e428 | 10 28.00 0.00 0.00 0 0 0.000 0.000 | 238 0.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1880.40 4957.42 -10.836 59.013 ; 15 -1858.73 4839.39 -10.836 59.013 ; 15 -1837.05 4721.37 -10.836 59.013 ; 15 -1815.38 4603.34 -10.836 59.013 ; 15 -1793.71 4485.31 -10.836 59.013
589 c346d16c | 10 29.00 0.00 0.00 0 0 0.000 0.000 | 238 1.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1891.23 5016.43 -10.836 59.013 ; 15 -1869.56 4898.41 -10.836 59.013 ; 15 -1847.89 4780.38 -10.836 59.013 ; 15 -1826.22 4662.35 -10.836 59.013 ; 15 -1804.54 4544.33 -10.836 59.013
590 caf323f0 | 10 30.00 0.00 0.00 0 0 0.000 0.000 | 238 2.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1902.07 5075.45 -10.836 59.013 ; 15 -1880.40 4957.42 -10.836 59.013 ; 15 -1858.73 4839.39 -10.836 59.013 ; 15 -1837.05 4721.37 -10.836 59.013 ; 15 -1815.38 4603.34 -10.836 59.013
591 37be7fb4 | 10 31.00 0.00 0.00 0 0 0.000 0.000 | 238 3.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1912.91 5134.46 -10.836 59.013 ; 15 -1891.23 5016.43 -10.836 59.013 ; 15 -1869.56 4898.41 -10.836 59.013 ; 15 -1847.89 4780.38 -10.836 59.013 ; 15 -1826.22 4662.35 -10.836 59.013
592 ccffc8b8 | 10 32.00 0.00 0.00 0 0 0.000 0.000 | 238 4.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1923.74 5193.47 -10.836 59.013 ; 15 -1902.07 5075.45 -10.836 59.013 ; 15 -1880.40 4957.42 -10.836 59.013 ; 15 -1858.73 4839.39 -10.836 59.013 ; 15 -1837.05 4721.37 -10.836 59.013
593 9c7522fc | 10 33.00 0.00 0.00 0 0 0.000 0.000 | 238 5.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1934.58 5252.49 -10.836 59.013 ; 15 -1912.91 5134.46 -10.836 59.013 ; 15 -1891.23 5016.43 -10.836 59.013 ; 15 -1869.56 4898.41 -10.836 59.013 ; 15 -1847.89 4780.38 -10.836 59.013
594 8a87f280 | 10 34.00 0.00 0.00 0 0 0.000 0.000 | 238 6.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1945.42 5311.50 -10.836 59.013 ; 15 -1923.74 5193.47 -10.836 59.013 ; 15 -1902.07 5075.45 -10.836 59.013 ; 15 -1880.40 4957.42 -10.836 59.013 ; 15 -1858.73 4839.39 -10.836 59.013
595 67d0db44 | 10 35.00 0.00 0.00 0 0 0.000 0.000 | 238 7.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1956.25 5370.51 -10.836 59.013 ; 15 -1934.58 5252.49 -10.836 59.013 ; 15 -1912.91 5134.46 -10.836 59.013 ; 15 -1891.23 5016.43 -10.836 59.013 ; 15 -1869.56 4898.41 -10.836 59.013
596 43dbc148 | 10 36.00 0.00 0.00 0 0 0.000 0.000 | 238 8.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1967.09 5429.53 -10.836 59.013 ; 15 -1945.42 5311.50 -10.836 59.013 ; 15 -1923.74 5193.47 -10.836 59.013 ; 15 -1902.07 5075.45 -10.836 59.013 ; 15 -1880.40 4957.42 -10.836 59.013
597 dd2bc88c | 10 37.00 0.00 0.00 0 0 0.000 0.000 | 238 9.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1977.93 5488.54 -10.836 59.013 ; 15 -1956.25 5370.51 -10.836 59.013 ; 15 -1934.58 5252.49 -10.836 59.013 ; 15 -1912.91 5134.46 -10.836 59.013 ; 15 -1891.23 5016.43 -10.836 59.013
598 527f5510 | 10 38.00 0.00 0.00 0 0 0.000 0.000 | 238 10.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1988.76 5547.55 -10.836 59.013 ; 15 -1967.09 5429.53 -10.836 59.013 ; 15 -1945.42 5311.50 -10.836 59.013 ; 15 -1923.74 5193.47 -10.836 59.013 ; 15 -1902.07 5075.45 -10.836 59.013
599 79540ad4 | 10 39.00 0.00 0.00 0 0 0.000 0.000 | 238 11.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -1999.60 5606.57 -10.836 59.013 ; 15 -1977.93 5488.54 -10.836 59.013 ; 15 -1956.25 5370.51 -10.836 59.013 ; 15 -1934.58 5252.49 -10.836 59.013 ; 15 -1912.91 5134.46 -10.836 59.013
600 7daacdd8 | 10 40.00 0.00 0.00 0 0 0.000 0.000 | 238 12.00 -1016.00 0.00 0 0 0.000 0.000 |  | 14 -2010.43 5665.58 -10.836 59.013 ; 15 -1988.76 5547.55 -10.836 59.013 ; 15 -1967.09 5429.52 -10.836 59.013 ; 15 -1945.42 5311.50 -10.836 59.013 ; 15 -1923.74 5193.47 -10.836 59.013
";

/// `vst1`'s routes: P1 Ness's PK Thunder, steered left and down; P2 Fox's
/// drop, walk and held reflector.
const VST1_P1: &str = "470:3:b+y=80;473:60:x=-80+y=-40";
const VST1_P2: &str = "405:3:y=-80;440:16:x=50;468:120:b+y=-80";

#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn a_reflected_pk_thunder_flies_off_as_on_the_n64() {
    let replay = Replay {
        p1: FighterKind::Ness,
        route: VST1_P1,
        weapons: &ness_weapons,
    };
    let vs = Vs {
        p2: FighterKind::Fox,
        route2: VST1_P2,
        spawns: VS_NESS_FOX,
    };
    let Some(out) = replay.run_vs(VS_PK_THUNDER_REFLECTED, &vs) else {
        return;
    };
    out.check("Fox reflects Ness's PK Thunder in VS");
    // Without the reflector (Fox's B released at 500) the head flies on.
    let vs = Vs {
        route2: "405:3:y=-80;440:16:x=50;468:32:b+y=-80",
        ..vs
    };
    let out = replay.run_vs(VS_PK_THUNDER_REFLECTED, &vs).unwrap();
    assert!(
        out.first_bad.is_some_and(|t| (500..=515).contains(&t)),
        "a replay without the reflector should leave the rows by 515, left at {:?}",
        out.first_bad
    );
}

/// `b36/vsm1` (`vsmario.z64`: VS Dream Land, Mario P1 vs Mario P2, both
/// human; the entries leave them where `vsness`'s do, Wait from 270 and
/// 330): P2 drops from the left platform and throws a Fireball (t460),
/// P1 throws one back from the floor (t470). They meet in t488's weapon
/// searches (`wpProcessProcSearchHitWeapon`): equal damage, both set off
/// and gone; the clash's sparks draw from the seed. Every frame from
/// P2's first Wait frame.
const VS_FIREBALL_CLANK: &str = "
330 cae1df84 | 10 10.00 0.00 0.00 0 0 0.000 0.000 | 10 0.00 -1397.00 904.00 0 0 0.000 0.000
331 cae1df84 | 10 11.00 0.00 0.00 0 0 0.000 0.000 | 10 1.00 -1397.00 904.00 0 0 0.000 0.000
332 cae1df84 | 10 12.00 0.00 0.00 0 0 0.000 0.000 | 10 2.00 -1397.00 904.00 0 0 0.000 0.000
333 cae1df84 | 10 13.00 0.00 0.00 0 0 0.000 0.000 | 10 3.00 -1397.00 904.00 0 0 0.000 0.000
334 cae1df84 | 10 14.00 0.00 0.00 0 0 0.000 0.000 | 10 4.00 -1397.00 904.00 0 0 0.000 0.000
335 cae1df84 | 10 15.00 0.00 0.00 0 0 0.000 0.000 | 10 5.00 -1397.00 904.00 0 0 0.000 0.000
336 cae1df84 | 10 16.00 0.00 0.00 0 0 0.000 0.000 | 10 6.00 -1397.00 904.00 0 0 0.000 0.000
337 cae1df84 | 10 17.00 0.00 0.00 0 0 0.000 0.000 | 10 7.00 -1397.00 904.00 0 0 0.000 0.000
338 cae1df84 | 10 18.00 0.00 0.00 0 0 0.000 0.000 | 10 8.00 -1397.00 904.00 0 0 0.000 0.000
339 cae1df84 | 10 19.00 0.00 0.00 0 0 0.000 0.000 | 10 9.00 -1397.00 904.00 0 0 0.000 0.000
340 cae1df84 | 10 20.00 0.00 0.00 0 0 0.000 0.000 | 10 10.00 -1397.00 904.00 0 0 0.000 0.000
341 cae1df84 | 10 21.00 0.00 0.00 0 0 0.000 0.000 | 10 11.00 -1397.00 904.00 0 0 0.000 0.000
342 cae1df84 | 10 22.00 0.00 0.00 0 0 0.000 0.000 | 10 12.00 -1397.00 904.00 0 0 0.000 0.000
343 cae1df84 | 10 23.00 0.00 0.00 0 0 0.000 0.000 | 10 13.00 -1397.00 904.00 0 0 0.000 0.000
344 cae1df84 | 10 24.00 0.00 0.00 0 0 0.000 0.000 | 10 14.00 -1397.00 904.00 0 0 0.000 0.000
345 cae1df84 | 10 25.00 0.00 0.00 0 0 0.000 0.000 | 10 15.00 -1397.00 904.00 0 0 0.000 0.000
346 cae1df84 | 10 26.00 0.00 0.00 0 0 0.000 0.000 | 10 16.00 -1397.00 904.00 0 0 0.000 0.000
347 cae1df84 | 10 27.00 0.00 0.00 0 0 0.000 0.000 | 10 17.00 -1397.00 904.00 0 0 0.000 0.000
348 cae1df84 | 10 28.00 0.00 0.00 0 0 0.000 0.000 | 10 18.00 -1397.00 904.00 0 0 0.000 0.000
349 cae1df84 | 10 29.00 0.00 0.00 0 0 0.000 0.000 | 10 19.00 -1397.00 904.00 0 0 0.000 0.000
350 cae1df84 | 10 30.00 0.00 0.00 0 0 0.000 0.000 | 10 20.00 -1397.00 904.00 0 0 0.000 0.000
351 cae1df84 | 10 31.00 0.00 0.00 0 0 0.000 0.000 | 10 21.00 -1397.00 904.00 0 0 0.000 0.000
352 cae1df84 | 10 32.00 0.00 0.00 0 0 0.000 0.000 | 10 22.00 -1397.00 904.00 0 0 0.000 0.000
353 cae1df84 | 10 33.00 0.00 0.00 0 0 0.000 0.000 | 10 23.00 -1397.00 904.00 0 0 0.000 0.000
354 cae1df84 | 10 34.00 0.00 0.00 0 0 0.000 0.000 | 10 24.00 -1397.00 904.00 0 0 0.000 0.000
355 cae1df84 | 10 35.00 0.00 0.00 0 0 0.000 0.000 | 10 25.00 -1397.00 904.00 0 0 0.000 0.000
356 cae1df84 | 10 36.00 0.00 0.00 0 0 0.000 0.000 | 10 26.00 -1397.00 904.00 0 0 0.000 0.000
357 cae1df84 | 10 37.00 0.00 0.00 0 0 0.000 0.000 | 10 27.00 -1397.00 904.00 0 0 0.000 0.000
358 cae1df84 | 10 38.00 0.00 0.00 0 0 0.000 0.000 | 10 28.00 -1397.00 904.00 0 0 0.000 0.000
359 cae1df84 | 10 39.00 0.00 0.00 0 0 0.000 0.000 | 10 29.00 -1397.00 904.00 0 0 0.000 0.000
360 cae1df84 | 10 40.00 0.00 0.00 0 0 0.000 0.000 | 10 30.00 -1397.00 904.00 0 0 0.000 0.000
361 cae1df84 | 10 41.00 0.00 0.00 0 0 0.000 0.000 | 10 31.00 -1397.00 904.00 0 0 0.000 0.000
362 cae1df84 | 10 42.00 0.00 0.00 0 0 0.000 0.000 | 10 32.00 -1397.00 904.00 0 0 0.000 0.000
363 cae1df84 | 10 43.00 0.00 0.00 0 0 0.000 0.000 | 10 33.00 -1397.00 904.00 0 0 0.000 0.000
364 cae1df84 | 10 44.00 0.00 0.00 0 0 0.000 0.000 | 10 34.00 -1397.00 904.00 0 0 0.000 0.000
365 cae1df84 | 10 45.00 0.00 0.00 0 0 0.000 0.000 | 10 35.00 -1397.00 904.00 0 0 0.000 0.000
366 cae1df84 | 10 46.00 0.00 0.00 0 0 0.000 0.000 | 10 36.00 -1397.00 904.00 0 0 0.000 0.000
367 cae1df84 | 10 47.00 0.00 0.00 0 0 0.000 0.000 | 10 37.00 -1397.00 904.00 0 0 0.000 0.000
368 cae1df84 | 10 48.00 0.00 0.00 0 0 0.000 0.000 | 10 38.00 -1397.00 904.00 0 0 0.000 0.000
369 cae1df84 | 10 49.00 0.00 0.00 0 0 0.000 0.000 | 10 39.00 -1397.00 904.00 0 0 0.000 0.000
370 cae1df84 | 10 0.00 0.00 0.00 0 0 0.000 0.000 | 10 40.00 -1397.00 904.00 0 0 0.000 0.000
371 cae1df84 | 10 1.00 0.00 0.00 0 0 0.000 0.000 | 10 41.00 -1397.00 904.00 0 0 0.000 0.000
372 cae1df84 | 10 2.00 0.00 0.00 0 0 0.000 0.000 | 10 42.00 -1397.00 904.00 0 0 0.000 0.000
373 cae1df84 | 10 3.00 0.00 0.00 0 0 0.000 0.000 | 10 43.00 -1397.00 904.00 0 0 0.000 0.000
374 cae1df84 | 10 4.00 0.00 0.00 0 0 0.000 0.000 | 10 44.00 -1397.00 904.00 0 0 0.000 0.000
375 cae1df84 | 10 5.00 0.00 0.00 0 0 0.000 0.000 | 10 45.00 -1397.00 904.00 0 0 0.000 0.000
376 cae1df84 | 10 6.00 0.00 0.00 0 0 0.000 0.000 | 10 46.00 -1397.00 904.00 0 0 0.000 0.000
377 cae1df84 | 10 7.00 0.00 0.00 0 0 0.000 0.000 | 10 47.00 -1397.00 904.00 0 0 0.000 0.000
378 cae1df84 | 10 8.00 0.00 0.00 0 0 0.000 0.000 | 10 48.00 -1397.00 904.00 0 0 0.000 0.000
379 cae1df84 | 10 9.00 0.00 0.00 0 0 0.000 0.000 | 10 49.00 -1397.00 904.00 0 0 0.000 0.000
380 cae1df84 | 10 10.00 0.00 0.00 0 0 0.000 0.000 | 10 0.00 -1397.00 904.00 0 0 0.000 0.000
381 cae1df84 | 10 11.00 0.00 0.00 0 0 0.000 0.000 | 10 1.00 -1397.00 904.00 0 0 0.000 0.000
382 cae1df84 | 10 12.00 0.00 0.00 0 0 0.000 0.000 | 10 2.00 -1397.00 904.00 0 0 0.000 0.000
383 cae1df84 | 10 13.00 0.00 0.00 0 0 0.000 0.000 | 10 3.00 -1397.00 904.00 0 0 0.000 0.000
384 cae1df84 | 10 14.00 0.00 0.00 0 0 0.000 0.000 | 10 4.00 -1397.00 904.00 0 0 0.000 0.000
385 cae1df84 | 10 15.00 0.00 0.00 0 0 0.000 0.000 | 10 5.00 -1397.00 904.00 0 0 0.000 0.000
386 cae1df84 | 10 16.00 0.00 0.00 0 0 0.000 0.000 | 10 6.00 -1397.00 904.00 0 0 0.000 0.000
387 cae1df84 | 10 17.00 0.00 0.00 0 0 0.000 0.000 | 10 7.00 -1397.00 904.00 0 0 0.000 0.000
388 cae1df84 | 10 18.00 0.00 0.00 0 0 0.000 0.000 | 10 8.00 -1397.00 904.00 0 0 0.000 0.000
389 cae1df84 | 10 19.00 0.00 0.00 0 0 0.000 0.000 | 10 9.00 -1397.00 904.00 0 0 0.000 0.000
390 cae1df84 | 10 20.00 0.00 0.00 0 0 0.000 0.000 | 10 10.00 -1397.00 904.00 0 0 0.000 0.000
391 cae1df84 | 10 21.00 0.00 0.00 0 0 0.000 0.000 | 10 11.00 -1397.00 904.00 0 0 0.000 0.000
392 cae1df84 | 10 22.00 0.00 0.00 0 0 0.000 0.000 | 10 12.00 -1397.00 904.00 0 0 0.000 0.000
393 cae1df84 | 10 23.00 0.00 0.00 0 0 0.000 0.000 | 10 13.00 -1397.00 904.00 0 0 0.000 0.000
394 cae1df84 | 10 24.00 0.00 0.00 0 0 0.000 0.000 | 10 14.00 -1397.00 904.00 0 0 0.000 0.000
395 cae1df84 | 10 25.00 0.00 0.00 0 0 0.000 0.000 | 10 15.00 -1397.00 904.00 0 0 0.000 0.000
396 cae1df84 | 10 26.00 0.00 0.00 0 0 0.000 0.000 | 10 16.00 -1397.00 904.00 0 0 0.000 0.000
397 cae1df84 | 10 27.00 0.00 0.00 0 0 0.000 0.000 | 10 17.00 -1397.00 904.00 0 0 0.000 0.000
398 cae1df84 | 10 28.00 0.00 0.00 0 0 0.000 0.000 | 10 18.00 -1397.00 904.00 0 0 0.000 0.000
399 cae1df84 | 10 29.00 0.00 0.00 0 0 0.000 0.000 | 10 19.00 -1397.00 904.00 0 0 0.000 0.000
400 cae1df84 | 10 30.00 0.00 0.00 0 0 0.000 0.000 | 10 20.00 -1397.00 904.00 0 0 0.000 0.000
401 cae1df84 | 10 31.00 0.00 0.00 0 0 0.000 0.000 | 10 21.00 -1397.00 904.00 0 0 0.000 0.000
402 cae1df84 | 10 32.00 0.00 0.00 0 0 0.000 0.000 | 10 22.00 -1397.00 904.00 0 0 0.000 0.000
403 cae1df84 | 10 33.00 0.00 0.00 0 0 0.000 0.000 | 10 23.00 -1397.00 904.00 0 0 0.000 0.000
404 cae1df84 | 10 34.00 0.00 0.00 0 0 0.000 0.000 | 10 24.00 -1397.00 904.00 0 0 0.000 0.000
405 cae1df84 | 10 35.00 0.00 0.00 0 0 0.000 0.000 | 28 1.00 -1397.00 904.00 0 0 0.000 0.000
406 cae1df84 | 10 36.00 0.00 0.00 0 0 0.000 0.000 | 28 2.00 -1397.00 904.00 0 0 0.000 0.000
407 cae1df84 | 10 37.00 0.00 0.00 0 0 0.000 0.000 | 28 3.00 -1397.00 904.00 0 0 0.000 0.000
408 cae1df84 | 10 38.00 0.00 0.00 0 0 0.000 0.000 | 33 1.00 -1397.00 901.60 0 0 0.000 0.000
409 cae1df84 | 10 39.00 0.00 0.00 0 0 0.000 0.000 | 33 2.00 -1397.00 896.80 0 0 0.000 0.000
410 cae1df84 | 10 40.00 0.00 0.00 0 0 0.000 0.000 | 33 3.00 -1397.00 889.60 0 0 0.000 0.000
411 cae1df84 | 10 41.00 0.00 0.00 0 0 0.000 0.000 | 33 4.00 -1397.00 880.00 0 0 0.000 0.000
412 cae1df84 | 10 42.00 0.00 0.00 0 0 0.000 0.000 | 33 5.00 -1397.00 868.00 0 0 0.000 0.000
413 cae1df84 | 10 43.00 0.00 0.00 0 0 0.000 0.000 | 33 6.00 -1397.00 853.60 0 0 0.000 0.000
414 cae1df84 | 10 44.00 0.00 0.00 0 0 0.000 0.000 | 33 7.00 -1397.00 836.80 0 0 0.000 0.000
415 cae1df84 | 10 45.00 0.00 0.00 0 0 0.000 0.000 | 33 8.00 -1397.00 817.60 0 0 0.000 0.000
416 cae1df84 | 10 46.00 0.00 0.00 0 0 0.000 0.000 | 33 9.00 -1397.00 796.00 0 0 0.000 0.000
417 cae1df84 | 10 47.00 0.00 0.00 0 0 0.000 0.000 | 33 10.00 -1397.00 772.00 0 0 0.000 0.000
418 cae1df84 | 10 48.00 0.00 0.00 0 0 0.000 0.000 | 33 11.00 -1397.00 745.60 0 0 0.000 0.000
419 cae1df84 | 10 49.00 0.00 0.00 0 0 0.000 0.000 | 33 12.00 -1397.00 716.80 0 0 0.000 0.000
420 cae1df84 | 10 0.00 0.00 0.00 0 0 0.000 0.000 | 33 13.00 -1397.00 685.60 0 0 0.000 0.000
421 cae1df84 | 10 1.00 0.00 0.00 0 0 0.000 0.000 | 33 14.00 -1397.00 652.00 0 0 0.000 0.000
422 cae1df84 | 10 2.00 0.00 0.00 0 0 0.000 0.000 | 33 15.00 -1397.00 616.00 0 0 0.000 0.000
423 cae1df84 | 10 3.00 0.00 0.00 0 0 0.000 0.000 | 33 16.00 -1397.00 577.60 0 0 0.000 0.000
424 cae1df84 | 10 4.00 0.00 0.00 0 0 0.000 0.000 | 33 17.00 -1397.00 536.80 0 0 0.000 0.000
425 cae1df84 | 10 5.00 0.00 0.00 0 0 0.000 0.000 | 33 18.00 -1397.00 493.60 0 0 0.000 0.000
426 cae1df84 | 10 6.00 0.00 0.00 0 0 0.000 0.000 | 33 19.00 -1397.00 449.60 0 0 0.000 0.000
427 cae1df84 | 10 7.00 0.00 0.00 0 0 0.000 0.000 | 33 20.00 -1397.00 405.60 0 0 0.000 0.000
428 cae1df84 | 10 8.00 0.00 0.00 0 0 0.000 0.000 | 33 21.00 -1397.00 361.60 0 0 0.000 0.000
429 cae1df84 | 10 9.00 0.00 0.00 0 0 0.000 0.000 | 33 22.00 -1397.00 317.60 0 0 0.000 0.000
430 cae1df84 | 10 10.00 0.00 0.00 0 0 0.000 0.000 | 33 23.00 -1397.00 273.60 0 0 0.000 0.000
431 cae1df84 | 10 11.00 0.00 0.00 0 0 0.000 0.000 | 33 24.00 -1397.00 229.60 0 0 0.000 0.000
432 cae1df84 | 10 12.00 0.00 0.00 0 0 0.000 0.000 | 26 0.00 -1397.00 185.60 0 0 0.000 0.000
433 cae1df84 | 10 13.00 0.00 0.00 0 0 0.000 0.000 | 26 1.00 -1397.00 141.60 0 0 0.000 0.000
434 cae1df84 | 10 14.00 0.00 0.00 0 0 0.000 0.000 | 26 2.00 -1397.00 97.60 0 0 0.000 0.000
435 cae1df84 | 10 15.00 0.00 0.00 0 0 0.000 0.000 | 26 3.00 -1397.00 53.60 0 0 0.000 0.000
436 cae1df84 | 10 16.00 0.00 0.00 0 0 0.000 0.000 | 26 4.00 -1397.00 9.60 0 0 0.000 0.000
437 2cd60a1e | 10 17.00 0.00 0.00 0 0 0.000 0.000 | 31 0.00 -1397.00 0.00 0 0 0.000 0.000
438 2cd60a1e | 10 18.00 0.00 0.00 0 0 0.000 0.000 | 31 1.00 -1397.00 0.00 0 0 0.000 0.000
439 2cd60a1e | 10 19.00 0.00 0.00 0 0 0.000 0.000 | 31 2.00 -1397.00 0.00 0 0 0.000 0.000
440 2cd60a1e | 10 20.00 0.00 0.00 0 0 0.000 0.000 | 31 3.00 -1397.00 0.00 0 0 0.000 0.000
441 2cd60a1e | 10 21.00 0.00 0.00 0 0 0.000 0.000 | 31 4.00 -1397.00 0.00 0 0 0.000 0.000
442 2cd60a1e | 10 22.00 0.00 0.00 0 0 0.000 0.000 | 31 5.00 -1397.00 0.00 0 0 0.000 0.000
443 2cd60a1e | 10 23.00 0.00 0.00 0 0 0.000 0.000 | 31 6.00 -1397.00 0.00 0 0 0.000 0.000
444 2cd60a1e | 10 24.00 0.00 0.00 0 0 0.000 0.000 | 10 0.00 -1397.00 0.00 0 0 0.000 0.000
445 2cd60a1e | 10 25.00 0.00 0.00 0 0 0.000 0.000 | 10 1.00 -1397.00 0.00 0 0 0.000 0.000
446 2cd60a1e | 10 26.00 0.00 0.00 0 0 0.000 0.000 | 10 2.00 -1397.00 0.00 0 0 0.000 0.000
447 2cd60a1e | 10 27.00 0.00 0.00 0 0 0.000 0.000 | 10 3.00 -1397.00 0.00 0 0 0.000 0.000
448 2cd60a1e | 10 28.00 0.00 0.00 0 0 0.000 0.000 | 10 4.00 -1397.00 0.00 0 0 0.000 0.000
449 2cd60a1e | 10 29.00 0.00 0.00 0 0 0.000 0.000 | 10 5.00 -1397.00 0.00 0 0 0.000 0.000
450 2cd60a1e | 10 30.00 0.00 0.00 0 0 0.000 0.000 | 10 6.00 -1397.00 0.00 0 0 0.000 0.000
451 2cd60a1e | 10 31.00 0.00 0.00 0 0 0.000 0.000 | 10 7.00 -1397.00 0.00 0 0 0.000 0.000
452 2cd60a1e | 10 32.00 0.00 0.00 0 0 0.000 0.000 | 10 8.00 -1397.00 0.00 0 0 0.000 0.000
453 2cd60a1e | 10 33.00 0.00 0.00 0 0 0.000 0.000 | 10 9.00 -1397.00 0.00 0 0 0.000 0.000
454 2cd60a1e | 10 34.00 0.00 0.00 0 0 0.000 0.000 | 10 10.00 -1397.00 0.00 0 0 0.000 0.000
455 2cd60a1e | 10 35.00 0.00 0.00 0 0 0.000 0.000 | 10 11.00 -1397.00 0.00 0 0 0.000 0.000
456 2cd60a1e | 10 36.00 0.00 0.00 0 0 0.000 0.000 | 10 12.00 -1397.00 0.00 0 0 0.000 0.000
457 2cd60a1e | 10 37.00 0.00 0.00 0 0 0.000 0.000 | 10 13.00 -1397.00 0.00 0 0 0.000 0.000
458 2cd60a1e | 10 38.00 0.00 0.00 0 0 0.000 0.000 | 10 14.00 -1397.00 0.00 0 0 0.000 0.000
459 2cd60a1e | 10 39.00 0.00 0.00 0 0 0.000 0.000 | 10 15.00 -1397.00 0.00 0 0 0.000 0.000
460 2cd60a1e | 10 40.00 0.00 0.00 0 0 0.000 0.000 | 223 1.00 -1397.00 0.00 0 0 0.000 0.000
461 2cd60a1e | 10 41.00 0.00 0.00 0 0 0.000 0.000 | 223 2.00 -1397.00 0.00 0 0 0.000 0.000
462 2cd60a1e | 10 42.00 0.00 0.00 0 0 0.000 0.000 | 223 3.00 -1397.00 0.00 0 0 0.000 0.000
463 2cd60a1e | 10 43.00 0.00 0.00 0 0 0.000 0.000 | 223 4.00 -1397.00 0.00 0 0 0.000 0.000
464 2cd60a1e | 10 44.00 0.00 0.00 0 0 0.000 0.000 | 223 5.00 -1397.00 0.00 0 0 0.000 0.000
465 2cd60a1e | 10 45.00 0.00 0.00 0 0 0.000 0.000 | 223 6.00 -1397.00 0.00 0 0 0.000 0.000
466 2cd60a1e | 10 46.00 0.00 0.00 0 0 0.000 0.000 | 223 7.00 -1397.00 0.00 0 0 0.000 0.000
467 2cd60a1e | 10 47.00 0.00 0.00 0 0 0.000 0.000 | 223 8.00 -1397.00 0.00 0 0 0.000 0.000
468 2cd60a1e | 10 48.00 0.00 0.00 0 0 0.000 0.000 | 223 9.00 -1397.00 0.00 0 0 0.000 0.000
469 2cd60a1e | 10 49.00 0.00 0.00 0 0 0.000 0.000 | 223 10.00 -1397.00 0.00 0 0 0.000 0.000
470 2cd60a1e | 223 1.00 0.00 0.00 0 0 0.000 0.000 | 223 11.00 -1397.00 0.00 0 0 0.000 0.000
471 2cd60a1e | 223 2.00 0.00 0.00 0 0 0.000 0.000 | 223 12.00 -1397.00 0.00 0 0 0.000 0.000
472 2cd60a1e | 223 3.00 0.00 0.00 0 0 0.000 0.000 | 223 13.00 -1397.00 0.00 0 0 0.000 0.000
473 2cd60a1e | 223 4.00 0.00 0.00 0 0 0.000 0.000 | 223 14.00 -1397.00 0.00 0 0 0.000 0.000
474 2cd60a1e | 223 5.00 0.00 0.00 0 0 0.000 0.000 | 223 15.00 -1397.00 0.00 0 0 0.000 0.000
475 2cd60a1e | 223 6.00 0.00 0.00 0 0 0.000 0.000 | 223 16.00 -1397.00 0.00 0 0 0.000 0.000 |  | 0 -1164.86 212.65 49.810 -5.558
476 2cd60a1e | 223 7.00 0.00 0.00 0 0 0.000 0.000 | 223 17.00 -1397.00 0.00 0 0 0.000 0.000 |  | 0 -1115.05 205.89 49.810 -6.758
477 2cd60a1e | 223 8.00 0.00 0.00 0 0 0.000 0.000 | 223 18.00 -1397.00 0.00 0 0 0.000 0.000 |  | 0 -1065.24 197.93 49.810 -7.958
478 2cd60a1e | 223 9.00 0.00 0.00 0 0 0.000 0.000 | 223 19.00 -1397.00 0.00 0 0 0.000 0.000 |  | 0 -1015.43 188.77 49.810 -9.158
479 2cd60a1e | 223 10.00 0.00 0.00 0 0 0.000 0.000 | 223 20.00 -1397.00 0.00 0 0 0.000 0.000 |  | 0 -965.62 178.42 49.810 -10.358
480 2cd60a1e | 223 11.00 0.00 0.00 0 0 0.000 0.000 | 223 21.00 -1397.00 0.00 0 0 0.000 0.000 |  | 0 -915.81 166.86 49.810 -11.558
481 2cd60a1e | 223 12.00 0.00 0.00 0 0 0.000 0.000 | 223 22.00 -1397.00 0.00 0 0 0.000 0.000 |  | 0 -866.00 154.10 49.810 -12.758
482 2cd60a1e | 223 13.00 0.00 0.00 0 0 0.000 0.000 | 223 23.00 -1397.00 0.00 0 0 0.000 0.000 |  | 0 -816.19 140.14 49.810 -13.958
483 2cd60a1e | 223 14.00 0.00 0.00 0 0 0.000 0.000 | 223 24.00 -1397.00 0.00 0 0 0.000 0.000 |  | 0 -766.38 124.99 49.810 -15.158
484 2cd60a1e | 223 15.00 0.00 0.00 0 0 0.000 0.000 | 223 25.00 -1397.00 0.00 0 0 0.000 0.000 |  | 0 -716.57 108.63 49.810 -16.358
485 2cd60a1e | 223 16.00 0.00 0.00 0 0 0.000 0.000 | 223 26.00 -1397.00 0.00 0 0 0.000 0.000 |  | 0 -666.76 91.07 49.810 -17.558 ; 0 -232.14 212.65 -49.810 -5.558
486 2cd60a1e | 223 17.00 0.00 0.00 0 0 0.000 0.000 | 223 27.00 -1397.00 0.00 0 0 0.000 0.000 |  | 0 -616.95 72.31 49.810 -18.758 ; 0 -281.95 205.89 -49.810 -6.758
487 2cd60a1e | 223 18.00 0.00 0.00 0 0 0.000 0.000 | 223 28.00 -1397.00 0.00 0 0 0.000 0.000 |  | 0 -567.14 52.35 49.810 -19.958 ; 0 -331.76 197.93 -49.810 -7.958
488 1649b442 | 223 19.00 0.00 0.00 0 0 0.000 0.000 | 223 29.00 -1397.00 0.00 0 0 0.000 0.000
489 e6bb16d7 | 223 20.00 0.00 0.00 0 0 0.000 0.000 | 223 30.00 -1397.00 0.00 0 0 0.000 0.000
490 f67de79d | 223 21.00 0.00 0.00 0 0 0.000 0.000 | 223 31.00 -1397.00 0.00 0 0 0.000 0.000
491 9ad4bb73 | 223 22.00 0.00 0.00 0 0 0.000 0.000 | 223 32.00 -1397.00 0.00 0 0 0.000 0.000
492 b23bc7d9 | 223 23.00 0.00 0.00 0 0 0.000 0.000 | 223 33.00 -1397.00 0.00 0 0 0.000 0.000
493 8bdb664f | 223 24.00 0.00 0.00 0 0 0.000 0.000 | 223 34.00 -1397.00 0.00 0 0 0.000 0.000
494 22ee7455 | 223 25.00 0.00 0.00 0 0 0.000 0.000 | 223 35.00 -1397.00 0.00 0 0 0.000 0.000
495 1a49b36b | 223 26.00 0.00 0.00 0 0 0.000 0.000 | 223 36.00 -1397.00 0.00 0 0 0.000 0.000
496 8ddc2911 | 223 27.00 0.00 0.00 0 0 0.000 0.000 | 223 37.00 -1397.00 0.00 0 0 0.000 0.000
497 2e407ec7 | 223 28.00 0.00 0.00 0 0 0.000 0.000 | 223 38.00 -1397.00 0.00 0 0 0.000 0.000
498 7eb7620d | 223 29.00 0.00 0.00 0 0 0.000 0.000 | 223 39.00 -1397.00 0.00 0 0 0.000 0.000
499 bef6be20 | 223 30.00 0.00 0.00 0 0 0.000 0.000 | 223 40.00 -1397.00 0.00 0 0 0.000 0.000
500 92e148e4 | 223 31.00 0.00 0.00 0 0 0.000 0.000 | 223 41.00 -1397.00 0.00 0 0 0.000 0.000
501 58b070e8 | 223 32.00 0.00 0.00 0 0 0.000 0.000 | 223 42.00 -1397.00 0.00 0 0 0.000 0.000
502 fbb95a2c | 223 33.00 0.00 0.00 0 0 0.000 0.000 | 223 43.00 -1397.00 0.00 0 0 0.000 0.000
503 704968b1 | 223 34.00 0.00 0.00 0 0 0.000 0.000 | 223 44.00 -1397.00 0.00 0 0 0.000 0.000
504 187ebcaa | 223 35.00 0.00 0.00 0 0 0.000 0.000 | 223 45.00 -1397.00 0.00 0 0 0.000 0.000
505 68994074 | 223 36.00 0.00 0.00 0 0 0.000 0.000 | 10 0.00 -1397.00 0.00 0 0 0.000 0.000
506 13e9f28e | 223 37.00 0.00 0.00 0 0 0.000 0.000 | 10 1.00 -1397.00 0.00 0 0 0.000 0.000
507 5db2c578 | 223 38.00 0.00 0.00 0 0 0.000 0.000 | 10 2.00 -1397.00 0.00 0 0 0.000 0.000
508 a3c93fb2 | 223 39.00 0.00 0.00 0 0 0.000 0.000 | 10 3.00 -1397.00 0.00 0 0 0.000 0.000
509 48cc27ad | 223 40.00 0.00 0.00 0 0 0.000 0.000 | 10 4.00 -1397.00 0.00 0 0 0.000 0.000
510 48cc27ad | 223 41.00 0.00 0.00 0 0 0.000 0.000 | 10 5.00 -1397.00 0.00 0 0 0.000 0.000
511 48cc27ad | 223 42.00 0.00 0.00 0 0 0.000 0.000 | 10 6.00 -1397.00 0.00 0 0 0.000 0.000
512 48cc27ad | 223 43.00 0.00 0.00 0 0 0.000 0.000 | 10 7.00 -1397.00 0.00 0 0 0.000 0.000
513 48cc27ad | 223 44.00 0.00 0.00 0 0 0.000 0.000 | 10 8.00 -1397.00 0.00 0 0 0.000 0.000
514 48cc27ad | 223 45.00 0.00 0.00 0 0 0.000 0.000 | 10 9.00 -1397.00 0.00 0 0 0.000 0.000
515 48cc27ad | 10 0.00 0.00 0.00 0 0 0.000 0.000 | 10 10.00 -1397.00 0.00 0 0 0.000 0.000
516 48cc27ad | 10 1.00 0.00 0.00 0 0 0.000 0.000 | 10 11.00 -1397.00 0.00 0 0 0.000 0.000
517 48cc27ad | 10 2.00 0.00 0.00 0 0 0.000 0.000 | 10 12.00 -1397.00 0.00 0 0 0.000 0.000
518 48cc27ad | 10 3.00 0.00 0.00 0 0 0.000 0.000 | 10 13.00 -1397.00 0.00 0 0 0.000 0.000
519 48cc27ad | 10 4.00 0.00 0.00 0 0 0.000 0.000 | 10 14.00 -1397.00 0.00 0 0 0.000 0.000
520 48cc27ad | 10 5.00 0.00 0.00 0 0 0.000 0.000 | 10 15.00 -1397.00 0.00 0 0 0.000 0.000
521 48cc27ad | 10 6.00 0.00 0.00 0 0 0.000 0.000 | 10 16.00 -1397.00 0.00 0 0 0.000 0.000
522 48cc27ad | 10 7.00 0.00 0.00 0 0 0.000 0.000 | 10 17.00 -1397.00 0.00 0 0 0.000 0.000
523 48cc27ad | 10 8.00 0.00 0.00 0 0 0.000 0.000 | 10 18.00 -1397.00 0.00 0 0 0.000 0.000
524 48cc27ad | 10 9.00 0.00 0.00 0 0 0.000 0.000 | 10 19.00 -1397.00 0.00 0 0 0.000 0.000
525 48cc27ad | 10 10.00 0.00 0.00 0 0 0.000 0.000 | 10 20.00 -1397.00 0.00 0 0 0.000 0.000
526 48cc27ad | 10 11.00 0.00 0.00 0 0 0.000 0.000 | 10 21.00 -1397.00 0.00 0 0 0.000 0.000
527 48cc27ad | 10 12.00 0.00 0.00 0 0 0.000 0.000 | 10 22.00 -1397.00 0.00 0 0 0.000 0.000
528 48cc27ad | 10 13.00 0.00 0.00 0 0 0.000 0.000 | 10 23.00 -1397.00 0.00 0 0 0.000 0.000
529 48cc27ad | 10 14.00 0.00 0.00 0 0 0.000 0.000 | 10 24.00 -1397.00 0.00 0 0 0.000 0.000
530 48cc27ad | 10 15.00 0.00 0.00 0 0 0.000 0.000 | 10 25.00 -1397.00 0.00 0 0 0.000 0.000
531 48cc27ad | 10 16.00 0.00 0.00 0 0 0.000 0.000 | 10 26.00 -1397.00 0.00 0 0 0.000 0.000
532 48cc27ad | 10 17.00 0.00 0.00 0 0 0.000 0.000 | 10 27.00 -1397.00 0.00 0 0 0.000 0.000
533 48cc27ad | 10 18.00 0.00 0.00 0 0 0.000 0.000 | 10 28.00 -1397.00 0.00 0 0 0.000 0.000
534 48cc27ad | 10 19.00 0.00 0.00 0 0 0.000 0.000 | 10 29.00 -1397.00 0.00 0 0 0.000 0.000
535 48cc27ad | 10 20.00 0.00 0.00 0 0 0.000 0.000 | 10 30.00 -1397.00 0.00 0 0 0.000 0.000
536 d7531bbc | 10 21.00 0.00 0.00 0 0 0.000 0.000 | 10 31.00 -1397.00 0.00 0 0 0.000 0.000
537 d7531bbc | 10 22.00 0.00 0.00 0 0 0.000 0.000 | 10 32.00 -1397.00 0.00 0 0 0.000 0.000
538 d7531bbc | 10 23.00 0.00 0.00 0 0 0.000 0.000 | 10 33.00 -1397.00 0.00 0 0 0.000 0.000
539 d7531bbc | 10 24.00 0.00 0.00 0 0 0.000 0.000 | 10 34.00 -1397.00 0.00 0 0 0.000 0.000
540 d7531bbc | 10 25.00 0.00 0.00 0 0 0.000 0.000 | 10 35.00 -1397.00 0.00 0 0 0.000 0.000
541 d7531bbc | 10 26.00 0.00 0.00 0 0 0.000 0.000 | 10 36.00 -1397.00 0.00 0 0 0.000 0.000
542 d7531bbc | 10 27.00 0.00 0.00 0 0 0.000 0.000 | 10 37.00 -1397.00 0.00 0 0 0.000 0.000
543 d7531bbc | 10 28.00 0.00 0.00 0 0 0.000 0.000 | 10 38.00 -1397.00 0.00 0 0 0.000 0.000
544 d7531bbc | 10 29.00 0.00 0.00 0 0 0.000 0.000 | 10 39.00 -1397.00 0.00 0 0 0.000 0.000
545 d7531bbc | 10 30.00 0.00 0.00 0 0 0.000 0.000 | 10 40.00 -1397.00 0.00 0 0 0.000 0.000
546 d7531bbc | 10 31.00 0.00 0.00 0 0 0.000 0.000 | 10 41.00 -1397.00 0.00 0 0 0.000 0.000
547 d7531bbc | 10 32.00 0.00 0.00 0 0 0.000 0.000 | 10 42.00 -1397.00 0.00 0 0 0.000 0.000
548 d7531bbc | 10 33.00 0.00 0.00 0 0 0.000 0.000 | 10 43.00 -1397.00 0.00 0 0 0.000 0.000
549 d7531bbc | 10 34.00 0.00 0.00 0 0 0.000 0.000 | 10 44.00 -1397.00 0.00 0 0 0.000 0.000
550 d7531bbc | 10 35.00 0.00 0.00 0 0 0.000 0.000 | 10 45.00 -1397.00 0.00 0 0 0.000 0.000
551 d7531bbc | 10 36.00 0.00 0.00 0 0 0.000 0.000 | 10 46.00 -1397.00 0.00 0 0 0.000 0.000
552 d7531bbc | 10 37.00 0.00 0.00 0 0 0.000 0.000 | 10 47.00 -1397.00 0.00 0 0 0.000 0.000
553 d7531bbc | 10 38.00 0.00 0.00 0 0 0.000 0.000 | 10 48.00 -1397.00 0.00 0 0 0.000 0.000
554 d7531bbc | 10 39.00 0.00 0.00 0 0 0.000 0.000 | 10 49.00 -1397.00 0.00 0 0 0.000 0.000
555 d7531bbc | 10 40.00 0.00 0.00 0 0 0.000 0.000 | 10 0.00 -1397.00 0.00 0 0 0.000 0.000
556 d7531bbc | 10 41.00 0.00 0.00 0 0 0.000 0.000 | 10 1.00 -1397.00 0.00 0 0 0.000 0.000
557 d7531bbc | 10 42.00 0.00 0.00 0 0 0.000 0.000 | 10 2.00 -1397.00 0.00 0 0 0.000 0.000
558 d7531bbc | 10 43.00 0.00 0.00 0 0 0.000 0.000 | 10 3.00 -1397.00 0.00 0 0 0.000 0.000
559 d7531bbc | 10 44.00 0.00 0.00 0 0 0.000 0.000 | 10 4.00 -1397.00 0.00 0 0 0.000 0.000
560 d7531bbc | 10 45.00 0.00 0.00 0 0 0.000 0.000 | 10 5.00 -1397.00 0.00 0 0 0.000 0.000
";

/// `vsm1`'s routes: P1 Mario's Fireball; P2 Mario's drop and Fireball.
const VSM1_P1: &str = "470:3:b";
const VSM1_P2: &str = "405:3:y=-80;460:3:b";

#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn two_fighters_fireballs_clank_as_on_the_n64() {
    let replay = Replay {
        p1: FighterKind::Mario,
        route: VSM1_P1,
        weapons: &fighter_weapons,
    };
    let vs = Vs {
        p2: FighterKind::Mario,
        route2: VSM1_P2,
        spawns: VS_NESS_FOX,
    };
    let Some(out) = replay.run_vs(VS_FIREBALL_CLANK, &vs) else {
        return;
    };
    out.check("two Marios' Fireballs clank in VS");
}
