//! Dream Land: Whispy Woods' wind (`grpupupu.c`).
//!
//! Whispy's state machine waits on the `anim_frame` of its four stage
//! objects: eyes, mouth, and the back and front flower beds. The front bed's
//! loop is what pushes fighters.

use super::{top_n, StageAnim, StageObj, StageObjects};
use crate::fighter::Fighter;
use crate::rng;
use ssb_engine::math::Vec3;

/// `GRPUPUPU_WHISPY_*` (`grvars.h`), US values.
pub const BLINK_WAIT_BASE: i32 = 30;
pub const BLINK_WAIT_RANDOM: i32 = 270;
pub const WAIT_DURATION_BASE: i32 = 960;
pub const WAIT_DURATION_RANDOM: i32 = 1140;
pub const WIND_DURATION_BASE: i32 = 240;
pub const WIND_DURATION_RANDOM: i32 = 80;
pub const WIND_RUMBLE_WAIT: u8 = 18;
pub const WHISPY_POS_X: f32 = -525.0;
pub const WIND_VEL_BASE: f32 = 6.0;
pub const WIND_DIST_DECAY: f32 = 0.0006;
pub const WINDBOX_TOP: f32 = 1000.0;
pub const WINDBOX_BOTTOM: f32 = -10.0;
pub const WINDBOX_EDGE_LEFT: f32 = -2325.0;
pub const WINDBOX_EDGE_RIGHT: f32 = 2275.0;

/// `grPupupuWhispyWindStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindStatus {
    Sleep,
    Wait,
    Turn,
    Open,
    Blow,
    Stop,
}

/// `grPupupuWhispyMouthStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouthAnim {
    Stretch,
    Turn,
    Open,
    Close,
}

/// `grPupupuWhispyEyesStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EyesAnim {
    Turn,
    Blink,
}

/// `grPupupuFlowerStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowerStatus {
    Default,
    WindStart,
    WindLoopStart,
    WindLoop,
    WindLoopEnd,
    WindStop,
}

/// `GRCommonGroundVarsPupupu`. The `-1` "no change" markers are `None`.
#[derive(Debug, Clone, PartialEq)]
pub struct Pupupu {
    pub wind_wait: u16,
    pub wind_duration: u16,
    pub blink_wait: i16,
    pub status: WindStatus,
    pub flowers_back_wait: u8,
    pub flowers_front_wait: u8,
    pub rumble_wait: u8,
    /// 0: the wind blows left; 1: right.
    pub lr_players: u8,
    pub flowers_back_status: FlowerStatus,
    pub flowers_front_status: FlowerStatus,
    pub eyes_anim: Option<EyesAnim>,
    pub mouth_anim: Option<MouthAnim>,
    /// `whispy_mouth_texture`, played on the back flowers.
    pub flowers_back_anim: Option<u8>,
    /// `whispy_eyes_texture`, played on the front flowers.
    pub flowers_front_anim: Option<u8>,
}

impl Pupupu {
    /// `grPupupuInitAll` @ 0x8010658C.
    pub fn new() -> Self {
        let wind_wait = (rng::rand_int_range(WAIT_DURATION_RANDOM) + WAIT_DURATION_BASE) as u16;
        let blink_wait = (rng::rand_int_range(BLINK_WAIT_RANDOM) + BLINK_WAIT_BASE) as i16;
        Pupupu {
            wind_wait,
            wind_duration: 0,
            blink_wait,
            status: WindStatus::Sleep,
            flowers_back_wait: 15,
            flowers_front_wait: 22,
            rumble_wait: 0,
            lr_players: 1,
            flowers_back_status: FlowerStatus::Default,
            flowers_front_status: FlowerStatus::Default,
            eyes_anim: None,
            mouth_anim: None,
            flowers_back_anim: None,
            flowers_front_anim: None,
        }
    }

    /// `grPupupuProcUpdate` @ 0x80106490.
    pub fn tick(
        &mut self,
        fighters: &mut [&mut Fighter],
        objects: &mut dyn StageObjects,
        started: bool,
    ) {
        self.update_whispy(fighters, objects, started);
        self.update_flowers_back(objects);
        self.update_flowers_front(fighters, objects);
        self.update_anims(objects);
    }

