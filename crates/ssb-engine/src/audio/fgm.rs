//! The FGM sound-effect engine: `n_env.c:3746-5458` (`D_8009EDD0_406D0`,
//! structs at `n_env.c:6-241`).
//!
//! Two interpreters share one synth client. A *script* (`siz34`, from
//! `fgm.ucd`) sequences notes; each note spawns or retunes a *voice*
//! (`EE0C`, from `fgm.tbl`), whose articulation program sets the wave,
//! volume, pan, fx and pitch and attaches *LFOs* (`siz24`, from `fgm.unk`).
//! [`Fgm::tick`] (`func_800293A8_29FA8`) runs every 184 samples.
//!
//! Every C pointer is a pool index here (pointer identity = index identity),
//! and every list keeps libultra's link order: head insertion, LIFO free
//! lists. FGM voice `j` drives synth vvoice [`FGM_VOICE_BASE`]` + j`.
//!
//! The C masks interrupts around the list operations of the API calls; the
//! caller serialises calls with [`Fgm::tick`] instead.

use super::data::{AudioData, WaveRef};
use super::synth::{Synth, VoiceConfig, AL_PLAYING, AL_STOPPED, Client, FGM_VOICE_BASE};
use super::FgmHandle;
use alloc::vec::Vec;

/// `audio_config.unk_80026204_0x0` = `unk31`, 48 in the settings but set to
/// 72 by `scmanager.c:836` before `func_80026204_26E04` runs.
pub const LFO_POOL: usize = 72;
/// `unk32`: the voice (`EE0C`) pool.
pub const VOICE_POOL: usize = 24;
/// `sndplayers_num`: the script (`siz34`) pool.
pub const SCRIPT_POOL: usize = 24;

/// The null link.
const NIL: u8 = 0xFF;

/// A script byte past the end of `fgm.ucd` reads as `0xD0` (end). The C reads
/// whatever follows in RDRAM; shipped scripts never run off their file.
const UCD_PAST_END: u8 = 0xD0;
/// A voice byte past the end of `fgm.tbl` reads as `0x70` (fade out, which
/// also sets a nonzero timer), for the same reason.
const TBL_PAST_END: u8 = 0x70;

/// Voice states (`EE0C.unk2A`).
const V_IDLE: u8 = 0;
const V_PLAYING: u8 = 1;
const V_FADING: u8 = 2;
const V_START: u8 = 3;

/// `alCents2Ratio` (`src/libultra/audio/cents2ratio.c:18`): square and
/// multiply in f32.
pub fn cents2ratio(cents: i32) -> f32 {
    let mut ratio = 1.0f32;
    let (mut x, mut c) = if cents >= 0 {
        (1.000_577_8_f32, cents as u32)
    } else {
        (0.999_422_54_f32, cents.unsigned_abs())
    };
    while c != 0 {
        if c & 1 != 0 {
            ratio *= x;
        }
        x *= x;
        c >>= 1;
    }
    ratio
}

/// `ALWhatever8009EDD0_siz24`: one LFO, linked into a voice's id-sorted list.
#[derive(Debug, Clone, Copy, Default)]
struct Lfo {
    next: u8,
    /// `0x4`: id (list key).
    id: u8,
    /// `0x5`: shape 0-8.
    shape: u8,
    /// `0x6`: target (10-15 voice fields, 16-23 own fields, 24+ another
    /// LFO's fields, <10 scratch).
    target: u8,
    /// `0x7`: postproc (`>>4`: 1 add, 2 multiply by `spC0[&0xF]`).
    postproc: u8,
    /// `0x8`.
    period: f32,
    /// `0xC`.
    amp: f32,
    /// `0x10`.
    offset: f32,
    /// `0x14`.
    phase: f32,
    /// `0x18`: hold length of the random shapes.
    hold: f32,
    /// `0x1C` / `0x20`: random endpoints.
    from: f32,
    to: f32,
}

/// `ALWhatever8009EE0C(_2)`: an articulated voice. The `N_ALVoice` at `0x4`
/// is `syn.vvoices[FGM_VOICE_BASE + index]`.
#[derive(Debug, Clone, Copy)]
struct Voice {
    next: u8,
    /// `0x20` / `0x24`: program counter and loop mark (offsets into
    /// `fgm.tbl`).
    pc: u32,
    loop_pc: u32,
    /// `0x28`.
    timer: u16,
    /// `0x2A`: 0 idle (reaped), 1 playing, 2 fading out, 3 start pending.
    state: u8,
    /// `0x2B`: synth priority `& 0x7F`; bit 7 = pausable.
    priority: u8,
    /// `0x2C` / `0x2E`: pitch (cents) and its previous value.
    pitch: i16,
    prev_pitch: i16,
    /// `0x30`: the note offset (cents).
    note: i16,
    /// `0x32`..`0x3D`: value / previous value pairs.
    vol: u8,
    prev_vol: u8,
    pan: u8,
    prev_pan: u8,
    fx: u8,
    prev_fx: u8,
    vol_scale: u8,
    prev_vol_scale: u8,
    pan_offset: u8,
    prev_pan_offset: u8,
    fx_scale: u8,
    prev_fx_scale: u8,
    /// `0x40`: the wavetable (sfx bank index). Never reset on allocation, so
    /// a program that starts without a `0x60` op reuses the node's previous
    /// wave; `None` only before the node's first wave (the C would start a
    /// NULL wavetable; the port drops the start instead).
    wave: Option<u16>,
    /// `0x44`: LFO list head.
    lfos: u8,
    /// `0x48`.
    serial: u16,
}

impl Voice {
    const ZERO: Self = Self {
        next: NIL,
        pc: 0,
        loop_pc: 0,
        timer: 0,
        state: 0,
        priority: 0,
        pitch: 0,
        prev_pitch: 0,
        note: 0,
        vol: 0,
        prev_vol: 0,
        pan: 0,
        prev_pan: 0,
        fx: 0,
        prev_fx: 0,
        vol_scale: 0,
        prev_vol_scale: 0,
        pan_offset: 0,
        prev_pan_offset: 0,
        fx_scale: 0,
        prev_fx_scale: 0,
        wave: None,
        lfos: NIL,
        serial: 0,
    };
}

/// `ALWhatever8009EDD0_siz34` (the game's `alSoundEffect`): a script.
#[derive(Debug, Clone, Copy)]
struct Script {
    next: u8,
    /// `0x4`: the fork root (`NIL` for a root script).
    parent: u8,
    /// `0x8` / `0xC`: program counter and loop mark (offsets into `fgm.ucd`).
    pc: u32,
    loop_pc: u32,
    /// `0x10`: 0xFFFF = ended (reaped at the end of the tick), 0 = free or
    /// stopped.
    timer: u16,
    /// `0x12[6]`: note durations 1-6.
    durations: [u16; 6],
    /// `0x1E`: set by D2, never read.
    unk1e: u8,
    /// `0x1F`: priority; bit 7 = pausable.
    priority: u8,
    /// `0x20`: release one tick early.
    early_release: u8,
    /// `0x21`: release pending.
    release_pending: u8,
    /// `0x22` / `0x23`: script volume (0..255) and pan.
    volume: u8,
    pan: u8,
    /// `0x24`: articulation (`fgm.tbl`) id. Not reset on allocation.
    artic: u16,
    /// `0x26`: the serial (`sfx_id`).
    serial: u16,
    /// `0x28`: the current voice.
    voice: u8,
    /// `0x2C`: fx.
    fx: u8,
    /// `0x2D`: exclusive group.
    group: u8,
    /// `0x2E` / `0x2F` / `0x30`: the game's volume, pan (`balance`) and fx;
    /// 0x80 = unset for pan and fx.
    game_vol: u8,
    game_pan: u8,
    game_fx: u8,
}

