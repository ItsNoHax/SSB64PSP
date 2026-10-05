//! Final Destination's boss wallpaper and its fades:
//! `sc/sc1pmode/sc1pgameboss.c`.
//!
//! `sc1PGameBossInitWallpaper` makes a controller (`sSC1PGameBossMain`)
//! that layers four wallpapers of animated effects over the stage's common
//! wallpaper: shooting stars, then the effects Master Hand's damage calls
//! up at 90 and 180, and at his defeat the closing animation whose end
//! fades the screen to white and then to black
//! (`sc1PGameBossProcDisplayFadeAlpha`/`...FadeColor`) before the scene
//! ends (`ifCommonBattleEndSetBossDefeat`).
//!
//! [`BossWallpaper`] is the controller and every effect's portable state:
//! its alpha (`gobj->user_data`), its fade step (`DObj::user_data`), its
//! comet colour (the child's `user_data`), the root transform the
//! controller writes and the animation clock the controller polls. The host
//! draws each [`Effect`] with its packed graph, animation and material
//! clocks at [`Effect::anim_frame`] (`file 114`'s `llGRLastMap*` labels,
//! [`EFFECT_ASSETS`]), through the two fixed cameras
//! `sc1PGameBossMakeCamera` makes ([`CAMERA_EYE`]).
//!
//! The effects' joint animations loop or end on their own; the two facts
//! the controller reads from them are fixed by the ROM and checked by
//! `crates/ssb-rom/tests/boss.rs`: the comets loop every
//! [`COMET_LOOP_FRAMES`] animation frames and the closing animation ends
//! after [`CLOSING_FRAMES`] plays.

use ssb_engine::math::Vec3;

/// `dSC1PGameBossCometEnvColor{R,G,B}`: the shooting stars' colours.
pub const COMET_ENV_COLORS: [[u8; 3]; 7] = [
    [0xFF, 0x00, 0x00],
    [0x00, 0xFF, 0x00],
    [0x00, 0x00, 0xFF],
    [0xFF, 0xFF, 0x00],
    [0xFF, 0x00, 0xFF],
    [0x00, 0xFF, 0xFF],
    [0x78, 0x78, 0x78],
];

/// The effects' sources in file 114 (`llGRLastMapEffects*DObjDesc`,
/// `...MObjSub`, `llGRLastMapAnims*AnimJoint`, `...MatAnimJoint`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EffectAsset {
    pub graph: u32,
    /// `MObjSub ***`, if the effect has materials.
    pub mobjsub: Option<u32>,
    pub anim_joint: Option<u32>,
    /// Only bound when the effect has materials (`o_mobjsub != 0`).
    pub matanim_joint: Option<u32>,
}

/// File 114, `StageLastFile2`: `gMPCollisionGroundData->gr_desc[1]`'s file,
/// whose head (`llGRLastMapFileHead`, 0x4D48) the controller subtracts.
pub const EFFECT_FILE: u32 = 114;

/// One `SC1PGameBossEffect` with its paired `SC1PGameBossAnim`, by
/// wallpaper and effect: `[wallpaper][effect]`.
pub const EFFECT_ASSETS: [[Option<EffectAsset>; 2]; 4] = [
    [
        Some(EffectAsset {
            graph: 0x8960,
            mobjsub: Some(0x86D8),
            anim_joint: Some(0x8A40),
            matanim_joint: Some(0x8C50),
        }),
        None,
    ],
    [
        Some(EffectAsset {
            graph: 0xA188,
            mobjsub: Some(0x97B0),
            anim_joint: Some(0xA340),
            matanim_joint: Some(0xB1B0),
        }),
        None,
    ],
    [
        Some(EffectAsset {
            graph: 0xDD90,
            mobjsub: Some(0xD470),
            anim_joint: Some(0xDE70),
            matanim_joint: Some(0xDEC0),
        }),
        Some(EffectAsset {
            graph: 0x11268,
            mobjsub: Some(0x10788),
            anim_joint: None,
            matanim_joint: Some(0x11420),
        }),
    ],
    [
        Some(EffectAsset {
            graph: 0x11268,
            mobjsub: Some(0x10788),
            anim_joint: None,
            matanim_joint: Some(0x115C0),
        }),
        // `llGRLastMapEffects3_1DObjDesc` has no `MObjSub`, so its anim's
        // `MatAnimJoint` is never bound.
        Some(EffectAsset {
            graph: 0x12858,
            mobjsub: None,
            anim_joint: Some(0x128E0),
            matanim_joint: None,
        }),
    ],
];

