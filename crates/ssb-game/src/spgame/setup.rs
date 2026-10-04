//! Each 1P Game stage's battle — `sc1pgame.c`'s stage and computer
//! tables, `sc1PGameSetupStageAll`, `sc1PGameSpawnEnemyTeamNext` and
//! `sc1PGameSetPlayerDefeatStats`.
//!
//! The tables are the US build's (`REGION_US`).

use alloc::vec::Vec;
use ssb_engine::math::Vec3;

use super::bonus::{BonusCounters, DeadStatus, DefeatRecord, EndStatus};
use super::{
    BattleState, Difficulty, PlayerKind, SceneData, Stage, Tag, COLOR_CP, KIRBY_SIM_COUNT,
    KIRBY_VARIATIONS_COUNT, MAX_OPPONENT_COUNT, MAX_TEAM_COUNT, MAX_VARIATIONS_COUNT, PLAYERS_MAX,
    TEAM_COM, YOSHI_VARIATIONS_COUNT,
};
use crate::battle::{Battle, GameStatus, TIMELIMIT_INFINITE};
use crate::computer::attack::Trait;
use crate::costume::costume_common_id;
use crate::fighter::{Facing, FighterKind};
use crate::item::normal::Appearance;
use crate::rng::rand_int_range;

/// `nMPMapObjKind1PGame*`: the start points each slot reads.
pub mod mapobj {
    pub const PLAYER: u16 = 0x21;
    pub const ALLY_START: u16 = 0x22;
    pub const ENEMY_START: u16 = 0x25;
    pub const BONUS3_TARU_BOMB: u16 = 0x29;
    /// Where a team's next member drops in.
    pub const ENEMY_TEAM: u16 = 0x2B;
    pub const CHALLENGER_PLAYER: u16 = 0x2C;
    pub const CHALLENGER_ENEMY_START: u16 = 0x2D;
}

/// `GRKind`s the stage table names.
pub mod gkind {
    pub const CASTLE: u8 = 0;
    pub const SECTOR: u8 = 1;
    pub const JUNGLE: u8 = 2;
    pub const ZEBES: u8 = 3;
    pub const HYRULE: u8 = 4;
    pub const PUPUPU: u8 = 6;
    pub const YAMABUKI: u8 = 7;
    pub const YOSTER_SMALL: u8 = 12;
    pub const METAL: u8 = 13;
    pub const ZAKO: u8 = 14;
    pub const BONUS3: u8 = 15;
    pub const LAST: u8 = 16;
    /// `nGRKindBonusStageStart`.
    pub const BONUS_STAGE_START: u8 = 17;
}

/// One `SC1PGameComputer`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComputerDesc {
    pub is_team_attack: bool,
    pub item_appearance: Appearance,
    pub enemy_level: [u8; 5],
    pub enemy_handicap: [u8; 5],
    pub ally_level: [u8; 5],
    pub ally_handicap: [u8; 5],
}

const fn com(
    is_team_attack: bool,
    item_appearance: Appearance,
    enemy_level: [u8; 5],
    enemy_handicap: [u8; 5],
    ally_level: [u8; 5],
    ally_handicap: [u8; 5],
) -> ComputerDesc {
    ComputerDesc {
        is_team_attack,
        item_appearance,
        enemy_level,
        enemy_handicap,
        ally_level,
        ally_handicap,
    }
}

const ALLY_NONE: [u8; 5] = [1, 1, 1, 1, 1];
const HANDICAP_9: [u8; 5] = [9, 9, 9, 9, 9];
const CHALLENGER_LEVEL: [u8; 5] = [6, 7, 7, 8, 9];
const CHALLENGER_HANDICAP: [u8; 5] = [6, 6, 6, 6, 6];

