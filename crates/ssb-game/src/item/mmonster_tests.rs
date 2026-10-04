//! The Poké Ball's Pokémon and their weapons (RE-435).
use super::mmonster::{Kind, Status};
use super::*;
use crate::fighter::FighterKind;
use crate::monster_weapon::{MonsterShot, ShotKind};
use crate::status;
use crate::weapon::WeaponPool;

fn floor() -> MapSurface {
    crate::map::floor_surface((
        0,
        crate::collision::Segment {
            x1: -4000,
            y1: 0,
            x2: 4000,
            y2: 0,
            flags: 0,
        },
    ))
}

const BOUNDS: BlastZone = BlastZone {
    top: 4000.0,
    bottom: -3000.0,
    left: -6000.0,
    right: 6000.0,
};

fn fighter(port: u8, x: f32) -> Fighter {
    let mut f = Fighter::new(FighterKind::Mario, port, 3);
    f.pos = Vec3::new(x, 0.0, 0.0);
    f.floor = Some(Standing {
        line: 0,
        normal: Vec2::new(0.0, 1.0),
        flags: 0,
    });
    status::set_wait(&mut f);
    f
}

/// Items, weapons and fighters run their main processes in the source's
/// order.
struct World {
    items: ItemPool,
    weapons: WeaponPool,
    fighters: Vec<Fighter>,
    /// Item weapons made on each step, by kind.
    made: Vec<Vec<MonsterShot>>,
}

impl World {
    fn new(fighters: &[(u8, f32)]) -> Self {
        World {
            items: ItemPool::default(),
            weapons: WeaponPool::default(),
            fighters: fighters.iter().map(|&(p, x)| fighter(p, x)).collect(),
            made: Vec::new(),
        }
    }

    /// A Pokémon from a ball at `x` on the floor that fighter 0 threw.
    fn spawn(&mut self, kind: Kind, x: f32) -> u8 {
        for f in &self.fighters {
            self.items.observe_owner(f);
        }
        let ball_y = mball::ATTRIBUTES.map_coll.top;
        self.items
            .spawn_mmonster(kind, Vec3::new(x, ball_y, 0.0), Some(0), 0, &|| [floor()])
            .unwrap()
    }

    fn step(&mut self) {
        self.items.observe_weapons(&mut self.weapons);
        for f in &self.fighters {
            self.items.observe_owner(f);
        }
        self.items
            .tick(|| [floor()], Some(BOUNDS), &[], &mut NoItemAnims);
        let before: Vec<_> = self.weapons.monster_shots().collect();
        self.items.flush_monster_shots(&mut self.weapons);
        let made = self
            .weapons
            .monster_shots()
            .filter(|s| s.plays == 0 && !before.contains(s))
            .collect();
        self.made.push(made);
        self.weapons.tick(|| [floor()], Some(BOUNDS));
    }

    fn item(&self, slot: u8) -> &Item {
        self.items.get(slot).unwrap()
    }

    fn status(&self, slot: u8) -> Status {
        let ItemStatus::MMonster(s) = self.item(slot).status else {
            panic!()
        };
        s
    }

    /// Steps until the item's status leaves `status`; returns the steps.
    fn until_not(&mut self, slot: u8, status: Status, max: usize) -> usize {
        for n in 1..=max {
            self.step();
            if self.items.get(slot).is_none() || self.status(slot) != status {
                return n;
            }
        }
        panic!("still {status:?} after {max}");
    }

    fn made_of(&self, kind: ShotKind) -> Vec<(usize, MonsterShot)> {
        self.made
            .iter()
            .enumerate()
            .flat_map(|(n, v)| v.iter().map(move |s| (n, *s)))
            .filter(|(_, s)| s.kind == kind)
            .collect()
    }
}

/// A seed whose next `rand_int_range(range)` is `want`.
fn seed_for(range: i32, want: i32) -> i32 {
    (1..)
        .find(|&s| {
            crate::rng::set_seed(s);
            crate::rng::rand_int_range(range) == want
        })
        .unwrap()
}

