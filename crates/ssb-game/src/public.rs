//! The crowd and the "<player> defeated" announcer (`src/ft/ftpublic.c`).
//!
//! As in the source, the crowd is a set of file statics that
//! [`make_actor`] (`ftPublicMakeActor`) resets when a battle scene starts,
//! and one process ([`proc_update`]) on link 13, after the fighters', the
//! items', the weapons', the effects', the camera's and the interface's.
//!
//! Its only reads of the audio system are whether its own chant and
//! announcer voices still play ([`crate::sound::fgm_alive`]); they pace
//! the next chant and the next announcement and nothing else.
//!
//! The source decides a reaction where the hit, the landing or the special
//! fall happens, reading the attacker's percent, kind and
//! `public_knockback` there. The port's fighter code holds one fighter, so
//! those sites queue the reaction ([`common_check`], [`play_cliff_react`],
//! [`try_play_fall_special_react`]) and [`proc_update`] decides them, in
//! order, before its own step and in the same frame. The attacker is read
//! at the end of the frame rather than at the hit: a PSP deviation that
//! can only change which crowd sound plays.

#[cfg(not(test))]
use core::ptr::addr_of_mut;

use crate::fighter::{Fighter, FighterKind};
use crate::sound::{self, id::*, FgmHandle};

/// `dFTCommonDataPublicFighterCallFGMs`: each fighter's chant, by `FTKind`
/// (none for Master Hand, the Polygons and Giant Donkey Kong).
const FIGHTER_CALL_FGMS: [u16; 27] = [
    nSYAudioVoicePublicMario,
    nSYAudioVoicePublicFox,
    nSYAudioVoicePublicDonkey,
    nSYAudioVoicePublicSamus,
    nSYAudioVoicePublicLuigi,
    nSYAudioVoicePublicLink,
    nSYAudioVoicePublicYoshi,
    nSYAudioVoicePublicCaptain,
    nSYAudioVoicePublicKirby,
    nSYAudioVoicePublicPikachu,
    nSYAudioVoicePublicPurin,
    nSYAudioVoicePublicNess,
    nSYAudioFGMVoiceEnd,
    nSYAudioVoicePublicMario,
    nSYAudioFGMVoiceEnd,
    nSYAudioFGMVoiceEnd,
    nSYAudioFGMVoiceEnd,
    nSYAudioFGMVoiceEnd,
    nSYAudioFGMVoiceEnd,
    nSYAudioFGMVoiceEnd,
    nSYAudioFGMVoiceEnd,
    nSYAudioFGMVoiceEnd,
    nSYAudioFGMVoiceEnd,
    nSYAudioFGMVoiceEnd,
    nSYAudioFGMVoiceEnd,
    nSYAudioFGMVoiceEnd,
    nSYAudioFGMVoiceEnd,
];

/// `FTCOMMON_DAMAGE_KNOCKBACK_VERYHIGH`.
const KNOCKBACK_VERYHIGH: f32 = 160.0;
/// `F_CLC_DTOR32(75.0F)` and `F_CLC_DTOR32(115.0F)`: a hit launched this
/// close to straight up excites the crowd less.
const GASP_ANGLE_LOW: f32 = 1.308_997;
const GASP_ANGLE_HIGH: f32 = 2.007_128_7;
/// `FTCOMMON_DAMAGE_PUBLIC_REACT_GASP_KNOCKBACK_MUL`.
const GASP_KNOCKBACK_MUL: f32 = 0.8;
/// `ARRAY_COUNT(sFTPublicDefeatedVoiceIDs)`.
const DEFEATED_MAX: usize = 10;
/// The reactions one frame can queue; more are dropped (and counted).
const EVENTS_MAX: usize = 16;

/// A reaction a fighter's code asked for this frame.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Event {
    /// `ftCommonDamageSetPublic`: `player` was hit by `attacker` with the
    /// crowd knockback `knockback`.
    Damage { attacker: i32, knockback: f32 },
    /// `ftPublicPlayCliffReact`.
    Cliff { player: i32, knockback: f32 },
    /// `ftPublicTryPlayFallSpecialReact`.
    FallSpecial { player: i32, pos_y: f32 },
}

