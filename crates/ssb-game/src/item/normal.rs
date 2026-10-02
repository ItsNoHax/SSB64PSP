//! Normal-item switches and `itManagerSetupContainerDrops`.
//! Utility makers remain separate ports; the runtime currently enables
//! only Capsule/Egg, so its container-drop table is empty.

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
    fn default() -> Self {
        Self {
            appearance: Appearance::Middle,
            toggles: (1 << 2) | (1 << 3),
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DropWeights {
    pub(crate) kinds: [u8; 17],
    pub(crate) blocks: [u16; 17],
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
}

/// Direct item-manager calls made from item callbacks; distinct from animation.
pub(super) trait CommonItems {
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
            DropWeights::new(Switches::default(), Some(&weights)),
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
