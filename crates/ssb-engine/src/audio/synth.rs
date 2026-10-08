//! libultra's `n_audio` synthesis driver (`n_env.c`'s n_synthesizer,
//! n_env, n_resample, n_load, n_auxbus, n_mainbus and n_save parts, and
//! the `n_syn*.c` setters), built with `N_MICRO` and `FINAL_ROUND`
//! (`SAMPLE_ROUND` is defined then undefined in `n_synthInternals.h`).
//!
//! Pointers become indices: physical voices `0..16`, the shared `ALParam`
//! pool `0..128`, and the virtual voices (`N_ALVoice`) the sequence player
//! and the FGM engine own, which live here so that stealing can clear the
//! loser's `pvoice` as `n_alSynAllocVoice` does.

use super::data::{AudioData, WaveRef, AL_ADPCM_WAVE};
use super::dsp::{self, Dmem, EnvMixInit, EnvMixState, State16};
use super::reverb::Fx;

pub const MAX_PVOICES: usize = 16;
pub const MAX_UPDATES: usize = 128;
/// Virtual voices: the sequence player's 24 (`N_ALVoiceState`), then the
/// FGM engine's 24 (`EE0C_2`).
pub const VVOICES: usize = 48;
pub const CSP_VOICE_BASE: usize = 0;
pub const FGM_VOICE_BASE: usize = 24;

/// `SAMPLES` / `FIXED_SAMPLE`.
pub const FIXED_SAMPLE: i32 = 184;
/// `UNITY_PITCH`, `MAX_RATIO` (`abi.h`).
const UNITY_PITCH: f32 = 32768.0;
const MAX_RATIO: f32 = 1.99996;
const ADPCMFSIZE: i32 = 16;
const ADPCMFBYTES: i32 = 9;
const LFSAMPLES: i32 = 4;

pub const AL_STOPPED: i32 = 0;
pub const AL_PLAYING: i32 = 1;
pub const AL_STOPPING: i32 = 2;

const NONE: u8 = 0xFF;

/// `SAMPLE184`.
#[inline]
pub fn sample184(delta: i32) -> i32 {
    ((delta + FIXED_SAMPLE / 2) / FIXED_SAMPLE) * FIXED_SAMPLE
}

/// `N_ALVoice`.
#[derive(Debug, Clone, Copy, Default)]
pub struct VVoice {
    pub priority: i16,
    pub unity_pitch: u8,
    pub fx_bus: u8,
    pub state: u8,
    pub pvoice: Option<u8>,
}

/// `ALVoiceConfig`.
#[derive(Debug, Clone, Copy, Default)]
pub struct VoiceConfig {
    pub priority: i16,
    pub fx_bus: u8,
    pub unity_pitch: u8,
}

/// An `ALParam` update and the variants that share its pool.
#[derive(Debug, Clone, Copy)]
enum ParamKind {
    /// `ALStartParamAlt` (`AL_FILTER_START_VOICE_ALT`).
    StartVoiceAlt {
        unity: u8,
        pan: u8,
        volume: i16,
        fx_mix: u8,
        unk1c: u8,
        unk1d: u8,
        pitch: f32,
        samples: i32,
        wave: WaveRef,
    },
    SetVolume {
        volume: i32,
        time: i32,
    },
    SetPan(i32),
    SetFxAmt(i32),
    SetFxAmtAlt {
        data: i32,
        more: i32,
    },
    StopVoice,
    /// `N_ALFreeParam`.
    FreeVoice(u8),
    SetPitch(f32),
}

#[derive(Debug, Clone, Copy)]
struct Param {
    next: u8,
    delta: i32,
    kind: ParamKind,
}

/// `ALRawLoop` as `N_PVoice::dc_loop` keeps it.
#[derive(Debug, Clone, Copy, Default)]
struct RawLoop {
    start: u32,
    end: u32,
    count: u32,
}

/// `N_PVoice`.
#[derive(Debug, Clone, Copy)]
pub struct PVoice {
    pub vvoice: Option<u8>,
    // ALLoadFilter
    dc_state: State16,
    dc_lstate: State16,
    dc_loop: RawLoop,
    dc_table: Option<WaveRef>,
    dc_sample: i32,
    dc_lastsam: i32,
    dc_first: i32,
    /// Byte offset into the bank's tbl.
    dc_memin: i32,
    // ALResampler
    rs_state: State16,
    rs_ratio: f32,
    rs_upitch: i32,
    rs_delta: f32,
    rs_first: i32,
    // ALEnvMixer
    em_state: EnvMixState,
    em_pan: i16,
    em_volume: i16,
    em_cvol_l: i16,
    em_cvol_r: i16,
    em_dryamt: i16,
    em_wetamt: i16,
    em_lratl: u16,
    em_lratm: i16,
    em_ltgt: i16,
    em_rratl: u16,
    em_rratm: i16,
    em_rtgt: i16,
    em_delta: i32,
    em_seg_end: i32,
    em_first: i32,
    em_ctrl_list: u8,
    em_ctrl_tail: u8,
    pub em_motion: i32,
    pub offset: i32,
}