impl Script {
    const ZERO: Self = Self {
        next: NIL,
        parent: NIL,
        pc: 0,
        loop_pc: 0,
        timer: 0,
        durations: [0; 6],
        unk1e: 0,
        priority: 0,
        early_release: 0,
        release_pending: 0,
        volume: 0,
        pan: 0,
        artic: 0,
        serial: 0,
        voice: NIL,
        fx: 0,
        group: 0,
        game_vol: 0,
        game_pan: 0,
        game_fx: 0,
    };
}

/// `D_8009EDD0_406D0`: the FGM player.
pub struct Fgm {
    lfos: [Lfo; LFO_POOL],
    voices: [Voice; VOICE_POOL],
    scripts: [Script; SCRIPT_POOL],
    /// `0x14`: `instArray[0]->soundCount` of the sfx bank (322).
    sound_count: u16,
    /// `0x28` / `0x2A` / `0x2C`: `fgm.ucd`, `fgm.tbl`, `fgm.unk` counts.
    ucd_count: u16,
    tbl_count: u16,
    unk_count: u16,
    /// `0x30` / `0x34` / `0x38`: free LFO, voice, script lists.
    free_lfos: u8,
    free_voices: u8,
    free_scripts: u8,
    /// `0x3C` / `0x40`: active voices and scripts.
    active_voices: u8,
    active_scripts: u8,
    /// `0x5C` / `0x60`: paused voices and scripts.
    paused_voices: u8,
    paused_scripts: u8,
    /// `0x44`: `184000000 / outputRate` microseconds (one 184-sample tick).
    tick_us: i32,
    /// `0x48` / `0x4A`: voice and script serials (0 skipped).
    voice_serial: u16,
    script_serial: u16,
    /// `0x4C` / `0x4E[6]`: never written by `func_80026204_26E04`, so BSS
    /// zero for the whole game.
    default_priority: u8,
    default_durations: [u16; 6],
    /// `0x5A`.
    master_volume: u8,
    /// `0x1C`: `fgm.ucd` entry offsets (copied: `play` gets no data).
    ucd_offsets: Vec<u32>,
    /// `sRandomSeed2` (`randFloat1`) and `sRandomSeed1` (`randFloat2`).
    seed2: i32,
    seed1: i32,
    /// Script allocations (play and D9 forks) that found the pool empty.
    pub script_exhausted: u32,
    /// Voice allocations (notes) that found the pool empty.
    pub voice_exhausted: u32,
    /// LFO allocations (op 0x40) that found the pool empty.
    pub lfo_exhausted: u32,
    /// `n_alSynAllocVoice` failures (state 3 -> 0: the note stays silent).
    pub synth_alloc_failed: u32,
}

/// One big-endian f32 of a `fgm.unk` record.
fn be_f32(b: &[u8], at: usize) -> f32 {
    f32::from_bits(u32::from_be_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]]))
}

impl Fgm {
    /// `func_80026204_26E04`: the three pools in index order (free head =
    /// node 0), the counts from the data, `0x44 = 184000000 / outputRate`,
    /// master volume 127, registers the client (`n_alSynAddSndPlayer`) and
    /// resets `sRandomSeed2` to 1. `sRandomSeed1` keeps its value: 1 from
    /// the data segment at boot. A re-init of the audio system that builds a
    /// new `Fgm` should carry it over with [`Fgm::rng_seeds`].
    pub fn new(syn: &mut Synth, data: &AudioData) -> Self {
        let mut f = Self {
            lfos: [Lfo::default(); LFO_POOL],
            voices: [Voice::ZERO; VOICE_POOL],
            scripts: [Script::ZERO; SCRIPT_POOL],
            sound_count: data
                .sfx
                .inst_array
                .first()
                .map_or(0, |&i| data.sfx.instruments[i as usize].sounds.len() as u16),
            ucd_count: data.fgm_ucd_offsets.len() as u16,
            tbl_count: data.fgm_tbl_offsets.len() as u16,
            unk_count: data.fgm_unk_count as u16,
            free_lfos: 0,
            free_voices: 0,
            free_scripts: 0,
            active_voices: NIL,
            active_scripts: NIL,
            paused_voices: NIL,
            paused_scripts: NIL,
            tick_us: 184_000_000 / syn.output_rate,
            voice_serial: 0,
            script_serial: 0,
            default_priority: 0,
            default_durations: [0; 6],
            master_volume: 127,
            ucd_offsets: data.fgm_ucd_offsets.clone(),
            seed2: 1,
            seed1: 1,
            script_exhausted: 0,
            voice_exhausted: 0,
            lfo_exhausted: 0,
            synth_alloc_failed: 0,
        };
        for (i, l) in f.lfos.iter_mut().enumerate() {
            l.next = if i + 1 < LFO_POOL { i as u8 + 1 } else { NIL };
        }
        for (i, v) in f.voices.iter_mut().enumerate() {
            v.next = if i + 1 < VOICE_POOL { i as u8 + 1 } else { NIL };
        }
        for (i, s) in f.scripts.iter_mut().enumerate() {
            s.next = if i + 1 < SCRIPT_POOL { i as u8 + 1 } else { NIL };
        }
        syn.add_client(Client::Snd);
        f
    }

    /// `(sRandomSeed1, sRandomSeed2)`.
    pub fn rng_seeds(&self) -> (i32, i32) {
        (self.seed1, self.seed2)
    }

    /// Restores `sRandomSeed1` (never reset by `func_80026204_26E04`).
    pub fn set_rng_seed1(&mut self, seed1: i32) {
        self.seed1 = seed1;
    }

    /// `randFloat1` (`n_env.c:4570`): `sRandomSeed2`.
    fn rand1(&mut self) -> f32 {
        self.seed2 = self.seed2.wrapping_mul(0x343FD).wrapping_add(0x269EC3);
        ((self.seed2 >> 16) & 0xFFFF) as f32 / 65536.0
    }

    /// `randFloat2` (`n_env.c:4576`): `sRandomSeed1`.
    fn rand2(&mut self) -> f32 {
        self.seed1 = self.seed1.wrapping_mul(0x343FD).wrapping_add(0x269EC3);
        ((self.seed1 >> 16) & 0xFFFF) as f32 / 65536.0
    }

    // --- The client ------------------------------------------------------

    /// `func_800293A8_29FA8`, the synth client handler: (a) reap idle voices
    /// (stop and free the synth voice, clear the active scripts' pointers to
    /// it, return its LFOs, push it on the free list), (b) run every active
    /// script, (c) articulate every active voice, (d) reap ended scripts
    /// (timer 0xFFFF: timer and serial cleared), (e) advance `randFloat2`.
    /// Returns `0x44`, the microseconds to the next call.
    pub fn tick(&mut self, syn: &mut Synth, data: &AudioData) -> i32 {
        // (a)
        let mut prev = NIL;
        let mut at = self.active_voices;
        while at != NIL {
            let next = self.voices[at as usize].next;
            if self.voices[at as usize].state == V_IDLE {
                let vv = FGM_VOICE_BASE + at as usize;
                syn.stop_voice(vv);
                syn.free_voice(vv);
                let mut s = self.active_scripts;
                while s != NIL {
                    if self.scripts[s as usize].voice == at {
                        self.scripts[s as usize].voice = NIL;
                    }
                    s = self.scripts[s as usize].next;
                }
                let head = self.voices[at as usize].lfos;
                if head != NIL {
                    let mut last = head;
                    while self.lfos[last as usize].next != NIL {
                        last = self.lfos[last as usize].next;
                    }
                    self.lfos[last as usize].next = self.free_lfos;
                    self.free_lfos = head;
                    self.voices[at as usize].lfos = NIL;
                }
                self.voices[at as usize].serial = 0;
                if prev != NIL {
                    self.voices[prev as usize].next = next;
                } else {
                    self.active_voices = next;
                }
                self.voices[at as usize].next = self.free_voices;
                self.free_voices = at;
            } else {
                prev = at;
            }
            at = next;
        }

        // (b) A fork (D9) appends to this list, so it runs this same tick.
        let mut at = self.active_scripts;
        while at != NIL {
            self.run_script(at, data);
            at = self.scripts[at as usize].next;
        }

        // (c) Voices spawned in (b) sit at the head and run too.
        let mut at = self.active_voices;
        while at != NIL {
            self.articulate(at, syn, data);
            at = self.voices[at as usize].next;
        }

        // (d)
        let mut prev = NIL;
        let mut at = self.active_scripts;
        while at != NIL {
            let next = self.scripts[at as usize].next;
            if self.scripts[at as usize].timer == 0xFFFF {
                self.scripts[at as usize].timer = 0;
                self.scripts[at as usize].serial = 0;
                if prev != NIL {
                    self.scripts[prev as usize].next = next;
                } else {
                    self.active_scripts = next;
                }
                self.scripts[at as usize].next = self.free_scripts;
                self.free_scripts = at;
            } else {
                prev = at;
            }
            at = next;
        }

        // (e)
        self.rand2();
        self.tick_us
    }