#[test]
fn every_draw_makes_its_pokemon_rising_for_22_frames() {
    let mut w = World::new(&[(0, -2000.0), (1, 2000.0)]);
    for (i, kind) in Kind::ALL.into_iter().enumerate() {
        assert_eq!(Kind::from_item_kind(32 + i as u8), Some(kind));
        assert_eq!(kind.item_kind(), 32 + i as u8);
        let slot = w.spawn(kind, 0.0);
        let item = *w.item(slot);
        assert_eq!(item.kind, ItemKind::MMonster(kind));
        assert_eq!(item.status, ItemStatus::MMonster(Status::Common));
        assert_eq!((item.multi, item.vel_air), (22, Vec3::new(0.0, 16.0, 0.0)));
        // The ball's owner and team come back after the maker; the three
        // that cleared them keep hitting everyone.
        assert_eq!((item.owner, item.team, item.player), (Some(0), 0, Some(0)));
        assert_eq!(
            item.is_damage_all,
            matches!(kind, Kind::Iwark | Kind::Tosakinto | Kind::Spear)
        );
        let off = matches!(
            kind,
            Kind::Tosakinto | Kind::MLucky | Kind::Dogas | Kind::Mew
        );
        assert_eq!(item.attack.state == AttackState::Off, off, "{kind:?}");
        let fighters_only = matches!(kind, Kind::Iwark | Kind::Kabigon | Kind::Spear);
        assert_eq!(item.attack.interact_mask == INTERACT_FIGHTER, fighters_only);
        assert_eq!(item.damage_coll.hitstatus, HitStatus::None);
        assert_eq!(item.ty, ItemType::Fighter);
        // Snorlax keeps the projected position; the rest lift by their
        // map box's bottom.
        let ball_y = mball::ATTRIBUTES.map_coll.top;
        if kind != Kind::Kabigon && kind != Kind::Mew {
            assert_eq!(item.pos.y, ball_y - item.attr.map_coll.bottom);
        }
        w.items.destroy(slot);
    }
}

#[test]
fn an_opened_ball_releases_one_pokemon_with_its_throwers_stats() {
    let mut w = World::new(&[(0, -2000.0), (1, 2000.0)]);
    let slot = w
        .items
        .make_setup_common(19, None, Vec3::new(0.0, 109.0, 0.0), Vec3::ZERO, &|| {
            [floor()]
        })
        .unwrap();
    {
        let ball = w.items.get_mut(slot).unwrap();
        ball.owner = Some(1);
        ball.player = Some(1);
        ball.team = 1;
        ball.handicap = 7;
        ball.status = ItemStatus::MBall(mball::Status::Open);
        ball.multi = 0;
    }
    crate::rng::set_seed(1);
    w.step();
    assert!(w
        .items
        .get(slot)
        .is_none_or(|i| !matches!(i.kind, ItemKind::MBall)));
    let monsters: Vec<_> = w
        .items
        .items()
        .filter(|i| matches!(i.kind, ItemKind::MMonster(_)))
        .collect();
    assert_eq!(monsters.len(), 1);
    let m = monsters[0];
    assert_eq!(
        (m.owner, m.player, m.team, m.handicap),
        (Some(1), Some(1), 1, 7)
    );
}

