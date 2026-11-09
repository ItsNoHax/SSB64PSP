//! The files a 1P stage's battle loads, known before `sc1PGameFuncStart`
//! runs: the host reads them while the stage's intro plays (RE-501,
//! D-046). Pure: no RNG draw, no state change.

use super::{setup::STAGE_DESC, BattleState, SceneData, Stage};
use crate::fighter::FighterKind;
use alloc::vec::Vec;

/// Break the Targets' and Board the Platforms' first `gkind`; each
/// fighter has its own map (`sc1PBonusStageFuncStart`).
pub const BONUS1_GKIND: u8 = 17;
pub const BONUS2_GKIND: u8 = 29;

/// The `gkind` of stage `stage` for player kind `fkind`.
pub fn battle_gkind(stage: Stage, fkind: FighterKind) -> u8 {
    match stage {
        Stage::Bonus1 => BONUS1_GKIND + fkind as u8,
        Stage::Bonus2 => BONUS2_GKIND + fkind as u8,
        _ => STAGE_DESC[stage as usize].gkind,
    }
}

/// The next battle's `gkind` and the fighter kinds known before it
/// starts: the player, the allies `Manager::prepare_stage` drew, and the
/// stage's fixed opponents. The Fighting Polygon Team's and Race to the
/// Finish's Polygons are drawn at the battle's start
/// (`Game::setup_stage_all`) and are not among them; the Kirby Team's
/// copies are the host's extras.
pub fn battle_files(data: &SceneData, state: &BattleState) -> Option<(u8, Vec<FighterKind>)> {
    let stage = data.stage()?;
    let mut kinds = Vec::new();
    kinds.push(data.fkind);
    if matches!(stage, Stage::Bonus1 | Stage::Bonus2) {
        return Some((battle_gkind(stage, data.fkind), kinds));
    }
    let desc = STAGE_DESC[stage as usize];
    for &ally in data.ally_players.iter().take(usize::from(desc.ally_count)) {
        kinds.push(state.players[usize::from(ally)].fkind);
    }
    kinds.extend(desc.fkind.iter().flatten().copied());
    kinds.sort_unstable_by_key(|&k| k as u8);
    kinds.dedup();
    Some((desc.gkind, kinds))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spgame::{manager::Scene, session::Session, Backup};

    /// Every 1P battle's roster is among the predicted kinds, but for the
    /// Polygons drawn at its start; the prediction draws nothing.
    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn predicts_each_stage_roster() {
        for stage in Stage::ALL {
            if stage as u8 > Stage::COMMON_END || matches!(stage, Stage::Bonus1 | Stage::Bonus2) {
                continue;
            }
            for seed in 0..8u32 {
                let mut backup = Backup::default();
                let mut s = Session::new(
                    SceneData {
                        stage: stage as u8,
                        ..Default::default()
                    },
                    &backup,
                );
                crate::rng::set_seed(1 + seed as i32 * 7919);
                assert_eq!(
                    s.manager.advance(&mut s.data, &mut s.state, &mut backup),
                    Scene::Battle
                );
                let before = crate::rng::seed();
                let (gkind, kinds) = battle_files(&s.data, &s.state).expect("1P stage");
                assert_eq!(crate::rng::seed(), before, "{stage:?} drew");
                s.start_battle(&backup);
                assert_eq!(gkind, s.state.gkind, "{stage:?} gkind");
                for (_, p) in s.state.present() {
                    assert!(
                        kinds.contains(&p.fkind) || p.fkind.is_polygon(),
                        "{stage:?} seed {seed}: {:?} not in {kinds:?}",
                        p.fkind
                    );
                }
            }
        }
    }

    #[test]
    fn bonus_gkind_is_per_fighter() {
        assert_eq!(battle_gkind(Stage::Bonus1, FighterKind::Mario), 17);
        assert_eq!(battle_gkind(Stage::Bonus2, FighterKind::Ness), 29 + 11);
    }
}