    // --- Articulation (voices) -------------------------------------------

    /// `func_80027460_28060`: runs the voice's program when its timer
    /// expires, then its LFOs, then applies the state to the synth voice.
    fn articulate(&mut self, vi: u8, syn: &mut Synth, data: &AudioData) {
        let tbl = data.fgm_tbl;
        let rd = |pc: &mut u32| -> u8 {
            let b = tbl.get(*pc as usize).copied().unwrap_or(TBL_PAST_END);
            *pc += 1;
            b
        };
        let v = vi as usize;
        if self.voices[v].timer != 0 {
            self.voices[v].timer -= 1;
            if self.voices[v].timer == 0 {
                let mut pc = self.voices[v].pc;
                let mut timer: u16;
                loop {
                    let instr = rd(&mut pc);
                    timer = (instr & 0xF) as u16;
                    if timer & 0x8 != 0 {
                        let p = rd(&mut pc) as u16;
                        timer = ((timer & 0x7) << 7) + (p & 0x7F);
                        if p & 0x80 != 0 {
                            timer = ((timer as u32) << 8).wrapping_add(rd(&mut pc) as u32) as u16;
                        }
                    }
                    match instr & 0xF0 {
                        0x00 => {
                            let p = rd(&mut pc);
                            self.voices[v].vol = set_or_add_7bit(p, self.voices[v].vol);
                        }
                        0x10 => {
                            let p = rd(&mut pc);
                            self.voices[v].pan = set_or_add_7bit(p, self.voices[v].pan);
                        }
                        0x20 => {
                            let hi = rd(&mut pc) as u16;
                            let lo = rd(&mut pc) as u16;
                            let mut param = ((hi << 8) | lo) as i16;
                            if param <= 0x4B0 {
                                if param < -0x4B0 {
                                    param = -0x4B0;
                                }
                            } else {
                                param = param.wrapping_sub(0x960).wrapping_add(self.voices[v].pitch);
                                param = param.clamp(-0x4B0, 0x4B0);
                            }
                            self.voices[v].pitch = param;
                        }
                        0x30 => {
                            let p = rd(&mut pc);
                            self.voices[v].fx = set_or_add_7bit(p, self.voices[v].fx);
                        }
                        0x40 => {
                            let id = rd(&mut pc);
                            let mut idx = rd(&mut pc) as u16;
                            if idx & 0x80 != 0 {
                                idx = rd(&mut pc) as u16 + ((idx & 0x7F) << 8);
                            }
                            if idx < self.unk_count {
                                self.add_lfo(vi, id, idx, data);
                            }
                        }
                        0x50 => {
                            let id = rd(&mut pc);
                            let mut prev = NIL;
                            let mut at = self.voices[v].lfos;
                            while at != NIL {
                                if self.lfos[at as usize].id == id {
                                    let next = self.lfos[at as usize].next;
                                    if prev == NIL {
                                        self.voices[v].lfos = next;
                                    } else {
                                        self.lfos[prev as usize].next = next;
                                    }
                                    self.lfos[at as usize].next = self.free_lfos;
                                    self.free_lfos = at;
                                    break;
                                }
                                prev = at;
                                at = self.lfos[at as usize].next;
                            }
                        }
                        0x60 => {
                            let mut idx = rd(&mut pc) as u16;
                            if idx & 0x80 != 0 {
                                idx = rd(&mut pc) as u16 + ((idx & 0x7F) << 8);
                            }
                            if idx < self.sound_count {
                                // `soundArray[idx]->wavetable`.
                                let inst = &data.sfx.instruments[data.sfx.inst_array[0] as usize];
                                let sound = inst.sounds[idx as usize];
                                self.voices[v].wave = Some(data.sfx.sounds[sound as usize].wavetable);
                                self.voices[v].state = V_START;
                            }
                        }
                        0x70 => {
                            self.voices[v].state = V_FADING;
                            timer = 10000;
                        }
                        0x80 => self.voices[v].loop_pc = pc,
                        0x90 => pc = self.voices[v].loop_pc,
                        _ => {}
                    }
                    if timer != 0 {
                        break;
                    }
                }
                self.voices[v].pc = pc;
                self.voices[v].timer = timer;
            }
        }

        // The LFOs. `spC0` is an uninitialised `f32[6]` on the C stack, read
        // by postproc 1/2 and written by targets < 10 (indices up to 15 and
        // 9: past the array). No shipped `fgm.unk` record uses either (see
        // `fgm_unk_never_touches_scratch`), so the port gives each call a
        // zeroed 16-entry scratch.
        let mut scratch = [0f32; 16];
        let mut at = self.voices[v].lfos;
        while at != NIL {
            let li = at as usize;
            let mut val = self.lfo_value(li, data);
            let l = self.lfos[li];
            match l.postproc >> 4 {
                1 => val += scratch[(l.postproc & 0xF) as usize],
                2 => val *= scratch[(l.postproc & 0xF) as usize],
                _ => {}
            }
            let vo = &mut self.voices[v];
            match l.target {
                10 | 11 => {
                    if l.target == 11 {
                        val += vo.vol as f32;
                    }
                    vo.vol = val.clamp(0.0, 127.0) as u8;
                }
                12 | 13 => {
                    if l.target == 13 {
                        val += vo.pitch as f32;
                    }
                    vo.pitch = val.clamp(-1200.0, 1200.0) as i16;
                }
                14 | 15 => {
                    if l.target == 15 {
                        val += vo.pan as f32;
                    }
                    vo.pan = val.clamp(0.0, 127.0) as u8;
                }
                t if t < 10 => scratch[t as usize] = val,
                t => {
                    let (dst, field) = if t < 24 {
                        (li as u8, t)
                    } else {
                        let id = (t - 24) / 8;
                        let mut d = vo.lfos;
                        while d != NIL && self.lfos[d as usize].id != id {
                            d = self.lfos[d as usize].next;
                        }
                        (d, (t - 24) % 8 + 16)
                    };
                    if dst != NIL {
                        let d = &mut self.lfos[dst as usize];
                        let slot = match field {
                            16 | 17 => &mut d.period,
                            18 | 19 => &mut d.amp,
                            20 | 21 => &mut d.offset,
                            _ => &mut d.phase,
                        };
                        if field & 1 != 0 {
                            val += *slot;
                        }
                        *slot = val;
                    }
                }
            }
            at = self.lfos[li].next;
        }

        let vv = FGM_VOICE_BASE + v;
        let vo = self.voices[v];
        let level = ((vo.vol as i32 * vo.vol_scale as i32 * self.master_volume as i32) >> 7) as i16;
        let pan = (vo.pan as i32 + vo.pan_offset as i32 - 64).clamp(0, 0x7F) as u8;
        let fx = ((vo.fx as i32 * (vo.fx_scale >> 1) as i32) >> 7).clamp(0, 0x7F) as u8;
        match vo.state {
            V_PLAYING => {
                if vo.pitch != vo.prev_pitch {
                    syn.set_pitch(vv, cents2ratio(vo.pitch as i32 + vo.note as i32));
                }
                if vo.vol != vo.prev_vol || vo.vol_scale != vo.prev_vol_scale {
                    syn.set_vol(vv, level, self.tick_us);
                }
                if vo.pan != vo.prev_pan || vo.pan_offset != vo.prev_pan_offset {
                    syn.set_pan(vv, pan);
                }
                if vo.fx != vo.prev_fx || vo.fx_scale != vo.prev_fx_scale {
                    syn.set_fx_mix(vv, fx);
                }
            }
            V_FADING => {
                syn.set_vol(vv, 0, self.tick_us);
                self.voices[v].state = V_IDLE;
            }
            V_START => {
                let vc = VoiceConfig {
                    priority: (vo.priority & 0x7F) as i16,
                    fx_bus: 0,
                    unity_pitch: 0,
                };
                // `wave == None` would be a NULL wavetable in the C.
                match vo.wave {
                    Some(w) if syn.alloc_voice(vv, &vc) => {
                        let ratio = cents2ratio(vo.pitch as i32 + vo.note as i32);
                        let wave = WaveRef { bank: 1, index: w };
                        syn.start_voice_params(vv, wave, ratio, level, pan, fx, 0, 0, 0x5F);
                        self.voices[v].state = V_PLAYING;
                    }
                    _ => {
                        self.synth_alloc_failed += 1;
                        self.voices[v].state = V_IDLE;
                    }
                }
            }
            _ => {}
        }
        let vo = &mut self.voices[v];
        vo.prev_pitch = vo.pitch;
        vo.prev_vol = vo.vol;
        vo.prev_pan = vo.pan;
        vo.prev_fx = vo.fx;
        vo.prev_vol_scale = vo.vol_scale;
        vo.prev_pan_offset = vo.pan_offset;
        vo.prev_fx_scale = vo.fx_scale;
    }

