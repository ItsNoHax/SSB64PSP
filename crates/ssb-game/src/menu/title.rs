//! `mn/mncommon/mntitle.c`: the title screen (US).
//!
//! The port enters the title as the original does after the N64 logo
//! (`mnStartup`) or a demo, never after the opening movie
//! (`nSCKindOpeningNewcomers`), which is not ported: so the title always
//! takes `nMNTitleLayoutAnimate`, its transitions starting at tic 169 with
//! the fire already lit, the small red logo at rest and no logo animation,
//! slash or logo-fire particles (the opening-only `mnTitleMakeLogo` branch,
//! `mnTitleMakeSlash` and `mnTitleMakeLogoFireParticles`).
//!
//! One [`Title::tick`] is `gcRunAll`: the GObjs' `func_run` in link order
//! (`mnTitleFuncRun`, `mnTitleFireFuncRun`, `mnTitleTransitionsFuncRun`),
//! then the processes in their creation order (the fire camera's colour, the
//! fire's frames, the labels' animation, the header and footer, "Press
//! Start"'s animation). [`Title::visit`] yields camera 100's fill, then
//! camera 60's display links 0 and 1.
//!
//! The labels and "Press Start" follow `DObj` trees their `AnimJoint`
//! scripts move (`ssb_rom::title`); the build plays them and the host
//! passes each play's `[x, y, scale x, scale y]` to [`Title::visit`] through
//! [`Anims`].

use super::{Draw, Pad, Piece, Scene};
use crate::backup::{Backup, CHARACTER_MASK_STARTER};
use crate::fighter::FighterKind;
use ssb_engine::input::N64Buttons;

/// `llMNTitleFileID` and `llMNTitleFireAnimFileID`.
pub const FILE_TITLE: u32 = 0xA7;
pub const FILE_FIRE: u32 = 0xA8;

/// `llMNTitle*Sprite` (`reloc_data.us.h`).
pub mod sprite {
    pub const LOGO_ANIM_FULL: u32 = 0xBBB0;
    pub const BORDER_UPPER: u32 = 0xC208;
    pub const CUTOUT: u32 = 0x11988;
    pub const TM_UNK: u32 = 0x11AA8;
    pub const COPYRIGHT: u32 = 0x15320;
    pub const PRESS_START: u32 = 0x15A48;
    pub const SUPER: u32 = 0x16728;
    pub const SMASH: u32 = 0x245C8;
    pub const BROS: u32 = 0x25188;

    /// `dMNTitleFireSpriteOffsets`: `llMNTitleFireAnimFrame1Sprite` to
    /// `...Frame30Sprite`.
    pub const FIRE: [u32; 30] = [
        0x1018, 0x2078, 0x30D8, 0x4138, 0x5198, 0x61F8, 0x7258, 0x82B8, 0x9318, 0xA378, 0xB3D8,
        0xC438, 0xD498, 0xE4F8, 0xF558, 0x105B8, 0x11618, 0x12678, 0x136D8, 0x14738, 0x15798,
        0x167F8, 0x17858, 0x188B8, 0x19918, 0x1A978, 0x1B9D8, 0x1CA38, 0x1DA98, 0x1EAF8,
    ];
}

/// `dMNTitleCommonSpriteDescs` (US): each kind's centre and sprite.
/// `DropShadow`, `Smash`, `Super`, `Bros` and `TM` are the labels,
/// `Footer` and `Header` the copyright and the upper border.
pub const SPRITE_DESCS: [([f32; 2], u32); 10] = [
    ([157.0, 94.0], sprite::CUTOUT),
    ([161.0, 88.0], sprite::SMASH),
    ([55.0, 96.0], sprite::SUPER),
    ([268.0, 96.0], sprite::BROS),
    ([270.0, 132.0], sprite::TM_UNK),
    ([160.0, 208.0], sprite::COPYRIGHT),
    ([160.0, 15.0], sprite::BORDER_UPPER),
    ([162.0, 177.0], sprite::PRESS_START),
    ([260.0, 60.0], sprite::LOGO_ANIM_FULL),
    ([277.0, 157.0], 0xF398),
];

/// `nMNTitleSpriteKind*` (US).
const KIND_DROP_SHADOW: usize = 0;
const KIND_TM: usize = 4;
const KIND_FOOTER: usize = 5;
const KIND_HEADER: usize = 6;
const KIND_PRESS_START: usize = 7;
const KIND_LOGO: usize = 8;

