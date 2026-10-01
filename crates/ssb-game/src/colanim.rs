//! Colour animations — `GMColAnim`, `ftMainUpdateColAnim` (`ft/ftmain.c`)
//! and the `ftParam*ColAnim*` setters (`ft/ftparam.c`).
//!
//! A colour animation runs a small script (`gm/gmcolscripts.c`) that sets
//! and blends `color1`, the colour a fighter is fogged towards
//! (`ftDisplayMainCalcFogColor`) and the colour the screen flash fills with
//! (`ifScreenFlashProcDisplay`), turns the fighter's light, and picks the
//! electric skeleton (`skeleton_id`). Every script and the id table
//! (`dGMColScriptsDescs`) are generated (`tools/gen-colanim-scripts.py`);
//! the interpreter follows the source event for event.
//!
//! One script thread is modelled: no script starts a parallel one
//! (`nGMColEventSetParallelScript`), so `End` always ends the animation.
//!
//! The `Effect` events queue `ftParamMakeEffect` on the fighter
//! ([`crate::fteffect`], RE-415); `PlayFGM` is read and skipped.

use crate::combat::{Element, HitStatus};
use crate::fighter::{Fighter, FighterKind};
use crate::status::AnyStatus;

#[path = "colanim_scripts.rs"]
mod scripts;

pub use scripts::{Script, DESCS, PRESERVE_COLANIM, PRESERVE_MODELPART, PRESERVE_TEXTUREPART};

/// `gmColCommandEffect` / `...EffectItemHold`: `ftParamMakeEffect`'s
/// arguments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColEffect {
    pub joint: i8,
    /// `efKind`.
    pub kind: u8,
    pub flag: u8,
    pub offset: [i16; 3],
    pub scatter: [i16; 3],
    /// `nGMColEventEffectItemHoldOffset`: the offset is scaled by the
    /// fighter's size.
    pub item_hold: bool,
}

/// One `gmColCommand*` event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColEvent {
    /// `nGMColEventEnd`.
    End,
    /// `nGMColEventWait`.
    Wait(u16),
    /// `nGMColEventGoto`: always to a script's first event.
    Goto(Script),
    /// `nGMColEventLoopBegin`.
    LoopBegin(u16),
    /// `nGMColEventLoopEnd`.
    LoopEnd,
    /// `nGMColEventSubroutine`.
    Subroutine(Script),
    /// `nGMColEventReturn`.
    Return,
    /// `nGMColEventClearColorAll`: both colours and the skeleton off (the
    /// light stays).
    ClearColorAll,
    /// `nGMColEventSetColor1`: `color1` at once, its steps cleared.
    SetColor1([u8; 4]),
    /// `nGMColEventSetColor2`.
    SetColor2([u8; 4]),
    /// `nGMColEventBlendColor1`: `color1` steps towards the colour over the
    /// frames.
    BlendColor1(u16, [u8; 4]),
    /// `nGMColEventEffect` / `...ItemHoldOffset`.
    Effect(ColEffect),
    /// `nGMColEventSetLight`: the fighter's light angles, in degrees.
    SetLight(i16, i16),
    /// `nGMColEventClearLight`.
    ClearLight,
    /// `nGMColEventPlayFGM`.
    PlayFgm,
    /// `nGMColEventSetSkeletonID`.
    SetSkeletonId(u8),
}

/// `GMColDesc`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColDesc {
    pub script: Option<Script>,
    pub priority: u8,
    /// A status change ends it (`ftMainSetStatus`).
    pub is_unlocked: bool,
}

/// A colour-animation id: an index into [`DESCS`]. The named constants are
/// generated after the table's scripts (`GMColAnimKind`'s names for ids 41
/// to 49 do not match them).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ColAnimId(pub u8);

impl ColAnimId {
    /// Id 0: no script.
    pub const NONE: Self = Self(0);
    /// `nGMColAnimFighterDamageFireStart`.
    pub const DAMAGE_FIRE_START: u8 = 12;
    /// `nGMColAnimFighterDamageIceStart`.
    pub const DAMAGE_ICE_START: u8 = 32;

