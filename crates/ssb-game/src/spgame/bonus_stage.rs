//! Break the Targets' separate battle and scene accounting
//! (`sc1pbonusstage.c`). Its results never overwrite campaign stocks.
use super::{BattleState, PlayerKind, SceneData, Stage, BONUSGAME_TASK_MAX};
use crate::{
    battle::{Battle, TIMELIMIT_INFINITE},
    fighter::Fighter,
};
use ssb_engine::math::Vec3;

pub struct BonusStage {
    pub state: BattleState,
    pub tasks_remain: u8,
}

impl BonusStage {
    pub fn targets(data: &SceneData) -> Self {
        assert_eq!(data.stage(), Some(Stage::Bonus1));
        let mut state = BattleState {
            gkind: 17 + data.fkind as u8,
            pl_count: 1,
            cp_count: 0,
            time_limit: if data.time_limit == TIMELIMIT_INFINITE {
                TIMELIMIT_INFINITE
            } else {
                2
            },
            ..Default::default()
        };
        let p = &mut state.players[data.player as usize];
        p.pkind = PlayerKind::Man;
        p.fkind = data.fkind;
        p.costume = data.costume;
        p.color = data.player;
        Self {
            state,
            tasks_remain: BONUSGAME_TASK_MAX,
        }
    }

    pub fn battle(&self) -> Battle {
        Battle::new_bonus(self.state.time_limit, self.state.battle_players())
    }

    /// `sc1PBonusStageMakeTargets`, in descriptor order, skipping its root.
    pub fn make_targets(&self, positions: &[Vec3], items: &mut dyn crate::stage::StageItems) {
        assert_eq!(positions.len(), BONUSGAME_TASK_MAX as usize);
        for (i, &pos) in positions.iter().enumerate() {
            assert!(
                items
                    .make_item(crate::stage::StageItem::Target(i as u8), pos)
                    .is_some(),
                "target allocation failed"
            );
        }
    }

    /// The fallen fighter's score callback precedes priority-0 target
    /// damage callbacks. Timer failure, already dispatched, wins ties.
    pub fn observe(&mut self, fighter: &mut Fighter, battle: &mut Battle, broken: u8) -> bool {
        for event in fighter.stats.drain() {
            if let super::live::Event::Damage(damage) = event {
                self.state
                    .update_damage(fighter.port, damage, u32::from(fighter.damage));
            }
        }
        let fell = core::mem::take(&mut fighter.dead.scored);
        if fell {
            battle.on_fall(fighter.port, fighter.damage_player);
        }
        assert!(broken <= self.tasks_remain);
        self.tasks_remain -= broken;
        if self.tasks_remain == 0 {
            battle.announce_complete();
        }
        fell
    }

    pub fn result(&mut self, battle: &Battle) -> &BattleState {
        self.state.time_passed = battle.time_passed;
        self.state.time_remain = battle.time_remain;
        &self.state
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::battle::{EndKind, Frame, GameStatus};
    use crate::fighter::FighterKind;

    fn course() -> (BonusStage, Fighter) {
        let data = SceneData {
            stage: Stage::Bonus1 as u8,
            ..Default::default()
        };
        (
            BonusStage::targets(&data),
            Fighter::new(FighterKind::Mario, 0, 3),
        )
    }

    #[test]
    fn go_wait_clock_and_timeout_use_the_bonus_rules() {
        let (s, _) = course();
        let mut b = s.battle();
        for _ in 0..60 {
            assert_eq!(b.begin_frame(), Frame::Run);
            assert_eq!(b.status, GameStatus::Wait);
        }
        b.begin_frame();
        assert_eq!(
            (b.status, b.time_passed, b.time_remain),
            (GameStatus::Go, 0, 7200)
        );
        b.begin_frame();
        for _ in 0..7200 {
            b.begin_frame();
        }
        assert_eq!(b.end, Some(EndKind::Failure));
        assert_eq!(
            (b.time_passed, b.time_remain, b.players[0].stock_count),
            (7200, 0, 0)
        );
    }

    #[test]
    fn completion_wait_returns_without_victory_zoom() {
        let (mut s, mut f) = course();
        let mut b = s.battle();
        s.observe(&mut f, &mut b, 9);
        assert!(b.end.is_none());
        s.observe(&mut f, &mut b, 1);
        assert_eq!(b.end, Some(EndKind::Complete));
        for _ in 0..90 {
            assert_eq!(b.begin_frame(), Frame::Frozen);
        }
        assert_eq!(b.status, GameStatus::BossDefeat);
        b.begin_frame();
        assert_eq!(b.status, GameStatus::Set);
        assert!(!b.set_zoom);
        for _ in 0..3 {
            assert_eq!(b.begin_frame(), Frame::Frozen);
        }
        assert_eq!(b.begin_frame(), Frame::Done);
    }

    #[test]
    fn fall_failure_wins_same_frame_last_target_and_never_takes_stock() {
        let (mut s, mut f) = course();
        let mut b = s.battle();
        f.dead.scored = true;
        assert!(s.observe(&mut f, &mut b, 10));
        assert_eq!(b.end, Some(EndKind::Failure));
        assert_eq!(
            (b.players[0].stock_count, b.players[0].falls, f.stocks),
            (0, 1, 3)
        );
        assert!(!s.observe(&mut f, &mut b, 0));
        assert_eq!(b.players[0].falls, 1);
    }

    #[test]
    fn every_fighter_course_and_infinite_limit_keep_selection() {
        for k in 0..12 {
            let fkind = FighterKind::from_ordinal(k).unwrap();
            let data = SceneData {
                stage: Stage::Bonus1 as u8,
                fkind,
                costume: 2,
                time_limit: TIMELIMIT_INFINITE,
                player: 3,
                ..Default::default()
            };
            let s = BonusStage::targets(&data);
            assert_eq!(s.state.gkind, 17 + k);
            assert_eq!(s.state.pl_count, 1);
            assert_eq!(s.state.players[3].costume, 2);
            assert_eq!(s.state.players[3].fkind, fkind);
            let mut b = s.battle();
            for _ in 0..8000 {
                b.begin_frame();
            }
            assert!(b.end.is_none());
        }
    }
}