impl PVoice {
    /// `alN_PVoiceNew`.
    fn new() -> Self {
        Self {
            vvoice: None,
            dc_state: [0; 16],
            dc_lstate: [0; 16],
            dc_loop: RawLoop::default(),
            dc_table: None,
            dc_sample: 0,
            dc_lastsam: 0,
            dc_first: 1,
            dc_memin: 0,
            rs_state: [0; 16],
            rs_ratio: 1.0,
            rs_upitch: 0,
            rs_delta: 0.0,
            rs_first: 1,
            em_state: EnvMixState::default(),
            em_pan: 0,
            em_volume: 1,
            em_cvol_l: 1,
            em_cvol_r: 1,
            em_dryamt: 0,
            em_wetamt: 0,
            em_lratl: 0,
            em_lratm: 1,
            em_ltgt: 1,
            em_rratl: 0,
            em_rratm: 0,
            em_rtgt: 1,
            em_delta: 0,
            em_seg_end: 0,
            em_first: 1,
            em_ctrl_list: NONE,
            em_ctrl_tail: NONE,
            em_motion: AL_STOPPED,
            offset: 0,
        }
    }
}

/// `ALLink` nodes: the 16 pvoices, then the three list heads.
#[derive(Debug, Clone, Copy)]
struct Link {
    next: u8,
    prev: u8,
}

const FREE_LIST: u8 = 16;
const LAME_LIST: u8 = 17;
const ALLOC_LIST: u8 = 18;

/// A synthesizer client (`ALPlayer`): `n_syn->n_sndp`, `n_seqp1`,
/// `n_seqp2`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Client {
    Snd = 0,
    Seq1 = 1,
    Seq2 = 2,
}

/// The players whose handlers `n_alAudioFrame` calls.
pub trait Clients {
    /// The client's `handler`: runs it, returns the microseconds until it
    /// wants to run again.
    fn handle(&mut self, client: Client, syn: &mut Synth, data: &AudioData) -> i32;
}

/// `N_ALSynth`.
pub struct Synth {
    pub pvoices: [PVoice; MAX_PVOICES],
    links: [Link; 19],
    params: [Param; MAX_UPDATES],
    param_list: u8,
    pub vvoices: [VVoice; VVOICES],
    pub cur_samples: i32,
    pub param_samples: i32,
    pub output_rate: i32,
    /// `samplesLeft` of each registered client.
    samples_left: [Option<i32>; 3],
    /// `n_syn->auxBus->fx`; `None` is `AL_FX_NONE`.
    pub fx: Option<Fx>,
    dmem: Dmem,
    /// `ALParam` allocations that found the pool empty.
    pub params_dropped: u32,
}

impl Synth {
    /// `n_alSynNew`: 16 pvoices on the free list, a LIFO pool of 128
    /// updates.
    pub fn new(output_rate: i32, fx: Option<Fx>) -> Self {
        let mut s = Self {
            pvoices: [PVoice::new(); MAX_PVOICES],
            links: [Link {
                next: NONE,
                prev: NONE,
            }; 19],
            params: [Param {
                next: NONE,
                delta: 0,
                kind: ParamKind::StopVoice,
            }; MAX_UPDATES],
            param_list: NONE,
            vvoices: [VVoice::default(); VVOICES],
            cur_samples: 0,
            param_samples: 0,
            output_rate,
            samples_left: [None; 3],
            fx,
            dmem: Dmem::default(),
            params_dropped: 0,
        };
        for i in 0..MAX_PVOICES as u8 {
            s.link(i, FREE_LIST);
        }
        for i in 0..MAX_UPDATES as u8 {
            s.params[i as usize].next = s.param_list;
            s.param_list = i;
        }
        s
    }

    /// `alLink`: insert `ln` after `to`.
    fn link(&mut self, ln: u8, to: u8) {
        let next = self.links[to as usize].next;
        self.links[ln as usize] = Link { next, prev: to };
        if next != NONE {
            self.links[next as usize].prev = ln;
        }
        self.links[to as usize].next = ln;
    }

    /// `alUnlink`.
    fn unlink(&mut self, ln: u8) {
        let Link { next, prev } = self.links[ln as usize];
        if next != NONE {
            self.links[next as usize].prev = prev;
        }
        if prev != NONE {
            self.links[prev as usize].next = next;
        }
    }