/// `sc1PGameBossMakeCamera`'s two cameras: eye at (0, 0, 2000) looking at
/// the origin, priorities 40 and 60, drawing display link 5 for camera
/// tags 1 and 2.
pub const CAMERA_EYE: Vec3 = Vec3::new(0.0, 0.0, 2000.0);
pub const DL_LINK: u8 = 5;

/// The comets' joint animation loops every 149 animation frames
/// (`llGRLastMapAnims0AnimJoint`): `gobj->anim_frame` returns to zero.
pub const COMET_LOOP_FRAMES: f32 = 149.0;
/// `llGRLastMapAnims3_1AnimJoint` ends after 100 plays.
pub const CLOSING_FRAMES: u32 = 100;

/// `SC1PGameBossPlan`.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Plan {
    /// Effects made on this plan before the next.
    count: u8,
    camera_tag: u8,
    pos: Vec3,
}

/// The display callback an effect draws with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Display {
    /// `SC1PGameBossWallpaper0ProcDisplay`: translucent, prim alpha and
    /// the comet's env colour.
    Comet,
    /// `SC1PGameBossWallpaper1ProcDisplay`: two-cycle, every child's
    /// material at the alpha.
    Fall,
    /// `SC1PGameBossWallpaper2ProcDisplay`: translucent through the
    /// display links, every material at the alpha.
    Translucent,
    /// `SC1PGameBossWallpaper3ProcDisplay0`: translucent, drawn directly
    /// (`gcDrawDObjTreeForGObj`), every material at the alpha.
    TranslucentDirect,
    /// `sc1PGameBossProcDisplayFadeAlpha`: a white rectangle over the
    /// screen at [`BossWallpaper::fade_step`] alpha.
    FadeAlpha,
    /// `sc1PGameBossProcDisplayFadeColor`: an opaque rectangle of grey
    /// [`BossWallpaper::fade_step`].
    FadeColor,
}

/// How an effect updates (`SC1PGameBossEffect::proc_update`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Update {
    /// `SC1PGameBossWallpaper0ProcUpdate`.
    Comet,
    /// `SC1PGameBossWallpaper1ProcUpdate`.
    Fall,
    /// `SC1PGameBossWallpaper2ProcUpdate0`.
    Grow,
    /// `SC1PGameBossWallpaper2ProcUpdate1`.
    Late,
    /// `SC1PGameBossWallpaper3ProcUpdate0`.
    Plain,
    /// `SC1PGameBossWallpaper3ProcUpdate1`.
    Closing,
}

/// `SC1PGameBossWallpaper`.
struct WallpaperDesc {
    loop_count: u8,
    effects: &'static [(Update, Display)],
    anim_speeds: &'static [f32],
    plans: &'static [Plan],
    /// `dobj_color_id`: the fade-in step of its effects.
    color_id: i32,
    change_wait_base: i32,
    change_damage_min: i32,
    is_random: bool,
}

const WALLPAPERS: [WallpaperDesc; 4] = [
    WallpaperDesc {
        loop_count: 24,
        effects: &[(Update::Comet, Display::Comet)],
        anim_speeds: &[0.5, 1.0, 0.7, 0.25],
        plans: &[
            Plan {
                count: 10,
                camera_tag: 1,
                pos: Vec3::new(0.0, 0.0, -2000.0),
            },
            Plan {
                count: 14,
                camera_tag: 2,
                pos: Vec3::new(0.0, 0.0, -3000.0),
            },
        ],
        color_id: 1,
        change_wait_base: -1,
        change_damage_min: 90,
        is_random: true,
    },
    WallpaperDesc {
        loop_count: 1,
        effects: &[(Update::Fall, Display::Fall)],
        anim_speeds: &[0.5],
        plans: &[Plan {
            count: 1,
            camera_tag: 2,
            pos: Vec3::new(0.0, 4800.0, -10000.0),
        }],
        color_id: 1,
        change_wait_base: -1,
        change_damage_min: 180,
        is_random: false,
    },
    WallpaperDesc {
        loop_count: 2,
        effects: &[
            (Update::Grow, Display::Translucent),
            (Update::Late, Display::TranslucentDirect),
        ],
        anim_speeds: &[1.0, 1.0],
        plans: &[
            Plan {
                count: 1,
                camera_tag: 2,
                pos: Vec3::new(0.0, 0.0, 2000.0),
            },
            Plan {
                count: 1,
                camera_tag: 2,
                pos: Vec3::new(0.0, 0.0, 1000.0),
            },
        ],
        color_id: 3,
        change_wait_base: -1,
        change_damage_min: -1,
        is_random: false,
    },
    WallpaperDesc {
        loop_count: 2,
        effects: &[
            (Update::Plain, Display::TranslucentDirect),
            (Update::Closing, Display::Translucent),
        ],
        anim_speeds: &[0.5, 1.0],
        plans: &[
            Plan {
                count: 1,
                camera_tag: 2,
                pos: Vec3::new(0.0, 0.0, 1000.0),
            },
            Plan {
                count: 1,
                camera_tag: 1,
                pos: Vec3::new(0.0, 0.0, -2000.0),
            },
        ],
        color_id: 1,
        change_wait_base: -1,
        change_damage_min: -1,
        is_random: false,
    },
];