/// `dSC1PGameComputerDesc`, by [`Stage`].
pub const COMPUTER_DESC: [ComputerDesc; 18] = [
    com(
        true,
        Appearance::Low,
        [1, 2, 3, 6, 8],
        HANDICAP_9,
        ALLY_NONE,
        HANDICAP_9,
    ),
    com(
        true,
        Appearance::Middle,
        [1, 2, 4, 6, 8],
        [10, 11, 12, 13, 14],
        ALLY_NONE,
        HANDICAP_9,
    ),
    com(
        true,
        Appearance::Middle,
        [2, 3, 5, 7, 9],
        HANDICAP_9,
        ALLY_NONE,
        HANDICAP_9,
    ),
    com(
        true,
        Appearance::None,
        ALLY_NONE,
        HANDICAP_9,
        ALLY_NONE,
        HANDICAP_9,
    ),
    com(
        false,
        Appearance::Middle,
        [2, 3, 5, 7, 9],
        HANDICAP_9,
        [5, 5, 5, 4, 2],
        HANDICAP_9,
    ),
    com(
        true,
        Appearance::Middle,
        [3, 4, 5, 7, 9],
        HANDICAP_9,
        ALLY_NONE,
        HANDICAP_9,
    ),
    com(
        false,
        Appearance::High,
        [2, 4, 6, 7, 8],
        [25, 26, 27, 28, 29],
        [4, 4, 4, 3, 2],
        [7, 7, 7, 7, 7],
    ),
    com(
        true,
        Appearance::None,
        ALLY_NONE,
        HANDICAP_9,
        ALLY_NONE,
        HANDICAP_9,
    ),
    com(
        true,
        Appearance::Middle,
        [3, 4, 5, 6, 7],
        [15, 16, 17, 18, 19],
        ALLY_NONE,
        HANDICAP_9,
    ),
    com(
        true,
        Appearance::Middle,
        [5, 6, 8, 9, 9],
        [9, 9, 9, 9, 40],
        ALLY_NONE,
        HANDICAP_9,
    ),
    com(
        true,
        Appearance::VeryLow,
        [1, 3, 4, 6, 8],
        [30, 31, 32, 33, 34],
        ALLY_NONE,
        HANDICAP_9,
    ),
    com(
        true,
        Appearance::None,
        [6, 8, 9, 9, 9],
        [1, 3, 5, 7, 9],
        ALLY_NONE,
        HANDICAP_9,
    ),
    com(
        true,
        Appearance::High,
        [2, 3, 4, 5, 7],
        [20, 21, 22, 23, 24],
        ALLY_NONE,
        HANDICAP_9,
    ),
    com(
        true,
        Appearance::None,
        [1, 2, 3, 4, 5],
        [35, 36, 37, 38, 39],
        ALLY_NONE,
        HANDICAP_9,
    ),
    com(
        true,
        Appearance::None,
        CHALLENGER_LEVEL,
        CHALLENGER_HANDICAP,
        ALLY_NONE,
        HANDICAP_9,
    ),
    com(
        true,
        Appearance::None,
        CHALLENGER_LEVEL,
        CHALLENGER_HANDICAP,
        ALLY_NONE,
        HANDICAP_9,
    ),
    com(
        true,
        Appearance::None,
        CHALLENGER_LEVEL,
        CHALLENGER_HANDICAP,
        ALLY_NONE,
        HANDICAP_9,
    ),
    com(
        true,
        Appearance::None,
        CHALLENGER_LEVEL,
        CHALLENGER_HANDICAP,
        ALLY_NONE,
        HANDICAP_9,
    ),
];

/// One `SC1PGameStage`. Every stage's item toggles are `0xFFFFFFFF`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StageDesc {
    pub screenflash_alpha: u8,
    pub gkind: u8,
    pub opponent_count: u8,
    pub fkind: [Option<FighterKind>; 2],
    pub opponent_trait: Trait,
    pub ally_count: u8,
    pub ally_trait: Trait,
}

const fn stage(
    screenflash_alpha: u8,
    gkind: u8,
    opponent_count: u8,
    fkind: [Option<FighterKind>; 2],
    opponent_trait: Trait,
    ally_count: u8,
    ally_trait: Trait,
) -> StageDesc {
    StageDesc {
        screenflash_alpha,
        gkind,
        opponent_count,
        fkind,
        opponent_trait,
        ally_count,
        ally_trait,
    }
}

use FighterKind as K;
use Trait as T;

