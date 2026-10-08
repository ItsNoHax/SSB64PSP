//! Fighter motion scripts — `ftMainParseMotionEvent`,
//! `ftMainUpdateMotionEventsAll` and `ftMainUpdateMotionEventsForward`.
//!
//! A status's attack collisions, hit-status windows, hurtbox edits and
//! script flags are `ftMotionCommand` bytecode in the fighter's
//! `relocData/*MainMotion.c`, not tables. The `scripts` module, generated
//! at build time from the user's ROM (`crates/ssb-tablegen`), carries that
//! bytecode word for word, and this module runs it the
//! way the original does: `ftMainSetStatus` starts the status's script with
//! `script_wait = anim_speed - frame_begin`, and every frame outside hitlag
//! the wait drops by the animation speed and the commands run until the next
//! wait. A status entered part-way (`frame_begin != 0`) fast-forwards,
//! skipping attack creation, sounds, flags and effects exactly as
//! `ftMainUpdateMotionEventsForward` does.
//!
//! Only commands with gameplay effects act here: attack collisions,
//! hit statuses, hurtbox edits, the four flags, the aerial-jump commands,
//! the colour animations ([`crate::colanim`]) and the effects
//! ([`crate::fteffect`]). The effects follow the source's two copies of the
//! scripts: from `ftMainProcUpdateInterrupt` to the end of
//! `ftMainProcPhysicsMap` (`is_events_forward`) the status scripts skip
//! them and [`forward_effect`] makes them from the copies after the map
//! step; a status set outside those passes makes them at once.
//! The model-part commands set [`crate::modelpart`]'s state (RE-425).
//! Texture-part commands select the joint material's sprite (RE-426).
//! Sounds, rumble, slope contours and throw descriptors are
//! decoded and skipped; the ported status code owns throws.

/// The fighters' motion scripts and tables, generated at build time from the
/// user's ROM by `crates/ssb-tablegen` (never committed).
mod scripts {
    include!(concat!(env!("OUT_DIR"), "/motion_scripts.rs"));
}

use ssb_engine::math::Vec3;

use crate::combat::{AttackState, Element, HitStatus};
use crate::fighter::{Fighter, FighterKind};
use crate::status::AnyStatus;

pub use scripts::{COMMON_STATUS_MOTION, SPECIAL_STATUS_START};

/// A motion without a script (`0x80000000` in `FTMotionDesc::offset`), or a
/// pointer outside the fighter's own file (common item movesets).
pub const NO_SCRIPT: u32 = u32::MAX;

/// A script index with this bit set reads `FTCommonMoveset`
/// (`scripts::COMMON_MOVESET_WORDS`) instead of the fighter's own words.
pub const COMMON_BIT: u32 = 0x4000_0000;

/// `FTMotionDesc`, reduced to its script and its figatree's length.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MotionDesc {
    /// Word index into [`FighterScripts::words`], or [`NO_SCRIPT`].
    pub script: u32,
    /// Frames the figatree runs for; `0` without one, `0xFFFF` when it
    /// loops.
    pub anim_length: u16,
    /// The figatree's clock (RE-474): its frames to the `End` or the first
    /// `Loop` command, with [`CLIP_LOOPS`] set when it loops, or
    /// [`CLIP_UNKNOWN`] without a figatree the generator could walk.
    pub clip: u16,
    /// `FTMotionDesc.anim_desc` (`FTANIM_FLAG_*`).
    pub anim_flags: u32,
}

/// [`MotionDesc::clip`]'s loop bit: the figatree ends in `Loop` (back to
/// its first frame) rather than `End`.
pub const CLIP_LOOPS: u16 = 0x8000;

/// [`MotionDesc::clip`] without a figatree file (`anim_file_id` 0, or a
/// shield pose): `ftMainSetStatus` leaves the playing figatree alone.
pub const CLIP_NONE: u16 = 0xFFFE;

/// [`MotionDesc::clip`] for a figatree the generator could not walk: the
/// figatree clock then counts on, as the status clock does.
pub const CLIP_UNKNOWN: u16 = 0xFFFF;

/// `FTAnimDesc.flags.is_use_transn_joint`.
pub const ANIM_FLAG_TRANSN: u32 = 0x4000_0000;

/// `FTAnimDesc.flags.is_use_xrotn_joint` (`ftdef.h`'s
/// `FTANIM_FLAG_TRANSN_JOINT`, the names swapped).
pub const ANIM_FLAG_XROTN: u32 = 0x8000_0000;

/// Whether the status's motion interposes `XRotN` between TopN and the
/// model and leads its figatree with it (and no detached TransN): the
/// clip's leading joint then poses the whole model (RE-468).
pub fn leads_with_xrotn(kind: FighterKind, status: AnyStatus) -> bool {
    motion_desc(kind, status)
        .is_some_and(|d| d.anim_flags & (ANIM_FLAG_XROTN | ANIM_FLAG_TRANSN) == ANIM_FLAG_XROTN)
}

/// Whether the status's motion moves the fighter by its TransN joint
/// (`fp->anim_desc.flags.is_use_transn_joint`), which
/// `ftPhysicsApplyGroundFrictionOrTransN` reads.
pub fn uses_transn(kind: FighterKind, status: AnyStatus) -> bool {
    motion_desc(kind, status).is_some_and(|d| d.anim_flags & ANIM_FLAG_TRANSN != 0)
}

/// One fighter's `*MainMotion` file and motion table.
#[derive(Debug)]
pub struct FighterScripts {
    pub words: &'static [u32],
    pub motions: &'static [MotionDesc],
    /// `dFT<Name>SpecialStatusDescs[..].mflags.motion_id`.
    pub special_status_motion: &'static [i16],
}

/// A script index with this bit set reads the fighter's demo scripts
/// ([`DemoScripts::words`]); only the part-event walk uses it.
pub const DEMO_BIT: u32 = 0x2000_0000;

/// One fighter's `dFT<Name>SubMotionDescs` scripts (`sc/scsubsys`): the
/// demo statuses the selects and the results set (`nFTDemoStatusNull` to
/// `nFTDemoStatusLose`, `D_ovl1_80390BE8`'s motion ids 0 to 5), RE-426.
#[derive(Debug)]
pub struct DemoScripts {
    pub words: &'static [u32],
    /// Word index of each row's script, or [`NO_SCRIPT`].
    pub rows: [u32; 24],
}

