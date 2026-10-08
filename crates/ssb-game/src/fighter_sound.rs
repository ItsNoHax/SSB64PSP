//! A fighter's sound slots and the `ftParam` sound helpers
//! (`src/ft/ftparam.c`), plus `ftMainPlayHitSFX` (`src/ft/ftmain.c`).
//!
//! `FTStruct` keeps three playing sounds, each as the pooled instance and
//! the serial it had when it started (`p_sfx`/`sfx_id`, `p_voice`/`voice_id`,
//! `p_loop_sfx`/`loop_sfx_id`); an [`FgmHandle`] is that pair. None of them
//! feeds back into gameplay.

use crate::fighter::Fighter;
use crate::sound::{self, id::*, FgmHandle};

/// `FTStruct::p_sfx`, `p_voice` and `p_loop_sfx` with their serials,
/// cleared by `ftManagerMakeFighter`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FighterSound {
    /// `p_sfx`/`sfx_id`: `PlayFGMStoreInfo`'s sound, which the next hit
    /// sound stops.
    pub sfx: Option<FgmHandle>,
    /// `p_voice`/`voice_id`.
    pub voice: Option<FgmHandle>,
    /// `p_loop_sfx`/`loop_sfx_id`: stopped by every status change without
    /// `FTSTATUS_PRESERVE_LOOPSFX`.
    pub loop_sfx: Option<FgmHandle>,
}

/// `ftParamPlayVoice`.
pub fn play_voice(f: &mut Fighter, voice_id: u16) {
    f.sound.voice = sound::play_fgm(voice_id);
}

/// `ftParamStopVoice`. The serial check (`p_voice->sfx_id == voice_id`) is
/// [`sound::stop_fgm`]'s own.
pub fn stop_voice(f: &mut Fighter) {
    if let Some(h) = f.sound.voice.take() {
        sound::stop_fgm(h);
    }
}

/// `ftParamPlayLoopSFX`: plays only into an empty slot. A play that found
/// the pool empty leaves the slot empty, so the next command retries.
pub fn play_loop_sfx(f: &mut Fighter, sfx_id: u16) {
    if f.sound.loop_sfx.is_none() {
        f.sound.loop_sfx = sound::play_fgm(sfx_id);
    }
}

/// `ftParamStopLoopSFX`.
pub fn stop_loop_sfx(f: &mut Fighter) {
    if let Some(h) = f.sound.loop_sfx.take() {
        sound::stop_fgm(h);
    }
}

/// `nFTMotionEventPlayFGMStoreInfo`: `fp->p_sfx = func_800269C0_275C0(id)`.
pub fn play_fgm_store_info(f: &mut Fighter, sfx_id: u16) {
    f.sound.sfx = sound::play_fgm(sfx_id);
}

/// `dFTMainHitCollisionFGMs`: the hit sound by `fgm_kind` (rows: punch,
/// kick, coin, burn, shock, slash, fan, bat) and `fgm_level` (columns).
pub const HIT_COLLISION_FGMS: [[u16; 3]; 8] = [
    [nSYAudioFGMPunchS, nSYAudioFGMPunchM, nSYAudioFGMPunchL],
    [nSYAudioFGMKickS, nSYAudioFGMKickM, nSYAudioFGMKickL],
    [
        nSYAudioFGMMarioSpecialHiCoin,
        nSYAudioFGMMarioSpecialHiCoin,
        nSYAudioFGMMarioSpecialHiCoin,
    ],
    [nSYAudioFGMBurnS, nSYAudioFGMBurnM, nSYAudioFGMBurnL],
    [nSYAudioFGMShockS, nSYAudioFGMShockM, nSYAudioFGMShockL],
    [nSYAudioFGMSlashS, nSYAudioFGMSlashM, nSYAudioFGMSlashL],
    [
        nSYAudioFGMHarisenHit,
        nSYAudioFGMHarisenHit,
        nSYAudioFGMHarisenHit,
    ],
    [nSYAudioFGMPunchM, nSYAudioFGMPunchL, nSYAudioFGMBatHit],
];

/// `ftMainPlayHitSFX`: the attacker `f` stops its `PlayFGMStoreInfo` sound
/// and plays the hit sound at its TopN x.
pub fn play_hit_sfx(f: &mut Fighter, fgm_kind: u8, fgm_level: u8) {
    if let Some(h) = f.sound.sfx.take() {
        sound::stop_fgm(h);
    }
    let row = HIT_COLLISION_FGMS[fgm_kind as usize & 7];
    sound::play_fgm_at(row[(fgm_level as usize).min(2)], f.pos.x);
}

/// `dFTCommonDataDownBounceSFX`, by `FTKind`.
pub const DOWN_BOUNCE_SFX: [u16; 27] = [
    nSYAudioFGMMarioDownBounce,
    nSYAudioFGMFoxDownBounce,
    nSYAudioFGMDonkeyDownBounce,
    nSYAudioFGMSamusDownBounce,
    nSYAudioFGMMarioDownBounce,
    nSYAudioFGMLinkDownBounce,
    nSYAudioFGMYoshiDownBounce,
    nSYAudioFGMCaptainDownBounce,
    nSYAudioFGMKirbyDownBounce,
    nSYAudioFGMPikachuDownBounce,
    nSYAudioFGMPurinDownBounce,
    nSYAudioFGMNessDownBounce,
    nSYAudioFGMMarioDownBounce,
    nSYAudioFGMMarioDownBounce,
    nSYAudioFGMMarioDownBounce,
    nSYAudioFGMFoxDownBounce,
    nSYAudioFGMDonkeyDownBounce,
    nSYAudioFGMSamusDownBounce,
    nSYAudioFGMMarioDownBounce,
    nSYAudioFGMLinkDownBounce,
    nSYAudioFGMYoshiDownBounce,
    nSYAudioFGMCaptainDownBounce,
    nSYAudioFGMKirbyDownBounce,
    nSYAudioFGMPikachuDownBounce,
    nSYAudioFGMPurinDownBounce,
    nSYAudioFGMNessDownBounce,
    nSYAudioFGMDonkeyDownBounce,
];