/// One effect GObj (`nGCCommonKindBossWallpaper`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Effect {
    /// Distinguishes the effects a slot has held, for the host's visuals.
    pub serial: u32,
    /// Which wallpaper and which of its effects: [`EFFECT_ASSETS`].
    pub wallpaper: u8,
    pub effect: u8,
    /// The plan's camera tag (1 or 2).
    pub camera_tag: u8,
    update: Update,
    pub display: Display,
    /// `gobj->user_data`: 0 to 255.
    pub alpha: i32,
    /// `DObjGetStruct(gobj)->user_data`: the fade step, negative once the
    /// next wallpaper replaces this one.
    pub step: i32,
    /// The child's `user_data`: the comet's [`COMET_ENV_COLORS`] entry.
    pub color: u8,
    /// The root `DObj`'s translation, Z rotation and scale.
    pub translate: Vec3,
    pub rotate_z: f32,
    pub scale: Vec3,
    /// `gcSetAllAnimSpeed`.
    pub anim_speed: f32,
    /// Animation frames played, as `gobj->anim_frame` reads it for a
    /// looping comet; the host's joint and material clocks play at
    /// [`Self::anim_speed`] each [`BossWallpaper::update`].
    pub anim_frame: f32,
    /// `gcPlayAnimAll` calls so far.
    pub plays: u32,
    /// `GOBJ_FLAG_HIDDEN`.
    pub hidden: bool,
    /// The closing effect's countdown (its child's `user_data`).
    counter: i32,
}

/// The most effects alive at once: 24 comets fading out over the next
/// wallpaper's.
pub const MAX_EFFECTS: usize = 32;

/// `sSC1PGameBossMain` and the effects it made.
#[derive(Debug, Clone)]
pub struct BossWallpaper {
    pub effects: [Option<Effect>; MAX_EFFECTS],
    is_skip_change: bool,
    /// `wallpaper_id`: the next wallpaper [`Self::update`] makes.
    pub wallpaper_id: u8,
    /// `bosswallpaper`: the current one.
    pub current: Option<u8>,
    change_wait: i32,
    /// `sSC1PGameBossWallpaperStepRGBA`.
    pub fade_step: f32,
    /// `ifCommonBattleEndSetBossDefeat` ran.
    pub done: bool,
    serial: u32,
}

/// The stage extents `func_ovl65_80191B44` scatters the comets over:
/// `gMPCollisionBounds.current.left`/`right` and
/// `MPGroundData::map_bound_top`/`bottom`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Extents {
    pub left: f32,
    pub right: f32,
    pub map_top: f32,
    pub map_bottom: f32,
}

impl Default for BossWallpaper {
    fn default() -> Self {
        Self::new()
    }
}

impl BossWallpaper {
    /// `sc1PGameBossInitWallpaper`.
    pub fn new() -> Self {
        BossWallpaper {
            effects: [None; MAX_EFFECTS],
            is_skip_change: false,
            wallpaper_id: 0,
            current: None,
            change_wait: 0,
            fade_step: 0.0,
            done: false,
            serial: 0,
        }
    }

    /// `sc1PGameBossSetChangeWallpaper`: Master Hand's defeat
    /// (`sc1PGameBossDefeatInterfaceProcSet`).
    pub fn set_change(&mut self) {
        self.is_skip_change = false;
    }

    /// The live effects.
    pub fn iter(&self) -> impl Iterator<Item = &Effect> {
        self.effects.iter().flatten()
    }