    /// Op 0x40: find LFO `id` in the voice's id-sorted list, or insert a new
    /// one from the free list in order; then (re)initialise it from
    /// `fgm.unk[idx]`. A full pool drops the op.
    fn add_lfo(&mut self, vi: u8, id: u8, idx: u16, data: &AudioData) {
        let rec = &data.fgm_unk[4 + idx as usize * 16..][..16];
        let shape = rec[0];
        let target = rec[1];
        let postproc = rec[2];
        let phase0 = rec[3] as f32;
        let period = be_f32(rec, 4);
        let amp = be_f32(rec, 8);
        let offset = be_f32(rec, 12);

        let v = vi as usize;
        let mut prev = NIL;
        let mut at = self.voices[v].lfos;
        let node = loop {
            if at == NIL || id < self.lfos[at as usize].id {
                // Append at the end, or insert before `at`.
                let n = self.free_lfos;
                if n == NIL {
                    break NIL;
                }
                self.free_lfos = self.lfos[n as usize].next;
                self.lfos[n as usize].next = at;
                if prev != NIL {
                    self.lfos[prev as usize].next = n;
                } else {
                    self.voices[v].lfos = n;
                }
                break n;
            }
            if self.lfos[at as usize].id == id {
                break at;
            }
            prev = at;
            at = self.lfos[at as usize].next;
        };
        if node == NIL {
            self.lfo_exhausted += 1;
            return;
        }
        let k = 1.0f32 / 256.0;
        {
            let l = &mut self.lfos[node as usize];
            l.phase = period * phase0 * k;
            l.id = id;
            l.shape = shape;
            l.target = target;
            l.postproc = postproc;
            l.period = period;
            l.amp = amp;
            l.offset = offset;
        }
        match shape {
            4 | 8 => {
                let r = if shape == 4 { self.rand1() } else { self.rand2() };
                let l = self.lfos[node as usize];
                self.lfos[node as usize].to = r * l.amp + l.offset;
                let r = if shape == 4 { self.rand1() } else { self.rand2() };
                let l = &mut self.lfos[node as usize];
                l.hold = r * l.period;
                l.phase = l.hold * phase0 * k;
            }
            5 => {
                self.lfos[node as usize].from = 0.0;
                let r = self.rand1();
                let l = self.lfos[node as usize];
                self.lfos[node as usize].to = r * l.amp + l.offset;
                let r = self.rand1();
                let l = &mut self.lfos[node as usize];
                l.hold = r * l.period + 0.5;
                l.phase = l.hold * phase0 * k;
            }
            _ => {}
        }
    }

    /// One LFO step (`func_80027460_28060`'s second switch).
    fn lfo_value(&mut self, li: usize, data: &AudioData) -> f32 {
        let shape = self.lfos[li].shape;
        if shape < 4 {
            let l = &mut self.lfos[li];
            l.phase += 1.0;
            if l.period < l.phase {
                l.phase -= l.period;
            }
        }
        let l = self.lfos[li];
        match shape {
            0 => {
                // `gSYSinTable[0x800]` is the first half period of a sine
                // scaled to 0x7FFF; / 65536 gives a half-amplitude sine.
                let id = (((l.phase / l.period) * 4096.0) as i32 & 0xFFF) as u16;
                let angle = data.sin_table[(id & 0x7FF) as usize] as f32 / 65536.0;
                (if id & 0x800 != 0 { -angle } else { angle }) * l.amp + l.offset
            }
            1 => {
                if l.period / 2.0 < l.phase {
                    l.amp
                } else {
                    l.offset
                }
            }
            2 => (l.amp * l.phase) / l.period + l.offset,
            3 => (l.amp * (l.period - l.phase)) / l.period + l.offset,
            4 | 8 => {
                self.lfos[li].phase += 1.0;
                if self.lfos[li].hold < self.lfos[li].phase {
                    let r = if shape == 4 { self.rand1() } else { self.rand2() };
                    let l = self.lfos[li];
                    self.lfos[li].to = r * l.amp + l.offset;
                    let r = if shape == 4 { self.rand1() } else { self.rand2() };
                    let l = &mut self.lfos[li];
                    l.hold = r * l.period;
                    l.phase = 0.0;
                }
                self.lfos[li].to
            }
            5 => {
                self.lfos[li].phase += 1.0;
                if self.lfos[li].hold < self.lfos[li].phase {
                    self.lfos[li].from = self.lfos[li].to;
                    let r = self.rand1();
                    let l = self.lfos[li];
                    self.lfos[li].to = r * l.amp + l.offset;
                    let r = self.rand1();
                    let l = &mut self.lfos[li];
                    l.hold = r * l.period + 0.5;
                    l.phase = 0.0;
                }
                let l = self.lfos[li];
                ((l.to - l.from) * l.phase) / l.hold + l.from
            }
            6 | 7 => {
                let l = &mut self.lfos[li];
                l.phase += 1.0;
                if l.period < l.phase {
                    l.phase = l.period;
                }
                if shape == 6 {
                    (l.amp * l.phase) / l.period + l.offset
                } else {
                    (l.amp * (l.period - l.phase)) / l.period + l.offset
                }
            }
            _ => 0.0,
        }
    }

    // --- Scripts ---------------------------------------------------------