/// `dSC1PGameStageDesc`, by [`Stage`].
pub const STAGE_DESC: [StageDesc; 18] = [
    stage(
        0xFF,
        gkind::HYRULE,
        1,
        [Some(K::Link), None],
        T::Link,
        0,
        T::Default,
    ),
    stage(
        0x80,
        gkind::YOSTER_SMALL,
        super::YOSHI_TEAM_COUNT,
        [Some(K::Yoshi), None],
        T::YoshiTeam,
        0,
        T::Default,
    ),
    stage(
        0xFF,
        gkind::SECTOR,
        1,
        [Some(K::Fox), None],
        T::Default,
        0,
        T::Default,
    ),
    stage(
        0xFF,
        gkind::CASTLE,
        1,
        [None, None],
        T::Default,
        0,
        T::Default,
    ),
    stage(
        0xFF,
        gkind::CASTLE,
        2,
        [Some(K::Mario), Some(K::Luigi)],
        T::MarioBros,
        1,
        T::Ally,
    ),
    stage(
        0xFF,
        gkind::YAMABUKI,
        1,
        [Some(K::Pikachu), None],
        T::Default,
        0,
        T::Default,
    ),
    stage(
        0xFF,
        gkind::JUNGLE,
        1,
        [Some(K::GiantDonkey), None],
        T::GiantDonkey,
        2,
        T::Ally,
    ),
    stage(
        0xFF,
        gkind::CASTLE,
        1,
        [None, None],
        T::Default,
        0,
        T::Default,
    ),
    stage(
        0x80,
        gkind::PUPUPU,
        super::KIRBY_TEAM_COUNT,
        [Some(K::Kirby), None],
        T::KirbyTeam,
        0,
        T::Default,
    ),
    stage(
        0xFF,
        gkind::ZEBES,
        1,
        [Some(K::Samus), None],
        T::Default,
        0,
        T::Default,
    ),
    stage(
        0xFF,
        gkind::METAL,
        1,
        [Some(K::MetalMario), None],
        T::Default,
        0,
        T::Default,
    ),
    stage(
        0xFF,
        gkind::BONUS3,
        3,
        [None, None],
        T::Bonus3,
        0,
        T::Default,
    ),
    stage(
        0x80,
        gkind::ZAKO,
        MAX_TEAM_COUNT as u8,
        [None, None],
        T::PolyTeam,
        0,
        T::Default,
    ),
    stage(
        0xFF,
        gkind::LAST,
        1,
        [Some(K::Boss), None],
        T::Default,
        0,
        T::Default,
    ),
    stage(
        0xFF,
        gkind::CASTLE,
        1,
        [Some(K::Luigi), None],
        T::Default,
        0,
        T::Default,
    ),
    stage(
        0xFF,
        gkind::PUPUPU,
        1,
        [Some(K::Ness), None],
        T::Default,
        0,
        T::Default,
    ),
    stage(
        0xFF,
        gkind::YAMABUKI,
        1,
        [Some(K::Purin), None],
        T::Default,
        0,
        T::Default,
    ),
    stage(
        0xFF,
        gkind::ZEBES,
        1,
        [Some(K::Captain), None],
        T::Default,
        0,
        T::Default,
    ),
];

/// `dSC1PGameKirbyTeamCopyKinds`: the copy each Kirby of the team wears,
/// in the order they appear.
pub const KIRBY_TEAM_COPY_KINDS: [FighterKind; 7] = [
    K::Mario,
    K::Donkey,
    K::Link,
    K::Samus,
    K::Yoshi,
    K::Fox,
    K::Pikachu,
];

/// `nFTKindNStart`: the Fighting Polygon Team's first kind.
const POLY_START: u8 = FighterKind::PolyMario as u8;

/// One `sSC1PGamePlayerSetups` entry: how a slot spawns.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerSetup {
    /// Which `nMPMapObjKind1PGame*` point the fighter starts on.
    pub mapobj_kind: u16,
    pub cp_trait: Trait,
    pub team_order: u8,
    /// Kirby's copy (`nFTKindKirby` for none).
    pub copy_kind: FighterKind,
    pub is_skip_entry: bool,
    pub is_magnify_ignore: bool,
    /// Multiplies the fighter's `camera_zoom_frame`.
    pub camera_frame_mul: f32,
    /// The Polygon Team shares one figatree heap sized for the largest.
    pub shared_figatree: bool,
}

impl Default for PlayerSetup {
    fn default() -> Self {
        Self {
            mapobj_kind: 0,
            cp_trait: Trait::Default,
            team_order: 0,
            copy_kind: FighterKind::Kirby,
            is_skip_entry: false,
            is_magnify_ignore: false,
            camera_frame_mul: 1.0,
            shared_figatree: false,
        }
    }
}

/// `sc1PGameGetFighterKindsNum`: the `index`-th kind whose bit is clear in
/// `used`.
pub fn free_index(used: u16, index: i32) -> u8 {
    let mut i: i32 = -1;
    let mut left = index + 1;
    loop {
        i += 1;
        if used & (1 << i) == 0 {
            left -= 1;
        }
        if left == 0 || i >= 15 {
            return i as u8;
        }
    }
}

