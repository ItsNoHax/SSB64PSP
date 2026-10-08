//! The 1P Game — `sc/sc1pmode/sc1pmanager.c` and `sc/sc1pmode/sc1pgame.c`.
//!
//! [`manager`] walks the stage sequence (`sc1PManagerUpdateScene`): the
//! allies it picks, continues and the CPU level they drop, the challenger
//! stages and the records it saves. [`setup`] builds each stage's battle
//! (`sc1PGameSetupStageAll`) and replaces a team's fallen members
//! (`sc1PGameSpawnEnemyTeamNext`). [`bonus`] keeps the stage's statistics
//! and awards its bonuses (`sc1PGameAppendBonusStats`). [`wait`] is the
//! stage's entry thread and its interface layout.
//!
//! The original's scene calls block until the scene returns; here the
//! manager is a state machine the host advances with each scene's result.
//! [`frontend`] owns the select and the intro, stage-clear and continue
//! controllers, and after the last stage the [`ending`] movie, the
//! [`staffroll`], the [`congra`] picture, the [`challenger`] warning and
//! the unlock [`message`]. [`bonus_stage`] owns Break the Targets' separate battle
//! and objective accounting; PSP drawing and scene orchestration live in
//! the host.

pub mod bonus;
pub mod bonus_stage;
pub mod boss;
pub mod challenger;
pub mod congra;
pub mod continue_scene;
pub mod ending;
pub mod frontend;
pub mod intro;
pub mod live;
pub mod manager;
pub mod message;
pub mod results;
pub mod select;
pub mod session;
pub mod setup;
pub mod staffroll;
pub mod stage_clear;
/// `FTStatusDesc.sflags`, generated at build time from the user's ROM by
/// `crates/ssb-tablegen` (never committed).
mod stat_flags {
    include!(concat!(env!("OUT_DIR"), "/stat_flags.rs"));
}
pub mod wait;

#[cfg(test)]
mod ending_tests;
#[cfg(test)]
mod frontend_tests;
#[cfg(test)]
mod live_tests;

use crate::fighter::FighterKind;

pub use setup::Game;

/// `SC1PGAME_STAGE_MAX_TEAM_COUNT`.
pub const MAX_TEAM_COUNT: usize = 30;
/// `SC1PGAME_STAGE_MAX_VARIATIONS_COUNT`.
pub const MAX_VARIATIONS_COUNT: usize = 12;
/// `SC1PGAME_STAGE_YOSHI_VARIATIONS_COUNT`.
pub const YOSHI_VARIATIONS_COUNT: usize = 6;
/// `SC1PGAME_STAGE_YOSHI_TEAM_COUNT`.
pub const YOSHI_TEAM_COUNT: u8 = 18;
/// `SC1PGAME_STAGE_KIRBY_VARIATIONS_COUNT`.
pub const KIRBY_VARIATIONS_COUNT: u8 = 7;
/// `SC1PGAME_STAGE_KIRBY_TEAM_COUNT`.
pub const KIRBY_TEAM_COUNT: u8 = 8;
/// `SC1PGAME_STAGE_KIRBY_SIM_COUNT`: Kirbys fought at once.
pub const KIRBY_SIM_COUNT: usize = 2;
/// `SC1PGAME_STAGE_MAX_OPPONENT_COUNT`: team members fought at once.
pub const MAX_OPPONENT_COUNT: usize = 3;
/// `SCBATTLE_BONUSGAME_TASK_MAX`.
pub const BONUSGAME_TASK_MAX: u8 = 10;
/// `GMCOMMON_PLAYERS_MAX`.
pub const PLAYERS_MAX: usize = 4;

/// `nSC1PGameStage*`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Stage {
    Link = 0,
    Yoshi,
    Fox,
    /// Break the Targets.
    Bonus1,
    Mario,
    Pikachu,
    Donkey,
    /// Board the Platforms.
    Bonus2,
    Kirby,
    Samus,
    MMario,
    /// Race to the Finish.
    Bonus3,
    /// The Fighting Polygon Team.
    Zako,
    /// Master Hand.
    Boss,
    Luigi,
    Ness,
    Purin,
    Captain,
}

impl Stage {
    /// `nSC1PGameStageCommonEnd`.
    pub const COMMON_END: u8 = Stage::Boss as u8;
    /// `nSC1PGameStageChallengerStart`.
    pub const CHALLENGER_START: u8 = Stage::Luigi as u8;

