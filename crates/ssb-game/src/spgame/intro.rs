//! `sc1PIntroFuncRun`, announcement and fighter entrance processes.
use super::Stage;
use crate::fighter::FighterKind;
use crate::sound::{self, id};
use ssb_engine::input::N64Buttons;

/// `sc1PIntroMakeStageCamera` and `sc1PIntroMakeFighterCamera`'s
/// projection `(aspect, near, far)`; near and far are the functions' own
/// immediates. Neither sets `persp.aspect`, so it stays the generated
/// default ([`crate::results_scene::PERSP_ASPECT`], 4:3) over the 300 x 220
/// viewport (N64 RDRAM: both cameras 1.3333, RE-458).
pub const CAMERA_PROJECTION: (f32, f32, f32) = (crate::results_scene::PERSP_ASPECT, 128.0, 16384.0);

/// How `ftDisplayMain` lights an intro fighter.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FighterLight {
    /// `ftDisplayLightsDrawReflect` from the angles, in degrees.
    Reflect([f32; 2]),
    /// The scene's `dSC1PIntroLights1`, which `sc1PIntroFuncLights` loads
    /// before anything draws: its direction's signed bytes (RE-458).
    Scene([i8; 3]),
}

/// `scSubsysFighterSetLightParams(-20, 30)` in `sc1PIntroFuncStart`: the
/// light every `nFTPlayerKindDemo` fighter's `ftDisplayMain` reloads after
/// drawing itself.
pub const RESTORED_LIGHT: [f32; 2] = [-20.0, 30.0];

/// Master Hand's `colanim.light_angle` (`sc1PIntroMakeVSFighter`), the
/// only intro fighter with `colanim.is_use_light`.
pub const BOSS_LIGHT: [f32; 2] = [-20.0, -30.0];

/// The RSP's light through one intro frame's fighters, in draw order.
///
/// The frame starts with `dSC1PIntroLights1`. `ftDisplayMain` loads a
/// fighter's own light only with `colanim.is_use_light`, and after drawing
/// any Demo fighter reloads [`RESTORED_LIGHT`]: so only the first fighter
/// drawn takes the scene light, every later one the restored light (N64
/// RDRAM poke of the scene light moved the player only, RE-458).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FighterLights {
    restored: bool,
}

impl FighterLights {
    /// The light the next fighter drawn this frame takes.
    pub fn next(&mut self, kind: FighterKind) -> FighterLight {
        let light = if kind == FighterKind::Boss {
            FighterLight::Reflect(BOSS_LIGHT)
        } else if self.restored {
            FighterLight::Reflect(RESTORED_LIGHT)
        } else {
            // `Lights1`: `Ambient` (8 bytes), then `Light` (`col`, pad,
            // `colc`, pad, `dir`, pad).
            let b = crate::d_arrays::SC_1P_INTRO_LIGHTS1;
            FighterLight::Scene([b[16] as i8, b[17] as i8, b[18] as i8])
        };
        self.restored = true;
        light
    }
}

/// One of `sc1PIntroUpdateAnnounce`'s voices, which [`Intro::tick`] plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Announce {
    Player(FighterKind),
    Versus,
    Opponent(Stage),
    Bonus(Stage),
}

/// `sc1PIntroUpdateAnnounce`'s `vs_fighter_voices`, by stage; the bonus
/// stages' and Master Hand's entries are never read.
const VS_FIGHTER_VOICES: [u16; 14] =
    voices(crate::d_arrays::SC_1P_INTRO_UPDATE_ANNOUNCE_VS_FIGHTER_VOICES);

/// `sc1PIntroUpdateAnnounce`'s `fighter_voices`, by [`FighterKind`].
const FIGHTER_VOICES: [u16; 12] =
    voices(crate::d_arrays::SC_1P_INTRO_UPDATE_ANNOUNCE_FIGHTER_VOICES);

/// The `u32` voice IDs as the `u16` the sound calls take.
const fn voices<const N: usize>(ids: [u32; N]) -> [u16; N] {
    let mut out = [0; N];
    let mut i = 0;
    while i < N {
        assert!(ids[i] <= u16::MAX as u32, "voice ID out of range");
        out[i] = ids[i] as u16;
        i += 1;
    }
    out
}

