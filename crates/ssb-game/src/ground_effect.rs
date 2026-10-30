//! The stage's ambient effect actor (`efground.c`): `efGroundMakeAppearActor`
//! and `EFGroundActorProcUpdate`, which every 6000–15999 frames picks a
//! weighted effect (Lakitu, birds, Arwings, ...) and makes it.
//!
//! PSP deviation: only the actor's schedule and its random draws are
//! ported. The made effect (`efGroundMakeEffect`'s DObj tree, animation and
//! flight) is not drawn and holds no effect struct; whether it would be made
//! is [`crate::effect::HitEffectSink::ground_effect_made`].

use crate::d_arrays::{
    EF_GROUND_DATAS_EFFECT_PARAMS, EF_GROUND_DATAS_PARAMS_NUM, EF_GROUND_PARAMS_EFFECT_ID,
    EF_GROUND_PARAMS_EFFECT_WEIGHT, EF_GROUND_PARAMS_LR, EF_GROUND_PARAMS_MAKE_QUEUE,
};
use crate::effect::HitEffectSink;
use crate::rng;

/// `nGRKindBattleEnd`.
const GKIND_BATTLE_END: u8 = 8;
/// `dEFGroundCastleParams`, the first `EFGroundParam` (12 bytes each).
const PARAMS_BASE: u32 = 0x8012_F8C0;
const PARAM_SIZE: u32 = 12;

/// `sEFGroundActor`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroundActor {
    pub make_wait: i32,
    pub effect_id: u16,
    pub make_queue: u16,
    pub lr: i32,
    /// The stage's first `EFGroundParam` in the generated tables.
    params: usize,
    /// `effect_ids[..effect_count]`: a param index per weight unit.
    effect_ids: [u8; 255],
    pub effect_count: u8,
    /// Effects made so far (host tests and the capture log).
    pub made: u32,
}

impl GroundActor {
    /// `efGroundMakeAppearActor`, from `grCommonSetupInitAll` after
    /// `itManagerMakeAppearActor`. `None` (no draw) past
    /// `nGRKindBattleEnd`, in Training, or for a stage without params.
    pub fn make(gkind: u8, training: bool) -> Option<Self> {
        if gkind > GKIND_BATTLE_END || training {
            return None;
        }
        let ptr = EF_GROUND_DATAS_EFFECT_PARAMS[usize::from(gkind)];
        if ptr == 0 {
            return None;
        }
        let params = ((ptr - PARAMS_BASE) / PARAM_SIZE) as usize;
        let mut actor = Self {
            make_wait: rng::rand_int_range(10000) + 6000,
            effect_id: 0,
            make_queue: 0,
            lr: 0,
            params,
            effect_ids: [0; 255],
            effect_count: 0,
            made: 0,
        };
        actor.setup_random_weights(EF_GROUND_DATAS_PARAMS_NUM[usize::from(gkind)]);
        Some(actor)
    }

    /// `efGroundSetupRandomWeights`.
    fn setup_random_weights(&mut self, params_num: u8) {
        let weight = |i: u8| EF_GROUND_PARAMS_EFFECT_WEIGHT[self.params + usize::from(i)];
        let mut j: u8 = 0;
        for i in 0..params_num {
            j = j.wrapping_add(weight(i));
        }
        self.effect_count = j;
        let mut effect_weights = weight(0);
        let mut k: u8 = 0;
        for i in 0..j {
            if effect_weights == i {
                k += 1;
                effect_weights = effect_weights.wrapping_add(weight(k));
            }
            self.effect_ids[usize::from(i)] = k;
        }
    }

    /// The param indices per weight unit (host tests).
    pub fn effect_ids(&self) -> &[u8] {
        &self.effect_ids[..usize::from(self.effect_count)]
    }

    /// `EFGroundActorProcUpdate` (priority 1, link 7).
    pub fn tick(&mut self, effects: &mut dyn HitEffectSink) {
        if self.make_wait == 0 {
            let mut effect_id = self.effect_id;
            if self.make_queue == 0 {
                let pick = rng::rand_int_range(i32::from(self.effect_count)) as usize;
                let param = self.params + usize::from(self.effect_ids[pick]);
                self.effect_id = EF_GROUND_PARAMS_EFFECT_ID[param];
                effect_id = self.effect_id;
                self.lr = EF_GROUND_PARAMS_LR[param];
                if self.lr == 0 {
                    self.lr = if rng::rand_int_range(2) == 0 { -1 } else { 1 };
                }
                self.make_queue = EF_GROUND_PARAMS_MAKE_QUEUE[param];
                if self.make_queue != 0 {
                    self.make_queue += rng::rand_int_range(2) as u16;
                }
            } else {
                self.make_queue -= 1;
            }
            self.make_effect_id(effect_id, effects);
        }
        self.make_wait -= 1;
    }

    /// `efGroundMakeEffectID`.
    fn make_effect_id(&mut self, _effect_id: u16, effects: &mut dyn HitEffectSink) {
        // `efGroundMakeEffect`: every desc has `EFFECT_FLAG_USERDATA`, so
        // no free struct makes nothing. Made, `efGroundUpdatePhysics`
        // draws the altitude (`alt_low + (alt_high - alt_low) * rand`).
        if effects.ground_effect_made() {
            let _altitude = rng::rand_float();
            self.made += 1;
        }
        self.make_wait = if self.make_queue == 0 {
            rng::rand_int_range(10000) + 6000
        } else {
            30
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effect::NoEffects;

    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn one_draw_on_the_battle_stages_with_params() {
        for gkind in 0..=12u8 {
            rng::set_seed(1);
            let actor = GroundActor::make(gkind, false);
            let expect = matches!(gkind, 0 | 1 | 2 | 3 | 5 | 6 | 7);
            assert_eq!(actor.is_some(), expect, "gkind {gkind}");
            assert_eq!(rng::seed() != 1, expect, "gkind {gkind}");
        }
        rng::set_seed(1);
        assert!(GroundActor::make(6, true).is_none());
        assert_eq!(rng::seed(), 1);
    }

    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn weights_expand_to_param_indices() {
        let a = GroundActor::make(6, false).unwrap();
        let w = &EF_GROUND_PARAMS_EFFECT_WEIGHT[a.params..a.params + 6];
        let total: u32 = w.iter().map(|&x| u32::from(x)).sum();
        assert_eq!(u32::from(a.effect_count), total);
        let mut expect = Vec::new();
        for (k, &n) in w.iter().enumerate() {
            expect.extend(core::iter::repeat_n(k as u8, usize::from(n)));
        }
        assert_eq!(a.effect_ids(), &expect[..]);
    }

    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn makes_after_the_wait_then_rewaits() {
        rng::set_seed(7);
        let mut a = GroundActor::make(6, false).unwrap();
        let wait = a.make_wait;
        assert!((6000..16000).contains(&wait));
        for _ in 0..wait {
            a.tick(&mut NoEffects);
        }
        assert_eq!(a.made, 0);
        a.tick(&mut NoEffects);
        assert_eq!(a.made, 1);
        // A queued param re-makes after 30 frames, else a new long wait.
        if a.make_queue != 0 {
            assert_eq!(a.make_wait, 29);
        } else {
            assert!((5999..15999).contains(&a.make_wait));
        }
    }
}