    pub const ALL: [Stage; 18] = [
        Stage::Link,
        Stage::Yoshi,
        Stage::Fox,
        Stage::Bonus1,
        Stage::Mario,
        Stage::Pikachu,
        Stage::Donkey,
        Stage::Bonus2,
        Stage::Kirby,
        Stage::Samus,
        Stage::MMario,
        Stage::Bonus3,
        Stage::Zako,
        Stage::Boss,
        Stage::Luigi,
        Stage::Ness,
        Stage::Purin,
        Stage::Captain,
    ];

    pub fn from_index(i: u8) -> Option<Stage> {
        Stage::ALL.get(usize::from(i)).copied()
    }

    /// The four Challenger Approaching stages.
    pub fn is_challenger(self) -> bool {
        self as u8 >= Self::CHALLENGER_START
    }
}

/// `nSC1PGameDifficulty*`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Difficulty {
    VeryEasy = 0,
    Easy,
    #[default]
    Normal,
    Hard,
    VeryHard,
}

/// `nLBBackupUnlock*`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unlock {
    Luigi = 0,
    Ness,
    Captain,
    Purin,
    Inishie,
    SoundTest,
    ItemSwitch,
}

impl Unlock {
    pub fn mask(self) -> u8 {
        1 << self as u8
    }
}

/// `nFTPlayerKind*` for a battle slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlayerKind {
    Man,
    Com,
    #[default]
    Not,
}

/// `nIFPlayerTagKind*`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tag {
    #[default]
    P1,
    P2,
    P3,
    P4,
    Cp,
    /// An ally's heart.
    Heart,
}

impl Tag {
    /// `tag = player` for a human port.
    pub fn port(player: u8) -> Tag {
        match player {
            0 => Tag::P1,
            1 => Tag::P2,
            2 => Tag::P3,
            _ => Tag::P4,
        }
    }
}

/// `nSCBattleTeamIDCom`: the enemies' team.
pub const TEAM_COM: u8 = 3;
/// `nSCBattlePlayerColorCP`.
pub const COLOR_CP: u8 = 4;

/// One `gSCManager1PGameBattleState.players` entry. The battle state lives
/// across the stages: the player's stocks carry from one to the next.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BattlePlayer {
    pub pkind: PlayerKind,
    pub fkind: FighterKind,
    pub costume: u8,
    pub shade: u8,
    pub color: u8,
    pub tag: Tag,
    pub team: u8,
    pub level: u8,
    pub handicap: u8,
    pub stock_count: i8,
    pub is_single_stockicon: bool,
    pub is_spgame_enemy: bool,
    // The battle statistics the bonuses read; the host keeps them
    // through the battle (`ftParamUpdatePlayerBattleStats` and the damage
    // setters).
    pub falls: u16,
    pub score: u16,
    pub total_damage_all: u32,
    /// `stock_damage_all`: the damage percent of the current stock.
    pub stock_damage_all: u32,
    pub total_damage_given: u32,
    pub total_damage_players: [u32; PLAYERS_MAX],
}

impl Default for BattlePlayer {
    fn default() -> Self {
        Self {
            pkind: PlayerKind::Not,
            fkind: FighterKind::Mario,
            costume: 0,
            shade: 0,
            color: 0,
            tag: Tag::P1,
            team: 0,
            level: 1,
            handicap: crate::stale::HANDICAP_DEFAULT,
            stock_count: 0,
            is_single_stockicon: false,
            is_spgame_enemy: false,
            falls: 0,
            score: 0,
            total_damage_all: 0,
            stock_damage_all: 0,
            total_damage_given: 0,
            total_damage_players: [0; PLAYERS_MAX],
        }
    }
}

/// `gSCManager1PGameBattleState`, the fields the 1P Game sets.
#[derive(Debug, Clone, PartialEq)]
pub struct BattleState {
    /// `GRKind`.
    pub gkind: u8,
    pub is_team_attack: bool,
    /// Minutes, or [`crate::battle::TIMELIMIT_INFINITE`].
    pub time_limit: u8,
    pub item_toggles: u32,
    pub item_appearance: crate::item::normal::Appearance,
    pub pl_count: u8,
    pub cp_count: u8,
    /// Ticks left when the stage ended (`time_remain`).
    pub time_remain: u32,
    /// Ticks the stage ran (`time_passed`).
    pub time_passed: u32,
    pub players: [BattlePlayer; PLAYERS_MAX],
}

impl Default for BattleState {
    fn default() -> Self {
        Self {
            gkind: 0,
            is_team_attack: false,
            time_limit: 5,
            item_toggles: !0,
            item_appearance: crate::item::normal::Appearance::Middle,
            pl_count: 0,
            cp_count: 0,
            time_remain: 0,
            time_passed: 0,
            players: [BattlePlayer::default(); PLAYERS_MAX],
        }
    }
}