#[test]
fn meowth_throws_four_coins_every_eight_frames_turning_13_degrees() {
    let mut w = World::new(&[(0, -2000.0), (1, 2000.0)]);
    let slot = w.spawn(Kind::Nyars, 0.0);
    assert_eq!(w.until_not(slot, Status::Common, 30), 23);
    assert_eq!(w.item(slot).multi, 239);
    let n = w.until_not(slot, Status::NyarsAttack, 400);
    assert_eq!(n, 240);
    let coins = w.made_of(ShotKind::NyarsCoin);
    // Sets at multi 236, 228, ... 4: thirty of four.
    assert_eq!(coins.len(), 120);
    let first = coins[0].0;
    for (k, chunk) in coins.chunks(4).enumerate() {
        assert!(chunk.iter().all(|(step, _)| *step == first + k * 8));
        for (i, (_, c)) in chunk.iter().enumerate() {
            let deg = i as f32 * 90.0 + k as f32 * 13.0;
            let (sin, cos) = ssb_engine::math::sin_cos(deg.to_radians());
            assert!((c.velocity.x - 130.0 * cos).abs() < 1e-3);
            assert!((c.velocity.y - 130.0 * sin).abs() < 1e-3);
        }
    }
}

#[test]
fn a_coin_lives_ten_updates_and_dies_on_a_hit() {
    let mut pool = WeaponPool::default();
    let p = crate::monster_weapon::ShotParent {
        stat: crate::spgame::live::AttackStat::default(),
        owner: Some(0),
        player: Some(0),
        team: 0,
        lr: 1.0,
        handle: 0,
    };
    pool.spawn_monster_shot(MonsterShot::coin(p, Vec3::new(0.0, 2000.0, 0.0), 0, 0.0));
    // `itNyarsWeaponCoinProcUpdate` tests before it counts down.
    for _ in 0..10 {
        pool.tick(|| [floor()], Some(BOUNDS));
        assert_eq!(pool.monster_shots().count(), 1);
    }
    pool.tick(|| [floor()], Some(BOUNDS));
    assert_eq!(pool.monster_shots().count(), 0);
    // On a fighter: `itNyarsWeaponCoinProcHit` returns TRUE.
    let mut pool = WeaponPool::default();
    pool.spawn_monster_shot(MonsterShot::coin(p, Vec3::new(0.0, 150.0, 0.0), 0, 0.0));
    let mut luigi = fighter(1, 0.0);
    pool.apply_hits(&mut luigi);
    crate::combat::resolve(&mut luigi);
    assert_eq!(luigi.damage, 6);
    assert_eq!(pool.monster_shots().count(), 0);
}

#[test]
fn koffing_makes_31_clouds_and_leaves_90_frames_later() {
    let mut w = World::new(&[(0, -2000.0), (1, 2000.0)]);
    let slot = w.spawn(Kind::Dogas, 0.0);
    w.until_not(slot, Status::Common, 30);
    let n = w.until_not(slot, Status::DogasAttack, 400);
    let clouds = w.made_of(ShotKind::DogasSmog);
    assert_eq!(clouds.len(), 31);
    assert!(clouds.windows(2).all(|c| c[1].0 - c[0].0 == 8));
    assert_eq!(n, 30 * 8 + 1);
    // Each drifts away from Koffing at 18.
    let at = w.item(slot).pos;
    for (_, c) in &clouds {
        assert_eq!(c.velocity.x.abs(), 18.0);
        assert_eq!(c.velocity.x < 0.0, c.position.x < at.x);
    }
    assert_eq!(w.until_not(slot, Status::DogasDisappear, 200), 91);
    assert!(w.items.get(slot).is_none());
}

#[test]
fn the_smog_grows_with_its_animation_and_passes_shields() {
    let mut pool = WeaponPool::default();
    let shot = MonsterShot::smog(
        crate::monster_weapon::ShotParent::GROUND,
        Vec3::new(0.0, 2000.0, 0.0),
        Vec3::ZERO,
    );
    pool.spawn_monster_shot(shot);
    let mut sizes = Vec::new();
    for _ in 0..40 {
        pool.tick(|| [floor()], Some(BOUNDS));
        let Some(s) = pool.monster_shots().next() else {
            break;
        };
        sizes.push(s.size);
    }
    assert_eq!(sizes.len(), 29);
    assert_eq!(sizes[0], 50.0);
    assert_eq!(sizes[28], 275.0);
    assert!(!crate::monster_weapon::ATTRIBUTES[ShotKind::DogasSmog as usize].can_shield);
}