    /// The pvoices on the allocated list, head first.
    pub fn allocated(&self) -> impl Iterator<Item = u8> + '_ {
        let mut at = self.links[ALLOC_LIST as usize].next;
        core::iter::from_fn(move || {
            (at != NONE).then(|| {
                let v = at;
                at = self.links[v as usize].next;
                v
            })
        })
    }

    /// Physical voices that are playing.
    pub fn active_voices(&self) -> usize {
        self.pvoices
            .iter()
            .filter(|p| p.em_motion == AL_PLAYING)
            .count()
    }

    /// `n_alSynAddSndPlayer` / `n_alSynAddSeqPlayer`.
    pub fn add_client(&mut self, client: Client) {
        self.samples_left[client as usize] = Some(self.cur_samples);
    }

    /// `_n_timeToSamplesNoRound`.
    pub fn time_to_samples_no_round(&self, micros: i32) -> i32 {
        (micros as f32 * self.output_rate as f32 / 1_000_000.0f32 + 0.5f32) as i32
    }

    /// `_n_timeToSamples`.
    pub fn time_to_samples(&self, micros: i32) -> i32 {
        self.time_to_samples_no_round(micros) & !0xF
    }

    /// `__n_nextSampleTime`: the earliest client, `n_sndp` first on ties.
    fn next_sample_time(&self) -> Option<(Client, i32)> {
        let mut delta = i32::MAX;
        let mut best = None;
        for c in [Client::Snd, Client::Seq1, Client::Seq2] {
            if let Some(left) = self.samples_left[c as usize] {
                let t = left.wrapping_sub(self.cur_samples);
                if t < delta {
                    delta = t;
                    best = Some((c, left));
                }
            }
        }
        best
    }

    // --- n_alAudioFrame ---------------------------------------------------

    /// `n_alAudioFrame`: runs every client whose next event falls in this
    /// frame, then renders `out_len` stereo samples into `out` in
    /// 184-sample sub-frames.
    pub fn audio_frame(
        &mut self,
        data: &AudioData,
        clients: &mut impl Clients,
        out: &mut [i16],
        out_len: usize,
    ) {
        if self.samples_left.iter().all(Option::is_none) {
            out[..out_len * 2].fill(0);
            return;
        }
        let len = out_len as i32;
        while let Some((client, at)) = self.next_sample_time() {
            self.param_samples = at;
            if at.wrapping_sub(self.cur_samples) >= len {
                break;
            }
            self.param_samples &= !0xF;
            let micros = clients.handle(client, self, data);
            let add = self.time_to_samples_no_round(micros);
            if let Some(l) = &mut self.samples_left[client as usize] {
                *l = l.wrapping_add(add);
            }
        }
        self.param_samples &= !0xF;

        for chunk in out[..out_len * 2].chunks_mut(FIXED_SAMPLE as usize * 2) {
            let n_out = chunk.len() as i32 / 2;
            self.save_pull(data, chunk);
            if self.cur_samples < 0x7FFF_FF47 {
                self.cur_samples += n_out;
            } else {
                self.cur_samples = 0x8000_0090u32 as i32;
            }
        }
        self.collect_pvoices();
    }

    /// `n_alSavePull`: main bus, interleave, save.
    fn save_pull(&mut self, data: &AudioData, out: &mut [i16]) {
        let offset = self.cur_samples;
        self.main_bus_pull(data, offset);
        self.dmem.interleave();
        let n = out.len();
        self.dmem.save(dsp::TEMP_0, out);
        debug_assert_eq!(n, dsp::COUNT);
    }

    /// `n_alMainBusPull`.
    fn main_bus_pull(&mut self, data: &AudioData, offset: i32) {
        self.dmem.clear(dsp::MAIN_L, dsp::DIVIDED << 1);
        if self.fx.is_some() {
            self.fx_pull(data, offset);
        } else {
            self.aux_bus_pull(data, offset);
        }
        self.dmem.mix(0x7FFF, dsp::AUX_L, dsp::MAIN_L);
        self.dmem.mix(0x7FFF, dsp::AUX_R, dsp::MAIN_R);
    }

    /// `n_alFxPull`.
    fn fx_pull(&mut self, data: &AudioData, offset: i32) {
        self.aux_bus_pull(data, offset);
        if let Some(fx) = &mut self.fx {
            fx.pull(&mut self.dmem, &data.resample);
        }
    }

    /// `n_alAuxBusPull`: every pvoice, in creation order.
    fn aux_bus_pull(&mut self, data: &AudioData, offset: i32) {
        self.dmem.clear(dsp::AUX_L, dsp::DIVIDED << 1);
        for i in 0..MAX_PVOICES {
            self.envmixer_pull(data, i, offset);
        }
    }

    // --- params -----------------------------------------------------------

    /// `__n_allocParam`.
    fn alloc_param(&mut self, delta: i32, kind: ParamKind) -> Option<u8> {
        let p = self.param_list;
        if p == NONE {
            self.params_dropped += 1;
            return None;
        }
        self.param_list = self.params[p as usize].next;
        self.params[p as usize] = Param {
            next: NONE,
            delta,
            kind,
        };
        Some(p)
    }

    /// `_n_freeParam`.
    fn free_param(&mut self, p: u8) {
        self.params[p as usize].next = self.param_list;
        self.param_list = p;
    }

    /// `n_alEnvmixerParam(AL_FILTER_ADD_UPDATE)`.
    fn add_update(&mut self, pv: u8, p: u8) {
        let v = &mut self.pvoices[pv as usize];
        if v.em_ctrl_tail != NONE {
            self.params[v.em_ctrl_tail as usize].next = p;
        } else {
            v.em_ctrl_list = p;
        }
        v.em_ctrl_tail = p;
    }

    /// Posts an update for `vv`'s pvoice at `paramSamples + offset`.
    fn post(&mut self, vv: usize, kind: ParamKind) {
        if let Some(pv) = self.vvoices[vv].pvoice {
            let delta = self.param_samples + self.pvoices[pv as usize].offset;
            if let Some(p) = self.alloc_param(delta, kind) {
                self.add_update(pv, p);
            }
        }
    }

    /// `_n_collectPVoices`: lame list to free list.
    fn collect_pvoices(&mut self) {
        loop {
            let dl = self.links[LAME_LIST as usize].next;
            if dl == NONE {
                break;
            }
            self.unlink(dl);
            self.link(dl, FREE_LIST);
        }
    }

    /// `_n_freePVoice`: allocated list to lame list.
    fn free_pvoice(&mut self, pv: u8) {
        self.unlink(pv);
        self.link(pv, LAME_LIST);
    }

    // --- n_syn* -----------------------------------------------------------

    /// `n_alSynAllocVoice` and `_allocatePVoice`: lame list, free list,
    /// then steal the last allocated voice with the lowest priority not
    /// above `priority` that is not already being stolen.
    pub fn alloc_voice(&mut self, vv: usize, vc: &VoiceConfig) -> bool {
        let v = &mut self.vvoices[vv];
        v.priority = vc.priority;
        v.unity_pitch = vc.unity_pitch;
        v.fx_bus = vc.fx_bus;
        v.state = AL_STOPPED as u8;
        v.pvoice = None;

        let mut stolen = false;
        let mut pvoice = None;
        for list in [LAME_LIST, FREE_LIST] {
            let dl = self.links[list as usize].next;
            if dl != NONE {
                self.unlink(dl);
                self.link(dl, ALLOC_LIST);
                pvoice = Some(dl);
                break;
            }
        }
        if pvoice.is_none() {
            let mut priority = vc.priority;
            let mut at = self.links[ALLOC_LIST as usize].next;
            while at != NONE {
                let pv = &self.pvoices[at as usize];
                if let Some(owner) = pv.vvoice {
                    let p = self.vvoices[owner as usize].priority;
                    if p <= priority && pv.offset == 0 {
                        pvoice = Some(at);
                        priority = p;
                        stolen = true;
                    }
                }
                at = self.links[at as usize].next;
            }
        }
        let Some(pv) = pvoice else {
            return false;
        };
        if stolen {
            self.pvoices[pv as usize].offset = 512;
            if let Some(old) = self.pvoices[pv as usize].vvoice {
                self.vvoices[old as usize].pvoice = None;
            }
            self.pvoices[pv as usize].vvoice = Some(vv as u8);
            self.vvoices[vv].pvoice = Some(pv);
            let ps = self.param_samples;
            if let Some(p) = self.alloc_param(
                ps,
                ParamKind::SetVolume {
                    volume: 0,
                    time: 512 - 64,
                },
            ) {
                self.add_update(pv, p);
            }
            if let Some(p) = self.alloc_param(ps + 512, ParamKind::StopVoice) {
                self.add_update(pv, p);
            }
        } else {
            self.pvoices[pv as usize].offset = 0;
            self.pvoices[pv as usize].vvoice = Some(vv as u8);
            self.vvoices[vv].pvoice = Some(pv);
        }
        true
    }

    /// `n_alSynFreeVoice`.
    pub fn free_voice(&mut self, vv: usize) {
        let Some(pv) = self.vvoices[vv].pvoice else {
            return;
        };
        if self.pvoices[pv as usize].offset != 0 {
            let delta = self.param_samples + self.pvoices[pv as usize].offset;
            if let Some(p) = self.alloc_param(delta, ParamKind::FreeVoice(pv)) {
                self.add_update(pv, p);
            }
        } else {
            self.free_pvoice(pv);
        }
        self.vvoices[vv].pvoice = None;
    }

    /// `n_alSynSetVol`: `t` in microseconds.
    pub fn set_vol(&mut self, vv: usize, volume: i16, t: i32) {
        let time = self.time_to_samples(t);
        self.post(
            vv,
            ParamKind::SetVolume {
                volume: volume as i32,
                time,
            },
        );
    }

    /// `n_alSynSetPan`.
    pub fn set_pan(&mut self, vv: usize, pan: u8) {
        self.post(vv, ParamKind::SetPan(pan as i32));
    }

    /// `n_alSynSetPitch`.
    pub fn set_pitch(&mut self, vv: usize, pitch: f32) {
        self.post(vv, ParamKind::SetPitch(pitch));
    }

    /// `n_alSynSetFXMix`.
    pub fn set_fx_mix(&mut self, vv: usize, fxmix: u8) {
        self.post(vv, ParamKind::SetFxAmt(fxmix as i32));
    }

    /// `n_alSynSetFXMix_Alt`.
    pub fn set_fx_mix_alt(&mut self, vv: usize, fxmix: u8, arg2: u8) {
        self.post(
            vv,
            ParamKind::SetFxAmtAlt {
                data: fxmix as i32,
                more: arg2 as i32,
            },
        );
    }

    /// `n_alSynSetPriority`.
    pub fn set_priority(&mut self, vv: usize, priority: i16) {
        self.vvoices[vv].priority = priority;
    }

    /// `n_alSynStopVoice`.
    pub fn stop_voice(&mut self, vv: usize) {
        self.post(vv, ParamKind::StopVoice);
    }

    /// `n_alSynStartVoiceParams` (`unk1C` 0, `unk1D` 0x5F) and `_Alt`.
    #[allow(clippy::too_many_arguments)]
    pub fn start_voice_params(
        &mut self,
        vv: usize,
        wave: WaveRef,
        pitch: f32,
        vol: i16,
        pan: u8,
        fxmix: u8,
        t: i32,
        unk1c: u8,
        unk1d: u8,
    ) {
        let samples = self.time_to_samples(t);
        let unity = self.vvoices[vv].unity_pitch;
        self.post(
            vv,
            ParamKind::StartVoiceAlt {
                unity,
                pan,
                volume: vol,
                fx_mix: fxmix,
                unk1c,
                unk1d,
                pitch,
                samples,
                wave,
            },
        );
    }

    // --- n_alEnvmixerPull -------------------------------------------------

    /// `n_alEnvmixerPull`: applies the updates due in this sub-frame, each
    /// at its 184-rounded offset, then pulls the rest of the sub-frame.
    fn envmixer_pull(&mut self, data: &AudioData, i: usize, sample_offset: i32) {
        let mut this_offset = sample_offset;
        let mut out_count = FIXED_SAMPLE;
        let mut inp = dsp::TEMP_0;
        loop {
            let p = self.pvoices[i].em_ctrl_list;
            if p == NONE {
                break;
            }
            let param = self.params[p as usize];
            let last = this_offset;
            this_offset = sample184(param.delta);
            let samples = this_offset.wrapping_sub(last);
            if samples > out_count {
                break;
            }
            match param.kind {
                ParamKind::StartVoiceAlt {
                    unity,
                    pan,
                    volume,
                    fx_mix,
                    unk1c,
                    unk1d,
                    pitch,
                    samples: s,
                    wave,
                } => {
                    let eq = &data.eqpower;
                    let e = &mut self.pvoices[i];
                    if unity != 0 {
                        e.rs_upitch = 1;
                    }
                    load_wavetable(e, data, wave);
                    e.em_motion = AL_PLAYING;
                    e.em_first = 1;
                    e.em_delta = 0;
                    e.em_seg_end = (((s + FIXED_SAMPLE / 2) / FIXED_SAMPLE) as u32
                        * FIXED_SAMPLE as u32) as i32;
                    e.em_volume = ((volume as i32 * volume as i32) >> 15) as i16;
                    e.em_pan = pan as i16;
                    if unk1c != 0 || unk1d != 0x5F {
                        e.em_wetamt = eq[127 - unk1c as usize];
                        e.em_dryamt = eq[127 - unk1d as usize];
                    } else {
                        e.em_dryamt = eq[fx_mix as usize];
                        e.em_wetamt = eq[127 - fx_mix as usize];
                    }
                    if s != 0 {
                        e.em_cvol_l = 1;
                        e.em_cvol_r = 1;
                    } else {
                        e.em_cvol_l = ((e.em_volume as i32 * eq[e.em_pan as usize] as i32) >> 15) as i16;
                        e.em_cvol_r =
                            ((e.em_volume as i32 * eq[127 - e.em_pan as usize] as i32) >> 15) as i16;
                    }
                    e.rs_ratio = pitch;
                }
                ParamKind::SetFxAmt(_)
                | ParamKind::SetPan(_)
                | ParamKind::SetVolume { .. }
                | ParamKind::SetFxAmtAlt { .. } => {
                    self.pull_sub_frame(data, i, &mut inp, samples);
                    let eq = &data.eqpower;
                    let e = &mut self.pvoices[i];
                    if e.em_delta >= e.em_seg_end {
                        e.em_ltgt = ((e.em_volume as i32 * eq[e.em_pan as usize] as i32) >> 15) as i16;
                        e.em_rtgt =
                            ((e.em_volume as i32 * eq[127 - e.em_pan as usize] as i32) >> 15) as i16;
                        e.em_delta = e.em_seg_end;
                        e.em_cvol_l = e.em_ltgt;
                        e.em_cvol_r = e.em_rtgt;
                    } else {
                        e.em_cvol_l = get_vol(e.em_cvol_l, e.em_delta, e.em_lratm, e.em_lratl);
                        e.em_cvol_r = get_vol(e.em_cvol_r, e.em_delta, e.em_rratm, e.em_rratl);
                    }
                    if e.em_cvol_l == 0 {
                        e.em_cvol_l = 1;
                    }
                    if e.em_cvol_r == 0 {
                        e.em_cvol_r = 1;
                    }
                    match param.kind {
                        ParamKind::SetPan(pan) => e.em_pan = pan as i16,
                        ParamKind::SetVolume { volume, time } => {
                            e.em_delta = 0;
                            e.em_volume = ((volume * volume) >> 15) as i16;
                            e.em_seg_end = sample184(time);
                        }
                        ParamKind::SetFxAmt(d) => {
                            e.em_dryamt = eq[d as usize];
                            e.em_wetamt = eq[127 - d as usize];
                        }
                        ParamKind::SetFxAmtAlt { data: d, more } => {
                            e.em_dryamt = eq[128 - more as usize];
                            e.em_wetamt = eq[127 - d as usize];
                        }
                        _ => {}
                    }
                    e.em_first = 1;
                }
                ParamKind::StopVoice => {
                    self.pull_sub_frame(data, i, &mut inp, samples);
                    envmixer_reset(&mut self.pvoices[i], data);
                }
                ParamKind::FreeVoice(pv) => {
                    self.pvoices[pv as usize].offset = 0;
                    self.free_pvoice(pv);
                }
                ParamKind::SetPitch(f) => {
                    self.pull_sub_frame(data, i, &mut inp, samples);
                    self.pvoices[i].rs_ratio = f;
                }
            }
            // `loutp` advances too, but n_aEnvMixer always writes the bus
            // start: every update falls on a 184-sample boundary, so
            // `samples` is 0 or the whole sub-frame.
            out_count -= samples;
            let e = &mut self.pvoices[i];
            e.em_ctrl_list = param.next;
            if e.em_ctrl_list == NONE {
                e.em_ctrl_tail = NONE;
            }
            self.free_param(p);
        }
        self.pull_sub_frame(data, i, &mut inp, out_count);
        let e = &mut self.pvoices[i];
        if e.em_delta > e.em_seg_end {
            e.em_delta = e.em_seg_end;
        }
    }

    /// `_pullSubFrame`.
    fn pull_sub_frame(&mut self, data: &AudioData, i: usize, inp: &mut usize, out_count: i32) {
        if self.pvoices[i].em_motion != AL_PLAYING || out_count == 0 {
            return;
        }
        self.resample_pull(data, i, *inp);
        let eq = &data.eqpower;
        let e = &mut self.pvoices[i];
        let init = if e.em_first != 0 {
            e.em_first = 0;
            e.em_ltgt = ((e.em_volume as i32 * eq[e.em_pan as usize] as i32) >> 15) as i16;
            e.em_lratm = get_rate(e.em_cvol_l as f32, e.em_ltgt as f32, e.em_seg_end, &mut e.em_lratl);
            e.em_rtgt = ((e.em_volume as i32 * eq[127 - e.em_pan as usize] as i32) >> 15) as i16;
            e.em_rratm = get_rate(e.em_cvol_r as f32, e.em_rtgt as f32, e.em_seg_end, &mut e.em_rratl);
            Some(EnvMixInit {
                vol: [e.em_cvol_l, e.em_cvol_r],
                tgt: [e.em_ltgt, e.em_rtgt],
                rate_m: [e.em_lratm, e.em_rratm],
                rate_l: [e.em_lratl, e.em_rratl],
                dry: e.em_dryamt,
                wet: e.em_wetamt,
            })
        } else {
            None
        };
        self.dmem.envmix(init.as_ref(), &mut e.em_state);
        *inp += (FIXED_SAMPLE as usize) << 1;
        e.em_delta += FIXED_SAMPLE;
    }

    /// `n_alResamplePull`: 184 samples to `N_AL_RESAMPLER_OUT`.
    fn resample_pull(&mut self, data: &AudioData, i: usize, outp: usize) {
        let mut inp = dsp::TEMP_1;
        let e = &mut self.pvoices[i];
        if e.rs_upitch != 0 {
            self.adpcm_pull(data, i, &mut inp, FIXED_SAMPLE);
            self.dmem.dmem_move(inp, outp, (FIXED_SAMPLE as usize) << 1);
        } else {
            if e.rs_ratio > MAX_RATIO {
                e.rs_ratio = MAX_RATIO;
            }
            e.rs_ratio = (e.rs_ratio * UNITY_PITCH) as i32 as f32;
            e.rs_ratio /= UNITY_PITCH;
            let fin_count = e.rs_delta + e.rs_ratio * FIXED_SAMPLE as f32;
            let in_count = fin_count as i32;
            e.rs_delta = fin_count - in_count as f32;
            self.adpcm_pull(data, i, &mut inp, in_count);
            let e = &mut self.pvoices[i];
            let incr = (e.rs_ratio * UNITY_PITCH) as i32;
            // n_aResample's output selector is 0: the main workspace.
            self.dmem.resample(
                e.rs_first as u32,
                incr as u16,
                &mut e.rs_state,
                inp,
                dsp::TEMP_0,
                &data.resample,
            );
            e.rs_first = 0;
        }
    }

    /// `n_alAdpcmPull`: decodes `out_count` samples to `*outp`, advancing it
    /// past the 16 history samples (or `dc_lastsam`).
    fn adpcm_pull(&mut self, data: &AudioData, i: usize, outp: &mut usize, mut out_count: i32) {
        if out_count == 0 {
            return;
        }
        let f = &mut self.pvoices[i];
        let Some(table) = f.dc_table else {
            return;
        };
        let (wt, bank, tbl) = data.wave(table);
        let book = wt.book.map_or(&[0i16; 64][..], |b| &bank.books[b as usize].book[..]);
        let base = wt.base as i32;
        let len = ADPCMFBYTES * (wt.len / ADPCMFBYTES);

        let looped = (out_count as u32).wrapping_add(f.dc_sample as u32) > f.dc_loop.end
            && f.dc_loop.count != 0;
        let mut n_sam = if looped {
            f.dc_loop.end as i32 - f.dc_sample
        } else {
            out_count
        };
        let n_left = if f.dc_lastsam != 0 {
            ADPCMFSIZE - f.dc_lastsam
        } else {
            0
        };
        let mut tsam = (n_sam - n_left).max(0);
        let mut nframes = (tsam + ADPCMFSIZE - 1) >> LFSAMPLES;
        let mut nbytes = nframes * ADPCMFBYTES;

        if looped {
            let first = f.dc_first as u32;
            decode_chunk(&mut self.dmem, f, tbl, book, tsam, nbytes, *outp, first);
            let f = &mut self.pvoices[i];
            if f.dc_lastsam != 0 {
                *outp += (f.dc_lastsam << 1) as usize;
            } else {
                *outp += (ADPCMFSIZE << 1) as usize;
            }
            f.dc_lastsam = (f.dc_loop.start & 0xF) as i32;
            f.dc_memin = base + ADPCMFBYTES * ((f.dc_loop.start >> LFSAMPLES) as i32 + 1);
            f.dc_sample = f.dc_loop.start as i32;
            let mut b_end = *outp as i32;
            while out_count > n_sam {
                let f = &mut self.pvoices[i];
                out_count -= n_sam;
                let op = (b_end + ((nframes + 1) << (LFSAMPLES + 1)) + 16) & !0x1F;
                b_end += n_sam << 1;
                if f.dc_loop.count != u32::MAX && f.dc_loop.count != 0 {
                    f.dc_loop.count -= 1;
                }
                n_sam = out_count.min(f.dc_loop.end as i32 - f.dc_loop.start as i32);
                tsam = (n_sam - ADPCMFSIZE + f.dc_lastsam).max(0);
                nframes = (tsam + ADPCMFSIZE - 1) >> LFSAMPLES;
                nbytes = nframes * ADPCMFBYTES;
                let flags = f.dc_first as u32 | dsp::A_LOOP;
                decode_chunk(&mut self.dmem, f, tbl, book, tsam, nbytes, op as usize, flags);
                let lastsam = self.pvoices[i].dc_lastsam;
                self.dmem
                    .dmem_move((op + (lastsam << 1)) as usize, b_end as usize, (n_sam << 1) as usize);
            }
            let f = &mut self.pvoices[i];
            f.dc_lastsam = (out_count + f.dc_lastsam) & 0xF;
            f.dc_sample += out_count;
            f.dc_memin += ADPCMFBYTES * nframes;
            return;
        }

        n_sam = nframes << LFSAMPLES;
        let overflow = (f.dc_memin + nbytes - (base + len)).max(0);
        let mut n_over = (overflow / ADPCMFBYTES) << LFSAMPLES;
        if n_over > n_sam + n_left {
            n_over = n_sam + n_left;
        }
        nbytes -= overflow;
        let mut decoded = false;
        if (n_over - (n_over & 0xF)) < out_count {
            decoded = true;
            let first = f.dc_first as u32;
            decode_chunk(&mut self.dmem, f, tbl, book, n_sam - n_over, nbytes, *outp, first);
            let f = &mut self.pvoices[i];
            if f.dc_lastsam != 0 {
                *outp += (f.dc_lastsam << 1) as usize;
            } else {
                *outp += (ADPCMFSIZE << 1) as usize;
            }
            f.dc_lastsam = (out_count + f.dc_lastsam) & 0xF;
            f.dc_sample += out_count;
            f.dc_memin += ADPCMFBYTES * nframes;
        } else {
            f.dc_lastsam = 0;
            f.dc_memin += ADPCMFBYTES * nframes;
        }
        if n_over != 0 {
            self.pvoices[i].dc_lastsam = 0;
            let start_zero = if decoded {
                (n_left + n_sam - n_over) << 1
            } else {
                0
            };
            self.dmem
                .clear((start_zero + *outp as i32) as usize, (n_over << 1) as usize);
        }
    }
}

