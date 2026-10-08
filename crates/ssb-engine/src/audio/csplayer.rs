//! The compressed MIDI sequence player (`N_ALCSPlayer`): `n_csplayer.c` in
//! `n_env.c:2383-3266` (`n_alCSPNew`, `__n_CSPVoiceHandler`,
//! `__n_CSPHandleNextSeqEvent`, `__n_CSPHandleMIDIMsg`,
//! `__n_CSPHandleMetaMsg`, `__n_CSPRepostEvent`, `__n_setUsptFromTempo`,
//! `__n_CSPPostNextSeqEvent`, SSB's `func_800290AC` / `func_8002909C`), the
//! `n_csp*.c` API, and the `n_seqplayer.c` helpers it shares with the
//! uncompressed player (`__n_vsVol`, `__n_vsPan`, `__n_vsDelta`,
//! `__n_seqpReleaseVoice`, `__n_voiceNeedsNoteKill`, `__n_seqpStopOsc`,
//! `__n_mapVoice`, `__n_unmapVoice`, `__n_lookupVoice`,
//! `__n_lookupSoundQuick`, `__n_initChanState`, `__n_initFromBank`,
//! `__n_resetPerfChanState`, `__n_setInstChanState(_Alt)`,
//! `n_alSeqpGetVol`, `n_alSeqpGetChlVol`).
//!
//! Voice state `i` drives synth vvoice [`CSP_VOICE_BASE`]` + i`. The game
//! has one player (`SYAUDIO_BGMPLAYERS_NUM`), so SSB's two globals
//! `D_8003D318` (tempo scale) and `D_8003D31C` (channel mask) live here.

use super::cseq::CSeq;
use super::data::{AudioData, WaveRef};
use super::evtq::{
    Event, EventQueue, MidiEvent, TempoEvent, AL_CSP_NOTEOFF_EVT, AL_EVTQ_END, AL_SEQP_MIDI_EVT,
    AL_SEQ_REF_EVT, NIL as EVT_NIL,
};
use super::osc::{cents2ratio, OscPool};
use super::synth::{
    Client, Synth, VoiceConfig, AL_PLAYING, AL_STOPPED, AL_STOPPING, CSP_VOICE_BASE,
};
use alloc::vec;
use alloc::vec::Vec;

/// `AL_MAX_CHANNELS`.
pub const MAX_CHANNELS: usize = 16;
/// `voices_num_max[0]` (`dSYAudioPublicSettings`).
pub const MAX_VOICES: usize = 24;
/// `events_num_max`.
pub const MAX_EVENTS: usize = 64;

/// `AL_USEC_PER_FRAME`.
const AL_USEC_PER_FRAME: i32 = 16000;
/// `AL_GAIN_CHANGE_TIME`.
const AL_GAIN_CHANGE_TIME: i32 = 1000;
/// `KILL_TIME`: 50 ms.
const KILL_TIME: i32 = 50000;
/// `AL_SUSTAIN`.
const AL_SUSTAIN: u8 = 63;
/// `AL_VOL_FULL`.
const AL_VOL_FULL: u8 = 127;

const AL_PHASE_ATTACK: u8 = 0;
const AL_PHASE_NOTEON: u8 = 0;
const AL_PHASE_DECAY: u8 = 1;
const AL_PHASE_SUSTAIN: u8 = 2;
const AL_PHASE_RELEASE: u8 = 3;
const AL_PHASE_SUSTREL: u8 = 4;

const AL_MIDI_NOTE_OFF: u8 = 0x80;
const AL_MIDI_NOTE_ON: u8 = 0x90;
const AL_MIDI_POLY_KEY_PRESSURE: u8 = 0xA0;
const AL_MIDI_CONTROL_CHANGE: u8 = 0xB0;
const AL_MIDI_PROGRAM_CHANGE: u8 = 0xC0;
const AL_MIDI_CHANNEL_PRESSURE: u8 = 0xD0;
const AL_MIDI_PITCH_BEND_CHANGE: u8 = 0xE0;

const AL_MIDI_VOLUME_CTRL: u8 = 0x07;
const AL_MIDI_PAN_CTRL: u8 = 0x0A;
const AL_MIDI_PRIORITY_CTRL: u8 = 0x10;
const AL_MIDI_FX_CTRL_0: u8 = 0x14;
const AL_MIDI_FX_CTRL_1: u8 = 0x15;
const AL_MIDI_FX_CTRL_2: u8 = 0x16;
const AL_MIDI_FX_CTRL_3: u8 = 0x17;
const AL_MIDI_FX_CTRL_4: u8 = 0x18;
const AL_MIDI_FX_CTRL_5: u8 = 0x19;
const AL_MIDI_SUSTAIN_CTRL: u8 = 0x40;
const AL_MIDI_FX1_CTRL: u8 = 0x5B;

const AL_MIDI_META: u8 = 0xFF;
const AL_MIDI_META_TEMPO: u8 = 0x51;

/// A null voice-state link.
const NIL: u8 = 0xFF;

/// `ALChanState` (`include/PR/libaudio.h:708-725`, with SSB's five extra
/// bytes).
#[derive(Debug, Clone, Copy, Default)]
pub struct ChanState {
    /// Index into the bank's instruments; `None` is NULL.
    pub instrument: Option<u16>,
    pub bend_range: i16,
    pub fx_id: u8,
    pub pan: u8,
    pub priority: u8,
    pub vol: u8,
    pub fxmix: u8,
    pub sustain: u8,
    pub pitch_bend: f32,
    /// `unk_0x10`: channel pressure, which scales the vibrato depth.
    pub pressure: u8,
    pub vol2: u8,
    /// `unk_0x12` / `unk_0x13`: the second fx mix pair of
    /// `n_alSynStartVoiceParams_Alt` / `n_alSynSetFXMix_Alt`.
    pub unk12: u8,
    pub unk13: u8,
    /// `unk_0x14`: the bank slot program changes use (0 `bank`, 1
    /// `unknown0`, 2 `unknown1`).
    pub bank_slot: u8,
}