    fn desc(self) -> &'static ColDesc {
        DESCS.get(usize::from(self.0)).unwrap_or(&DESCS[0])
    }

    /// `dGMColScriptsDescs[id].p_script`.
    pub fn script(self) -> Option<Script> {
        self.desc().script
    }

    /// `dGMColScriptsDescs[id].priority`.
    pub fn priority(self) -> u8 {
        self.desc().priority
    }

    /// `dGMColScriptsDescs[id].is_unlocked`.
    pub fn is_unlocked(self) -> bool {
        self.desc().is_unlocked
    }
}

/// `GMColKeys`: a colour and its per-frame step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ColKeys {
    pub rgba: [u8; 4],
    pub step: [i16; 4],
}

/// `GMColScript`'s `p_subroutine`/`loop_count`/`p_goto` words, which one
/// `script_id` indexes: a loop takes two slots (its start, its count), a
/// subroutine one (its return).
const STACK: usize = 5;

/// `GMColAnim`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ColAnim {
    pub id: ColAnimId,
    /// `cs[0].p_script`: a script and an event index.
    pc: Option<(Script, u16)>,
    /// `cs[0].color_event_timer`.
    timer: u16,
    /// `cs[0].script_id`.
    script_id: u8,
    stack_pc: [Option<(Script, u16)>; STACK],
    stack_count: [i32; STACK],
    /// `length`: frames until the animation ends by itself; 0 for none.
    pub length: i32,
    pub color1: ColKeys,
    pub is_use_color1: bool,
    pub color2: ColKeys,
    pub is_use_color2: bool,
    /// `light_angle_x`, `light_angle_y`, when `is_use_light`.
    pub light: Option<(f32, f32)>,
    /// `skeleton_id`: which `FTAttributes::skeleton` set draws, 0 for the
    /// model.
    pub skeleton_id: u8,
    /// Set by [`Fighter::tick_timers`] when the last of
    /// `intangible_tics`/`invincible_tics` runs out this frame.
    pub(crate) is_nodamage_expired: bool,
}

impl ColAnim {
    /// `ftParamCheckSetColAnimID`: starts `id` unless a higher-priority
    /// animation is running.
    pub fn check_set(&mut self, id: ColAnimId, length: i32) -> bool {
        if id.priority() < self.id.priority() {
            return false;
        }
        self.id = id;
        self.length = length;
        self.pc = id.script().map(|s| (s, 0));
        self.timer = 0;
        self.script_id = 0;
        self.clear();
        true
    }

    fn clear(&mut self) {
        self.is_use_color1 = false;
        self.is_use_color2 = false;
        self.light = None;
        self.skeleton_id = 0;
    }

    /// `ftParamResetColAnim`. The colours and their steps are kept.
    pub fn reset(&mut self) {
        self.pc = None;
        self.length = 0;
        self.id = ColAnimId::NONE;
        self.clear();
    }

    /// `ftMainUpdateColAnim`: one frame. Returns `true` when the animation
    /// ends (its script's `End`, or its `length` running out). The
    /// screen flash's animation runs no `Effect` event.
    pub fn update(&mut self) -> bool {
        self.update_effects(&mut |_| {})
    }