    /// One tick of the processes `gcRunAll` runs: the controller
    /// (`sc1PGameBossWallpaperProcUpdate`, priority 3), then every effect's
    /// update in link order. `boss_damage` is Master Hand's
    /// `stock_damage_all`.
    pub fn update(&mut self, boss_damage: i32, extents: Extents) {
        self.update_controller(boss_damage, extents);
        for slot in self.effects.iter_mut() {
            if let Some(e) = slot.as_mut() {
                if !Self::update_effect(
                    e,
                    boss_damage,
                    extents,
                    &mut self.fade_step,
                    &mut self.done,
                ) {
                    *slot = None;
                }
            }
        }
    }

    /// `sc1PGameBossWallpaperProcUpdate`.
    fn update_controller(&mut self, boss_damage: i32, extents: Extents) {
        if !self.is_skip_change {
            if let Some(next) = WALLPAPERS.get(usize::from(self.wallpaper_id)) {
                // `sc1PGameBossUpdateWallpaperColorID`: the outgoing
                // wallpaper's effects fade out by its colour id.
                if let Some(old) = self.current {
                    let id = WALLPAPERS[usize::from(old)].color_id;
                    for e in self.effects.iter_mut().flatten() {
                        e.step = -id;
                    }
                }
                self.current = Some(self.wallpaper_id);
                self.advance(next, extents);
            }
        }
        if self.change_wait != -1 {
            self.change_wait -= 1;
        }
        let Some(current) = self.current.map(|c| &WALLPAPERS[usize::from(c)]) else {
            return;
        };
        if current.change_damage_min != -1 {
            if current.change_damage_min < boss_damage {
                self.is_skip_change = false;
            }
        } else if self.change_wait == 0 {
            self.is_skip_change = false;
        }
    }

    /// `sc1PGameBossAdvanceWallpaper`.
    fn advance(&mut self, desc: &WallpaperDesc, extents: Extents) {
        let wallpaper = self.wallpaper_id;
        let (mut j, mut k, mut plan) = (0u8, 0usize, 0usize);
        for i in 0..desc.loop_count {
            let (effect, anim) = if desc.is_random {
                (
                    crate::rng::rand_int_range(desc.effects.len() as i32) as usize,
                    crate::rng::rand_int_range(desc.anim_speeds.len() as i32) as usize,
                )
            } else {
                (usize::from(i), usize::from(i))
            };
            if desc.plans.get(k).is_some_and(|p| j == p.count) {
                plan += 1;
                k += 1;
                j = 0;
            }
            self.make_effect(wallpaper, desc, effect, anim, plan, extents);
            j += 1;
        }
        self.is_skip_change = true;
        self.wallpaper_id += 1;
        self.change_wait = desc.change_wait_base;
    }

    /// `sc1PGameBossMakeWallpaperEffect` and
    /// `sc1PGameBossSetWallpaperTranslate`.
    fn make_effect(
        &mut self,
        wallpaper: u8,
        desc: &WallpaperDesc,
        effect: usize,
        anim: usize,
        plan_id: usize,
        extents: Extents,
    ) {
        let Some(slot) = self.effects.iter_mut().find(|s| s.is_none()) else {
            // `gcMakeGObjSPAfter` returned NULL.
            return;
        };
        let plan = desc.plans[plan_id.min(desc.plans.len() - 1)];
        let (update, display) = desc.effects[effect];
        // `syUtilsRandIntRange((7 + 7 + 7) / 3)`.
        let color = crate::rng::rand_int_range(COMET_ENV_COLORS.len() as i32) as u8;
        self.serial = self.serial.wrapping_add(1);
        let mut e = Effect {
            serial: self.serial,
            wallpaper,
            effect: effect as u8,
            camera_tag: plan.camera_tag,
            update,
            display,
            alpha: 0,
            step: desc.color_id,
            color,
            translate: Vec3::ZERO,
            rotate_z: 0.0,
            scale: Vec3::new(1.0, 1.0, 1.0),
            anim_speed: desc.anim_speeds[anim],
            // The make's `gcPlayAnimAll`.
            anim_frame: 0.0,
            plays: 1,
            hidden: false,
            counter: 0,
        };
        if desc.is_random {
            scatter(&mut e, extents);
            e.translate.z = plan.pos.z;
        } else {
            e.translate = plan.pos;
        }
        *slot = Some(e);
    }