/// The fighter's [`DemoScripts`] (every kind has one).
pub fn demo_scripts(kind: FighterKind) -> Option<&'static DemoScripts> {
    Some(match kind {
        FighterKind::Mario => &scripts::MARIO_DEMO,
        FighterKind::Fox => &scripts::FOX_DEMO,
        FighterKind::Donkey => &scripts::DONKEY_DEMO,
        FighterKind::Samus => &scripts::SAMUS_DEMO,
        FighterKind::Luigi => &scripts::LUIGI_DEMO,
        FighterKind::Link => &scripts::LINK_DEMO,
        FighterKind::Yoshi => &scripts::YOSHI_DEMO,
        FighterKind::Captain => &scripts::CAPTAIN_DEMO,
        FighterKind::Kirby => &scripts::KIRBY_DEMO,
        FighterKind::Pikachu => &scripts::PIKACHU_DEMO,
        FighterKind::Purin => &scripts::PURIN_DEMO,
        FighterKind::Ness => &scripts::NESS_DEMO,
        FighterKind::Boss => &scripts::BOSS_DEMO,
        FighterKind::MetalMario => &scripts::MMARIO_DEMO,
        FighterKind::PolyMario => &scripts::NMARIO_DEMO,
        FighterKind::PolyFox => &scripts::NFOX_DEMO,
        FighterKind::PolyDonkey => &scripts::NDONKEY_DEMO,
        FighterKind::PolySamus => &scripts::NSAMUS_DEMO,
        FighterKind::PolyLuigi => &scripts::NLUIGI_DEMO,
        FighterKind::PolyLink => &scripts::NLINK_DEMO,
        FighterKind::PolyYoshi => &scripts::NYOSHI_DEMO,
        FighterKind::PolyCaptain => &scripts::NCAPTAIN_DEMO,
        FighterKind::PolyKirby => &scripts::NKIRBY_DEMO,
        FighterKind::PolyPikachu => &scripts::NPIKACHU_DEMO,
        FighterKind::PolyPurin => &scripts::NPURIN_DEMO,
        FighterKind::PolyNess => &scripts::NNESS_DEMO,
        FighterKind::GiantDonkey => &scripts::GDONKEY_DEMO,
    })
}

/// A demo fighter's status script (`scSubsysFighterSetStatus`, then
/// `scSubsysFighterProcUpdate`'s `ftMainPlayAnimEventsAll` each frame), of
/// which the port runs the flow and the model- and texture-part events
/// (RE-426). Demo statuses play at speed 1 from frame 0.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DemoScript {
    kind: FighterKind,
    thread: ScriptThread,
    anim_frame: f32,
}

impl DemoScript {
    /// `ftMainSetStatus(fighter_gobj, nFTDemoStatusNull + row,
    /// FTSTATUS_PRESERVE_NONE, 1.0F, 0.0F)`: the parts reset if modified,
    /// the row's script starts and its time-zero commands run.
    pub fn start(kind: FighterKind, row: usize, parts: &mut crate::modelpart::ModelParts) -> Self {
        let base = parts.detail_base;
        parts.set_detail_all(base);
        if parts.is_modify {
            parts.reset_all();
        }
        if parts.texture.is_modify {
            parts.reset_textures();
        }
        let pc = demo_scripts(kind)
            .and_then(|d| d.rows.get(row).copied())
            .unwrap_or(NO_SCRIPT);
        let mut d = DemoScript {
            kind,
            thread: ScriptThread {
                pc,
                wait: 1.0,
                ..ScriptThread::default()
            },
            anim_frame: 0.0,
        };
        d.run(parts);
        d
    }

    /// One frame: the clip advances, then the events.
    pub fn tick(&mut self, parts: &mut crate::modelpart::ModelParts) {
        self.anim_frame += 1.0;
        self.run(parts);
    }

    fn run(&mut self, parts: &mut crate::modelpart::ModelParts) {
        let Some(words) = demo_scripts(self.kind).map(|d| d.words) else {
            return;
        };
        let t = &mut self.thread;
        if t.pc == NO_SCRIPT {
            return;
        }
        if t.wait != f32::MAX {
            t.wait -= 1.0;
        }
        for _ in 0..4096 {
            let Some(&w) = words.get(t.pc as usize) else {
                t.pc = NO_SCRIPT;
                return;
            };
            if t.wait == f32::MAX {
                // A paused script resumes when the clip starts over.
                if 1.0 <= self.anim_frame {
                    return;
                }
                t.wait = -self.anim_frame;
            } else if t.wait > 0.0 {
                return;
            }
            let opcode = w >> 26;
            let value = w & 0x03FF_FFFF;
            let next = t.pc + command_words(opcode);
            let word1 = words.get(t.pc as usize + 1).copied().unwrap_or(NO_SCRIPT);
            match opcode {
                op::END => {
                    t.pc = NO_SCRIPT;
                    return;
                }
                op::SYNC_WAIT => t.wait += value as f32,
                op::ASYNC_WAIT => t.wait = value as f32 - self.anim_frame,
                op::LOOP_BEGIN => {
                    if t.script_id + 2 <= STACK_MAX {
                        t.p_goto[t.script_id] = next;
                        t.loop_count[t.script_id] = value as i32;
                        t.script_id += 2;
                    }
                }
                op::LOOP_END => {
                    if t.script_id >= 2 {
                        let slot = t.script_id - 2;
                        t.loop_count[slot] -= 1;
                        if t.loop_count[slot] != 0 {
                            t.pc = t.p_goto[slot];
                            continue;
                        }
                        t.script_id -= 2;
                    }
                }
                op::SUBROUTINE => {
                    if word1 != NO_SCRIPT && t.script_id < STACK_MAX {
                        t.p_goto[t.script_id] = next;
                        t.script_id += 1;
                        t.pc = word1;
                        continue;
                    }
                }
                op::RETURN => {
                    if t.script_id == 0 {
                        t.pc = NO_SCRIPT;
                        return;
                    }
                    t.script_id -= 1;
                    t.pc = t.p_goto[t.script_id];
                    continue;
                }
                op::GOTO => {
                    t.pc = word1;
                    continue;
                }
                op::PAUSE_SCRIPT => t.wait = f32::MAX,
                op::SET_MODEL_PART_ID => {
                    let joint = sign((w >> 19) & 0x7F, 7);
                    let part = sign(w & 0x7_FFFF, 19);
                    if let Ok(joint) = u8::try_from(joint) {
                        parts.set(joint, part as i8);
                    }
                }
                op::RESET_MODEL_PART_ALL => parts.reset_all(),
                op::HIDE_MODEL_PART_ALL => parts.hide_all(),
                op::SET_TEXTURE_PART_ID => {
                    let (part, id) = texture_part_event(w);
                    parts.set_texture(part, id);
                }
                _ => {}
            }
            t.pc = next;
        }
    }
}

