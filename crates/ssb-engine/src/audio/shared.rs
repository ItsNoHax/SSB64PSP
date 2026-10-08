//! [`SharedAudio`]: an [`AudioSystem`] behind a platform lock, so the game
//! thread's [`AudioApi`] calls and the audio thread's rendering never run
//! at once. The N64 masks interrupts around the same list operations
//! (`osSetIntMask` in `n_env.c`).

use core::cell::UnsafeCell;

use super::{AudioApi, AudioSystem, FgmHandle};

/// A mutual-exclusion primitive the platform supplies (a kernel semaphore
/// on the PSP, a mutex on the host). `lock` blocks until it is held.
pub trait RawLock: Sync {
    fn lock(&self);
    fn unlock(&self);
}

/// An [`AudioSystem`] shared between the game thread (through
/// [`AudioApi`]) and the audio thread (through [`SharedAudio::with`]).
pub struct SharedAudio<L: RawLock> {
    lock: L,
    sys: UnsafeCell<AudioSystem>,
}

// SAFETY: every access to `sys` happens while `lock` is held.
unsafe impl<L: RawLock> Sync for SharedAudio<L> {}

impl<L: RawLock> SharedAudio<L> {
    pub fn new(lock: L, sys: AudioSystem) -> Self {
        Self {
            lock,
            sys: UnsafeCell::new(sys),
        }
    }

    /// Runs `f` with the system locked: the audio thread renders a frame
    /// this way.
    pub fn with<R>(&self, f: impl FnOnce(&mut AudioSystem) -> R) -> R {
        self.lock.lock();
        // SAFETY: the lock is held, so this is the only reference.
        let r = f(unsafe { &mut *self.sys.get() });
        self.lock.unlock();
        r
    }
}

impl<L: RawLock> AudioApi for SharedAudio<L> {
    fn play_fgm(&self, id: u16) -> Option<FgmHandle> {
        self.with(|s| s.play_fgm(id))
    }
    fn play_fgm_balance(&self, id: u16, balance: u8) -> Option<FgmHandle> {
        self.with(|s| s.play_fgm_balance(id, balance))
    }
    fn stop_fgm(&self, handle: FgmHandle) {
        self.with(|s| s.stop_fgm(handle))
    }
    fn stop_all_fgm(&self) {
        self.with(|s| s.stop_all_fgm())
    }
    fn pause_fgm(&self) {
        self.with(|s| s.pause_fgm())
    }
    fn resume_fgm(&self) {
        self.with(|s| s.resume_fgm())
    }
    fn fgm_alive(&self, handle: FgmHandle) -> bool {
        self.with(|s| s.fgm_alive(handle))
    }
    fn fgm_count(&self) -> u16 {
        self.with(|s| s.fgm_count())
    }
    fn set_fgm_count(&self, count: u16) {
        self.with(|s| s.set_fgm_count(count))
    }
    fn set_fgm_master_volume(&self, volume: u8) {
        self.with(|s| s.set_fgm_master_volume(volume))
    }
    fn set_fgm_volume(&self, handle: FgmHandle, volume: u8) {
        self.with(|s| s.set_fgm_volume(handle, volume))
    }
    fn set_fgm_pan(&self, handle: FgmHandle, pan: u8) {
        self.with(|s| s.set_fgm_pan(handle, pan))
    }
    fn set_fgm_fx(&self, handle: FgmHandle, fx: u8) {
        self.with(|s| s.set_fgm_fx(handle, fx))
    }
    fn play_bgm(&self, player: u32, bgm: u32) -> i32 {
        self.with(|s| s.play_bgm(player, bgm))
    }
    fn stop_bgm(&self, player: u32) {
        self.with(|s| s.stop_bgm(player))
    }
    fn stop_bgm_all(&self) {
        self.with(|s| s.stop_bgm_all())
    }
    fn set_bgm_volume(&self, player: u32, volume: u32) {
        self.with(|s| s.set_bgm_volume(player, volume))
    }
    fn set_bgm_volume_fade(&self, player: u32, volume: u32, time: u32) {
        self.with(|s| s.set_bgm_volume_fade(player, volume, time))
    }
    fn set_bgm_reverb(&self, player: u32, reverb: u32) {
        self.with(|s| s.set_bgm_reverb(player, reverb))
    }
    fn set_bgm_priority(&self, player: u32, priority: u8) {
        self.with(|s| s.set_bgm_priority(player, priority))
    }
    fn bgm_playing(&self, player: u32) -> bool {
        self.with(|s| s.bgm_playing(player))
    }
    fn sy_play_fgm(&self, fgm: u32) -> i32 {
        self.with(|s| s.sy_play_fgm(fgm))
    }
    fn sy_stop_fgm(&self, slot: i32) {
        self.with(|s| s.sy_stop_fgm(slot))
    }
    fn set_quality(&self, quality: u32) {
        self.with(|s| s.set_quality(quality))
    }
    fn set_fx_type(&self, fx_type: i32) {
        self.with(|s| s.set_fx_type(fx_type))
    }
}