    /// [`Self::update`], handing each `Effect` event to `effect` as it is
    /// read.
    pub fn update_effects(&mut self, effect: &mut dyn FnMut(ColEffect)) -> bool {
        if self.pc.is_some() && self.timer != 0 {
            self.timer -= 1;
        }
        // A script that loops without waiting would hang the original; the
        // cap only keeps a data error from hanging the port.
        for _ in 0..1024 {
            let Some((script, at)) = self.pc.filter(|_| self.timer == 0) else {
                break;
            };
            let Some(&event) = script.events().get(usize::from(at)) else {
                self.pc = None;
                break;
            };
            let next = Some((script, at + 1));
            self.pc = next;
            match event {
                ColEvent::End => return true,
                ColEvent::Wait(frames) => self.timer = frames,
                ColEvent::Goto(s) => self.pc = Some((s, 0)),
                ColEvent::LoopBegin(count) => {
                    let id = usize::from(self.script_id);
                    if id + 2 <= STACK {
                        self.stack_pc[id] = next;
                        self.stack_count[id + 1] = i32::from(count);
                        self.script_id += 2;
                    }
                }
                ColEvent::LoopEnd => {
                    let id = usize::from(self.script_id);
                    if id >= 2 {
                        self.stack_count[id - 1] -= 1;
                        if self.stack_count[id - 1] != 0 {
                            self.pc = self.stack_pc[id - 2];
                        } else {
                            self.script_id -= 2;
                        }
                    }
                }
                ColEvent::Subroutine(s) => {
                    let id = usize::from(self.script_id);
                    if id < STACK {
                        self.stack_pc[id] = next;
                        self.script_id += 1;
                    }
                    self.pc = Some((s, 0));
                }
                ColEvent::Return => {
                    if self.script_id == 0 {
                        self.pc = None;
                    } else {
                        self.script_id -= 1;
                        self.pc = self.stack_pc[usize::from(self.script_id)];
                    }
                }
                ColEvent::ClearColorAll => {
                    self.is_use_color1 = false;
                    self.is_use_color2 = false;
                    self.skeleton_id = 0;
                }
                ColEvent::SetColor1(rgba) => {
                    self.is_use_color1 = true;
                    self.color1 = ColKeys { rgba, step: [0; 4] };
                }
                ColEvent::SetColor2(rgba) => {
                    self.is_use_color2 = true;
                    self.color2 = ColKeys { rgba, step: [0; 4] };
                }
                ColEvent::BlendColor1(frames, target) => {
                    let frames = i32::from(frames.max(1));
                    for (step, (&to, &from)) in self
                        .color1
                        .step
                        .iter_mut()
                        .zip(target.iter().zip(&self.color1.rgba))
                    {
                        *step = ((i32::from(to) - i32::from(from)) / frames) as i16;
                    }
                }
                ColEvent::SetLight(x, y) => self.light = Some((f32::from(x), f32::from(y))),
                ColEvent::ClearLight => self.light = None,
                ColEvent::SetSkeletonId(id) => self.skeleton_id = id,
                ColEvent::Effect(e) => effect(e),
                ColEvent::PlayFgm => {}
            }
        }
        for (keys, used) in [
            (&mut self.color1, self.is_use_color1),
            (&mut self.color2, self.is_use_color2),
        ] {
            if used {
                for (c, &step) in keys.rgba.iter_mut().zip(&keys.step) {
                    *c = c.wrapping_add(step as u8);
                }
            }
        }
        if self.length != 0 {
            self.length -= 1;
            if self.length == 0 {
                return true;
            }
        }
        false
    }

    /// `color1` when it is in use.
    pub fn color(&self) -> Option<[u8; 4]> {
        self.is_use_color1.then_some(self.color1.rgba)
    }
}

/// `ftParamCheckSetFighterColAnimID`.
pub fn check_set(f: &mut Fighter, id: ColAnimId, length: i32) -> bool {
    f.colanim.check_set(id, length)
}

/// `FTDONKEY_GIANTPUNCH_CHARGE_MAX`.
const DONKEY_GIANTPUNCH_CHARGE_MAX: u8 = 10;

/// `ftParamGetBestHitStatusPart`: `hitstatus`, or the weakest damage
/// collision's when every box is better.
pub fn best_hit_status_part(f: &Fighter) -> HitStatus {
    let n = crate::hurtbox::damage_colls(f.kind).map_or(0, |d| d.len());
    let parts = &f.damage_colls.hitstatus[..n.min(f.damage_colls.hitstatus.len())];
    let mut best = parts.first().copied().unwrap_or(HitStatus::None);
    if best != HitStatus::Normal {
        for &h in parts.iter().skip(1) {
            if h == HitStatus::None {
                break;
            }
            if (best as u8) > (h as u8) {
                best = h;
            }
        }
    }
    if (f.hitstatus as u8) < (best as u8) {
        best
    } else {
        f.hitstatus
    }
}

