//! The portable boundary between the scene manager and a live battle.
use super::{bonus::DefeatRecord, manager::Manager, wait::Wait, *};
use crate::{battle::Battle, fighter::Fighter, status::BlastZone};
use ssb_engine::math::Vec3;

/// A campaign owns its scene data and battle records across scene returns.
/// The host owns fighters, maps, presentation and backup persistence.
pub struct Session {
    pub manager: Manager,
    pub data: SceneData,
    pub state: BattleState,
    pub game: Option<Game>,
    pub wait: Option<Wait>,
    pub battle: Option<Battle>,
}

/// One fighter `sc1PGameFuncStart` makes (`FTDesc`): the battle state's
/// player and the stage's setup for its slot.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Entrant {
    pub kind: FighterKind,
    pub costume: u8,
    pub shade: u8,
    pub level: u8,
    pub handicap: u8,
    pub team: u8,
    /// `players[].color`, the player tag and damage colour.
    pub color: u8,
    pub human: bool,
    /// `is_spgame_enemy`.
    pub enemy: bool,
    pub setup: setup::PlayerSetup,
    /// `desc.detail`: high detail with fewer than three fighters.
    pub detail_high: bool,
}

impl Session {
    /// Advance the battle and sample end status at Set, once. The return's
    /// second flag requests the Meta Crystal/Polygon music on Go.
    pub fn begin_frame(
        &mut self,
        end: impl FnOnce() -> (bonus::EndStatus, u32),
    ) -> (crate::battle::Frame, bool) {
        let battle = self.battle.as_mut().expect("active battle");
        let frame = battle.begin_frame();
        let bgm = self
            .game
            .as_mut()
            .expect("active game")
            .update(battle.status, end);
        (frame, bgm)
    }

    /// At the end of an enemy's KO wait. The host consumes the request
    /// and creates the described fighter or puts the old fighter to sleep.
    pub fn next_enemy(
        &mut self,
        player: u8,
        points: &[Vec3],
        camera_top: f32,
        map_top: f32,
    ) -> setup::NextEnemy {
        let next = self
            .game
            .as_mut()
            .expect("active game")
            .spawn_enemy_team_next(
                &mut self.state,
                &self.data,
                player,
                points,
                camera_top,
                map_top,
            );
        if matches!(next, setup::NextEnemy::Spawn(_)) {
            self.state.init_player_battle_stats(player);
            self.state.players[player as usize].stock_damage_all = 0;
            let p = &mut self.battle.as_mut().expect("active battle").players[player as usize];
            *p = crate::battle::Player {
                present: true,
                is_human: false,
                team: p.team,
                ..Default::default()
            };
        }
        next
    }

    /// Apply the result scene's ledger once before resuming the manager.
    pub fn finish_stage_clear(&mut self, backup: &mut Backup) -> manager::Scene {
        assert_eq!(self.manager.scene, manager::Scene::StageClear);
        self.data.score = results::score(&self.data, &self.state, backup.spgame_difficulty);
        self.manager
            .advance(&mut self.data, &mut self.state, backup)
    }

    /// `mnPlayers1PGameContinue` halves the score when Yes is selected,
    /// before returning to the manager. Keep its float expression.
    pub fn finish_continue(&mut self, accepted: bool, backup: &mut Backup) -> manager::Scene {
        assert_eq!(self.manager.scene, manager::Scene::Continue);
        self.data.is_continue = accepted;
        if accepted {
            self.data.score = (self.data.score as f32 * 0.5) as u32;
        }
        self.manager
            .advance(&mut self.data, &mut self.state, backup)
    }

