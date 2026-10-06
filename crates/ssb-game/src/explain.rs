//! `sc/sccommon/scexplain.c`: How to Play, the title's first attract demo.
//!
//! Mario and Luigi (`nFTPlayerKindGameKey`, [`crate::key`]) fight on the
//! How to Play stage under input scripts while a window below the battle
//! walks through 22 phases (`SCExplainPhase`, file `SCExplainMain`): a
//! caption, the control stick in one of six states, a tap spark, the
//! special move's colour overlay and up to six button and marker sprites.
//! Phase 16's end drops a Fire Flower. After the last phase the scene goes
//! on to Characters' demo; A, B or START go back to the title.
//!
//! [`Explain`] is the scene's interface GObjs: the phase process
//! (`scExplainSceneInterfaceProcUpdate`), the stick's
//! (`scExplainProcUpdateControlStickSprite`) and the spark's
//! (`scExplainTapSparkProcUpdate`). The stick's and spark's material
//! animations are played by the host through [`Anims`]; the stick's frame
//! decides when the spark fires.

use alloc::vec::Vec;

use crate::menu::Scene;
use ssb_engine::math::Vec3;

/// `llSCExplainGraphicsFileID` and `llSCExplainMainFileID`.
pub const FILE_GRAPHICS: u32 = 0xC6;
pub const FILE_MAIN: u32 = 0xFC;

/// `llSCExplainMain{0,1,2,3}KeyEvent`: the four players' input scripts.
pub const KEY_EVENTS: [u32; 4] = [0x0, 0x9D4, 0x13FC, 0x1400];
/// `llSCExplainMainExplainPhase`.
pub const PHASES: u32 = 0x1404;
/// `sizeof(SCExplainPhase)`.
pub const PHASE_SIZE: usize = 0x2C;
/// The table's entries; [`Explain`] ends after the last.
pub const PHASE_COUNT: usize = 22;

/// `scExplainSetPhaseSObjs`' sprites (`reloc_data.us.h`): the caption's
/// first, then `phase_sobj0` to `phase_sobj5`.
pub mod sprite {
    pub const TAP_THE_STICK: u32 = 0x11F60;
    pub const A_BUTTON: u32 = 0x1D338;
    pub const B_BUTTON: u32 = 0x1D948;
    pub const Z_BUTTON: u32 = 0x1DF58;
    pub const HERE_TEXT: u32 = 0x9628;
    pub const PLUS_SYMBOL: u32 = 0x1E018;
    /// `phase_sobj0..5`.
    pub const PHASE: [u32; 6] = [
        A_BUTTON,
        B_BUTTON,
        Z_BUTTON,
        HERE_TEXT,
        PLUS_SYMBOL,
        PLUS_SYMBOL,
    ];
}

