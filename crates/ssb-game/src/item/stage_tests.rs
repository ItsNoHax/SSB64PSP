//! The stage items: the Castle Bumper, the POW Block and the Piranha
//! Plants, made through [`StageItems`] and played with a scripted
//! [`ItemAnims`].

use super::*;
use crate::stage::{StageItem, StageItems};

/// One tree's root clock: `len` plays from its start, writing `curve[t]`
/// to the root's Y while it runs.
#[derive(Default, Clone)]
struct Clock {
    t: usize,
    len: usize,
    curve: Vec<f32>,
    live: bool,
}

impl Clock {
    fn play(&mut self) -> RootWrite {
        if !self.live {
            return RootWrite::default();
        }
        let y = self.curve.get(self.t).copied();
        self.t += 1;
        if self.t >= self.len {
            self.live = false;
        }
        RootWrite {
            translate: [None, y, None],
        }
    }
}

/// The POW's 21-play pop-in and 23-frame squash, and a Piranha Plant rise
/// that clears the rim for a while and sinks back over `appear_len` frames.
struct Scripted {
    pow: Clock,
    pakkun: [Clock; 2],
    appear_len: usize,
    made: Vec<ItemAnimTarget>,
}

const POW_APPEAR_LEN: usize = 21;
const POW_DAMAGE_LEN: usize = 23;

fn appear_curve(len: usize) -> Vec<f32> {
    (0..len)
        .map(|t| {
            let rise = (t as f32 * 20.0).min(600.0);
            let fall = len.saturating_sub(t) as f32 * 20.0;
            rise.min(fall)
        })
        .collect()
}

impl Scripted {
    fn new(appear_len: usize) -> Self {
        Scripted {
            pow: Clock::default(),
            pakkun: [Clock::default(), Clock::default()],
            appear_len,
            made: Vec::new(),
        }
    }
    fn clock(&mut self, target: ItemAnimTarget) -> Option<&mut Clock> {
        match target {
            ItemAnimTarget::PowerBlock => Some(&mut self.pow),
            ItemAnimTarget::Pakkun(i) => self.pakkun.get_mut(usize::from(i)),
            ItemAnimTarget::Untracked
            | ItemAnimTarget::Monster(_)
            | ItemAnimTarget::Bonus3Bumper(_) => None,
        }
    }
}

impl ItemAnims for Scripted {
    /// `itManagerMakeItem`'s `gcAddAnimAll` + `gcPlayAnimAll`: the POW's
    /// `anim_joints` pop-in lands on the item's root once the descriptor's
    /// empty root is ejected.
    fn make(&mut self, target: ItemAnimTarget) {
        self.made.push(target);
        if target == ItemAnimTarget::PowerBlock {
            self.pow = Clock {
                t: 0,
                len: POW_APPEAR_LEN,
                curve: Vec::new(),
                live: true,
            };
            self.pow.play();
        }
    }
    fn play(&mut self, target: ItemAnimTarget) -> RootWrite {
        self.clock(target).map_or(RootWrite::default(), Clock::play)
    }
    fn add_play(&mut self, target: ItemAnimTarget, anim: ItemAnim) -> RootWrite {
        let appear_len = self.appear_len;
        let Some(c) = self.clock(target) else {
            return RootWrite::default();
        };
        match anim {
            ItemAnim::PowerBlockDamage => {
                *c = Clock {
                    t: 0,
                    len: POW_DAMAGE_LEN,
                    curve: Vec::new(),
                    live: true,
                }
            }
            ItemAnim::PakkunAppear => {
                *c = Clock {
                    t: 0,
                    len: appear_len,
                    curve: appear_curve(appear_len),
                    live: true,
                }
            }
            // A material script: the root stays as it is.
            ItemAnim::PakkunDamaged => return RootWrite::default(),
        }
        c.play()
    }
    fn root_idle(&self, target: ItemAnimTarget) -> bool {
        match target {
            ItemAnimTarget::PowerBlock => !self.pow.live,
            ItemAnimTarget::Pakkun(i) => !self.pakkun[usize::from(i)].live,
            ItemAnimTarget::Untracked
            | ItemAnimTarget::Monster(_)
            | ItemAnimTarget::Bonus3Bumper(_) => true,
        }
    }
    fn stop_root(&mut self, target: ItemAnimTarget) {
        if let Some(c) = self.clock(target) {
            c.live = false;
        }
    }
}

