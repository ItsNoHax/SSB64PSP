//! The battle's game status, countdown, timer, stocks and results —
//! `if/ifcommon.c`'s `ifCommonBattle*`, `ifCommonTimer*`,
//! `ifCommonEntryAllThread` and `ifCommonCountdownThread`, and
//! `sc/sccommon/scvsbattle.c`'s start facing and sudden-death check.
//!
//! The host calls [`Battle::begin_frame`] once per frame, as
//! `scVSBattleFuncUpdate` calls `ifCommonBattleUpdateInterfaceAll`. It says
//! whether the world runs this frame and whether the scene is over. Fighter
//! deaths reach the battle through [`Battle::on_fall`], the
//! `ftCommonDeadUpdateScore` half the battle state owns.
//!
//! The countdown's sprites, announcer voices, the pause menu and its camera
//! are presentation and stay with the host.

use crate::fighter::Facing;

/// `SCBATTLE_TIMELIMIT_INFINITE`.
pub const TIMELIMIT_INFINITE: u8 = 100;
/// `I_MIN_TO_TICS(1)`.
pub const TICS_PER_MINUTE: u32 = 3600;
/// `ifCommonEntryAllThread` sleeps 90 ticks before it makes the countdown.
pub const ENTRY_WAIT: u32 = 90;
/// `ifCommonCountdownThread`'s timer reads 120, 180 and 240 at "3", "2" and
/// "1", and 300 (`I_SEC_TO_TICS(5)`) at "Go".
pub const COUNTDOWN_THREE: u32 = 120;
pub const COUNTDOWN_GO: u32 = 300;
/// `ifCommonAnnounceTimeUpInitInterface` and `ifCommonAnnounceEndMessage`
/// hold the "Time!"/"Game!" screen 90 ticks.
pub const END_RESTORE_WAIT: u16 = 90;
/// `ifCommonBattleInterfaceProcSet`: three more ticks before the next
/// scene loads.
pub const SET_RESTORE_WAIT: u16 = 3;
/// `ifCommonBattlePauseUpdateInterface`: the camera eases back for 20 ticks
/// after an unpause that had zoomed on the player.
pub const UNPAUSE_RESTORE_WAIT: u16 = 20;

/// `nSCBattleGameStatus*`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameStatus {
    Wait,
    Go,
    Pause,
    Unpause,
    End,
    BossDefeat,
    Set,
}

/// `SCBATTLE_GAMERULE_TIME` or `_STOCK`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rule {
    Time,
    Stock,
}

/// Why the battle ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndKind {
    /// `ifCommonAnnounceTimeUpInitInterface`.
    TimeUp,
    /// `ifCommonAnnounceEndMessage`: one player or team is left.
    GameSet,
    /// `ifCommonAnnounceCompleteInitInterface`: a bonus-stage task or gate.
    Complete,
    /// Bonus course timeout or a fall.
    Failure,
    /// Master Hand's hit points ran out (`sc1PGameBossDefeatInitInterface`).
    BossDefeat,
}

/// The phases of Master Hand's defeat (`sc1PGameBossDefeatInitInterface`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BossDefeat {
    /// `ifCommonBattleSetInterface(..., 90)`: for 90 ticks every process
    /// is paused but the camera's, the interface's, the effects' and the
    /// boss wallpaper's (`sc1PGameBossDefeatInterfaceProcUpdate`).
    Zoom,
    /// `ifCommonBattleBossDefeatSetGameStatus`: the world runs one tick in
    /// three (`dIFCommonBattleBossUpdateInterval = 2`) while the camera
    /// runs every tick, until the wallpaper's fade ends
    /// (`ifCommonBattleEndSetBossDefeat`, [`Battle::boss_wallpaper_done`]).
    Slow,
}

/// `ifCommonBattleSetInterface`'s restore wait for Master Hand's defeat.
pub const BOSS_DEFEAT_ZOOM_WAIT: u16 = 90;
/// `dIFCommonBattleBossUpdateInterval`.
pub const BOSS_DEFEAT_INTERVAL: u8 = 2;

