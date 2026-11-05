//! libultra's `n_reverb.c` and `n_drvrNew.c` parts of `n_env.c`: the aux
//! bus effect (`ALFx`), built by `n_alFxNew` and run by `n_alFxPull`.
//!
//! The game only ever builds `AL_FX_CUSTOM` from `dSYAudioCustomFXParams`
//! (14 sections, a 19,200-sample delay line, no chorus, one low-pass).

use alloc::vec;
use alloc::vec::Vec;

use super::dsp::{self, Dmem, State16};
use super::mem;

/// `SCALE` (`n_drvrNew.c`), the low-pass coefficient scale.
const SCALE: f32 = 16384.0;
const UNITY_PITCH: f32 = 32768.0;
const FIXED_SAMPLE: usize = 184;

/// Whether `_n_filterBuffer`'s `n_aPoleFilter` changes the section's
/// output. In `n_aspMain` (N_MICRO) the command carries no buffer size and
/// names its DMEM buffer only as `buff >> 8` (`TEMP_1` = 368 -> 1). The
/// N64 reference capture (mupen64plus HLE audio, /tmp/ssb-audio/n64,
/// S-n64-compare.md) matches the port sample for sample only when the
/// low-pass leaves the saved output untouched; with the filter applied the
/// reverb return differs from 0.2 s on (corr 0.988). The port follows the
/// reference; real hardware is unverified (D-049).
const APPLY_LOWPASS: bool = false;

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

/// A platform kernel for one plain section, bit-identical to [`section`]
/// (`coef` = `[ff, fb, gain]`; a zero coefficient's mix is not issued). [`Fx::pull_fused`] calls it only for
/// windows that start 16-byte aligned and run a multiple of 8 samples.
///
/// # Safety
///
/// `x`, `y` and `out` each point at `n` valid, 16-byte-aligned, mutually
/// disjoint samples; `n` is a multiple of 8.
pub type SectionKernel =
    unsafe fn(x: *mut i16, y: *mut i16, out: *mut i16, n: usize, coef: [i32; 3]);

/// A `T` in cache lines of its own: what another processor writes back
/// never covers a neighbour's bytes.
#[derive(Debug, Clone)]
#[repr(C, align(64))]
struct Own<T>(T);

impl<T> core::ops::Deref for Own<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T> core::ops::DerefMut for Own<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