/// `N_ALVoiceState`.
#[derive(Debug, Clone, Copy, Default)]
pub struct VoiceState {
    next: u8,
    /// Index into the bank's sounds.
    sound: u16,
    env_end_time: i32,
    pitch: f32,
    vibrato: f32,
    env_gain: u8,
    channel: u8,
    key: u8,
    velocity: u8,
    env_phase: u8,
    phase: u8,
    tremelo: u8,
    flags: u8,
}

/// `N_ALCSPlayer`, plus the per-player sequence storage
/// (`sSYAudioBGMSequenceDatas[0]`, `sSYAudioCSeqs[0]`).
pub struct Csp {
    /// `bank`, `unknown0`, `unknown1`: whether each bank slot is set. Slot
    /// 0 is the music bank after `AL_SEQP_BANK_EVT`; SSB never sets the
    /// other two.
    banks: [bool; 3],
    /// `target != NULL`.
    target: bool,
    seq: CSeq,
    seq_buf: Vec<u8>,
    pub cur_time: i32,
    uspt: i32,
    next_delta: i32,
    state: i32,
    vol: i16,
    frame_time: i32,
    next_event: Event,
    evtq: EventQueue,
    /// 16 channels and the `__n_initFromBank` overrun sink at index 16
    /// (R1 §0.6): on the N64 that write lands in the first voice state's
    /// `N_ALVoice` (node links and `pvoice`), which is on the free list at
    /// the time and is rewritten when the voice state is next used.
    chan: [ChanState; MAX_CHANNELS + 1],
    voices: [VoiceState; MAX_VOICES],
    v_alloc_head: u8,
    v_alloc_tail: u8,
    v_free: u8,
    master_vol: u8,
    pub osc: OscPool,
    /// `D_8003D318`: the handler's return divisor.
    tempo_scale: f32,
    /// `D_8003D31C`: note-ons only play on channels whose bit is set.
    chan_mask: u16,
    /// Handler calls that found the event queue empty (the C would spin
    /// forever; never happens while the API event keeps itself queued).
    pub empty_queue: u32,
}

impl Csp {
    /// `n_alCSPNew` with the game's config (24 voices, 64 events, 16
    /// channels, `syAudioInitOsc` callbacks), then `n_alCSPSetBank` with
    /// the music bank (audio.c:964-972). Allocates every buffer the player
    /// will use, including the sequence buffer (the longest sequence,
    /// audio.c:813-828).
    pub fn new(syn: &mut Synth, data: &AudioData) -> Self {
        let max_len = data
            .seqs
            .iter()
            .map(|s| s.len as usize + 1)
            .max()
            .unwrap_or(0);
        let mut c = Self {
            banks: [false; 3],
            target: false,
            seq: CSeq::default(),
            seq_buf: vec![0; max_len],
            cur_time: 0,
            uspt: 488,
            next_delta: 0,
            state: AL_STOPPED,
            vol: 0x7FFF,
            frame_time: AL_USEC_PER_FRAME,
            next_event: Event::SeqpApi,
            evtq: EventQueue::new(MAX_EVENTS),
            chan: [ChanState::default(); MAX_CHANNELS + 1],
            voices: [VoiceState::default(); MAX_VOICES],
            v_alloc_head: NIL,
            v_alloc_tail: NIL,
            v_free: NIL,
            master_vol: 0,
            osc: OscPool::new(),
            tempo_scale: 1.0,
            chan_mask: 0xFFFF,
            empty_queue: 0,
        };
        c.init_chan_state();
        // `vs->next = vFreeList; vFreeList = vs`: the last state heads it.
        for i in 0..MAX_VOICES as u8 {
            c.voices[i as usize].next = c.v_free;
            c.v_free = i;
        }
        syn.add_client(Client::Seq1);
        c.evtq.post_event(Event::SeqpBank(0), 0);
        c
    }

    // --- API (n_csp*.c, syAudio) -------------------------------------------

    /// `seqp->state`.
    pub fn state(&self) -> i32 {
        self.state
    }

    /// `syAudioReadRom` of sequence `id` into the player's buffer
    /// (audio.c:1113). The bytes past the sequence keep the previous
    /// sequence's tail, as on the N64.
    pub fn load_sequence(&mut self, data: &AudioData, id: usize) {
        let Some(e) = data.seqs.get(id) else { return };
        let start = e.offset as usize;
        let len = (e.len as usize).min(self.seq_buf.len());
        let src = data.sbk.get(start..start + len).unwrap_or(&[]);
        self.seq_buf[..src.len()].copy_from_slice(src);
    }

    /// `n_alCSeqNew` over the loaded sequence, then `n_alCSPSetSeq`.
    pub fn set_sequence(&mut self) {
        self.seq = CSeq::new(&self.seq_buf);
        self.evtq.post_event(Event::SeqpSeq, 0);
    }

    /// `n_alCSPPlay`.
    pub fn play(&mut self) {
        self.evtq.post_event(Event::SeqpPlay, 0);
    }

    /// `n_alCSPStop`.
    pub fn stop(&mut self) {
        self.evtq.post_event(Event::SeqpStopping, 0);
    }