    /// `func_80026B90_27790`: counts the script's timer down; at 0 either
    /// performs a pending early release (timer 1) or runs ops until one sets
    /// a nonzero timer.
    fn run_script(&mut self, si: u8, data: &AudioData) {
        let ucd = data.fgm_ucd;
        let rd = |pc: &mut u32| -> u8 {
            let b = ucd.get(*pc as usize).copied().unwrap_or(UCD_PAST_END);
            *pc += 1;
            b
        };
        let varint = |pc: &mut u32| -> u16 {
            let p = rd(pc) as u16;
            if p & 0x80 != 0 {
                rd(pc) as u16 + ((p & 0x7F) << 8)
            } else {
                p
            }
        };
        let s = si as usize;
        let mut t5: i16 = 0;
        let sc = &mut self.scripts[s];
        if sc.timer == 0xFFFF || sc.timer == 0 {
            return;
        }
        sc.timer -= 1;
        if sc.timer != 0 {
            return;
        }
        if sc.release_pending != 0 {
            sc.release_pending = 0;
            let voice = sc.voice;
            sc.voice = NIL;
            sc.timer = 1;
            if voice != NIL {
                self.release_voice(voice);
            }
            return;
        }
        let mut pc = sc.pc;
        let mut timer: u16;
        loop {
            let instr = rd(&mut pc);
            if instr & 0xF8 >= 0xD0 {
                timer = 0;
                match instr {
                    0xD0 => {
                        timer = 0xFFFF;
                        self.release_script_voice(si);
                    }
                    0xD1 => self.scripts[s].artic = varint(&mut pc),
                    0xD2 => {
                        let p = rd(&mut pc);
                        self.scripts[s].unk1e = p & 0x7F;
                        self.scripts[s].early_release = (p & 0x80 != 0) as u8;
                    }
                    0xD3 => self.scripts[s].priority = rd(&mut pc),
                    0xD4 => {
                        for i in 0..6 {
                            self.scripts[s].durations[i] = varint(&mut pc);
                        }
                    }
                    0xD5 => self.scripts[s].volume = rd(&mut pc),
                    0xD6 => {
                        let d = rd(&mut pc) as i8 as i16;
                        self.scripts[s].volume = (d + self.scripts[s].volume as i16).clamp(0, 0xFF) as u8;
                    }
                    0xD7 => self.scripts[s].pan = rd(&mut pc),
                    0xD8 => {
                        let d = rd(&mut pc) as i8 as i16;
                        self.scripts[s].pan = (d + self.scripts[s].pan as i16).clamp(0, 0x7F) as u8;
                    }
                    0xD9 => {
                        let id = varint(&mut pc);
                        self.fork(si, id, data);
                    }
                    0xDA => self.scripts[s].loop_pc = pc,
                    0xDB => pc = self.scripts[s].loop_pc,
                    0xDC => self.scripts[s].fx = rd(&mut pc),
                    0xDD => {
                        let d = rd(&mut pc) as i8 as i16;
                        self.scripts[s].fx = (d + self.scripts[s].fx as i16).clamp(0, 0x7F) as u8;
                    }
                    0xDE => {
                        let group = rd(&mut pc);
                        self.scripts[s].group = group;
                        if group != 0 {
                            let mine = self.scripts[s].priority & 0x7F;
                            let mut at = self.active_scripts;
                            while at != NIL {
                                let o = at as usize;
                                if at != si && self.scripts[o].group == group {
                                    if mine >= self.scripts[o].priority & 0x7F {
                                        self.scripts[o].timer = 0xFFFF;
                                        self.release_script_voice(at);
                                    } else {
                                        timer = 0xFFFF;
                                        self.release_script_voice(si);
                                    }
                                }
                                at = self.scripts[o].next;
                            }
                        }
                    }
                    0xDF => t5 = -0x960,
                    0xE0 => t5 = -0x12C0,
                    _ => {}
                }
            } else {
                timer = match instr & 7 {
                    0 => 0,
                    7 => varint(&mut pc),
                    n => self.scripts[s].durations[n as usize - 1],
                };
                if instr & 0xF8 == 0 {
                    // A rest.
                    self.release_script_voice(si);
                } else {
                    let note = ((instr >> 3) as i32 * 100 - 1300) as i16;
                    let note = note.wrapping_add(t5);
                    let sc = self.scripts[s];
                    if sc.voice != NIL {
                        let vo = &mut self.voices[sc.voice as usize];
                        vo.note = note;
                        t5 = 0;
                        // Force a pitch update.
                        vo.prev_pitch = vo.pitch.wrapping_add(1);
                    } else {
                        let nv = self.spawn_voice(sc.artic, data);
                        self.scripts[s].voice = nv;
                        if nv != NIL {
                            let vo = &mut self.voices[nv as usize];
                            vo.priority = sc.priority;
                            vo.note = note;
                            t5 = 0;
                            vo.vol_scale = ((sc.volume as u32 * sc.game_vol as u32) >> 7) as u8;
                            vo.pan_offset = if sc.game_pan != 0x80 { sc.game_pan } else { sc.pan };
                            vo.fx_scale = if sc.game_fx != 0x80 {
                                sc.game_fx.wrapping_mul(2)
                            } else {
                                sc.fx
                            };
                        }
                    }
                    if timer > 1 && self.scripts[s].early_release != 0 {
                        timer -= 1;
                        self.scripts[s].release_pending = 1;
                    }
                }
            }
            if timer != 0 {
                break;
            }
        }
        self.scripts[s].pc = pc;
        self.scripts[s].timer = timer;
    }

    /// The common `if (0x28 != NULL) { func_8002668C(0x28); 0x28 = NULL; }`.
    fn release_script_voice(&mut self, si: u8) {
        let v = self.scripts[si as usize].voice;
        if v != NIL {
            self.release_voice(v);
            self.scripts[si as usize].voice = NIL;
        }
    }

    /// `func_8002668C_2728C`: timer 0, state 2 (fade out), serial 0.
    fn release_voice(&mut self, v: u8) {
        let vo = &mut self.voices[v as usize];
        vo.timer = 0;
        vo.state = V_FADING;
        vo.serial = 0;
    }

    /// Op D9: copies the script into a free node appended to the end of the
    /// list the script is in, with its own serial, timer 1, the forked
    /// program, no voice, and the root as its parent.
    fn fork(&mut self, si: u8, id: u16, data: &AudioData) {
        if id >= self.ucd_count || id as usize >= data.fgm_ucd_offsets.len() {
            return;
        }
        let n = self.free_scripts;
        if n == NIL {
            self.script_exhausted += 1;
            return;
        }
        self.free_scripts = self.scripts[n as usize].next;
        self.scripts[n as usize] = self.scripts[si as usize];
        self.scripts[n as usize].next = NIL;
        let mut last = si;
        while self.scripts[last as usize].next != NIL {
            last = self.scripts[last as usize].next;
        }
        self.scripts[last as usize].next = n;
        let serial = self.next_script_serial();
        let c = &mut self.scripts[n as usize];
        c.timer = 1;
        c.pc = data.fgm_ucd_offsets[id as usize];
        c.loop_pc = c.pc;
        c.serial = serial;
        c.voice = NIL;
        if c.parent == NIL {
            c.parent = si;
        }
    }

    fn next_script_serial(&mut self) -> u16 {
        self.script_serial = self.script_serial.wrapping_add(1);
        if self.script_serial == 0 {
            self.script_serial = 1;
        }
        self.script_serial
    }

    /// `func_80026B40_27740` + `func_80026A6C_2766C`: a voice for `fgm.tbl`
    /// entry `id`, pushed on the head of the active list in state 3 with
    /// timer 1. Returns `NIL` for a bad id or an empty pool. The previous
    /// values, the wave and LFO fields stay as the node last left them.
    fn spawn_voice(&mut self, id: u16, data: &AudioData) -> u8 {
        if id >= self.tbl_count {
            return NIL;
        }
        let n = self.free_voices;
        if n == NIL {
            self.voice_exhausted += 1;
            return NIL;
        }
        self.free_voices = self.voices[n as usize].next;
        self.voices[n as usize].next = self.active_voices;
        self.active_voices = n;
        self.voice_serial = self.voice_serial.wrapping_add(1);
        if self.voice_serial == 0 {
            self.voice_serial = 1;
        }
        let pc = data.fgm_tbl_offsets[id as usize];
        let vo = &mut self.voices[n as usize];
        vo.timer = 1;
        vo.pc = pc;
        vo.loop_pc = pc;
        vo.state = V_START;
        vo.vol = 0x7F;
        vo.pan = 0x40;
        vo.fx = 0;
        vo.pitch = 0;
        vo.priority = self.default_priority;
        vo.lfos = NIL;
        vo.note = 0;
        vo.vol_scale = 0xFF;
        vo.pan_offset = 0x40;
        vo.fx_scale = 0;
        vo.serial = self.voice_serial;
        n
    }

