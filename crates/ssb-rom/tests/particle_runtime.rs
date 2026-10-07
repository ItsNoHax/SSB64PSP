//! The match particle runtime (`ssb_game::particle`, `ssb_game::effect`,
//! RE-413) run on the packed `efcommon` bank: what the hit sparks and the
//! KO particles do, and how much of the source's pools they use.

use ssb_engine::math::Vec3;
use ssb_game::effect::{self, EffectRuntime, Effects};
use ssb_game::particle::{self as lb, flag, Banks, Particles, Script};
use ssb_game::wpeffect::{self, WeaponEffect};
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

impl PackBanks<'_, '_> {
    fn bank(&self, id: u8) -> Option<ParticleBankDesc> {
        match id {
            0 => Some(self.common),
            effect::YOSHI_PARTICLE_BANK => self.pack.particle_bank(3),
            _ => None,
        }
    }
}

impl Banks for PackBanks<'_, '_> {
    fn script_count(&self, bank: u8) -> u16 {
        self.bank(bank).map_or(0, |b| b.script_count as u16)
    }

    fn script(&self, bank: u8, id: u16) -> Option<Script<'_>> {
        let bank = self.bank(bank)?;
        if u32::from(id) >= bank.script_count {
            return None;
        }
        let s = self
            .pack
            .particle_script(bank.first_script + u32::from(id))?;
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
        let Some(bank) = self.bank(bank) else {
            return 0;
        };
        self.pack
            .particle_texture(bank.first_texture + u32::from(texture))
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
        .frame();
    }
    panic!("the effect never ended");
}

fn open(bytes: &[u8]) -> Pack<'_> {
    Pack::open(bytes).unwrap()
}