/// One `gSCManagerBattleState->players` entry, the fields the battle reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Player {
    /// `pkind != nFTPlayerKindNot`.
    pub present: bool,
    /// `pkind == nFTPlayerKindMan`.
    pub is_human: bool,
    pub team: u8,
    /// Lives left beyond the current one; -1 is out.
    pub stock_count: i8,
    /// KOs scored.
    pub score: u16,
    pub falls: u16,
    pub self_destructs: u16,
    /// `total_kos_players`.
    pub kos: [u16; 4],
    /// `total_damage_given` (`ftParamUpdatePlayerBattleStats`).
    pub total_damage_given: u32,
    /// `total_damage_all` (`ftParamUpdateDamage`).
    pub total_damage_all: u32,
    /// Final place, 0 first, for a stock battle's losers in the order they
    /// went out.
    pub place: u8,
}

/// What the world does this frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Frame {
    /// `gcRunAll` with the world running.
    Run,
    /// The world is paused (`ifCommonBattleInterfaceProcUpdate`); only the
    /// interface runs.
    Frozen,
    /// `syTaskmanSetLoadScene`: on to the results.
    Done,
}

/// The battle state.
#[derive(Debug, Clone, PartialEq)]
pub struct Battle {
    pub status: GameStatus,
    pub rule: Rule,
    pub is_team_battle: bool,
    /// Team Attack: `dSCManagerDefaultBattleState`'s FALSE, as VS Options
    /// is not ported.
    pub is_team_attack: bool,
    /// Minutes, or [`TIMELIMIT_INFINITE`].
    pub time_limit: u8,
    pub time_remain: u32,
    pub time_passed: u32,
    pub players: [Player; 4],
    pub end: Option<EndKind>,
    timer_started: bool,
    /// `sIFCommonBattlePlace`: the place the next team to go out takes.
    place: i32,
    restore_wait: u16,
    /// Ticks since the scene started, for the countdown threads.
    clock: u32,
    /// The frame "Go" lands on.
    go_tick: u32,
    /// `gSCManagerSceneData.is_suddendeath`.
    pub is_sudden_death: bool,
    /// `gSCManagerSceneData.is_reset`: A+B+R+Z in the pause menu.
    pub is_reset: bool,
    /// `SCBATTLE_GAMERULE_1PGAME`: a fall takes a stock without the stock
    /// rule's placement, and the 1P Game ([`crate::spgame`]) decides the end.
    pub is_1p_game: bool,
    pub is_bonus: bool,
    /// The timer callback selected by `sc1PGameInitTimeUpMessage`.
    /// Race uses FAILURE while retaining the ordinary campaign stock rule.
    pub time_up_is_failure: bool,
    /// The wait the end's proc-set leaves before the next scene: 3
    /// (`ifCommonBattleInterfaceProcSet`) or 45
    /// (`ifCommon1PGameInterfaceProcSet`).
    set_wait: u16,
    /// `ifCommon1PGameInterfaceProcSet` ran: the host zooms on the player
    /// (`sc1PGameSetCameraZoom`) while the scene holds 45 ticks.
    pub set_zoom: bool,
    /// Master Hand's defeat, when it is under way.
    pub boss_defeat: Option<BossDefeat>,
    /// `dIFCommonBattleBossUpdateWait`.
    boss_wait: u8,
    /// `sc1PGameBossDefeatInterfaceProcSet` ran this tick: the host changes
    /// the wallpaper and starts the defeat camera animation.
    pub boss_set: bool,
    /// How to Play or the auto demo (`nSCBattleGameTypeExplain`,
    /// `...Demo`): the scene makes no battle interface, so there is no
    /// countdown, timer or pause, and "Go" is set at once.
    pub is_demo: bool,
    /// `sIFCommonBattleEndSoundsQueue`/`...Num`: the sounds the end replays
    /// after it stops every FGM ([`Battle::add_end_sound`]).
    end_sounds: [u16; END_SOUNDS_MAX],
    end_sounds_num: u8,
    /// The voice `ifCommonAnnounceCompleteInitInterface` queues for a
    /// completed bonus task ([`Battle::announce_complete`]).
    complete_sfx: u16,
    /// `sSC1PGameBossDefeatSoundTerminateTemp`: the FGM count Master Hand's
    /// defeat saved before blocking every FGM.
    boss_fgm_count: Option<u16>,
    /// `sIFCommonIsAnnouncedSecond`: the last five seconds' voices.
    announced_seconds: [bool; 5],
    /// `gSCManagerBattleState->gkind`, which the timer reads for Mushroom
    /// Kingdom's hurry music. `None` where the host has not set it.
    pub gkind: Option<u8>,
}

/// `nGRKindInishie`.
const GKIND_INISHIE: u8 = 8;