/// `sc1PGameGetNextFreeCostume`: the first `ftParamGetCostumeCommonID`
/// costume no other present slot of the same kind wears.
pub fn next_free_costume(state: &BattleState, com: usize) -> u8 {
    let kind = state.players[com].fkind;
    let mut used = 0;
    'search: loop {
        let costume = costume_common_id(kind, used);
        for (i, p) in state.players.iter().enumerate() {
            if i == com || p.pkind == PlayerKind::Not {
                continue;
            }
            if p.fkind == kind && p.costume == costume && used < 4 {
                used += 1;
                continue 'search;
            }
        }
        return costume;
    }
}

/// A stage's 1P Game state: `sc1pgame.c`'s statics for one stage.
#[derive(Debug, Clone, PartialEq)]
pub struct Game {
    pub stage: Stage,
    pub setups: [PlayerSetup; PLAYERS_MAX],
    /// `sSC1PGameEnemyPlayerCount`.
    pub enemy_player_count: u8,
    /// `sSC1PGameEnemyStocksRemaining`.
    pub enemy_stocks_remaining: u8,
    /// `sSC1PGameEnemyStockSpriteFlags`: the team members KO'd, by order.
    pub enemy_stock_sprite_flags: u32,
    /// `sSC1PGameTeamPlayersRemaining`: members not yet on the stage.
    pub team_players_remaining: u8,
    /// `sSC1PGameCurrentEnemyVariation`: the next member's order.
    pub current_variation: u8,
    /// `sSC1PGameEnemyVariations`: Yoshi's costumes or the Polygons'
    /// kinds, by team order.
    pub variations: [u8; MAX_TEAM_COUNT],
    /// `sSC1PGameEnemyKirbyCostume`.
    pub kirby_costume: u8,
    /// `gSC1PManagerKirbyTeamFinalCopy`: the eighth Kirby's copy.
    pub kirby_final_copy: FighterKind,
    /// `sSC1PGameIsStartStage`.
    pub is_start: bool,
    /// `sSC1PGameIsEndStage`.
    pub is_end: bool,
    pub bonus: BonusCounters,
}

/// The enemy `sc1PGameSpawnEnemyTeamNext` drops in.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EnemySpawn {
    pub player: u8,
    pub fkind: FighterKind,
    pub costume: u8,
    pub shade: u8,
    pub copy_kind: FighterKind,
    pub team_order: u8,
    pub pos: Vec3,
    pub facing: Facing,
    /// `nFTPartsDetailHigh` with fewer than three fighters.
    pub detail_high: bool,
    pub level: u8,
    pub handicap: u8,
    pub cp_trait: Trait,
    pub is_magnify_ignore: bool,
    pub camera_frame_mul: f32,
}

/// What a fallen 1P Game enemy becomes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NextEnemy {
    /// No member is left: `ftCommonSleepSetStatus`.
    Sleep,
    /// The fighter is destroyed and the next member made in its slot,
    /// skipping its entry and unlocked at once.
    Spawn(EnemySpawn),
}

/// `sc1PGameSetupEnemyPlayer`.
#[allow(clippy::too_many_arguments)]
fn setup_enemy_player(
    game: &mut Game,
    state: &mut BattleState,
    desc: &StageDesc,
    com: &ComputerDesc,
    difficulty: Difficulty,
    level_drop: i32,
    player: u8,
    enemy_num: usize,
) {
    let d = difficulty as usize;
    let level = i32::from(com.enemy_level[d]) - level_drop;
    let p = &mut state.players[usize::from(player)];
    p.level = level.max(1) as u8;
    p.handicap = com.enemy_handicap[d];
    // The kind of a team stage's members is set by the caller.
    if let Some(kind) = desc.fkind[enemy_num] {
        p.fkind = kind;
    }
    p.team = TEAM_COM;
    p.costume = 0;
    p.shade = 0;
    p.color = COLOR_CP;
    p.tag = Tag::Cp;
    p.is_single_stockicon = true;
    p.stock_count = 0;
    p.is_spgame_enemy = true;
    p.pkind = PlayerKind::Com;
    game.setups[usize::from(player)].cp_trait = desc.opponent_trait;
    game.team_players_remaining = game.team_players_remaining.wrapping_sub(1);
}

/// `sc1PGameGetRandomStartPosition`'s pick.
fn random_point(points: &[Vec3]) -> Vec3 {
    if points.is_empty() {
        return Vec3::default();
    }
    let n = points.len().min(10);
    points[rand_int_range(n as i32) as usize]
}

