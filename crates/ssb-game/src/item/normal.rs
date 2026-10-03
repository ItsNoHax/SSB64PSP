//! Normal-item switches, `itManagerSetupContainerDrops` and the
//! appearance actor (`itManagerMakeAppearActor`). Kinds without a ported
//! maker are still drawn from the tables, so the RNG stream matches the
//! source; their maker makes nothing (RE-433).

use ssb_engine::math::Vec3;

/// `SCBattleItemSwitch` order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Appearance {
    None,
    VeryLow,
    Low,
    Middle,
    High,
    VeryHigh,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Switches {
    pub appearance: Appearance,
    /// Original `ITKind` bits: Capsule = 2, Egg = 3, utilities = 4..19.
    pub toggles: u32,
}

impl Default for Switches {
    /// `dSCManagerDefaultBattleState`: every item, middle appearance.
    fn default() -> Self {
        Self {
            appearance: Appearance::Middle,
            toggles: !0,
        }
    }
}
impl Switches {
    pub fn enabled(self, kind: u8) -> bool {
        self.appearance != Appearance::None && kind < 32 && self.toggles & (1 << kind) != 0
    }
}

/// `ITRandomWeights`: cumulative lower bounds, including the explosion
/// sentinel (`nITKindMBallMonsterStart`, 32). Zero-weight entries are omitted.
/// The appearance actor's table spans all 20 common kinds; the container
/// table, the 16 utilities and the sentinel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DropWeights {
    pub(crate) kinds: [u8; 21],
    pub(crate) blocks: [u16; 21],
    pub(crate) len: usize,
    pub(crate) sum: u16,
}
impl DropWeights {
    pub fn new(switches: Switches, stage_weights: Option<&[u8; 20]>) -> Self {
        let mut out = Self::default();
        let Some(weights) = stage_weights else {
            return out;
        };
        for (kind, &weight) in weights.iter().enumerate().skip(4) {
            if switches.enabled(kind as u8) && weight != 0 {
                out.kinds[out.len] = kind as u8;
                out.blocks[out.len] = out.sum;
                out.len += 1;
                out.sum += u16::from(weight);
            }
        }
        if out.sum != 0 {
            out.kinds[out.len] = 32;
            out.blocks[out.len] = out.sum;
            out.len += 1;
            out.sum += ((f32::from(out.sum) * 0.1) as u16).max(1);
        }
        out
    }
    /// No RNG draw for an empty table. One shared draw otherwise.
    pub fn choose(&self) -> Option<u8> {
        if self.sum == 0 {
            return None;
        }
        let random = crate::rng::rand_int_range(i32::from(self.sum)) as u16;
        Some(self.kinds[self.blocks[..self.len].partition_point(|&b| b <= random) - 1])
    }
    /// Crate rerolls omit the terminal explosion weight. The first draw
    /// still uses the whole table (`itBoxCommonCheckSpawnItems`).
    pub(super) fn choose_utility(&self) -> Option<u8> {
        let mut utilities = *self;
        if utilities.len == 0 {
            return None;
        }
        utilities.len -= 1;
        utilities.sum = utilities.blocks[utilities.len];
        utilities.choose()
    }
}

/// `I_SEC_TO_TICS(n)`.
const fn secs(n: u16) -> u16 {
    n * 60
}
/// `dITManagerAppearanceRatesMin` / `Max`, by [`Appearance`].
const APPEARANCE_RATES_MIN: [u16; 6] = [secs(0), secs(30), secs(25), secs(20), secs(15), secs(10)];
const APPEARANCE_RATES_MAX: [u16; 6] = [
    secs(0),
    secs(30) + 90,
    secs(25) + 75,
    secs(20) + 60,
    secs(15) + 45,
    secs(10) + 30,
];
/// `item_mapobj_ids[30]`: more item points halt the source.
pub const APPEAR_POINTS_MAX: usize = 30;

/// `gITManagerAppearActor`: drops a random common item (containers
/// included, no explosion sentinel) on a random `nMPMapObjKindItem` point
/// every 10 to 30 seconds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AppearActor {
    pub(crate) weights: DropWeights,
    appearance: Appearance,
    points: [Vec3; APPEAR_POINTS_MAX],
    points_len: usize,
    pub spawn_wait: u16,
}

/// What [`AppearActor::tick`] asks the item manager to make.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AppearSpawn {
    pub kind: u8,
    pub pos: Vec3,
}

