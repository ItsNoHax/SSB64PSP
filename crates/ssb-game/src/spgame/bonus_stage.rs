//! Break the Targets and Board the Platforms' separate battle and accounting
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
    pub platforms: [Option<Platform>; BONUSGAME_TASK_MAX as usize],
    /// Ticks of the source's 12-tick scene-entry fade while the world runs.
    pub fade_ticks: u8,
    /// The bonus pause menu's L: RETRY scene reload request.
    pub retry_requested: bool,
}

/// A DETECT floor's yakumono, shared by every line on that platform.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Platform {
    pub group: u16,
    pub kind: u8,
    pub boarded: bool,
}

pub fn platform_kind(width: f32) -> u8 {
    if width <= 750.0 {
        0
    } else if width <= 1050.0 {
        1
    } else {
        2
    }
}

impl BonusStage {
    pub fn targets(data: &SceneData) -> Self {
        assert_eq!(data.stage(), Some(Stage::Bonus1));
        Self::new(data, 17)
    }

    pub fn platforms(data: &SceneData, platforms: &[Platform]) -> Self {
        assert_eq!(data.stage(), Some(Stage::Bonus2));
        assert_eq!(platforms.len(), BONUSGAME_TASK_MAX as usize);
        let mut this = Self::new(data, 29);
        for (slot, platform) in this.platforms.iter_mut().zip(platforms) {
            *slot = Some(*platform);
        }
        this
    }

    fn new(data: &SceneData, first_ground: u8) -> Self {
        let mut state = BattleState {
            gkind: first_ground + data.fkind as u8,
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
            platforms: [None; BONUSGAME_TASK_MAX as usize],
            fade_ticks: 0,
            retry_requested: false,
        }
    }

    /// `lbFadeProcDisplay`: the source fades opaque black to transparent,
    /// ejecting its actor two ticks after the 12 alpha updates.
    pub fn fade_alpha(&self) -> u8 {
        if self.fade_ticks >= 14 {
            return 0;
        }
        let current = self.fade_ticks.min(12);
        255 - (u16::from(current) * 255 / 12) as u8
    }

    /// Priority 4, after fighter interrupts and before movement/item hits.
    /// Return the replaced platform's index so the host can restart its tree.
    pub fn board(&mut self, fighter: &Fighter, group: u16, battle: &mut Battle) -> Option<usize> {
        if !fighter.is_grounded()
            || !fighter
                .floor
                .is_some_and(|s| s.material() == crate::stage::bonus3::MATERIAL_DETECT)
        {
            return None;
        }
        let i = self
            .platforms
            .iter()
            .position(|p| p.is_some_and(|p| p.group == group && !p.boarded))?;
        self.platforms[i].as_mut().unwrap().boarded = true;
        self.tasks_remain -= 1;
        if self.tasks_remain == 0 {
            battle.announce_complete();
        }
        Some(i)
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

    fn platform_course() -> (BonusStage, Fighter) {
        let data = SceneData {
            stage: Stage::Bonus2 as u8,
            ..Default::default()
        };
        let platforms = core::array::from_fn::<_, 10, _>(|i| Platform {
            group: i as u16 + 1,
            kind: (i % 3) as u8,
            boarded: false,
        });
        (
            BonusStage::platforms(&data, &platforms),
            Fighter::new(FighterKind::Mario, 0, 3),
        )
    }

    #[test]
    fn platforms_credit_ground_contact_once_per_group_and_complete_all_ten() {
        use crate::{fighter::Situation, ground::Standing};
        let (mut scene, mut fighter) = platform_course();
        let mut battle = scene.battle();
        fighter.floor = Some(Standing {
            line: 42,
            flags: 0x800e,
            normal: ssb_engine::math::Vec2::new(0.0, 1.0),
        });
        fighter.situation = Situation::Air;
        assert_eq!(scene.board(&fighter, 1, &mut battle), None);
        fighter.situation = Situation::Ground;
        fighter.floor.as_mut().unwrap().flags = 0x800d;
        assert_eq!(scene.board(&fighter, 1, &mut battle), None);
        fighter.floor.as_mut().unwrap().flags = 0x800e;
        assert_eq!(scene.board(&fighter, 99, &mut battle), None);
        for i in 0..10 {
            assert_eq!(scene.board(&fighter, i + 1, &mut battle), Some(i as usize));
            assert_eq!(scene.board(&fighter, i + 1, &mut battle), None);
            assert_eq!(scene.tasks_remain, 9 - i as u8);
            assert_eq!(
                battle.end,
                if i == 9 {
                    Some(EndKind::Complete)
                } else {
                    None
                }
            );
        }
        assert_eq!(fighter.stocks, 3);
    }

    #[test]
    fn platforms_keep_selection_limits_and_source_width_boundaries() {
        assert_eq!(
            [750.0, 750.1, 1050.0, 1050.1].map(platform_kind),
            [0, 1, 1, 2]
        );
        let (s, _) = platform_course();
        let platforms: alloc::vec::Vec<_> = s.platforms.into_iter().flatten().collect();
        for k in 0..12 {
            for time in [2, TIMELIMIT_INFINITE] {
                let data = SceneData {
                    stage: Stage::Bonus2 as u8,
                    fkind: FighterKind::from_ordinal(k).unwrap(),
                    costume: 2,
                    player: 3,
                    time_limit: time,
                    ..Default::default()
                };
                let s = BonusStage::platforms(&data, &platforms);
                assert_eq!(s.state.gkind, 29 + k);
                assert_eq!(s.state.players[3].fkind, data.fkind);
                assert_eq!(s.state.players[3].costume, 2);
                assert_eq!(s.state.time_limit, time);
            }
        }
    }

    #[test]
    fn platform_timeout_keeps_partial_credit_and_wins_final_boarding_tie() {
        use crate::{fighter::Situation, ground::Standing};
        let (mut s, mut f) = platform_course();
        let mut b = s.battle();
        f.situation = Situation::Ground;
        f.floor = Some(Standing {
            line: 0,
            flags: 14,
            normal: ssb_engine::math::Vec2::new(0.0, 1.0),
        });
        for i in 1..10 {
            s.board(&f, i, &mut b);
        }
        for _ in 0..7262 {
            b.begin_frame();
        }
        assert_eq!(b.end, Some(EndKind::Failure));
        assert_eq!(s.tasks_remain, 1);
        s.board(&f, 10, &mut b);
        assert_eq!(b.end, Some(EndKind::Failure));
        assert_eq!(f.stocks, 3);
    }
}
