//! `sc1PGameAppendBonusStats` and `sc1PGameInitBonusStats` (US).
//! Counters use hit-stat IDs. The original's item scans name motion IDs;
//! those IDs happen to agree with hit-stat IDs over the item range.
use super::manager::Manager;
use super::{BattleState, SceneData, Stage, MAX_TEAM_COUNT};
use crate::{item::normal::Appearance, stale::MotionAttackId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Bonus {
    CheapShot,
    StarFinish,
    NoItem,
    ShieldBreaker,
    JudoWarrior,
    Hawk,
    Shooter,
    HeavyDamage,
    AllVariations,
    ItemStrike,
    DoubleKO,
    Trickster,
    GiantImpact,
    Speedster,
    ItemThrow,
    TripleKO,
    LastChance,
    Pacifist,
    Perfect,
    NoMiss,
    NoDamage,
    FullPower,
    StageClear,
    NoMissClear,
    NoDamageClear,
    SpeedKing,
    SpeedDemon,
    MewCatcher,
    StarClear,
    Vegetarian,
    HeartThrob,
    ThrowDown,
    SmashMania,
    Smashless,
    SpecialMove,
    SingleMove,
    PokemonFinish,
    BoobyTrap,
    FighterStance,
    Mystic,
    CometMystic,
    AcidClear,
    BumperClear,
    TornadoClear,
    ArwingClear,
    CounterAttack,
    MeteorSmash,
    Aerial,
    LastSecond,
    Lucky3,
    Jackpot,
    YoshiRainbow,
    KirbyRanks,
    BrosCalamity,
    DKDefender,
    DKPerfect,
    GoodFriend,
    TrueFriend,
}

impl Bonus {
    pub fn insert(self, mask: &mut [u32; 3]) {
        mask[self as usize / 32] |= 1 << (self as u8 % 32);
    }
    pub fn contains(self, mask: &[u32; 3]) -> bool {
        mask[self as usize / 32] & (1 << (self as u8 % 32)) != 0
    }
}

/// `FTStatusAttackIndex`, which places the later jabs after aerials.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum HitAttackId {
    #[default]
    None,
    Attack11,
    AttackDash,
    AttackS3,
    AttackHi3,
    AttackLw3,
    AttackS4,
    AttackHi4,
    AttackLw4,
    AttackAirN,
    AttackAirF,
    AttackAirB,
    AttackAirHi,
    AttackAirLw,
    Attack12,
    Attack13,
    Attack100,
    SpecialHi,
    SpecialN,
    SpecialNCopyMario,
    SpecialNCopyLuigi,
    SpecialNCopyFox,
    SpecialNCopySamus,
    SpecialNCopyDonkey,
    SpecialNCopyPikachu,
    SpecialNCopyNess,
    SpecialNCopyLink,
    SpecialNCopyPurin,
    SpecialNCopyCaptain,
    SpecialNCopyYoshi,
    SpecialLw,
    DownAttackD,
    DownAttackU,
    CliffAttackQuick,
    CliffAttackSlow,
    ThrowF,
    ThrowB,
    SwordSwing1,
    SwordSwing3,
    SwordSwing4,
    SwordSwingDash,
    BatSwing1,
    BatSwing3,
    BatSwing4,
    BatSwingDash,
    HarisenSwing1,
    HarisenSwing3,
    HarisenSwing4,
    HarisenSwingDash,
    StarRodSwing1,
    StarRodSwing3,
    StarRodSwing4,
    StarRodSwingDash,
    LGunShoot,
    FireFlowerShoot,
    Hammer,
    ItemThrow,
    Null,
}
pub const ATTACK_COUNT: usize = HitAttackId::Null as usize + 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DeadStatus {
    Down,
    LeftRight,
    UpStar,
    UpFall,
    #[default]
    Other,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EndStatus {
    Appeal,
    DeadDown,
    DeadLeftRight,
    DeadUpStar,
    #[default]
    Other,
}
impl From<crate::status::Status> for EndStatus {
    fn from(s: crate::status::Status) -> Self {
        use crate::status::Status;
        match s {
            Status::Appeal => Self::Appeal,
            Status::DeadDown => Self::DeadDown,
            Status::DeadLeftRight => Self::DeadLeftRight,
            Status::DeadUpStar => Self::DeadUpStar,
            _ => Self::Other,
        }
    }
}

/// Only object categories read by the bonus predicates. PokemonWeapon
/// includes the original MonsterStart..MonsterEnd range's Saffron shots.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DamageObject {
    #[default]
    Other,
    PokemonItem,
    PokemonWeapon,
    MSBomb,
    GroundBumper,
    Bumper,
    Acid,
    Twister,
    Arwing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DefeatRecord {
    pub team_order: u8,
    pub status: DeadStatus,
    pub damage_player: Option<u8>,
    pub object: DamageObject,
    pub attack_id: HitAttackId,
    pub stat_count: u16,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BonusCounters {
    pub defeats: [DefeatRecord; MAX_TEAM_COUNT],
    pub defeat_count: usize,
    pub attack_id_count: [u32; ATTACK_COUNT],
    pub attack_is_smash: [u32; 2],
    pub attack_ground_air: [u32; 2],
    pub attack_is_projectile: [u32; 2],
    pub defend_id_count: [u32; ATTACK_COUNT],
    pub defend_is_smash: [u32; 2],
    pub defend_ground_air: [u32; 2],
    pub defend_is_projectile: [u32; 2],
    pub tomato_count: u8,
    pub heart_count: u8,
    pub star_count: u8,
    pub shield_breaker: bool,
    pub bros_calamity: bool,
    pub mew_catcher: bool,
    pub end_player_status: Option<EndStatus>,
    pub end_invincible_tics: u32,
}

impl Default for BonusCounters {
    fn default() -> Self {
        Self {
            defeats: [DefeatRecord::default(); MAX_TEAM_COUNT],
            defeat_count: 0,
            attack_id_count: [0; ATTACK_COUNT],
            attack_is_smash: [0; 2],
            attack_ground_air: [0; 2],
            attack_is_projectile: [0; 2],
            defend_id_count: [0; ATTACK_COUNT],
            defend_is_smash: [0; 2],
            defend_ground_air: [0; 2],
            defend_is_projectile: [0; 2],
            tomato_count: 0,
            heart_count: 0,
            star_count: 0,
            shield_breaker: false,
            bros_calamity: false,
            mew_catcher: false,
            end_player_status: None,
            end_invincible_tics: 0,
        }
    }
}

impl BonusCounters {
    pub fn push_defeat(&mut self, record: DefeatRecord) {
        assert!(self.defeat_count < MAX_TEAM_COUNT);
        self.defeats[self.defeat_count] = record;
        self.defeat_count += 1;
    }

    /// `ftParamUpdate1PGameAttackStats`: count the current nonzero attack
    /// when it differs from the saved previous stat ID.
    pub fn attack(&mut self, id: HitAttackId, smash: bool, air: bool, projectile: bool) {
        if id == HitAttackId::None {
            return;
        }
        self.attack_id_count[id as usize] += 1;
        self.attack_is_smash[usize::from(smash)] += 1;
        self.attack_ground_air[usize::from(air)] += 1;
        self.attack_is_projectile[usize::from(projectile)] += 1;
    }

    /// `ftParamUpdate1PGameDamageStats`: a new hit-stat count credited to
    /// the human player (the caller suppresses duplicate nonzero counts).
    pub fn defend(&mut self, id: HitAttackId, smash: bool, air: bool, projectile: bool) {
        if id == HitAttackId::None {
            return;
        }
        self.defend_id_count[id as usize] += 1;
        self.defend_is_smash[usize::from(smash)] += 1;
        self.defend_ground_air[usize::from(air)] += 1;
        self.defend_is_projectile[usize::from(projectile)] += 1;
    }

    fn append(
        &self,
        game: &super::Game,
        state: &BattleState,
        scene: &mut SceneData,
        manager: &Manager,
    ) {
        use Bonus::*;
        use HitAttackId as H;
        let p = state.players[usize::from(scene.player)];
        let mask = &mut scene.bonus_get_mask;
        let mut award = |bonus: Bonus, yes: bool| {
            if yes {
                bonus.insert(mask);
            }
        };
        let defeats = &self.defeats[..self.defeat_count];
        award(
            if matches!(game.stage, Stage::Yoshi | Stage::Kirby | Stage::Zako) {
                Trickster
            } else {
                StarFinish
            },
            !defeats.is_empty()
                && defeats.iter().all(|d| {
                    d.damage_player == Some(scene.player)
                        && matches!(d.status, DeadStatus::UpStar | DeadStatus::UpFall)
                }),
        );
        let used: alloc::vec::Vec<usize> = (1..ATTACK_COUNT)
            .filter(|&i| self.defend_id_count[i] != 0)
            .collect();
        let item_strike = !used.is_empty() && used.iter().all(|&i| i >= H::SwordSwing1 as usize);
        award(ItemStrike, item_strike);
        award(
            NoItem,
            !item_strike
                && state.item_appearance != Appearance::None
                && self.tomato_count == 0
                && self.heart_count == 0
                && self.star_count == 0
                && self.attack_id_count
                    [MotionAttackId::SwordSwing1 as usize..=MotionAttackId::ItemThrow as usize]
                    .iter()
                    .all(|&n| n == 0)
                && self.defend_id_count[H::Null as usize] == 0,
        );
        award(ShieldBreaker, self.shield_breaker);
        award(
            JudoWarrior,
            !used.is_empty()
                && used
                    .iter()
                    .all(|&i| i == H::ThrowF as usize || i == H::ThrowB as usize),
        );
        award(
            Hawk,
            self.defend_ground_air[1] != 0 && self.defend_ground_air[0] == 0,
        );
        award(
            Shooter,
            self.defend_is_projectile[1] != 0 && self.defend_is_projectile[0] == 0,
        );
        // Ness intentionally requires only jab 2, just like the source.
        use crate::fighter::FighterKind as K;
        let jab = |id: H| self.defend_id_count[id as usize] != 0;
        let jabs = match scene.fkind {
            K::Mario | K::Luigi => jab(H::Attack12) && jab(H::Attack13),
            K::Fox | K::Kirby => jab(H::Attack12) && jab(H::Attack100),
            K::Link | K::Captain => jab(H::Attack12) && jab(H::Attack13) && jab(H::Attack100),
            _ => jab(H::Attack12),
        };
        award(
            AllVariations,
            (H::Attack11 as usize..H::Attack12 as usize).all(|i| self.defend_id_count[i] != 0)
                && jabs,
        );
        award(
            ItemThrow,
            !used.is_empty()
                && used
                    .iter()
                    .all(|&i| i == MotionAttackId::ItemThrow as usize),
        );
        award(
            SmashMania,
            self.defend_is_smash[1] != 0 && self.defend_is_smash[0] == 0,
        );
        award(
            Smashless,
            self.defend_is_smash[1] == 0 && self.defend_is_smash[0] != 0,
        );
        let specials_only = !used.is_empty()
            && used
                .iter()
                .all(|&i| (H::SpecialHi as usize..=H::SpecialLw as usize).contains(&i));
        award(SingleMove, specials_only && used.len() == 1);
        award(SpecialMove, specials_only && used.len() != 1);
        award(HeavyDamage, p.stock_damage_all >= 200);
        award(Speedster, state.time_passed / 60 <= 30);
        award(Pacifist, p.total_damage_given == 0);
        award(NoMiss, manager.total_falls == 0);
        award(NoDamage, p.falls == 0 && p.total_damage_all == 0);
        award(FullPower, p.stock_damage_all == 0);
        award(StarClear, self.end_invincible_tics != 0);
        award(Vegetarian, self.tomato_count >= 3);
        award(HeartThrob, self.heart_count >= 3);
        award(MewCatcher, self.mew_catcher);
        if let Some(d) = defeats.last() {
            let mine = d.damage_player == Some(scene.player);
            let owner = d.damage_player.is_some();
            award(
                PokemonFinish,
                mine && matches!(
                    d.object,
                    DamageObject::PokemonItem | DamageObject::PokemonWeapon
                ),
            );
            award(
                BoobyTrap,
                mine && d.object == DamageObject::MSBomb && d.attack_id == H::Null,
            );
            award(
                ThrowDown,
                mine && matches!(d.attack_id, H::ThrowF | H::ThrowB),
            );
            award(AcidClear, owner && d.object == DamageObject::Acid);
            award(
                BumperClear,
                (owner && d.object == DamageObject::GroundBumper)
                    || (mine && d.object == DamageObject::Bumper),
            );
            award(TornadoClear, owner && d.object == DamageObject::Twister);
            award(ArwingClear, owner && d.object == DamageObject::Arwing);
        }
        award(
            FighterStance,
            self.end_player_status == Some(EndStatus::Appeal),
        );
        award(
            Mystic,
            matches!(
                self.end_player_status,
                Some(EndStatus::DeadDown | EndStatus::DeadLeftRight)
            ),
        );
        award(
            CometMystic,
            self.end_player_status == Some(EndStatus::DeadUpStar),
        );
        award(
            LastSecond,
            state.time_limit != crate::battle::TIMELIMIT_INFINITE && scene.time_remain == 1,
        );
        award(
            Lucky3,
            state.time_limit != crate::battle::TIMELIMIT_INFINITE && scene.time_remain == 213,
        );
        let damage = p.stock_damage_all;
        award(
            Jackpot,
            damage >= 11
                && if damage < 100 {
                    damage / 10 == damage % 10
                } else {
                    damage / 100 == damage / 10 % 10 && damage / 10 % 10 == damage % 10
                },
        );
        // Yoshi's first six can be any order (even duplicates); the next
        // twelve must repeat that exact sequence. Do not add uniqueness.
        award(
            YoshiRainbow,
            game.stage == Stage::Yoshi
                && game.enemy_stocks_remaining == 0
                && defeats.len() >= 6
                && (6..defeats.len()).all(|i| {
                    game.variations[defeats[i].team_order as usize]
                        == game.variations[defeats[(i - 6) % 6].team_order as usize]
                }),
        );
        award(
            KirbyRanks,
            game.stage == Stage::Kirby
                && game.enemy_stocks_remaining == 0
                && defeats.len() >= 7
                && defeats
                    .iter()
                    .take(7)
                    .enumerate()
                    .all(|(i, d)| usize::from(d.team_order) == i),
        );
        let ally0 = state.players[scene.ally_players[0] as usize];
        let ally1 = state.players[scene.ally_players[1] as usize];
        if game.stage == Stage::Mario {
            award(TrueFriend, ally0.falls == 0 && ally0.total_damage_all == 0);
            award(GoodFriend, ally0.falls == 0 && ally0.total_damage_all != 0);
            award(BrosCalamity, self.bros_calamity);
        }
        if game.stage == Stage::Donkey && ally0.falls == 0 && ally1.falls == 0 {
            award(
                DKPerfect,
                ally0.total_damage_all == 0 && ally1.total_damage_all == 0,
            );
            award(
                DKDefender,
                ally0.total_damage_all != 0 || ally1.total_damage_all != 0,
            );
        }
        if game.stage == Stage::Boss {
            award(StageClear, true);
            award(NoMissClear, manager.total_falls == 0);
            award(NoDamageClear, manager.total_damage == 0);
            award(SpeedDemon, manager.total_time_tics / 60 <= 8 * 60);
            award(
                SpeedKing,
                manager.total_time_tics / 60 > 8 * 60 && manager.total_time_tics / 60 <= 12 * 60,
            );
        }
        let exclusions = [
            SingleMove,
            SpecialMove,
            Hawk,
            JudoWarrior,
            Shooter,
            AllVariations,
            ItemStrike,
            Trickster,
            ItemThrow,
            Pacifist,
            SmashMania,
        ];
        if !exclusions.iter().any(|b| b.contains(mask)) {
            let total: u32 = self.attack_id_count[1..].iter().sum();
            if total != 0
                && self.attack_id_count[1..]
                    .iter()
                    .any(|&n| n as f32 / total as f32 >= 0.35)
            {
                CheapShot.insert(mask);
            }
        }
    }
}

impl super::Game {
    /// Called once after the battle returns. Totals include unsuccessful
    /// attempts; bonus stages 1/2 have their own scene accounting.
    pub fn init_bonus_stats(
        &self,
        state: &BattleState,
        scene: &mut SceneData,
        manager: &mut Manager,
    ) {
        scene.bonus_get_mask = [0; 3];
        scene.time_remain = if state.time_limit == crate::battle::TIMELIMIT_INFINITE {
            0
        } else {
            state.time_remain.div_ceil(60)
        };
        let p = state.players[usize::from(scene.player)];
        manager.total_time_tics += state.time_passed;
        manager.total_falls += u32::from(p.falls);
        manager.total_damage += p.total_damage_all;
        if self.stage == Stage::Bonus3 {
            if p.falls == 0 && scene.time_remain != 0 && p.total_damage_all == 0 {
                Bonus::NoDamage.insert(&mut scene.bonus_get_mask);
            }
        } else {
            scene.score += u32::from(p.score) * 1000;
            self.bonus.append(self, state, scene, manager);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fighter::FighterKind as K;
    use crate::spgame::{Backup, Difficulty};

    fn evaluate(
        stage: Stage,
        damage: u32,
        time: u32,
        edit: impl FnOnce(&mut super::super::Game, &mut BattleState, &mut SceneData, &mut Manager),
    ) -> [u32; 3] {
        let mut scene = SceneData {
            stage: stage as u8,
            fkind: K::Ness,
            ..Default::default()
        };
        let mut state = BattleState::default();
        let mut manager = Manager::new(&mut scene, &mut state, &Backup::default());
        let mut game = super::super::Game::setup_stage_all(
            &mut state,
            &mut scene,
            Difficulty::Normal,
            0,
            K::Ness,
        );
        state.time_passed = time;
        state.time_remain = 1;
        state.players[0].stock_damage_all = damage;
        edit(&mut game, &mut state, &mut scene, &mut manager);
        game.init_bonus_stats(&state, &mut scene, &mut manager);
        scene.bonus_get_mask
    }

    #[test]
    fn thresholds_use_integer_seconds_ceil_remaining_and_us_damage() {
        for (damage, ticks, heavy, speed, jackpot) in [
            (199, 1859, false, true, false),
            (200, 1860, true, false, false),
            (11, 0, false, true, true),
            (111, 0, false, true, true),
            (0, 0, false, true, false),
            (101, 0, false, true, false),
            (999, 0, true, true, true),
        ] {
            let mask = evaluate(Stage::Link, damage, ticks, |_, _, _, _| {});
            assert_eq!(Bonus::HeavyDamage.contains(&mask), heavy);
            assert_eq!(Bonus::Speedster.contains(&mask), speed);
            assert_eq!(Bonus::Jackpot.contains(&mask), jackpot);
            assert!(Bonus::LastSecond.contains(&mask));
        }
    }

    #[test]
    fn star_finish_requires_every_enemy_and_team_awards_trickster() {
        for stage in [Stage::Link, Stage::Yoshi] {
            let mask = evaluate(stage, 0, 0, |g, _, _, _| {
                g.bonus.push_defeat(DefeatRecord {
                    status: DeadStatus::UpStar,
                    damage_player: Some(0),
                    ..Default::default()
                });
            });
            assert_eq!(Bonus::Trickster.contains(&mask), stage == Stage::Yoshi);
            assert_eq!(Bonus::StarFinish.contains(&mask), stage == Stage::Link);
            let mask = evaluate(stage, 0, 0, |g, _, _, _| {
                g.bonus.push_defeat(DefeatRecord {
                    status: DeadStatus::UpStar,
                    damage_player: Some(0),
                    ..Default::default()
                });
                g.bonus.push_defeat(DefeatRecord {
                    status: DeadStatus::UpFall,
                    damage_player: Some(1),
                    ..Default::default()
                });
            });
            assert!(!Bonus::Trickster.contains(&mask) && !Bonus::StarFinish.contains(&mask));
        }
    }

    #[test]
    fn item_strike_disables_no_item_and_null_damage_also_disables_it() {
        let mask = evaluate(Stage::Link, 0, 0, |g, _, _, _| {
            g.bonus.defend(HitAttackId::Null, false, false, false)
        });
        assert!(Bonus::ItemStrike.contains(&mask));
        assert!(!Bonus::NoItem.contains(&mask));
        let mask = evaluate(Stage::Link, 0, 0, |g, _, _, _| {
            g.bonus.defend(HitAttackId::Null, false, false, false);
            g.bonus.defend(HitAttackId::Attack11, false, false, false);
        });
        assert!(!Bonus::ItemStrike.contains(&mask) && !Bonus::NoItem.contains(&mask));
        let mask = evaluate(Stage::Link, 0, 0, |g, _, _, _| g.bonus.star_count = 1);
        assert!(!Bonus::NoItem.contains(&mask));
    }

    #[test]
    fn single_special_and_special_move_are_exclusive_and_suppress_cheap_shot() {
        for count in 1..=2 {
            let mask = evaluate(Stage::Link, 0, 0, |g, s, _, _| {
                s.players[0].total_damage_given = 1;
                g.bonus.defend(HitAttackId::SpecialN, false, false, false);
                g.bonus.attack(HitAttackId::SpecialN, false, false, false);
                if count == 2 {
                    g.bonus.defend(HitAttackId::SpecialHi, false, false, false);
                }
            });
            assert_eq!(Bonus::SingleMove.contains(&mask), count == 1);
            assert_eq!(Bonus::SpecialMove.contains(&mask), count == 2);
            assert!(!Bonus::CheapShot.contains(&mask));
        }
    }

    #[test]
    fn all_variations_preserves_ness_jab_two_quirk_and_item_throw_id() {
        let mask = evaluate(Stage::Link, 0, 0, |g, _, _, _| {
            for n in &mut g.bonus.defend_id_count[1..=HitAttackId::Attack12 as usize] {
                *n = 1;
            }
        });
        assert!(Bonus::AllVariations.contains(&mask));
        assert_eq!(
            MotionAttackId::ItemThrow as usize,
            HitAttackId::ItemThrow as usize
        );
        let mask = evaluate(Stage::Link, 0, 0, |g, _, _, _| {
            g.bonus.defend(HitAttackId::ItemThrow, false, false, false)
        });
        assert!(Bonus::ItemThrow.contains(&mask));
    }

    #[test]
    fn enemy_credit_and_environment_gate_finishing_bonuses() {
        for (object, owner, expected) in [
            (DamageObject::MSBomb, Some(0), Some(Bonus::BoobyTrap)),
            (DamageObject::MSBomb, Some(1), None),
            (DamageObject::Acid, None, None),
            (DamageObject::Acid, Some(1), Some(Bonus::AcidClear)),
            (
                DamageObject::PokemonWeapon,
                Some(0),
                Some(Bonus::PokemonFinish),
            ),
        ] {
            let mask = evaluate(Stage::Link, 0, 0, |g, _, _, _| {
                g.bonus.push_defeat(DefeatRecord {
                    object,
                    damage_player: owner,
                    attack_id: HitAttackId::Null,
                    ..Default::default()
                })
            });
            for b in [Bonus::BoobyTrap, Bonus::AcidClear, Bonus::PokemonFinish] {
                assert_eq!(b.contains(&mask), expected == Some(b));
            }
        }
    }

    #[test]
    fn boss_totals_include_previous_attempts_and_speed_uses_inclusive_seconds() {
        for (ticks, demon, king) in [
            (28_859, true, false),
            (28_860, false, true),
            (43_259, false, true),
            (43_260, false, false),
        ] {
            let mask = evaluate(Stage::Boss, 0, ticks, |_, _, _, m| {
                m.total_falls = 1;
                m.total_damage = 1;
            });
            assert!(Bonus::StageClear.contains(&mask));
            assert!(!Bonus::NoMissClear.contains(&mask) && !Bonus::NoDamageClear.contains(&mask));
            assert_eq!(Bonus::SpeedDemon.contains(&mask), demon);
            assert_eq!(Bonus::SpeedKing.contains(&mask), king);
        }
    }

    #[test]
    fn race_only_awards_no_damage_when_alive_with_time_left() {
        let mask = evaluate(Stage::Bonus3, 0, 0, |_, s, _, _| s.players[0].score = 20);
        assert_eq!(mask, [1 << Bonus::NoDamage as u8, 0, 0]);
        let mask = evaluate(Stage::Bonus3, 0, 0, |_, s, _, _| s.time_remain = 0);
        assert_eq!(mask, [0; 3]);
    }
}