    /// One effect's update; `false` ejects it.
    fn update_effect(
        e: &mut Effect,
        boss_damage: i32,
        extents: Extents,
        fade_step: &mut f32,
        done: &mut bool,
    ) -> bool {
        match e.update {
            Update::Comet => {
                if !fade_and_play(e) {
                    return false;
                }
                if e.anim_frame <= 0.0 {
                    scatter(e, extents);
                }
                true
            }
            Update::Fall => {
                if e.step == -1 {
                    e.anim_speed += -0.0012;
                } else if e.alpha < 0xFF {
                    e.translate.y += -18.8;
                }
                fade_and_play(e)
            }
            Update::Grow => {
                if e.alpha == 0 {
                    e.scale = Vec3::ZERO;
                }
                if boss_damage > 270 {
                    e.anim_speed += 0.02;
                } else if e.alpha < 0xFF {
                    e.scale += Vec3::new(0.004, 0.004, 0.004);
                }
                fade_and_play(e)
            }
            Update::Late => {
                if boss_damage > 270 {
                    e.hidden = false;
                    fade_and_play(e)
                } else {
                    e.hidden = true;
                    true
                }
            }
            Update::Plain => fade_and_play(e),
            Update::Closing => {
                if e.plays < CLOSING_FRAMES {
                    return fade_and_play(e);
                }
                match e.display {
                    Display::FadeAlpha => {
                        e.counter -= 1;
                        if e.counter == 0 {
                            *fade_step = 255.0;
                            e.counter = 100;
                            e.display = Display::FadeColor;
                        }
                    }
                    Display::FadeColor => {
                        e.counter -= 1;
                        if e.counter == 0 {
                            *done = true;
                        }
                    }
                    _ => {
                        *fade_step = 230.0;
                        e.counter = 100;
                        e.display = Display::FadeAlpha;
                    }
                }
                true
            }
        }
    }

    /// The fades' display callbacks, once per drawn frame: the white
    /// rises one step to 255, the grey falls 2.55 to 0.
    pub fn display(&mut self) -> Option<(Display, u8)> {
        let e = self
            .iter()
            .find(|e| matches!(e.display, Display::FadeAlpha | Display::FadeColor))?;
        match e.display {
            Display::FadeAlpha => {
                self.fade_step = (self.fade_step + 1.0).min(255.0);
                Some((Display::FadeAlpha, self.fade_step as u8))
            }
            _ => {
                self.fade_step = (self.fade_step - 2.55).max(0.0);
                Some((Display::FadeColor, self.fade_step as u8))
            }
        }
    }
}

impl Effect {
    /// The controller writes the root's Z rotation (the comets'
    /// `func_ovl65_80191B44`); other effects keep their descriptor's.
    pub fn sets_rotate_z(&self) -> bool {
        self.update == Update::Comet
    }

    /// The controller writes the root's scale (`...Wallpaper2ProcUpdate0`).
    pub fn sets_scale(&self) -> bool {
        self.update == Update::Grow
    }
}

/// `SC1PGameBossWallpaper3ProcUpdate0`: the alpha steps, an effect under
/// zero is ejected, otherwise its animations play.
fn fade_and_play(e: &mut Effect) -> bool {
    e.alpha += e.step;
    if e.alpha < 0 {
        return false;
    }
    if e.alpha > 0xFF {
        e.alpha = 0xFF;
    }
    e.plays += 1;
    e.anim_frame += e.anim_speed;
    if e.update == Update::Comet && e.anim_frame >= COMET_LOOP_FRAMES {
        e.anim_frame -= COMET_LOOP_FRAMES;
    }
    true
}