    fn handle(&self, slot: u8) -> FgmHandle {
        FgmHandle {
            slot,
            serial: self.scripts[slot as usize].serial,
        }
    }

    // --- API -------------------------------------------------------------

    /// `func_800269C0_275C0` (+ `func_80026958_27558`): starts script `id`
    /// at the head of the active list. `None` when `id >= count()` or the
    /// pool is empty. The handle's `slot` is the node.
    pub fn play(&mut self, id: u16) -> Option<FgmHandle> {
        // The C bounds `id` by the count only; `set_count` above the real
        // count would index past the package, so the port also checks it.
        if id >= self.ucd_count || id as usize >= self.ucd_offsets.len() {
            return None;
        }
        let n = self.alloc_script_id(id)?;
        self.link(n);
        Some(self.handle(n))
    }

    /// `lbCommonMakePositionFGM`'s sequence: `func_80026A10_27610`, set
    /// `balance` (`0x2F`, unclamped), then `func_800267F4_273F4` (link at
    /// the head).
    pub fn play_balance(&mut self, id: u16, balance: u8) -> Option<FgmHandle> {
        // The C bounds `id` by the count only; `set_count` above the real
        // count would index past the package, so the port also checks it.
        if id >= self.ucd_count || id as usize >= self.ucd_offsets.len() {
            return None;
        }
        let n = self.alloc_script_id(id)?;
        self.scripts[n as usize].game_pan = balance;
        self.link(n);
        Some(self.handle(n))
    }

    /// `func_80026844_27444`: a script node for `fgm.ucd` entry `id`, not
    /// linked. `0x24` (articulation id) keeps its stale value.
    fn alloc_script_id(&mut self, id: u16) -> Option<u8> {
        let n = self.free_scripts;
        if n == NIL {
            self.script_exhausted += 1;
            return None;
        }
        self.free_scripts = self.scripts[n as usize].next;
        let serial = self.next_script_serial();
        let pc = self.ucd_offsets[id as usize];
        let default_priority = self.default_priority;
        let durations = self.default_durations;
        let s = &mut self.scripts[n as usize];
        s.timer = 1;
        s.pc = pc;
        s.loop_pc = pc;
        s.unk1e = 0x30;
        s.priority = default_priority;
        s.durations = durations;
        s.early_release = 0;
        s.release_pending = 0;
        s.voice = NIL;
        s.parent = NIL;
        s.volume = 0xFF;
        s.pan = 0x40;
        s.fx = 0x40;
        s.game_vol = 0x7F;
        s.game_pan = 0x80;
        s.game_fx = 0x80;
        s.serial = serial;
        s.group = 0;
        Some(n)
    }

    /// `func_800267F4_273F4`.
    fn link(&mut self, n: u8) {
        self.scripts[n as usize].next = self.active_scripts;
        self.active_scripts = n;
    }

    /// `sfx_id != 0 && sfx_id == serial`: the exact sound still exists
    /// (playing or paused).
    pub fn alive(&self, h: FgmHandle) -> bool {
        (h.slot as usize) < SCRIPT_POOL
            && h.serial != 0
            && self.scripts[h.slot as usize].serial == h.serial
    }

    /// `func_80026738_27338` for game code that checks `sfx_id` first
    /// (`wpMainStopFGM`, `ftPublic*`): a no-op unless [`Fgm::alive`].
    pub fn stop(&mut self, syn: &mut Synth, h: FgmHandle) {
        if self.alive(h) {
            self.stop_node(syn, h.slot);
        }
    }

    /// `func_80026738_27338` on a raw node, as `syAudioStopFGM` and the
    /// unchecked game callers do. Walks the *active* list only (a paused
    /// script is not stopped) and stops the node and every script whose fork
    /// root is the node: timer 0, serial 0, its voice released (timer 0,
    /// state 2, serial 0), moved to the free list. The C compares pointers,
    /// so on a stale node it stops whatever now occupies it, plus orphaned
    /// forks of an earlier occupant whose root pointer still names it; this
    /// port does the same. `syn` is unused (the C does not touch the synth).
    pub fn stop_node(&mut self, _syn: &mut Synth, slot: u8) {
        self.stop_matching(slot);
    }

    /// `func_80026738_27338(NULL)`: matches every script whose fork root is
    /// NULL, i.e. stops every active *root* script (forks survive). Callers
    /// like `mnPlayersVS*` pass a NULL `p_sfx` unchecked.
    pub fn stop_null(&mut self, _syn: &mut Synth) {
        self.stop_matching(NIL);
    }

    fn stop_matching(&mut self, target: u8) {
        let mut prev = NIL;
        let mut at = self.active_scripts;
        while at != NIL {
            let s = at as usize;
            let next = self.scripts[s].next;
            if (target != NIL && at == target) || self.scripts[s].parent == target {
                self.scripts[s].timer = 0;
                self.scripts[s].serial = 0;
                let v = self.scripts[s].voice;
                if v != NIL {
                    self.release_voice(v);
                }
                if prev != NIL {
                    self.scripts[prev as usize].next = next;
                } else {
                    self.active_scripts = next;
                }
                self.scripts[s].next = self.free_scripts;
                self.free_scripts = at;
            } else {
                prev = at;
            }
            at = next;
        }
    }

    /// `func_800266A0_272A0`: resume, then stop every active script (timer
    /// and serial 0, voice released) and splice the whole active list, in
    /// order, onto the front of the free list.
    pub fn stop_all(&mut self, syn: &mut Synth) {
        self.resume(syn);
        let mut last = NIL;
        let mut at = self.active_scripts;
        while at != NIL {
            let s = at as usize;
            self.scripts[s].timer = 0;
            self.scripts[s].serial = 0;
            let v = self.scripts[s].voice;
            if v != NIL {
                self.release_voice(v);
            }
            last = at;
            at = self.scripts[s].next;
        }
        if self.active_scripts != NIL {
            self.scripts[last as usize].next = self.free_scripts;
            self.free_scripts = self.active_scripts;
            self.active_scripts = NIL;
        }
    }

    /// `func_80027338`: releases every active voice (timer 0, state 2,
    /// serial 0); scripts keep running.
    pub fn release_all_voices(&mut self) {
        let mut at = self.active_voices;
        while at != NIL {
            self.release_voice(at);
            at = self.voices[at as usize].next;
        }
    }

    /// `func_80026594_27194`: moves voices with priority bit 7 to the paused
    /// voice list (head insertion, so reversed) and silences the playing
    /// ones by setting their pvoice's `em_motion` to `AL_STOPPED` directly;
    /// then moves scripts with priority bit 7 to the paused script list.
    pub fn pause(&mut self, syn: &mut Synth) {
        let mut prev = NIL;
        let mut at = self.active_voices;
        while at != NIL {
            let v = at as usize;
            let next = self.voices[v].next;
            if self.voices[v].priority & 0x80 != 0 {
                if prev != NIL {
                    self.voices[prev as usize].next = next;
                } else {
                    self.active_voices = next;
                }
                if self.voices[v].state == V_PLAYING {
                    if let Some(p) = syn.vvoices[FGM_VOICE_BASE + v].pvoice {
                        syn.pvoices[p as usize].em_motion = AL_STOPPED;
                    }
                }
                self.voices[v].next = self.paused_voices;
                self.paused_voices = at;
            } else {
                prev = at;
            }
            at = next;
        }
        let mut prev = NIL;
        let mut at = self.active_scripts;
        while at != NIL {
            let s = at as usize;
            let next = self.scripts[s].next;
            if self.scripts[s].priority & 0x80 != 0 {
                if prev != NIL {
                    self.scripts[prev as usize].next = next;
                } else {
                    self.active_scripts = next;
                }
                self.scripts[s].next = self.paused_scripts;
                self.paused_scripts = at;
            } else {
                prev = at;
            }
            at = next;
        }
    }