/// `FTAttributes` fields the hit pipeline reads (generated from the
/// fighters' `relocData/*Main.c`, US).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CombatAttrs {
    /// Model scale; `MakeAttackCollScaled` offsets divide by it.
    pub size: f32,
    /// `ReboundWait`'s animation spreads over the rebound time.
    pub rebound_anim_length: f32,
    pub shield_size: f32,
    /// `ShieldBreakFly`'s launch speed.
    pub shield_break_vel_y: f32,
    pub jostle_width: f32,
    pub jostle_x: f32,
    /// `hit_detect_range`: up, down, sideways reach from TopN.
    pub hit_detect_range: [f32; 3],
    /// `effect_joint_ids`: the joints `ftParamGetEffectJointPosition`
    /// cycles through for flames, sparks and shocks.
    pub effect_joint_ids: [u8; 5],
    /// `joint_itemlight_id`: `ftParamGetJointID`'s `-2`.
    pub joint_itemlight_id: u8,
    /// `dead_fgm_ids`: the KO voice and sound `ftCommonDeadInitStatusVars`
    /// queues.
    pub dead_fgm_ids: [u16; 2],
    /// `deadup_sfx`: the Star-KO voice.
    pub deadup_sfx: u16,
    /// `damage_sfx`: `ftCommonDamageInitDamageVars`'s voice.
    pub damage_sfx: u16,
    /// `smash_sfx`: `nFTMotionEventPlaySmashVoice` picks one at random.
    pub smash_sfx: [u16; 3],
    /// `heavyget_sfx`: `itMainSetFighterHold` for a heavy item.
    pub heavyget_sfx: u16,
    /// `is_have_voice`: gates `PlayVoiceStoreInfo` and
    /// `PlayLoopVoiceStoreInfo`.
    pub is_have_voice: bool,
}

/// `attr->is_have_voice`.
fn has_voice(f: &Fighter) -> bool {
    combat_attrs(f.kind).is_some_and(|a| a.is_have_voice)
}

/// The fighter's [`CombatAttrs`] (every kind has them).
pub fn combat_attrs(kind: FighterKind) -> Option<&'static CombatAttrs> {
    Some(match kind {
        FighterKind::Mario => &scripts::MARIO_ATTRS,
        FighterKind::Fox => &scripts::FOX_ATTRS,
        FighterKind::Donkey => &scripts::DONKEY_ATTRS,
        FighterKind::Samus => &scripts::SAMUS_ATTRS,
        FighterKind::Luigi => &scripts::LUIGI_ATTRS,
        FighterKind::Link => &scripts::LINK_ATTRS,
        FighterKind::Yoshi => &scripts::YOSHI_ATTRS,
        FighterKind::Captain => &scripts::CAPTAIN_ATTRS,
        FighterKind::Kirby => &scripts::KIRBY_ATTRS,
        FighterKind::Pikachu => &scripts::PIKACHU_ATTRS,
        FighterKind::Purin => &scripts::PURIN_ATTRS,
        FighterKind::Ness => &scripts::NESS_ATTRS,
        FighterKind::Boss => &scripts::BOSS_ATTRS,
        FighterKind::MetalMario => &scripts::MMARIO_ATTRS,
        FighterKind::PolyMario => &scripts::NMARIO_ATTRS,
        FighterKind::PolyFox => &scripts::NFOX_ATTRS,
        FighterKind::PolyDonkey => &scripts::NDONKEY_ATTRS,
        FighterKind::PolySamus => &scripts::NSAMUS_ATTRS,
        FighterKind::PolyLuigi => &scripts::NLUIGI_ATTRS,
        FighterKind::PolyLink => &scripts::NLINK_ATTRS,
        FighterKind::PolyYoshi => &scripts::NYOSHI_ATTRS,
        FighterKind::PolyCaptain => &scripts::NCAPTAIN_ATTRS,
        FighterKind::PolyKirby => &scripts::NKIRBY_ATTRS,
        FighterKind::PolyPikachu => &scripts::NPIKACHU_ATTRS,
        FighterKind::PolyPurin => &scripts::NPURIN_ATTRS,
        FighterKind::PolyNess => &scripts::NNESS_ATTRS,
        FighterKind::GiantDonkey => &scripts::GDONKEY_ATTRS,
    })
}

/// `FTMotionEvent`.
mod op {
    pub const END: u32 = 0;
    pub const SYNC_WAIT: u32 = 1;
    pub const ASYNC_WAIT: u32 = 2;
    pub const MAKE_ATTACK_COLL: u32 = 3;
    pub const MAKE_ATTACK_COLL_SCALED: u32 = 4;
    pub const CLEAR_ATTACK_COLL_ID: u32 = 5;
    pub const CLEAR_ATTACK_COLL_ALL: u32 = 6;
    pub const SET_ATTACK_COLL_OFFSET: u32 = 7;
    pub const SET_ATTACK_COLL_DAMAGE: u32 = 8;
    pub const SET_ATTACK_COLL_SIZE: u32 = 9;
    pub const SET_ATTACK_COLL_SOUND_LEVEL: u32 = 10;
    pub const REFRESH_ATTACK_COLL_ID: u32 = 11;
    pub const SET_THROW: u32 = 12;
    pub const SET_DAMAGE_THROWN: u32 = 13;
    pub const PLAY_FGM: u32 = 14;
    pub const PLAY_LOOP_SFX_STORE_INFO: u32 = 15;
    pub const STOP_LOOP_SFX: u32 = 16;
    pub const PLAY_VOICE_STORE_INFO: u32 = 17;
    pub const PLAY_LOOP_VOICE_STORE_INFO: u32 = 18;
    pub const PLAY_FGM_STORE_INFO: u32 = 19;
    pub const PLAY_SMASH_VOICE: u32 = 20;
    pub const SET_FLAG0: u32 = 21;
    pub const SET_FLAG3: u32 = 24;
    pub const SET_AIR_JUMP_ADD: u32 = 25;
    pub const SET_AIR_JUMP_MAX: u32 = 26;
    pub const SET_HIT_STATUS_PART_ALL: u32 = 27;
    pub const SET_HIT_STATUS_PART_ID: u32 = 28;
    pub const SET_HIT_STATUS_ALL: u32 = 29;
    pub const RESET_DAMAGE_COLL_PART_ALL: u32 = 30;
    pub const SET_DAMAGE_COLL_PART_ID: u32 = 31;
    pub const LOOP_BEGIN: u32 = 32;
    pub const LOOP_END: u32 = 33;
    pub const SUBROUTINE: u32 = 34;
    pub const RETURN: u32 = 35;
    pub const GOTO: u32 = 36;
    pub const PAUSE_SCRIPT: u32 = 37;
    pub const EFFECT: u32 = 38;
    pub const EFFECT_ITEM_HOLD: u32 = 39;
    pub const SET_MODEL_PART_ID: u32 = 40;
    pub const RESET_MODEL_PART_ALL: u32 = 41;
    pub const HIDE_MODEL_PART_ALL: u32 = 42;
    pub const SET_TEXTURE_PART_ID: u32 = 43;
    pub const SET_COL_ANIM: u32 = 44;
    pub const RESET_COL_ANIM: u32 = 45;
    pub const SET_PARALLEL_SCRIPT: u32 = 46;
}

