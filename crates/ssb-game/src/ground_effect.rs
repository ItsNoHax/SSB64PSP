//! The stage's ambient effects (`ef/efground.c`): the actor
//! (`efGroundMakeAppearActor`, `EFGroundActorProcUpdate`), which every
//! 6000–15999 frames picks a weighted effect (Lakitu, birds, Arwings, ...)
//! and makes it, and the descs the made effects fly by
//! ([`crate::effect::DisplayKind::Ground`]: `efGroundMakeEffect`,
//! `efGroundUpdatePhysics` and the three `proc_update`s).

use crate::d_arrays::{
    EF_GROUND_DATAS_EFFECT_DESCS, EF_GROUND_DATAS_EFFECT_PARAMS, EF_GROUND_DATAS_PARAMS_NUM,
    EF_GROUND_DESCS_PROC_GROUNDEFFECT, EF_GROUND_DESCS_PROC_UPDATE, EF_GROUND_PARAMS_EFFECT_ID,
    EF_GROUND_PARAMS_EFFECT_WEIGHT, EF_GROUND_PARAMS_LR, EF_GROUND_PARAMS_MAKE_QUEUE,
};
pub use crate::d_arrays::{
    EF_GROUND_DESCS_ALT_HIGH as ALT_HIGH, EF_GROUND_DESCS_ALT_LOW as ALT_LOW,
    EF_GROUND_DESCS_EFFECT_STATUS as EFFECT_STATUS, EF_GROUND_DESCS_FLAGS as FLAGS,
    EF_GROUND_DESCS_O_ANIM_JOINT as O_ANIM_JOINT, EF_GROUND_DESCS_O_DOBJSETUP as O_DOBJSETUP,
    EF_GROUND_DESCS_O_MATANIM_JOINT as O_MATANIM_JOINT, EF_GROUND_DESCS_O_MOBJSUB as O_MOBJSUB,
    EF_GROUND_DESCS_POS_Z as POS_Z, EF_GROUND_DESCS_SCALE as SCALE,
};
use crate::effect::HitEffectSink;
use crate::rng;

/// Each desc's flight cycle, generated from the ROM's scripts by
/// `crates/ssb-tablegen` (`ground_effect.rs`; D-048, zeros in a stub
/// build): `LIFE`, the `gcPlayAnimAll` call (the add's counted as 1) on
/// which the root's child ends its script, and `END`, the child's
/// `translate.x`, `.y` there on the first cycle and on later ones (its
/// `AObj`s outlive the re-add).
pub mod cycle {
    include!(concat!(env!("OUT_DIR"), "/ground_effects.rs"));
}

/// `dEFGroundCastleEffectDescs`, the first `EFGroundDesc` (64 bytes each).
const DESCS_BASE: u32 = 0x8012_F240;
const DESC_SIZE: u32 = 64;
/// The descs, from `dEFGroundCastleEffectDescs` to the end of
/// `dEFGroundYamabukiEffectDescs`.
pub const DESC_COUNT: usize = 24;
/// `efGroundSetStepPositions`, the one `proc_groundeffect`.
const SET_STEP_POSITIONS: u32 = 0x8011_61A0;
/// `efGroundUpdateStepPositions`, a `proc_update`.
const UPDATE_STEP_POSITIONS: u32 = 0x8011_60E8;
/// `efGroundUpdateEffectYaw`, a `proc_update`.
const UPDATE_EFFECT_YAW: u32 = 0x8011_6090;

/// Desc `desc`'s `proc_groundeffect` is `efGroundSetStepPositions`.
pub fn set_step_positions(desc: usize) -> bool {
    EF_GROUND_DESCS_PROC_GROUNDEFFECT[desc] == SET_STEP_POSITIONS
}

/// Desc `desc`'s `proc_update` is `efGroundUpdateStepPositions`.
pub fn step_positions(desc: usize) -> bool {
    EF_GROUND_DESCS_PROC_UPDATE[desc] == UPDATE_STEP_POSITIONS
}

/// Desc `desc`'s `proc_update` is `efGroundUpdateEffectYaw`: with `lr` +1,
/// the root's grandchild's `rotate.z` is negated after each play.
pub fn effect_yaw(desc: usize) -> bool {
    EF_GROUND_DESCS_PROC_UPDATE[desc] == UPDATE_EFFECT_YAW
}

/// `nGRKindBattleEnd`.
const GKIND_BATTLE_END: u8 = 8;

/// The first of `gkind`'s `EFGroundDesc`s (`dEFGroundDatas[gkind]
/// .effect_descs`), or `None` for a battle stage without ground effects.
pub fn first_desc(gkind: u8) -> Option<usize> {
    if gkind >= GKIND_BATTLE_END {
        return None;
    }
    let descs = EF_GROUND_DATAS_EFFECT_DESCS[usize::from(gkind)];
    (descs != 0).then(|| (descs.wrapping_sub(DESCS_BASE) / DESC_SIZE) as usize)
}
/// The file `sEFGroundActor.file_head` points at, by battle `gkind`: the
/// stage's `Stage*File2` (`relocData/1NN_Stage*File2.c`), which a desc's
/// `o_dobjsetup`, `o_mobjsub`, `o_anim_joint` and `o_matanim_joint` are
/// relative to. Layout, as `ssb_tablegen::ground_effect::STAGE_FILES`.
pub const STAGE_FILES: [u32; 8] = [106, 109, 108, 105, 0, 111, 104, 112];

/// The stage file desc `desc` belongs to: that of the battle stage whose
/// descs it falls among, or `None` past them (or in a stub build).
pub fn desc_file(desc: usize) -> Option<u32> {
    if desc >= DESC_COUNT {
        return None;
    }
    (0..GKIND_BATTLE_END)
        .filter_map(|g| Some((first_desc(g)?, STAGE_FILES[usize::from(g)])))
        .filter(|&(first, _)| first <= desc)
        .max_by_key(|&(first, _)| first)
        .map(|(_, file)| file)
}
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
    /// The stage's first `EFGroundDesc` (`effect_data->effect_descs`).
    descs: usize,
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
        let descs = EF_GROUND_DATAS_EFFECT_DESCS[usize::from(gkind)];
        let descs = (descs.wrapping_sub(DESCS_BASE) / DESC_SIZE) as usize;
        let mut actor = Self {
            make_wait: rng::rand_int_range(10000) + 6000,
            effect_id: 0,
            make_queue: 0,
            lr: 0,
            params,
            descs,
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
    fn make_effect_id(&mut self, effect_id: u16, effects: &mut dyn HitEffectSink) {
        // `efGroundMakeEffect`: every desc has `EFFECT_FLAG_USERDATA`, so
        // no free struct makes nothing. Made, `efGroundUpdatePhysics`
        // draws the altitude (`alt_low + (alt_high - alt_low) * rand`).
        let desc = self.descs + usize::from(effect_id);
        if effects.ground_effect(desc as u8, self.lr) {
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
