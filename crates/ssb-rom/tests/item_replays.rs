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
use posed::{tick_pair, Pair, Posed};

#[path = "common/dream_land.rs"]
mod dream_land;
use dream_land::{
    camera_stage, dream_land_map, map_groups, stage_bounds, stage_floors, training_fighter,
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
/// gate_pos.y`).
struct Row {
    t: u64,
    seed: Option<i32>,
    groups: [Vec<Vec<f32>>; 6],
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
            let mut groups: [Vec<Vec<f32>>; 6] = Default::default();
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
/// `sc1PTrainingModeUpdateItemOption`).
struct Route {
    /// `(first tick, last tick, (buttons, stick x, stick y))`.
    steps: Vec<(u64, u64, (u16, i8, i8))>,
    /// `(tick, ITKind)`.
    menu: Vec<(u64, u8)>,
}

impl Route {
    fn parse(route: &str) -> Self {
        let mut out = Route {
            steps: Vec::new(),
            menu: Vec::new(),
        };
        let (mut open, mut option) = (false, 0u8);
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
                continue;
            }
            if open {
                if x > 0 {
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

    fn pad(&self, tick: u64) -> (u16, i8, i8) {
        self.steps
            .iter()
            .find(|(a, b, _)| (*a..=*b).contains(&tick))
            .map_or((0, 0, 0), |s| s.2)
    }
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
        self.run_stage(stage_id, table, monster, dummy_percent, None)
    }

    /// [`Self::run_forced`] on Saffron with its gate controller, and
    /// `monster_wait` written after frame `t` as the trace's poke did.
    fn run_saffron(&self, table: &str, monster: u8, monster_wait: (u64, u16)) -> Option<Outcome> {
        let bytes = open_pack()?;
        let pack = ssb_rom::pack::Pack::open(&bytes).unwrap();
        let stage_id = pack
            .stage_of_file(ssb_rom::stage::VS_GROUND_FILES[SAFFRON_GKIND as usize])
            .unwrap();
        self.run_stage(stage_id, table, monster, None, Some(monster_wait))
    }

    /// [`Self::run_on`]; with `gate = Some((t, wait))` the stage's own
    /// controller runs (`StageInit` from the pack, as `hyrule_pair`) and
    /// Saffron's `monster_wait` is written after frame `t`.
    fn run_stage(
        &self,
        stage_id: u32,
        table: &str,
        monster: u8,
        dummy_percent: Option<(u64, u16)>,
        gate: Option<(u64, u16)>,
    ) -> Option<Outcome> {
        let bytes = open_pack()?;
        let pack = ssb_rom::pack::Pack::open(&bytes).unwrap();
        let (stage, segments) = stage_floors(&pack, stage_id);
        let map = dream_land_map(&pack, &stage);
        let mut fighters: Vec<Posed> = [(self.p1, 0), (FighterKind::Mario, 1)]
            .into_iter()
            .map(|(kind, port)| {
                let mut f = training_fighter(&pack, &stage, kind, port, &segments);
                f.dead.bounds = Some(stage_bounds(&pack, &stage));
                Posed::new(&pack, f)
            })
            .collect();
        let want = parse(table);
        let route = Route::parse(self.route);
        let pad = |t: u64| route.pad(t);
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
        let mut camera = want[0].groups[4].first().map(|c| {
            let mut rig = ssb_game::camera::CameraRig::default();
            rig.camera.eye = ssb_engine::math::Vec3::new(c[0], c[1], c[2]);
            rig.camera.at = ssb_engine::math::Vec3::new(c[3], c[4], c[5]);
            rig.camera.fovy_degrees = c[6];
            rig.camera.target_dist = c[7];
            (rig, camera_stage(&stage))
        });
        let framing = [self.p1, FighterKind::Mario].map(|kind| {
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
        for tick in 0..=last {
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
            let _ = tick_pair(&pack, &map, &mut fighters, &pad, tick, &mut pair);
            if let Some((_, percent)) = dummy_percent.filter(|(t, _)| *t == tick) {
                fighters[1].fighter.damage = percent;
            }
            if let Some((_, wait)) = gate.filter(|(t, _)| *t == tick) {
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
                    let wait = if gate.is_some_and(|(t, _)| tick >= t) && !effect_draws {
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
            let got: [Vec<Vec<f32>>; 6] = [vec![c0], vec![c1], items, weapons, cam, gate_row];
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
