//! `mn1pcontinue.c`: the option gate, retry and Game Over clocks.
//! Fade alphas advance on draw in the source, so `draw` is separate.
//! [`Continue::tick`] makes the scene's sound calls.
use crate::sound::{self, id};
use ssb_engine::input::{ControllerState, N64Buttons};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Choose,
    Retry,
    GameOver,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Spotlight,
    Continue,
    Options,
    Accepted,
    GameOver,
}

#[derive(Debug, Clone)]
pub struct Continue {
    pub total_tics: u32,
    pub status: Status,
    pub yes: bool,
    pub score: u32,
    pub spotlight_shown: bool,
    pub room_shown: bool,
    pub prompt_shown: bool,
    pub options_shown: bool,
    pub room_fade_in: Option<u8>,
    pub room_fade_out: Option<u8>,
    pub spotlight_fade: Option<u8>,
    pub game_over_color_step: f32,
    pub game_over_scale: f32,
    pub fighter_y_offset: f32,
    retry_tic: u32,
    game_over_input_tic: u32,
    game_over_auto_tic: u32,
    change_wait: i32,
    finished: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Frame {
    pub event: Option<Event>,
    pub result: Option<bool>,
}

impl Continue {
    /// `mnPlayers1PGameContinueFuncStart`'s state, then its music: the
    /// battle's stops, the choice's starts.
    pub fn new(score: u32) -> Self {
        sound::stop_bgm_all();
        sound::play_bgm(0, id::nSYAudioBGM1PGameEndChoice);
        Self {
            total_tics: 0,
            status: Status::Choose,
            yes: true,
            score,
            spotlight_shown: false,
            room_shown: false,
            prompt_shown: false,
            options_shown: false,
            room_fade_in: None,
            room_fade_out: None,
            spotlight_fade: None,
            game_over_color_step: 0.0,
            game_over_scale: 1.0,
            fighter_y_offset: 0.0,
            retry_tic: 0,
            game_over_input_tic: 0,
            game_over_auto_tic: 0,
            change_wait: 0,
            finished: false,
        }
    }

    fn hide_options(&mut self) {
        self.spotlight_shown = false;
        self.prompt_shown = false;
        self.options_shown = false;
    }

    fn game_over(&mut self) {
        self.hide_options();
        self.status = Status::GameOver;
        self.room_fade_out = Some(0);
        self.game_over_input_tic = self.total_tics + 90;
        self.game_over_auto_tic = self.total_tics + 1800;
        sound::play_bgm(0, id::nSYAudioBGM1PGameOver);
        sound::play_fgm(id::nSYAudioVoiceAnnounceGameOver);
    }

    pub fn tick(&mut self, input: ControllerState, taps: N64Buttons) -> Frame {
        if self.finished {
            return Frame::default();
        }
        self.total_tics += 1;
        let mut frame = Frame::default();
        if self.total_tics >= 10 {
            if self.total_tics == self.retry_tic {
                frame.result = Some(true);
            }
            if self.change_wait != 0 {
                self.change_wait -= 1;
            }
            if (-15..=15).contains(&input.stick_x) && (-15..=15).contains(&input.stick_y) {
                self.change_wait = 0;
            }
            if self.total_tics == 2400 && self.status == Status::Choose {
                self.game_over();
                frame.event = Some(Event::GameOver);
            }
            if self.status == Status::Choose {
                if self.total_tics > 150 && taps.0 & (N64Buttons::A | N64Buttons::START) != 0 {
                    if self.yes {
                        self.hide_options();
                        self.score = (self.score as f32 * 0.5) as u32;
                        self.status = Status::Retry;
                        self.retry_tic = self.total_tics + 240;
                        sound::play_fgm(id::nSYAudioFGM1PGameContinue);
                        frame.event = Some(Event::Accepted);
                    } else {
                        self.game_over();
                        frame.event = Some(Event::GameOver);
                    }
                }
                // This remains inside the Choose branch even after accepting,
                // matching FuncRun's ordering on a combined confirm/direction.
                if self.total_tics > 120 {
                    let sx = i32::from(input.stick_x);
                    let left = taps.0 & (N64Buttons::L | N64Buttons::D_LEFT | N64Buttons::C_LEFT)
                        != 0
                        || (self.change_wait == 0 && sx < -15);
                    if left && !self.yes {
                        sound::play_fgm(id::nSYAudioFGMMenuScroll1);
                        self.yes = true;
                        self.change_wait = (sx + 160) / 5;
                    }
                    let right =
                        taps.0 & (N64Buttons::R | N64Buttons::D_RIGHT | N64Buttons::C_RIGHT) != 0
                            || (self.change_wait == 0 && sx > 15);
                    if right && self.yes {
                        sound::play_fgm(id::nSYAudioFGMMenuScroll1);
                        self.yes = false;
                        self.change_wait = (160 - sx) / 5;
                    }
                }
            }
            if self.status == Status::GameOver
                && ((self.game_over_input_tic < self.total_tics
                    && taps.0 & (N64Buttons::A | N64Buttons::START) != 0)
                    || self.total_tics == self.game_over_auto_tic)
            {
                frame.result = Some(false);
            }
            match self.total_tics {
                40 => {
                    self.spotlight_shown = true;
                    self.spotlight_fade = Some(255);
                    frame.event = Some(Event::Spotlight);
                }
                60 => {
                    self.room_shown = true;
                    self.prompt_shown = true;
                    self.room_fade_in = Some(255);
                    sound::play_fgm(id::nSYAudioVoiceAnnounceContinue);
                    frame.event = Some(Event::Continue);
                }
                120 => {
                    self.options_shown = true;
                    frame.event = Some(Event::Options);
                }
                _ => {}
            }
        }
        if self.status == Status::GameOver {
            self.game_over_color_step = (self.game_over_color_step + 0.01).min(1.0);
            if self.game_over_scale > 0.01 {
                self.game_over_scale = (self.game_over_scale - 0.01).max(0.01);
                self.fighter_y_offset += 3.0;
            }
        }
        self.finished = frame.result.is_some();
        frame
    }

    pub fn draw(&mut self) {
        if let Some(alpha) = &mut self.room_fade_in {
            *alpha = alpha.saturating_sub(5);
        }
        if let Some(alpha) = &mut self.spotlight_fade {
            *alpha = alpha.saturating_sub(5);
        }
        if let Some(alpha) = &mut self.room_fade_out {
            *alpha = alpha.saturating_add(5);
        }
    }
}
