//! [`AudioSystem`]: everything `syAudioThreadMain` owns (`src/sys/audio.c`):
//! the synthesizer, the BGM sequence player, the FGM engine and the
//! `syAudio` state machine, run one N64 audio frame at a time.
//!
//! Deviations (D-049):
//! - The settings-updated / restart path (audio.c:1162-1239) is not ported.
//!   Boot restarts twice (scmanager.c:836-849) to reach `AL_FX_CUSTOM` with
//!   an FGM LFO pool of 72; [`AudioSystem::new`] builds that final
//!   configuration directly. [`AudioSystem::set_fx_type`] (only the debug
//!   cube changes it) swaps the effect without restarting the players.
//! - The mono pass averages the frame just rendered: the N64 averages the
//!   previous frame's buffer right before queueing it (audio.c:1067-1078),
//!   with the same samples and the quality flag read one frame later.
//! - The game calls the sound API synchronously under the platform's lock
//!   (`SharedAudio`), as it does under `osSetIntMask` on the N64.

use super::csplayer::Csp;
use super::data::AudioData;
use super::fgm::Fgm;
use super::reverb::{self, Fx};
use super::synth::{Client, Clients, Synth, AL_STOPPED};
use super::{FgmHandle, FRAME_SAMPLES_MAX, FRAME_SAMPLES_MIN, N64_OUTPUT_RATE};

/// Why the audio section could not be opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioError {
    /// The pack's audio section is missing or malformed.
    BadSection,
}

/// Drop counters of the fixed pools ([`AudioSystem::drops`]).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AudioDrops {
    /// `ALParam` updates (pool of 128).
    pub synth_params: u32,
    /// Sequence player events (pool of 64).
    pub seq_events: u32,
    /// `syAudioInitOsc` with no free state (pool of 32).
    pub oscillators: u32,
    /// FGM scripts (24), voices (24) and LFOs (72).
    pub fgm_scripts: u32,
    pub fgm_voices: u32,
    pub fgm_lfos: u32,
    /// FGM voices that found no physical voice.
    pub fgm_synth_voices: u32,
}

/// `AL_MAX_CHANNELS`.
const MAX_CHANNELS: u8 = 16;
/// `AL_MAX_PRIORITY`.
const MAX_PRIORITY: u8 = 127;
/// The BGM volume ceiling (`syAudioSetBGMVolume`).
const BGM_VOLUME_MAX: u32 = 30720;
/// `dSYAudioPublicSettings.sndplayers_num`: `syAudioPlayFGM`'s slots.
const SOUND_PLAYERS: usize = 24;
/// `dSYAudioPublicSettings.priority`.
const BGM_PRIORITY: u8 = 20;

/// The synthesizer's clients: `n_sndp` is the FGM engine (added first by
/// `func_80026204`), `n_seqp1` the BGM player.
struct Players<'a> {
    csp: &'a mut Csp,
    fgm: &'a mut Fgm,
}

impl Clients for Players<'_> {
    fn handle(&mut self, client: Client, syn: &mut Synth, data: &AudioData) -> i32 {
        match client {
            Client::Snd => self.fgm.tick(syn, data),
            Client::Seq1 | Client::Seq2 => self.csp.handler(syn, data),
        }
    }
}

/// The whole audio system: synthesizer, sequence player, FGM engine and the
/// `syAudio` state. Built once from the pack's resident audio section; all
/// pools are allocated here, so rendering never allocates.
pub struct AudioSystem {
    data: AudioData,
    synth: Synth,
    csp: Csp,
    fgm: Fgm,
    /// `dSYAudioSoundQuality == 0`.
    mono: bool,
    /// `sp73`: the frame-length chooser's countdown.
    short_frames: u8,
    /// `dSYAudioCurrentFxType`.
    fx_type: i32,
    /// `sSYAudioCSPlayerStatuses[0]`: 0 idle, 1 stop then load, 2 start,
    /// 3 playing.
    bgm_status: i32,
    /// `sSYAudioBGMPlayingIDs[0]`.
    bgm_id: i32,
    /// `sSYAudioBGMVolumeTimers[0]`, `sSYAudioBGMVolumes[0]`,
    /// `sSYAudioBGMVolumeRates[0]`.
    bgm_volume_timer: i32,
    bgm_volume: f32,
    bgm_volume_rate: f32,
    /// `gSYAudioGlobalBGMPriority`.
    bgm_priority: u8,
    /// `sSYAudioSoundPlayers`: FGM node indices.
    sound_players: [Option<u8>; SOUND_PLAYERS],
}