/// The graphics file's material animations and their textures
/// (`reloc_data.us.h`).
pub mod graphics {
    /// `dSCExplainStickMatAnimJoints`, by stick status (index 0 unused):
    /// neutral, neutral with the arrows, hold up, tap up, hold forward, tap
    /// forward. Each is an `AObjEvent32**` whose first word points at the
    /// script.
    pub const STICK_MAT_ANIM_JOINTS: [u32; 7] = [0, 0x5390, 0x5390, 0x53C0, 0x53F0, 0x5430, 0x5450];
    /// `llSCExplainGraphicsTapSparkMatAnimJoint`.
    pub const TAP_SPARK_MAT_ANIM_JOINT: u32 = 0x5C20;
    /// The stick's `MObjSub` sprites array: five IA8 64 x 64 frames,
    /// indexed by `texture_id_curr`.
    pub const STICK_TEXTURES: [u32; 5] = [0x4028, 0x3020, 0x2018, 0x1010, 0x8];
    /// The spark's: three I4 32 x 32 frames, mirrored on both axes.
    pub const SPARK_TEXTURES: [u32; 3] = [0x5898, 0x5690, 0x5488];
    /// `llSCExplainGraphicsSpecialMoveRGBDisplayList`'s CI4 16 x 48 texture
    /// and its palette.
    pub const RGB_TEXTURE: u32 = 0x5C58;
    pub const RGB_PALETTE: u32 = 0x5DE0;
    /// The animations' end: the host keeps the file's bytes below it.
    pub const ANIM_END: u32 = 0x5C48;
    /// The stick's quad (`0x50C8`): 30 x 30 around the DObj, the texture's
    /// 64 texels across it.
    pub const STICK_HALF: f32 = 15.0;
    /// The arrows' child DObj (`0x5108`, at z 0.3): four red triangles,
    /// right, up, down and left, around the stick.
    pub const ARROWS: [[[f32; 2]; 3]; 4] = [
        [[17.0, 0.0], [11.0, -6.0], [11.0, 6.0]],
        [[0.0, 17.0], [6.0, 11.0], [-6.0, 11.0]],
        [[0.0, -17.0], [-6.0, -11.0], [6.0, -10.0]],
        [[-17.0, 0.0], [-11.0, 6.0], [-11.0, -6.0]],
    ];
    /// `G_SETPRIMCOLOR` alpha 0xFF over the shade's red.
    pub const ARROW_COLOR: [u8; 4] = [0xFF, 0x00, 0x00, 0xFF];
    /// The spark's quad (`0x5B28`): 22 x 22, the 32-texel frame then its
    /// mirror image across each axis; yellow shade through the texel.
    pub const SPARK_HALF: f32 = 11.0;
    pub const SPARK_COLOR: [u8; 4] = [0xFF, 0xFF, 0x00, 0xFF];
    /// The colour overlay's quad (`0x5E00`): x -27..18, y -18..18, its 16
    /// texels across 12 pixels and the last column clamped over the rest
    /// (s 0..60), at primitive alpha 0x80.
    pub const RGB_RECT: [f32; 4] = [-27.0, -18.0, 18.0, 18.0];
    pub const RGB_S_MAX: f32 = 60.0;
    pub const RGB_ALPHA: u8 = 0x80;
}

/// `dSCExplainInterfacePositions` and `scExplainSetPlayerInterfacePositions`'
/// `player_pos_y`.
pub const INTERFACE_POSITIONS_X: [i32; 4] = [55, 125, 195, 265];
pub const INTERFACE_POSITION_Y: i32 = 150;
/// `gmCameraSetViewportDimensions(10, 10, 310, 160)`: the battle above the
/// window.
pub const BATTLE_VIEWPORT: [f32; 4] = [10.0, 10.0, 310.0, 160.0];
/// `scExplainWindowProcDisplay`'s black fill and the text camera's
/// viewport.
pub const WINDOW: [f32; 4] = [10.0, 160.0, 310.0, 230.0];
/// `scExplainSetBattleState`: Mario and Luigi.
pub const FIGHTERS: [crate::fighter::FighterKind; 2] = [
    crate::fighter::FighterKind::Mario,
    crate::fighter::FighterKind::Luigi,
];
/// The How to Play stage, `nGRKindExplain`.
pub const GKIND: u8 = 11;
/// `lbFadeMakeActor(..., 12, ...)`: the scene fades in from black.
pub const FADE_LENGTH: u32 = 12;
/// `dSCExplainRandomSeed1`: the scene's update runs on its own seed,
/// starting at 1 each time the overlay loads.
pub const RANDOM_SEED: i32 = 1;
/// `scExplainTryMakeFireFlower`: when phase 16 ends, a Fire Flower at
/// this position, moving up.
pub const FIRE_FLOWER_PHASE: i32 = 16;
pub const FIRE_FLOWER_POS: Vec3 = Vec3::new(-1400.0, 1500.0, 0.0);
pub const FIRE_FLOWER_VEL: Vec3 = Vec3::new(0.0, 10.0, 0.0);
/// `nITKindFFlower`.
pub const FIRE_FLOWER_KIND: u8 = 12;

/// `SCExplainArgs`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Args {
    pub x: u16,
    pub y: u8,
    pub status: u8,
}

impl Args {
    fn read(b: &[u8]) -> Args {
        Args {
            x: u16::from_be_bytes([b[0], b[1]]),
            y: b[2],
            status: b[3],
        }
    }

