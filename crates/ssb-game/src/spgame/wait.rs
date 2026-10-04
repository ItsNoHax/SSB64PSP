//! Entry-thread sleeps expressed as scene-clock events. Frame one starts
//! the thread, matching `Battle::GO_TICK`'s convention.
use super::{next_port, Stage, PLAYERS_MAX};
use crate::rng::rand_int_range;
use alloc::vec::Vec;

pub const COMMON_SLEEP_TICS: [u32; 3] = [22, 15, 60];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Countdown,
    Appear(u8),
    AppearPosition(u8),
    Zoom(u8),
    CameraDefault,
    Bonus3Follow,
    BossCameraAnim,
    Go,
    BossGo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Event {
    pub tick: u32,
    pub action: Action,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wait {
    pub events: Vec<Event>,
    pub go_tick: u32,
}

impl Wait {
    pub fn new(stage: Stage, player: u8, count: u8, boss_player: u8) -> Self {
        let random = if matches!(
            stage,
            Stage::Yoshi
                | Stage::Kirby
                | Stage::Zako
                | Stage::Boss
                | Stage::Bonus3
                | Stage::Bonus1
                | Stage::Bonus2
        ) {
            0
        } else {
            rand_int_range(3) as usize
        };
        Self::with_pattern(stage, player, count, boss_player, random)
    }

    pub fn with_pattern(
        stage: Stage,
        player: u8,
        count: u8,
        boss_player: u8,
        random: usize,
    ) -> Self {
        let mut out = Self {
            events: Vec::new(),
            go_tick: crate::battle::Battle::GO_TICK,
        };
        let mut push = |tick, action| out.events.push(Event { tick, action });
        match stage {
            Stage::Bonus1 | Stage::Bonus2 => {
                out.go_tick = 0;
            }
            Stage::Bonus3 => {
                push(1, Action::Bonus3Follow);
                push(61, Action::Go);
                out.go_tick = 61;
            }
            Stage::Boss => {
                push(1, Action::BossCameraAnim);
                push(1, Action::Appear(boss_player));
                push(601, Action::BossGo);
                out.go_tick = 601;
            }
            Stage::Yoshi | Stage::Kirby | Stage::Zako => {
                push(91, Action::Countdown);
                let mut port = player;
                for i in 0..count {
                    push(
                        91 + u32::from(i) * 60,
                        if port == player {
                            Action::Appear(port)
                        } else {
                            Action::AppearPosition(port)
                        },
                    );
                    port = next_port(port);
                }
            }
            _ => {
                let sleep = COMMON_SLEEP_TICS[random];
                push(91, Action::Countdown);
                let mut tick =
                    91 + if random == 1 { 90 } else { 0 } + if count < 3 { sleep } else { 0 };
                let mut port = player;
                for _ in 0..count {
                    push(tick, Action::Appear(port));
                    if random == 2 {
                        push(tick + 30, Action::Zoom(port));
                    }
                    tick += sleep;
                    port = next_port(port);
                }
                if random == 2 {
                    push(tick + 30, Action::CameraDefault);
                }
            }
        }
        out
    }

    pub fn at(&self, tick: u32) -> impl Iterator<Item = Action> + '_ {
        self.events
            .iter()
            .filter(move |e| e.tick == tick)
            .map(|e| e.action)
    }
}

/// `sc1PGameSetPlayerInterfacePositions`: consecutive ports, beginning
/// with the human player's port, irrespective of numeric slot order.
pub fn interface_positions(player: u8, count: u8) -> [i32; PLAYERS_MAX] {
    let positions: &[i32] = match count {
        2 => &[125, 195],
        3 => &[90, 160, 230],
        4 => &[55, 125, 195, 265],
        _ => &[],
    };
    let mut out = [0; PLAYERS_MAX];
    let mut port = player;
    for &x in positions {
        out[usize::from(port)] = x;
        port = next_port(port);
    }
    out
}
