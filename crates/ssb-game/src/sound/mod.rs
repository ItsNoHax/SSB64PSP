//! The game's audio calls: `syAudio*` and the FGM entry points
//! (`func_800269C0_275C0` and its siblings) as gameplay, menus and scenes
//! make them.
//!
//! As in the original, these are global calls into the audio system, made
//! where the decomp makes them. The platform installs its
//! [`AudioApi`] once at boot ([`install`]); with none installed (host tests,
//! tools) every play returns `None` and the rest do nothing. Sound calls
//! never draw from [`crate::rng`] and never move a draw.
//!
//! Tests install a [`testing::Recorder`], which is per thread.

pub mod id;

pub use ssb_engine::audio::{AudioApi, FgmHandle};

#[cfg(not(test))]
static mut SINK: Option<&'static dyn AudioApi> = None;

#[cfg(test)]
std::thread_local! {
    static SINK: core::cell::Cell<Option<&'static dyn AudioApi>> = const { core::cell::Cell::new(None) };
}

/// Installs the platform's audio system. Called once, before the first
/// scene.
pub fn install(api: &'static dyn AudioApi) {
    #[cfg(not(test))]
    // SAFETY: set once at boot on the game thread, before any reader.
    unsafe {
        SINK = Some(api)
    };
    #[cfg(test)]
    SINK.with(|s| s.set(Some(api)));
}

/// Removes the installed audio system (tests).
pub fn uninstall() {
    #[cfg(not(test))]
    // SAFETY: as in `install`.
    unsafe {
        SINK = None
    };
    #[cfg(test)]
    SINK.with(|s| s.set(None));
}

#[inline]
fn sink() -> Option<&'static dyn AudioApi> {
    #[cfg(not(test))]
    // SAFETY: written only by `install`/`uninstall` on the game thread.
    return unsafe { *core::ptr::addr_of!(SINK) };
    #[cfg(test)]
    return SINK.with(|s| s.get());
}

/// `func_800269C0_275C0`.
pub fn play_fgm(id: u16) -> Option<FgmHandle> {
    sink().and_then(|s| s.play_fgm(id))
}

/// `lbCommonMakePositionFGM`: plays `id` panned by the source's x position.
/// `bal = clamp((x / 8000) * 60, ±60)`, `balance = 64 - bal` (a source at
/// +x pans left, as on the N64).
pub fn play_fgm_at(id: u16, x: f32) -> Option<FgmHandle> {
    let mut bal = (x / 8000.0) * 60.0;
    bal = bal.clamp(-60.0, 60.0);
    let balance = (64 - bal as i32) as u8;
    sink().and_then(|s| s.play_fgm_balance(id, balance))
}

/// `func_80026738_27338`.
pub fn stop_fgm(handle: FgmHandle) {
    if let Some(s) = sink() {
        s.stop_fgm(handle);
    }
}

/// `func_800266A0_272A0`.
pub fn stop_all_fgm() {
    if let Some(s) = sink() {
        s.stop_all_fgm();
    }
}

/// `func_80026594_27194`.
pub fn pause_fgm() {
    if let Some(s) = sink() {
        s.pause_fgm();
    }
}

/// `func_800264A4_270A4`.
pub fn resume_fgm() {
    if let Some(s) = sink() {
        s.resume_fgm();
    }
}

/// `p_sfx->sfx_id == saved`: whether the sound still plays.
pub fn fgm_alive(handle: FgmHandle) -> bool {
    sink().is_some_and(|s| s.fgm_alive(handle))
}

/// `D_8009EDD0.sfx_max`.
pub fn fgm_count() -> u16 {
    sink().map_or(0, |s| s.fgm_count())
}

/// Sets `D_8009EDD0.sfx_max`.
pub fn set_fgm_count(count: u16) {
    if let Some(s) = sink() {
        s.set_fgm_count(count);
    }
}

/// `func_80026070_26C70`.
pub fn set_fgm_master_volume(volume: u8) {
    if let Some(s) = sink() {
        s.set_fgm_master_volume(volume);
    }
}

/// `func_80026174_26D74`.
pub fn set_fgm_volume(handle: FgmHandle, volume: u8) {
    if let Some(s) = sink() {
        s.set_fgm_volume(handle, volume);
    }
}

/// `func_80026104_26D04`.
pub fn set_fgm_pan(handle: FgmHandle, pan: u8) {
    if let Some(s) = sink() {
        s.set_fgm_pan(handle, pan);
    }
}

/// `func_80026094_26C94`.
pub fn set_fgm_fx(handle: FgmHandle, fx: u8) {
    if let Some(s) = sink() {
        s.set_fgm_fx(handle, fx);
    }
}