    /// `n_alCSPSetVol`.
    pub fn set_vol(&mut self, vol: i16) {
        self.evtq.post_event(Event::SeqpVol(vol), 0);
    }

    /// `n_alCSPSetChlPriority`.
    pub fn set_chl_priority(&mut self, chan: u8, priority: u8) {
        self.evtq
            .post_event(Event::SeqpPriority { chan, priority }, 0);
    }

    /// `n_alCSPSetChlFXMix`: a control change 0x5B through the queue.
    pub fn set_chl_fx_mix(&mut self, chan: u8, fxmix: u8) {
        let m = MidiEvent {
            ticks: 0,
            status: AL_MIDI_CONTROL_CHANGE | chan,
            byte1: AL_MIDI_FX1_CTRL,
            byte2: fxmix,
            duration: 0,
        };
        self.evtq.post_event(Event::SeqpMidi(m), 0);
    }

    /// `func_800290AC`: the tempo scale, 1.0 unless in (0, 10].
    pub fn set_tempo_scale(&mut self, scale: f32) {
        self.tempo_scale = if scale <= 0.0 || scale > 10.0 {
            1.0
        } else {
            scale
        };
    }

    /// `func_8002909C`: the note-on channel mask.
    pub fn set_channel_mask(&mut self, mask: u16) {
        self.chan_mask = mask;
    }

    /// Voice states in use (diagnostic).
    pub fn voices_in_use(&self) -> usize {
        let mut n = 0;
        let mut at = self.v_alloc_head;
        while at != NIL {
            n += 1;
            at = self.voices[at as usize].next;
        }
        n
    }

    /// Events dropped because the queue was full (diagnostic).
    pub fn events_dropped(&self) -> u32 {
        self.evtq.dropped
    }

    /// The channel states (diagnostic and tests).
    pub fn chan_state(&self, chan: usize) -> &ChanState {
        &self.chan[chan]
    }

    /// `uspt` (tests).
    pub fn uspt(&self) -> i32 {
        self.uspt
    }

    // --- The voice handler ---------------------------------------------------

    /// `__n_CSPVoiceHandler`: handles `nextEvent` and every following event
    /// due at the same time, then returns the microseconds to the next one
    /// divided by the tempo scale.
    pub fn handler(&mut self, syn: &mut Synth, data: &AudioData) -> i32 {
        loop {
            match self.next_event {
                Event::SeqRef => self.handle_next_seq_event(syn, data),
                Event::SeqpApi => self.evtq.post_event(Event::SeqpApi, self.frame_time),
                Event::NoteEnd { voice } => {
                    let vv = CSP_VOICE_BASE + voice as usize;
                    syn.stop_voice(vv);
                    syn.free_voice(vv);
                    if self.voices[voice as usize].flags != 0 {
                        self.stop_osc(voice);
                    }
                    self.unmap_voice(voice);
                }
                Event::Env { voice, delta, vol } => {
                    let vs = &mut self.voices[voice as usize];
                    if vs.env_phase == AL_PHASE_ATTACK {
                        vs.env_phase = AL_PHASE_DECAY;
                    }
                    vs.env_end_time = self.cur_time.wrapping_add(delta);
                    vs.env_gain = vol;
                    let v = self.vs_vol(voice, data);
                    syn.set_vol(CSP_VOICE_BASE + voice as usize, v, delta);
                }
                Event::TremOsc { vs, osc } => {
                    let (delta, value) = self.osc.update(osc);
                    if let Some(v) = value {
                        self.voices[vs as usize].tremelo = v as u8;
                    }
                    let vol = self.vs_vol(vs, data);
                    let d = self.vs_delta(vs);
                    syn.set_vol(CSP_VOICE_BASE + vs as usize, vol, d);
                    self.evtq.post_event(Event::TremOsc { vs, osc }, delta);
                }
                Event::VibOsc { vs, osc, chan } => {
                    let (delta, value) = self.osc.update(osc);
                    if let Some(v) = value {
                        self.voices[vs as usize].vibrato = v;
                    }
                    let v = &self.voices[vs as usize];
                    let c = &self.chan[chan as usize];
                    let pitch = (v.pitch
                        * ((((v.vibrato - 1.0) * c.pressure as f32) / 127.0) + 1.0))
                        * c.pitch_bend;
                    syn.set_pitch(CSP_VOICE_BASE + vs as usize, pitch);
                    self.evtq.post_event(Event::VibOsc { vs, osc, chan }, delta);
                }
                Event::SeqpMidi(m) | Event::CspNoteOff(m) => self.handle_midi(syn, data, &m),
                Event::SeqpVol(vol) => {
                    self.vol = vol;
                    let mut at = self.v_alloc_head;
                    while at != NIL {
                        let v = self.vs_vol(at, data);
                        let d = self.vs_delta(at);
                        syn.set_vol(CSP_VOICE_BASE + at as usize, v, d);
                        at = self.voices[at as usize].next;
                    }
                }
                Event::SeqpPlay => {
                    self.master_vol = 0x64;
                    if self.state != AL_PLAYING {
                        self.state = AL_PLAYING;
                        self.post_next_seq_event();
                    }
                }
                Event::SeqpStop => {
                    if self.state == AL_STOPPING {
                        while self.v_alloc_head != NIL {
                            let vs = self.v_alloc_head;
                            let vv = CSP_VOICE_BASE + vs as usize;
                            syn.stop_voice(vv);
                            syn.free_voice(vv);
                            if self.voices[vs as usize].flags != 0 {
                                self.stop_osc(vs);
                            }
                            self.unmap_voice(vs);
                        }
                        self.init_chan_state();
                        self.state = AL_STOPPED;
                    }
                }
                Event::SeqpStopping => {
                    if self.state == AL_PLAYING {
                        self.evtq.flush_type(AL_SEQ_REF_EVT);
                        self.evtq.flush_type(AL_CSP_NOTEOFF_EVT);
                        self.evtq.flush_type(AL_SEQP_MIDI_EVT);
                        let mut at = self.v_alloc_head;
                        while at != NIL {
                            if self.voice_needs_note_kill(at, KILL_TIME) {
                                self.release_voice(syn, at, KILL_TIME);
                            }
                            at = self.voices[at as usize].next;
                        }
                        self.state = AL_STOPPING;
                        self.evtq.post_event(Event::SeqpStop, AL_EVTQ_END);
                    }
                }
                Event::SeqpPriority { chan, priority } => {
                    self.chan[chan as usize].priority = priority;
                }
                Event::SeqpSeq => {
                    self.target = true;
                    self.set_uspt_from_tempo(500_000.0);
                    if self.banks[0] {
                        self.init_from_bank(data);
                    }
                }
                Event::SeqpBank(_) => {
                    self.banks[0] = true;
                    self.init_from_bank(data);
                }
                Event::AltBank { slot, .. } => {
                    // `AL_SEQ_END_EVT + 20 / + 21`: never posted by SSB.
                    self.banks[slot as usize] = true;
                    self.init_from_bank(data);
                }
                // AL_SEQ_END_EVT, AL_TEMPO_EVT, AL_SEQ_MIDI_EVT and the rest:
                // nothing.
                _ => {}
            }
            let (e, d) = self.evtq.next_event();
            self.next_event = e;
            self.next_delta = d;
            if matches!(e, Event::Empty) {
                // The C loops forever on an empty queue; the API event that
                // re-posts itself every frame keeps that from happening
                // unless the queue overflowed. Idle for a frame instead.
                self.empty_queue += 1;
                self.next_event = Event::SeqpApi;
                self.next_delta = self.frame_time;
            }
            if self.next_delta != 0 {
                break;
            }
        }
        self.cur_time = self.cur_time.wrapping_add(self.next_delta);
        (self.next_delta as f32 / self.tempo_scale) as i32
    }