#[test]
fn onix_drops_its_rocks_and_leaves_once_every_rock_is_gone() {
    let mut w = World::new(&[(0, -2000.0), (1, 2000.0)]);
    let slot = w.spawn(Kind::Iwark, 0.0);
    w.until_not(slot, Status::Common, 30);
    // The descriptor's update counts the new 30 down once.
    assert_eq!(w.until_not(slot, Status::IwarkFly, 40), 30);
    let max = w.item(slot).vars.mmonster.rock_spawn_max;
    assert!((8..=16).contains(&max));
    assert_eq!(w.item(slot).vel_air.y, 80.0);
    let mut steps = 0;
    while w.items.get(slot).is_some() {
        w.step();
        steps += 1;
        assert!(steps < 3000);
        // The update clamps it the frame after it crosses the stop line.
        if let Some(item) = w.items.get(slot) {
            assert!(item.pos.y <= BOUNDS.top - 200.0 + 80.0);
        }
    }
    let rocks = w.made_of(ShotKind::IwarkRock);
    assert_eq!(rocks.len(), usize::from(max));
    // A rock bounces off the floor once, then falls through it.
    assert!(w.weapons.monster_shots().next().is_none());
}

#[test]
fn a_rock_bounces_once_per_floor_and_reports_its_end() {
    let mut pool = WeaponPool::default();
    let p = crate::monster_weapon::ShotParent {
        stat: crate::spgame::live::AttackStat::default(),
        owner: None,
        player: None,
        team: crate::team::TEAM_DEFAULT,
        lr: 1.0,
        handle: 77,
    };
    crate::rng::set_seed(seed_for(2, 1));
    pool.spawn_monster_shot(MonsterShot::rock(p, Vec3::new(0.0, 600.0, 0.0), 0));
    let mut events = Vec::new();
    let mut bounced = 0;
    for _ in 0..200 {
        let before = pool.monster_shots().next();
        pool.tick(|| [floor()], Some(BOUNDS));
        events.extend(pool.take_rock_events());
        if let (Some(b), Some(a)) = (before, pool.monster_shots().next()) {
            if b.velocity.y < 0.0 && a.velocity.y > 0.0 {
                bounced += 1;
            }
        }
    }
    assert_eq!(bounced, 1);
    assert_eq!(events, [(77, false), (77, true)]);
}

#[test]
fn beedrill_calls_sixteen_across_the_stage_and_goes() {
    let mut w = World::new(&[(0, -2000.0), (1, 2000.0)]);
    let slot = w.spawn(Kind::Spear, 0.0);
    w.until_not(slot, Status::Common, 30);
    // Its appear animation's frame 51.
    assert_eq!(w.until_not(slot, Status::SpearAppear, 60), 51);
    let lr = w.item(slot).lr;
    let mut steps = 0;
    while w.items.get(slot).is_some() {
        w.step();
        steps += 1;
        assert!(steps < 2000);
    }
    let swarm = w.made_of(ShotKind::SpearSwarm);
    assert_eq!(swarm.len(), 16);
    for (_, s) in &swarm {
        assert_eq!((s.lr, s.velocity.x), (-lr, -lr * 130.0));
        // Where Beedrill stopped, past the call line.
        if lr == 1.0 {
            assert!(s.position.x >= BOUNDS.right - 500.0);
        } else {
            assert!(s.position.x <= BOUNDS.left + 500.0);
        }
    }
    assert!(swarm
        .windows(2)
        .all(|s| (12..=20).contains(&(s[1].0 - s[0].0))));
    // Members end 500 inside the far bound.
    while w.weapons.monster_shots().next().is_some() {
        w.step();
    }
}