impl AppearActor {
    /// `itManagerMakeAppearActor`: `None` when the switches, the stage's
    /// weights or its item points rule the actor out. Draws the first
    /// spawn wait.
    pub fn new(
        switches: Switches,
        stage_weights: Option<&[u8; 20]>,
        points: impl IntoIterator<Item = Vec3>,
    ) -> Option<Self> {
        if switches.appearance == Appearance::None || switches.toggles == 0 {
            return None;
        }
        let weights = stage_weights?;
        let mut out = Self {
            weights: DropWeights::default(),
            appearance: switches.appearance,
            points: [Vec3::ZERO; APPEAR_POINTS_MAX],
            points_len: 0,
            spawn_wait: 0,
        };
        for (kind, &weight) in weights.iter().enumerate() {
            if switches.toggles & (1 << kind) != 0 {
                out.weights.sum += u16::from(weight);
            }
        }
        if out.weights.sum == 0 {
            return None;
        }
        for p in points {
            // The source halts with "Item positions are over 30!".
            assert!(
                out.points_len < APPEAR_POINTS_MAX,
                "item positions are over 30"
            );
            out.points[out.points_len] = p;
            out.points_len += 1;
        }
        if out.points_len == 0 {
            return None;
        }
        let mut block = 0;
        for (kind, &weight) in weights.iter().enumerate() {
            if switches.toggles & (1 << kind) != 0 && weight != 0 {
                out.weights.kinds[out.weights.len] = kind as u8;
                out.weights.blocks[out.weights.len] = block;
                out.weights.len += 1;
                block += u16::from(weight);
            }
        }
        out.set_spawn_wait();
        Some(out)
    }

    /// `itManagerSetItemSpawnWait`.
    fn set_spawn_wait(&mut self) {
        let i = self.appearance as usize;
        let (min, max) = (APPEARANCE_RATES_MIN[i], APPEARANCE_RATES_MAX[i]);
        self.spawn_wait = min + crate::rng::rand_int_range(i32::from(max - min)) as u16;
    }

    /// `itManagerAppearActorProcUpdate`. `started` is the game status past
    /// `nSCBattleGameStatusWait`; `can_alloc` is `itManagerGetCurrentAlloc`.
    /// Returns the item to make with `itManagerMakeItemSetupCommon` (no
    /// parent, zero velocity).
    pub fn tick(&mut self, started: bool, can_alloc: bool) -> Option<AppearSpawn> {
        if !started {
            return None;
        }
        if self.spawn_wait > 0 {
            self.spawn_wait -= 1;
            return None;
        }
        let spawn = can_alloc.then(|| {
            let kind = self.weights.choose().unwrap_or(0);
            let point = crate::rng::rand_int_range(self.points_len as i32) as usize;
            AppearSpawn {
                kind,
                pos: self.points[point],
            }
        });
        self.set_spawn_wait();
        spawn
    }
}

/// Direct item-manager calls made from item callbacks; distinct from animation.
pub(super) trait CommonItems {
    /// Direct call, before the weighted-drop RNG (`itBox.c`, `itTaru.c`).
    fn smash_container(&mut self, _pos: ssb_engine::math::Vec3) {}
    fn eggs_enabled(&self) -> bool;
    fn make_egg(
        &mut self,
        parent: &super::Item,
        pos: ssb_engine::math::Vec3,
        vel: ssb_engine::math::Vec3,
    ) -> bool;
    /// `itMainMakeContainerItem`: true for a utility selection even if its
    /// maker cannot allocate; false for an empty table or explosion sentinel.
    fn open_container(&mut self, parent: &mut super::Item) -> bool;
    fn open_crate(&mut self, parent: &mut super::Item) -> bool {
        self.open_container(parent)
    }
    /// `itMainMakeMonster` from a Poké Ball.
    fn make_monster(&mut self, _parent: &super::Item) {}
    /// `itManagerMakeItemSetupCommon(parent, nITKindEgg, ...)` with
    /// `ITEM_FLAG_COLLPROJECT | ITEM_FLAG_PARENT_ITEM`: the new Egg's
    /// facing, or `None` when no struct is free (Chansey's eggs).
    fn make_common_egg(
        &mut self,
        _parent: &super::Item,
        _pos: ssb_engine::math::Vec3,
        _vel: ssb_engine::math::Vec3,
    ) -> Option<i8> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn switches_and_cumulative_weights_keep_the_source_sentinel() {
        let mut weights = [0; 20];
        weights[4] = 1;
        weights[6] = 9;
        weights[19] = 10;
        let switches = Switches {
            appearance: Appearance::Low,
            toggles: (1 << 4) | (1 << 6) | (1 << 19),
        };
        let drops = DropWeights::new(switches, Some(&weights));
        assert_eq!(
            (&drops.kinds[..4], &drops.blocks[..4], drops.sum),
            (&[4, 6, 19, 32][..], &[0, 1, 10, 20][..], 22)
        );
        let small = DropWeights::new(
            Switches {
                toggles: 1 << 4,
                ..switches
            },
            Some(&weights),
        );
        assert_eq!(small.sum, 2);
        for drops in [
            DropWeights::new(
                Switches {
                    toggles: 0xF,
                    ..switches
                },
                Some(&weights),
            ),
            DropWeights::new(
                Switches {
                    appearance: Appearance::None,
                    ..switches
                },
                Some(&weights),
            ),
            DropWeights::new(switches, None),
        ] {
            let seed = crate::rng::seed();
            assert_eq!(drops.choose(), None);
            assert_eq!(crate::rng::seed(), seed);
        }
        for _ in 0..100 {
            assert!([4, 6, 19, 32].contains(&drops.choose().unwrap()));
        }
    }
}