struct State {
    common_tics_past: i32,
    common_player_num: i32,
    common_knockback: f32,
    common_sound: Option<FgmHandle>,
    call_is_interrupt: bool,
    call_wait: i32,
    call_player_num: i32,
    call_sound: Option<FgmHandle>,
    call_count: i32,
    call_id: u16,
    players_down: i32,
    defeated_voice_ids: [u16; DEFEATED_MAX],
    defeated_current: usize,
    defeated_end: usize,
    defeated_sound: Option<FgmHandle>,
    events: [Option<Event>; EVENTS_MAX],
    events_num: usize,
    events_dropped: u32,
}

const fn initial() -> State {
    State {
        common_tics_past: u16::MAX as i32 + 1,
        common_player_num: -1,
        common_knockback: 0.0,
        common_sound: None,
        call_is_interrupt: false,
        call_wait: 1200,
        call_player_num: -1,
        call_sound: None,
        call_count: 9,
        call_id: 0,
        players_down: 0,
        defeated_voice_ids: [0; DEFEATED_MAX],
        defeated_current: 0,
        defeated_end: 0,
        defeated_sound: None,
        events: [None; EVENTS_MAX],
        events_num: 0,
        events_dropped: 0,
    }
}

#[cfg(not(test))]
static mut STATE: State = initial();

#[cfg(test)]
std::thread_local! {
    static STATE: core::cell::UnsafeCell<State> = const { core::cell::UnsafeCell::new(initial()) };
}

/// The crowd's statics. The game thread is the only user (per thread in
/// tests).
#[allow(clippy::mut_from_ref)]
fn with<R>(f: impl FnOnce(&mut State) -> R) -> R {
    #[cfg(not(test))]
    // SAFETY: only the game thread touches the crowd, and no call nests.
    return f(unsafe { &mut *addr_of_mut!(STATE) });
    #[cfg(test)]
    // SAFETY: per-thread state, and no call nests.
    return STATE.with(|s| f(unsafe { &mut *s.get() }));
}

/// `ftPublicMakeActor`: a battle scene starts with a quiet crowd.
pub fn make_actor() {
    with(|s| *s = initial());
}

/// Reactions dropped because a frame queued more than the queue holds.
pub fn events_dropped() -> u32 {
    with(|s| s.events_dropped)
}

fn push(event: Event) {
    with(|s| {
        if s.events_num < EVENTS_MAX {
            s.events[s.events_num] = Some(event);
            s.events_num += 1;
        } else {
            s.events_dropped += 1;
        }
    });
}

/// `damage_player_num`: the attacker's player, which the source sets to 0
/// for a hit by the stage (`ftMainSearchHitGround` and friends).
fn player_num(port: Option<u8>) -> i32 {
    port.map_or(0, i32::from)
}

/// `ftCommonDamageSetPublic` from `ftCommonDamageInitDamageVars`: the hit
/// fighter's crowd knockback (less for a hit launched near straight up),
/// which also waits in the damage status until hitstun ends
/// (`ftCommonDamageDecHitStunSetPublic`).
pub fn damage_set_public(f: &mut Fighter, knockback: f32, angle: f32) {
    let mut public = knockback;
    if angle > GASP_ANGLE_LOW && angle < GASP_ANGLE_HIGH {
        public *= GASP_KNOCKBACK_MUL;
    }
    f.reaction.public_knockback = public;
    f.public_knockback = 0.0;
    common_check(f.damage_player, public);
}

/// `ftPublicCommonCheck`, queued: a hit of 100 or more by `attacker`.
pub fn common_check(attacker: Option<u8>, knockback: f32) {
    if knockback >= 100.0 {
        push(Event::Damage {
            attacker: player_num(attacker),
            knockback,
        });
    }
}

/// `ftPublicPlayCliffReact`, queued: a fighter launched hard lands or
/// grabs a ledge.
pub fn play_cliff_react(port: u8, knockback: f32) {
    push(Event::Cliff {
        player: i32::from(port),
        knockback,
    });
}

