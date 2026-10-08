//! libultra's `n_reverb.c` and `n_drvrNew.c` parts of `n_env.c`: the aux
//! bus effect (`ALFx`), built by `n_alFxNew` and run by `n_alFxPull`.
//!
//! The game only ever builds `AL_FX_CUSTOM` from `dSYAudioCustomFXParams`
//! (14 sections, a 19,200-sample delay line, no chorus, one low-pass).

use alloc::vec;
use alloc::vec::Vec;

use super::dsp::{self, Dmem, State16};

/// `SCALE` (`n_drvrNew.c`), the low-pass coefficient scale.
const SCALE: f32 = 16384.0;
const UNITY_PITCH: f32 = 32768.0;
const FIXED_SAMPLE: usize = 184;

/// `ALLowPass`.
#[derive(Debug, Clone)]
struct LowPass {
    fgain: i16,
    coefs: [i16; 16],
    fstate: [i16; 4],
    first: i32,
}

impl LowPass {
    /// `_init_lpfilter`. The coefficient powers are `f64` in the original;
    /// this runs once per effect build.
    fn new(param: i32) -> Self {
        let fc_in = param as i16;
        let temp = (fc_in as f32 * SCALE) as i32;
        let fc = (temp >> 15) as i16;
        let mut coefs = [0i16; 16];
        coefs[8] = fc;
        let ffc = fc as f64 / SCALE as f64;
        let mut fcoef = ffc;
        for c in &mut coefs[9..] {
            fcoef *= ffc;
            *c = (fcoef * SCALE as f64) as i16;
        }
        Self {
            fgain: (SCALE as i32 - fc as i32) as i16,
            coefs,
            fstate: [0; 4],
            first: 1,
        }
    }
}

/// `ALResampler` as a chorus section uses it.
#[derive(Debug, Clone)]
struct ChorusResampler {
    state: State16,
    delta: f32,
    first: i32,
}

/// `ALDelay`.
#[derive(Debug, Clone)]
struct Delay {
    input: u32,
    output: u32,
    ffcoef: i16,
    fbcoef: i16,
    gain: i16,
    rsinc: f32,
    rsval: f32,
    rsdelta: i32,
    rsgain: f32,
    lp: Option<LowPass>,
    rs: Option<ChorusResampler>,
}

/// `ALFx`.
#[derive(Debug, Clone)]
pub struct Fx {
    base: Vec<i16>,
    /// `r->input - r->base`.
    input: usize,
    length: usize,
    delay: Vec<Delay>,
}

/// `AL_FX_*`.
pub const AL_FX_NONE: i32 = 0;
pub const AL_FX_SMALLROOM: i32 = 1;
pub const AL_FX_BIGROOM: i32 = 2;
pub const AL_FX_CHORUS: i32 = 3;
pub const AL_FX_FLANGE: i32 = 4;
pub const AL_FX_ECHO: i32 = 5;
pub const AL_FX_CUSTOM: i32 = 6;

/// `n_alFxNew`'s parameter table for `fx_type`: `presets` is the ROM's
/// `SMALLROOM_PARAMS_N` .. `NULL_PARAMS_N` block (26, 34, 10, 10, 10, 10
/// words).
pub fn params_for<'a>(fx_type: i32, custom: &'a [i32], presets: &'a [i32]) -> &'a [i32] {
    match fx_type {
        AL_FX_SMALLROOM => &presets[0..26],
        AL_FX_BIGROOM => &presets[26..60],
        AL_FX_ECHO => &presets[60..70],
        AL_FX_CHORUS => &presets[70..80],
        AL_FX_FLANGE => &presets[80..90],
        AL_FX_CUSTOM => custom,
        _ => &presets[90..100],
    }
}