impl Announce {
    /// The voice `sc1PIntroUpdateAnnounce` plays for it.
    pub fn voice(self) -> Option<u16> {
        match self {
            Announce::Player(kind) => FIGHTER_VOICES.get(kind as usize).copied(),
            Announce::Versus => Some(id::nSYAudioVoiceAnnounceVersus),
            Announce::Opponent(stage) => VS_FIGHTER_VOICES.get(stage as usize).copied(),
            Announce::Bonus(Stage::Bonus1) => Some(id::nSYAudioVoiceAnnounceBreakTheTargets),
            Announce::Bonus(Stage::Bonus2) => Some(id::nSYAudioVoiceAnnounceBoardThePlatforms),
            Announce::Bonus(Stage::Bonus3) => Some(id::nSYAudioVoiceAnnounceRaceToTheFinish),
            Announce::Bonus(_) => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Intro {
    pub stage: Stage,
    pub player: FighterKind,
    pub total_tics: u32,
    announced: [bool; 3],
    finished: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Frame {
    pub announce: [Option<Announce>; 3],
    pub proceed: bool,
}

impl Intro {
    /// `sc1PIntroFuncStart`'s state and its BGM: the boss's card has its
    /// own.
    pub fn new(stage: Stage, player: FighterKind) -> Self {
        assert!(!stage.is_challenger());
        sound::play_bgm(
            0,
            if stage == Stage::Boss {
                id::nSYAudioBGMBossStage
            } else {
                id::nSYAudioBGM1PIntro
            },
        );
        Self {
            stage,
            player,
            total_tics: 0,
            announced: [false; 3],
            finished: false,
        }
    }

    /// Scheduler time and this scene's counter are distinct in the source.
    /// The host supplies the scheduler's tick, rather than inferring it.
    pub fn tick(&mut self, scheduler_tic: u32, taps: N64Buttons) -> Frame {
        let mut frame = Frame::default();
        if self.finished {
            return frame;
        }
        self.total_tics += 1;
        if !matches!(
            self.stage,
            Stage::Bonus1 | Stage::Bonus2 | Stage::Bonus3 | Stage::Boss
        ) {
            // `announce_wait_tics`, by the player's kind.
            let wait = crate::d_arrays::SC_1P_INTRO_UPDATE_ANNOUNCE_ANNOUNCE_WAIT_TICS
                [self.player as usize]
                + 1;
            for (i, (ready, announce)) in [
                (scheduler_tic >= 2, Announce::Player(self.player)),
                (scheduler_tic > wait, Announce::Versus),
                (scheduler_tic > wait + 60, Announce::Opponent(self.stage)),
            ]
            .into_iter()
            .enumerate()
            {
                if ready && !self.announced[i] {
                    frame.announce[i] = Some(announce);
                    self.announced[i] = true;
                }
            }
        } else if self.total_tics == 1 && self.stage != Stage::Boss {
            frame.announce[0] = Some(Announce::Bonus(self.stage));
        }
        for v in frame.announce.iter().flatten().filter_map(|a| a.voice()) {
            sound::play_fgm(v);
        }
        frame.proceed = scheduler_tic >= 60
            && (taps.0 & (N64Buttons::A | N64Buttons::B | N64Buttons::START) != 0
                || scheduler_tic > 360);
        if frame.proceed {
            sound::stop_all_fgm();
        }
        self.finished = frame.proceed;
        frame
    }
}

/// `sc1PIntroFighterProcUpdate` and `VSFighterProcUpdate`'s render state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Entrance {
    pub z: f32,
    pub shown: bool,
    velocity_z: f32,
    delay: Option<u32>,
}

impl Entrance {
    pub fn ally(card: usize) -> Self {
        Self {
            // `sc1PIntroGetFighterPositionZ` and `...VelocityZ`.
            z: crate::d_arrays::SC_1P_INTRO_GET_FIGHTER_POSITION_Z_POS_Z[card],
            shown: true,
            velocity_z: crate::d_arrays::SC_1P_INTRO_GET_FIGHTER_VELOCITY_Z_VEL_Z[card],
            delay: None,
        }
    }

    pub fn opponent(stage: Stage, kind: FighterKind, card: u8) -> Self {
        let delay = match stage {
            Stage::Yoshi => Some(u32::from(card) * 3),
            Stage::Kirby => Some(u32::from(card) * 10),
            _ => None,
        };
        Self {
            z: if matches!(stage, Stage::Yoshi | Stage::Kirby | Stage::Zako) {
                0.0
            } else if stage == Stage::Mario {
                if kind == FighterKind::Mario {
                    -600.0
                } else {
                    -800.0
                }
            } else {
                -500.0
            },
            shown: true,
            velocity_z: if stage == Stage::Mario { 40.0 } else { 30.0 },
            delay,
        }
    }

    /// One of a Polygon's three frozen card frames (`frame` 0 to 2) on the
    /// Fighting Polygon Team's card: drawn once the scene's clock passes
    /// [`polygon_frames`]' threshold (`sc1PIntroVSFighterProcDisplay`).
    pub fn polygon(kind: FighterKind, frame: usize) -> Self {
        let threshold = [-28, -8, 12][frame.min(2)] + kind as i32 * 2;
        Self {
            z: 0.0,
            shown: false,
            velocity_z: 30.0,
            delay: Some(threshold.max(0) as u32),
        }
    }

    pub fn tick(&mut self, scene_tic: u32) {
        if let Some(delay) = self.delay {
            self.shown = delay < scene_tic;
        } else if self.z < 0.0 {
            self.z = (self.z + self.velocity_z).min(0.0);
        }
    }
}

/// Polygon fighters draw three frozen card frames, with separate thresholds.
/// Each ready frame is drawn; this is not a single changing animation frame.
pub fn polygon_frames(kind: FighterKind, scene_tic: u32) -> [bool; 3] {
    let kind = kind as i32;
    [kind * 2 - 28, kind * 2 - 8, kind * 2 + 12]
        .map(|threshold| i64::from(threshold) < i64::from(scene_tic))
}

/// `sc1PIntroGetAlliesNum`: the Mario Bros.' one ally, Giant Donkey Kong's
/// two.
pub fn allies_num(stage: Stage) -> usize {
    match stage {
        Stage::Mario => 1,
        Stage::Donkey => 2,
        _ => 0,
    }
}

/// The allies `sc1PIntroMakeVSName` names: the fighters on the first
/// [`allies_num`] ally ports. On any other stage those ports still hold the
/// last battle's opponents (after Race to the Finish and the Fighting
/// Polygon Team, Polygons, which have no name sprite), and the source never
/// looks them up (RE-478).
pub fn named_allies<'a>(
    stage: Stage,
    data: &'a super::SceneData,
    state: &'a super::BattleState,
) -> impl Iterator<Item = FighterKind> + 'a {
    data.ally_players
        .iter()
        .take(allies_num(stage))
        .map(move |&port| state.players[usize::from(port)].fkind)
}