    /// `func_800264A4_270A4`: sets `em_motion = AL_PLAYING` on every paused
    /// playing voice that still has a pvoice and prepends the paused lists,
    /// in their order, to the active lists.
    pub fn resume(&mut self, syn: &mut Synth) {
        let head = self.paused_voices;
        if head != NIL {
            let mut at = head;
            loop {
                let v = at as usize;
                if self.voices[v].state == V_PLAYING {
                    if let Some(p) = syn.vvoices[FGM_VOICE_BASE + v].pvoice {
                        syn.pvoices[p as usize].em_motion = AL_PLAYING;
                    }
                }
                if self.voices[v].next == NIL {
                    break;
                }
                at = self.voices[v].next;
            }
            self.voices[at as usize].next = self.active_voices;
            self.active_voices = head;
            self.paused_voices = NIL;
        }
        let head = self.paused_scripts;
        if head != NIL {
            let mut at = head;
            while self.scripts[at as usize].next != NIL {
                at = self.scripts[at as usize].next;
            }
            self.scripts[at as usize].next = self.active_scripts;
            self.active_scripts = head;
            self.paused_scripts = NIL;
        }
    }

    /// `D_8009EDD0.0x28`: the `fgm.ucd` count that bounds `play`.
    pub fn count(&self) -> u16 {
        self.ucd_count
    }

    pub fn set_count(&mut self, count: u16) {
        self.ucd_count = count;
    }

    /// `func_80026070_26C70`: clamped to 127.
    pub fn set_master_volume(&mut self, volume: u8) {
        self.master_volume = volume.min(127);
    }

    /// `func_80026174_26D74` on a live handle.
    pub fn set_volume(&mut self, h: FgmHandle, v: u8) {
        if self.alive(h) {
            self.set_volume_node(h.slot, v);
        }
    }

    /// `func_80026104_26D04` on a live handle.
    pub fn set_pan(&mut self, h: FgmHandle, v: u8) {
        if self.alive(h) {
            self.set_pan_node(h.slot, v);
        }
    }

    /// `func_80026094_26C94` on a live handle.
    pub fn set_fx(&mut self, h: FgmHandle, v: u8) {
        if self.alive(h) {
            self.set_fx_node(h.slot, v);
        }
    }

    /// `func_80020FFC`'s write of `0x1F` on a live handle.
    pub fn set_priority(&mut self, h: FgmHandle, prio: u8) {
        if self.alive(h) {
            self.set_priority_node(h.slot, prio);
        }
    }

    /// `func_80026174_26D74`: clamps to 127, sets `0x2E` and the voice's
    /// volume scale `(0x22 * v) >> 7` on the node and on every active fork
    /// whose root is the node; the forks use the *root's* `0x22`
    /// (`n_env.c:5376`). No serial check, as in the C.
    pub fn set_volume_node(&mut self, slot: u8, v: u8) {
        let v = v.min(127);
        let s = slot as usize;
        self.scripts[s].game_vol = v;
        let scale = ((self.scripts[s].volume as u32 * v as u32) >> 7) as u8;
        if self.scripts[s].voice != NIL {
            self.voices[self.scripts[s].voice as usize].vol_scale = scale;
        }
        let mut at = self.active_scripts;
        while at != NIL {
            let c = at as usize;
            if self.scripts[c].parent == slot {
                self.scripts[c].game_vol = v;
                if self.scripts[c].voice != NIL {
                    self.voices[self.scripts[c].voice as usize].vol_scale = scale;
                }
            }
            at = self.scripts[c].next;
        }
    }

    /// `func_80026104_26D04`: clamps to 127, sets `0x2F` and the voice's pan
    /// offset on the node and its active forks.
    pub fn set_pan_node(&mut self, slot: u8, v: u8) {
        let v = v.min(127);
        self.apply_to_family(slot, |s, vo| {
            s.game_pan = v;
            if let Some(vo) = vo {
                vo.pan_offset = v;
            }
        });
    }

    /// `func_80026094_26C94`: clamps to 127, sets `0x30` and the voice's fx
    /// scale on the node and its active forks. The voice gets `v`, not the
    /// `2 * v` a note spawned afterwards gets (`n_env.c:5432` vs 4884).
    pub fn set_fx_node(&mut self, slot: u8, v: u8) {
        let v = v.min(127);
        self.apply_to_family(slot, |s, vo| {
            s.game_fx = v;
            if let Some(vo) = vo {
                vo.fx_scale = v;
            }
        });
    }

    fn apply_to_family(&mut self, slot: u8, f: impl Fn(&mut Script, Option<&mut Voice>)) {
        let visit = |fgm: &mut Self, i: usize| {
            let vi = fgm.scripts[i].voice;
            let (scripts, voices) = (&mut fgm.scripts, &mut fgm.voices);
            let vo = if vi != NIL { Some(&mut voices[vi as usize]) } else { None };
            f(&mut scripts[i], vo);
        };
        visit(self, slot as usize);
        let mut at = self.active_scripts;
        while at != NIL {
            let c = at as usize;
            let next = self.scripts[c].next;
            if self.scripts[c].parent == slot {
                visit(self, c);
            }
            at = next;
        }
    }

    /// `func_80020FFC`: writes the node's priority byte (`0x1F`).
    pub fn set_priority_node(&mut self, slot: u8, prio: u8) {
        self.scripts[slot as usize].priority = prio;
    }

    /// The node's `0x10`: `syAudioThreadMain` frees a `syAudioPlayFGM` slot
    /// when it reads 0 (audio.c:1092-1098).
    pub fn timer(&self, slot: u8) -> u16 {
        self.scripts[slot as usize].timer
    }

    // --- Diagnostics -----------------------------------------------------

    /// Scripts on the active list.
    pub fn active_scripts(&self) -> usize {
        let mut n = 0;
        let mut at = self.active_scripts;
        while at != NIL {
            n += 1;
            at = self.scripts[at as usize].next;
        }
        n
    }

    /// Voices on the active list.
    pub fn active_voices(&self) -> usize {
        let mut n = 0;
        let mut at = self.active_voices;
        while at != NIL {
            n += 1;
            at = self.voices[at as usize].next;
        }
        n
    }
}

