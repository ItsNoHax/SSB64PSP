//! `sc1pmanager.c` (US): blocking overlay calls expressed as scene requests.
use super::*;
use crate::{costume::costume_common_id, rng::rand_int_range};

pub const KIRBY_MODEL_PARTS: [u8; 12] = [12, 7, 4, 8, 11, 10, 5, 9, 0, 6, 3, 13];
const CHALLENGERS: [FighterKind; 4] = [
    FighterKind::Luigi,
    FighterKind::Ness,
    FighterKind::Purin,
    FighterKind::Captain,
];
const NEWCOMERS: [Unlock; 4] = [Unlock::Luigi, Unlock::Ness, Unlock::Purin, Unlock::Captain];

/// An overlay the host must run to completion before calling `advance`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scene {
    Intro,
    Battle,
    BonusStage,
    Continue,
    StageClear,
    Ending,
    Staffroll,
    Congratulations,
    Challenger,
    Message,
    ModeMenu,
    Bonus1Select,
    Startup,
}

/// The fields kept by the manager across scene changes.
#[derive(Debug, Clone, PartialEq)]
pub struct Manager {
    pub scene: Scene,
    pub total_time_tics: u32,
    pub total_falls: u32,
    pub total_damage: u32,
    pub level_drop: i32,
    pub level_guard: u8,
    pub kirby_final_copy: FighterKind,
    pub kirby_model_part: u8,
}

/// `sc1PManagerGetShuffledFighterKind` / `GetShuffledKirbyCopy`.
pub fn shuffled_kind(mask: u16, previous: u16, random: i32) -> FighterKind {
    let eligible = mask & !previous;
    let ordinal = (0..12)
        .filter(|i| eligible & (1 << i) != 0)
        .nth(random as usize)
        .expect("random index must be within the eligible fighter mask");
    FighterKind::from_ordinal(ordinal).expect("playable fighter")
}

pub fn check_unlock_sound_test(backup: &Backup) -> bool {
    backup.unlock_mask & Unlock::SoundTest.mask() == 0
        && backup.spgame_records.iter().all(|r| {
            r.bonus1_task_count == BONUSGAME_TASK_MAX && r.bonus2_task_count == BONUSGAME_TASK_MAX
        })
}

/// `sc1PManagerTrySaveBackup`: completion and score are independent writes.
pub fn try_save_backup(backup: &mut Backup, scene: &SceneData, complete: bool) {
    let r = &mut backup.spgame_records[scene.fkind as usize];
    let mut write = false;
    if r.spgame_hiscore < scene.score {
        r.spgame_hiscore = scene.score;
        r.spgame_continues = scene.continues_used;
        r.spgame_total_bonuses = scene.bonus_count;
        r.spgame_best_difficulty = if complete {
            backup.spgame_difficulty as u8 + 1
        } else {
            0
        };
        write = true;
    }
    if !r.is_spgame_complete && complete {
        r.is_spgame_complete = true;
        write = true;
    }
    if write {
        backup.writes += 1;
    }
}

impl Manager {
    pub fn new(data: &mut SceneData, state: &mut BattleState, backup: &Backup) -> Self {
        if backup.error_flags & ERROR_1PGAME_MARIO != 0 {
            data.fkind = FighterKind::Mario;
            data.costume = 0;
        }
        let p = &mut state.players[usize::from(data.player)];
        *p = BattlePlayer {
            pkind: PlayerKind::Man,
            fkind: data.fkind,
            costume: data.costume,
            color: data.player,
            tag: Tag::port(data.player),
            stock_count: backup.spgame_stock_count,
            ..BattlePlayer::default()
        };
        data.score = 0;
        data.continues_used = 0;
        data.bonus_count = 0;
        let mut port = data.player;
        for ally in &mut data.ally_players {
            port = next_port(port);
            *ally = port;
        }
        let mut out = Self {
            scene: Scene::Intro,
            total_time_tics: 0,
            total_falls: 0,
            total_damage: 0,
            level_drop: 0,
            level_guard: 2,
            kirby_final_copy: FighterKind::Kirby,
            kirby_model_part: KIRBY_MODEL_PARTS[FighterKind::Kirby as usize],
        };
        if data.stage >= Stage::CHALLENGER_START {
            out.begin_challenger(data);
        } else {
            out.prepare_stage(data, state, backup);
        }
        out
    }

    fn prepare_stage(&mut self, data: &SceneData, state: &mut BattleState, backup: &Backup) {
        let mut mask = (backup.fighter_mask | CHARACTER_MASK_STARTER) & !(1 << data.fkind as u8);
        match data.stage() {
            Some(Stage::Mario | Stage::Donkey) => {
                let count = if data.stage() == Some(Stage::Mario) {
                    mask &= !1;
                    1
                } else {
                    2
                };
                let mut previous = 0;
                for i in 0..count {
                    let choices = (mask & !previous).count_ones() as i32;
                    let kind = shuffled_kind(mask, previous, rand_int_range(choices));
                    previous |= 1 << kind as u8;
                    let p = &mut state.players[usize::from(data.ally_players[i])];
                    p.fkind = kind;
                    p.costume = if data.stage() == Some(Stage::Mario) && kind == FighterKind::Luigi
                    {
                        costume_common_id(kind, 1)
                    } else {
                        0
                    };
                    p.shade = 0;
                }
            }
            Some(Stage::Kirby) => {
                mask = backup.fighter_mask | (1 << FighterKind::Kirby as u8);
                self.kirby_final_copy =
                    shuffled_kind(mask, 0, rand_int_range(mask.count_ones() as i32));
                self.kirby_model_part = KIRBY_MODEL_PARTS[self.kirby_final_copy as usize];
            }
            _ => {}
        }
    }

