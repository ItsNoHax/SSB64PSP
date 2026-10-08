//! [`AudioSystem`]: everything `syAudioThreadMain` owns. PLACEHOLDER: the
//! interface is final; the body renders silence until the synth lands.

use super::{FgmHandle, FRAME_SAMPLES_MAX, FRAME_SAMPLES_MIN};

/// Why the audio section could not be opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioError {
    /// The pack's audio section is missing or malformed.
    BadSection,
}

/// The whole audio system: synthesizer, sequence player, FGM engine and the
/// `syAudio` state. Built once from the pack's resident audio section; all
/// pools are allocated here, so rendering never allocates.
pub struct AudioSystem {
    mono: bool,
    short_frames: u8,
}

impl AudioSystem {
    /// Builds the system over the pack's audio section, which must outlive
    /// it (it stays resident for the process's life).
    pub fn new(section: &'static [u8]) -> Result<Self, AudioError> {
        let _ = section;
        Ok(Self {
            mono: false,
            short_frames: 0,
        })
    }

    /// `syAudioThreadMain`'s frame-length choice (audio.c:1004-1018):
    /// `queued` is the output queue's length in sample frames (the N64's
    /// `AI_LEN / 4`). Returns 552 or 368.
    pub fn frame_samples(&mut self, queued: u32) -> usize {
        if (queued > FRAME_SAMPLES_MIN as u32 && self.short_frames != 2)
            || (queued > 184 && self.short_frames == 0)
        {
            self.short_frames = 2;
            FRAME_SAMPLES_MIN
        } else {
            self.short_frames = self.short_frames.saturating_sub(1);
            FRAME_SAMPLES_MAX
        }
    }

    /// Renders one audio frame of `samples` stereo sample frames into
    /// `out[..samples * 2]` (interleaved L, R), running the frame's
    /// sequencer, FGM and `syAudio` work first. Applies the mono option.
    pub fn render_frame(&mut self, out: &mut [i16], samples: usize) {
        out[..samples * 2].fill(0);
        let _ = self.mono;
    }

    pub fn play_fgm(&mut self, _id: u16) -> Option<FgmHandle> {
        None
    }
    pub fn play_fgm_balance(&mut self, _id: u16, _balance: u8) -> Option<FgmHandle> {
        None
    }
    pub fn stop_fgm(&mut self, _handle: FgmHandle) {}
    pub fn stop_all_fgm(&mut self) {}
    pub fn pause_fgm(&mut self) {}
    pub fn resume_fgm(&mut self) {}
    pub fn fgm_alive(&mut self, _handle: FgmHandle) -> bool {
        false
    }
    pub fn fgm_count(&mut self) -> u16 {
        0
    }
    pub fn set_fgm_count(&mut self, _count: u16) {}
    pub fn set_fgm_master_volume(&mut self, _volume: u8) {}
    pub fn set_fgm_volume(&mut self, _handle: FgmHandle, _volume: u8) {}
    pub fn set_fgm_pan(&mut self, _handle: FgmHandle, _pan: u8) {}
    pub fn set_fgm_fx(&mut self, _handle: FgmHandle, _fx: u8) {}
    pub fn play_bgm(&mut self, _player: u32, _bgm: u32) -> i32 {
        -1
    }
    pub fn stop_bgm(&mut self, _player: u32) {}
    pub fn stop_bgm_all(&mut self) {}
    pub fn set_bgm_volume(&mut self, _player: u32, _volume: u32) {}
    pub fn set_bgm_volume_fade(&mut self, _player: u32, _volume: u32, _time: u32) {}
    pub fn set_bgm_reverb(&mut self, _player: u32, _reverb: u32) {}
    pub fn set_bgm_priority(&mut self, _player: u32, _priority: u8) {}
    pub fn bgm_playing(&mut self, _player: u32) -> bool {
        false
    }
    pub fn sy_play_fgm(&mut self, _fgm: u32) -> i32 {
        -1
    }
    pub fn sy_stop_fgm(&mut self, _slot: i32) {}
    /// `syAudioSetQuality`: 0 mono, 1 stereo.
    pub fn set_quality(&mut self, quality: u32) {
        self.mono = quality == 0;
    }
    pub fn set_fx_type(&mut self, _fx_type: i32) {}
}