/// `_decodeChunk`: decode `tsam` samples from `dc_memin` to `outp`.
#[allow(clippy::too_many_arguments)]
fn decode_chunk(
    dmem: &mut Dmem,
    f: &mut PVoice,
    tbl: &[u8],
    book: &[i16],
    tsam: i32,
    nbytes: i32,
    outp: usize,
    flags: u32,
) {
    let input = if nbytes > 0 {
        let start = (f.dc_memin.max(0) as usize).min(tbl.len());
        let end = (start + nbytes as usize).min(tbl.len());
        &tbl[start..end]
    } else {
        &[][..]
    };
    let lstate = f.dc_lstate;
    dmem.adpcm(flags, &mut f.dc_state, &lstate, book, input, outp, (tsam << 1) as usize);
    f.dc_first = 0;
}

/// `n_alLoadParam(AL_FILTER_SET_WAVETABLE)`. The original also truncates
/// the shared `ALWaveTable::len` to whole frames; [`Synth::adpcm_pull`]
/// does that on use.
fn load_wavetable(a: &mut PVoice, data: &AudioData, w: WaveRef) {
    let (wt, bank, _) = data.wave(w);
    a.dc_table = Some(w);
    a.dc_memin = wt.base as i32;
    a.dc_sample = 0;
    if wt.kind == AL_ADPCM_WAVE || wt.kind == super::data::AL_RAW16_WAVE {
        if let Some(l) = wt.loop_ {
            let l = &bank.loops[l as usize];
            a.dc_loop = RawLoop {
                start: l.start,
                end: l.end,
                count: l.count,
            };
            if wt.kind == AL_ADPCM_WAVE {
                a.dc_lstate = l.state;
            }
        } else {
            a.dc_loop = RawLoop::default();
        }
    }
}