    /// Where the window's origin puts this: `x + 10`, `y + 160`.
    pub fn screen(self) -> (f32, f32) {
        (f32::from(self.x) + 10.0, f32::from(self.y) + 160.0)
    }
}

/// `SCExplainPhase`, with `sprite` the caption's offset in the graphics
/// file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Phase {
    pub time: u16,
    pub textbox_x: u8,
    pub textbox_y: u8,
    pub sprite: u32,
    pub stick: Args,
    /// `phase_args0` to `phase_args5`.
    pub args: [Args; 6],
    pub rgb: Args,
}

impl Phase {
    /// One table entry; the host has replaced the sprite pointer with its
    /// offset in the graphics file.
    pub fn read(b: &[u8]) -> Option<Phase> {
        let b = b.get(..PHASE_SIZE)?;
        let at = |o: usize| Args::read(&b[o..o + 4]);
        Some(Phase {
            time: u16::from_be_bytes([b[0], b[1]]),
            textbox_x: b[4],
            textbox_y: b[5],
            sprite: u32::from_be_bytes([b[8], b[9], b[10], b[11]]),
            stick: at(0xC),
            args: [at(0x10), at(0x14), at(0x18), at(0x1C), at(0x20), at(0x28)],
            rgb: at(0x24),
        })
    }

    /// The whole table.
    pub fn read_all(b: &[u8]) -> Option<Vec<Phase>> {
        (0..PHASE_COUNT)
            .map(|i| Phase::read(b.get(i * PHASE_SIZE..)?))
            .collect()
    }
}

/// The stick's and spark's material animations, which the host plays
/// (`gcAddAnimAll` and `gcPlayAnimAll` on their `MObj`s).
pub trait Anims {
    /// `gcAddAnimAll(stick, dSCExplainStickMatAnimJoints[status], 0)` and
    /// its first `gcPlayAnimAll`.
    fn start_stick(&mut self, status: u8);
    /// `gcPlayAnimAll` on the stick; returns its `mobj->anim_frame`, or
    /// `None` before any animation was added.
    fn play_stick(&mut self) -> Option<f32>;
    /// `gcAddAnimAll(spark, llSCExplainGraphicsTapSparkMatAnimJoint, 0)`
    /// and its first play.
    fn start_spark(&mut self);
    /// `gcPlayAnimAll` on the spark; returns whether it still runs
    /// (`anim_wait != AOBJ_ANIM_NULL`).
    fn play_spark(&mut self) -> bool;
}

/// The stick (`scExplainMakeControlStickInterface`), drawn when `shown`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Stick {
    pub shown: bool,
    /// The child DObj's arrows (stick status 2).
    pub arrows: bool,
    /// The DObj's centre in N64 screen pixels.
    pub x: f32,
    pub y: f32,
}

/// What a frame asks of the scene.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Tick {
    /// `syTaskmanSetLoadScene` with this scene.
    pub load: Option<Scene>,
    /// `scExplainTryMakeFireFlower`.
    pub fire_flower: bool,
}

/// The scene's interface GObjs and `sSCExplainStruct`.
#[derive(Debug, Clone, PartialEq)]
pub struct Explain {
    phases: Vec<Phase>,
    /// `sSCExplainPhase`: the next entry.
    next: usize,
    pub phase: i32,
    advance_wait: i32,
    /// `stick_status`.
    pub stick_status: u8,
    /// The caption (`textbox_sobj`): its graphics-file sprite and top-left,
    /// or `None` while hidden.
    pub textbox: Option<(u32, f32, f32)>,
    /// `phase_sobj0..5`'s top-left when shown ([`sprite::PHASE`]).
    pub sobjs: [Option<(f32, f32)>; 6],
    pub stick: Stick,
    /// The spark's centre when shown.
    pub spark: Option<(f32, f32)>,
    /// The colour overlay's centre when shown.
    pub rgb: Option<(f32, f32)>,
}

