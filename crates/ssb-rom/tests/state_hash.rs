//! A per-tick hash of the whole gameplay state over long four-fighter
//! matches: fighter 0 on a scripted pseudo-random pad, the others level-9
//! CPUs, on Dream Land through [`ssb_game::matchtick::step`].
//!
//! The test checks that two runs hash alike tick for tick. With
//! `STATE_HASH_OUT=FILE` it also writes every tick's hashes to `FILE`, so a
//! change meant to keep gameplay bit-identical (a performance change) can
//! be compared against the build before it.

use ssb_game::fighter::FighterKind;

#[path = "common/posed.rs"]
mod posed;

#[path = "common/dream_land.rs"]
mod dream_land;

use dream_land::{dream_land, dream_land_map, stage_bounds, training_fighter};
use posed::{tick_cpus, HostCpu, Pair, Posed};

fn open_pack() -> Option<Vec<u8>> {
    ssb_tablegen::test_rom(env!("CARGO_MANIFEST_DIR"))?;
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak");
    std::fs::read(path).ok()
}

/// FNV-1a over `text`.
fn fnv(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// Fighter 0's pad at `tick`: a new random input every 8 ticks.
fn script_pad(tick: u64) -> (u16, i8, i8) {
    use ssb_capture::script::button;
    let mut x = (tick / 8).wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ 0x5851_f42d;
    x ^= x >> 29;
    x = x.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x ^= x >> 32;
    let buttons = [button::A, button::B, button::Z, button::C_UP]
        .into_iter()
        .enumerate()
        .filter(|&(i, _)| x >> (i * 3) & 7 == 0)
        .fold(0, |b, (_, bit)| b | bit);
    (buttons, (x >> 16) as u8 as i8, (x >> 24) as u8 as i8)
}

/// Every tick's hash of each fighter (its state and skeleton), the item
/// and weapon pools and the RNG.
fn run(pack: &ssb_rom::pack::Pack<'_>, kinds: [FighterKind; 4], ticks: u64) -> Vec<[u64; 6]> {
    // `ftManagerAllocFighter`: the battle's motion and stat counts start
    // again at 1.
    ssb_game::manager_counts::ManagerCounts::new();
    let (stage, segments) = dream_land(pack);
    let map = dream_land_map(pack, &stage);
    let bounds = stage_bounds(pack, &stage);
    let mut fighters: Vec<Posed> = kinds
        .iter()
        .enumerate()
        .map(|(port, &kind)| {
            let mut f = training_fighter(pack, &stage, kind, port as u8, &segments);
            f.dead.bounds = Some(bounds);
            Posed::new(pack, f)
        })
        .collect();
    let mut cpus: Vec<Option<HostCpu>> = fighters
        .iter()
        .enumerate()
        .map(|(port, p)| {
            (port != 0).then(|| {
                HostCpu::one_player(
                    &p.fighter,
                    &stage,
                    bounds,
                    (&map, None),
                    9,
                    ssb_game::computer::attack::Trait::None,
                )
            })
        })
        .collect();
    let mut pair = Pair::default();
    ssb_game::rng::set_seed(1);
    let mut out = Vec::new();
    for tick in 0..ticks {
        tick_cpus(
            pack,
            &map,
            &mut fighters,
            &script_pad,
            &mut cpus,
            tick,
            &mut pair,
        );
        let mut row = [0; 6];
        for (h, p) in row.iter_mut().zip(&fighters) {
            *h = fnv(&format!("{:?}{:?}", p.fighter, p.skeleton));
        }
        row[4] = fnv(&format!("{:?}{:?}", pair.items, pair.weapons));
        row[5] = ssb_game::rng::seed() as u32 as u64;
        out.push(row);
    }
    out
}

/// The matches: four Pikachus (the PSP's `vs4@2170` cast), and a mixed
/// cast whose fighters fit their legs to slopes and lead with `XRotN`.
const CASTS: [[FighterKind; 4]; 2] = [
    [FighterKind::Pikachu; 4],
    [
        FighterKind::Mario,
        FighterKind::Fox,
        FighterKind::Samus,
        FighterKind::Yoshi,
    ],
];

const TICKS: u64 = 3600;

#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
#[test]
fn gameplay_state_hashes_repeat_tick_for_tick() {
    let Some(bytes) = open_pack() else {
        return;
    };
    let pack = ssb_rom::pack::Pack::open(&bytes).unwrap();
    let mut log = String::new();
    for kinds in CASTS {
        let first = run(&pack, kinds, TICKS);
        let second = run(&pack, kinds, TICKS);
        let differ = first.iter().zip(&second).position(|(a, b)| a != b);
        assert_eq!(
            differ, None,
            "{kinds:?}: the second run differs at that tick"
        );
        for (tick, row) in first.iter().enumerate() {
            let row: Vec<String> = row.iter().map(|h| format!("{h:016x}")).collect();
            log += &format!("{kinds:?} {tick} {}\n", row.join(" "));
        }
    }
    if let Some(path) = std::env::var_os("STATE_HASH_OUT") {
        std::fs::write(path, log).unwrap();
    }
}