    fn done(objects: &dyn StageObjects, obj: StageObj) -> bool {
        objects.anim_frame(obj) <= 0.0
    }

    /// `grPupupuWhispyGetLR`: -1 on a tie.
    fn players_lr(fighters: &[&mut Fighter]) -> i8 {
        let (mut right, mut left) = (0, 0);
        for f in fighters.iter() {
            if top_n(f).x > WHISPY_POS_X {
                right += 1;
            } else {
                left += 1;
            }
        }
        if right == left {
            -1
        } else if right < left {
            0
        } else {
            1
        }
    }

    /// `grPupupuUpdateWhispyStatus` @ 0x80105EF4.
    fn update_whispy(
        &mut self,
        fighters: &[&mut Fighter],
        objects: &dyn StageObjects,
        started: bool,
    ) {
        match self.status {
            WindStatus::Sleep => {
                if started {
                    self.status = WindStatus::Wait;
                }
            }
            WindStatus::Wait => {
                if self.wind_wait != 0 {
                    self.wind_wait -= 1;
                } else {
                    let lr = match Self::players_lr(fighters) {
                        -1 => self.lr_players,
                        lr => lr as u8,
                    };
                    if lr != self.lr_players {
                        self.eyes_anim = Some(EyesAnim::Turn);
                        self.mouth_anim = Some(MouthAnim::Turn);
                        self.status = WindStatus::Turn;
                        self.lr_players = lr;
                    } else {
                        self.mouth_anim = Some(MouthAnim::Open);
                        self.status = WindStatus::Open;
                    }
                }
            }
            WindStatus::Turn => {
                if Self::done(objects, StageObj::WhispyMouth) {
                    self.mouth_anim = Some(MouthAnim::Open);
                    self.status = WindStatus::Open;
                }
            }
            WindStatus::Open => {
                if Self::done(objects, StageObj::WhispyMouth) {
                    self.status = WindStatus::Blow;
                    self.flowers_back_status = FlowerStatus::WindStart;
                    self.flowers_front_status = FlowerStatus::WindStart;
                    self.wind_duration =
                        (rng::rand_int_range(WIND_DURATION_RANDOM) + WIND_DURATION_BASE) as u16;
                    self.rumble_wait = 0;
                }
            }
            WindStatus::Blow => {
                self.wind_duration = self.wind_duration.wrapping_sub(1);
                if self.wind_duration == 0 {
                    self.mouth_anim = Some(MouthAnim::Close);
                    self.flowers_back_status = FlowerStatus::WindLoopEnd;
                    self.flowers_front_status = FlowerStatus::WindLoopEnd;
                    self.status = WindStatus::Stop;
                }
                // `grPupupuWhispyUpdateWindRumble`: only the quake effect.
                if self.rumble_wait == 0 {
                    self.rumble_wait = WIND_RUMBLE_WAIT;
                }
                self.rumble_wait -= 1;
            }
            WindStatus::Stop => {
                if Self::done(objects, StageObj::WhispyMouth) {
                    self.wind_wait =
                        (rng::rand_int_range(WAIT_DURATION_RANDOM) + WAIT_DURATION_BASE) as u16;
                    self.status = WindStatus::Wait;
                }
            }
        }
        // `grPupupuWhispyUpdateBlink` @ 0x80105E34.
        if self.eyes_anim.is_none() && Self::done(objects, StageObj::WhispyEyes) {
            self.blink_wait -= 1;
            if self.blink_wait == 0 || self.blink_wait == -10 {
                self.eyes_anim = Some(EyesAnim::Blink);
                if Self::done(objects, StageObj::WhispyMouth) && self.status != WindStatus::Blow {
                    self.mouth_anim = Some(MouthAnim::Stretch);
                }
                if self.blink_wait != 0 {
                    self.blink_wait =
                        (rng::rand_int_range(BLINK_WAIT_RANDOM) + BLINK_WAIT_BASE) as i16;
                }
            }
        }
    }