/// The 8-bit set-or-add of ops 0x00/0x10/0x30: `p <= 127` sets, else adds
/// `p - 192` clamped to 0..=127.
fn set_or_add_7bit(p: u8, cur: u8) -> u8 {
    if p <= 127 {
        p
    } else {
        (p as i16 - 192 + cur as i16).clamp(0, 127) as u8
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;
    use crate::audio::testdata;

    use crate::audio::synth::Synth;

    fn setup() -> Option<(AudioData, Synth, Fgm)> {
        let d = testdata::data()?;
        let mut syn = Synth::new(32006, None);
        let fgm = Fgm::new(&mut syn, &d);
        Some((d, syn, fgm))
    }

    /// fgm 0 (`nSYAudioFGMExplodeS`): `DE 00 D1 07 D2 FF D3 44 D2 21 D5 DC`,
    /// then three notes `1F` (-1000 cents, varint duration) of 20, 30 and
    /// 85 ticks with priorities 0x44, 0x32, 0x14 set between them, then
    /// `D0`. The first note spawns the voice; the next two retune it.
    #[test]
    fn fgm_explode_s_trace() {
        let Some((d, mut syn, mut fgm)) = setup() else { return };
        let h = fgm.play(0).expect("play");
        assert_eq!((h.slot, h.serial), (0, 1));
        let s = h.slot as usize;
        assert_eq!(fgm.scripts[s].timer, 1);
        let mut events = Vec::new();
        let mut last_timer = 1u16;
        for tick in 0..200 {
            fgm.tick(&mut syn, &d);
            let sc = fgm.scripts[s];
            if sc.timer > last_timer || (sc.serial == 0 && last_timer != 0) {
                let note = if sc.voice != NIL {
                    fgm.voices[sc.voice as usize].note
                } else {
                    0
                };
                events.push((tick, sc.timer, sc.priority, note));
            }
            last_timer = if sc.serial == 0 { 0 } else { sc.timer };
            if tick == 0 {
                // The voice spawned this tick with the script's priority,
                // volume scale (0xDC * 0x7F) >> 7 and the default pan.
                let v = fgm.voices[sc.voice as usize];
                assert_eq!(v.priority, 0x44);
                assert_eq!(v.vol_scale, ((0xDC * 0x7F) >> 7) as u8);
                assert_eq!(v.pan_offset, 0x40);
                assert_eq!(v.fx_scale, 0x40);
                assert_eq!(v.state, V_PLAYING, "started in the same tick");
                assert_eq!(fgm.scripts[s].artic, 7);
                assert_eq!(fgm.scripts[s].volume, 0xDC);
            }
        }
        assert_eq!(
            events,
            [(0, 20, 0x44, -1000), (20, 30, 0x32, -1000), (50, 85, 0x14, -1000), (135, 0, 0x14, 0)]
        );
        // Ended at tick 135: reaped (timer and serial cleared), the voice
        // faded that tick and was reaped on the next.
        assert!(!fgm.alive(h));
        assert_eq!(fgm.active_scripts(), 0);
        assert_eq!(fgm.active_voices(), 0);
        assert_eq!(syn.active_voices(), 0);
    }

    /// Both generators are the MSVC `rand` LCG (`0x343FD`, `0x269EC3`) from
    /// seed 1; `randFloat` keeps 16 bits, so its low 15 are MSVC's `rand()`
    /// sequence 41, 18467, 6334, 26500, 19169. `randFloat2` (`sRandomSeed1`)
    /// advances once per tick; `randFloat1` (`sRandomSeed2`) only for LFO
    /// shapes 4/5, and `Fgm::new` resets only `sRandomSeed2`.
    #[test]
    fn fgm_rng_sequences() {
        let Some((d, mut syn, mut fgm)) = setup() else { return };
        let msvc = [41u32, 18467, 6334, 26500, 19169];
        for &want in &msvc {
            let x = fgm.rand1();
            assert_eq!((x * 65536.0) as u32 & 0x7FFF, want);
            assert!((0.0..1.0).contains(&x));
        }
        assert_eq!(fgm.rng_seeds().0, 1, "rand1 leaves sRandomSeed1 alone");
        for &want in &msvc {
            fgm.tick(&mut syn, &d);
            let (s1, _) = fgm.rng_seeds();
            assert_eq!((s1 as u32 >> 16) & 0x7FFF, want);
        }
        let seeds = fgm.rng_seeds();
        let mut syn2 = Synth::new(32006, None);
        let mut again = Fgm::new(&mut syn2, &d);
        assert_eq!(again.rng_seeds(), (1, 1));
        again.set_rng_seed1(seeds.0);
        assert_eq!(again.rng_seeds(), (seeds.0, 1));
    }

    /// Serials skip 0 on wrap, for scripts (`0x4A`) and voices (`0x48`).
    #[test]
    fn fgm_serials_skip_zero() {
        let Some((d, mut syn, mut fgm)) = setup() else { return };
        fgm.script_serial = 0xFFFF;
        fgm.voice_serial = 0xFFFF;
        let h = fgm.play(0).unwrap();
        assert_eq!(h.serial, 1);
        fgm.tick(&mut syn, &d);
        let v = fgm.scripts[h.slot as usize].voice;
        assert_eq!(fgm.voices[v as usize].serial, 1);
    }

    /// Pools: 24 scripts; the 25th play fails and is counted. Stopping by
    /// a stale handle is a no-op; `stop_null` stops only root scripts.
    #[test]
    fn fgm_pools_and_stop() {
        let Some((d, mut syn, mut fgm)) = setup() else { return };
        let hs: Vec<_> = (0..SCRIPT_POOL).map(|_| fgm.play(0).unwrap()).collect();
        // LIFO pool: node 0 first; the newest script heads the active list.
        assert_eq!(hs[0].slot, 0);
        assert_eq!(fgm.active_scripts, (SCRIPT_POOL - 1) as u8);
        assert!(fgm.play(0).is_none());
        assert_eq!(fgm.script_exhausted, 1);
        fgm.stop(&mut syn, hs[3]);
        assert!(!fgm.alive(hs[3]));
        let again = fgm.play(1).unwrap();
        assert_eq!(again.slot, hs[3].slot, "the freed node is reused first");
        // The stale handle must not stop the new occupant.
        fgm.stop(&mut syn, hs[3]);
        assert!(fgm.alive(again));
        fgm.stop_null(&mut syn);
        assert_eq!(fgm.active_scripts(), 0);
        fgm.tick(&mut syn, &d);
        assert_eq!(fgm.active_voices(), 0);
    }

    /// `func_80026594` / `func_800264A4`: only priority-bit-7 nodes move;
    /// paused playing pvoices are stopped in place and resumed later;
    /// paused scripts do not run.
    #[test]
    fn fgm_pause_resume() {
        let Some((d, mut syn, mut fgm)) = setup() else { return };
        let a = fgm.play(0).unwrap();
        let b = fgm.play(0).unwrap();
        fgm.tick(&mut syn, &d);
        // Mark `b` and its voice pausable.
        fgm.scripts[b.slot as usize].priority |= 0x80;
        let bv = fgm.scripts[b.slot as usize].voice;
        fgm.voices[bv as usize].priority |= 0x80;
        fgm.pause(&mut syn);
        assert_eq!(fgm.active_scripts(), 1);
        assert_eq!(fgm.active_voices(), 1);
        let p = syn.vvoices[FGM_VOICE_BASE + bv as usize].pvoice.unwrap();
        assert_eq!(syn.pvoices[p as usize].em_motion, AL_STOPPED);
        let timer = fgm.scripts[b.slot as usize].timer;
        for _ in 0..5 {
            fgm.tick(&mut syn, &d);
        }
        assert_eq!(fgm.scripts[b.slot as usize].timer, timer, "paused script frozen");
        fgm.resume(&mut syn);
        assert_eq!(syn.pvoices[p as usize].em_motion, AL_PLAYING);
        assert_eq!(fgm.active_scripts(), 2);
        assert_eq!(fgm.active_voices(), 2);
        assert_eq!(fgm.active_scripts, b.slot, "paused list goes in front");
        assert!(fgm.alive(a) && fgm.alive(b));
    }

    /// No shipped `fgm.unk` record reads or writes the `spC0` scratch
    /// (postproc 0, no target < 10), so the port's zeroed scratch is exact.
    #[test]
    fn fgm_unk_never_touches_scratch() {
        let Some(d) = testdata::data() else { return };
        for i in 0..d.fgm_unk_count {
            let r = &d.fgm_unk[4 + i * 16..][..16];
            assert!(r[0] <= 8, "shape {}", r[0]);
            assert_eq!(r[2], 0, "postproc of record {i}");
            assert!(r[1] >= 10, "target of record {i}");
        }
    }
}