/// `dMNTitleFireColorsR`, `G` and `B`.
const FIRE_COLORS: [[u8; 3]; 7] = [
    [0xFF, 0xFF, 0xFF],
    [0xFF, 0xF0, 0x9B],
    [0xFF, 0xFF, 0x64],
    [0xFF, 0xD1, 0xD1],
    [0xE6, 0xFF, 0xE6],
    [0xFF, 0xE2, 0xB8],
    [0xFF, 0xD2, 0x94],
];

/// `nMNTitleLayout*`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Layout {
    Opening,
    Animate,
    Final,
}

/// The `gSCManagerSceneData` fields the title and its demos keep.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DemoData {
    /// `is_title_anim_viewed`: START works before the animation ends.
    pub is_title_anim_viewed: bool,
    /// `is_extend_demo_wait`: the demo waits 1190 tics, not 650.
    pub is_extend_demo_wait: bool,
    /// `demo_mask_prev`, `demo_fkind` and `demo_first_fkind`.
    pub demo_mask_prev: u16,
    pub demo_fkind: [FighterKind; 2],
    pub demo_first_fkind: FighterKind,
    /// `demo_gkind_order`: the auto demo's next stage
    /// (`dSCAutoDemoGroundOrder`).
    pub demo_gkind_order: u8,
}

impl Default for DemoData {
    /// `dSCManagerDefaultSceneData`.
    fn default() -> Self {
        DemoData {
            is_title_anim_viewed: false,
            is_extend_demo_wait: false,
            demo_mask_prev: 0,
            demo_fkind: [FighterKind::Mario; 2],
            demo_first_fkind: FighterKind::Mario,
            demo_gkind_order: 0,
        }
    }
}

/// `mnTitleGetFighterKindsNum` (and `scAutoDemoGetFighterKindsNum`).
pub(crate) fn kinds_num(mask: u16) -> i32 {
    mask.count_ones() as i32
}

/// `mnTitleGetShuffledFighterKind`: the `random`th kind of `this_mask` not
/// in `prev_mask` (and `scAutoDemoGetShuffledFighterKind`).
pub(crate) fn shuffled_kind(this_mask: u16, prev_mask: u16, random: i32) -> usize {
    let mut fkind: i32 = -1;
    let mut random = random + 1;
    loop {
        fkind += 1;
        let bit = 1u16.checked_shl(fkind as u32).unwrap_or(0);
        if this_mask & bit != 0 && prev_mask & bit == 0 {
            random -= 1;
        }
        if random == 0 || fkind >= 15 {
            return fkind as usize;
        }
    }
}

pub(crate) fn kind_of(i: usize) -> FighterKind {
    FighterKind::from_ordinal(i as u8).unwrap_or(FighterKind::Mario)
}

impl DemoData {
    /// `mnTitleSetDemoFighterKinds`. `rand_range` is `syUtilsRandIntRange`.
    pub fn set_demo_fighter_kinds(
        &mut self,
        backup: &Backup,
        rand_range: &mut impl FnMut(i32) -> i32,
    ) {
        let unlocked = backup.fighter_mask | CHARACTER_MASK_STARTER;
        if !unlocked & self.demo_mask_prev != 0 {
            self.demo_mask_prev = 0;
        }
        let count = kinds_num(unlocked);
        if count <= kinds_num(self.demo_mask_prev) {
            self.demo_mask_prev = 0;
        }
        let first = shuffled_kind(
            unlocked,
            self.demo_mask_prev,
            rand_range(count - kinds_num(self.demo_mask_prev)),
        );
        self.demo_fkind[0] = kind_of(first);
        if self.demo_mask_prev == 0 {
            self.demo_first_fkind = self.demo_fkind[0];
        }
        self.demo_mask_prev |= 1 << first;
        let fresh = count - kinds_num(self.demo_mask_prev);
        if fresh == 0 {
            self.demo_fkind[1] = self.demo_first_fkind;
        } else {
            let second = shuffled_kind(unlocked, self.demo_mask_prev, rand_range(fresh));
            self.demo_fkind[1] = kind_of(second);
            self.demo_mask_prev |= 1 << second;
        }
    }
}

/// `mnTitleStartScene`: a boot that has not yet seen the title's animation
/// counts, and the backup is written.
pub fn count_boot(demo: &DemoData, backup: &mut Backup) {
    if !demo.is_title_anim_viewed {
        backup.boot = backup.boot.wrapping_add(1);
        backup.write();
    }
}