impl Fx {
    /// `n_alFxNew`.
    pub fn new(param: &[i32], output_rate: i32) -> Self {
        let mut j = 0;
        let mut next = || {
            let v = param.get(j).copied().unwrap_or(0);
            j += 1;
            v
        };
        let section_count = next() as u8;
        let length = next() as u32 as usize;
        let mut delay = Vec::with_capacity(section_count as usize);
        for _ in 0..section_count {
            let input = next() as u32;
            let output = next() as u32;
            let fbcoef = next() as i16;
            let ffcoef = next() as i16;
            let gain = next() as i16;
            let rate = next();
            let depth = next();
            let mut d = Delay {
                input,
                output,
                ffcoef,
                fbcoef,
                gain,
                rsinc: 0.0,
                rsval: 0.0,
                rsdelta: 0,
                rsgain: 0.0,
                lp: None,
                rs: None,
            };
            if rate != 0 {
                const RANGE: f32 = 2.0;
                const CONVERT: f32 = 173_123.4;
                d.rsinc = ((rate as f32 / 1000.0) * RANGE) / output_rate as f32;
                d.rsgain = (depth as f32 / CONVERT) * output.wrapping_sub(input) as f32;
                d.rsval = 1.0;
                d.rsdelta = 0;
                d.rs = Some(ChorusResampler {
                    state: [0; 16],
                    delta: 0.0,
                    first: 1,
                });
            }
            let fc = next();
            if fc != 0 {
                d.lp = Some(LowPass::new(fc));
            }
            delay.push(d);
        }
        Self {
            base: vec![0; length],
            input: 0,
            length,
            delay,
        }
    }

    /// Delay-line index `input + off`, as the pointer arithmetic gives it
    /// before `_n_loadBuffer`/`_n_saveBuffer` wrap it.
    fn at(&self, off: isize) -> isize {
        self.input as isize + off
    }

    /// `_n_loadBuffer`: `count` samples from delay index `curr` to `buff`.
    fn load_buffer(&self, dmem: &mut Dmem, mut curr: isize, buff: usize, count: usize) {
        let len = self.length as isize;
        if curr < 0 {
            curr += len;
        }
        let curr = curr as usize;
        // `A_LOADBUFF` moves whole 8-byte units.
        let words = |n: usize| n.next_multiple_of(4);
        if curr + count > self.length {
            let before = self.length - curr;
            let after = curr + count - self.length;
            let b = words(before).min(self.length - curr);
            dmem.load(buff, &self.base[curr..curr + b]);
            dmem.load(buff + (before << 1), &self.base[..words(after).min(self.length)]);
        } else {
            let n = words(count).min(self.length - curr);
            dmem.load(buff, &self.base[curr..curr + n]);
        }
    }

    /// `_n_saveBuffer`: 184 samples from `buff` to delay index `curr`.
    fn save_buffer(&mut self, dmem: &Dmem, mut curr: isize, buff: usize) {
        let len = self.length as isize;
        if curr < 0 {
            curr += len;
        }
        let curr = curr as usize;
        if curr + FIXED_SAMPLE > self.length {
            let before = self.length - curr;
            let after = curr + FIXED_SAMPLE - self.length;
            dmem.save(buff, &mut self.base[curr..curr + before]);
            dmem.save(buff + (before << 1), &mut self.base[..after]);
        } else {
            dmem.save(buff, &mut self.base[curr..curr + FIXED_SAMPLE]);
        }
    }

