//! Fighter motion scripts — `ftMainParseMotionEvent`,
//! `ftMainUpdateMotionEventsAll` and `ftMainUpdateMotionEventsForward`.
//!
//! A status's attack collisions, hit-status windows, hurtbox edits and
//! script flags are `ftMotionCommand` bytecode in the fighter's
//! `relocData/*MainMotion.c`, not tables. The generated `scripts` module carries that bytecode
//! word for word (`tools/gen-motion-scripts.py`), and this module runs it the
//! way the original does: `ftMainSetStatus` starts the status's script with
//! `script_wait = anim_speed - frame_begin`, and every frame outside hitlag
//! the wait drops by the animation speed and the commands run until the next
//! wait. A status entered part-way (`frame_begin != 0`) fast-forwards,
//! skipping attack creation, sounds, flags and effects exactly as
//! `ftMainUpdateMotionEventsForward` does.
//!
//! Only commands with gameplay effects act here: attack collisions,
//! hit statuses, hurtbox edits, the four flags and the aerial-jump commands.
//! Effects, sounds, rumble, colour animations, model/texture parts, slope
//! contours and throw descriptors are decoded and skipped; the ported status
//! code owns throws.

mod scripts;

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
}

/// One fighter's `*MainMotion` file and motion table.
#[derive(Debug)]
pub struct FighterScripts {
    pub words: &'static [u32],
    pub motions: &'static [MotionDesc],
    /// `dFT<Name>SpecialStatusDescs[..].mflags.motion_id`.
    pub special_status_motion: &'static [i16],
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
}

/// The fighter's [`CombatAttrs`], or `None` for an unported fighter.
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
        _ => return None,
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

/// `FTStruct::motion_scripts[0]` and `motion_vars.flags`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct MotionState {
    /// `[0]` the status's script, `[1]` a `SetParallelScript` thread.
    pub threads: [ScriptThread; 2],
    /// `motion_vars.flags.flag0`..`flag3`.
    pub flags: [u32; 4],
}

/// The fighter's script table, or `None` for an unported fighter.
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
        _ => return None,
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
    let Some(desc) = motion_desc(f.kind, f.status.status) else {
        return;
    };
    let speed = f.status.timing.anim_speed;
    f.motion_script.threads[0] = ScriptThread {
        pc: desc.script,
        wait: speed - frame_begin,
        ..ScriptThread::default()
    };
    run_all(f, frame_begin != 0.0);
    // `ftMainSetStatus`: a status that starts at frame 0 also runs a frame
    // of its colour animation.
    if frame_begin == 0.0 {
        f.colanim.run_update();
    }
}

/// `ftMainUpdateMotionEventsAll`: one frame of every running thread. Call
/// once per frame outside hitlag, after the animation clock advances.
pub fn advance(f: &mut Fighter) {
    run_all(f, false);
}

fn run_all(f: &mut Fighter, forward: bool) {
    for thread in 0..2 {
        run(f, thread, forward);
    }
}

fn sign(value: u32, bits: u32) -> i32 {
    let shift = 32 - bits;
    ((value << shift) as i32) >> shift
}

fn run(f: &mut Fighter, thread: usize, forward: bool) {
    let Some(table) = fighter_scripts(f.kind) else {
        return;
    };
    let words = table.words;
    let speed = f.status.timing.anim_speed;
    let anim_frame = f.status.anim_frame;
    {
        let t = &mut f.motion_script.threads[thread];
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
        let t = f.motion_script.threads[thread];
        let (words, base) = if t.pc != NO_SCRIPT && t.pc & COMMON_BIT != 0 {
            (&scripts::COMMON_MOVESET_WORDS[..], COMMON_BIT)
        } else {
            (words, 0)
        };
        if t.pc == NO_SCRIPT || (t.pc - base) as usize >= words.len() {
            f.motion_script.threads[thread].pc = NO_SCRIPT;
            return;
        }
        if t.wait == f32::MAX {
            if speed <= anim_frame {
                return;
            }
            f.motion_script.threads[thread].wait = -anim_frame;
        } else if t.wait > 0.0 {
            return;
        }
        let pc = (t.pc - base) as usize;
        let w = words[pc];
        let opcode = w >> 26;
        let skip = forward
            && matches!(
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
                    | op::SET_FLAG0
                    ..=23 | op::SET_AIR_JUMP_ADD | op::SET_AIR_JUMP_MAX
            );
        if skip {
            f.motion_script.threads[thread].pc += command_words(opcode);
            continue;
        }
        execute(f, thread, words, base, pc, w, opcode, anim_frame, speed);
    }
}

#[allow(clippy::too_many_arguments)]
fn execute(
    f: &mut Fighter,
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
            f.motion_script.threads[thread].pc = NO_SCRIPT;
            return;
        }
        op::SYNC_WAIT => f.motion_script.threads[thread].wait += value as f32,
        op::ASYNC_WAIT => f.motion_script.threads[thread].wait = value as f32 - anim_frame,
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
            // `ftParamSetHitStatusAll`.
            f.hitstatus = HitStatus::from_raw(value);
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
            let t = &mut f.motion_script.threads[thread];
            if t.script_id + 2 <= STACK_MAX {
                t.p_goto[t.script_id] = next;
                t.loop_count[t.script_id] = value as i32;
                t.script_id += 2;
            }
        }
        op::LOOP_END => {
            let t = &mut f.motion_script.threads[thread];
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
        op::SUBROUTINE => {
            let target = word(1);
            let t = &mut f.motion_script.threads[thread];
            if target == NO_SCRIPT || t.script_id >= STACK_MAX {
                // An item moveset outside this file: nothing of it runs.
                t.pc = next;
                return;
            }
            t.p_goto[t.script_id] = next;
            t.script_id += 1;
            t.pc = target;
            return;
        }
        op::RETURN => {
            let t = &mut f.motion_script.threads[thread];
            if t.script_id == 0 {
                t.pc = NO_SCRIPT;
                return;
            }
            t.script_id -= 1;
            t.pc = t.p_goto[t.script_id];
            return;
        }
        op::GOTO => {
            f.motion_script.threads[thread].pc = word(1);
            return;
        }
        op::PAUSE_SCRIPT => f.motion_script.threads[thread].wait = f32::MAX,
        op::SET_PARALLEL_SCRIPT => {
            let target = word(1);
            if thread == 0 && f.motion_script.threads[1].pc == NO_SCRIPT && target != NO_SCRIPT {
                f.motion_script.threads[1] = ScriptThread {
                    pc: target,
                    wait: speed - anim_frame,
                    ..ScriptThread::default()
                };
            }
        }
        _ => {}
    }
    f.motion_script.threads[thread].pc = next;
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