/// Words each command occupies (`ftMotionEventAdvance` sizes).
fn command_words(opcode: u32) -> u32 {
    match opcode {
        op::MAKE_ATTACK_COLL | op::MAKE_ATTACK_COLL_SCALED => 5,
        op::EFFECT | op::EFFECT_ITEM_HOLD | op::SET_DAMAGE_COLL_PART_ID => 4,
        op::SET_ATTACK_COLL_OFFSET
        | op::SET_THROW
        | op::SET_DAMAGE_THROWN
        | op::SUBROUTINE
        | op::GOTO
        | op::SET_PARALLEL_SCRIPT => 2,
        _ => 1,
    }
}

const STACK_MAX: usize = 8;

/// `FTMotionScript`: one running script. `p_goto`/`loop_count` share one
/// stack indexed by `script_id`; a loop takes two slots, a subroutine one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScriptThread {
    pub pc: u32,
    pub wait: f32,
    script_id: usize,
    p_goto: [u32; STACK_MAX],
    loop_count: [i32; STACK_MAX],
}

impl Default for ScriptThread {
    fn default() -> Self {
        ScriptThread {
            pc: NO_SCRIPT,
            wait: 0.0,
            script_id: 0,
            p_goto: [NO_SCRIPT; STACK_MAX],
            loop_count: [0; STACK_MAX],
        }
    }
}

/// `FTStruct::motion_scripts` and `motion_vars.flags`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct MotionState {
    /// `motion_scripts[0]`: `[0]` the status's script, `[1]` a
    /// `SetParallelScript` thread.
    pub threads: [ScriptThread; 2],
    /// `motion_scripts[1]`: the copies whose `Effect` events
    /// `ftMainUpdateMotionEventsForwardEffect` makes at the end of
    /// `ftMainProcPhysicsMap`.
    pub effect_threads: [ScriptThread; 2],
    /// `is_events_forward`: set from `ftMainProcUpdateInterrupt` to the end
    /// of `ftMainProcPhysicsMap`. While it is set the status scripts skip
    /// their `Effect` events, which the effect copies make later.
    pub is_events_forward: bool,
    /// `motion_vars.flags.flag0`..`flag3`.
    pub flags: [u32; 4],
}

/// How a pass reads the script (`ftMainUpdateMotionEventsAll`,
/// `...Forward` and `...ForwardEffect`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pass {
    All,
    Forward,
    ForwardEffect,
}

/// The fighter's script table (every kind has one).
pub fn fighter_scripts(kind: FighterKind) -> Option<&'static FighterScripts> {
    Some(match kind {
        FighterKind::Mario => &scripts::MARIO,
        FighterKind::Fox => &scripts::FOX,
        FighterKind::Donkey => &scripts::DONKEY,
        FighterKind::Samus => &scripts::SAMUS,
        FighterKind::Luigi => &scripts::LUIGI,
        FighterKind::Link => &scripts::LINK,
        FighterKind::Yoshi => &scripts::YOSHI,
        FighterKind::Captain => &scripts::CAPTAIN,
        FighterKind::Kirby => &scripts::KIRBY,
        FighterKind::Pikachu => &scripts::PIKACHU,
        FighterKind::Purin => &scripts::PURIN,
        FighterKind::Ness => &scripts::NESS,
        FighterKind::Boss => &scripts::BOSS,
        FighterKind::MetalMario => &scripts::MMARIO,
        FighterKind::PolyMario => &scripts::NMARIO,
        FighterKind::PolyFox => &scripts::NFOX,
        FighterKind::PolyDonkey => &scripts::NDONKEY,
        FighterKind::PolySamus => &scripts::NSAMUS,
        FighterKind::PolyLuigi => &scripts::NLUIGI,
        FighterKind::PolyLink => &scripts::NLINK,
        FighterKind::PolyYoshi => &scripts::NYOSHI,
        FighterKind::PolyCaptain => &scripts::NCAPTAIN,
        FighterKind::PolyKirby => &scripts::NKIRBY,
        FighterKind::PolyPikachu => &scripts::NPIKACHU,
        FighterKind::PolyPurin => &scripts::NPURIN,
        FighterKind::PolyNess => &scripts::NNESS,
        FighterKind::GiantDonkey => &scripts::GDONKEY,
    })
}

/// `status_desc->mflags.motion_id`, or `None` for `-1`/`-2` (no new motion).
pub fn motion_id(kind: FighterKind, status: AnyStatus) -> Option<usize> {
    let id = status.id();
    let motion = if id < SPECIAL_STATUS_START {
        *COMMON_STATUS_MOTION.get(usize::from(id))?
    } else {
        *fighter_scripts(kind)?
            .special_status_motion
            .get(usize::from(id - SPECIAL_STATUS_START))?
    };
    usize::try_from(motion).ok()
}

/// The motion desc a status plays.
pub fn motion_desc(kind: FighterKind, status: AnyStatus) -> Option<MotionDesc> {
    let table = fighter_scripts(kind)?;
    table.motions.get(motion_id(kind, status)?).copied()
}

/// What `ftMainSetStatus` does to the figatree and its clock
/// (`gobj->anim_frame`) when it enters `status` (RE-474).
pub fn clip_on_set_status(kind: FighterKind, status: AnyStatus) -> crate::status::ClipStart {
    use crate::status::{Clip, ClipStart};
    let Some(table) = fighter_scripts(kind) else {
        return ClipStart::Restart(Clip::Unknown);
    };
    let id = status.id();
    let motion = if id < SPECIAL_STATUS_START {
        COMMON_STATUS_MOTION.get(usize::from(id)).copied()
    } else {
        table
            .special_status_motion
            .get(usize::from(id - SPECIAL_STATUS_START))
            .copied()
    };
    match motion {
        None => ClipStart::Restart(Clip::Unknown),
        // `motion_id` -1: the figatree plays on; -2: `ftParamPlayAnim`
        // skips the figatrees, so it stands still.
        Some(-1) => ClipStart::Keep,
        Some(m) if m < 0 => ClipStart::Freeze,
        Some(m) => match table.motions.get(m as usize).map(|d| d.clip) {
            None | Some(CLIP_UNKNOWN) => ClipStart::Restart(Clip::Unknown),
            Some(CLIP_NONE) => ClipStart::Keep,
            Some(c) if c & CLIP_LOOPS != 0 => {
                ClipStart::Restart(Clip::Loops(f32::from(c & !CLIP_LOOPS)))
            }
            Some(c) => ClipStart::Restart(Clip::Ends(f32::from(c))),
        },
    }
}

/// The status's figatree length in frames, for statuses that end with their
/// animation (`ftAnimEndCheckSetStatus`). `None` without a figatree, for a
/// looping one, or for an unported fighter.
pub fn anim_length(kind: FighterKind, status: AnyStatus) -> Option<f32> {
    match motion_desc(kind, status)?.anim_length {
        0 | 0xFFFF => None,
        n => Some(f32::from(n)),
    }
}