impl AudioSystem {
    /// Builds the system over the pack's audio section, which must outlive
    /// it (it stays resident for the process's life). `syAudioMakeBGMPlayers`
    /// (audio.c:880-988) in its final boot configuration.
    pub fn new(section: &'static [u8]) -> Result<Self, AudioError> {
        let data = AudioData::new(section).map_err(|_| AudioError::BadSection)?;
        let fx_type = reverb::AL_FX_CUSTOM;
        let fx = Fx::new(
            reverb::params_for(fx_type, &data.custom_fx, &data.presets),
            N64_OUTPUT_RATE,
        );
        let mut synth = Synth::new(N64_OUTPUT_RATE, Some(fx));
        let fgm = Fgm::new(&mut synth, &data);
        let csp = Csp::new(&mut synth, &data);
        Ok(Self {
            data,
            synth,
            csp,
            fgm,
            mono: false,
            short_frames: 0,
            fx_type,
            bgm_status: 0,
            bgm_id: -1,
            bgm_volume_timer: 0,
            bgm_volume: BGM_VOLUME_MAX as f32,
            bgm_volume_rate: 0.0,
            bgm_priority: BGM_PRIORITY,
            sound_players: [None; SOUND_PLAYERS],
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
    /// sequencer, FGM and `syAudio` work. Applies the mono option.
    pub fn render_frame(&mut self, out: &mut [i16], samples: usize) {
        let t_total = super::prof::start();
        let out = &mut out[..samples * 2];
        let mut players = Players {
            csp: &mut self.csp,
            fgm: &mut self.fgm,
        };
        self.synth
            .audio_frame(&self.data, &mut players, out, samples);
        let t = super::prof::start();
        if self.mono {
            mono(out);
        }
        // audio.c:1090-1096: free the slots whose script has ended.
        for slot in &mut self.sound_players {
            if let Some(node) = *slot {
                if self.fgm.timer(node) == 0 {
                    *slot = None;
                }
            }
        }
        self.bgm_frame();
        self.volume_fade_frame();
        super::prof::stop(super::prof::Stage::Post, t);
        super::prof::stop(super::prof::Stage::Total, t_total);
    }

    /// The BGM status machine (audio.c:1097-1141).
    fn bgm_frame(&mut self) {
        match self.bgm_status {
            1 => {
                if self.csp.state() != AL_STOPPED {
                    self.csp.stop();
                } else if self.bgm_id < 0 {
                    self.bgm_status -= 1;
                } else {
                    self.csp.load_sequence(&self.data, self.bgm_id as usize);
                    self.bgm_status += 1;
                }
            }
            2 => {
                self.csp.set_sequence();
                self.csp.play();
                for chan in 0..MAX_CHANNELS {
                    self.csp.set_chl_priority(chan, self.bgm_priority);
                }
                self.bgm_status += 1;
            }
            3 if self.csp.state() == AL_STOPPED => {
                self.bgm_status = 0;
                self.bgm_id = -1;
            }
            _ => {}
        }
    }

    /// The BGM volume fade (audio.c:1142-1161).
    fn volume_fade_frame(&mut self) {
        if self.bgm_volume_timer != 0 {
            self.bgm_volume_timer -= 1;
            self.bgm_volume += self.bgm_volume_rate;
            if self.bgm_volume < 0.0 {
                self.bgm_volume = 0.0;
            } else if self.bgm_volume > BGM_VOLUME_MAX as f32 {
                self.bgm_volume = BGM_VOLUME_MAX as f32;
            }
            self.csp.set_vol(self.bgm_volume as i32 as i16);
        }
    }

    // --- FGM: `n_env.c` ------------------------------------------------------

    /// `func_800269C0`.
    pub fn play_fgm(&mut self, id: u16) -> Option<FgmHandle> {
        self.fgm.play(id)
    }
    /// `func_80026A10`, node `0x2F = balance`, `func_800267F4`.
    pub fn play_fgm_balance(&mut self, id: u16, balance: u8) -> Option<FgmHandle> {
        self.fgm.play_balance(id, balance)
    }
    /// `func_80026738`, for a handle that still matches.
    pub fn stop_fgm(&mut self, handle: FgmHandle) {
        self.fgm.stop(&mut self.synth, handle);
    }
    /// `func_800266A0` (and `func_80020E28`).
    pub fn stop_all_fgm(&mut self) {
        self.fgm.stop_all(&mut self.synth);
    }
    /// `func_80026594`.
    pub fn pause_fgm(&mut self) {
        self.fgm.pause(&mut self.synth);
    }
    /// `func_800264A4`.
    pub fn resume_fgm(&mut self) {
        self.fgm.resume(&mut self.synth);
    }
    pub fn fgm_alive(&mut self, handle: FgmHandle) -> bool {
        self.fgm.alive(handle)
    }
    pub fn fgm_count(&mut self) -> u16 {
        self.fgm.count()
    }
    pub fn set_fgm_count(&mut self, count: u16) {
        self.fgm.set_count(count);
    }
    /// `func_80026070`.
    pub fn set_fgm_master_volume(&mut self, volume: u8) {
        self.fgm.set_master_volume(volume);
    }
    /// `func_80026174`.
    pub fn set_fgm_volume(&mut self, handle: FgmHandle, volume: u8) {
        self.fgm.set_volume(handle, volume);
    }
    /// `func_80026104`.
    pub fn set_fgm_pan(&mut self, handle: FgmHandle, pan: u8) {
        self.fgm.set_pan(handle, pan);
    }
    /// `func_80026094`.
    pub fn set_fgm_fx(&mut self, handle: FgmHandle, fx: u8) {
        self.fgm.set_fx(handle, fx);
    }

    // --- syAudio --------------------------------------------------------------

    /// `syAudioPlayBGM`. There is one BGM player
    /// (`SYAUDIO_BGMPLAYERS_NUM`); other `player` values are ignored.
    pub fn play_bgm(&mut self, player: u32, bgm: u32) -> i32 {
        if (bgm as usize) < self.data.seqs.len() {
            if player == 0 {
                self.bgm_status = 1;
                self.bgm_id = bgm as i32;
            }
            bgm as i32
        } else {
            -1
        }
    }
    /// `syAudioStopBGM`.
    pub fn stop_bgm(&mut self, player: u32) {
        if player == 0 {
            self.bgm_status = 1;
            self.bgm_id = -1;
        }
    }
    /// `syAudioStopBGMAll`.
    pub fn stop_bgm_all(&mut self) {
        self.stop_bgm(0);
    }
    /// `syAudioSetBGMVolume`.
    pub fn set_bgm_volume(&mut self, player: u32, volume: u32) {
        if player != 0 {
            return;
        }
        let vol = volume.min(BGM_VOLUME_MAX);
        self.csp.set_vol(vol as i16);
        self.bgm_volume = vol as f32;
        self.bgm_volume_timer = 0;
    }
    /// `syAudioSetBGMVolumeFade`: `(vol - current) / time` in `f32`, applied
    /// once per audio frame.
    pub fn set_bgm_volume_fade(&mut self, player: u32, volume: u32, time: u32) {
        if player != 0 {
            return;
        }
        let vol = volume.min(BGM_VOLUME_MAX);
        if time != 0 {
            self.bgm_volume_timer = time as i32;
            self.bgm_volume_rate = (vol as f32 - self.bgm_volume) / time as f32;
        } else {
            self.set_bgm_volume(player, vol);
        }
    }
    /// `syAudioSetBGMReverb`.
    pub fn set_bgm_reverb(&mut self, player: u32, reverb: u32) {
        if player != 0 {
            return;
        }
        let reverb = reverb.min(127) as u8;
        for chan in 0..MAX_CHANNELS {
            self.csp.set_chl_fx_mix(chan, reverb);
        }
    }
    /// `syAudioSetBGMPriority`.
    pub fn set_bgm_priority(&mut self, player: u32, priority: u8) {
        let priority = priority.min(MAX_PRIORITY);
        self.bgm_priority = priority;
        if player != 0 {
            return;
        }
        for chan in 0..MAX_CHANNELS {
            self.csp.set_chl_priority(chan, priority);
        }
    }
    /// `syAudioCheckBGMPlaying`: the player's own state, so a play request
    /// still waiting in the status machine reads as not playing.
    pub fn bgm_playing(&mut self, player: u32) -> bool {
        player == 0 && self.csp.state() != AL_STOPPED
    }
    /// `syAudioPlayFGM`: the first free slot takes `func_800269C0`'s node
    /// (which may be none); returns the slot or -1.
    pub fn sy_play_fgm(&mut self, fgm: u32) -> i32 {
        let Some(i) = self.sound_players.iter().position(Option::is_none) else {
            return -1;
        };
        // `func_800269C0` takes a u16: the id is truncated, not clamped.
        self.sound_players[i] = self.fgm.play(fgm as u16).map(|h| h.slot);
        i as i32
    }
    /// `syAudioStopFGM`: stops whatever node the slot still points at.
    pub fn sy_stop_fgm(&mut self, slot: i32) {
        let Some(entry) = usize::try_from(slot)
            .ok()
            .and_then(|s| self.sound_players.get_mut(s))
        else {
            return;
        };
        if let Some(node) = entry.take() {
            self.fgm.stop_node(&mut self.synth, node);
        }
    }
    /// `syAudioSetQuality`: 0 mono, 1 stereo.
    pub fn set_quality(&mut self, quality: u32) {
        self.mono = quality == 0;
    }
    /// `syAudioSetFXType`: rebuilds the aux effect (see the module doc).
    pub fn set_fx_type(&mut self, fx_type: i32) {
        if fx_type == self.fx_type {
            return;
        }
        self.fx_type = fx_type;
        self.synth.fx = (fx_type != reverb::AL_FX_NONE).then(|| {
            Fx::new(
                reverb::params_for(fx_type, &self.data.custom_fx, &self.data.presets),
                N64_OUTPUT_RATE,
            )
        });
    }

    // --- Diagnostics ------------------------------------------------------------

    /// Physical voices playing (at most 16).
    pub fn active_voices(&self) -> u32 {
        self.synth.active_voices() as u32
    }
    /// Work dropped because a fixed pool was full, as the original drops
    /// it (all zero in normal play).
    pub fn drops(&self) -> AudioDrops {
        AudioDrops {
            synth_params: self.synth.params_dropped,
            seq_events: self.csp.events_dropped(),
            oscillators: self.csp.osc.exhausted,
            fgm_scripts: self.fgm.script_exhausted,
            fgm_voices: self.fgm.voice_exhausted,
            fgm_lfos: self.fgm.lfo_exhausted,
            fgm_synth_voices: self.fgm.synth_alloc_failed,
        }
    }
    /// Physical voices stolen so far (`_allocatePVoice`).
    pub fn voice_steals(&self) -> u32 {
        self.synth.steals
    }
    /// The BGM id the status machine holds (-1 when none).
    pub fn bgm_id(&self) -> i32 {
        self.bgm_id
    }
}

/// The mono pass: `(L + R) / 2`, C division truncating toward zero.
fn mono(out: &mut [i16]) {
    for pair in out.as_chunks_mut::<2>().0 {
        let m = ((pair[0] as i32 + pair[1] as i32) / 2) as i16;
        pair[0] = m;
        pair[1] = m;
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::super::testdata;
    use super::*;
    use alloc::vec;
    use alloc::vec::Vec;

    #[test]
    fn mono_average_truncates_toward_zero() {
        let mut s = [3, 4, -3, -4, 32767, 32767, -32768, -32767, 1, -2];
        mono(&mut s);
        assert_eq!(s, [3, 3, -3, -3, 32767, 32767, -32767, -32767, 0, 0]);
    }

    fn system() -> Option<AudioSystem> {
        Some(AudioSystem::new(testdata::section()?).expect("audio system"))
    }

    /// Renders `frames` frames of 552 samples, appending to `pcm`.
    fn run(sys: &mut AudioSystem, frames: usize, pcm: &mut Vec<i16>) {
        let mut buf = [0i16; FRAME_SAMPLES_MAX * 2];
        for _ in 0..frames {
            sys.render_frame(&mut buf, FRAME_SAMPLES_MAX);
            pcm.extend_from_slice(&buf);
            assert!(sys.active_voices() <= 16);
        }
    }

    /// audio.c:1097-1141: play -> 1 (stop, then read) -> 2 (start) -> 3;
    /// stop -> 1 (stop the player) -> 0 with the id cleared.
    #[test]
    fn bgm_status_machine() {
        let Some(mut sys) = system() else { return };
        let mut pcm = Vec::new();
        assert_eq!(sys.play_bgm(0, 47), -1, "past seqCount");
        assert_eq!(sys.bgm_status, 0);
        assert_eq!(sys.play_bgm(0, 34), 34);
        assert_eq!((sys.bgm_status, sys.bgm_id), (1, 34));
        assert!(!sys.bgm_playing(0), "a request is not playing yet");
        run(&mut sys, 1, &mut pcm);
        assert_eq!(sys.bgm_status, 2, "player idle: sequence read");
        run(&mut sys, 1, &mut pcm);
        assert_eq!(sys.bgm_status, 3, "seq set and play posted");
        run(&mut sys, 1, &mut pcm);
        assert!(sys.bgm_playing(0));
        assert_eq!(sys.bgm_status, 3);
        run(&mut sys, 30, &mut pcm);
        // A new request while playing stops the player first.
        sys.play_bgm(0, 33);
        run(&mut sys, 1, &mut pcm);
        assert_eq!(sys.bgm_status, 1, "waits for the player to stop");
        let mut frames = 1;
        while sys.bgm_status == 1 {
            run(&mut sys, 1, &mut pcm);
            frames += 1;
            assert!(frames < 30);
        }
        assert_eq!((sys.bgm_status, sys.bgm_id), (2, 33));
        run(&mut sys, 3, &mut pcm);
        assert!(sys.bgm_playing(0));
        sys.stop_bgm(0);
        assert_eq!((sys.bgm_status, sys.bgm_id), (1, -1));
        let mut frames = 0;
        while sys.bgm_status != 0 {
            run(&mut sys, 1, &mut pcm);
            frames += 1;
            assert!(frames < 30);
        }
        assert_eq!(sys.bgm_id, -1);
        assert!(!sys.bgm_playing(0));
    }

    /// `syAudioSetBGMVolumeFade`: `(vol - cur) / time` in f32 (negative
    /// when fading down, although `vol` is a u32), applied per frame and
    /// clamped to [0, 30720]; the target is clamped to 30720 first.
    #[test]
    fn fade_rate_negative() {
        let Some(mut sys) = system() else { return };
        let mut pcm = Vec::new();
        sys.set_bgm_volume_fade(0, 0, 10);
        assert_eq!(sys.bgm_volume_rate, -3072.0);
        assert_eq!(sys.bgm_volume_timer, 10);
        run(&mut sys, 4, &mut pcm);
        assert_eq!(sys.bgm_volume, 30720.0 - 4.0 * 3072.0);
        run(&mut sys, 10, &mut pcm);
        assert_eq!((sys.bgm_volume, sys.bgm_volume_timer), (0.0, 0));
        sys.set_bgm_volume_fade(0, 40000, 3);
        assert_eq!(sys.bgm_volume_rate, 10240.0);
        run(&mut sys, 3, &mut pcm);
        assert_eq!(sys.bgm_volume, 30720.0);
        // A fade over 0 frames is an immediate set that cancels the fade.
        sys.set_bgm_volume_fade(0, 100, 5);
        sys.set_bgm_volume_fade(0, 200, 0);
        assert_eq!((sys.bgm_volume, sys.bgm_volume_timer), (200.0, 0));
    }

    /// `syAudioPlayFGM` slots: the first free slot, freed by the thread
    /// when the script ends; the id is truncated to u16.
    #[test]
    fn sy_play_fgm_slots() {
        let Some(mut sys) = system() else { return };
        let mut pcm = Vec::new();
        assert_eq!(sys.sy_play_fgm(0), 0);
        assert_eq!(sys.sy_play_fgm(0x1_0000), 1, "truncated to fgm 0");
        assert!(sys.sound_players[1].is_some());
        sys.sy_stop_fgm(0);
        assert!(sys.sound_players[0].is_none());
        assert_eq!(sys.sy_play_fgm(0), 0);
        run(&mut sys, 120, &mut pcm);
        assert!(
            sys.sound_players.iter().all(Option::is_none),
            "ended scripts free their slots"
        );
    }

    /// Peak and RMS of an interleaved buffer.
    fn level(pcm: &[i16]) -> (i32, f32) {
        let peak = pcm.iter().map(|s| (*s as i32).abs()).max().unwrap_or(0);
        let sum: f32 = pcm.iter().map(|s| (*s as f32) * (*s as f32)).sum();
        (peak, (sum / pcm.len().max(1) as f32).sqrt())
    }

    /// End to end: 10 s each of the Opening (33) and How to Play (34)
    /// BGMs, then a burst of FGMs over the music: audible, the music
    /// unclipped, at most 16 voices, nothing dropped, silent once stopped.
    #[test]
    fn renders_bgm_and_fgms() {
        let Some(mut sys) = system() else { return };
        for bgm in [33, 34] {
            sys.play_bgm(0, bgm);
            let mut pcm = Vec::new();
            run(&mut sys, 580, &mut pcm);
            let (peak, rms) = level(&pcm[pcm.len() / 2..]);
            assert!(
                rms > 200.0 && peak > 2000,
                "bgm {bgm}: rms {rms} peak {peak}"
            );
            assert!(peak < 32767, "bgm {bgm} clips");
        }
        let mut pcm = Vec::new();
        for (i, fgm) in [0u16, 1, 2, 3, 30, 60, 100, 150].into_iter().enumerate() {
            assert!(sys.play_fgm(fgm).is_some());
            run(&mut sys, 5 + i, &mut pcm);
        }
        run(&mut sys, 120, &mut pcm);
        // Eight loud effects over the music saturate now and then, as the
        // N64's mixer does (its own captures peak at 32768).
        let (_, rms) = level(&pcm);
        let clipped = pcm.iter().filter(|s| s.unsigned_abs() >= 32767).count();
        assert!(rms > 200.0, "fgms: rms {rms}");
        assert!(clipped * 200 < pcm.len(), "fgms: {clipped} clipped samples");
        // The oscillator pool runs dry by design: instruments 2 and 3 of
        // the music bank have a vibrato with delay 0, whose state
        // `syAudioInitOsc` takes but no event ever returns (see
        // `Csp::handle_midi`), exactly as on the N64.
        let drops = sys.drops();
        assert_eq!(
            AudioDrops {
                oscillators: 0,
                ..drops
            },
            AudioDrops::default()
        );
        sys.stop_bgm_all();
        sys.stop_all_fgm();
        let mut tail = Vec::new();
        run(&mut sys, 120, &mut tail);
        let (_, rms) = level(&tail[tail.len() - 552 * 2 * 10..]);
        assert!(rms < 50.0, "silence after stopping everything: rms {rms}");
        assert_eq!(sys.active_voices(), 0);
    }

    /// Locks the renderer's output (`golden`): every optimisation of the
    /// synthesizer must keep these hashes.
    #[test]
    fn pcm_hash_golden() {
        let Some(section) = testdata::section() else {
            return;
        };
        let r = super::super::golden::run(section, || 0).unwrap();
        std::println!("pcm hashes: {:#018x?}", r.hashes);
        assert_eq!(r.max_voices, 16);
        assert_eq!(r.hashes, super::super::golden::HASHES);
    }

    /// The same calls produce the same PCM, sample for sample, including
    /// 368-sample frames and the mono pass.
    #[test]
    fn deterministic() {
        let Some(section) = testdata::section() else {
            return;
        };
        let render = || {
            let mut sys = AudioSystem::new(section).unwrap();
            let mut pcm = vec![0i16; 0];
            let mut buf = [0i16; FRAME_SAMPLES_MAX * 2];
            sys.play_bgm(0, 33);
            for f in 0..300u32 {
                if f % 37 == 5 {
                    sys.play_fgm((f % 200) as u16);
                }
                if f == 150 {
                    sys.set_quality(0);
                    sys.set_bgm_volume_fade(0, 10000, 40);
                }
                let n = sys.frame_samples(if f % 7 == 0 { 400 } else { 0 });
                sys.render_frame(&mut buf, n);
                pcm.extend_from_slice(&buf[..n * 2]);
            }
            pcm
        };
        let a = render();
        let b = render();
        assert!(a.iter().any(|s| *s != 0));
        assert!(a == b, "renders differ");
    }
}