#[test]
fn blastoise_faces_its_nearest_opponent_and_is_pushed_back_by_each_stream() {
    let mut w = World::new(&[(0, -500.0), (1, 2500.0), (2, -1500.0)]);
    let slot = w.spawn(Kind::Kamex, 0.0);
    // Port 0 threw it; port 2 is nearer than port 1.
    assert_eq!(w.item(slot).lr, -1.0);
    w.until_not(slot, Status::Common, 30);
    w.until_not(slot, Status::KamexAppear, 100);
    assert_eq!(w.status(slot), Status::KamexAttack);
    assert_eq!(w.item(slot).coll.top, 341.0);
    let start = w.made.len();
    for _ in 0..90 {
        w.step();
    }
    let streams: Vec<_> = w
        .made_of(ShotKind::KamexHydro)
        .into_iter()
        .filter(|(n, _)| *n >= start - 1)
        .collect();
    // `syUtilsRandIntRange(1) + 30`.
    assert!(streams.windows(2).all(|s| s[1].0 - s[0].0 == 30));
    let s = streams[0].1;
    assert_eq!(s.lr, -1.0);
    assert_eq!(s.velocity, Vec3::ZERO);
}

#[test]
fn the_hydro_pump_attack_reaches_along_its_animation() {
    let mut pool = WeaponPool::default();
    let p = crate::monster_weapon::ShotParent {
        stat: crate::spgame::live::AttackStat::default(),
        lr: -1.0,
        ..crate::monster_weapon::ShotParent::GROUND
    };
    pool.spawn_monster_shot(MonsterShot::hydro(p, Vec3::new(0.0, 2000.0, 0.0)));
    let mut reach = Vec::new();
    for _ in 0..30 {
        pool.tick(|| [floor()], Some(BOUNDS));
        let Some(s) = pool.monster_shots().next() else {
            break;
        };
        reach.push(s.attack_positions());
    }
    assert_eq!(reach.len(), 19);
    assert_eq!(reach[0].0.x, -60.0);
    // `wpManagerMakeWeapon` positions the attack before the Hydro Pump's
    // first animation play supplies its 60-unit offset.
    assert_eq!(reach[0].1, Vec3::new(0.0, 2000.0, 0.0));
    assert_eq!((reach[1].0.x, reach[1].1.x), (-204.375, -60.0));
}

#[test]
fn a_weapon_sweeps_its_first_process_from_its_maker_position() {
    let origin = Vec3::new(0.0, 2000.0, 0.0);
    for shot in [
        MonsterShot::saffron(false, origin),
        MonsterShot::saffron(true, origin),
        MonsterShot::hydro(crate::monster_weapon::ShotParent::GROUND, origin),
    ] {
        let mut pool = WeaponPool::default();
        pool.spawn_monster_shot(shot);
        assert_eq!(shot.attack_positions(), (origin, origin));
        pool.tick(|| [], None);
        let (current, previous) = pool.monster_shots().next().unwrap().attack_positions();
        assert_eq!(previous, origin, "{:?}", shot.kind);
        assert_ne!(current, previous, "{:?}", shot.kind);
    }
}

#[test]
fn mew_leaves_on_its_ripple_and_floats_off() {
    let mut w = World::new(&[(0, -2000.0), (1, 2000.0)]);
    let slot = w.spawn(Kind::Mew, 0.0);
    w.until_not(slot, Status::Common, 30);
    let item = *w.item(slot);
    assert_eq!(item.status, ItemStatus::MMonster(Status::MewFly));
    assert_eq!(item.vel_air.x.abs(), 8.0);
    assert_eq!(item.vel_air.y, -20.0);
    w.step();
    assert!((w.item(slot).vel_air.y - -19.2).abs() < 1e-4);
}