impl BattleState {
    /// `ftParamInitPlayerBattleStats`'s reset of the totals.
    pub fn init_player_battle_stats(&mut self, player: u8) {
        let p = &mut self.players[usize::from(player) % PLAYERS_MAX];
        p.total_damage_given = 0;
        p.total_damage_all = 0;
        p.total_damage_players = [0; PLAYERS_MAX];
        p.falls = 0;
        p.score = 0;
    }

    /// `ftParamUpdateDamage`'s battle half: `percent` is the fighter's
    /// damage after the hit, capped at 999.
    pub fn update_damage(&mut self, player: u8, damage: u32, percent: u32) {
        let p = &mut self.players[usize::from(player) % PLAYERS_MAX];
        p.total_damage_all += damage;
        p.stock_damage_all = percent.min(999);
    }

    /// `ftParamUpdatePlayerBattleStats`: `attack_player` is
    /// [`PLAYERS_MAX`] for an attack no player owns.
    pub fn update_player_battle_stats(
        &mut self,
        attack_player: u8,
        defend_player: u8,
        damage: u32,
    ) {
        let (a, d) = (usize::from(attack_player), usize::from(defend_player));
        if a != PLAYERS_MAX && a != d && a < PLAYERS_MAX && d < PLAYERS_MAX {
            self.players[a].total_damage_given += damage;
            self.players[d].total_damage_players[a] += damage;
        }
    }

    /// The `pkind != nFTPlayerKindNot` slots.
    pub fn present(&self) -> impl Iterator<Item = (usize, &BattlePlayer)> {
        self.players
            .iter()
            .enumerate()
            .filter(|(_, p)| p.pkind != PlayerKind::Not)
    }

    /// The [`crate::battle::Player`]s the battle runs with.
    pub fn battle_players(&self) -> [crate::battle::Player; PLAYERS_MAX] {
        let mut out = [crate::battle::Player::default(); PLAYERS_MAX];
        for (i, p) in self.present() {
            out[i] = crate::battle::Player {
                present: true,
                is_human: p.pkind == PlayerKind::Man,
                team: p.team,
                stock_count: p.stock_count,
                ..crate::battle::Player::default()
            };
        }
        out
    }
}

pub use crate::backup::{
    Backup, Record, CHARACTER_MASK_ALL, CHARACTER_MASK_STARTER, ERROR_1PGAME_MARIO, GROUND_MASK_ALL,
};

/// The 1P Game's part of `gSCManagerSceneData`.
#[derive(Debug, Clone, PartialEq)]
pub struct SceneData {
    pub player: u8,
    pub fkind: FighterKind,
    pub costume: u8,
    /// `spgame_time_limit`: minutes, or 100 for infinite; 5 by default.
    pub time_limit: u8,
    /// `spgame_stage`, a [`Stage`] index. The manager steps it past the
    /// last stage and back, so it is kept as the original's byte.
    pub stage: u8,
    pub ally_players: [u8; 2],
    /// `spgame_time_remain`: seconds left when the stage ended.
    pub time_remain: u32,
    pub score: u32,
    pub continues_used: u32,
    pub bonus_count: u32,
    pub bonus_tasks_complete: u8,
    pub bonus_get_mask: [u32; 3],
    pub challenger_fkind: FighterKind,
    /// Levels the challenger's CPU drops after each loss.
    pub challenger_level_drop: u8,
    /// `unlock_messages[0]`.
    pub unlock_message: Option<Unlock>,
    pub is_reset: bool,
    pub is_continue: bool,
}

impl Default for SceneData {
    /// `dSCManagerDefaultSceneData`.
    fn default() -> Self {
        Self {
            player: 0,
            fkind: FighterKind::Mario,
            costume: 0,
            time_limit: 5,
            stage: 0,
            ally_players: [0, 0],
            time_remain: 0,
            score: 0,
            continues_used: 0,
            bonus_count: 0,
            bonus_tasks_complete: 0,
            bonus_get_mask: [0; 3],
            challenger_fkind: FighterKind::Luigi,
            challenger_level_drop: 0,
            unlock_message: None,
            is_reset: false,
            is_continue: false,
        }
    }
}

impl SceneData {
    pub fn stage(&self) -> Option<Stage> {
        Stage::from_index(self.stage)
    }
}

/// `sc1PGameGetNextFreePlayerPort`.
pub fn next_port(player: u8) -> u8 {
    if usize::from(player) == PLAYERS_MAX - 1 {
        0
    } else {
        player + 1
    }
}

#[cfg(test)]
mod tests;