/// `dIFCommonAnnounceTimerVoiceIDs`: "one" to "five".
const TIMER_VOICES: [u16; 5] = [
    crate::sound::id::nSYAudioVoiceAnnounceOne,
    crate::sound::id::nSYAudioVoiceAnnounceTwo,
    crate::sound::id::nSYAudioVoiceAnnounceThree,
    crate::sound::id::nSYAudioVoiceAnnounceFour,
    crate::sound::id::nSYAudioVoiceAnnounceFive,
];

/// `ARRAY_COUNT(sIFCommonBattleEndSoundsQueue)`.
pub const END_SOUNDS_MAX: usize = 16;
/// `syAudioSetBGMVolume(0, 0x7800)`: the battle music's normal volume.
pub const BGM_VOLUME_NORMAL: u32 = 0x7800;
/// `syAudioSetBGMVolume(0, 0x3C00)`: half volume while paused.
pub const BGM_VOLUME_PAUSE: u32 = 0x3C00;

impl Battle {
    /// `scVSBattleStartBattle`'s battle half: `ifCommonBattleInitPlacement`,
    /// `ifCommonEntryAllMakeInterface` (status Wait) and
    /// `ifCommonTimerMakeInterface`.
    pub fn new(rule: Rule, time_limit: u8, stocks: i8, players: [Player; 4]) -> Battle {
        let mut players = players;
        for p in players.iter_mut().filter(|p| p.present) {
            p.stock_count = stocks;
        }
        let mut b = Battle {
            status: GameStatus::Wait,
            rule,
            is_team_battle: false,
            is_team_attack: false,
            time_limit,
            time_remain: u32::from(time_limit) * TICS_PER_MINUTE,
            time_passed: 0,
            players,
            end: None,
            timer_started: false,
            place: 0,
            restore_wait: 0,
            clock: 0,
            go_tick: Self::GO_TICK,
            is_sudden_death: false,
            is_reset: false,
            is_1p_game: false,
            is_bonus: false,
            time_up_is_failure: false,
            set_wait: SET_RESTORE_WAIT,
            set_zoom: false,
            boss_defeat: None,
            boss_wait: 0,
            boss_set: false,
            is_demo: false,
            end_sounds: [0; END_SOUNDS_MAX],
            end_sounds_num: 0,
            complete_sfx: crate::sound::id::nSYAudioVoiceAnnounceComplete,
            boss_fgm_count: None,
            announced_seconds: [false; 5],
            gkind: None,
        };
        b.init_placement();
        b
    }

    /// `scExplainStartBattle` and `scAutoDemoStartBattle`'s battle:
    /// `dSCManagerDefaultBattleState`'s time rule and two stocks, with no
    /// timer (`ifCommonTimerMakeInterface` is not made) and the status
    /// "Go" from the first frame.
    pub fn new_demo(players: [Player; 4]) -> Battle {
        let mut b = Battle::new(Rule::Time, TIMELIMIT_INFINITE, 2, players);
        b.status = GameStatus::Go;
        b.go_tick = 0;
        b.is_demo = true;
        b
    }

    /// The battle with `gSCManagerBattleState->is_team_battle` and
    /// `is_team_attack` set, placed by team (`ifCommonBattleInitPlacement`).
    /// `is_show_score`: `dSCManagerDefaultBattleState`'s TRUE for a VS
    /// battle, cleared for sudden death (`scVSBattleSetScoreCheckSuddenDeath`),
    /// the 1P Game (`sc1PManagerUpdateScene`) and the bonus stages
    /// (`sc1PBonusStageInitVars`); Training clears it too
    /// (`sc1PTrainingModeInitVars`). A fall
    /// then shows "-1" and its scorer's "+1" (`ftCommonDeadUpdateScore`).
    pub fn is_show_score(&self) -> bool {
        !(self.is_sudden_death || self.is_1p_game || self.is_bonus)
    }

    pub fn with_teams(mut self, is_team_battle: bool, is_team_attack: bool) -> Battle {
        self.is_team_battle = is_team_battle;
        self.is_team_attack = is_team_attack;
        self.init_placement();
        self
    }

    /// `sc1PManagerUpdateScene`'s battle state: a team battle under
    /// `SCBATTLE_GAMERULE_1PGAME | SCBATTLE_GAMERULE_TIME`, each player
    /// keeping the stocks it brings. `go_tick` is the frame the stage's
    /// wait thread says "Go" ([`crate::spgame::wait`]).
    pub fn new_1p(
        time_limit: u8,
        players: [Player; 4],
        is_team_attack: bool,
        go_tick: u32,
    ) -> Battle {
        let mut b =
            Battle::new(Rule::Time, time_limit, 0, players).with_teams(true, is_team_attack);
        b.players = players;
        b.is_1p_game = true;
        b.go_tick = go_tick;
        b
    }