/// `ftPublicTryPlayFallSpecialReact`, queued: a helpless fall below the
/// stage.
pub fn try_play_fall_special_react(port: u8, pos_y: f32) {
    push(Event::FallSpecial {
        player: i32::from(port),
        pos_y,
    });
}

/// `ftPublicDefeatedAddID`.
pub fn defeated_add_id(sfx_id: u16) {
    with(|s| {
        s.defeated_voice_ids[s.defeated_end] = sfx_id;
        s.defeated_end += 1;
        if s.defeated_end == DEFEATED_MAX {
            s.defeated_end = 0;
        }
    });
}

/// `alSound->sfx_id != order`: the handle's sound has ended (or never
/// started).
fn ended(h: Option<FgmHandle>) -> bool {
    h.is_none_or(|h| !sound::fgm_alive(h))
}

fn stop(h: Option<FgmHandle>) {
    if let Some(h) = h {
        sound::stop_fgm(h);
    }
}

/// `ftPublicPlayCommon`.
fn play_common(s: &mut State, sfx: u16) {
    stop(s.common_sound);
    s.common_sound = sound::play_fgm(sfx);
}

/// `ftPublicTryInterruptCall`.
fn try_interrupt_call(s: &mut State) {
    if s.call_count < 9 && s.call_count >= 3 {
        s.call_is_interrupt = true;
    }
}

/// `ftPublicTryStartCall`: a fighter at 100% or more who scores a big hit
/// after 1200 quiet tics starts their name chant.
fn try_start_call(s: &mut State, fighters: &[&Fighter], knockback: f32, player_num: i32) -> bool {
    let Some(attacker) = fighters.iter().find(|f| i32::from(f.port) == player_num) else {
        return false;
    };
    if attacker.damage < 100 || s.call_wait < 1200 {
        return false;
    }
    if player_num == s.call_player_num {
        return false;
    }
    s.call_id = FIGHTER_CALL_FGMS
        .get(attacker.kind as usize)
        .copied()
        .unwrap_or(nSYAudioFGMVoiceEnd);
    if s.call_id == nSYAudioFGMVoiceEnd {
        return false;
    }
    stop(s.call_sound);
    s.call_sound = sound::play_fgm(if knockback >= KNOCKBACK_VERYHIGH {
        nSYAudioVoicePublicCheer
    } else {
        nSYAudioVoicePublicAmazed
    });
    if s.call_sound.is_some() {
        s.call_player_num = player_num;
        s.call_count = 0;
        return true;
    }
    false
}

/// `ftPublicDecideCall`.
fn decide_call(s: &mut State, fighters: &[&Fighter], player_num: i32, knockback: f32) {
    if knockback >= 130.0 {
        if try_start_call(s, fighters, knockback, player_num) {
            // `ftPublicCommonStop`.
            stop(s.common_sound);
            return;
        } else if knockback >= KNOCKBACK_VERYHIGH {
            try_interrupt_call(s);
            play_common(s, nSYAudioVoicePublicCheer);
            return;
        } else if player_num == s.call_player_num {
            try_interrupt_call(s);
        }
        play_common(s, nSYAudioVoicePublicAmazed);
    } else if knockback >= 100.0 {
        play_common(s, nSYAudioVoicePublicGaspClap);
    }
}

/// `ftPublicDecideCommon`. `is_force_curr_knockback` is set when the
/// attacker had itself just been launched hard (`public_knockback >= 160`).
fn decide_common(
    s: &mut State,
    fighters: &[&Fighter],
    player_num: i32,
    knockback: f32,
    is_force: bool,
) {
    if is_force {
        decide_call(s, fighters, player_num, knockback);
    } else if player_num == s.common_player_num && s.common_tics_past < 60 {
        let kb = if knockback > s.common_knockback {
            knockback
        } else {
            s.common_knockback
        };
        decide_call(s, fighters, player_num, kb);
    } else if knockback >= KNOCKBACK_VERYHIGH {
        try_interrupt_call(s);
        play_common(s, nSYAudioVoicePublicDamageL);
    } else if knockback >= 130.0 {
        if player_num == s.call_player_num {
            try_interrupt_call(s);
        }
        play_common(s, nSYAudioVoicePublicDamageM);
    } else if knockback >= 100.0 {
        play_common(s, nSYAudioVoicePublicDamageS);
    }
    s.common_tics_past = 0;
    s.common_player_num = player_num;
    s.common_knockback = knockback;
}