fn make(pool: &mut ItemPool, kind: StageItem, pos: Vec3) -> u8 {
    let handle = pool.make_item(kind, pos).expect("a free struct");
    assert_eq!(pool.item_pos_width(handle).map(|(p, _)| p), Some(pos));
    (handle & 0xFF) as u8
}

fn tick(pool: &mut ItemPool, anims: &mut Scripted, fighters: &[Vec3]) {
    pool.tick(core::iter::empty, None, fighters, anims);
}

fn hit(pool: &mut ItemPool, slot: u8, by: u8, knock: Knock) {
    queue_damage(
        pool.get_mut(slot).unwrap(),
        5,
        361,
        Element::Normal,
        -1.0,
        Attacker {
            owner: Some(by),
            team: by,
            player: Some(by),
            handicap: 9,
        },
        knock,
    );
}

const SOFT: Knock = Knock {
    weight: 0,
    scale: 0,
    base: 10,
};
const HARD: Knock = Knock {
    weight: 0,
    scale: 0,
    base: 150,
};

#[test]
fn power_block_settles_squashes_and_leaves() {
    let mut pool = ItemPool::default();
    let mut anims = Scripted::new(0);
    let slot = make(&mut pool, StageItem::PowerBlock, Vec3::new(0.0, 500.0, 0.0));
    assert!(anims.made.is_empty());
    let pow = pool.get(slot).unwrap();
    assert_eq!(pow.damage_coll.hitstatus, HitStatus::None);
    assert_eq!(pow.damage_coll.interact_mask, INTERACT_FIGHTER);
    assert_eq!(pow.attack.state, AttackState::Off);

    // The tree starts on the first process; the block settles on the play
    // that ends its pop-in.
    for n in 1..POW_APPEAR_LEN - 1 {
        tick(&mut pool, &mut anims, &[]);
        let pow = pool.get(slot).unwrap();
        assert_eq!(
            pow.status,
            ItemStatus::PowerBlock(power_block::Status::Init),
            "{n}"
        );
        assert_eq!(pow.damage_coll.hitstatus, HitStatus::None);
    }
    assert_eq!(anims.made, [ItemAnimTarget::PowerBlock]);
    tick(&mut pool, &mut anims, &[]);
    let pow = pool.get(slot).unwrap();
    assert_eq!(
        pow.status,
        ItemStatus::PowerBlock(power_block::Status::Wait)
    );
    assert_eq!(pow.damage_coll.hitstatus, HitStatus::Normal);

    hit(&mut pool, slot, 2, SOFT);
    pool.resolve(&[], &mut anims, core::iter::empty);
    let pow = pool.get(slot).unwrap();
    assert_eq!(
        pow.status,
        ItemStatus::PowerBlock(power_block::Status::Damaged)
    );
    assert_eq!(pow.damage_coll.hitstatus, HitStatus::None);
    assert_eq!(
        pool.take_stage_events().collect::<Vec<_>>(),
        [StageItemEvent::PowerBlockDamage {
            handicap: 9,
            hitter: Some(2),
        }]
    );

    // `add_play` ran the squash's first frame. Hitlag holds the clock until
    // its last tick, which plays again; the block goes on the play that
    // ends the squash.
    let mut frames = 0;
    while pool.get(slot).is_some() {
        tick(&mut pool, &mut anims, &[]);
        frames += 1;
        assert!(frames < 100);
    }
    let hitlag = crate::combat::hitlag_frames(5, crate::status::Status::Wait.into(), 1.0);
    assert_eq!(frames, usize::from(hitlag) - 1 + POW_DAMAGE_LEN - 1);
    assert_eq!(
        pool.take_stage_events().collect::<Vec<_>>(),
        [StageItemEvent::PowerBlockGone]
    );
}

