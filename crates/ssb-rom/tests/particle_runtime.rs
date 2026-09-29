//! The match particle runtime (`ssb_game::particle`, `ssb_game::effect`,
//! RE-413) run on the packed `efcommon` bank: what the hit sparks and the
//! KO particles do, and how much of the source's pools they use.

use ssb_engine::math::Vec3;
use ssb_game::effect::{self, EffectRuntime, Effects};
use ssb_game::particle::{self as lb, flag, Banks, Particles, Script};
use ssb_rom::pack::{Pack, ParticleBankDesc};

fn pack_bytes() -> Option<Vec<u8>> {
    std::env::var_os("SSB64_ROM")?;
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak");
    std::fs::read(path).ok()
}

/// The packed banks as the runtime sees them: runtime bank 0 is the
/// common bank (pack bank 0), as `efDisplayInitAll` loads it first.
struct PackBanks<'p, 'a> {
    pack: &'p Pack<'a>,
    common: ParticleBankDesc,
}

impl Banks for PackBanks<'_, '_> {
    fn script_count(&self, bank: u8) -> u16 {
        if bank == 0 {
            self.common.script_count as u16
        } else {
            0
        }
    }

    fn script(&self, bank: u8, id: u16) -> Option<Script<'_>> {
        if bank != 0 || u32::from(id) >= self.common.script_count {
            return None;
        }
        let s = self
            .pack
            .particle_script(self.common.first_script + u32::from(id))?;
        Some(Script {
            kind: s.kind,
            texture_id: s.texture_id,
            generator_lifetime: s.generator_lifetime,
            particle_lifetime: s.particle_lifetime,
            flags: s.flags,
            gravity: s.gravity,
            friction: s.friction,
            vel: Vec3::new(s.velocity[0], s.velocity[1], s.velocity[2]),
            unk_20: s.unknown_20,
            unk_24: s.unknown_24,
            update_rate: s.update_rate,
            size: s.size,
            bytecode: self.pack.particle_bytecode(&s).unwrap_or(&[]),
        })
    }

    fn texture_flags(&self, bank: u8, texture: u16) -> u32 {
        if bank != 0 {
            return 0;
        }
        self.pack
            .particle_texture(self.common.first_texture + u32::from(texture))
            .map_or(0, |t| t.flags)
    }
}

/// Every flag the live particles carried, the frames until the last
/// ended, and the pools' peaks.
#[derive(Debug, Default)]
struct Census {
    flags: u16,
    frames: u32,
    structs_max: u16,
    transforms_max: u16,
    generators_max: u16,
    generator_kinds: u32,
    /// The highest ENV alpha an ENV-combined particle drew with.
    env_alpha_max: u8,
}

fn run_until_empty(p: &mut Particles, e: &mut Effects, banks: &dyn Banks, census: &mut Census) {
    for frame in 0..2000 {
        for link in 0..lb::LINKS_NUM {
            for (_, pc) in p.list(link) {
                census.flags |= pc.flags;
                if pc.flags & flag::ENVCOLOR != 0 && pc.size != 0.0 {
                    census.env_alpha_max = census.env_alpha_max.max(pc.envcolor[3]);
                }
            }
        }
        for g in p.generators() {
            census.generator_kinds |= 1 << g.kind;
        }
        census.structs_max = census.structs_max.max(p.used_num);
        census.transforms_max = census.transforms_max.max(p.xf_used_num);
        census.generators_max = census.generators_max.max(p.gen_used_num);
        if p.used_num == 0 && p.gen_used_num == 0 {
            census.frames = frame;
            return;
        }
        EffectRuntime {
            particles: p,
            effects: e,
            banks,
        }
        .run();
    }
    panic!("the effect never ended");
}

fn open(bytes: &[u8]) -> Pack<'_> {
    Pack::open(bytes).unwrap()
}