impl Game {
    /// `sc1PGameSetupStageAll`: the stage's battle state and each slot's
    /// spawn. `level_drop` is `gSC1PManagerLevelDrop`; `kirby_final_copy`
    /// is the manager's pick for the Kirby Team's last member.
    pub fn setup_stage_all(
        state: &mut BattleState,
        scene: &mut SceneData,
        difficulty: Difficulty,
        level_drop: i32,
        kirby_final_copy: FighterKind,
    ) -> Game {
        let stage = scene.stage().unwrap_or(Stage::Link);
        let desc = STAGE_DESC[stage as usize];
        let com = COMPUTER_DESC[stage as usize];
        let d = difficulty as usize;

        scene.is_reset = false;
        state.gkind = desc.gkind;
        state.is_team_attack = com.is_team_attack;
        state.time_limit = match stage {
            Stage::Bonus3 => 1,
            Stage::Luigi | Stage::Ness | Stage::Purin | Stage::Captain => TIMELIMIT_INFINITE,
            _ => scene.time_limit,
        };
        state.item_toggles = !0;
        state.item_appearance = com.item_appearance;

        let mut game = Game {
            stage,
            setups: [PlayerSetup::default(); PLAYERS_MAX],
            enemy_player_count: desc.opponent_count,
            enemy_stocks_remaining: desc.opponent_count,
            enemy_stock_sprite_flags: 0,
            team_players_remaining: desc.opponent_count,
            current_variation: 0,
            variations: [0; MAX_TEAM_COUNT],
            kirby_costume: 0,
            kirby_final_copy,
            is_start: false,
            is_end: false,
            bonus: BonusCounters::default(),
        };
        let me = usize::from(scene.player);
        for (i, p) in state.players.iter_mut().enumerate() {
            if i != me {
                p.pkind = PlayerKind::Not;
            }
        }
        if stage as u8 <= Stage::COMMON_END {
            state.players[me].is_single_stockicon = false;
            game.setups[me].mapobj_kind = mapobj::PLAYER;
        } else {
            state.players[me].is_single_stockicon = true;
            game.setups[me].mapobj_kind = mapobj::CHALLENGER_PLAYER;
        }
        game.setups[me].is_skip_entry = matches!(stage, Stage::Boss | Stage::Bonus3);

        let mut player = scene.player;
        match stage {
            Stage::Yoshi | Stage::Zako | Stage::Kirby | Stage::Bonus3 => {
                game.setup_team(state, scene, &desc, &com, difficulty, level_drop, player)
            }
            Stage::Bonus1 | Stage::Bonus2 => {}
            _ => {
                for i in 0..usize::from(desc.ally_count) {
                    let ally = scene.ally_players[i];
                    let a = usize::from(ally);
                    let p = &mut state.players[a];
                    p.level = com.ally_level[d];
                    p.handicap = com.ally_handicap[d];
                    p.team = 0;
                    p.color = scene.player;
                    p.tag = Tag::Heart;
                    p.is_single_stockicon = true;
                    p.stock_count = 0;
                    p.is_spgame_enemy = false;
                    p.pkind = PlayerKind::Com;
                    player = ally;
                    game.setups[a].mapobj_kind = mapobj::ALLY_START + i as u16;
                    game.setups[a].cp_trait = desc.ally_trait;
                }
                for i in 0..usize::from(desc.opponent_count) {
                    player = super::next_port(player);
                    let p = usize::from(player);
                    setup_enemy_player(
                        &mut game, state, &desc, &com, difficulty, level_drop, player, i,
                    );
                    state.players[p].costume = next_free_costume(state, p);
                    if stage as u8 <= Stage::COMMON_END {
                        game.setups[p].mapobj_kind = mapobj::ENEMY_START + i as u16;
                    } else {
                        game.setups[p].mapobj_kind = mapobj::CHALLENGER_ENEMY_START + i as u16;
                        let level = i32::from(state.players[p].level)
                            - i32::from(scene.challenger_level_drop);
                        state.players[p].level = level.max(1) as u8;
                    }
                    if stage == Stage::Boss {
                        game.setups[p].is_magnify_ignore = true;
                    }
                }
            }
        }
        state.pl_count = 0;
        state.cp_count = 0;
        for p in &state.players {
            match p.pkind {
                PlayerKind::Man => state.pl_count += 1,
                PlayerKind::Com => state.cp_count += 1,
                PlayerKind::Not => {}
            }
        }
        game
    }

