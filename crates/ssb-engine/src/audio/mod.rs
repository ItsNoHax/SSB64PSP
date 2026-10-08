//! The N64 audio system: libultra's `n_audio` synthesizer and compressed
//! sequence player, the FGM sound-effect engine (`n_env.c`) and the game's
//! `syAudio` layer (`src/sys/audio.c`), ported as one portable subsystem
//! ([D-049](../../../../docs/decisions/D-049.md)).
//!
//! Nothing here touches the PSP. [`AudioApi`] is what game code calls (the
//! decomp's `syAudio*` and `func_8002xxxx` FGM entry points); a platform
//! implements it over an [`AudioSystem`] it renders on its own thread.

mod api;
mod shared;
mod system;

pub use api::{AudioApi, FgmHandle};
pub use shared::{RawLock, SharedAudio};
pub use system::{AudioError, AudioSystem};

/// `osAiSetFrequency(32000)`'s real NTSC rate: 48,681,812 / 1521. The synth's
/// timing (`_n_timeToSamples`, the FGM tick, the reverb) runs at this rate.
pub const N64_OUTPUT_RATE: i32 = 32006;

/// The PSP plays the N64-rate stream through its 32 kHz SRC channel (0.02 %
/// slow, D-049).
pub const PSP_OUTPUT_RATE: u32 = 32000;

/// `FIXED_SAMPLE`: `n_alAudioFrame` builds its output in sub-frames of 184
/// samples.
pub const SUB_FRAME_SAMPLES: usize = 184;

/// `sSYAudioFrequency`: ((32000 / 60) / 184) * 184 + 184.
pub const FRAME_SAMPLES_MAX: usize = 552;

/// `D_8009D920`: the short frame used while the output queue is full.
pub const FRAME_SAMPLES_MIN: usize = 368;