    /// `__n_CSPHandleNextSeqEvent`.
    fn handle_next_seq_event(&mut self, syn: &mut Synth, data: &AudioData) {
        if !self.target {
            return;
        }
        let evt = self.seq.next_event(&mut self.seq_buf);
        match evt {
            Event::SeqMidi(m) => {
                self.handle_midi(syn, data, &m);
                self.post_next_seq_event();
            }
            Event::Tempo(t) => {
                self.handle_meta(&t);
                self.post_next_seq_event();
            }
            Event::SeqEnd => {
                self.state = AL_STOPPING;
                self.evtq.post_event(Event::SeqpStop, AL_EVTQ_END);
            }
            Event::TrackEnd | Event::LoopStart | Event::LoopEnd => self.post_next_seq_event(),
            _ => {}
        }
    }

    /// `__n_CSPPostNextSeqEvent`.
    fn post_next_seq_event(&mut self) {
        if self.state != AL_PLAYING || !self.target {
            return;
        }
        let Some(ticks) = self.seq.next_delta() else {
            return;
        };
        self.evtq
            .post_event(Event::SeqRef, ticks.wrapping_mul(self.uspt));
    }

    /// `__n_setUsptFromTempo`.
    fn set_uspt_from_tempo(&mut self, tempo: f32) {
        self.uspt = if self.target {
            (tempo * self.seq.qnpt) as i32
        } else {
            488
        };
    }

    /// `__n_CSPHandleMetaMsg`: a tempo change rescales every pending note
    /// off from the old `uspt` to the new one and reposts it.
    fn handle_meta(&mut self, t: &TempoEvent) {
        if t.status != AL_MIDI_META || t.kind != AL_MIDI_META_TEMPO {
            return;
        }
        let old_uspt = self.uspt;
        let tempo = ((t.byte1 as i32) << 16) | ((t.byte2 as i32) << 8) | t.byte3 as i32;
        self.set_uspt_from_tempo(tempo as f32);

        // Pull the note offs out into a temporary list. `alLink(this,
        // firstTemp)` inserts after the first, so the list ends up as the
        // first one followed by the rest in reverse.
        let mut first_temp = EVT_NIL;
        let mut cur_delta: i32 = 0;
        let mut this = self.evtq.first();
        while this != EVT_NIL {
            let item = *self.evtq.item(this);
            cur_delta = cur_delta.wrapping_add(item.delta);
            let next = item.next;
            if item.evt.ty() == AL_CSP_NOTEOFF_EVT {
                self.evtq.unlink(this);
                if first_temp != EVT_NIL {
                    self.evtq.link(this, first_temp);
                } else {
                    let it = self.evtq.item_mut(this);
                    it.next = EVT_NIL;
                    it.prev = EVT_NIL;
                    first_temp = this;
                }
                let temp_delta = cur_delta;
                if next != EVT_NIL {
                    cur_delta = cur_delta.wrapping_sub(item.delta);
                    let n = self.evtq.item_mut(next);
                    n.delta = n.delta.wrapping_add(item.delta);
                }
                self.evtq.item_mut(this).delta = temp_delta;
            }
            this = next;
        }
        let mut this = first_temp;
        while this != EVT_NIL {
            let next = self.evtq.item(this).next;
            let d = self.evtq.item(this).delta;
            // `ticks` is a u32 holding the signed quotient.
            let ticks = if old_uspt != 0 {
                d.wrapping_div(old_uspt) as u32
            } else {
                0
            };
            self.evtq.item_mut(this).delta = ticks.wrapping_mul(self.uspt as u32) as i32;
            self.evtq.repost(this);
            this = next;
        }
    }