    /// The team stages' half of `sc1PGameSetupStageAll`.
    #[allow(clippy::too_many_arguments)]
    fn setup_team(
        &mut self,
        state: &mut BattleState,
        scene: &SceneData,
        desc: &StageDesc,
        com: &ComputerDesc,
        difficulty: Difficulty,
        level_drop: i32,
        mut player: u8,
    ) {
        let opponents = usize::from(desc.opponent_count);
        // The shuffled costumes or kinds, then the order repeated.
        let (unique, base) = match self.stage {
            Stage::Yoshi => (YOSHI_VARIATIONS_COUNT, 0),
            Stage::Zako => (MAX_VARIATIONS_COUNT, POLY_START),
            // Race to the Finish draws three of the twelve Polygons.
            Stage::Bonus3 => (MAX_OPPONENT_COUNT, POLY_START),
            _ => (0, 0),
        };
        if unique != 0 {
            let mut left = if self.stage == Stage::Yoshi {
                YOSHI_VARIATIONS_COUNT
            } else {
                MAX_VARIATIONS_COUNT
            } as i32;
            let mut used: u16 = 0;
            for i in 0..unique {
                let v = free_index(used, rand_int_range(left));
                used |= 1 << v;
                self.variations[i] = v + base;
                left -= 1;
            }
            if self.stage != Stage::Bonus3 {
                let mut k = 0;
                for i in unique..opponents {
                    self.variations[i] = self.variations[k];
                    k = if k == unique - 1 { 0 } else { k + 1 };
                }
            }
        }
        self.current_variation = 0;
        let at_once = if self.stage == Stage::Kirby {
            KIRBY_SIM_COUNT
        } else {
            MAX_OPPONENT_COUNT
        };
        for i in 0..at_once {
            player = super::next_port(player);
            let p = usize::from(player);
            setup_enemy_player(self, state, desc, com, difficulty, level_drop, player, 0);
            let v = self.variations[usize::from(self.current_variation)];
            let s = &mut self.setups[p];
            s.mapobj_kind = mapobj::ENEMY_START + i as u16;
            match self.stage {
                Stage::Yoshi => {
                    state.players[p].costume = v;
                    state.players[p].shade =
                        u8::from(scene.fkind == FighterKind::Yoshi && scene.costume == v);
                }
                Stage::Zako | Stage::Bonus3 => {
                    state.players[p].fkind =
                        FighterKind::from_ordinal(v).unwrap_or(FighterKind::PolyMario);
                }
                Stage::Kirby => {
                    // The enemy's costume was just set to 0.
                    let costume = if scene.fkind == FighterKind::Kirby && scene.costume == 0 {
                        costume_common_id(FighterKind::Kirby, 1)
                    } else {
                        0
                    };
                    state.players[p].costume = costume;
                    self.kirby_costume = costume;
                    s.copy_kind = KIRBY_TEAM_COPY_KINDS[usize::from(self.current_variation)];
                }
                _ => {}
            }
            if self.stage == Stage::Bonus3 {
                s.is_magnify_ignore = true;
                s.is_skip_entry = true;
            } else {
                s.team_order = self.current_variation;
                s.camera_frame_mul = 0.3;
                s.is_magnify_ignore = true;
            }
            if self.stage == Stage::Zako {
                s.shared_figatree = true;
            }
            self.current_variation += 1;
        }
    }

    /// `sc1PGameGetEnemyStartLR`: a fighter starts facing the nearest
    /// start point of another team, right when there is none.
    /// `start_x(player)` is the x of the slot's `mapobj_kind` point.
    pub fn start_facing(
        &self,
        state: &BattleState,
        this: usize,
        start_x: impl Fn(usize) -> f32,
    ) -> Facing {
        let me = start_x(this);
        let others = state
            .present()
            .filter(|&(i, p)| i != this && p.team != state.players[this].team)
            .map(|(i, _)| start_x(i));
        crate::battle::start_facing(me, others.collect::<Vec<_>>().into_iter())
    }