/// `ftParamResetStatUpdateColAnim`: the animation ends, and the fighter's
/// standing states pick the next one. Healing, the Star and the Hammer are
/// items the port does not have.
pub fn reset_stat_update(f: &mut Fighter) {
    f.colanim.reset();
    match best_hit_status_part(f) {
        HitStatus::Invincible => {
            check_set(f, ColAnimId::FIGHTER_HIT_STATUS_INVINCIBLE, 0);
        }
        HitStatus::Intangible => {
            check_set(f, ColAnimId::FIGHTER_HIT_STATUS_INTANGIBLE, 0);
        }
        _ => {}
    }
    let charged = match f.kind {
        FighterKind::Donkey | FighterKind::PolyDonkey | FighterKind::GiantDonkey => {
            // `nFTKindNDonkey`/`GDonkey` share the Donkey Kong case.
            f.donkey_special_n.charge_level == DONKEY_GIANTPUNCH_CHARGE_MAX
        }
        FighterKind::Samus | FighterKind::PolySamus => {
            f.samus.charge_level == crate::samus::CHARGE_MAX
        }
        FighterKind::Kirby => {
            let copy = f.kirby.copy_id;
            (matches!(copy, FighterKind::Samus | FighterKind::PolySamus)
                && f.kirby.copy.samus_charge_level == crate::kirby_copy::CHARGE_MAX)
                || (matches!(copy, FighterKind::Donkey | FighterKind::PolyDonkey)
                    && f.kirby.copy.donkey_charge_level == crate::kirby_copy::GIANTPUNCH_CHARGE_MAX)
        }
        _ => false,
    };
    if charged {
        check_set(f, ColAnimId::FIGHTER_COMMON_SPECIAL_N_CHARGE, 0);
    }
    if matches!(f.kind, FighterKind::Ness | FighterKind::PolyNess) && crate::ness::absorbing(f) {
        check_set(f, ColAnimId::FIGHTER_NESS_SPECIAL_LW_HOLD, 0);
    }
    if f.invincible_frames != 0 || f.intangible_frames != 0 {
        check_set(f, ColAnimId::FIGHTER_NO_DAMAGE, 0);
    }
}

/// `ftMainRunUpdateColAnim`: a frame of the animation; one that ends gives
/// way to [`reset_stat_update`].
pub fn run_update(f: &mut Fighter) {
    // Every animation `reset_stat_update` starts loops; the cap only keeps
    // a data error from hanging the port.
    for _ in 0..8 {
        let (kind, lr) = (f.kind, f.facing.sign() as i8);
        let queue = &mut f.effects;
        let ended = f.colanim.update_effects(&mut |e| {
            // `ftParamMakeEffect(..., fp->lr, is_item_hold, flag)`.
            queue.push(crate::fteffect::FighterEffect::Param(
                crate::fteffect::EffectRequest {
                    kind: u16::from(e.kind),
                    joint: crate::fteffect::joint_id(kind, e.joint),
                    offset: Some(ssb_engine::math::Vec3::new(
                        f32::from(e.offset[0]),
                        f32::from(e.offset[1]),
                        f32::from(e.offset[2]),
                    )),
                    scatter: Some(ssb_engine::math::Vec3::new(
                        f32::from(e.scatter[0]),
                        f32::from(e.scatter[1]),
                        f32::from(e.scatter[2]),
                    )),
                    lr,
                    is_scale_pos: e.item_hold,
                    flag: u16::from(e.flag),
                },
            ));
        });
        if !ended {
            return;
        }
        reset_stat_update(f);
    }
}

/// `ftMainProcUpdateInterrupt`'s colour half: [`run_update`], then, when
/// the hit-status timers ran out this frame, the end of a `NoDamage`
/// flicker (`colanim_id == 0xA`). The port counts the timers down earlier
/// in the frame; nothing between reads them.
pub fn run_update_interrupt(f: &mut Fighter) {
    run_update(f);
    if core::mem::take(&mut f.colanim.is_nodamage_expired)
        && f.colanim.id == ColAnimId::FIGHTER_NO_DAMAGE
    {
        reset_stat_update(f);
    }
}

/// `ftMainSetStatus`: an unlocked animation ends with the status unless the
/// new one preserves it (`FTSTATUS_PRESERVE_COLANIM`).
pub(crate) fn on_set_status(f: &mut Fighter, preserve: bool) {
    if !preserve && f.colanim.id.is_unlocked() {
        reset_stat_update(f);
    }
}

/// Whether `ftMainSetStatus` from `from` into `to` carries
/// `FTSTATUS_PRESERVE_COLANIM` ([`PRESERVE_COLANIM`], generated from every
/// such call: map switches between a move's ground and air statuses, and a
/// move's own later statuses).
pub fn preserved(kind: FighterKind, from: AnyStatus, to: AnyStatus) -> bool {
    preserved_in(&PRESERVE_COLANIM, kind, from, to)
}