    /// `n_alFxPull` after `n_alAuxBusPull`: mixes the wet buses to mono,
    /// writes the delay line, runs every section into `AUX_R`, then copies
    /// it to `AUX_L`.
    pub fn pull(&mut self, dmem: &mut Dmem, table: &[[i16; 4]; 64]) {
        let input = dsp::AUX_L;
        let output = dsp::AUX_R;
        let buff1 = dsp::TEMP_0;
        let buff2 = dsp::TEMP_1;
        dmem.mix(0xDA83u16 as i16, dsp::AUX_L, input);
        dmem.mix(0x5A82, dsp::AUX_R, input);
        self.save_buffer(dmem, self.input as isize, input);
        dmem.clear(output, FIXED_SAMPLE << 1);
        for i in 0..self.delay.len() {
            let (d_in, d_out) = (self.delay[i].input, self.delay[i].output);
            let in_ptr = self.at(-(d_in as i32 as isize));
            let out_ptr = self.at(-(d_out as i32 as isize));
            // `prev_out_ptr` is `&r->input[+d->output]` in the original, so
            // the SWAP shortcut never fires: always load.
            self.load_buffer(dmem, in_ptr, buff1, FIXED_SAMPLE);
            self.load_output_buffer(dmem, i, buff2, table);
            let d = &self.delay[i];
            let (ff, fb, gain) = (d.ffcoef, d.fbcoef, d.gain);
            let (has_rs, has_lp) = (d.rs.is_some(), d.lp.is_some());
            if ff != 0 {
                dmem.mix(ff, buff1, buff2);
                if !has_rs && !has_lp {
                    self.save_buffer(dmem, out_ptr, buff2);
                }
            }
            if fb != 0 {
                dmem.mix(fb, buff2, buff1);
                self.save_buffer(dmem, in_ptr, buff1);
            }
            if let Some(lp) = &mut self.delay[i].lp {
                // `_n_filterBuffer`: n_aLoadADPCM of the 16 coefficients,
                // then n_aPoleFilter in place.
                dmem.polef(lp.first as u32, lp.fgain as u16, &lp.coefs, &mut lp.fstate, buff2);
                lp.first = 0;
            }
            if !has_rs {
                self.save_buffer(dmem, out_ptr, buff2);
            }
            if gain != 0 {
                dmem.mix(gain, buff2, output);
            }
        }
        self.input += FIXED_SAMPLE;
        if self.input > self.length {
            self.input -= self.length;
        }
        dmem.dmem_move(output, dsp::AUX_L, FIXED_SAMPLE << 1);
    }

    /// `_n_loadOutputBuffer`.
    fn load_output_buffer(&mut self, dmem: &mut Dmem, i: usize, buff: usize, table: &[[i16; 4]; 64]) {
        if self.delay[i].rs.is_none() {
            let out_ptr = self.at(-(self.delay[i].output as i32 as isize));
            self.load_buffer(dmem, out_ptr, buff, FIXED_SAMPLE);
            return;
        }
        let incount = FIXED_SAMPLE as i32;
        let rbuff = dsp::TEMP_2;
        let d = &mut self.delay[i];
        let length = d.output.wrapping_sub(d.input) as i32;
        // `_doModFunc`.
        d.rsval += d.rsinc * incount as f32;
        if d.rsval > 2.0 {
            d.rsval -= 4.0;
        }
        let val = d.rsval.abs() - 1.0;
        let mut delta = d.rsgain * val;
        delta /= length as f32;
        delta = (delta * UNITY_PITCH) as i32 as f32;
        delta /= UNITY_PITCH;
        let fratio = 1.0 - delta;
        let rs = d.rs.as_mut().expect("chorus section");
        let fincount = rs.delta + fratio * incount as f32;
        let count = fincount as i32;
        rs.delta = fincount - count as f32;
        let off = -(d.output.wrapping_sub(d.rsdelta as u32) as i32 as isize);
        let out_ptr = self.at(off);
        // The heap gives the delay line 16-byte alignment, so the RDRAM
        // misalignment is the index's.
        let ramalign = out_ptr.rem_euclid(4);
        self.load_buffer(
            dmem,
            out_ptr - ramalign,
            rbuff,
            (count as isize + ramalign) as usize,
        );
        let ratio = (fratio * UNITY_PITCH) as i32;
        let d = &mut self.delay[i];
        let rs = d.rs.as_mut().expect("chorus section");
        let out = if buff >> 8 != 0 { dsp::TEMP_1 } else { dsp::TEMP_0 };
        dmem.resample(
            rs.first as u32,
            ratio as u16,
            &mut rs.state,
            rbuff + ((ramalign as usize) << 1),
            out,
            table,
        );
        rs.first = 0;
        d.rsdelta += count - incount;
    }
}