    /// `sc1PGameSpawnEnemyTeamNext`, at the end of a fallen enemy's
    /// `Dead*` wait. `team_points` are the stage's
    /// `nMPMapObjKind1PGameEnemyTeam` points; the member drops in halfway
    /// between `camera_bound_top` and `map_bound_top`.
    pub fn spawn_enemy_team_next(
        &mut self,
        state: &mut BattleState,
        scene: &SceneData,
        player: u8,
        team_points: &[Vec3],
        camera_bound_top: f32,
        map_bound_top: f32,
    ) -> NextEnemy {
        if self.team_players_remaining == 0 {
            return NextEnemy::Sleep;
        }
        self.team_players_remaining -= 1;
        let p = usize::from(player) % PLAYERS_MAX;
        state.players[p].stock_count = 0;
        let v = self.variations[usize::from(self.current_variation).min(MAX_TEAM_COUNT - 1)];
        match self.stage {
            Stage::Yoshi => {
                state.players[p].costume = v;
                state.players[p].shade =
                    u8::from(scene.fkind == FighterKind::Yoshi && scene.costume == v);
            }
            Stage::Zako => {
                state.players[p].fkind =
                    FighterKind::from_ordinal(v).unwrap_or(FighterKind::PolyMario);
            }
            Stage::Kirby => {
                self.setups[p].copy_kind = if self.current_variation == KIRBY_VARIATIONS_COUNT {
                    self.kirby_final_copy
                } else {
                    KIRBY_TEAM_COPY_KINDS[usize::from(self.current_variation).min(6)]
                };
            }
            _ => {}
        }
        self.setups[p].team_order = self.current_variation;
        self.current_variation += 1;

        let mut pos = random_point(team_points);
        pos.y = (camera_bound_top + map_bound_top) * 0.5;
        let s = self.setups[p];
        let b = state.players[p];
        NextEnemy::Spawn(EnemySpawn {
            player,
            fkind: b.fkind,
            costume: b.costume,
            shade: b.shade,
            copy_kind: s.copy_kind,
            team_order: s.team_order,
            pos,
            facing: if pos.x >= 0.0 {
                Facing::Left
            } else {
                Facing::Right
            },
            detail_high: state.pl_count + state.cp_count < 3,
            level: b.level,
            handicap: b.handicap,
            cp_trait: s.cp_trait,
            is_magnify_ignore: s.is_magnify_ignore,
            camera_frame_mul: s.camera_frame_mul,
        })
    }

    /// `sc1PGameSetPlayerDefeatStats`, after the battle took the stock
    /// ([`Battle::on_fall`]). The player's last stock or the enemies'
    /// last ends the stage.
    pub fn set_player_defeat_stats(
        &mut self,
        battle: &mut Battle,
        state: &BattleState,
        scene: &SceneData,
        player: u8,
        record: DefeatRecord,
        fkind: FighterKind,
    ) {
        let p = usize::from(player) % PLAYERS_MAX;
        let me = usize::from(scene.player);
        if p == me && battle.players[p].stock_count == -1 && battle.status != GameStatus::End {
            battle.announce_end_1p(false);
        } else if state.players[p].is_spgame_enemy {
            self.enemy_stocks_remaining = self.enemy_stocks_remaining.saturating_sub(1);
            self.enemy_stock_sprite_flags |=
                1u32.checked_shl(u32::from(record.team_order)).unwrap_or(0);
            self.bonus.push_defeat(record);

            // Bros. Calamity: Luigi KO'd by the player before the player
            // dealt the slot before Luigi (Mario) any damage.
            if self.stage == Stage::Mario
                && fkind == FighterKind::Luigi
                && self.enemy_stocks_remaining != 0
                && record.damage_player == Some(scene.player)
            {
                let prev = if p == 0 { PLAYERS_MAX - 1 } else { p - 1 };
                if state.players[prev].total_damage_players[me] == 0 {
                    self.bonus.bros_calamity = true;
                }
            }
            if self.enemy_stocks_remaining == 0 {
                battle.announce_end_1p(battle.players[me].stock_count != -1);
            }
        }
    }

    /// `sc1PGameFuncUpdate`'s two checks. Returns whether
    /// `mpCollisionSetPlayBGM` starts the music on "Go" (Meta Crystal and
    /// the Polygon Team wait for it). `end` is the player's status and
    /// star time, read once the stage sets.
    pub fn update(&mut self, status: GameStatus, end: impl FnOnce() -> (EndStatus, u32)) -> bool {
        let mut play_bgm = false;
        if !self.is_start && status == GameStatus::Go {
            self.is_start = true;
            play_bgm = matches!(self.stage, Stage::MMario | Stage::Zako);
        }
        if !self.is_end && status == GameStatus::Set {
            self.is_end = true;
            let (s, star) = end();
            self.bonus.end_player_status = Some(s);
            self.bonus.end_invincible_tics = star;
        }
        play_bgm
    }

