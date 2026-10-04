//! `sc1pstageclear.c`'s US score calculation. [`super::stage_clear`]
//! registers these values at the original presentation deadlines.
use super::{bonus::Bonus, BattleState, Difficulty, SceneData, Stage};

/// `dSC1PStageClearBonusData`, in bonus ID order (including unused IDs).
pub const BONUS_POINTS: [i32; 58] = [
    -99, 10000, 1000, 8000, 5000, 18000, 12000, 28000, 30000, 20000, 0, 11000, 0, 10000, 16000, 0,
    0, 60000, 30000, 5000, 15000, 5000, 70000, 70000, 400000, 40000, 80000, 15000, 12000, 9000,
    17000, 2000, 3500, 5000, 5000, 8000, 11000, 12000, 100, 7000, 10000, 1500, 10000, 3000, 4000,
    0, 0, 0, 8000, 9990, 3330, 50000, 25000, 25000, 10000, 50000, 8000, 25000,
];
pub const NO_MISS_MULTIPLIERS: [i32; 14] = [1, 2, 3, 0, 4, 5, 6, 0, 7, 8, 9, 0, 10, 11];

pub fn bonus_points(id: usize, stage: Stage, difficulty: Difficulty) -> i32 {
    let points = BONUS_POINTS[id];
    if id == Bonus::NoMiss as usize {
        points * NO_MISS_MULTIPLIERS[stage as usize]
    } else if id == Bonus::StageClear as usize {
        points * (difficulty as i32 + 1)
    } else {
        points
    }
}

/// Final ledger after timer, damage or targets, then bonus pages. The
/// battle already added KO points before requesting this scene.
pub fn score(scene: &SceneData, state: &BattleState, difficulty: Difficulty) -> u32 {
    let stage = scene.stage().expect("valid campaign stage");
    let multiplier = match stage {
        Stage::Bonus1 | Stage::Bonus2 => 200,
        Stage::Bonus3 => 500,
        _ => 50,
    };
    let mut total = i64::from(scene.score);
    match stage {
        Stage::Bonus1 | Stage::Bonus2 => {
            total += i64::from(scene.bonus_tasks_complete) * 1000;
            if scene.bonus_tasks_complete == 10
                && scene.time_limit != crate::battle::TIMELIMIT_INFINITE
            {
                total += i64::from(scene.time_remain) * multiplier;
            }
        }
        Stage::Bonus3 => total += i64::from(scene.time_remain) * multiplier,
        _ => {
            if scene.time_limit != crate::battle::TIMELIMIT_INFINITE {
                total += i64::from(scene.time_remain) * multiplier;
            }
            total += i64::from(state.players[scene.player as usize].total_damage_given) * 10
        }
    }
    let mut page_points = 0i64;
    let mut page_rows = 0;
    for id in 0..BONUS_POINTS.len() {
        if scene.bonus_get_mask[id / 32] & (1 << (id % 32)) != 0 {
            page_points += i64::from(bonus_points(id, stage, difficulty));
            page_rows += 1;
            if page_rows == 9 {
                total = (total + page_points).max(0);
                page_points = 0;
                page_rows = 0;
            }
        }
    }
    (total + page_points).max(0) as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn score_combines_us_timer_damage_no_miss_multiplier_and_penalty() {
        let mut scene = SceneData {
            stage: Stage::Mario as u8,
            score: 1000,
            time_remain: 10,
            ..Default::default()
        };
        let mut state = BattleState::default();
        state.players[0].total_damage_given = 50;
        Bonus::NoMiss.insert(&mut scene.bonus_get_mask);
        Bonus::CheapShot.insert(&mut scene.bonus_get_mask);
        assert_eq!(score(&scene, &state, Difficulty::Normal), 21901);
        assert_eq!(
            bonus_points(
                Bonus::StageClear as usize,
                Stage::Boss,
                Difficulty::VeryHard
            ),
            350000
        );
    }
    #[test]
    fn partial_targets_have_no_time_score_and_race_uses_five_hundred() {
        let mut s = SceneData {
            stage: Stage::Bonus1 as u8,
            time_remain: 20,
            bonus_tasks_complete: 9,
            ..Default::default()
        };
        assert_eq!(score(&s, &BattleState::default(), Difficulty::Normal), 9000);
        s.bonus_tasks_complete = 10;
        assert_eq!(
            score(&s, &BattleState::default(), Difficulty::Normal),
            14000
        );
        s.stage = Stage::Bonus3 as u8;
        assert_eq!(
            score(&s, &BattleState::default(), Difficulty::Normal),
            10000
        );
    }
    #[test]
    fn negative_bonus_page_floors_score_at_zero() {
        let mut s = SceneData::default();
        Bonus::CheapShot.insert(&mut s.bonus_get_mask);
        assert_eq!(score(&s, &BattleState::default(), Difficulty::Normal), 0);
    }
}