/// `dFTYoshiData` names `particles_unk2`. An explosion must use that
/// bank's script and textures, not common-bank script 3.
#[test]
fn yoshi_explosion_runs_the_packed_fighter_bank_and_ends() {
    let Some(bytes) = pack_bytes() else { return };
    let pack = open(&bytes);
    let banks = PackBanks {
        pack: &pack,
        common: pack.particle_bank(0).unwrap(),
    };
    let rom = std::fs::read(std::env::var_os("SSB64_ROM").unwrap()).unwrap();
    let (scripts, textures) =
        ssb_rom::particle::decode_bank(&rom, ssb_rom::particle::BANKS[3]).unwrap();
    let expected = &scripts[3];
    let actual = banks.script(effect::YOSHI_PARTICLE_BANK, 3).unwrap();
    assert_eq!(actual.bytecode, expected.bytecode);
    assert_eq!(actual.flags, expected.flags);
    assert_eq!(actual.texture_id, expected.texture_id);
    let packed_bank = pack.particle_bank(3).unwrap();
    assert_eq!(packed_bank.texture_count as usize, textures.len());
    let mut p = Box::new(Particles::new());
    let mut e = Effects::new(0);
    ssb_game::rng::set_seed(1);
    let pos = Vec3::new(800.0, 200.0, 30.0);
    wpeffect::make(&WeaponEffect::YoshiEggExplode(pos), &mut e, &mut p, &banks);
    assert!(p.used_num > 0);
    let (_, pc) = (0..lb::LINKS_NUM).flat_map(|l| p.list(l)).next().unwrap();
    assert_eq!(pc.bank_id & 7, effect::YOSHI_PARTICLE_BANK);
    assert_eq!(p.transform(pc.xf).translate, pos);
    assert_eq!(
        e.free_num as usize,
        effect::EFFECT_ALLOC_NUM,
        "no effect struct is held"
    );
    let mut census = Census::default();
    run_until_empty(&mut p, &mut e, &banks, &mut census);
    assert_eq!(
        census.flags
            & (flag::VORTEX | flag::ATTACH | flag::NOISE | flag::ALPHABLEND | flag::DITHER),
        0
    );
    assert_eq!(census.frames, 18);
    assert_eq!(census.structs_max, 3);
    assert_eq!(census.transforms_max, 1);
    eprintln!("Yoshi explosion: {census:?}");
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
        // RE-415: `ftParamMakeEffect`'s particle makers.
        ("dust light", |e, p, b| {
            e.dust_light(p, b, Vec3::ZERO, 1, 1.0)
        }),
        ("dust light rapid", |e, p, b| {
            e.dust_light(p, b, Vec3::ZERO, -1, 2.0)
        }),
        ("dust heavy", |e, p, b| e.dust_heavy(p, b, Vec3::ZERO, -1)),
        ("dust heavy double", |e, p, b| {
            e.dust_heavy_double(p, b, Vec3::ZERO, 1, 1.0)
        }),
        ("dust heavy double rapid", |e, p, b| {
            e.dust_heavy_double(p, b, Vec3::ZERO, -1, 1.7)
        }),
        ("dust expand large", |e, p, b| {
            e.dust_expand_large(p, b, Vec3::ZERO)
        }),
        ("dust dash", |e, p, b| {
            e.dust_dash(p, b, Vec3::ZERO, -1, 1.5)
        }),
        ("sparkle white", |e, p, b| {
            e.ready_at(p, b, true, effect::script::SPARKLE_WHITE, Vec3::ZERO, 1.0)
        }),
        ("sparkle white multi", |e, p, b| {
            e.ready_at(
                p,
                b,
                true,
                effect::script::SPARKLE_WHITE_MULTI,
                Vec3::ZERO,
                1.0,
            )
        }),
        ("sparkle white multi explode", |e, p, b| {
            e.ready_at(
                p,
                b,
                true,
                effect::script::SPARKLE_WHITE_MULTI_EXPLODE,
                Vec3::ZERO,
                1.0,
            )
        }),
        ("sparkle white scale", |e, p, b| {
            e.ready_at(
                p,
                b,
                false,
                effect::script::SPARKLE_WHITE_SCALE,
                Vec3::ZERO,
                0.7,
            )
        }),
        ("thunder amp", |e, p, b| {
            e.ready_at(p, b, false, effect::script::THUNDER_AMP, Vec3::ZERO, 1.0)
        }),
        ("heal sparkles", |e, p, b| {
            e.ready_at(p, b, true, effect::script::HEAL_SPARKLES, Vec3::ZERO, 1.0)
        }),
        ("egg break", |e, p, b| {
            e.ready_at(p, b, false, effect::script::EGG_BREAK, Vec3::ZERO, 1.0)
        }),
        ("music note", |e, p, b| e.music_note(p, b, Vec3::ZERO)),
        ("flame lr", |e, p, b| e.flame_lr(p, b, Vec3::ZERO, 1)),
        ("flame random", |e, p, b| e.flame(p, b, Vec3::ZERO, true)),
        ("flame static", |e, p, b| e.flame(p, b, Vec3::ZERO, false)),
        ("fura sparkle", |e, p, b| {
            e.common_at(p, b, true, effect::script::FURA_SPARKLE, Vec3::ZERO)
        }),
        ("psionic", |e, p, b| {
            e.common_at(p, b, false, effect::script::PSIONIC, Vec3::ZERO)
        }),
        ("flash small", |e, p, b| {
            e.common_at(p, b, false, effect::script::FLASH_SMALL, Vec3::ZERO)
        }),
        ("flash middle", |e, p, b| {
            e.common_at(p, b, false, effect::script::FLASH_MIDDLE, Vec3::ZERO)
        }),
        ("flash large", |e, p, b| {
            e.common_at(p, b, false, effect::script::FLASH_LARGE, Vec3::ZERO)
        }),
        ("ripple", |e, p, b| {
            e.generator_at(p, b, effect::script::RIPPLE_GEN, Vec3::ZERO)
        }),
        ("kirby star", |e, p, b| {
            e.generator_at(p, b, effect::script::KIRBY_STAR_GEN, Vec3::ZERO)
        }),
        // RE-416: the weapons' own makers.
        ("impact shock", |e, p, b| {
            e.impact_shock(p, b, Vec3::ZERO, 12)
        }),
        ("dust collide", |e, p, b| e.dust_collide(p, b, Vec3::ZERO)),
        ("fire grind", |e, p, b| {
            e.ready_at(p, b, true, wpeffect::FIRE_GRIND_ID, Vec3::ZERO, 1.0)
        }),
        ("fox blaster glow", |e, p, b| {
            e.common_at(p, b, false, wpeffect::FOX_BLASTER_GLOW_ID, Vec3::ZERO)
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
        .frame();
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

/// RE-415: four fighters dashing, landing and trailing dust (a light
/// cloud and a double heavy one every four frames each), each struck every
/// eight frames with a heavy spark, a set-off, a slash and the orbs and
/// sparks, while two are shocked (`ShockSmall` every other frame) and one
/// quake plays. The particle pools never run out: only the source's own
/// five-free rule refuses, and the display pool never does.
#[test]
fn a_dusty_melee_stays_inside_the_source_pools() {
    let Some(bytes) = pack_bytes() else { return };
    let pack = open(&bytes);
    let banks = PackBanks {
        pack: &pack,
        common: pack.particle_bank(0).unwrap(),
    };
    ssb_game::rng::set_seed(1);
    let mut p = Box::new(Particles::new());
    let mut e = Effects::new(0);
    let mut refused_particles = 0;
    let mut structs_max = 0;
    let mut displays_max = 0;
    for frame in 0..240 {
        for player in 0..4u8 {
            let at = Vec3::new(f32::from(player) * 300.0, 0.0, 0.0);
            if frame % 4 == 0 {
                e.dust_light(&mut p, &banks, at, 1, 1.0);
                e.dust_heavy_double(&mut p, &banks, at, -1, 1.0);
                refused_particles +=
                    usize::from(e.dust_dash(&mut p, &banks, at, 1, 1.0) == lb::NIL);
            }
            if frame % 8 == 0 {
                e.damage_normal_heavy(&mut p, &banks, at, player, 18);
                e.set_off(&mut p, &banks, at, 18);
                e.damage_slash(at, 18, 0.0);
                e.damage_spawn_orbs(at);
                e.damage_spawn_sparks(at, 1, false);
            }
            if player < 2 && frame % 2 == 0 {
                e.shock_small(at);
            }
        }
        if frame == 0 {
            e.quake(1);
        }
        structs_max = structs_max.max(e.used());
        displays_max = displays_max.max(e.displays().count());
        let mut rt = EffectRuntime {
            particles: &mut p,
            effects: &mut e,
            banks: &banks,
        };
        rt.frame();
    }
    println!(
        "dusty melee: particles max {}, transforms max {}, structs max {structs_max}, displays max {displays_max}, refused {refused_particles}",
        p.used_max, p.xf_used_num
    );
    assert_eq!(refused_particles, 0);
    assert!(usize::from(p.used_max) < lb::STRUCTS_NUM);
    assert_eq!(e.displays_refused, 0);
    assert!(displays_max < effect::DISPLAY_MAX);
}

/// RE-416: four fighters' weapons at once. For four seconds each fighter's
/// weapons end every eight frames (a dust cloud, a glow, a shock and a
/// set-off), two weapons rebound every sixteen (a Fireball's grind and a
/// Boomerang's dust), an Egg lands and a Bomb explodes every 32 (the shell,
/// the sparkles and a quake) and two Thunder segments fade out every frame,
/// beside the fighters' dust of RE-415. Only the
/// source's five-free rule refuses; the particle and display pools never
/// run out.
#[test]
fn a_melee_of_weapons_stays_inside_the_source_pools() {
    let Some(bytes) = pack_bytes() else { return };
    let pack = open(&bytes);
    let banks = PackBanks {
        pack: &pack,
        common: pack.particle_bank(0).unwrap(),
    };
    ssb_game::rng::set_seed(1);
    let mut p = Box::new(Particles::new());
    let mut e = Effects::new(0);
    let mut structs_max = 0;
    let mut displays_max = 0;
    for frame in 0..240 {
        let mut fx = std::vec::Vec::new();
        for player in 0..4u8 {
            let at = Vec3::new(f32::from(player) * 300.0, 0.0, 0.0);
            if frame % 8 == u32::from(player) * 2 {
                fx.push(WeaponEffect::DustExpandSmall(at));
                fx.push(WeaponEffect::FoxBlasterGlow(at));
                fx.push(WeaponEffect::ImpactShock { pos: at, size: 10 });
                fx.push(WeaponEffect::SetOff { pos: at, size: 7 });
            }
            if player < 2 {
                fx.push(WeaponEffect::ThunderTrail {
                    pos: at,
                    lifetime: 6,
                    texture: 0,
                });
                fx.push(WeaponEffect::TextureRand(3));
            }
            if frame % 4 == 0 {
                e.dust_light(&mut p, &banks, at, 1, 1.0);
            }
            if player < 2 && frame % 16 == u32::from(player) * 8 {
                fx.push(WeaponEffect::DustCollide(at));
                fx.push(WeaponEffect::FireGrind(at));
            }
        }
        if frame % 32 == 0 {
            fx.push(WeaponEffect::Quake(2));
            fx.push(WeaponEffect::EggBreak(Vec3::ZERO));
            fx.push(WeaponEffect::SparkleWhiteMultiExplode(Vec3::ZERO));
            fx.push(WeaponEffect::SparkleWhite(Vec3::ZERO));
        }
        for f in &fx {
            wpeffect::make(f, &mut e, &mut p, &banks);
        }
        structs_max = structs_max.max(e.used());
        displays_max = displays_max.max(e.displays().count());
        EffectRuntime {
            particles: &mut p,
            effects: &mut e,
            banks: &banks,
        }
        .frame();
    }
    println!(
        "weapon melee: particles max {}, structs max {structs_max}, displays max {displays_max}",
        p.used_max
    );
    assert!(usize::from(p.used_max) < lb::STRUCTS_NUM);
    assert_eq!(e.displays_refused, 0);
    assert!(displays_max < effect::DISPLAY_MAX);
}

/// RE-420: `mnVSResultsMakeConfetti`'s two scripts: the one behind the
/// fighters on list 4 (`LBPARTICLE_MASK_GENLINK(3)`), the one in front on
/// list 0. Each uses only what the runtime models and ends inside the
/// pools.
#[test]
fn the_results_confetti_falls_on_lists_four_and_zero_and_ends() {
    let Some(bytes) = pack_bytes() else { return };
    let pack = open(&bytes);
    let banks = PackBanks {
        pack: &pack,
        common: pack.particle_bank(0).unwrap(),
    };
    let mut p = Box::new(Particles::new());
    let mut e = Effects::new(0);
    for (pos, is_genlink_mask) in ssb_game::results_emblem::CONFETTI {
        assert_ne!(e.confetti(&mut p, &banks, pos, is_genlink_mask), lb::NIL);
    }
    // Each script makes four generators; their particles take their lists.
    assert_eq!(p.generators().count(), 8);
    let mut lists = 0u16;
    for _ in 0..30 {
        EffectRuntime {
            particles: &mut p,
            effects: &mut e,
            banks: &banks,
        }
        .frame();
        for l in 0..lb::LINKS_NUM {
            if p.list(l).next().is_some() {
                lists |= 1 << l;
            }
        }
    }
    assert_eq!(lists, (1 << 0) | (1 << 4));
    // It falls for as long as the screen stays up.
    let (mut flags, mut structs_max, mut gens, mut env_alpha) = (0u16, 0u16, 0usize, 0u8);
    for _ in 0..1200 {
        EffectRuntime {
            particles: &mut p,
            effects: &mut e,
            banks: &banks,
        }
        .frame();
        for l in 0..lb::LINKS_NUM {
            for (_, pc) in p.list(l) {
                flags |= pc.flags;
                if pc.flags & flag::ENVCOLOR != 0 {
                    env_alpha = env_alpha.max(pc.envcolor[3]);
                }
            }
        }
        structs_max = structs_max.max(p.used_num);
        gens = p.generators().count();
    }
    assert_eq!(flags & (flag::VORTEX | flag::ATTACH), 0);
    assert_eq!(gens, 8, "the generators never end");
    assert!(structs_max <= lb::STRUCTS_NUM as u16);
    // The draw's `PRIM.a * TEXEL0.a` is exact at ENV alpha 0 (RE-413).
    assert_eq!(env_alpha, 0);
    println!("confetti: flags {flags:#x}, peak {structs_max} particles");
}

/// RE-464: the interface's effects (`efManagerStockSnapMakeEffect`,
/// `...StockSteal{Start,End}...`, `...BattleScoreMakeEffect`) are packed
/// common-bank scripts on list 3, the effect camera's, and each ends.
#[test]
fn the_interface_effects_run_on_list_three_and_end() {
    let Some(bytes) = pack_bytes() else { return };
    let pack = open(&bytes);
    let banks = PackBanks {
        pack: &pack,
        common: pack.particle_bank(0).unwrap(),
    };
    type Make = fn(&mut Effects, &mut Particles, &dyn Banks) -> u8;
    let makers: [(&str, Make); 5] = [
        ("snap", |e, p, b| e.stock_snap(p, b, 55.0, 210.0)),
        ("steal start", |e, p, b| {
            e.stock_steal_start(p, b, 171.0, 190.0)
        }),
        ("steal end", |e, p, b| e.stock_steal_end(p, b, 31.0, 190.0)),
        ("+1", |e, p, b| {
            e.battle_score(p, b, Vec3::new(220.0, 892.0, 0.0), 1)
        }),
        ("-1", |e, p, b| {
            e.battle_score(p, b, Vec3::new(220.0, 892.0, 0.0), -1)
        }),
    ];
    for (name, make) in makers {
        let mut p = Box::new(Particles::new());
        let mut e = Effects::new(0);
        ssb_game::rng::set_seed(1);
        let pc = make(&mut e, &mut p, &banks);
        assert_ne!(pc, lb::NIL, "{name}");
        assert!(p.list(3).count() > 0, "{name} on list 3");
        assert_eq!(
            (0..3).map(|l| p.list(l).count()).sum::<usize>(),
            0,
            "{name}"
        );
        if name.ends_with('1') {
            let xf = p.particle(pc).xf;
            assert_eq!(p.transform(xf).scale.y, 0.25, "{name}");
        }
        let mut census = Census::default();
        run_until_empty(&mut p, &mut e, &banks, &mut census);
        eprintln!("{name}: {census:?}");
    }
}

/// How to Play's Fire Flower (RE-473): `itManagerMakeItemSetupCommon`
/// makes the item's spawn swirl at once, and the swirl's first update
/// makes generators 0x66 to 0x68. The next frame's generator run draws
/// from them nine times before anything else: the N64's generator goes
/// from 436000293 (frame 3249) to -1873138207 at frame 3250, those nine
/// draws and the three of Mario's dust after them.
#[test]
fn the_item_spawn_swirl_draws_nine_times_at_the_next_frame_start() {
    let Some(bytes) = pack_bytes() else { return };
    let pack = open(&bytes);
    let banks = PackBanks {
        pack: &pack,
        common: pack.particle_bank(0).unwrap(),
    };
    let step = |s: i32| s.wrapping_mul(214013).wrapping_add(2531011);
    let n64_3249 = 436_000_293;
    let after_generators = (0..9).fold(n64_3249, |s, _| step(s));
    assert_eq!(
        (0..3).fold(after_generators, |s, _| step(s)),
        -1_873_138_207
    );

    let mut p = Box::new(Particles::new());
    let mut e = Effects::new(0);
    ssb_game::rng::set_seed(n64_3249);
    let pos = ssb_game::explain::FIRE_FLOWER_POS;
    wpeffect::make(&WeaponEffect::ItemSpawnSwirl(pos), &mut e, &mut p, &banks);
    assert_eq!(
        ssb_game::rng::seed(),
        n64_3249,
        "the swirl's making draws nothing"
    );
    assert_eq!(p.generators().count(), 3);
    EffectRuntime {
        particles: &mut p,
        effects: &mut e,
        banks: &banks,
    }
    .run();
    assert_eq!(ssb_game::rng::seed(), after_generators);
}