/// `ALFx`.
#[derive(Debug, Clone)]
pub struct Fx {
    /// The delay line, `length` samples from `org`: allocated with slack so
    /// the line starts 16-byte aligned for [`SectionKernel`].
    base: Vec<i16>,
    org: usize,
    /// [`Fx::set_kernel`].
    kernel: Option<SectionKernel>,
    /// `r->input - r->base`.
    input: usize,
    length: usize,
    delay: Vec<Own<Delay>>,
    /// Whether [`Fx::pull_fused`] may run this effect (see there).
    fused: bool,
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
            delay.push(Own(d));
        }
        // The fused path needs plain sections (no chorus; the low-pass
        // never changes the output, `APPLY_LOWPASS`) whose input and output
        // windows never overlap, and no window longer than the line.
        let fused = !APPLY_LOWPASS
            && length >= FIXED_SAMPLE
            && delay.iter().all(|d| {
                let (i, o) = (d.input as usize, d.output as usize);
                let gap = |a: usize, b: usize| (b + length - a % length) % length;
                d.rs.is_none()
                    && i < length
                    && o < length
                    && gap(i, o) >= FIXED_SAMPLE
                    && gap(o, i) >= FIXED_SAMPLE
            });
        // 64-byte alignment (16 for `SectionKernel`, 64 so another
        // processor can own the line's cache lines): at most 31 samples of
        // slack in front, at most 31 behind to fill the last line.
        let base = vec![0; length + 62];
        let org = (base.as_ptr() as usize).wrapping_neg() % 64 / 2;
        Self {
            base,
            org,
            kernel: None,
            input: 0,
            length,
            delay,
            fused,
        }
    }

    /// Installs (or removes) the platform kernel [`Fx::pull_fused`] runs
    /// its sections with.
    pub fn set_kernel(&mut self, kernel: Option<SectionKernel>) {
        self.kernel = kernel;
    }

    /// The delay line.
    fn line(&self) -> &[i16] {
        &self.base[self.org..self.org + self.length]
    }

    fn line_mut(&mut self) -> &mut [i16] {
        let org = self.org;
        &mut self.base[org..org + self.length]
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
            dmem.load(buff, &self.line()[curr..curr + b]);
            dmem.load(
                buff + (before << 1),
                &self.line()[..words(after).min(self.length)],
            );
        } else {
            let n = words(count).min(self.length - curr);
            dmem.load(buff, &self.line()[curr..curr + n]);
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
            dmem.save(buff, &mut self.line_mut()[curr..curr + before]);
            dmem.save(buff + (before << 1), &mut self.line_mut()[..after]);
        } else {
            dmem.save(buff, &mut self.line_mut()[curr..curr + FIXED_SAMPLE]);
        }
    }

    /// `n_alFxPull` after `n_alAuxBusPull`: mixes the wet buses to mono,
    /// writes the delay line, runs every section into `AUX_R`, then copies
    /// it to `AUX_L`.
    pub fn pull(&mut self, dmem: &mut Dmem, table: &[[i16; 4]; 64]) {
        if self.fused {
            self.pull_fused(dmem);
            return;
        }
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
            if let Some(lp) = self.delay[i].lp.as_mut().filter(|_| APPLY_LOWPASS) {
                // `_n_filterBuffer`: n_aLoadADPCM of the 16 coefficients,
                // then n_aPoleFilter in place.
                dmem.polef(
                    lp.first as u32,
                    lp.fgain as u16,
                    &lp.coefs,
                    &mut lp.fstate,
                    buff2,
                );
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

    /// [`Fx::pull`] for plain sections, computed in place on the delay
    /// line in one pass per section. The original loads each section's
    /// input and output windows into DMEM, mixes `ff` into the output, `fb`
    /// into the input, saves both back and mixes `gain` of the output into
    /// the bus. With the two windows disjoint (checked in [`Fx::new`]) every
    /// sample's arithmetic only reads its own two delay-line samples, so
    /// doing it in place, sample by sample, gives the same line and bus.
    fn pull_fused(&mut self, dmem: &mut Dmem) {
        let len = self.length;
        let base = &mut self.base[self.org..self.org + len];
        let kernel = self.kernel;
        // The wet buses to mono, written to the line at `input`:
        // n_aMix(0xDA83, AUX_L -> AUX_L), n_aMix(0x5A82, AUX_R -> AUX_L).
        {
            let (l, r) = dmem.w[dsp::AUX_L / 2..].split_at_mut(FIXED_SAMPLE);
            let r = &r[..FIXED_SAMPLE];
            let l = &mut l[..FIXED_SAMPLE];
            let g1 = 0xDA83u16 as i16 as i32;
            for k in 0..FIXED_SAMPLE {
                let a = l[k] as i32;
                let a = dsp::mixq(a, a, g1) as i32;
                l[k] = dsp::mixq(a, r[k] as i32, 0x5A82);
            }
            let at = self.input;
            let first = (len - at).min(FIXED_SAMPLE);
            mem::copy(&mut base[at..at + first], &l[..first]);
            mem::copy(&mut base[..FIXED_SAMPLE - first], &l[first..]);
        }
        // The output bus, AUX_R, cleared first.
        let (aux_l, out) = dmem.w[dsp::AUX_L / 2..].split_at_mut(FIXED_SAMPLE);
        let out = &mut out[..FIXED_SAMPLE];
        mem::zero(out);
        for d in &self.delay {
            let (ff, fb, gain) = (d.ffcoef as i32, d.fbcoef as i32, d.gain as i32);
            // `r->input - d->input`, wrapped as `_n_loadBuffer` wraps it.
            let mut xi = (self.input + len - d.input as usize) % len;
            let mut yi = (self.input + len - d.output as usize) % len;
            let mut k = 0;
            while k < FIXED_SAMPLE {
                let n = (FIXED_SAMPLE - k).min(len - xi).min(len - yi);
                let (x, y) = if xi < yi {
                    let (lo, hi) = base.split_at_mut(yi);
                    (&mut lo[xi..xi + n], &mut hi[..n])
                } else {
                    let (lo, hi) = base.split_at_mut(xi);
                    (&mut hi[..n], &mut lo[yi..yi + n])
                };
                let o = &mut out[k..k + n];
                match kernel {
                    Some(f) if fits(x, y, o) => {
                        // SAFETY: `fits` checked the alignment and length;
                        // the windows are disjoint (`Fx::new`) and `out` is
                        // DMEM.
                        unsafe {
                            f(
                                x.as_mut_ptr(),
                                y.as_mut_ptr(),
                                o.as_mut_ptr(),
                                n,
                                [ff, fb, gain],
                            )
                        }
                    }
                    _ => section(x, y, o, ff, fb, gain),
                }
                k += n;
                xi = (xi + n) % len;
                yi = (yi + n) % len;
            }
        }
        self.input += FIXED_SAMPLE;
        if self.input > self.length {
            self.input -= self.length;
        }
        // The output bus is then moved to AUX_L.
        mem::copy(&mut aux_l[..FIXED_SAMPLE], out);
    }

    /// `_n_loadOutputBuffer`.
    fn load_output_buffer(
        &mut self,
        dmem: &mut Dmem,
        i: usize,
        buff: usize,
        table: &[[i16; 4]; 64],
    ) {
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
        let out = if buff >> 8 != 0 {
            dsp::TEMP_1
        } else {
            dsp::TEMP_0
        };
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

/// Whether [`SectionKernel`] may run on these windows: each starts
/// 16-byte aligned and all run the same multiple of 8 samples.
fn fits(x: &[i16], y: &[i16], out: &[i16]) -> bool {
    let a = |s: &[i16]| (s.as_ptr() as usize).is_multiple_of(16);
    let n = out.len();
    n.is_multiple_of(8) && x.len() == n && y.len() == n && a(x) && a(y) && a(out)
}

/// One plain section over disjoint windows, sample by sample: `y = mixq(y,
/// x, ff)`, then `x = mixq(x, y, fb)`, then `out = mixq(out, y, gain)` (each
/// an `n_aMix`, issued only for a nonzero coefficient).
#[inline(always)]
fn section(x: &mut [i16], y: &mut [i16], out: &mut [i16], ff: i32, fb: i32, gain: i32) {
    #[cfg(target_arch = "mips")]
    return dsp::allegrex::section(x, y, out, ff, fb, gain);
    #[cfg(not(target_arch = "mips"))]
    {
        let n = out.len();
        let (x, y) = (&mut x[..n], &mut y[..n]);
        // A zero coefficient's mix is not issued: with the RSP's rounding
        // (`dsp::mixq`) a gain-0 mix is not the identity.
        for k in 0..n {
            let yv = if ff != 0 {
                dsp::mixq(y[k] as i32, x[k] as i32, ff)
            } else {
                y[k]
            } as i32;
            y[k] = yv as i16;
            if fb != 0 {
                x[k] = dsp::mixq(x[k] as i32, yv, fb);
            }
            if gain != 0 {
                out[k] = dsp::mixq(out[k] as i32, yv, gain);
            }
        }
    }
}

/// The PSP's VFPU section kernel (`psp-runtime`'s `reverb_vfpu`) step for
/// step in `f32`: every intermediate is exact, so these tests prove its
/// arithmetic equals [`dsp::mixq`] for every input.
///
/// One mix, `mixq(o, i, g) = clamp16(o + floor(T / 2^15))` with
/// `T = i * g - o + 2^14`. With `g = 256 * gh + gl` (`gh = g >> 8`, `gl = g & 255`):
/// `T / 2^15 = a + b + c`, `a = i * gh / 128` (|i * gh| <= 2^22: exact),
/// `b = i * gl / 2^15` (|i * gl| < 2^23: exact), `c = 1/2 - o / 2^15`
/// (17 significant bits). `q = floor(a)`, `fa = a - q` (in [0, 1), 7
/// fraction bits), `w = fa + (b + c)` (|w| < 258, 15 fraction bits: 24
/// bits), so `floor(T / 2^15) = q + floor(w)` and every step is exact in
/// single precision whatever the rounding mode.
#[cfg(test)]
pub(crate) mod vfpu_model {
    use core::sync::atomic::{AtomicUsize, Ordering};

    /// Calls to [`kernel`], so a test can show it ran.
    pub(crate) static CALLS: AtomicUsize = AtomicUsize::new(0);

    /// `g` split as the kernel's constants: `gh / 128`, `gl / 2^15`.
    pub(crate) fn split(g: i32) -> (f32, f32) {
        ((g >> 8) as f32 / 128.0, (g & 255) as f32 / 32768.0)
    }

    /// One mix: `of` = `o`, `c` = `1/2 - o / 2^15`, `i` the input.
    pub(crate) fn mix(of: f32, c: f32, i: f32, (gh, gl): (f32, f32)) -> f32 {
        let a = i * gh; // vscl
        let q = a.floor(); // vf2id + vi2f
        let fa = a - q; // vsub
        let bc = i * gl + c; // vscl, vadd
        let w = (fa + bc).floor(); // vadd, vf2id + vi2f
        let r = of + q + w; // vadd, vadd
        r.clamp(-32768.0, 32767.0) // vmin, vmax
    }

    /// `o` as the kernel converts it: `(o as f32, 1/2 - o / 2^15)`.
    pub(crate) fn load(o: i16) -> (f32, f32) {
        let v = (o as i32) << 16; // vs2i
        (v as f32 / 65536.0, 0.5 - v as f32 / 2_147_483_648.0) // vi2f 16, 31
    }

    /// [`super::SectionKernel`] in `f32`.
    ///
    /// # Safety
    ///
    /// As [`super::SectionKernel`].
    pub(crate) unsafe fn kernel(x: *mut i16, y: *mut i16, out: *mut i16, n: usize, coef: [i32; 3]) {
        CALLS.fetch_add(1, Ordering::Relaxed);
        assert!(
            n.is_multiple_of(8) && [x, y, out].iter().all(|p| (*p as usize).is_multiple_of(16))
        );
        let [ff, fb, gain] = coef.map(split);
        for k in 0..n {
            // SAFETY: the caller's contract.
            let (x, y, out) = unsafe { (&mut *x.add(k), &mut *y.add(k), &mut *out.add(k)) };
            let ((xf, cx), (yf, cy), (of, co)) = (load(*x), load(*y), load(*out));
            // A zero coefficient's mix is skipped, as in `section`.
            let yn = if coef[0] != 0 {
                mix(yf, cy, xf, ff)
            } else {
                yf
            };
            let xn = if coef[1] != 0 {
                mix(xf, cx, yn, fb)
            } else {
                xf
            };
            let on = if coef[2] != 0 {
                mix(of, co, yn, gain)
            } else {
                of
            };
            // vf2iz 16, vi2s
            (*y, *x, *out) = (yn as i16, xn as i16, on as i16);
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::audio::dsp::mixq;

        /// The custom effect's coefficients, the RSP gains' extremes and
        /// both sides of every split boundary.
        const GAINS: [i32; 26] = [
            -32768,
            -32767,
            -9830,
            -5000,
            -3276,
            -257,
            -256,
            -255,
            -129,
            -128,
            -1,
            1,
            127,
            128,
            255,
            256,
            257,
            255 + 256,
            1535,
            2559,
            3276,
            4095,
            5000,
            8191,
            9830,
            32767,
        ];
        const OUTS: [i16; 12] = [
            -32768, -32767, -16385, -16384, -1, 0, 1, 255, 16383, 16384, 32766, 32767,
        ];

        #[test]
        fn mix_matches_mixq_for_every_input() {
            for &g in &GAINS {
                let s = split(g);
                for &o in &OUTS {
                    let (of, c) = load(o);
                    for i in i16::MIN..=i16::MAX {
                        let want = mixq(o as i32, i as i32, g);
                        let got = mix(of, c, i as f32, s);
                        assert_eq!(got, want as f32, "o={o} i={i} g={g}");
                    }
                }
            }
        }

        #[test]
        fn mix_matches_mixq_random() {
            let mut r = 0x1234_5678u32;
            let mut next = || {
                r ^= r << 13;
                r ^= r >> 17;
                r ^= r << 5;
                r
            };
            for _ in 0..4_000_000 {
                let v = next();
                let (o, i) = (v as i16, (v >> 16) as i16);
                let g = next() as i16 as i32;
                let (of, c) = load(o);
                let want = mixq(o as i32, i as i32, g);
                assert_eq!(
                    mix(of, c, i as f32, split(g)),
                    want as f32,
                    "o={o} i={i} g={g}"
                );
            }
        }

        /// The kernel equals [`super::super::section`] on random windows.
        #[test]
        fn kernel_matches_section() {
            #[repr(C, align(16))]
            struct W([i16; 64]);
            let mut r = 0x9E37_79B9u32;
            let mut next = || {
                r ^= r << 13;
                r ^= r >> 17;
                r ^= r << 5;
                r as i16
            };
            for coef in [
                [-3276, 3276, 8191],
                [-9830, 9830, 0],
                [0, 5000, 0],
                [-32768, 32767, -32768],
            ] {
                for _ in 0..2000 {
                    let mut v = [W([0; 64]), W([0; 64]), W([0; 64])];
                    for w in &mut v {
                        w.0 = core::array::from_fn(|_| next());
                    }
                    let mut want = [v[0].0, v[1].0, v[2].0];
                    let [a, b, c] = &mut want;
                    super::super::section(a, b, c, coef[0], coef[1], coef[2]);
                    let [x, y, o] = &mut v;
                    // SAFETY: three aligned, disjoint 64-sample windows.
                    unsafe {
                        kernel(
                            x.0.as_mut_ptr(),
                            y.0.as_mut_ptr(),
                            o.0.as_mut_ptr(),
                            64,
                            coef,
                        )
                    };
                    assert_eq!([v[0].0, v[1].0, v[2].0], want, "coef={coef:?}");
                }
            }
        }
    }
}