#[test]
fn goldeen_bounces_where_it_crosses_the_floor() {
    let mut w = World::new(&[(0, -2000.0), (1, 2000.0)]);
    let slot = w.spawn(Kind::Tosakinto, 0.0);
    w.until_not(slot, Status::Common, 30);
    w.until_not(slot, Status::TosakintoAppear, 100);
    let item = *w.item(slot);
    assert_eq!(item.vel_air, Vec3::new(10.0, 60.0, 0.0));
    // The floor test notes the crossing without stopping it.
    assert!(item.pos.y + item.attr.map_coll.bottom < 0.0);
    assert_eq!(item.ga, Ga::Air);
    let mut bounces = 0;
    let mut prev = item.vel_air.y;
    while w.items.get(slot).is_some() {
        w.step();
        if let Some(i) = w.items.get(slot) {
            if i.vel_air.y == 60.0 && prev < 0.0 {
                bounces += 1;
            }
            prev = i.vel_air.y;
        }
    }
    assert!(bounces > 5);
}

#[test]
fn chansey_lays_three_eggs_and_a_hit_holds_the_next_one() {
    let mut w = World::new(&[(0, -2000.0), (1, 2000.0)]);
    let slot = w.spawn(Kind::MLucky, 0.0);
    w.until_not(slot, Status::Common, 30);
    w.until_not(slot, Status::MLuckyAppear, 100);
    assert_eq!(w.item(slot).damage_coll.hitstatus, HitStatus::Normal);
    let eggs = |w: &World| {
        w.items
            .items()
            .filter(|i| i.kind == ItemKind::Container(container::Kind::Egg))
            .count()
    };
    let mut times = Vec::new();
    let mut gone = None;
    for n in 0..400 {
        let before = eggs(&w);
        w.step();
        if eggs(&w) > before {
            times.push(n);
        }
        if n == 40 {
            // `itMLuckyMakeEggProcDamage`.
            let item = w.items.get_mut(slot).unwrap();
            assert_eq!(
                mmonster::hit_proc(item, Status::MLuckyMakeEgg, HitProc::Damage),
                Some(true)
            );
        }
        if times.len() == 3 && n == times[2] + 30 {
            assert_eq!(w.status(slot), Status::MLuckyDisappear);
            assert_eq!(w.item(slot).damage_coll.hitstatus, HitStatus::None);
        }
        if w.items.get(slot).is_none() {
            gone = Some(n);
            break;
        }
    }
    assert_eq!(times.len(), 3);
    assert_eq!(times[1] - times[0], 30 + 4);
    assert_eq!(times[2] - times[1], 30);
    // The third egg empties the count; the next update disappears it, and
    // 91 more end it.
    assert_eq!(gone, Some(times[2] + 1 + 91));
}

#[test]
fn starmie_flies_beside_its_target_then_fires_and_recoils() {
    let mut w = World::new(&[(0, -2000.0), (1, 1500.0)]);
    let slot = w.spawn(Kind::Starmie, 0.0);
    w.until_not(slot, Status::Common, 30);
    let item = *w.item(slot);
    let target = item.vars.mmonster.target_pos;
    let f = w.fighters[1].coll;
    assert_eq!(target.x, 1500.0 - (f.width + 400.0));
    assert_eq!(target.y, 250.0 - f.bottom);
    assert_eq!(item.lr, 1.0);
    assert!((item.vel_air.length() - 20.0).abs() < 1e-3);
    w.until_not(slot, Status::StarmieNFollow, 400);
    assert_eq!(w.status(slot), Status::StarmieAttack);
    w.step();
    let swift = w.made_of(ShotKind::StarmieSwift);
    assert_eq!(swift.len(), 1);
    assert_eq!(swift[0].1.velocity.x, 150.0);
    // `ITSTARMIE_PUSH_VEL_X` then `ITSTARMIE_ADD_VEL_X`.
    assert_eq!(w.item(slot).vel_air.x, -70.0 + 10.0);
}