/// `syAudioPlayBGM`.
pub fn play_bgm(player: u32, bgm: u32) -> i32 {
    sink().map_or(-1, |s| s.play_bgm(player, bgm))
}

/// `syAudioStopBGM`.
pub fn stop_bgm(player: u32) {
    if let Some(s) = sink() {
        s.stop_bgm(player);
    }
}

/// `syAudioStopBGMAll`.
pub fn stop_bgm_all() {
    if let Some(s) = sink() {
        s.stop_bgm_all();
    }
}

/// `syAudioSetBGMVolume`.
pub fn set_bgm_volume(player: u32, volume: u32) {
    if let Some(s) = sink() {
        s.set_bgm_volume(player, volume);
    }
}

/// `syAudioSetBGMVolumeFade`.
pub fn set_bgm_volume_fade(player: u32, volume: u32, time: u32) {
    if let Some(s) = sink() {
        s.set_bgm_volume_fade(player, volume, time);
    }
}

/// `syAudioSetBGMReverb`.
pub fn set_bgm_reverb(player: u32, reverb: u32) {
    if let Some(s) = sink() {
        s.set_bgm_reverb(player, reverb);
    }
}

/// `syAudioSetBGMPriority`.
pub fn set_bgm_priority(player: u32, priority: u8) {
    if let Some(s) = sink() {
        s.set_bgm_priority(player, priority);
    }
}

/// `syAudioCheckBGMPlaying`.
pub fn bgm_playing(player: u32) -> bool {
    sink().is_some_and(|s| s.bgm_playing(player))
}

/// `syAudioPlayFGM`.
pub fn sy_play_fgm(fgm: u32) -> i32 {
    sink().map_or(-1, |s| s.sy_play_fgm(fgm))
}

/// `syAudioStopFGM`.
pub fn sy_stop_fgm(slot: i32) {
    if let Some(s) = sink() {
        s.sy_stop_fgm(slot);
    }
}

/// `syAudioSetQuality`: 0 mono, 1 stereo.
pub fn set_quality(quality: u32) {
    if let Some(s) = sink() {
        s.set_quality(quality);
    }
}

/// `syAudioSetFXType`.
pub fn set_fx_type(fx_type: i32) {
    if let Some(s) = sink() {
        s.set_fx_type(fx_type);
    }
}

/// A recording [`AudioApi`] for tests: every call is appended to a log, and
/// plays hand out handles from a counter (never `None` unless the count is
/// set to 0, as the boss defeat does).
#[cfg(test)]
pub mod testing {
    use super::{AudioApi, FgmHandle};
    use std::sync::Mutex;
    use std::vec::Vec;