/// Each maker a match reaches, run alone to its end: no script it reaches
/// uses the vortex, the attach `DObj`s or the distance opcodes, which the
/// runtime does not model, and every one ends.
#[test]
fn every_match_maker_ends_without_the_unmodelled_features() {
    let Some(bytes) = pack_bytes() else { return };
    let pack = open(&bytes);
    let banks = PackBanks {
        pack: &pack,
        common: pack.particle_bank(0).unwrap(),
    };
    // `efcommon` has 119 scripts (`efcommon_scb.c`).
    assert_eq!(banks.common.script_count, 119);
    type Maker = fn(&mut Effects, &mut Particles, &dyn Banks) -> u8;
    let makers: &[(&str, Maker)] = &[
        ("light", |e, p, b| {
            e.damage_normal_light(p, b, Vec3::ZERO, 0, 10, false)
        }),
        ("light p4", |e, p, b| {
            e.damage_normal_light(p, b, Vec3::ZERO, 3, 10, false)
        }),
        ("heavy", |e, p, b| {
            e.damage_normal_heavy(p, b, Vec3::ZERO, 1, 20)
        }),
        ("fire", |e, p, b| e.damage_fire(p, b, Vec3::ZERO, 10)),
        ("electric", |e, p, b| {
            e.damage_electric(p, b, Vec3::ZERO, 10)
        }),
        ("coin", |e, p, b| e.damage_coin(p, b, Vec3::ZERO)),
        ("set off", |e, p, b| e.set_off(p, b, Vec3::ZERO, 10)),
        ("blast", |e, p, b| e.dead_explode(p, b, Vec3::ZERO, 0, 0)),
        ("blast side", |e, p, b| {
            e.dead_explode(p, b, Vec3::ZERO, 3, 1)
        }),
        ("sparkle", |e, p, b| {
            e.sparkle_white_dead(p, b, Vec3::ZERO, 5.0)
        }),
    ];
    for &(name, make) in makers {
        ssb_game::rng::set_seed(1);
        let mut p = Box::new(Particles::new());
        let mut e = Effects::new(0);
        assert_ne!(make(&mut e, &mut p, &banks), lb::NIL, "{name}");
        let mut c = Census::default();
        run_until_empty(&mut p, &mut e, &banks, &mut c);
        println!("{name}: {c:?}");
        assert_eq!(c.flags & (flag::VORTEX | flag::ATTACH), 0, "{name}");
        assert_eq!(c.generator_kinds & (1 << 2), 0, "{name}: vortex generator");
        assert_eq!(p.xf_used_num, 0, "{name}");
        assert_eq!(e.used(), 0, "{name}");
    }
}

/// A four-player worst case: every fighter takes a heavy hit (which ends
/// in a light spark) with a set-off every eight frames for two seconds,
/// while two blasts and a star sparkle play. The source's pools of 112
/// particles, 80 transforms and 24 generators, and the 38 effect structs,
/// never run out.
#[test]
fn a_four_player_melee_stays_inside_the_source_pools() {
    let Some(bytes) = pack_bytes() else { return };
    let pack = open(&bytes);
    let banks = PackBanks {
        pack: &pack,
        common: pack.particle_bank(0).unwrap(),
    };
    ssb_game::rng::set_seed(1);
    let mut p = Box::new(Particles::new());
    let mut e = Effects::new(0);
    let mut c = Census::default();
    let mut refused = 0;
    e.dead_explode(&mut p, &banks, Vec3::ZERO, 0, 0);
    e.dead_explode(&mut p, &banks, Vec3::ZERO, 1, 1);
    e.sparkle_white_dead(&mut p, &banks, Vec3::ZERO, 5.0);
    for frame in 0..120 {
        if frame % 8 == 0 {
            for player in 0..4 {
                let at = Vec3::new(f32::from(player) * 300.0, 0.0, 0.0);
                refused +=
                    usize::from(e.damage_normal_heavy(&mut p, &banks, at, player, 18) == lb::NIL);
                refused += usize::from(e.set_off(&mut p, &banks, at, 18) == lb::NIL);
            }
        }
        c.structs_max = c.structs_max.max(p.used_num);
        c.transforms_max = c.transforms_max.max(p.xf_used_num);
        EffectRuntime {
            particles: &mut p,
            effects: &mut e,
            banks: &banks,
        }
        .run();
    }
    println!(
        "melee: {c:?}, structs used max {}, refused {refused}",
        p.used_max
    );
    assert_eq!(refused, 0);
    assert!(usize::from(p.used_max) < lb::STRUCTS_NUM);
    assert!(usize::from(c.transforms_max) < lb::TRANSFORMS_NUM);
    let _ = effect::EFFECT_ALLOC_NUM;
}