    /// `__n_CSPHandleMIDIMsg`.
    fn handle_midi(&mut self, syn: &mut Synth, data: &AudioData, midi: &MidiEvent) {
        let bank = &data.music;
        let status = midi.status & 0xF0;
        let chan = midi.status & 0x0F;
        let ch = chan as usize;
        let key = midi.byte1;
        let byte1 = midi.byte1;
        let vel = midi.byte2;
        let byte2 = midi.byte2;
        match status {
            AL_MIDI_NOTE_ON if vel != 0 => {
                if self.state != AL_PLAYING {
                    return;
                }
                if self.chan_mask & (1 << chan) == 0 {
                    return;
                }
                let Some(sound) = self.lookup_sound_quick(data, key, vel, chan) else {
                    return;
                };
                let config = VoiceConfig {
                    priority: self.chan[ch].priority as i16,
                    fx_bus: 0,
                    unity_pitch: 0,
                };
                let Some(vi) = self.map_voice(key, vel, chan) else {
                    return;
                };
                let vv = CSP_VOICE_BASE + vi as usize;
                // The result is ignored: a voice that found no pvoice keeps
                // its state and events and simply makes no sound.
                syn.alloc_voice(vv, &config);
                let snd = &bank.sounds[sound as usize];
                let env = &bank.envelopes[snd.envelope as usize];
                let km = &bank.keymaps[snd.keymap as usize];
                let sustain = self.chan[ch].sustain;
                let cur_time = self.cur_time;
                {
                    let vs = &mut self.voices[vi as usize];
                    vs.sound = sound;
                    vs.env_phase = AL_PHASE_ATTACK;
                    vs.phase = if sustain > AL_SUSTAIN {
                        AL_PHASE_SUSTAIN
                    } else {
                        AL_PHASE_NOTEON
                    };
                    let cents = ((key as i32 - km.key_base as i32) * 100 + km.detune as i32) as i16;
                    vs.pitch = cents2ratio(cents as i32);
                    vs.env_gain = env.attack_volume;
                    vs.env_end_time = cur_time.wrapping_add(env.attack_time);
                    vs.flags = 0;
                }

                // Tremolo and vibrato. A zero delay makes `syAudioInitOsc`
                // return 0 ("don't run the osc") after it has taken a state
                // from the pool; no event is posted, so nothing ever returns
                // it. Music instruments 2 and 3 have a vibrato with delay 0:
                // each of their notes leaks one of the 32 states, and once
                // the pool is empty no note gets vibrato until the audio
                // system is rebuilt. Ported as shipped.
                let inst = match self.chan[ch].instrument {
                    Some(i) => bank.instruments[i as usize].clone_osc(),
                    None => OscParams::default(),
                };
                let mut osc_value = AL_VOL_FULL as f32;
                if inst.trem_type != 0 {
                    let r = self.osc.init(
                        inst.trem_type,
                        inst.trem_rate,
                        inst.trem_depth,
                        inst.trem_delay,
                    );
                    if let Some(v) = r.value {
                        osc_value = v;
                    }
                    if r.delta != 0 {
                        if let Some(osc) = r.state {
                            self.evtq
                                .post_event(Event::TremOsc { vs: vi, osc }, r.delta);
                            self.voices[vi as usize].flags |= 0x01;
                        }
                    }
                }
                self.voices[vi as usize].tremelo = osc_value as u8;
                let mut osc_value = 1.0f32;
                if inst.vib_type != 0 {
                    let r =
                        self.osc
                            .init(inst.vib_type, inst.vib_rate, inst.vib_depth, inst.vib_delay);
                    if let Some(v) = r.value {
                        osc_value = v;
                    }
                    if r.delta != 0 {
                        if let Some(osc) = r.state {
                            self.evtq
                                .post_event(Event::VibOsc { vs: vi, osc, chan }, r.delta);
                            self.voices[vi as usize].flags |= 0x02;
                        }
                    }
                }
                self.voices[vi as usize].vibrato = osc_value;

                let c = self.chan[ch];
                let vs = self.voices[vi as usize];
                let pitch = vs.pitch * c.pitch_bend * vs.vibrato;
                let pan = self.vs_pan(vi, data);
                let vol = self.vs_vol(vi, data);
                let delta = env.attack_time;
                let wave = WaveRef {
                    bank: 0,
                    index: snd.wavetable,
                };
                syn.start_voice_params(vv, wave, pitch, vol, pan, c.fxmix, delta, c.unk12, c.unk13);
                self.evtq.post_event(
                    Event::Env {
                        voice: vi,
                        delta: env.decay_time,
                        vol: env.decay_volume,
                    },
                    delta,
                );
                if midi.duration != 0 {
                    let off = MidiEvent {
                        ticks: 0,
                        status: chan | AL_MIDI_NOTE_OFF,
                        byte1: key,
                        byte2: 0,
                        duration: 0,
                    };
                    let d = (self.uspt as u32).wrapping_mul(midi.duration) as i32;
                    self.evtq.post_event(Event::CspNoteOff(off), d);
                }
            }
            // A note on with velocity 0 falls through to note off.
            AL_MIDI_NOTE_ON | AL_MIDI_NOTE_OFF => {
                let Some(vi) = self.lookup_voice(key, chan) else {
                    return;
                };
                if self.voices[vi as usize].phase == AL_PHASE_SUSTAIN {
                    self.voices[vi as usize].phase = AL_PHASE_SUSTREL;
                } else {
                    self.voices[vi as usize].phase = AL_PHASE_RELEASE;
                    let rel = self.release_time(vi, data);
                    self.release_voice(syn, vi, rel);
                }
            }
            AL_MIDI_POLY_KEY_PRESSURE => {
                let Some(vi) = self.lookup_voice(key, chan) else {
                    return;
                };
                self.voices[vi as usize].velocity = byte2;
                let v = self.vs_vol(vi, data);
                let d = self.vs_delta(vi);
                syn.set_vol(CSP_VOICE_BASE + vi as usize, v, d);
            }
            AL_MIDI_CHANNEL_PRESSURE => {
                // SSB: the pressure scales the vibrato depth; the stock
                // per-voice volume update is commented out.
                self.chan[ch].pressure = key;
            }
            AL_MIDI_CONTROL_CHANGE => self.handle_control(syn, data, chan, byte1, byte2),
            AL_MIDI_PROGRAM_CHANGE => {
                // Bank slot `unk_0x14`; SSB only ever has slot 0.
                let slot = self.chan[ch].bank_slot as usize;
                if slot == 0 && self.banks[0] && (key as usize) < bank.inst_array.len() {
                    let inst = bank.inst_array[key as usize];
                    self.set_inst_chan_state_alt(data, inst, ch);
                }
            }
            AL_MIDI_PITCH_BEND_CHANGE => {
                let bend_val = ((byte2 as i32) << 7) + byte1 as i32 - 8192;
                let cents = (self.chan[ch].bend_range as i32 * bend_val) / 8192;
                let bend_ratio = cents2ratio(cents);
                self.chan[ch].pitch_bend = bend_ratio;
                let pressure = self.chan[ch].pressure as f32;
                let mut at = self.v_alloc_head;
                while at != NIL {
                    let vs = self.voices[at as usize];
                    if vs.channel == chan {
                        let p = (((pressure * (vs.vibrato - 1.0)) / 127.0) + 1.0)
                            * (vs.pitch * bend_ratio);
                        syn.set_pitch(CSP_VOICE_BASE + at as usize, p);
                    }
                    at = vs.next;
                }
            }
            _ => {}
        }
    }