impl Explain {
    /// `scExplainSetInterfaceGObjs`, `scExplainSetPhaseSObjs` and
    /// `scExplainMakeSceneInterface`, which runs its process once: the first
    /// phase is set up before the scene's first frame.
    pub fn new(phases: Vec<Phase>, anims: &mut dyn Anims) -> (Explain, Tick) {
        let mut e = Explain {
            phases,
            next: 0,
            phase: 0,
            advance_wait: 0,
            stick_status: 0,
            textbox: None,
            sobjs: [None; 6],
            stick: Stick::default(),
            spark: None,
            rgb: None,
        };
        let tick = e.interface_update(false, anims);
        (e, tick)
    }

    /// One frame of the scene's processes, after the battle's: the stick's,
    /// the spark's, then the phases'. `tapped` is A, B or START on any
    /// controller.
    pub fn tick(&mut self, tapped: bool, anims: &mut dyn Anims) -> Tick {
        // `scExplainProcUpdateControlStickSprite`.
        if anims.play_stick() == Some(15.0) && matches!(self.stick_status, 4 | 6) {
            self.update_tap_spark(anims);
        }
        // `scExplainTapSparkProcUpdate`.
        if !anims.play_spark() {
            self.spark = None;
        }
        self.interface_update(tapped, anims)
    }

    /// `scExplainUpdateTapSparkEffect`.
    fn update_tap_spark(&mut self, anims: &mut dyn Anims) {
        // The DObj's translation is in the stick camera's ortho units, y up.
        let (dx, dy) = if self.stick_status == 4 {
            (5.0, 15.0)
        } else {
            (15.0, 5.0)
        };
        self.spark = Some((self.stick.x + dx, self.stick.y - dy));
        anims.start_spark();
    }

    /// `scExplainSceneInterfaceProcUpdate`: `scExplainDetectExit`, then
    /// `scExplainUpdatePhase`.
    fn interface_update(&mut self, tapped: bool, anims: &mut dyn Anims) -> Tick {
        let mut tick = Tick::default();
        if tapped {
            tick.load = Some(Scene::Title);
        }
        if self.advance_wait == 0 {
            tick.fire_flower = self.phase == FIRE_FLOWER_PHASE;
            self.phase += 1;
            if self.phase > PHASE_COUNT as i32 {
                tick.load = Some(Scene::Characters);
                return tick;
            }
            let Some(p) = self.phases.get(self.next).copied() else {
                tick.load = Some(Scene::Characters);
                return tick;
            };
            // `scExplainUpdateTextBoxSprite`.
            self.textbox = Some((
                p.sprite,
                f32::from(p.textbox_x) + 10.0,
                f32::from(p.textbox_y) + 160.0,
            ));
            self.update_stick(p, anims);
            // `scExplainHideTapSpark`.
            self.spark = None;
            // `scExplainSpecialMoveRGBProcUpdate`: over the stick.
            self.rgb = (p.rgb.status == 1).then(|| p.stick.screen());
            // `scExplainUpdateArgsSObj`.
            for (sobj, args) in self.sobjs.iter_mut().zip(p.args) {
                *sobj = (args.status == 1).then(|| args.screen());
            }
            self.advance_wait = i32::from(p.time);
            self.next += 1;
        }
        self.advance_wait -= 1;
        tick
    }

    /// `func_ovl63_8018DDBC`: the stick's status, place and animation.
    fn update_stick(&mut self, p: Phase, anims: &mut dyn Anims) {
        let sw = p.stick.status;
        if sw == 0 {
            self.stick.shown = false;
        } else {
            let (x, y) = p.stick.screen();
            self.stick = Stick {
                shown: true,
                arrows: sw == 2,
                x,
                y,
            };
            anims.start_stick(sw);
        }
        self.stick_status = sw;
    }