/// `n_alEnvmixerParam(AL_FILTER_RESET)` with `n_alLoadParam`'s reset.
fn envmixer_reset(e: &mut PVoice, data: &AudioData) {
    e.em_first = 1;
    e.em_motion = AL_STOPPED;
    e.em_volume = 1;
    e.rs_delta = 0.0;
    e.rs_first = 1;
    e.rs_upitch = 0;
    e.dc_lastsam = 0;
    e.dc_first = 1;
    e.dc_sample = 0;
    if let Some(w) = e.dc_table {
        let (wt, bank, _) = data.wave(w);
        e.dc_memin = wt.base as i32;
        if let Some(l) = wt.loop_ {
            e.dc_loop.count = bank.loops[l as usize].count;
        }
    }
}

/// `_getRate` (N_MICRO): the per-8-samples volume step towards `tgt` over
/// `count` samples, as an `s16` mantissa and `u16` fraction.
pub fn get_rate(mut vol: f32, mut tgt: f32, count: i32, ratel: &mut u16) -> i16 {
    if count == 0 {
        if tgt >= vol {
            *ratel = 0xFFFF;
            return 0x7FFF;
        }
        *ratel = 0;
        return 0;
    }
    let invn = 1.0f32 / count as f32;
    if tgt < 1.0 {
        tgt = 1.0;
    }
    if vol <= 0.0 {
        vol = 1.0;
    }
    let a = (tgt - vol) * invn * 8.0;
    let mut s = a as i32 as i16;
    let mut f = a - s as f32;
    s = s.wrapping_sub(1);
    f += 1.0;
    let tmp = f as i32 as i16;
    s = s.wrapping_add(tmp);
    f -= tmp as f32;
    *ratel = (65535.0f32 * f) as i32 as u16;
    s
}