    /// The control-change half of `__n_CSPHandleMIDIMsg`.
    fn handle_control(
        &mut self,
        syn: &mut Synth,
        data: &AudioData,
        chan: u8,
        byte1: u8,
        byte2: u8,
    ) {
        let ch = chan as usize;
        match byte1 {
            AL_MIDI_PAN_CTRL => {
                self.chan[ch].pan = byte2;
                let mut at = self.v_alloc_head;
                while at != NIL {
                    if self.voices[at as usize].channel == chan {
                        let pan = self.vs_pan(at, data);
                        syn.set_pan(CSP_VOICE_BASE + at as usize, pan);
                    }
                    at = self.voices[at as usize].next;
                }
            }
            AL_MIDI_VOLUME_CTRL => {
                self.chan[ch].vol = byte2;
                let mut at = self.v_alloc_head;
                while at != NIL {
                    let vs = self.voices[at as usize];
                    if vs.channel == chan && vs.env_phase != AL_PHASE_RELEASE {
                        let v = self.vs_vol(at, data);
                        let d = self.vs_delta(at);
                        syn.set_vol(CSP_VOICE_BASE + at as usize, v, d);
                    }
                    at = vs.next;
                }
            }
            AL_MIDI_PRIORITY_CTRL | AL_MIDI_FX_CTRL_5 => self.chan[ch].priority = byte2,
            AL_MIDI_SUSTAIN_CTRL => {
                self.chan[ch].sustain = byte2;
                let mut at = self.v_alloc_head;
                while at != NIL {
                    let vs = self.voices[at as usize];
                    if vs.channel == chan && vs.phase != AL_PHASE_RELEASE {
                        if byte2 > AL_SUSTAIN {
                            if vs.phase == AL_PHASE_NOTEON {
                                self.voices[at as usize].phase = AL_PHASE_SUSTAIN;
                            }
                        } else if vs.phase == AL_PHASE_SUSTAIN {
                            self.voices[at as usize].phase = AL_PHASE_NOTEON;
                        } else if vs.phase == AL_PHASE_SUSTREL {
                            self.voices[at as usize].phase = AL_PHASE_RELEASE;
                            let rel = self.release_time(at, data);
                            self.release_voice(syn, at, rel);
                        }
                    }
                    at = self.voices[at as usize].next;
                }
            }
            AL_MIDI_FX1_CTRL => {
                self.chan[ch].fxmix = byte2;
                let mut at = self.v_alloc_head;
                while at != NIL {
                    if self.voices[at as usize].channel == chan {
                        syn.set_fx_mix(CSP_VOICE_BASE + at as usize, byte2);
                    }
                    at = self.voices[at as usize].next;
                }
            }
            AL_MIDI_FX_CTRL_0 => {
                self.chan[ch].bend_range = if byte2 >= 0x79 {
                    0x4B0
                } else {
                    byte2 as i16 * 10
                };
            }
            AL_MIDI_FX_CTRL_1 => self.master_vol = byte2,
            AL_MIDI_FX_CTRL_2 | AL_MIDI_FX_CTRL_3 => {
                if byte1 == AL_MIDI_FX_CTRL_2 {
                    self.chan[ch].unk12 = byte2;
                } else {
                    self.chan[ch].unk13 = byte2;
                }
                let (a, b) = (self.chan[ch].unk12, self.chan[ch].unk13);
                let mut at = self.v_alloc_head;
                while at != NIL {
                    if self.voices[at as usize].channel == chan {
                        syn.set_fx_mix_alt(CSP_VOICE_BASE + at as usize, a, b);
                    }
                    at = self.voices[at as usize].next;
                }
            }
            AL_MIDI_FX_CTRL_4 if byte2 <= 2 && self.banks[byte2 as usize] => {
                self.chan[ch].bank_slot = byte2;
            }
            _ => {}
        }
    }