/// `ftMainSetStatus`'s script half: every thread stops, the status's own
/// script starts `frame_begin` into its motion, and the time-zero commands
/// run (or, entered part-way, the script fast-forwards). A status with no
/// motion (`-1`/`-2`) leaves every thread stopped.
pub fn start(f: &mut Fighter, frame_begin: f32) {
    f.motion_script.threads = [ScriptThread::default(); 2];
    f.motion_script.effect_threads = [ScriptThread::default(); 2];
    let Some(desc) = motion_desc(f.kind, f.status.status) else {
        return;
    };
    let speed = f.status.timing.anim_speed;
    f.motion_script.threads[0] = ScriptThread {
        pc: desc.script,
        wait: speed - frame_begin,
        ..ScriptThread::default()
    };
    f.motion_script.effect_threads[0] = f.motion_script.threads[0];
    if frame_begin != 0.0 {
        run_all(f, Pass::Forward);
        // `ftMainUpdateMotionEventsForward` always copies.
        f.motion_script.effect_threads = f.motion_script.threads;
    } else {
        advance(f);
    }
    // `ftMainSetStatus`: a status that starts at frame 0 also runs a frame
    // of its colour animation.
    if frame_begin == 0.0 {
        crate::colanim::run_update(f);
    }
}

/// `ftMainUpdateMotionEventsAll`: one frame of every running thread. Call
/// once per frame outside hitlag, after the animation clock advances.
pub fn advance(f: &mut Fighter) {
    run_all(f, Pass::All);
    if !f.motion_script.is_events_forward {
        f.motion_script.effect_threads = f.motion_script.threads;
    }
}

/// `ftMainUpdateMotionEventsForwardEffect`: the effect copies run their
/// flow and `Effect` events only. `ftMainProcPhysicsMap` runs it outside
/// hitlag, after `proc_map`; `ftMainSetStatus` before a new status's
/// scripts while the frame's passes run.
pub fn forward_effect(f: &mut Fighter) {
    run_all(f, Pass::ForwardEffect);
}

/// The end of `ftMainProcPhysicsMap`: the effect copies' pass outside
/// hitlag, then `is_events_forward = FALSE`.
pub fn end_physics(f: &mut Fighter) {
    if !f.is_in_hitlag() {
        forward_effect(f);
    }
    f.motion_script.is_events_forward = false;
}

fn run_all(f: &mut Fighter, pass: Pass) {
    for thread in 0..2 {
        run(f, thread, pass);
    }
}

fn thread_mut(f: &mut Fighter, pass: Pass, thread: usize) -> &mut ScriptThread {
    if pass == Pass::ForwardEffect {
        &mut f.motion_script.effect_threads[thread]
    } else {
        &mut f.motion_script.threads[thread]
    }
}

fn sign(value: u32, bits: u32) -> i32 {
    let shift = 32 - bits;
    ((value << shift) as i32) >> shift
}