    /// `sc1PBonusStageSetBonusStats` and `WriteBackup` for a campaign
    /// bonus stage. Its separate battle never consumes campaign stocks or
    /// contributes to the campaign's time/fall totals, only damage.
    pub fn finish_bonus_stage(
        &mut self,
        result: &BattleState,
        tasks_remain: u8,
        backup: &mut Backup,
    ) -> manager::Scene {
        assert_eq!(self.manager.scene, manager::Scene::BonusStage);
        assert!(tasks_remain <= BONUSGAME_TASK_MAX);
        if !self.data.is_reset {
            self.data.bonus_tasks_complete = BONUSGAME_TASK_MAX - tasks_remain;
            self.manager.total_damage += result.players[self.data.player as usize].total_damage_all;
            self.data.bonus_get_mask = [0; 3];
            self.data.time_remain = if tasks_remain != 0 {
                0
            } else {
                bonus::Bonus::Perfect.insert(&mut self.data.bonus_get_mask);
                result.time_remain.div_ceil(60)
            };
            let record = &mut backup.spgame_records[self.data.fkind as usize];
            let (count, time) = if self.data.stage() == Some(Stage::Bonus1) {
                (&mut record.bonus1_task_count, &mut record.bonus1_time)
            } else {
                (&mut record.bonus2_task_count, &mut record.bonus2_time)
            };
            if tasks_remain != 0 {
                if *count < self.data.bonus_tasks_complete {
                    *count = self.data.bonus_tasks_complete;
                    backup.writes += 1;
                }
            } else {
                *count = BONUSGAME_TASK_MAX;
                if result.time_passed < *time {
                    *time = result.time_passed;
                    backup.writes += 1;
                }
            }
        }
        self.manager
            .advance(&mut self.data, &mut self.state, backup)
    }

    pub fn new(mut data: SceneData, backup: &Backup) -> Self {
        let mut state = BattleState::default();
        let manager = Manager::new(&mut data, &mut state, backup);
        Self {
            manager,
            data,
            state,
            game: None,
            wait: None,
            battle: None,
        }
    }

    /// `sc1PGameFuncStart`: stage setup precedes fighter creation; battle
    /// totals reset when the new fighters are made, after allies were picked.
    pub fn start_battle(&mut self, backup: &Backup) {
        assert_eq!(self.manager.scene, manager::Scene::Battle);
        super::live::reset_count();
        let game = Game::setup_stage_all(
            &mut self.state,
            &mut self.data,
            backup.spgame_difficulty,
            self.manager.level_drop,
            self.manager.kirby_final_copy,
        );
        let ports: alloc::vec::Vec<_> = self.state.present().map(|(i, _)| i as u8).collect();
        for port in ports {
            self.state.init_player_battle_stats(port);
            self.state.players[port as usize].stock_damage_all = 0;
        }
        let boss = self
            .state
            .present()
            .find(|(_, p)| p.is_spgame_enemy)
            .map_or(0, |(i, _)| i as u8);
        let wait = Wait::new(
            game.stage,
            self.data.player,
            self.state.pl_count + self.state.cp_count,
            boss,
        );
        self.battle = Some(Battle::new_1p(
            self.state.time_limit,
            self.state.battle_players(),
            self.state.is_team_attack,
            wait.go_tick,
        ));
        self.game = Some(game);
        self.wait = Some(wait);
    }

    /// The fighters the started battle makes, by port.
    pub fn entrants(&self) -> [Option<Entrant>; PLAYERS_MAX] {
        let game = self.game.as_ref().expect("active game");
        let detail_high = self.state.pl_count + self.state.cp_count < 3;
        core::array::from_fn(|port| {
            let p = &self.state.players[port];
            (p.pkind != PlayerKind::Not).then_some(Entrant {
                kind: p.fkind,
                costume: p.costume,
                shade: p.shade,
                level: p.level,
                handicap: p.handicap,
                team: p.team,
                color: p.color,
                human: p.pkind == PlayerKind::Man,
                enemy: p.is_spgame_enemy,
                setup: game.setups[port],
                detail_high,
            })
        })
    }

    /// Configure the shared KO machinery for a fighter the host made.
    pub fn configure_fighter(&mut self, f: &mut Fighter, team_bounds: BlastZone) {
        f.stats.enable();
        // `ftManagerInitFighter` publishes the new fighter's damage.
        // This is not damage taken this stage.
        self.state.players[f.port as usize].stock_damage_all = u32::from(f.damage);
        let p = self.state.players[f.port as usize];
        f.stocks = p.stock_count;
        f.team = p.team;
        f.handicap = p.handicap;
        f.costume = p.costume;
        f.dead.stock_rule = false;
        f.dead.spgame_rule = true;
        f.dead.team_bounds = p.is_spgame_enemy.then_some(team_bounds);
        f.dead.enemy_next_pending = false;
    }