    /// The rule the hit, catch and CPU searches read ([`crate::team`]).
    pub fn team_rules(&self) -> crate::team::TeamRules {
        crate::team::TeamRules {
            is_team_battle: self.is_team_battle,
            is_team_attack: self.is_team_attack,
        }
    }

    /// `sc1PBonusStageMakeInterface`: its thread first runs on tick 1
    /// and sleeps 60 before GO. The bonus rules do not consume stocks.
    pub fn new_bonus(time_limit: u8, players: [Player; 4]) -> Self {
        let mut b = Self::new(Rule::Time, time_limit, 0, players);
        b.is_bonus = true;
        b.go_tick = 61;
        b
    }

    /// `scVSBattleSetScoreCheckSuddenDeath`'s new battle for the tied
    /// players: a stock battle with no stocks to spare
    /// (`scVSBattleStartSuddenDeath`, where each fighter starts at 300%).
    /// `ifCommonSuddenDeathThread` says "Go" after 90 ticks.
    pub fn sudden_death_battle(&self) -> Option<Battle> {
        let tied = self.sudden_death()?;
        let mut players = [Player::default(); 4];
        for (i, p) in players.iter_mut().enumerate() {
            if tied[i] {
                *p = Player {
                    present: true,
                    is_human: self.players[i].is_human,
                    team: self.players[i].team,
                    ..Player::default()
                };
            }
        }
        // `gSCManagerVSBattleState` is a copy of the transfer state: the
        // team settings carry over.
        let mut b = Battle::new(Rule::Stock, self.time_limit, 0, players)
            .with_teams(self.is_team_battle, self.is_team_attack);
        b.go_tick = 1 + ENTRY_WAIT;
        b.is_sudden_death = true;
        Some(b)
    }

    /// `ifCommonBattleInitPlacement`: one place per team (or player) in the
    /// battle, counted from the last.
    fn init_placement(&mut self) {
        let mut members = [0u8; 5];
        for p in self.players.iter().filter(|p| p.present) {
            let i = if self.is_team_battle {
                usize::from(p.team)
            } else {
                0
            };
            members[i.min(4)] += 1;
        }
        let teams = if self.is_team_battle {
            members.iter().filter(|&&m| m != 0).count() as i32
        } else {
            self.players.iter().filter(|p| p.present).count() as i32
        };
        self.place = teams - 1;
    }

    fn timed(&self) -> bool {
        self.rule == Rule::Time && self.time_limit != TIMELIMIT_INFINITE
    }

    /// The frame "Go" lands on, counting the scene's first frame as 1: the
    /// entry thread first runs on frame 1 and sleeps 90, and the countdown
    /// thread it then makes reads 300 another 300 frames on.
    pub const GO_TICK: u32 = 1 + ENTRY_WAIT + COUNTDOWN_GO;

    /// Frames since the scene started, counting the current one: the
    /// clock the entry and countdown threads read.
    pub fn clock(&self) -> u32 {
        self.clock
    }