fn run(f: &mut Fighter, thread: usize, pass: Pass) {
    let Some(table) = fighter_scripts(f.kind) else {
        return;
    };
    let words = table.words;
    let speed = f.status.timing.anim_speed;
    let anim_frame = f.status.anim_frame;
    {
        let t = thread_mut(f, pass, thread);
        if t.pc == NO_SCRIPT {
            return;
        }
        if t.wait != f32::MAX {
            t.wait -= speed;
        }
    }
    // A script that loops without waiting would hang the original too; the
    // cap only keeps a data error from hanging the port.
    for _ in 0..4096 {
        let t = *thread_mut(f, pass, thread);
        let (words, base) = if t.pc != NO_SCRIPT && t.pc & COMMON_BIT != 0 {
            (&scripts::COMMON_MOVESET_WORDS[..], COMMON_BIT)
        } else {
            (words, 0)
        };
        if t.pc == NO_SCRIPT || (t.pc - base) as usize >= words.len() {
            thread_mut(f, pass, thread).pc = NO_SCRIPT;
            return;
        }
        if t.wait == f32::MAX {
            if speed <= anim_frame {
                return;
            }
            thread_mut(f, pass, thread).wait = -anim_frame;
        } else if t.wait > 0.0 {
            return;
        }
        let pc = (t.pc - base) as usize;
        let w = words[pc];
        let opcode = w >> 26;
        let is_effect = matches!(opcode, op::EFFECT | op::EFFECT_ITEM_HOLD);
        let skip = match pass {
            Pass::All => is_effect && f.motion_script.is_events_forward,
            Pass::Forward => {
                is_effect
                    || matches!(
                        opcode,
                        op::MAKE_ATTACK_COLL
                            | op::MAKE_ATTACK_COLL_SCALED
                            | op::SET_ATTACK_COLL_OFFSET
                            | op::CLEAR_ATTACK_COLL_ID
                            | op::CLEAR_ATTACK_COLL_ALL
                            | op::SET_ATTACK_COLL_DAMAGE
                            | op::SET_ATTACK_COLL_SIZE
                            | op::SET_ATTACK_COLL_SOUND_LEVEL
                            | op::REFRESH_ATTACK_COLL_ID
                            | op::PLAY_FGM..=op::PLAY_SMASH_VOICE
                            | op::SET_FLAG0
                            ..=23
                                | op::SET_AIR_JUMP_ADD
                                | op::SET_AIR_JUMP_MAX
                                | op::SET_COL_ANIM
                                | op::RESET_COL_ANIM
                    )
            }
            Pass::ForwardEffect => {
                !(is_effect
                    || matches!(
                        opcode,
                        op::END
                            | op::SYNC_WAIT
                            | op::ASYNC_WAIT
                            | op::SET_DAMAGE_THROWN
                            | op::LOOP_BEGIN
                            | op::LOOP_END
                            | op::SUBROUTINE
                            | op::RETURN
                            | op::GOTO
                            | op::PAUSE_SCRIPT
                    ))
            }
        };
        if skip {
            thread_mut(f, pass, thread).pc += command_words(opcode);
            continue;
        }
        execute(
            f, pass, thread, words, base, pc, w, opcode, anim_frame, speed,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn execute(
    f: &mut Fighter,
    pass: Pass,
    thread: usize,
    words: &'static [u32],
    base: u32,
    pc: usize,
    w: u32,
    opcode: u32,
    anim_frame: f32,
    speed: f32,
) {
    let value = w & 0x03FF_FFFF;
    let next = base + (pc as u32) + command_words(opcode);
    let word = |i: usize| words.get(pc + i).copied().unwrap_or(0);
    match opcode {
        op::END => {
            thread_mut(f, pass, thread).pc = NO_SCRIPT;
            return;
        }
        op::SYNC_WAIT => thread_mut(f, pass, thread).wait += value as f32,
        op::ASYNC_WAIT => thread_mut(f, pass, thread).wait = value as f32 - anim_frame,
        op::MAKE_ATTACK_COLL | op::MAKE_ATTACK_COLL_SCALED => {
            make_attack_coll(f, [w, word(1), word(2), word(3), word(4)], opcode);
        }
        op::SET_ATTACK_COLL_OFFSET => {
            let id = ((w >> 23) & 7) as usize;
            let w2 = word(1);
            if let Some(coll) = f.attack_colls.get_mut(id) {
                coll.offset = Vec3::new(
                    sign((w >> 7) & 0xFFFF, 16) as f32,
                    sign(w2 >> 16, 16) as f32,
                    sign(w2 & 0xFFFF, 16) as f32,
                );
            }
        }
        op::SET_ATTACK_COLL_DAMAGE => {
            let id = ((w >> 23) & 7) as usize;
            let damage = ((w >> 15) & 0xFF) as i32;
            if id < f.attack_colls.len() {
                let coll = f.attack_colls[id];
                f.attack_colls[id].damage =
                    f.stale
                        .staled_damage(damage, coll.motion_attack_id, coll.motion_count);
            }
        }
        op::SET_ATTACK_COLL_SIZE => {
            let id = ((w >> 23) & 7) as usize;
            if let Some(coll) = f.attack_colls.get_mut(id) {
                coll.size = ((w >> 7) & 0xFFFF) as f32 * 0.5;
            }
        }
        op::SET_ATTACK_COLL_SOUND_LEVEL => {
            let id = ((w >> 23) & 7) as usize;
            if let Some(coll) = f.attack_colls.get_mut(id) {
                coll.fgm_level = ((w >> 20) & 7) as u8;
            }
        }
        op::REFRESH_ATTACK_COLL_ID => {
            // `ftParamRefreshAttackCollID`.
            if let Some(coll) = f.attack_colls.get_mut(value as usize) {
                coll.state = AttackState::New;
                coll.clear_records();
            }
        }
        op::CLEAR_ATTACK_COLL_ID => {
            if let Some(coll) = f.attack_colls.get_mut(value as usize) {
                coll.state = AttackState::Off;
            }
        }
        op::CLEAR_ATTACK_COLL_ALL => crate::combat::clear_attack_colls(f),
        // The sound commands. `is_muted` gates them all but
        // `StopLoopSFX`; only the Characters menu mutes a fighter, and its
        // fighters run no motion script here, so the gate always passes.
        // `nFTMotionEventPlayFGMStoreInfo`.
        op::PLAY_FGM_STORE_INFO => crate::fighter_sound::play_fgm_store_info(f, value as u16),
        // `nFTMotionEventPlayFGM`: the handle is dropped.
        op::PLAY_FGM => {
            crate::sound::play_fgm(value as u16);
        }
        op::PLAY_LOOP_SFX_STORE_INFO => crate::fighter_sound::play_loop_sfx(f, value as u16),
        op::STOP_LOOP_SFX => crate::fighter_sound::stop_loop_sfx(f),
        // `nFTMotionEventPlayVoiceStoreInfo` and
        // `nFTMotionEventPlayLoopVoiceStoreInfo` (into the loop slot, so a
        // status change stops it): `attr->is_have_voice` gates them.
        op::PLAY_VOICE_STORE_INFO => {
            if has_voice(f) {
                crate::fighter_sound::play_voice(f, value as u16);
            }
        }
        op::PLAY_LOOP_VOICE_STORE_INFO => {
            if has_voice(f) {
                crate::fighter_sound::play_loop_sfx(f, value as u16);
            }
        }
        // `nFTMotionEventPlaySmashVoice`: one of `attr->smash_sfx[3]` at
        // random. The draw comes first and always happens, as the
        // original's does (RE-468: Mario's up smash in How to Play).
        op::PLAY_SMASH_VOICE => {
            let r = crate::rng::rand_int_range(3) as usize;
            if let Some(a) = combat_attrs(f.kind) {
                crate::fighter_sound::play_voice(f, a.smash_sfx[r]);
            }
        }

        op::SET_FLAG0..=op::SET_FLAG3 => {
            f.motion_script.flags[(opcode - op::SET_FLAG0) as usize] = value;
        }
        op::SET_AIR_JUMP_ADD => {
            f.become_airborne();
            f.physics.vel_air.z = 0.0;
            f.pos.z = 0.0;
            f.physics.jumps_used = f.physics.jumps_used.saturating_add(1);
        }
        op::SET_AIR_JUMP_MAX => {
            f.become_airborne();
            f.physics.vel_air.z = 0.0;
            f.pos.z = 0.0;
            f.physics.jumps_used = f.attributes.jumps_max;
        }
        op::SET_HIT_STATUS_PART_ALL => {
            crate::hurtbox::set_hit_status_part_all(f, HitStatus::from_raw(value));
        }
        op::SET_HIT_STATUS_PART_ID => {
            let joint = sign((w >> 19) & 0x7F, 7);
            let status = HitStatus::from_raw(w & 0x7FFFF);
            crate::hurtbox::set_hit_status_part_id(f, joint, status);
        }
        op::SET_HIT_STATUS_ALL => {
            crate::hurtbox::set_hit_status_all(f, HitStatus::from_raw(value));
        }
        op::RESET_DAMAGE_COLL_PART_ALL => crate::hurtbox::reset_damage_colls(f),
        op::SET_DAMAGE_COLL_PART_ID => {
            let joint = sign((w >> 19) & 0x7F, 7);
            let (w2, w3, w4) = (word(1), word(2), word(3));
            let offset = Vec3::new(
                sign(w2 >> 16, 16) as f32,
                sign(w2 & 0xFFFF, 16) as f32,
                sign(w3 >> 16, 16) as f32,
            );
            let size = Vec3::new(
                sign(w3 & 0xFFFF, 16) as f32,
                sign(w4 >> 16, 16) as f32,
                sign(w4 & 0xFFFF, 16) as f32,
            );
            crate::hurtbox::modify_damage_coll(f, joint, offset, size);
        }
        op::LOOP_BEGIN => {
            let t = thread_mut(f, pass, thread);
            if t.script_id + 2 <= STACK_MAX {
                t.p_goto[t.script_id] = next;
                t.loop_count[t.script_id] = value as i32;
                t.script_id += 2;
            }
        }
        op::LOOP_END => {
            let t = thread_mut(f, pass, thread);
            if t.script_id >= 2 {
                let slot = t.script_id - 2;
                t.loop_count[slot] -= 1;
                if t.loop_count[slot] != 0 {
                    t.pc = t.p_goto[slot];
                    return;
                }
                t.script_id -= 2;
            }
        }
        op::SUBROUTINE | op::SET_DAMAGE_THROWN => {
            let target = if opcode == op::SET_DAMAGE_THROWN {
                f.thrown
                    .owner
                    .and_then(|owner| {
                        let table = word(1);
                        if table == NO_SCRIPT || f.thrown.script_id >= 2 {
                            return None;
                        }
                        let (data, table_base) = if table & COMMON_BIT != 0 {
                            (&scripts::COMMON_MOVESET_WORDS[..], COMMON_BIT)
                        } else {
                            (fighter_scripts(f.kind)?.words, 0)
                        };
                        data.get(
                            (table - table_base) as usize
                                + usize::from(f.thrown.script_id) * 27
                                + owner.kind as usize,
                        )
                        .copied()
                    })
                    .unwrap_or(NO_SCRIPT)
            } else {
                word(1)
            };
            let t = thread_mut(f, pass, thread);
            if target == NO_SCRIPT || t.script_id >= STACK_MAX {
                // No subroutine (or no throwing fighter).
                t.pc = next;
                return;
            }
            t.p_goto[t.script_id] = next;
            t.script_id += 1;
            t.pc = target;
            return;
        }
        op::RETURN => {
            let t = thread_mut(f, pass, thread);
            if t.script_id == 0 {
                t.pc = NO_SCRIPT;
                return;
            }
            t.script_id -= 1;
            t.pc = t.p_goto[t.script_id];
            return;
        }
        op::GOTO => {
            thread_mut(f, pass, thread).pc = word(1);
            return;
        }
        op::PAUSE_SCRIPT => thread_mut(f, pass, thread).wait = f32::MAX,
        // `fp->is_effect_skip` (Mushroom Kingdom's pipes) skips the event.
        op::EFFECT | op::EFFECT_ITEM_HOLD if f.dokan.is_effect_skip => {}
        op::EFFECT | op::EFFECT_ITEM_HOLD => {
            let (w2, w3, w4) = (word(1), word(2), word(3));
            let joint = crate::fteffect::joint_id(f.kind, sign((w >> 19) & 0x7F, 7) as i8);
            crate::fteffect::request(
                f,
                crate::fteffect::EffectRequest {
                    kind: ((w >> 10) & 0x1FF) as u16,
                    joint,
                    offset: Some(Vec3::new(
                        sign(w2 >> 16, 16) as f32,
                        sign(w2 & 0xFFFF, 16) as f32,
                        sign(w3 >> 16, 16) as f32,
                    )),
                    scatter: Some(Vec3::new(
                        sign(w3 & 0xFFFF, 16) as f32,
                        sign(w4 >> 16, 16) as f32,
                        sign(w4 & 0xFFFF, 16) as f32,
                    )),
                    lr: f.facing.sign() as i8,
                    is_scale_pos: opcode == op::EFFECT_ITEM_HOLD,
                    flag: (w & 0x3FF) as u16,
                },
            );
        }
        op::SET_COL_ANIM => {
            // `ftMotionCommandSetColAnim(id, length)`: 8-bit id, 18-bit length.
            let id = crate::colanim::ColAnimId(((w >> 18) & 0xFF) as u8);
            crate::colanim::check_set(f, id, (w & 0x3_FFFF) as i32);
        }
        op::RESET_COL_ANIM => crate::colanim::reset_stat_update(f),
        op::SET_MODEL_PART_ID => {
            // `ftMotionCommandSetModelPartID(jid, mid)`: a 7-bit joint and a
            // 19-bit part, both signed (RE-425).
            let joint = sign((w >> 19) & 0x7F, 7);
            let part = sign(w & 0x7_FFFF, 19);
            if let Some(joint) = crate::modelpart::motion_joint(f, joint) {
                f.model_parts.set(joint, part as i8);
            }
        }
        op::RESET_MODEL_PART_ALL => f.model_parts.reset_all(),
        op::HIDE_MODEL_PART_ALL => f.model_parts.hide_all(),
        op::SET_TEXTURE_PART_ID => {
            // `ftMotionCommandSetTexturePartID`: a 6-bit part and a 20-bit
            // texture id (RE-426).
            let (part, id) = texture_part_event(w);
            f.model_parts.set_texture(part, id);
        }
        op::SET_PARALLEL_SCRIPT => {
            let target = word(1);
            if thread == 0 && f.motion_script.threads[1].pc == NO_SCRIPT && target != NO_SCRIPT {
                f.motion_script.threads[1] = ScriptThread {
                    pc: target,
                    wait: speed - anim_frame,
                    ..ScriptThread::default()
                };
                f.motion_script.effect_threads[1] = f.motion_script.threads[1];
            }
        }
        _ => {}
    }
    thread_mut(f, pass, thread).pc = next;
}

/// `FTMotionEventSetTexturePartID`'s `(texturepart_id, frame)`.
fn texture_part_event(w: u32) -> (usize, i8) {
    (((w >> 20) & 0x3F) as usize, (w & 0xF_FFFF) as i8)
}

/// Every `SetModelPartID` a playable fighter's scripts can reach, as
/// `(joint, part)` with `ftParamGetJointID`'s -2 resolved, walked from each
/// motion's script through its gotos, subroutines and parallel scripts. The
/// asset pipeline packs the parts these name (RE-425).
#[cfg(feature = "std")]
pub fn model_part_events(kind: FighterKind) -> std::collections::BTreeSet<(i32, i32)> {
    part_events(kind, op::SET_MODEL_PART_ID)
}

/// Every `SetTexturePartID` a playable fighter's scripts can reach, as
/// `(texturepart_id, texture id)`, from the battle motions and the demo
/// statuses' scripts (RE-426). The asset pipeline packs the textures these
/// name.
#[cfg(feature = "std")]
pub fn texture_part_events(kind: FighterKind) -> std::collections::BTreeSet<(i32, i32)> {
    part_events(kind, op::SET_TEXTURE_PART_ID)
}

#[cfg(feature = "std")]
fn part_events(kind: FighterKind, wanted: u32) -> std::collections::BTreeSet<(i32, i32)> {
    let mut out = std::collections::BTreeSet::new();
    let Some(table) = fighter_scripts(kind) else {
        return out;
    };
    let item_joint = combat_attrs(kind).map_or(-2, |a| i32::from(a.joint_itemlight_id));
    let mut todo: Vec<u32> = table
        .motions
        .iter()
        .map(|m| m.script)
        .filter(|&s| s != NO_SCRIPT)
        .collect();
    // The demo statuses' scripts (`scsubsysdata*.c`) live in their own
    // word blob, marked with `DEMO_BIT`.
    if let Some(demo) = demo_scripts(kind) {
        todo.extend(
            demo.rows
                .iter()
                .filter(|&&s| s != NO_SCRIPT)
                .map(|&s| s | DEMO_BIT),
        );
    }
    let mut seen = std::collections::BTreeSet::new();
    while let Some(start) = todo.pop() {
        let mut pc = start;
        loop {
            if !seen.insert(pc) {
                break;
            }
            let (words, base) = if pc & DEMO_BIT != 0 {
                (demo_scripts(kind).map_or(&[][..], |d| d.words), DEMO_BIT)
            } else if pc & COMMON_BIT != 0 {
                (&scripts::COMMON_MOVESET_WORDS[..], COMMON_BIT)
            } else {
                (table.words, 0)
            };
            let Some(&w) = words.get((pc - base) as usize) else {
                break;
            };
            let opcode = w >> 26;
            let target = words.get((pc - base) as usize + 1).copied();
            match opcode {
                op::END | op::RETURN => break,
                op::GOTO => {
                    todo.extend(
                        target
                            .filter(|&t| t != NO_SCRIPT)
                            .map(|t| t | (pc & DEMO_BIT)),
                    );
                    break;
                }
                op::SUBROUTINE | op::SET_PARALLEL_SCRIPT => {
                    todo.extend(
                        target
                            .filter(|&t| t != NO_SCRIPT)
                            .map(|t| t | (pc & DEMO_BIT)),
                    );
                }
                op::SET_MODEL_PART_ID if wanted == op::SET_MODEL_PART_ID => {
                    let joint = sign((w >> 19) & 0x7F, 7);
                    let joint = if joint == -2 { item_joint } else { joint };
                    out.insert((joint, sign(w & 0x7_FFFF, 19)));
                }
                op::SET_TEXTURE_PART_ID if wanted == op::SET_TEXTURE_PART_ID => {
                    let (part, id) = texture_part_event(w);
                    out.insert((part as i32, i32::from(id)));
                }
                _ => {}
            }
            pc += command_words(opcode);
        }
    }
    out
}

/// `nFTMotionEventMakeAttackColl`: a new slot, or a slot whose group
/// changes, starts a fresh hit record unless another live slot of the same
/// group already has one, which it copies.
fn make_attack_coll(f: &mut Fighter, w: [u32; 5], opcode: u32) {
    let id = ((w[0] >> 23) & 7) as usize;
    if id >= f.attack_colls.len() {
        return;
    }
    let group = ((w[0] >> 20) & 7) as u8;
    let current = f.attack_colls[id];
    if current.state == AttackState::Off || current.group != group {
        let shared = f
            .attack_colls
            .iter()
            .enumerate()
            .find(|(i, c)| *i != id && c.state != AttackState::Off && c.group == group)
            .map(|(_, c)| c.records);
        let coll = &mut f.attack_colls[id];
        coll.group = group;
        coll.state = AttackState::New;
        match shared {
            Some(records) => coll.records = records,
            None => coll.clear_records(),
        }
    }
    // `ftParamGetJointID`'s `-2` (the item joint) is used only by the item
    // movesets, which live outside the fighter files.
    let joint = sign((w[0] >> 13) & 0x7F, 7);
    let raw_damage = ((w[0] >> 5) & 0xFF) as i32;
    let (attack_id, count) = (f.motion.attack_id, f.motion.count);
    let damage = f.stale.staled_damage(raw_damage, attack_id, count);
    let coll = &mut f.attack_colls[id];
    coll.joint = joint.max(0) as u8;
    coll.damage = damage;
    coll.can_rebound = (w[0] >> 4) & 1 != 0;
    coll.element = Element::from_raw(w[0] & 0xF);
    coll.size = (w[1] >> 16) as f32 * 0.5;
    coll.offset = Vec3::new(
        sign(w[1] & 0xFFFF, 16) as f32,
        sign(w[2] >> 16, 16) as f32,
        sign(w[2] & 0xFFFF, 16) as f32,
    );
    coll.angle = sign(w[3] >> 22, 10);
    coll.kb_scale = ((w[3] >> 12) & 0x3FF) as i32;
    coll.kb_weight = ((w[3] >> 2) & 0x3FF) as i32;
    coll.is_hit_air = w[3] & 1 != 0;
    coll.is_hit_ground = w[3] & 2 != 0;
    coll.shield_damage = sign(w[4] >> 24, 8);
    coll.fgm_level = ((w[4] >> 21) & 7) as u8;
    coll.fgm_kind = ((w[4] >> 17) & 0xF) as u8;

    coll.kb_base = ((w[4] >> 7) & 0x3FF) as i32;
    coll.is_scale_pos = opcode == op::MAKE_ATTACK_COLL_SCALED;
    coll.motion_attack_id = attack_id;
    coll.motion_count = count;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::status::{self, Status, StatusTiming};

    fn fighter(kind: FighterKind) -> Fighter {
        let mut f = Fighter::new(kind, 0, 3);
        f.situation = crate::fighter::Situation::Ground;
        f
    }

    fn live(f: &Fighter) -> usize {
        f.attack_colls
            .iter()
            .filter(|c| c.state != AttackState::Off)
            .count()
    }

    /// `dMarioMainMotion_Jab1`: `WaitAsync(2)`, two boxes, `Wait(2)`,
    /// `ClearAttackCollAll`.
    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn mario_jab_boxes_follow_the_script() {
        let mut f = fighter(FighterKind::Mario);
        status::set_status(&mut f, Status::Attack11, 0.0, StatusTiming::frames(18.0));
        let mut active = [false; 8];
        assert_eq!(live(&f), 0);
        for slot in active.iter_mut() {
            f.status.anim_frame += 1.0;
            advance(&mut f);
            *slot = live(&f) > 0;
        }
        assert_eq!(
            active,
            [false, true, true, false, false, false, false, false]
        );
    }

    #[test]
    fn every_motion_script_runs_to_a_wait_or_an_end() {
        for kind in [
            FighterKind::Mario,
            FighterKind::Fox,
            FighterKind::Donkey,
            FighterKind::Samus,
            FighterKind::Luigi,
            FighterKind::Link,
            FighterKind::Yoshi,
            FighterKind::Captain,
            FighterKind::Kirby,
            FighterKind::Pikachu,
            FighterKind::Purin,
            FighterKind::Ness,
        ] {
            let table = fighter_scripts(kind).unwrap();
            for desc in table.motions {
                let mut f = fighter(kind);
                f.motion_script.threads[0] = ScriptThread {
                    pc: desc.script,
                    wait: 0.0,
                    ..ScriptThread::default()
                };
                for _ in 0..300 {
                    f.status.anim_frame += 1.0;
                    advance(&mut f);
                }
            }
        }
    }

    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn roll_is_intangible_from_frame_4_to_20() {
        let mut f = fighter(FighterKind::Mario);
        status::set_status(&mut f, Status::EscapeF, 0.0, StatusTiming::unknown());
        let mut intangible = [false; 22];
        for (frame, slot) in intangible.iter_mut().enumerate() {
            if frame > 0 {
                f.status.anim_frame += 1.0;
                advance(&mut f);
            }
            *slot = f.hitstatus == HitStatus::Intangible;
        }
        assert!(!intangible[3] && intangible[4] && intangible[19] && !intangible[20]);
    }
}