    /// `ftParamUpdatePlayerBattleStats` and `ftParamUpdateDamage`'s totals.
    /// Call once per applied hit, including environmental damage.
    pub fn damage(&mut self, attacker: u8, defender: u8, damage: u32, percent: u32) {
        self.state
            .update_player_battle_stats(attacker, defender, damage);
        self.state.update_damage(defender, damage, percent);
    }

    /// Drain after live callbacks, before KO records or results read the
    /// ledger. Totals are callback amounts; percent is sampled separately
    /// so healing, cap saturation and rebirth never fabricate damage.
    pub fn collect_fighter(&mut self, f: &mut Fighter) {
        use super::live::Event;
        let port = f.port;
        let me = self.data.player;
        for event in f.stats.drain() {
            match event {
                Event::Damage(damage) => {
                    self.state.update_damage(port, damage, u32::from(f.damage));
                }
                Event::Credit { player, damage } => {
                    self.state.update_player_battle_stats(player, port, damage);
                }
                Event::Attack(flags) if port == me => {
                    self.game.as_mut().expect("active game").bonus.attack(
                        flags.id(),
                        flags.smash(),
                        flags.air(),
                        flags.projectile(),
                    );
                }
                Event::Defend {
                    player: Some(player),
                    flags,
                } if player == me => {
                    self.game.as_mut().expect("active game").bonus.defend(
                        flags.id(),
                        flags.smash(),
                        flags.air(),
                        flags.projectile(),
                    );
                }
                Event::Item(kind) if port == me => {
                    let b = &mut self.game.as_mut().expect("active game").bonus;
                    use crate::item::utility::Kind;
                    let count = match kind {
                        Kind::Tomato => &mut b.tomato_count,
                        Kind::Heart => &mut b.heart_count,
                        Kind::Star => &mut b.star_count,
                    };
                    *count = count.saturating_add(1);
                }
                Event::ShieldBreak {
                    player: Some(player),
                } if player == me && port != me => {
                    self.game
                        .as_mut()
                        .expect("active game")
                        .bonus
                        .shield_breaker = true;
                }
                Event::Mew if port == me => {
                    self.game.as_mut().expect("active game").bonus.mew_catcher = true
                }
                _ => {}
            }
        }
        self.state.players[port as usize].stock_damage_all = u32::from(f.damage);
    }

    /// Source KO record from the fighter and the actual stage's team order.
    /// Consumes `dead.scored` once, after its callbacks have been collected.
    pub fn collect_fall(&mut self, f: &mut Fighter) {
        self.collect_fighter(f);
        if core::mem::take(&mut f.dead.scored) {
            let order = self.game.as_ref().expect("active game").setups[f.port as usize].team_order;
            self.fall(f.port, f.stats.defeat(f, order), f.kind);
        }
    }

    /// After the fighter raises `dead.scored`, take the battle's stock
    /// once and run `sc1PGameSetPlayerDefeatStats` in source order.
    pub fn fall(&mut self, port: u8, record: DefeatRecord, kind: FighterKind) {
        let battle = self.battle.as_mut().expect("active battle");
        battle.on_fall(port, record.damage_player);
        self.game
            .as_mut()
            .expect("active game")
            .set_player_defeat_stats(battle, &self.state, &self.data, port, record, kind);
    }

    /// Called after `grBonus3FinishProcUpdate` finds the player on Detect.
    pub fn complete_race(&mut self) {
        if self.data.stage() == Some(Stage::Bonus3) {
            self.battle
                .as_mut()
                .expect("active battle")
                .announce_complete();
        }
    }

    /// Copy live match records before `sc1PGameInitBonusStats` and the
    /// manager's loss/clear decision. Each result is consumed only once.
    pub fn finish_battle(&mut self, backup: &mut Backup) -> manager::Scene {
        let battle = self.battle.take().expect("active battle");
        self.state.time_passed = battle.time_passed;
        self.state.time_remain = battle.time_remain;
        self.data.is_reset = battle.is_reset;
        for (p, b) in self.state.players.iter_mut().zip(battle.players) {
            p.stock_count = b.stock_count;
            p.falls = b.falls;
            p.score = b.score;
        }
        self.game.take().expect("active game").init_bonus_stats(
            &self.state,
            &mut self.data,
            &mut self.manager,
        );
        self.wait = None;
        self.manager
            .advance(&mut self.data, &mut self.state, backup)
    }
}