/// The baked plays of the labels' and "Press Start"'s trees: per play, per
/// child, `[translate x, translate y, scale x, scale y]`.
pub trait Anims {
    fn labels(&self, play: usize, child: usize) -> Option<[f32; 4]>;
    fn press_start(&self, play: usize) -> Option<[f32; 4]>;
}

/// No baked plays: every child stays at its sprite's place.
pub struct NoAnims;

impl Anims for NoAnims {
    fn labels(&self, _: usize, _: usize) -> Option<[f32; 4]> {
        None
    }
    fn press_start(&self, _: usize) -> Option<[f32; 4]> {
        None
    }
}

/// "Press Start"'s script loops back to its second play every 39 plays:
/// 20 shown, 19 at no size (`ssb_rom::title`'s test pins this).
pub const PRESS_START_PERIOD: usize = 39;

/// The press start play to read for its `play`th play.
pub fn press_start_frame(play: usize) -> usize {
    if play == 0 {
        0
    } else {
        1 + (play - 1) % PRESS_START_PERIOD
    }
}

/// `syUtilsRandTimeUCharRange(range)` from a time byte.
fn time_range(byte: u8, range: i32) -> i32 {
    ((f32::from(byte) * range as f32) / 256.0) as i32
}

/// The title's state (`sMNTitle*`).
#[derive(Debug, Clone, PartialEq)]
pub struct Title {
    pub layout: Layout,
    transition_tics: i32,
    is_start_actor_process: bool,
    is_proceed_scene: bool,
    proceed_wait: i32,
    fire_alpha: i32,
    logo_alpha: i32,
    fire_timer: i32,
    fire_color: [f32; 3],
    fire_delta: [f32; 3],
    fire_color_id: usize,
    allow_proceed_wait: i32,
    /// The two fire `SObj`s' `user_data.s`: the frame each shows next.
    fire_frames: [usize; 2],
    /// The fire `SObj`s' sprite this frame.
    fire_shown: [usize; 2],
    labels_shown: bool,
    /// The labels' process runs until `mnTitleSetEndLayout` ends it.
    labels_running: bool,
    labels_play: usize,
    press_start_shown: bool,
    press_start_play: usize,
    /// `mnTitleProceed*`'s black default camera.
    black: bool,
    /// The scene `syTaskmanSetLoadScene` will load.
    scene_next: Scene,
}

impl Title {
    /// `mnTitleFuncStart` after `mnStartup` or a demo
    /// (`mnTitleInitVars`'s `nMNTitleLayoutAnimate`). `time` is
    /// `osGetTime`'s low byte.
    pub fn new(time: u8) -> Title {
        let id = time_range(time, 7) as usize;
        let c = FIRE_COLORS[id];
        Title {
            layout: Layout::Animate,
            transition_tics: 169,
            is_start_actor_process: false,
            is_proceed_scene: false,
            proceed_wait: 3,
            // `mnTitleShowFire`.
            fire_alpha: 0xFF,
            logo_alpha: 0,
            fire_timer: 0,
            fire_color: c.map(f32::from),
            fire_delta: [0.0; 3],
            fire_color_id: id,
            allow_proceed_wait: 0,
            fire_frames: [12, 0],
            fire_shown: [12, 0],
            labels_shown: false,
            labels_running: true,
            labels_play: 0,
            press_start_shown: false,
            press_start_play: 0,
            black: false,
            scene_next: Scene::Title,
        }
    }

    /// `mnTitleProceedDemoNext`: the next demo after the scene the title
    /// was entered from.
    fn proceed_demo_next(
        &mut self,
        scene_prev: Scene,
        demo: &mut DemoData,
        backup: &Backup,
        rand_range: &mut impl FnMut(i32) -> i32,
    ) {
        self.black = true;
        demo.set_demo_fighter_kinds(backup, rand_range);
        self.scene_next = match scene_prev {
            Scene::Explain => Scene::Characters,
            Scene::ModeSelect | Scene::AutoDemo => Scene::Startup,
            _ => Scene::Explain,
        };
        demo.is_extend_demo_wait = true;
        self.is_proceed_scene = true;
    }

    /// `mnTitleProceedModeSelect`.
    fn proceed_mode_select(&mut self) {
        self.black = true;
        self.scene_next = Scene::ModeSelect;
        self.is_proceed_scene = true;
    }