fn run_event(s: &mut State, fighters: &[&Fighter], bounds_bottom: f32, event: Event) {
    match event {
        Event::Damage {
            attacker,
            knockback,
        } => {
            let is_force = fighters
                .iter()
                .find(|f| i32::from(f.port) == attacker)
                .is_some_and(|f| f.public_knockback >= KNOCKBACK_VERYHIGH);
            decide_common(s, fighters, attacker, knockback, is_force);
        }
        Event::Cliff { player, knockback } => {
            if s.call_player_num == player {
                try_interrupt_call(s);
            }
            if knockback >= 160.0 {
                play_common(s, nSYAudioVoicePublicGaspL);
            } else if knockback >= 130.0 {
                play_common(s, nSYAudioVoicePublicGaspM);
            } else if knockback >= 100.0 {
                play_common(s, nSYAudioVoicePublicGaspS);
            }
        }
        Event::FallSpecial { player, pos_y } => {
            if pos_y >= bounds_bottom || pos_y < -2400.0 {
                return;
            } else if pos_y >= -300.0 {
                play_common(s, nSYAudioVoicePublicGaspL);
            } else if pos_y >= -900.0 {
                play_common(s, nSYAudioVoicePublicGaspM);
            } else {
                play_common(s, nSYAudioVoicePublicGaspS);
            }
            if s.call_player_num == player {
                try_interrupt_call(s);
            }
        }
    }
}

/// `ftPublicProcUpdate`, once a frame after the interface (link 13), while
/// the world runs. `fighters` are every fighter in the battle,
/// `bounds_bottom` is `gMPCollisionBounds.current.bottom` and `stock_rule`
/// is `game_rules & SCBATTLE_GAMERULE_STOCK`.
pub fn proc_update(fighters: &[&Fighter], bounds_bottom: f32, stock_rule: bool) {
    with(|s| {
        let n = core::mem::take(&mut s.events_num);
        for i in 0..n {
            if let Some(e) = s.events[i].take() {
                run_event(s, fighters, bounds_bottom, e);
            }
        }
        if s.common_tics_past < u16::MAX as i32 + 1 {
            s.common_tics_past += 1;
        }
        let players_down_bak = s.players_down;
        s.players_down = 0;
        let mut down: Option<&Fighter> = None;
        for f in fighters.iter().copied() {
            if !stock_rule || f.stocks != -1 {
                if f.pos.y < bounds_bottom - 100.0 {
                    s.players_down += 1;
                } else {
                    down = Some(f);
                }
            }
        }
        if players_down_bak < 3 && s.players_down >= 3 {
            play_common(s, nSYAudioVoicePublicGaspL);
            if down.is_some_and(|f| s.call_player_num == i32::from(f.port)) {
                try_interrupt_call(s);
            }
        }
        if s.call_count < 9 {
            if ended(s.call_sound) {
                s.call_count += 1;
                if s.call_count < 9 {
                    if s.call_is_interrupt {
                        s.call_is_interrupt = false;
                        s.call_wait = 0;
                        s.call_count = 9;
                    } else {
                        s.call_sound = sound::play_fgm(s.call_id);
                    }
                } else {
                    s.call_wait = 0;
                    play_common(s, nSYAudioVoicePublicCheer);
                }
            }
        } else if s.call_wait < 1200 {
            s.call_wait += 1;
        }
        if s.defeated_current != s.defeated_end && ended(s.defeated_sound) {
            s.defeated_sound = sound::play_fgm(s.defeated_voice_ids[s.defeated_current]);
            s.defeated_current += 1;
            if s.defeated_current == DEFEATED_MAX {
                s.defeated_current = 0;
            }
        }
    });
}

/// The fighter kinds that have a chant (tests).
#[cfg(test)]
pub(crate) fn has_call(kind: FighterKind) -> bool {
    FIGHTER_CALL_FGMS[kind as usize] != nSYAudioFGMVoiceEnd
}