    /// One recorded call.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Call {
        PlayFgm(u16),
        PlayFgmBalance(u16, u8),
        StopFgm(FgmHandle),
        StopAllFgm,
        PauseFgm,
        ResumeFgm,
        SetFgmCount(u16),
        SetFgmMasterVolume(u8),
        SetFgmVolume(FgmHandle, u8),
        SetFgmPan(FgmHandle, u8),
        SetFgmFx(FgmHandle, u8),
        PlayBgm(u32, u32),
        StopBgm(u32),
        StopBgmAll,
        SetBgmVolume(u32, u32),
        SetBgmVolumeFade(u32, u32, u32),
        SetBgmReverb(u32, u32),
        SetBgmPriority(u32, u8),
        SyPlayFgm(u32),
        SyStopFgm(i32),
        SetQuality(u32),
        SetFxType(i32),
    }

    #[derive(Default)]
    struct State {
        calls: Vec<Call>,
        serial: u16,
        count: Option<u16>,
        bgm_playing: bool,
    }

    /// See the module docs.
    #[derive(Default)]
    pub struct Recorder {
        state: Mutex<State>,
    }

    impl Recorder {
        /// Leaks a fresh recorder and installs it for this thread.
        pub fn install() -> &'static Recorder {
            let r: &'static Recorder = Box::leak(Box::default());
            super::install(r);
            r
        }

        /// Takes the calls recorded so far.
        pub fn take(&self) -> Vec<Call> {
            core::mem::take(&mut self.state.lock().unwrap().calls)
        }

        fn push(&self, c: Call) {
            self.state.lock().unwrap().calls.push(c);
        }

        fn handle(&self) -> Option<FgmHandle> {
            let mut s = self.state.lock().unwrap();
            if s.count == Some(0) {
                return None;
            }
            s.serial = s.serial.wrapping_add(1).max(1);
            Some(FgmHandle {
                slot: 0,
                serial: s.serial,
            })
        }
    }

    impl AudioApi for Recorder {
        fn play_fgm(&self, id: u16) -> Option<FgmHandle> {
            self.push(Call::PlayFgm(id));
            self.handle()
        }
        fn play_fgm_balance(&self, id: u16, balance: u8) -> Option<FgmHandle> {
            self.push(Call::PlayFgmBalance(id, balance));
            self.handle()
        }
        fn stop_fgm(&self, h: FgmHandle) {
            self.push(Call::StopFgm(h));
        }
        fn stop_all_fgm(&self) {
            self.push(Call::StopAllFgm);
        }
        fn pause_fgm(&self) {
            self.push(Call::PauseFgm);
        }
        fn resume_fgm(&self) {
            self.push(Call::ResumeFgm);
        }
        fn fgm_alive(&self, h: FgmHandle) -> bool {
            self.state.lock().unwrap().serial == h.serial
        }
        fn fgm_count(&self) -> u16 {
            self.state
                .lock()
                .unwrap()
                .count
                .unwrap_or(super::id::nSYAudioFGMVoiceEnd)
        }
        fn set_fgm_count(&self, count: u16) {
            self.state.lock().unwrap().count = Some(count);
            self.push(Call::SetFgmCount(count));
        }
        fn set_fgm_master_volume(&self, v: u8) {
            self.push(Call::SetFgmMasterVolume(v));
        }
        fn set_fgm_volume(&self, h: FgmHandle, v: u8) {
            self.push(Call::SetFgmVolume(h, v));
        }
        fn set_fgm_pan(&self, h: FgmHandle, v: u8) {
            self.push(Call::SetFgmPan(h, v));
        }
        fn set_fgm_fx(&self, h: FgmHandle, v: u8) {
            self.push(Call::SetFgmFx(h, v));
        }
        fn play_bgm(&self, p: u32, b: u32) -> i32 {
            self.push(Call::PlayBgm(p, b));
            self.state.lock().unwrap().bgm_playing = true;
            if b < super::id::nSYAudioBGMEnd {
                b as i32
            } else {
                -1
            }
        }
        fn stop_bgm(&self, p: u32) {
            self.push(Call::StopBgm(p));
            self.state.lock().unwrap().bgm_playing = false;
        }
        fn stop_bgm_all(&self) {
            self.push(Call::StopBgmAll);
            self.state.lock().unwrap().bgm_playing = false;
        }
        fn set_bgm_volume(&self, p: u32, v: u32) {
            self.push(Call::SetBgmVolume(p, v));
        }
        fn set_bgm_volume_fade(&self, p: u32, v: u32, t: u32) {
            self.push(Call::SetBgmVolumeFade(p, v, t));
        }
        fn set_bgm_reverb(&self, p: u32, r: u32) {
            self.push(Call::SetBgmReverb(p, r));
        }
        fn set_bgm_priority(&self, p: u32, pr: u8) {
            self.push(Call::SetBgmPriority(p, pr));
        }
        fn bgm_playing(&self, _p: u32) -> bool {
            self.state.lock().unwrap().bgm_playing
        }
        fn sy_play_fgm(&self, f: u32) -> i32 {
            self.push(Call::SyPlayFgm(f));
            0
        }
        fn sy_stop_fgm(&self, s: i32) {
            self.push(Call::SyStopFgm(s));
        }
        fn set_quality(&self, q: u32) {
            self.push(Call::SetQuality(q));
        }
        fn set_fx_type(&self, f: i32) {
            self.push(Call::SetFxType(f));
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn calls_reach_the_installed_recorder_in_order() {
            let r = Recorder::install();
            let h = crate::sound::play_fgm(crate::sound::id::nSYAudioFGMExplodeS).unwrap();
            crate::sound::stop_fgm(h);
            crate::sound::play_bgm(0, crate::sound::id::nSYAudioBGMOpening);
            assert_eq!(
                r.take(),
                [
                    Call::PlayFgm(0),
                    Call::StopFgm(h),
                    Call::PlayBgm(0, crate::sound::id::nSYAudioBGMOpening)
                ]
            );
            crate::sound::uninstall();
            assert_eq!(crate::sound::play_fgm(0), None);
        }

        #[test]
        fn position_pan_is_inverted_and_clamped() {
            let r = Recorder::install();
            crate::sound::play_fgm_at(5, 0.0);
            crate::sound::play_fgm_at(5, 4000.0);
            crate::sound::play_fgm_at(5, -100_000.0);
            assert_eq!(
                r.take(),
                [
                    Call::PlayFgmBalance(5, 64),
                    Call::PlayFgmBalance(5, 34),
                    Call::PlayFgmBalance(5, 124)
                ]
            );
            crate::sound::uninstall();
        }
    }
}