    /// `grPupupuFlowersBackUpdateAll` @ 0x80106044.
    fn update_flowers_back(&mut self, objects: &dyn StageObjects) {
        match self.flowers_back_status {
            FlowerStatus::WindStart => {
                self.flowers_back_wait = self.flowers_back_wait.wrapping_sub(1);
                if self.flowers_back_wait == 0 {
                    self.flowers_back_anim = Some(0);
                    self.flowers_back_status = FlowerStatus::WindLoopStart;
                }
            }
            FlowerStatus::WindLoopStart => {
                if Self::done(objects, StageObj::FlowersBack) {
                    self.flowers_back_anim = Some(1);
                    self.flowers_back_status = FlowerStatus::WindLoop;
                    self.flowers_back_wait = 15;
                }
            }
            FlowerStatus::WindLoopEnd => {
                self.flowers_back_wait = self.flowers_back_wait.wrapping_sub(1);
                if self.flowers_back_wait == 0 {
                    self.flowers_back_anim = Some(2);
                    self.flowers_back_status = FlowerStatus::WindStop;
                    self.flowers_back_wait = 15;
                }
            }
            _ => {}
        }
    }

    /// `grPupupuFlowersFrontUpdateAll` @ 0x80106290.
    fn update_flowers_front(&mut self, fighters: &mut [&mut Fighter], objects: &dyn StageObjects) {
        match self.flowers_front_status {
            FlowerStatus::WindStart => {
                self.flowers_front_wait = self.flowers_front_wait.wrapping_sub(1);
                if self.flowers_front_wait == 0 {
                    self.flowers_front_anim = Some(0);
                    self.flowers_front_status = FlowerStatus::WindLoopStart;
                }
            }
            FlowerStatus::WindLoopStart => {
                if Self::done(objects, StageObj::FlowersFront) {
                    self.flowers_front_anim = Some(1);
                    self.flowers_front_status = FlowerStatus::WindLoop;
                    self.flowers_front_wait = 22;
                }
            }
            FlowerStatus::WindLoop => self.push(fighters),
            FlowerStatus::WindLoopEnd => {
                self.flowers_front_wait = self.flowers_front_wait.wrapping_sub(1);
                if self.flowers_front_wait == 0 {
                    self.flowers_front_anim = Some(2);
                    self.flowers_front_status = FlowerStatus::WindStop;
                    self.flowers_front_wait = 22;
                } else {
                    self.push(fighters);
                }
            }
            _ => {}
        }
    }

    /// `grPupupuWhispySetWindPush` @ 0x8010595C.
    pub fn push(&self, fighters: &mut [&mut Fighter]) {
        let blow_right = self.lr_players != 0;
        for f in fighters.iter_mut() {
            let pos = top_n(f);
            if pos.y > WINDBOX_TOP || pos.y < WINDBOX_BOTTOM {
                continue;
            }
            let inside = if blow_right {
                pos.x <= WINDBOX_EDGE_RIGHT && pos.x >= WHISPY_POS_X
            } else {
                pos.x >= WINDBOX_EDGE_LEFT && pos.x <= WHISPY_POS_X
            };
            if !inside {
                continue;
            }
            let dist = (pos.x - WHISPY_POS_X).abs();
            let push = WIND_VEL_BASE - dist * WIND_DIST_DECAY;
            if push > 0.0 {
                // `ftParamSetVelPush` replaces the vector.
                f.hazard.vel_push = Vec3::new(if blow_right { push } else { -push }, 0.0, 0.0);
            }
        }
    }

    /// `grPupupuUpdateGObjAnims` @ 0x80106314.
    fn update_anims(&mut self, objects: &mut dyn StageObjects) {
        let lr = self.lr_players;
        if let Some(status) = self.eyes_anim.take() {
            objects.play(StageAnim::WhispyEyes { lr, status });
        }
        if let Some(status) = self.mouth_anim.take() {
            objects.play(StageAnim::WhispyMouth { lr, status });
        }
        if let Some(phase) = self.flowers_back_anim.take() {
            objects.play(StageAnim::FlowersBack { lr, phase });
        }
        if let Some(phase) = self.flowers_front_anim.take() {
            objects.play(StageAnim::FlowersFront { lr, phase });
        }
    }

    /// Whether the wind is pushing this frame.
    pub fn is_blowing(&self) -> bool {
        matches!(
            self.flowers_front_status,
            FlowerStatus::WindLoop | FlowerStatus::WindLoopEnd
        )
    }
}

impl Default for Pupupu {
    fn default() -> Self {
        Self::new()
    }
}