    // --- n_seqplayer.c helpers ------------------------------------------------

    /// The release time of the voice's sound's envelope.
    fn release_time(&self, vi: u8, data: &AudioData) -> i32 {
        let bank = &data.music;
        let snd = &bank.sounds[self.voices[vi as usize].sound as usize];
        bank.envelopes[snd.envelope as usize].release_time
    }

    /// `__n_vsVol`.
    fn vs_vol(&self, vi: u8, data: &AudioData) -> i16 {
        let vs = &self.voices[vi as usize];
        let snd = &data.music.sounds[vs.sound as usize];
        let t1 = ((vs.tremelo as u32) * (vs.velocity as u32) * (vs.env_gain as u32)) >> 6;
        let t2 = ((snd.sample_volume as i32)
            .wrapping_mul(self.get_vol() as i32)
            .wrapping_mul(self.get_chl_vol(vs.channel) as i32)
            >> 14) as u32;
        (t1.wrapping_mul(t2) >> 15) as i16
    }

    /// `n_alSeqpGetVol`.
    fn get_vol(&self) -> i16 {
        ((self.vol as i32 * self.master_vol as i32) >> 7) as i16
    }

    /// `n_alSeqpGetChlVol`.
    fn get_chl_vol(&self, chan: u8) -> u8 {
        let c = &self.chan[chan as usize];
        ((c.vol as i32 * c.vol2 as i32) / 127) as u8
    }

    /// `__n_vsPan`.
    fn vs_pan(&self, vi: u8, data: &AudioData) -> u8 {
        let vs = &self.voices[vi as usize];
        let snd = &data.music.sounds[vs.sound as usize];
        let tmp = self.chan[vs.channel as usize].pan as i32 - 64 + snd.sample_pan as i32;
        tmp.clamp(0, 127) as u8
    }

    /// `__n_vsDelta`.
    fn vs_delta(&self, vi: u8) -> i32 {
        let delta = self.voices[vi as usize]
            .env_end_time
            .wrapping_sub(self.cur_time);
        if delta >= 0 {
            delta
        } else {
            AL_GAIN_CHANGE_TIME
        }
    }

    /// `__n_seqpReleaseVoice`.
    fn release_voice(&mut self, syn: &mut Synth, vi: u8, delta_time: i32) {
        if self.voices[vi as usize].env_phase == AL_PHASE_ATTACK {
            let mut at = self.evtq.first();
            while at != EVT_NIL {
                let item = *self.evtq.item(at);
                if let Event::Env { voice, .. } = item.evt {
                    if voice == vi {
                        self.evtq.remove(at);
                    }
                }
                at = item.next;
            }
        }
        let vs = &mut self.voices[vi as usize];
        vs.velocity = 0;
        vs.env_phase = AL_PHASE_RELEASE;
        vs.env_gain = 0;
        vs.env_end_time = self.cur_time.wrapping_add(delta_time);
        let vv = CSP_VOICE_BASE + vi as usize;
        syn.set_priority(vv, 0);
        syn.set_vol(vv, 0, delta_time);
        self.evtq
            .post_event(Event::NoteEnd { voice: vi }, delta_time);
    }

    /// `__n_voiceNeedsNoteKill`: whether the voice's note end falls after
    /// `kill_time` (it is then removed) or is missing.
    fn voice_needs_note_kill(&mut self, vi: u8, kill_time: i32) -> bool {
        let mut item_time: i32 = 0;
        let mut at = self.evtq.first();
        while at != EVT_NIL {
            let item = *self.evtq.item(at);
            item_time = item_time.wrapping_add(item.delta);
            if let Event::NoteEnd { voice } = item.evt {
                if voice == vi {
                    if item_time > kill_time {
                        self.evtq.remove(at);
                        return true;
                    }
                    return false;
                }
            }
            at = item.next;
        }
        true
    }

    /// `__n_seqpStopOsc`: removes the voice's oscillator events and
    /// returns their states.
    fn stop_osc(&mut self, vi: u8) {
        let mut at = self.evtq.first();
        while at != EVT_NIL {
            let item = *self.evtq.item(at);
            let hit = match item.evt {
                Event::TremOsc { vs, osc } if vs == vi => Some((osc, 0xFE)),
                Event::VibOsc { vs, osc, .. } if vs == vi => Some((osc, 0xFD)),
                _ => None,
            };
            if let Some((osc, mask)) = hit {
                self.osc.stop(osc);
                self.evtq.remove(at);
                let vs = &mut self.voices[vi as usize];
                vs.flags &= mask;
                if vs.flags == 0 {
                    return;
                }
            }
            at = item.next;
        }
    }

    /// `__n_mapVoice`: the head of the free list, appended to the
    /// allocated list.
    fn map_voice(&mut self, key: u8, vel: u8, channel: u8) -> Option<u8> {
        let vi = self.v_free;
        if vi == NIL {
            return None;
        }
        self.v_free = self.voices[vi as usize].next;
        self.voices[vi as usize].next = NIL;
        if self.v_alloc_head == NIL {
            self.v_alloc_head = vi;
        } else {
            self.voices[self.v_alloc_tail as usize].next = vi;
        }
        self.v_alloc_tail = vi;
        let vs = &mut self.voices[vi as usize];
        vs.channel = channel;
        vs.key = key;
        vs.velocity = vel;
        Some(vi)
    }