/// `_getVol` (N_MICRO).
pub fn get_vol(ivol: i16, samples: i32, ratem: i16, ratel: u16) -> i16 {
    let samples = samples >> 3;
    if samples == 0 {
        return ivol;
    }
    let mut tmp = (ratel as i32).wrapping_mul(samples);
    tmp >>= 16;
    tmp = tmp.wrapping_add((ratem as i32).wrapping_mul(samples));
    (ivol as i32).wrapping_add(tmp) as i16
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::testdata;

    struct Silent;
    impl Clients for Silent {
        fn handle(&mut self, _: Client, _: &mut Synth, _: &AudioData) -> i32 {
            1_000_000
        }
    }

    #[test]
    fn one_voice_renders_sound() {
        let Some(data) = testdata::data() else {
            return;
        };
        let mut syn = Synth::new(32006, None);
        syn.add_client(Client::Snd);
        let vc = VoiceConfig {
            priority: 10,
            fx_bus: 0,
            unity_pitch: 0,
        };
        assert!(syn.alloc_voice(FGM_VOICE_BASE, &vc));
        syn.start_voice_params(FGM_VOICE_BASE, WaveRef { bank: 1, index: 0 }, 1.0, 0x7FFF, 64, 0, 0, 0, 0x5F);
        let mut out = [0i16; 552 * 2];
        let mut peak = 0i32;
        for _ in 0..20 {
            syn.audio_frame(&data, &mut Silent, &mut out, 552);
            peak = peak.max(out.iter().map(|s| (*s as i32).abs()).max().unwrap());
        }
        assert!(peak > 1000, "peak {peak}");
        assert_eq!(syn.active_voices(), 1);
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    /// `(vol, tgt, count, ratem, ratel, _getVol(vol, count, ratem, ratel))`
    /// from the decomp's N_MICRO `_getRate`/`_getVol` compiled as C.
    const GET_RATE: [(f32, f32, i32, i16, u16, i16); 9] = [
        (1.0, 20000.0, 184, 869, 34191, 19999),
        (20000.0, 1.0, 552, -290, 10447, 0),
        (0.0, 32767.0, 368, 712, 19947, 32766),
        (5000.0, 5000.0, 184, 0, 0, 5000),
        (100.0, 0.0, 1104, -1, 18520, 0),
        (32000.0, 1.0, 184, -1392, 48439, 0),
        (1.0, 1.0, 0, 32767, 65535, 1),
        (2.0, 1.0, 0, 0, 0, 2),
        (12345.0, 23456.0, 3864, 23, 271, 23455),
    ];

    #[test]
    fn get_rate_matches_c() {
        for (vol, tgt, count, m, l, v) in GET_RATE {
            let mut ratel = 0;
            assert_eq!(get_rate(vol, tgt, count, &mut ratel), m, "{vol} {tgt} {count}");
            assert_eq!(ratel, l, "{vol} {tgt} {count}");
            assert_eq!(get_vol(vol as i16, count, m, ratel), v, "{vol} {tgt} {count}");
        }
    }

    #[test]
    fn voice_steal_order() {
        let mut syn = Synth::new(32006, None);
        let vc = |priority| VoiceConfig {
            priority,
            fx_bus: 0,
            unity_pitch: 0,
        };
        // The free list was built by head insertion: pvoice 15 comes first.
        for vv in 0..16 {
            assert!(syn.alloc_voice(vv, &vc(if vv == 3 { 1 } else { 5 })));
        }
        assert_eq!(syn.vvoices[0].pvoice, Some(15));
        assert_eq!(syn.vvoices[3].pvoice, Some(12));
        // Lowest priority first, even below the requester's.
        assert!(syn.alloc_voice(16, &vc(5)));
        assert_eq!(syn.vvoices[16].pvoice, Some(12));
        assert_eq!(syn.vvoices[3].pvoice, None);
        assert_eq!(syn.pvoices[12].offset, 512);
        // Then the last on the allocated list (the oldest) at equal
        // priority, skipping one already stolen.
        assert!(syn.alloc_voice(17, &vc(5)));
        assert_eq!(syn.vvoices[17].pvoice, Some(15));
        assert!(syn.alloc_voice(18, &vc(5)));
        assert_eq!(syn.vvoices[18].pvoice, Some(14));
        // Nothing at or below priority 4 remains.
        assert!(!syn.alloc_voice(19, &vc(4)));
        // Each steal queued a volume ramp and a stop: 3 x 2 params.
        let queued = MAX_UPDATES - {
            let mut n = 0;
            let mut p = syn.param_list;
            while p != NONE {
                n += 1;
                p = syn.params[p as usize].next;
            }
            n
        };
        assert_eq!(queued, 6);
    }

    #[test]
    fn time_to_samples_rounds_like_c() {
        let syn = Synth::new(32006, None);
        // The FGM tick: 184000000 / 32006 = 5748 us -> 184 samples.
        assert_eq!(syn.time_to_samples_no_round(5748), 184);
        assert_eq!(syn.time_to_samples(16000), 512);
        assert_eq!(syn.time_to_samples_no_round(16000), 512);
    }
}