#[test]
fn piranha_plant_waits_for_a_clear_pipe_then_bites() {
    let mut pool = ItemPool::default();
    let mut anims = Scripted::new(141);
    let base = Vec3::new(0.0, 100.0, 0.0);
    let slot = make(&mut pool, StageItem::Pakkun(1), base);
    let plant = *pool.get(slot).unwrap();
    assert_eq!(plant.anim_target(), ItemAnimTarget::Pakkun(1));
    assert_eq!(plant.multi, pakkun::APPEAR_WAIT);
    assert!(plant.is_allow_knockback);
    // Inside its pipe its damage box starts out normal, as the attributes say.
    assert_eq!(plant.damage_coll.hitstatus, HitStatus::Normal);

    // A fighter standing over the pipe holds it down for another wait.
    for _ in 0..pakkun::APPEAR_WAIT {
        tick(&mut pool, &mut anims, &[Vec3::new(500.0, 0.0, 0.0)]);
    }
    let plant = pool.get(slot).unwrap();
    assert_eq!(plant.status, ItemStatus::Pakkun(pakkun::Status::Wait));
    assert_eq!(plant.multi, pakkun::APPEAR_WAIT);

    for _ in 0..pakkun::APPEAR_WAIT {
        tick(&mut pool, &mut anims, &[Vec3::new(700.0, 0.0, 0.0)]);
    }
    let plant = pool.get(slot).unwrap();
    assert_eq!(plant.status, ItemStatus::Pakkun(pakkun::Status::Appear));
    assert_eq!(plant.pos.y, base.y);

    // The box and the bite turn on once the plant clears the rim.
    let mut rose = false;
    let mut frames = 0;
    while pool.get(slot).unwrap().status == ItemStatus::Pakkun(pakkun::Status::Appear) {
        tick(&mut pool, &mut anims, &[]);
        frames += 1;
        let plant = pool.get(slot).unwrap();
        if plant.status != ItemStatus::Pakkun(pakkun::Status::Appear) {
            break;
        }
        let off_y = plant.pos.y - base.y + pakkun::APPEAR_OFF_Y;
        if off_y > pakkun::CLAMP_OFF_Y {
            rose = true;
            assert_eq!(plant.damage_coll.hitstatus, HitStatus::Normal);
            assert_ne!(plant.attack.state, AttackState::Off);
            let size = (off_y - pakkun::CLAMP_OFF_Y) * pakkun::HURT_SIZE_MUL_Y;
            assert_eq!(plant.damage_coll.size.y, size);
        } else {
            assert_eq!(plant.damage_coll.hitstatus, HitStatus::None);
            assert_eq!(plant.attack.state, AttackState::Off);
        }
    }
    assert!(rose);
    assert_eq!(frames, 140);
    let plant = pool.get(slot).unwrap();
    assert_eq!(plant.pos.y, base.y);
    assert_eq!(plant.multi, pakkun::APPEAR_WAIT);
    assert_eq!(plant.damage_coll.hitstatus, HitStatus::None);
}

#[test]
fn piranha_plant_knocked_out_regrows_after_the_rebirth_wait() {
    let mut pool = ItemPool::default();
    let mut anims = Scripted::new(141);
    let base = Vec3::new(0.0, 100.0, 0.0);
    let slot = make(&mut pool, StageItem::Pakkun(0), base);
    for _ in 0..pakkun::APPEAR_WAIT + 40 {
        tick(&mut pool, &mut anims, &[]);
    }
    assert_eq!(
        pool.get(slot).unwrap().damage_coll.hitstatus,
        HitStatus::Normal
    );

    // A light hit only hurts.
    hit(&mut pool, slot, 0, SOFT);
    pool.resolve(&[], &mut anims, core::iter::empty);
    let plant = *pool.get(slot).unwrap();
    assert_eq!(plant.status, ItemStatus::Pakkun(pakkun::Status::Appear));
    assert!(plant.damage_knockback == 0.0);

    for _ in 0..30 {
        tick(&mut pool, &mut anims, &[]);
    }
    hit(&mut pool, slot, 0, HARD);
    let kb = pool.get(slot).unwrap().damage_knockback;
    assert!(kb >= pakkun::NDAMAGE_KNOCKBACK_MIN, "{kb}");
    pool.resolve(&[], &mut anims, core::iter::empty);
    let plant = *pool.get(slot).unwrap();
    assert_eq!(plant.status, ItemStatus::Pakkun(pakkun::Status::Damaged));
    assert_eq!(plant.rotate_z, core::f32::consts::PI);
    assert_eq!(plant.damage_coll.hitstatus, HitStatus::None);
    assert_eq!(plant.attack.state, AttackState::Off);
    // The airborne Sakurai angle, away from the hitter's side.
    let (sin, cos) = ssb_engine::math::sin_cos(43.0_f32.to_radians());
    assert!((plant.vel_air.x - cos * kb).abs() < 1e-3);
    assert!((plant.vel_air.y - sin * kb).abs() < 1e-3);
    assert!(anims.root_idle(ItemAnimTarget::Pakkun(0)));

    // Leaving the map sends it home instead of destroying it.
    let bounds = BlastZone {
        left: -50.0,
        right: 50.0,
        bottom: -1000.0,
        top: 1000.0,
    };
    let mut frames = 0;
    while pool.get(slot).unwrap().status == ItemStatus::Pakkun(pakkun::Status::Damaged) {
        pool.tick(core::iter::empty, Some(bounds), &[], &mut anims);
        frames += 1;
        assert!(frames < 100);
    }
    let plant = *pool.get(slot).unwrap();
    assert_eq!(plant.pos, base);
    assert_eq!(plant.vel_air, Vec3::ZERO);
    assert_eq!(plant.rotate_z, 0.0);
    assert_eq!(plant.multi, pakkun::REBIRTH_WAIT);
    assert_eq!(plant.status, ItemStatus::Pakkun(pakkun::Status::Wait));
}