    /// `ifCommonBattleUpdateInterfaceAll`, and the interface processes the
    /// frame's `gcRunAll` runs: the countdown and the timer.
    pub fn begin_frame(&mut self) -> Frame {
        if self.status != GameStatus::Go {
            self.timer_started = false;
        } else if !self.timer_started {
            // `sySchedulerSetTicCount(0)`: the first Go frame reads no time.
            self.timer_started = true;
            self.clock += 1;
            return Frame::Run;
        }
        match self.status {
            GameStatus::Wait => {
                self.clock += 1;
                // `ifCommonAnnounceGoSetStatus` unlocks every fighter.
                if self.clock >= self.go_tick {
                    self.status = GameStatus::Go;
                }
                Frame::Run
            }
            GameStatus::Go => {
                self.clock += 1;
                self.tick_timer();
                // The time-up proc runs from the timer itself, inside the
                // frame's `gcRunAll`.
                Frame::Run
            }
            GameStatus::Pause => Frame::Frozen,
            // `ifCommonBattlePauseRestoreInterfaceAll`: the camera eases
            // back, then Go resumes and the world runs that same frame.
            GameStatus::Unpause => {
                if self.restore_wait != 0 {
                    self.restore_wait -= 1;
                    Frame::Frozen
                } else {
                    self.status = GameStatus::Go;
                    Self::resume_audio();
                    Frame::Run
                }
            }
            // `ifCommonBattleEndUpdateInterface` pauses the world and falls
            // through to `ifCommonBattleBossDefeatUpdateInterface`.
            GameStatus::End | GameStatus::BossDefeat if self.boss_defeat.is_some() => {
                if self.status == GameStatus::End {
                    self.boss_defeat_proc_update();
                }
                self.boss_frame()
            }
            GameStatus::End | GameStatus::BossDefeat => {
                if self.status == GameStatus::End {
                    // `ifCommonBattleInterfaceProcUpdate` and
                    // `ifCommonBonusInterfaceProcUpdate`: every FGM stops
                    // and the end's queue plays.
                    crate::sound::stop_all_fgm();
                    self.play_end_sounds();
                }
                self.status = GameStatus::BossDefeat;
                if self.restore_wait != 0 {
                    self.restore_wait -= 1;
                } else {
                    // `ifCommonBattleInterfaceProcSet`, or the 1P Game's
                    // `ifCommon1PGameInterfaceProcSet`.
                    self.status = GameStatus::Set;
                    self.restore_wait = self.set_wait;
                    self.set_zoom = self.set_wait != SET_RESTORE_WAIT;
                }
                Frame::Frozen
            }
            GameStatus::Set => {
                if self.restore_wait != 0 {
                    self.restore_wait -= 1;
                    Frame::Frozen
                } else {
                    Frame::Done
                }
            }
        }
    }

    /// `ifCommonBattleBossDefeatUpdateInterface` during Master Hand's
    /// defeat: the restore wait, its proc-set, then whether the world runs.
    fn boss_frame(&mut self) -> Frame {
        self.status = GameStatus::BossDefeat;
        self.boss_set = false;
        if self.restore_wait != 0 {
            self.restore_wait -= 1;
        } else {
            match self.boss_defeat {
                // `sc1PGameBossDefeatInterfaceProcSet`, then
                // `ifCommonBattleBossDefeatSetGameStatus`.
                Some(BossDefeat::Zoom) => {
                    self.boss_defeat = Some(BossDefeat::Slow);
                    self.boss_set = true;
                    self.restore_wait = u16::MAX;
                    self.boss_wait = 0;
                }
                // `ifCommonBattleInterfaceProcSet`.
                _ => {
                    self.status = GameStatus::Set;
                    self.restore_wait = SET_RESTORE_WAIT;
                    self.set_zoom = false;
                    return Frame::Frozen;
                }
            }
        }
        match self.boss_defeat {
            Some(BossDefeat::Slow) if self.boss_wait == 0 => {
                self.boss_wait = BOSS_DEFEAT_INTERVAL;
                Frame::Run
            }
            Some(BossDefeat::Slow) => {
                self.boss_wait -= 1;
                Frame::Frozen
            }
            _ => Frame::Frozen,
        }
    }

    /// `sc1PGameBossDefeatInitInterface`'s battle half
    /// (`ifCommonBattleSetInterface(sc1PGameBossDefeatInterfaceProcUpdate,
    /// sc1PGameBossDefeatInterfaceProcSet, ..., 90)`).
    pub fn boss_defeat(&mut self) {
        if self.end.is_some() {
            return;
        }
        self.status = GameStatus::End;
        self.restore_wait = BOSS_DEFEAT_ZOOM_WAIT;
        self.end = Some(EndKind::BossDefeat);
        self.end_sounds_num = 0;
        self.boss_defeat = Some(BossDefeat::Zoom);
    }

    /// `sc1PGameBossDefeatInterfaceProcUpdate`'s audio: every FGM and the
    /// music stop, the queue and the defeat sounds play, then every later
    /// FGM is blocked (`D_8009EDD0.sfx_max = 0`).
    fn boss_defeat_proc_update(&mut self) {
        use crate::sound::{self, id::*};
        sound::stop_all_fgm();
        sound::stop_bgm_all();
        self.play_end_sounds();
        sound::play_fgm(nSYAudioFGMExplodeL);
        sound::play_fgm(nSYAudioVoiceBossDead);
        sound::play_fgm(nSYAudioFGMBossDefeatL);
        self.boss_fgm_count = Some(sound::fgm_count());
        sound::set_fgm_count(0);
    }