#[test]
fn hitmonlee_kicks_at_its_target_with_a_300_attack() {
    let mut w = World::new(&[(0, -2000.0), (1, 1500.0)]);
    let slot = w.spawn(Kind::Sawamura, 0.0);
    w.until_not(slot, Status::Common, 30);
    w.until_not(slot, Status::SawamuraFall, 100);
    assert_eq!(w.until_not(slot, Status::SawamuraWait, 100), 40);
    let item = *w.item(slot);
    assert_eq!(item.status, ItemStatus::MMonster(Status::SawamuraAttack));
    assert_eq!(item.attack.size, 300.0);
    assert_eq!(item.lr, 1.0);
    assert!(item.vel_air.x > 0.0);
    while w.items.get(slot).is_some() {
        w.step();
    }
}

#[test]
fn snorlax_jumps_to_the_top_and_falls_four_times_its_size() {
    let mut w = World::new(&[(0, -2000.0), (1, 2000.0)]);
    let slot = w.spawn(Kind::Kabigon, 0.0);
    w.until_not(slot, Status::Common, 30);
    let radius = w.item(slot).attack.size;
    w.until_not(slot, Status::KabigonJump, 400);
    let item = *w.item(slot);
    assert_eq!(item.vel_air.y, -220.0);
    assert_eq!((item.scale.x, item.scale.y), (4.0, 4.0));
    assert_eq!(item.attack.size, radius * 4.0);
    assert!(item.pos.x.abs() <= 1000.0);
    while w.items.get(slot).is_some() {
        w.step();
    }
}

#[test]
fn charizard_flames_every_eight_frames_and_turns_every_26() {
    let mut w = World::new(&[(0, -2000.0), (1, 2000.0)]);
    let slot = w.spawn(Kind::Lizardon, 0.0);
    w.until_not(slot, Status::Common, 30);
    w.until_not(slot, Status::LizardonFall, 200);
    assert_eq!(w.status(slot), Status::LizardonAttack);
    assert_eq!(w.item(slot).lr, -1.0);
    let start = w.made.len();
    let mut turns = Vec::new();
    let mut lr = -1.0;
    for n in 0..100 {
        w.step();
        if w.item(slot).lr != lr {
            lr = w.item(slot).lr;
            turns.push(n);
        }
    }
    assert!(turns.windows(2).all(|t| t[1] - t[0] == 26));
    let flames: Vec<_> = w
        .made_of(ShotKind::LizardonFlame)
        .into_iter()
        .filter(|(n, _)| *n >= start)
        .collect();
    assert!(flames.windows(2).all(|f| f[1].0 - f[0].0 == 8));
}

#[test]
fn clefairy_runs_another_status_as_itself() {
    // Index 3: Meowth's attack, from Clefairy's own position, no flip.
    let mut w = World::new(&[(0, -2000.0), (1, 2000.0)]);
    let slot = w.spawn(Kind::Pippi, 0.0);
    for _ in 0..22 {
        w.step();
    }
    crate::rng::set_seed(seed_for(12, 3));
    w.step();
    assert_eq!(w.status(slot), Status::NyarsAttack);
    assert_eq!(w.item(slot).kind, ItemKind::MMonster(Kind::Pippi));
    // Index 11: Mew's flight, with the attack off.
    let slot = w.spawn(Kind::Pippi, 500.0);
    for _ in 0..22 {
        w.step();
    }
    crate::rng::set_seed(seed_for(12, 11));
    w.step();
    assert_eq!(w.status(slot), Status::MewFly);
    assert_eq!(w.item(slot).attack.state, AttackState::Off);
    // Index 1: Snorlax's jump; its fall is 5.2 times Clefairy.
    let slot = w.spawn(Kind::Pippi, -500.0);
    for _ in 0..22 {
        w.step();
    }
    crate::rng::set_seed(seed_for(12, 1));
    w.step();
    assert_eq!(w.status(slot), Status::KabigonJump);
    w.until_not(slot, Status::KabigonJump, 400);
    assert_eq!(w.item(slot).scale.x, 5.2);
}