    fn begin_challenger(&mut self, data: &mut SceneData) {
        data.challenger_fkind = CHALLENGERS[usize::from(data.stage - Stage::CHALLENGER_START)];
        self.scene = Scene::Challenger;
    }

    fn after_challenger(&mut self, data: &mut SceneData, backup: &Backup) {
        if data.stage() == Some(Stage::Luigi) {
            self.scene = Scene::Bonus1Select;
            return;
        }
        let complete_mask = backup
            .spgame_records
            .iter()
            .enumerate()
            .fold(0u16, |mask, (i, r)| {
                mask | if r.is_spgame_complete { 1 << i } else { 0 }
            });
        if backup.unlock_mask & Unlock::Inishie.mask() == 0
            && backup.ground_mask & GROUND_MASK_ALL == GROUND_MASK_ALL
            && complete_mask & CHARACTER_MASK_STARTER == CHARACTER_MASK_STARTER
        {
            data.unlock_message = Some(Unlock::Inishie);
            self.scene = Scene::Message;
        } else {
            self.scene = Scene::Startup;
        }
    }

    fn next_stage(&mut self, data: &mut SceneData, state: &mut BattleState, backup: &mut Backup) {
        data.stage += 1;
        if data.stage <= Stage::COMMON_END {
            self.prepare_stage(data, state, backup);
            self.scene = Scene::Intro;
        } else {
            try_save_backup(backup, data, true);
            self.scene = Scene::Ending;
        }
    }

    /// Resume after the requested scene returns. Battle/bonus scenes must
    /// first copy their result into `data` and `state`; Continue supplies
    /// `is_continue`. Message owns the unlock/save operation, as in the source.
    pub fn advance(
        &mut self,
        data: &mut SceneData,
        state: &mut BattleState,
        backup: &mut Backup,
    ) -> Scene {
        match self.scene {
            Scene::Intro => {
                self.scene = if matches!(data.stage(), Some(Stage::Bonus1 | Stage::Bonus2)) {
                    Scene::BonusStage
                } else {
                    Scene::Battle
                }
            }
            Scene::Battle | Scene::BonusStage => {
                if data.is_reset {
                    self.scene = Scene::ModeMenu;
                } else if data.stage >= Stage::CHALLENGER_START {
                    if state.players[usize::from(data.player)].stock_count != -1
                        && state.time_remain != 0
                    {
                        data.challenger_level_drop = 0;
                        data.unlock_message =
                            Some(NEWCOMERS[usize::from(data.stage - Stage::CHALLENGER_START)]);
                        self.scene = Scene::Message;
                    } else {
                        if data.challenger_level_drop < 9 {
                            data.challenger_level_drop += 2;
                        }
                        self.after_challenger(data, backup);
                    }
                } else if self.scene == Scene::Battle
                    && data.stage() != Some(Stage::Bonus3)
                    && (state.players[usize::from(data.player)].stock_count == -1
                        || state.time_remain == 0)
                {
                    self.scene = Scene::Continue;
                } else {
                    self.level_drop = 0;
                    self.level_guard = 2;
                    data.bonus_count += data
                        .bonus_get_mask
                        .iter()
                        .map(|m| m.count_ones())
                        .sum::<u32>();
                    self.scene = Scene::StageClear;
                }
            }
            Scene::Continue => {
                if data.is_continue {
                    data.continues_used += 1;
                    state.players[usize::from(data.player)].stock_count = backup.spgame_stock_count;
                    self.level_guard -= 1;
                    if self.level_guard == 0 {
                        self.level_guard = 2;
                        self.level_drop = (self.level_drop + 1).min(9);
                    }
                    self.prepare_stage(data, state, backup);
                    self.scene = Scene::Intro;
                } else {
                    try_save_backup(backup, data, false);
                    self.scene = Scene::Startup;
                }
            }
            Scene::StageClear => self.next_stage(data, state, backup),
            Scene::Ending => self.scene = Scene::Staffroll,
            Scene::Staffroll => self.scene = Scene::Congratulations,
            Scene::Congratulations => {
                data.stage -= 1;
                data.stage = if backup.unlock_mask & Unlock::Ness.mask() == 0
                    && backup.spgame_difficulty as u8 >= Difficulty::Normal as u8
                    && data.continues_used == 0
                    && backup.spgame_stock_count < 3
                {
                    Stage::Ness as u8
                } else if backup.unlock_mask & Unlock::Captain.mask() == 0
                    && self.total_time_tics < 12 * 60 * 60
                {
                    Stage::Captain as u8
                } else if backup.unlock_mask & Unlock::Purin.mask() == 0 {
                    Stage::Purin as u8
                } else {
                    data.stage
                };
                if data.stage >= Stage::CHALLENGER_START {
                    self.begin_challenger(data);
                } else {
                    self.after_challenger(data, backup);
                }
            }
            Scene::Challenger => {
                state.players[usize::from(data.player)].stock_count = 0;
                self.scene = Scene::Battle;
            }
            Scene::Message => {
                if data.unlock_message == Some(Unlock::Inishie) {
                    self.scene = Scene::Startup;
                } else {
                    self.after_challenger(data, backup);
                }
            }
            Scene::ModeMenu | Scene::Bonus1Select | Scene::Startup => {}
        }
        self.scene
    }
}