#[test]
fn pipe_entry_sends_a_rising_plant_back_down() {
    let mut pool = ItemPool::default();
    let mut anims = Scripted::new(141);
    let base = Vec3::new(0.0, 100.0, 0.0);
    let slot = make(&mut pool, StageItem::Pakkun(0), base);
    for _ in 0..pakkun::APPEAR_WAIT + 10 {
        tick(&mut pool, &mut anims, &[]);
    }
    let handle = pool.handle_of(slot);
    pool.pakkun_set_wait_fighter(handle);
    tick(&mut pool, &mut anims, &[]);
    let plant = pool.get(slot).unwrap();
    assert_eq!(plant.status, ItemStatus::Pakkun(pakkun::Status::Wait));
    assert_eq!(plant.pos.y, base.y);
    assert_eq!(plant.multi, pakkun::APPEAR_WAIT);
    assert!(!plant.vars.pakkun_is_wait_fighter);
}

#[test]
fn castle_bumper_follows_the_ground_and_swells_on_a_hit() {
    let mut pool = ItemPool::default();
    let mut anims = Scripted::new(0);
    let slot = make(
        &mut pool,
        StageItem::Bumper {
            castle: true,
            joint: None,
        },
        Vec3::new(100.0, 200.0, 0.0),
    );
    let bumper = *pool.get(slot).unwrap();
    assert_eq!(bumper.attack.kb_weight, gbumper::CASTLE_KNOCKBACK);
    assert_eq!(bumper.attack.angle, gbumper::CASTLE_ANGLE);
    assert_eq!(bumper.attack.interact_mask, INTERACT_FIGHTER);
    assert!(bumper.attack.can_rehit_shield);
    assert!(bumper.is_damage_all);
    assert_eq!(bumper.owner, None);
    assert_eq!(bumper.damage_coll.hitstatus, HitStatus::None);

    let handle = pool.handle_of(slot);
    pool.set_item_x(handle, 340.0);
    assert_eq!(pool.get(slot).unwrap().pos.x, 340.0);

    pool.get_mut(slot).unwrap().hit_normal_damage = 1;
    pool.resolve(&[], &mut anims, core::iter::empty);
    let b = pool.get(slot).unwrap();
    assert_eq!(
        (b.scale.x, b.palette, b.multi),
        (2.0, 1, gbumper::HIT_SCALE)
    );
    let mut scales = Vec::new();
    let mut palettes = Vec::new();
    for _ in 0..12 {
        tick(&mut pool, &mut anims, &[]);
        let b = pool.get(slot).unwrap();
        scales.push((b.scale.x * 10.0).round() as i32);
        palettes.push(b.palette);
    }
    assert_eq!(scales, [20, 19, 18, 17, 16, 15, 14, 13, 12, 11, 10, 10]);
    assert_eq!(palettes, [1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
}

#[test]
fn a_stale_handle_names_nothing() {
    let mut pool = ItemPool::default();
    let slot = make(
        &mut pool,
        StageItem::Bumper {
            castle: true,
            joint: None,
        },
        Vec3::ZERO,
    );
    let handle = pool.handle_of(slot);
    pool.destroy(slot);
    let again = make(&mut pool, StageItem::PowerBlock, Vec3::ZERO);
    assert_eq!(again, slot);
    assert_eq!(pool.item_pos_width(handle), None);
    assert!(ItemPool::default()
        .make_item(StageItem::Monster(5), Vec3::ZERO)
        .is_none());
}