    /// `sc1PGameTeamStockDisplayProcDisplay`'s layout: the team's members
    /// not yet KO'd, highest order first, ten to a row from (20, 20). Each
    /// icon's palette is its Yoshi costume or the Kirbys' costume; the
    /// Polygons share one sprite. Nothing draws once the team is gone.
    pub fn team_stock_icons(&self) -> Vec<StockIcon> {
        if !matches!(self.stage, Stage::Yoshi | Stage::Kirby | Stage::Zako)
            || self.enemy_stocks_remaining == 0
        {
            return Vec::new();
        }
        let mut out = Vec::new();
        let (mut ix, mut iy) = (0, 0);
        for n in (0..usize::from(self.enemy_player_count)).rev() {
            if self.enemy_stock_sprite_flags & (1 << n) != 0 {
                continue;
            }
            out.push(StockIcon {
                x: ix * 10 + 20,
                y: iy * 10 + 20,
                costume: match self.stage {
                    Stage::Yoshi => Some(self.variations[n]),
                    Stage::Kirby => Some(self.kirby_costume),
                    _ => None,
                },
            });
            ix += 1;
            if ix >= 10 {
                ix = 0;
                iy += 1;
            }
        }
        out
    }

    /// `sc1PGameInitTimeUpMessage`: Race to the Finish's timer ends in
    /// "Failure", every other stage's in "Time!".
    pub fn time_up_is_failure(&self) -> bool {
        self.stage == Stage::Bonus3
    }

    /// `sc1PGameTryInitPlayerArrows`: no off-screen arrows in Race to the
    /// Finish.
    pub fn has_player_arrows(&self) -> bool {
        self.stage != Stage::Bonus3
    }

    /// `sc1PGameFuncStart`'s music: Meta Crystal and the Polygon Team set
    /// theirs to wait for "Go" and play the prologue sound; the others
    /// play at once with the crowd's cheer.
    pub fn bgm_waits_for_go(&self) -> bool {
        matches!(self.stage, Stage::MMario | Stage::Zako)
    }

    /// The stage's screen flash alpha (`ifScreenFlashMakeInterface`).
    pub fn screenflash_alpha(&self) -> u8 {
        STAGE_DESC[self.stage as usize].screenflash_alpha
    }
}

/// One team stock icon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StockIcon {
    pub x: i32,
    pub y: i32,
    pub costume: Option<u8>,
}

/// `dSC1PGameZoomEyeX`, degrees.
pub const ZOOM_EYE_X: [f32; 6] = [-15.0, 0.0, 15.0, 30.0, 45.0, 60.0];
/// `dSC1PGameZoomEyeY`, degrees.
pub const ZOOM_EYE_Y: [f32; 4] = [1.0, 2.0, -8.0, -30.0];

/// `sc1PGameSetCameraZoom`'s angles in radians: a random eye x turned by
/// the player's facing, then a random eye y. The camera eases at 0.06
/// over a 28-degree field of view to the fighter's `closeup_camera_zoom`.
pub fn set_camera_zoom_angles(facing: Facing) -> (f32, f32) {
    let lr = match facing {
        Facing::Left => -1.0,
        Facing::Right => 1.0,
    };
    let x = ZOOM_EYE_X[rand_int_range(ZOOM_EYE_X.len() as i32) as usize].to_radians() * lr;
    let y = ZOOM_EYE_Y[rand_int_range(ZOOM_EYE_Y.len() as i32) as usize].to_radians();
    (x, y)
}

/// `gmCameraSetStatusPlayerZoom`'s ease and field of view in
/// `sc1PGameSetCameraZoom`.
pub const CAMERA_ZOOM_EASE: f32 = 0.06;
pub const CAMERA_ZOOM_FOV: f32 = 28.0;

/// The fighter's status at its death, for [`DefeatRecord::status`].
pub fn dead_status(status: crate::status::Status) -> DeadStatus {
    use crate::status::Status;
    match status {
        Status::DeadDown => DeadStatus::Down,
        Status::DeadLeftRight => DeadStatus::LeftRight,
        Status::DeadUpStar => DeadStatus::UpStar,
        Status::DeadUpFall => DeadStatus::UpFall,
        _ => DeadStatus::Other,
    }
}

#[cfg(test)]
#[path = "setup_tests.rs"]
mod tests;