/// Whether a `(fighter, from, to)` table generated from the `ftMainSetStatus`
/// calls ([`PRESERVE_COLANIM`], [`PRESERVE_MODELPART`]) names this switch.
pub(crate) fn preserved_in(
    table: &[(u8, u16, u16)],
    kind: FighterKind,
    from: AnyStatus,
    to: AnyStatus,
) -> bool {
    let base = match kind.polygon_base().unwrap_or(kind) {
        // Luigi runs Mario's specials (`ftmariospecialn.c`).
        FighterKind::Luigi => FighterKind::Mario,
        FighterKind::GiantDonkey => FighterKind::Donkey,
        k => k,
    } as u8;
    let (from, to) = (from.id(), to.id());
    table
        .iter()
        .any(|&(k, f, t)| (k == 0xFF || k == base) && (f == 0xFFFF || f == from) && t == to)
}

/// `ftParamSetHitStatusColAnim`, from `ftParamSetHitStatusPartAll` and
/// `ftParamSetHitStatusAll`.
pub fn set_hit_status_colanim(f: &mut Fighter, status: HitStatus) {
    let id = match status {
        HitStatus::Normal => ColAnimId::FIGHTER_HIT_STATUS_NORMAL,
        HitStatus::Invincible => ColAnimId::FIGHTER_HIT_STATUS_INVINCIBLE,
        HitStatus::Intangible => ColAnimId::FIGHTER_HIT_STATUS_INTANGIBLE,
        HitStatus::None => return,
    };
    check_set(f, id, 0);
}

/// `dFTParamSkeletonColAnimIDs`: the first of a fighter's four electric
/// damage animations. The decompilation's names for ids 24 and 28 are
/// swapped: Kirby and Jigglypuff take 24, whose script flashes skeleton 2
/// (which only their `FTAttributes::skeleton` has), and Samus 28.
pub fn skeleton_colanim_base(kind: FighterKind) -> u8 {
    match kind {
        FighterKind::Samus => 0x1C,
        FighterKind::Kirby | FighterKind::Purin => 0x18,
        FighterKind::Mario
        | FighterKind::Fox
        | FighterKind::Donkey
        | FighterKind::Luigi
        | FighterKind::Link
        | FighterKind::Yoshi
        | FighterKind::Captain
        | FighterKind::Pikachu
        | FighterKind::Ness => 0x14,
        _ => 0x10,
    }
}

/// `ftCommonDamageCheckElementSetColAnim`: the damage animation for the
/// hit's element and level (0 to 3, `ftCommonDamageGetDamageLevel`).
pub fn damage_element_colanim(f: &mut Fighter, element: Element, level: u8) -> bool {
    let level = level.min(3);
    let id = match element {
        Element::Fire => ColAnimId(ColAnimId::DAMAGE_FIRE_START + level),
        // `ftParamCheckSetSkeletonColAnimID`.
        Element::Electric => ColAnimId(skeleton_colanim_base(f.kind) + level),
        Element::Freezing => ColAnimId(ColAnimId::DAMAGE_ICE_START + level),
        _ => ColAnimId::FIGHTER_DAMAGE_COMMON,
    };
    check_set(f, id, 0)
}

/// `ftCommonDamageUpdateDamageColAnim`: a hit that leaves the fighter in its
/// status still flashes, and the animation's first frame runs at once.
pub fn update_damage_colanim(f: &mut Fighter, knockback: f32, element: Element) {
    let level = crate::attack::damage_level(crate::attack::hitstun_frames(knockback));
    if damage_element_colanim(f, element, level) {
        run_update(f);
    }
}

/// `FTCOMMON_DAMAGE_KNOCKBACK_VERYHIGH`.
pub const KNOCKBACK_VERYHIGH: f32 = 160.0;

/// `ftCommonDamageCheckMakeScreenFlash`: the screen flash a strong hit
/// makes, by element.
pub fn damage_screen_flash(knockback: f32, element: Element) -> Option<ColAnimId> {
    if knockback <= KNOCKBACK_VERYHIGH {
        return None;
    }
    Some(match element {
        Element::Fire => ColAnimId::SCREEN_FLASH_DAMAGE_FIRE,
        Element::Electric => ColAnimId::SCREEN_FLASH_DAMAGE_ELECTRIC,
        Element::Freezing => ColAnimId::SCREEN_FLASH_DAMAGE_ICE,
        _ => ColAnimId::SCREEN_FLASH_DAMAGE_NORMAL,
    })
}

#[cfg(test)]
#[path = "colanim_tests.rs"]
mod tests;
