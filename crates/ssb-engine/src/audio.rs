//! Audio abstraction.
//!
//! Deliberately *not* modelled on the N64 audio architecture. The original
//! runs an RSP microcode synthesiser (`aspMain`) driving a sequence player
//! over ALBank instrument banks with VADPCM samples, all scheduled as audio
//! tasks alongside graphics tasks. Reproducing that shape on PSP would mean
//! carrying an N64-specific design into a machine with completely different
//! audio hardware.
//!
//! Instead the build-time pipeline lowers the N64 formats to something a
//! simple mixer can play, and this trait describes what the game needs:
//! trigger a sound, start a track, adjust volume.

/// Identifies a sound effect. In the original these are "FGM" ids
/// (`gmFGMVoiceID`), which is the numbering the extracted assets keep.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SfxId(pub u16);

/// Identifies a music sequence. 47 exist in the US ROM.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MusicId(pub u16);

/// A playing voice, so the caller can stop or retune it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VoiceHandle(pub u32);

/// Volume in `0.0..=1.0`.
pub type Volume = f32;

/// Stereo pan: `-1.0` full left, `0.0` centre, `1.0` full right.
pub type Pan = f32;

/// Mixer channels the game controls independently, matching the original's
/// separate music/SFX volume options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bus {
    Music,
    Sfx,
    /// Announcer and character voice clips, which the options menu treats
    /// separately from ordinary effects.
    Voice,
}

/// What the game needs from an audio backend.
pub trait AudioBackend {
    /// Plays a sound effect, returning a handle if a voice was free.
    ///
    /// Returning `None` rather than stealing silently is deliberate: voice
    /// exhaustion during a busy 4-player match is a real condition worth
    /// surfacing to the profiler.
    fn play_sfx(&mut self, id: SfxId, volume: Volume, pan: Pan) -> Option<VoiceHandle>;

    fn stop_sfx(&mut self, voice: VoiceHandle);

    /// Starts a music track, replacing whatever was playing.
    fn play_music(&mut self, id: MusicId, looping: bool);

    fn stop_music(&mut self);

    /// Fades the current track out over `frames` simulation ticks.
    fn fade_music_out(&mut self, frames: u32);

    fn set_bus_volume(&mut self, bus: Bus, volume: Volume);

    fn bus_volume(&self, bus: Bus) -> Volume;

    /// Called once per frame to advance sequencing and refill buffers.
    fn update(&mut self);

    /// Voices currently sounding, for the profiler.
    fn active_voices(&self) -> u32;
}

/// Sample rate the converted assets are resampled to.
///
/// The PSP's hardware audio output runs at 44.1 kHz; resampling once at build
/// time is cheaper and better-sounding than doing it per-voice at runtime.
pub const OUTPUT_SAMPLE_RATE: u32 = 44_100;

/// Samples the PSP audio hardware consumes per `sceAudioOutputBlocking` call.
/// Must be a multiple of 64.
///
/// At 44.1 kHz this is ~23 ms of audio — **longer than a 16.67 ms frame**.
/// That is not a bug, it is the constraint that dictates the backend's shape:
/// mixing cannot be a step inside the frame loop, because a blocking output
/// call would stall rendering for longer than the frame budget. The PSP
/// backend runs mixing on its own thread and the game thread only ever
/// enqueues events. [`AudioBackend::update`] is therefore expected to be
/// cheap and non-blocking.
pub const AUDIO_BLOCK_SAMPLES: usize = 1024;

/// Duration of one audio block, in microseconds.
pub const AUDIO_BLOCK_US: u64 = AUDIO_BLOCK_SAMPLES as u64 * 1_000_000 / OUTPUT_SAMPLE_RATE as u64;

/// Compile-time record of the constraint that shapes the audio backend: one
/// hardware block outlasts a simulation tick, so a blocking write must never
/// happen on the game thread.
///
/// A `const` assertion rather than a test, because both operands are constants
/// — a runtime `assert!` on them is tautological and clippy rightly objects.
/// If either constant is ever changed so that a block fits inside a frame, this
/// fails the build and the comment above becomes wrong, which is the point.
const _: () = assert!(AUDIO_BLOCK_US > crate::timing::TICK_US);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_block_is_hardware_aligned() {
        // sceAudioChReserve rejects sample counts that are not a multiple of 64.
        assert_eq!(AUDIO_BLOCK_SAMPLES % 64, 0);
    }

    #[test]
    fn block_duration_matches_sample_count() {
        assert_eq!(
            AUDIO_BLOCK_US,
            AUDIO_BLOCK_SAMPLES as u64 * 1_000_000 / OUTPUT_SAMPLE_RATE as u64
        );
    }
}