/// `func_ovl65_80191B44`: anywhere over the stage and 2000 beyond, the
/// comet turned to fly from its quadrant.
fn scatter(e: &mut Effect, x: Extents) {
    let mut angle = crate::rng::rand_int_range(2) * 30 + 30;
    let lr = (x.left - 2000.0).abs() + (x.right + 2000.0).abs();
    let bt = (x.map_top - 2000.0).abs() + (x.map_bottom + 2000.0).abs();
    e.translate.x = crate::rng::rand_float() * lr + (x.left - 2000.0);
    e.translate.y = crate::rng::rand_float() * bt + (x.map_bottom + 2000.0);
    let mut sw = 0;
    if e.translate.x < 0.0 {
        sw = 2;
    }
    if e.translate.y < 0.0 {
        sw += 1;
    }
    match sw {
        1 => angle += 180,
        3 => angle += 90,
        0 => angle += 270,
        _ => {}
    }
    e.rotate_z = (angle as f32).to_radians();
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXTENTS: Extents = Extents {
        left: -3000.0,
        right: 3000.0,
        map_top: 5000.0,
        map_bottom: -2400.0,
    };

    fn kinds(w: &BossWallpaper) -> Vec<(u8, u8)> {
        w.iter().map(|e| (e.wallpaper, e.effect)).collect()
    }

    #[test]
    fn twenty_four_comets_on_two_planes_fade_in() {
        let mut w = BossWallpaper::new();
        w.update(0, EXTENTS);
        assert_eq!(w.iter().count(), 24);
        let near = w.iter().filter(|e| e.translate.z == -2000.0).count();
        assert_eq!(near, 10);
        assert!(w.iter().take(10).all(|e| e.camera_tag == 1));
        assert!(w
            .iter()
            .skip(10)
            .all(|e| e.camera_tag == 2 && e.translate.z == -3000.0));
        assert!(w
            .iter()
            .all(|e| e.alpha == 1 && e.display == Display::Comet));
        for _ in 0..300 {
            w.update(0, EXTENTS);
        }
        assert!(w.iter().all(|e| e.alpha == 0xFF));
        assert_eq!(w.wallpaper_id, 1);
    }

    #[test]
    fn damage_calls_up_the_next_wallpapers_and_the_old_fade_out() {
        let mut w = BossWallpaper::new();
        w.update(0, EXTENTS);
        for _ in 0..255 {
            w.update(0, EXTENTS);
        }
        // Over 90: the next tick makes wallpaper 1; the comets fade at 1.
        w.update(91, EXTENTS);
        w.update(91, EXTENTS);
        assert_eq!(w.current, Some(1));
        assert!(w.iter().any(|e| e.wallpaper == 1));
        assert!(w.iter().filter(|e| e.wallpaper == 0).all(|e| e.step == -1));
        for _ in 0..300 {
            w.update(91, EXTENTS);
        }
        assert_eq!(kinds(&w), [(1, 0)]);
        // Over 180, wallpaper 2; its second effect hides until 270.
        w.update(181, EXTENTS);
        w.update(181, EXTENTS);
        assert_eq!(w.current, Some(2));
        let late = w
            .iter()
            .find(|e| e.wallpaper == 2 && e.effect == 1)
            .unwrap();
        assert!(late.hidden);
        w.update(271, EXTENTS);
        let late = w
            .iter()
            .find(|e| e.wallpaper == 2 && e.effect == 1)
            .unwrap();
        assert!(!late.hidden);
        // Wallpaper 2 stays until the defeat.
        for _ in 0..1000 {
            w.update(299, EXTENTS);
        }
        assert_eq!(w.current, Some(2));
    }

    #[test]
    fn the_defeat_wallpaper_ends_in_white_then_black() {
        let mut w = BossWallpaper::new();
        w.update(0, EXTENTS);
        w.update(200, EXTENTS);
        w.update(200, EXTENTS);
        w.update(200, EXTENTS);
        assert_eq!(w.current, Some(2));
        w.set_change();
        w.update(300, EXTENTS);
        assert_eq!(w.current, Some(3));
        let mut ticks = 0;
        while !w.done {
            w.update(300, EXTENTS);
            if let Some((d, v)) = w.display() {
                if d == Display::FadeAlpha {
                    assert!(v >= 230);
                }
            }
            ticks += 1;
            assert!(ticks < 1000);
        }
        // The make's tick already played it once more (the effects' process
        // runs after the controller's): 98 plays to the animation's end,
        // the check, then 100 updates of white and 100 of grey.
        assert_eq!(ticks, 98 + 1 + 100 + 100);
        assert_eq!(w.display(), Some((Display::FadeColor, 0)));
    }

    #[test]
    fn a_comet_at_whole_speeds_restarts_where_its_loop_ends() {
        let mut e = Effect {
            serial: 1,
            wallpaper: 0,
            effect: 0,
            camera_tag: 1,
            update: Update::Comet,
            display: Display::Comet,
            alpha: 255,
            step: 1,
            color: 0,
            translate: Vec3::ZERO,
            rotate_z: 0.0,
            scale: Vec3::new(1.0, 1.0, 1.0),
            anim_speed: 1.0,
            anim_frame: 0.0,
            plays: 1,
            hidden: false,
            counter: 0,
        };
        let (mut step, mut done) = (0.0, false);
        let mut moves = Vec::new();
        for tick in 1..=300 {
            let before = e.translate;
            BossWallpaper::update_effect(&mut e, 0, EXTENTS, &mut step, &mut done);
            if e.translate != before {
                moves.push(tick);
            }
        }
        assert_eq!(moves, [149, 298]);
    }
}