    /// `mnTitleTransitionFromFireLogo`'s `mnTitleUpdateFireVars` (the
    /// opening layout's fire logo is not made).
    fn update_fire_vars(&mut self, time: u8) {
        let id = time_range(time, 7) as usize;
        self.fire_color_id = id;
        self.fire_color = [0.0; 3];
        self.fire_delta = core::array::from_fn(|i| f32::from(FIRE_COLORS[id][i]) / 80.0);
    }

    /// `mnTitleSetAllowProceedWait`.
    fn set_allow_proceed_wait(&mut self) {
        self.allow_proceed_wait = if self.layout == Layout::Final {
            280
        } else {
            364
        };
    }

    /// One frame. `scene_prev` is the scene the title came from; `time`
    /// is `osGetTime`'s low byte; `rand_range` is `syUtilsRandIntRange`.
    /// Returns the scene to load.
    pub fn tick(
        &mut self,
        pad: &Pad,
        scene_prev: Scene,
        demo: &mut DemoData,
        backup: &Backup,
        time: u8,
        rand_range: &mut impl FnMut(i32) -> i32,
    ) -> Option<Scene> {
        let mut load = None;
        // `mnTitleFuncRun`.
        if !self.is_start_actor_process {
            self.is_start_actor_process = true;
        } else if self.is_proceed_scene {
            self.proceed_wait -= 1;
            if self.proceed_wait == 0 {
                load = Some(self.scene_next);
            }
        } else if pad.tap(N64Buttons::A | N64Buttons::B | N64Buttons::START) {
            if self.layout != Layout::Opening {
                // `osResetType` is a cold boot.
                if demo.is_title_anim_viewed && !pad.tap(N64Buttons::B) {
                    self.proceed_mode_select();
                }
            } else {
                self.transition_tics = 169;
                self.layout = Layout::Animate;
                self.update_fire_vars(time);
            }
        }
        // `mnTitleFireFuncRun`.
        self.fire_alpha = (self.fire_alpha + 0x0D).min(0xFF);
        // `mnTitleTransitionsFuncRun`.
        self.transition_tics += 1;
        if self.transition_tics == self.allow_proceed_wait {
            demo.is_title_anim_viewed = true;
        }
        match self.transition_tics {
            111 => self.update_fire_vars(time),
            170 => {
                // `mnTitleSetEndLogoPosition`.
                self.logo_alpha = 0x4C;
                self.labels_shown = true;
                // `mnTitleAdvanceLayout`.
                self.layout = match self.layout {
                    Layout::Opening => Layout::Animate,
                    _ => Layout::Final,
                };
                self.set_allow_proceed_wait();
            }
            220 => {
                // `mnTitleSetEndLayout`.
                self.fire_alpha = 0xFF;
                self.labels_running = false;
            }
            280 => self.press_start_shown = true,
            650 if !demo.is_extend_demo_wait => {
                self.proceed_demo_next(scene_prev, demo, backup, rand_range)
            }
            1190 if demo.is_extend_demo_wait => {
                self.proceed_demo_next(scene_prev, demo, backup, rand_range)
            }
            _ => {}
        }
        // The fire camera's `mnTitleFireCameraProcUpdate`.
        self.update_fire_color(time);
        // `mnTitleFireProcUpdate`.
        for i in 0..2 {
            self.fire_shown[i] = self.fire_frames[i];
            self.fire_frames[i] = (self.fire_frames[i] + 1) % sprite::FIRE.len();
        }
        // `mnTitleProcUpdate` and `mnTitlePressStartProcUpdate`.
        if self.labels_shown && self.labels_running {
            self.labels_play += 1;
        }
        if self.press_start_shown {
            self.press_start_play += 1;
        }
        load
    }

    /// `mnTitleFireCameraProcUpdate`.
    fn update_fire_color(&mut self, time: u8) {
        if self.transition_tics < 40 {
            return;
        }
        if self.transition_tics <= 110 {
            self.fire_color[0] = (self.fire_color[0] + 4.0).min(255.0);
        } else {
            if self.fire_timer == 0 {
                self.fire_timer = 260;
                let mut id = time_range(time, 7) as usize;
                if id == self.fire_color_id {
                    id += 1;
                    if id >= 7 {
                        id = 0;
                    }
                }
                self.fire_color_id = id;
                self.fire_delta = core::array::from_fn(|i| {
                    (f32::from(FIRE_COLORS[id][i]) - self.fire_color[i]) / 80.0
                });
            }
            if self.fire_timer >= 80 {
                for i in 0..3 {
                    self.fire_color[i] += self.fire_delta[i];
                }
            }
            self.fire_timer -= 1;
        }
        for c in &mut self.fire_color {
            *c = c.clamp(0.0, 255.0);
        }
    }