    /// `ifCommonBattleEndSetBossDefeat`: the defeat wallpaper's last fade
    /// ended, and the next tick sets the scene.
    pub fn boss_wallpaper_done(&mut self) {
        // `func_ovl65_8018F6DC`: the FGMs play again.
        if let Some(count) = self.boss_fgm_count.take() {
            crate::sound::set_fgm_count(count);
        }
        if self.boss_defeat == Some(BossDefeat::Slow) {
            self.status = GameStatus::BossDefeat;
            self.restore_wait = 0;
        }
    }

    /// `ifCommonTimerFuncRun` with one tic of scheduler time per frame.
    fn tick_timer(&mut self) {
        self.time_passed += 1;
        if !self.timed() || self.time_remain == 0 {
            return;
        }
        self.time_remain -= 1;
        if self.gkind == Some(GKIND_INISHIE)
            && self.time_remain <= 30 * 60
            && crate::music::bgm_default() != crate::sound::id::nSYAudioBGMInishieHurry
        {
            crate::music::set_bgm_default(crate::sound::id::nSYAudioBGMInishieHurry);
            crate::music::request_update();
        }
        if self.time_remain <= 5 * 60 {
            if self.time_remain == 0 {
                self.set_end(if self.is_bonus || self.time_up_is_failure {
                    EndKind::Failure
                } else {
                    EndKind::TimeUp
                });
            } else {
                for (i, announced) in self.announced_seconds.iter_mut().enumerate() {
                    if !*announced && self.time_remain <= (i as u32 + 1) * 60 {
                        crate::sound::play_fgm(TIMER_VOICES[i]);
                        *announced = true;
                    }
                }
            }
            // The music fades with the last five seconds.
            crate::sound::set_bgm_volume(
                0,
                ((self.time_remain as f32 / 300.0) * 20480.0 + 10240.0) as u32,
            );
        }
    }

    /// `ifCommonBattlePauseInitInterface`: START during Go. The pausable
    /// FGMs hold, the pause chime plays and the music drops to half.
    pub fn pause(&mut self) {
        if self.status == GameStatus::Go {
            self.status = GameStatus::Pause;
            crate::sound::pause_fgm();
            crate::sound::play_fgm(crate::sound::id::nSYAudioFGMGamePause);
            crate::sound::set_bgm_volume(0, BGM_VOLUME_PAUSE);
        }
    }

    /// START in the pause menu: `restore` is whether the camera had zoomed
    /// on the player and eases back (20 ticks) or snaps (0).
    pub fn unpause(&mut self, restore: bool) {
        if self.status == GameStatus::Pause {
            self.status = GameStatus::Unpause;
            self.restore_wait = if restore { UNPAUSE_RESTORE_WAIT } else { 0 };
        }
    }

    /// `ifCommonBattlePauseRestoreInterfaceAll`'s audio, as Go resumes:
    /// the held FGMs play on and the music returns to full volume.
    fn resume_audio() {
        crate::sound::resume_fgm();
        crate::sound::set_bgm_volume(0, BGM_VOLUME_NORMAL);
    }

    /// A+B+R+Z in the pause menu: `is_reset`, then
    /// `ifCommonBattleInterfaceProcSet`.
    pub fn reset(&mut self) {
        if self.status == GameStatus::Pause {
            // `func_800266A0_272A0`.
            crate::sound::stop_all_fgm();
            self.is_reset = true;
            self.status = GameStatus::Set;
            self.restore_wait = SET_RESTORE_WAIT;
        }
    }

    /// `ifCommonAnnounceEndMessage` in a 1P Game stage: while the player
    /// still has a stock the end sets through
    /// `ifCommon1PGameInterfaceProcSet`, which holds 45 ticks.
    pub fn announce_end_1p(&mut self, player_in: bool) {
        if self.end.is_none() && player_in {
            self.set_wait = 45;
        }
        self.set_end(EndKind::GameSet);
    }

    /// The bonus interface holds the completion message for 90 ticks,
    /// then uses the common three-tick scene return, without victory zoom.
    /// `sfx_id` is the voice `ifCommonAnnounceCompleteInitInterface` queues
    /// (`nSYAudioVoiceAnnounceComplete` or `...NewRecord`).
    pub fn announce_complete(&mut self, sfx_id: u16) {
        if self.end.is_none() {
            self.complete_sfx = sfx_id;
        }
        self.set_end(EndKind::Complete);
    }

