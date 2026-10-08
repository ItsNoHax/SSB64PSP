//! The game-facing audio entry points, as the decomp names them.

/// A playing FGM script: `func_800269C0_275C0`'s returned `alSoundEffect*`,
/// as the pool slot plus the serial the game keeps beside it (`sfx_id`,
/// the node's `0x26`). The sound is still playing while the slot's serial
/// equals `serial`; a stopped or recycled node has a different one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FgmHandle {
    pub slot: u8,
    pub serial: u16,
}

/// What game code calls. Implementations synchronise internally (the N64
/// masks interrupts around the same list operations), so every method takes
/// `&self` and may be called from the game thread at any time.
pub trait AudioApi: Sync {
    // --- FGM (sound effects and voices), `n_env.c` -----------------------

    /// `func_800269C0_275C0`: starts FGM script `id`. `None` when `id` is at
    /// or past the FGM count or the script pool is empty.
    fn play_fgm(&self, id: u16) -> Option<FgmHandle>;

    /// `lbCommonMakePositionFGM`'s path: `func_80026A10_27610`, set the
    /// node's balance (`0x2F`), then `func_800267F4_273F4`.
    fn play_fgm_balance(&self, id: u16, balance: u8) -> Option<FgmHandle>;

    /// `func_80026738_27338`: stops a script and its forks. A no-op on a
    /// handle whose serial no longer matches.
    fn stop_fgm(&self, handle: FgmHandle);

    /// `func_800266A0_272A0`: stops every script.
    fn stop_all_fgm(&self);

    /// `func_80026594_27194`: holds every pausable script and voice.
    fn pause_fgm(&self);

    /// `func_800264A4_270A4`: releases what `pause_fgm` held.
    fn resume_fgm(&self);

    /// `handle->sfx_id == serial`: whether that exact sound still plays.
    fn fgm_alive(&self, handle: FgmHandle) -> bool;

    /// `D_8009EDD0.sfx_max` (the FGM count, `0x28`): the boss defeat saves it
    /// and sets it to 0 so that every later play returns `None`.
    fn fgm_count(&self) -> u16;
    fn set_fgm_count(&self, count: u16);

    /// `func_80026070_26C70`: the master FGM volume (0..=127).
    fn set_fgm_master_volume(&self, volume: u8);
    /// `func_80026174_26D74`: a script's (and its forks') volume.
    fn set_fgm_volume(&self, handle: FgmHandle, volume: u8);
    /// `func_80026104_26D04`: a script's pan.
    fn set_fgm_pan(&self, handle: FgmHandle, pan: u8);
    /// `func_80026094_26C94`: a script's effect send.
    fn set_fgm_fx(&self, handle: FgmHandle, fx: u8);

    // --- syAudio (`src/sys/audio.c`) --------------------------------------

    /// `syAudioPlayBGM`: returns `bgm`, or -1 when it is not a sequence.
    fn play_bgm(&self, player: u32, bgm: u32) -> i32;
    /// `syAudioStopBGM`.
    fn stop_bgm(&self, player: u32);
    /// `syAudioStopBGMAll`.
    fn stop_bgm_all(&self);
    /// `syAudioSetBGMVolume` (0..=0x7800).
    fn set_bgm_volume(&self, player: u32, volume: u32);
    /// `syAudioSetBGMVolumeFade`: ramps to `volume` over `time` audio frames.
    fn set_bgm_volume_fade(&self, player: u32, volume: u32, time: u32);
    /// `syAudioSetBGMReverb`.
    fn set_bgm_reverb(&self, player: u32, reverb: u32);
    /// `syAudioSetBGMPriority`.
    fn set_bgm_priority(&self, player: u32, priority: u8);
    /// `syAudioCheckBGMPlaying`.
    fn bgm_playing(&self, player: u32) -> bool;

    /// `syAudioPlayFGM`: plays through one of the 24 `sSYAudioSoundPlayers`
    /// slots; returns the slot or -1.
    fn sy_play_fgm(&self, fgm: u32) -> i32;
    /// `syAudioStopFGM`.
    fn sy_stop_fgm(&self, slot: i32);

    /// `syAudioSetQuality`: 0 mono, 1 stereo (`dSYAudioSoundQuality`).
    fn set_quality(&self, quality: u32);
    /// `syAudioSetFXType`.
    fn set_fx_type(&self, fx_type: i32);
}