    /// `__n_unmapVoice`.
    fn unmap_voice(&mut self, vi: u8) {
        let mut prev = NIL;
        let mut at = self.v_alloc_head;
        while at != NIL {
            if at == vi {
                let next = self.voices[at as usize].next;
                if prev != NIL {
                    self.voices[prev as usize].next = next;
                } else {
                    self.v_alloc_head = next;
                }
                if at == self.v_alloc_tail {
                    self.v_alloc_tail = prev;
                }
                self.voices[at as usize].next = self.v_free;
                self.v_free = at;
                return;
            }
            prev = at;
            at = self.voices[at as usize].next;
        }
    }

    /// `__n_lookupVoice`.
    fn lookup_voice(&self, key: u8, channel: u8) -> Option<u8> {
        let mut at = self.v_alloc_head;
        while at != NIL {
            let vs = &self.voices[at as usize];
            if vs.key == key
                && vs.channel == channel
                && vs.phase != AL_PHASE_RELEASE
                && vs.phase != AL_PHASE_SUSTREL
            {
                return Some(at);
            }
            at = vs.next;
        }
        None
    }

    /// `__n_lookupSoundQuick`: a binary search over the instrument's key
    /// maps. A NULL instrument (only before the first bank) finds nothing.
    fn lookup_sound_quick(&self, data: &AudioData, key: u8, vel: u8, chan: u8) -> Option<u16> {
        let bank = &data.music;
        let inst = &bank.instruments[self.chan[chan as usize].instrument? as usize];
        let mut l: i32 = 1;
        let mut r: i32 = inst.sounds.len() as i32;
        while r >= l {
            let i = (l + r) / 2;
            let s = inst.sounds[(i - 1) as usize];
            let km = &bank.keymaps[bank.sounds[s as usize].keymap as usize];
            if key >= km.key_min
                && key <= km.key_max
                && vel >= km.velocity_min
                && vel <= km.velocity_max
            {
                return Some(s);
            } else if key < km.key_min || (vel < km.velocity_min && key <= km.key_max) {
                r = i - 1;
            } else {
                l = i + 1;
            }
        }
        None
    }

    /// `__n_initChanState` (with SSB's pressure reset).
    fn init_chan_state(&mut self) {
        for i in 0..MAX_CHANNELS {
            self.chan[i].instrument = None;
            self.chan[i].pressure = 0;
            self.reset_perf_chan_state(i);
        }
    }

    /// `__n_resetPerfChanState`.
    fn reset_perf_chan_state(&mut self, chan: usize) {
        let c = &mut self.chan[chan];
        c.fx_id = 0;
        c.fxmix = 0;
        c.pan = 64;
        c.vol = 127;
        c.priority = 5;
        c.sustain = 0;
        c.bend_range = 200;
        c.pitch_bend = 1.0;
        c.unk12 = 0;
        c.unk13 = 95;
        c.bank_slot = 0;
    }

    /// `__n_initFromBank`: every channel gets the bank's first instrument
    /// (`instArray[0]`, the file header read as an instrument), and channel
    /// 9 the percussion. The percussion branch resets channel 16, one past
    /// the array: [`Csp::chan`]'s sink entry.
    fn init_from_bank(&mut self, data: &AudioData) {
        let bank = &data.music;
        let Some(&inst) = bank.inst_array.first() else {
            return;
        };
        for i in 0..MAX_CHANNELS {
            self.reset_perf_chan_state(i);
            self.set_inst_chan_state(data, inst, i);
        }
        if let Some(perc) = bank.percussion {
            self.reset_perf_chan_state(MAX_CHANNELS);
            self.set_inst_chan_state(data, perc, 9);
        }
    }

    /// `__n_setInstChanState`.
    fn set_inst_chan_state(&mut self, data: &AudioData, inst: u16, chan: usize) {
        let i = &data.music.instruments[inst as usize];
        let c = &mut self.chan[chan];
        c.instrument = Some(inst);
        c.pan = i.pan;
        c.vol = 127;
        c.priority = i.priority;
        c.bend_range = i.bend_range;
        c.vol2 = i.volume;
    }

    /// `__n_setInstChanState_Alt`: keeps the channel's pan and volume.
    fn set_inst_chan_state_alt(&mut self, data: &AudioData, inst: u16, chan: usize) {
        let i = &data.music.instruments[inst as usize];
        let c = &mut self.chan[chan];
        c.instrument = Some(inst);
        c.priority = i.priority;
        c.bend_range = i.bend_range;
        c.vol2 = i.volume;
    }
}

/// The oscillator fields of an `ALInstrument`.
#[derive(Debug, Clone, Copy, Default)]
struct OscParams {
    trem_type: u8,
    trem_rate: u8,
    trem_depth: u8,
    trem_delay: u8,
    vib_type: u8,
    vib_rate: u8,
    vib_depth: u8,
    vib_delay: u8,
}

trait CloneOsc {
    fn clone_osc(&self) -> OscParams;
}

impl CloneOsc for super::data::Instrument {
    fn clone_osc(&self) -> OscParams {
        OscParams {
            trem_type: self.trem_type,
            trem_rate: self.trem_rate,
            trem_depth: self.trem_depth,
            trem_delay: self.trem_delay,
            vib_type: self.vib_type,
            vib_rate: self.vib_rate,
            vib_depth: self.vib_depth,
            vib_delay: self.vib_delay,
        }
    }
}