    /// The phases' total length: the scene's frames when nothing exits it.
    pub fn total_time(&self) -> u32 {
        self.phases.iter().map(|p| u32::from(p.time)).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Clock {
        stick: Option<(u8, f32)>,
        spark: Option<u32>,
        sparks: u32,
    }

    impl Anims for Clock {
        fn start_stick(&mut self, status: u8) {
            self.stick = Some((status, 0.0));
        }
        fn play_stick(&mut self) -> Option<f32> {
            let (status, frame) = self.stick.as_mut()?;
            // Tap up loops every 24 frames.
            *frame = if *status == 4 {
                (*frame + 1.0) % 24.0
            } else {
                *frame + 1.0
            };
            Some(*frame)
        }
        fn start_spark(&mut self) {
            self.spark = Some(0);
            self.sparks += 1;
        }
        fn play_spark(&mut self) -> bool {
            match self.spark.as_mut() {
                Some(t) if *t < 7 => {
                    *t += 1;
                    true
                }
                _ => false,
            }
        }
    }

    fn phases() -> Vec<Phase> {
        (0..PHASE_COUNT)
            .map(|i| Phase {
                time: 10,
                sprite: 0x1000 + i as u32,
                stick: Args {
                    x: 257,
                    y: 36,
                    status: if i == 2 { 4 } else { 0 },
                },
                args: [Args {
                    x: 245,
                    y: 18,
                    status: u8::from(i == 3),
                }; 6],
                ..Phase::default()
            })
            .collect()
    }

    #[test]
    fn the_phases_run_in_order_then_characters() {
        let mut anims = Clock::default();
        let (mut e, first) = Explain::new(phases(), &mut anims);
        assert_eq!(first, Tick::default());
        assert_eq!(e.phase, 1);
        assert_eq!(e.textbox, Some((0x1000, 10.0, 160.0)));
        let mut frames = 0;
        let mut flowers = 0;
        loop {
            frames += 1;
            let t = e.tick(false, &mut anims);
            flowers += u32::from(t.fire_flower);
            if let Some(scene) = t.load {
                assert_eq!(scene, Scene::Characters);
                break;
            }
        }
        // 22 phases of 10 frames, the first set up before the first frame.
        assert_eq!(frames, 22 * 10);
        assert_eq!(flowers, 1);
    }

    #[test]
    fn a_tap_up_fires_the_spark_on_frame_15() {
        let mut anims = Clock::default();
        let mut p = phases();
        p[0].stick.status = 4;
        p[0].time = 100;
        let (mut e, _) = Explain::new(p, &mut anims);
        assert!(e.stick.shown);
        assert_eq!((e.stick.x, e.stick.y), (267.0, 196.0));
        for _ in 0..14 {
            e.tick(false, &mut anims);
        }
        assert_eq!(e.spark, None);
        e.tick(false, &mut anims);
        assert_eq!(e.spark, Some((272.0, 181.0)));
        assert_eq!(anims.sparks, 1);
    }

    #[test]
    fn a_button_returns_to_the_title() {
        let mut anims = Clock::default();
        let (mut e, _) = Explain::new(phases(), &mut anims);
        assert_eq!(e.tick(true, &mut anims).load, Some(Scene::Title));
    }

    #[test]
    fn the_phase_table_layout() {
        let mut b = [0u8; PHASE_SIZE];
        b[0..2].copy_from_slice(&180u16.to_be_bytes());
        b[4] = 20;
        b[5] = 11;
        b[8..12].copy_from_slice(&0x11F60u32.to_be_bytes());
        b[0xC..0x10].copy_from_slice(&[0x01, 0x01, 26, 6]);
        b[0x24..0x28].copy_from_slice(&[0x00, 237, 36, 1]);
        b[0x28..0x2C].copy_from_slice(&[0x00, 253, 33, 1]);
        let p = Phase::read(&b).unwrap();
        assert_eq!(
            (p.time, p.textbox_x, p.textbox_y, p.sprite),
            (180, 20, 11, 0x11F60)
        );
        assert_eq!(
            p.stick,
            Args {
                x: 257,
                y: 26,
                status: 6
            }
        );
        assert_eq!(
            p.rgb,
            Args {
                x: 237,
                y: 36,
                status: 1
            }
        );
        assert_eq!(
            p.args[5],
            Args {
                x: 253,
                y: 33,
                status: 1
            }
        );
    }
}