    /// `ifCommonBattleSetInterface`: the end's status, and its sound queue
    /// emptied, then given the end's voice.
    fn set_end(&mut self, kind: EndKind) {
        use crate::sound::id::*;
        if self.end.is_some() {
            return;
        }
        self.status = GameStatus::End;
        self.restore_wait = END_RESTORE_WAIT;
        self.end = Some(kind);
        self.end_sounds_num = 0;
        let sfx_id = match kind {
            EndKind::TimeUp => nSYAudioVoiceAnnounceTimeUp,
            // `ifCommonAnnounceEndMessage`: a bonus stage's fall fails it.
            EndKind::GameSet if self.is_bonus => nSYAudioVoiceAnnounceFailure,
            EndKind::GameSet => nSYAudioVoiceAnnounceGameSet,
            EndKind::Complete => self.complete_sfx,
            EndKind::Failure => nSYAudioVoiceAnnounceFailure,
            EndKind::BossDefeat => nSYAudioFGMVoiceEnd,
        };
        if sfx_id != nSYAudioFGMVoiceEnd {
            self.add_end_sound(sfx_id);
        }
    }

    /// `ifCommonBattleEndAddSoundQueueID`: queued only once the battle has
    /// ended, for the end to replay after it stops every FGM.
    pub fn add_end_sound(&mut self, sfx_id: u16) {
        if self.status == GameStatus::End && (self.end_sounds_num as usize) < END_SOUNDS_MAX {
            self.end_sounds[self.end_sounds_num as usize] = sfx_id;
            self.end_sounds_num += 1;
        }
    }

    /// `ifCommonBattleEndPlaySoundQueue`.
    fn play_end_sounds(&self) {
        for &id in &self.end_sounds[..self.end_sounds_num as usize] {
            crate::sound::play_fgm(id);
        }
    }

    /// `ftParamUpdateDamage`'s and `ftParamUpdatePlayerBattleStats`'
    /// totals from `f`'s queued hit callbacks (its stats must be enabled):
    /// damage taken from anything, and damage another player dealt.
    pub fn collect_damage(&mut self, f: &mut crate::fighter::Fighter) {
        use crate::spgame::live::Event;
        let d = usize::from(f.port);
        for event in f.stats.drain() {
            match event {
                Event::Damage(damage) if d < 4 => {
                    self.players[d].total_damage_all =
                        self.players[d].total_damage_all.wrapping_add(damage);
                }
                Event::Credit { player, damage } => {
                    let a = usize::from(player);
                    if a < 4 && d < 4 && a != d {
                        self.players[a].total_damage_given =
                            self.players[a].total_damage_given.wrapping_add(damage);
                    }
                }
                _ => {}
            }
        }
    }

    /// The battle half of `ftCommonDeadUpdateScore`: `damage_player` is
    /// credited with the KO, or the fall counts as a self-destruct, and a
    /// stock battle takes a stock (`ifCommonBattleUpdateScoreStocks`).
    pub fn on_fall(&mut self, player: u8, damage_player: Option<u8>) {
        let i = usize::from(player).min(3);
        self.players[i].falls += 1;
        match damage_player {
            Some(k) if usize::from(k) < 4 => {
                self.players[usize::from(k)].score += 1;
                self.players[usize::from(k)].kos[i] += 1;
            }
            _ => self.players[i].self_destructs += 1,
        }
        if self.rule == Rule::Stock {
            self.players[i].stock_count -= 1;
            self.update_score_stocks(i);
        }
        // `SCBATTLE_GAMERULE_1PGAME`: `sc1PGameSetPlayerDefeatStats`
        // follows ([`crate::spgame::Game::set_player_defeat_stats`]).
        if self.is_1p_game {
            self.players[i].stock_count -= 1;
        }
        if self.is_bonus {
            self.set_end(EndKind::Failure);
        }
    }

    /// `ftCommonSleepProcUpdate`'s steal by sleeping player `thief`
    /// (RE-464): its teammates with stocks, in port order, the list
    /// starting over at each larger count it meets (`steal_from_player`),
    /// one of them at random (`syUtilsRandIntRange`). The chosen player
    /// loses a stock and the thief's count becomes -2 until its wait ends
    /// ([`crate::dead::update_sleep`]). Returns the stolen player.
    pub fn steal_stock(&mut self, thief: u8) -> Option<u8> {
        let t = usize::from(thief).min(3);
        let team = self.players[t].team;
        let mut from = [0u8; 4];
        let (mut count, mut most) = (0usize, 0i8);
        for (j, p) in self.players.iter().enumerate() {
            if j == t || !p.present || p.team != team || p.stock_count <= 0 {
                continue;
            }
            if most < p.stock_count {
                count = 0;
                most = p.stock_count;
            }
            from[count] = j as u8;
            count += 1;
        }
        if count == 0 {
            return None;
        }
        let stolen = from[crate::rng::rand_int_range(count as i32) as usize];
        self.players[usize::from(stolen)].stock_count -= 1;
        self.players[t].stock_count = -2;
        Some(stolen)
    }