    /// The fire camera's fill colour (`cobj->color`).
    pub fn fire_color(&self) -> [u8; 3] {
        self.fire_color.map(|c| c as i32 as u8)
    }

    /// `mnTitleSetColors`.
    fn colored(kind: usize, piece: Piece) -> Piece {
        match kind {
            KIND_DROP_SHADOW | KIND_TM => piece.prim([0; 3]),
            0..KIND_FOOTER => piece.prim([0xFF, 0xFE, 0x2A]).env([0; 3]),
            KIND_FOOTER => piece.prim([0xB7, 0xAE, 0x7C]).env([0x14, 0x12, 0x06]),
            KIND_HEADER => piece.prim([0x14, 0x12, 0x06]),
            KIND_PRESS_START => piece.prim([0xFF; 3]).env([0x17, 0x10, 0xA4]),
            _ => piece,
        }
    }

    /// A sprite of `kind` centred at its desc's place, scaled.
    fn label(kind: usize, centre: [f32; 2], scale: [f32; 2]) -> Draw {
        let mut p = Piece::clear(FILE_TITLE, SPRITE_DESCS[kind].1, centre[0], centre[1]);
        p.centred = true;
        p.scale = scale;
        Draw::Sprite(Self::colored(kind, p))
    }

    /// The cameras back to front: the fire camera's fill (100), then camera
    /// 60's display link 0 (the fire, the logo) and 1 (the labels, the
    /// header and footer, "Press Start"); then a proceeding title's black
    /// camera (0).
    pub fn visit(&self, anims: &impl Anims, f: &mut impl FnMut(Draw)) {
        let [r, g, b] = self.fire_color();
        f(Draw::Clear([r, g, b, 0xFF]));
        // `mnTitleMakeFire`'s two SObjs (`mnTitleFireProcDisplay`).
        for (i, &(x, y, sx, sy)) in [(-32.0, -16.0, 12.0, 8.5), (8.0, 8.0, 9.5, 7.0)]
            .iter()
            .enumerate()
        {
            let mut p = Piece::clear(FILE_FIRE, sprite::FIRE[self.fire_shown[i]], x, y);
            p.scale = [sx, sy];
            p.alpha = Some(self.fire_alpha as u8);
            f(Draw::Sprite(p));
        }
        // `mnTitleMakeLogoNoOpening` (`mnTitleLogoProcDisplay`).
        let [cx, cy] = SPRITE_DESCS[KIND_LOGO].0;
        let mut logo = Piece::clear(FILE_TITLE, sprite::LOGO_ANIM_FULL, cx, cy).prim([0xFF, 0, 0]);
        logo.centred = true;
        logo.alpha = Some(self.logo_alpha as u8);
        logo.solid = true;
        f(Draw::Sprite(logo));
        if self.labels_shown {
            // `mnTitleMakeLabels`' first GObj: the anim's children, until
            // `mnTitleSetEndLayout` puts them at rest.
            for (kind, desc) in SPRITE_DESCS.iter().enumerate().take(KIND_FOOTER) {
                let at = if self.labels_running {
                    anims.labels(self.labels_play, kind)
                } else {
                    None
                };
                let (centre, scale) = match at {
                    Some([x, y, sx, sy]) => ([x + 160.0, 120.0 - y], [sx, sy]),
                    None => (desc.0, [1.0, 1.0]),
                };
                f(Self::label(kind, centre, scale));
            }
            // The second: the footer and header at rest.
            for kind in [KIND_FOOTER, KIND_HEADER] {
                f(Self::label(kind, SPRITE_DESCS[kind].0, [1.0, 1.0]));
            }
        }
        if self.press_start_shown {
            let (centre, scale) = match anims.press_start(press_start_frame(self.press_start_play))
            {
                Some([x, y, sx, sy]) => ([x + 160.0, 120.0 - y], [sx, sy]),
                None => (SPRITE_DESCS[KIND_PRESS_START].0, [1.0, 1.0]),
            };
            f(Self::label(KIND_PRESS_START, centre, scale));
        }
        if self.black {
            f(Draw::Clear([0, 0, 0, 0xFF]));
        }
    }
}