    /// The end of a steal's wait: the thief has one life
    /// (`stock_count = 0`).
    pub fn land_stolen_stock(&mut self, thief: u8) {
        self.players[usize::from(thief).min(3)].stock_count = 0;
    }

    /// `ifCommonBattleUpdateScoreStocks`.
    fn update_score_stocks(&mut self, i: usize) {
        let team = if self.is_team_battle {
            self.players[i].team
        } else {
            i as u8
        };
        let remain = self
            .players
            .iter()
            .enumerate()
            .filter(|(j, p)| {
                p.present
                    && (if self.is_team_battle {
                        p.team
                    } else {
                        *j as u8
                    }) == team
                    && p.stock_count != -1
            })
            .count();
        if remain == 0 {
            let place = self.place.max(0) as u8;
            if self.is_team_battle {
                for p in self
                    .players
                    .iter_mut()
                    .filter(|p| p.present && p.team == team)
                {
                    p.place = place;
                }
            } else {
                self.players[usize::from(team)].place = place;
            }
            self.place -= 1;
            if self.place == 0 {
                self.set_end(EndKind::GameSet);
            }
        }
    }

    /// `scVSBattleSetScoreCheckSuddenDeath`: after a time battle, the
    /// players tied on `score - falls` at the top, when there is more than
    /// one. A team battle sums each team's and takes every member of the
    /// tied teams.
    pub fn sudden_death(&self) -> Option<[bool; 4]> {
        if self.rule != Rule::Time {
            return None;
        }
        let tko = |p: &Player| i32::from(p.score) - i32::from(p.falls);
        let side = |i: usize| {
            if self.is_team_battle {
                usize::from(self.players[i].team).min(4)
            } else {
                i
            }
        };
        let mut sides = [None::<i32>; 5];
        for (i, p) in self.players.iter().enumerate().filter(|(_, p)| p.present) {
            let total = sides[side(i)].get_or_insert(0);
            *total += tko(p);
        }
        let best = sides.iter().flatten().copied().max()?;
        if sides.iter().flatten().filter(|&&t| t == best).count() < 2 {
            return None;
        }
        let mut tied = [false; 4];
        for (i, p) in self.players.iter().enumerate() {
            tied[i] = p.present && sides[side(i)] == Some(best);
        }
        Some(tied)
    }

    /// The results' winner: the best `score - falls` after a time battle
    /// (a tie means sudden death), or the player still in after a stock
    /// battle.
    pub fn winner(&self) -> Option<usize> {
        match self.rule {
            Rule::Time => {
                if self.sudden_death().is_some() {
                    return None;
                }
                let tko = |p: &Player| i32::from(p.score) - i32::from(p.falls);
                self.players
                    .iter()
                    .enumerate()
                    .filter(|(_, p)| p.present)
                    .max_by_key(|(_, p)| tko(p))
                    .map(|(i, _)| i)
            }
            Rule::Stock => self
                .players
                .iter()
                .position(|p| p.present && p.stock_count != -1),
        }
    }
}

/// `desc.damage` in `scVSBattleStartSuddenDeath`.
pub const SUDDEN_DEATH_DAMAGE: u16 = 300;

/// `scVSBattleGetStartPlayerLR`: a VS fighter starts facing the nearest
/// other player's spawn, right when there is none or it is level.
pub fn start_facing(this_x: f32, others_x: impl Iterator<Item = f32>) -> Facing {
    let mut near_dist = 65536.0_f32;
    let mut near_spawn = 0.0_f32;
    for x in others_x {
        let dist = (x - this_x).abs();
        if near_dist > dist {
            near_dist = dist;
            near_spawn = x - this_x;
        }
    }
    if near_spawn >= 0.0 {
        Facing::Right
    } else {
        Facing::Left
    }
}

#[cfg(test)]
#[path = "battle_tests.rs"]
mod tests;
