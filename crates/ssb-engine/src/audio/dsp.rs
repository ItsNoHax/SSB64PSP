//! The audio microcode's commands (`n_aspMain`, the N_MICRO ABI), run
//! directly on an emulated DMEM workspace instead of being packed into an
//! `Acmd` list.
//!
//! `n_aspMain` itself is not decompiled. The numeric behaviour follows
//! BattleShip's scalar interpreter (`port/audio/mixer.c`, MIT, derived
//! from Starship), which follows the mupen64plus rsp-hle algorithms (used
//! here only as a description; no code from rsp-hle). Buffer addresses are
//! byte offsets in the N_MICRO workspace (DMEM 0x4F0 onwards), as
//! `n_synthInternals.h`'s `N_AL_*` constants name them.

/// `N_AL_DECODER_IN` / `N_AL_RESAMPLER_OUT` / `N_AL_TEMP_0`.
pub const TEMP_0: usize = 0;
/// `N_AL_DECODER_OUT` / `N_AL_TEMP_1`.
pub const TEMP_1: usize = 368;
pub const TEMP_2: usize = 736;
pub const MAIN_L: usize = 1248;
pub const MAIN_R: usize = 1616;
pub const AUX_L: usize = 1984;
pub const AUX_R: usize = 2352;
/// `N_AL_DIVIDED`: one clear of 2 x 368 bytes covers a bus's L and R.
pub const DIVIDED: usize = 368;
/// The N_MICRO fixed command length: 184 samples (`0x170` bytes).
pub const COUNT: usize = 368;

/// Workspace words. DMEM holds 0x1000 - 0x4F0 = 2832 bytes of workspace;
/// the looped ADPCM path can place a chunk past that on long loops, so the
/// array is larger instead of wrapping into the microcode's own data.
pub const WORDS: usize = 2048;
const _: () = assert!(WORDS.is_power_of_two());

pub const A_INIT: u32 = 1;
pub const A_LOOP: u32 = 2;

/// 16 history samples: `ADPCM_STATE` / `RESAMPLE_STATE`.
pub type State16 = [i16; 16];

#[inline(always)]
fn clamp16(v: i32) -> i16 {
    v.clamp(i16::MIN as i32, i16::MAX as i32) as i16
}

#[inline(always)]
fn round_up(n: usize, a: usize) -> usize {
    (n + a - 1) & !(a - 1)
}

/// `ENVMIX_STATE`, as the interpreter keeps it between calls.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EnvMixState {
    pub wet: i16,
    pub dry: i16,
    /// Q16.16 ramps: left, right.
    pub value: [i32; 2],
    pub target: [i32; 2],
    pub step: [i32; 2],
}

/// The `A_SETVOL` registers an `A_INIT` envmixer reads.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EnvMixInit {
    pub vol: [i16; 2],
    pub tgt: [i16; 2],
    pub rate_m: [i16; 2],
    pub rate_l: [u16; 2],
    pub dry: i16,
    pub wet: i16,
}

/// The emulated workspace.
pub struct Dmem {
    pub w: [i16; WORDS],
}

impl Default for Dmem {
    fn default() -> Self {
        Self { w: [0; WORDS] }
    }
}

impl Dmem {
    #[inline]
    fn words(&mut self, addr: usize, nbytes: usize) -> &mut [i16] {
        &mut self.w[addr / 2..(addr + nbytes) / 2]
    }

    #[inline]
    pub fn get(&self, addr: usize, nbytes: usize) -> &[i16] {
        &self.w[addr / 2..(addr + nbytes) / 2]
    }

    /// `A_CLEARBUFF`.
    pub fn clear(&mut self, addr: usize, nbytes: usize) {
        let n = round_up(nbytes, 16);
        self.words(addr, n).fill(0);
    }

    /// `A_DMEMMOVE` (memmove semantics).
    pub fn dmem_move(&mut self, src: usize, dst: usize, nbytes: usize) {
        let n = round_up(nbytes, 16) / 2;
        self.w.copy_within(src / 2..src / 2 + n, dst / 2);
    }

    /// `A_LOADBUFF` from an `s16` DRAM buffer (the reverb delay line);
    /// `src` holds exactly the words to load.
    pub fn load(&mut self, dst: usize, src: &[i16]) {
        self.w[dst / 2..dst / 2 + src.len()].copy_from_slice(src);
    }

    /// `A_SAVEBUFF` to an `s16` DRAM buffer.
    pub fn save(&self, src: usize, dst: &mut [i16]) {
        dst.copy_from_slice(&self.w[src / 2..src / 2 + dst.len()]);
    }

    /// `A_ADPCM` (4-bit mode; n_env never sets the 2-bit flag). Writes the
    /// 16 history samples at `out`, then `round_up_32(nbytes)` bytes of
    /// decoded samples, reading 9-byte frames from `input` (bytes past its
    /// end read as 0). `book` is `[pred][2][8]`.
    #[allow(clippy::too_many_arguments)]
    pub fn adpcm(
        &mut self,
        flags: u32,
        state: &mut State16,
        loop_state: &State16,
        book: &[i16],
        input: &[u8],
        out: usize,
        nbytes: usize,
    ) {
        let frames = round_up(nbytes, 32) / 32;
        let base = out / 2;
        let end = base + 16 + frames * 16;
        let o = &mut self.w[base..end];
        if flags & A_INIT != 0 {
            o[..16].fill(0);
        } else if flags & A_LOOP != 0 {
            o[..16].copy_from_slice(loop_state);
        } else {
            o[..16].copy_from_slice(state);
        }
        let mut pos = 16;
        for f in 0..frames {
            let mut frame = [0u8; 9];
            if let Some(src) = input.get(f * 9..f * 9 + 9) {
                frame.copy_from_slice(src);
            } else if let Some(src) = input.get(f * 9..) {
                frame[..src.len()].copy_from_slice(src);
            }
            let shift = (frame[0] >> 4) as u32;
            let pred = (frame[0] & 15) as usize;
            // Scale 12 means x4096 (rshift 0), not silence.
            let rshift = 12u32.saturating_sub(shift);
            let c = book.get(pred * 16..pred * 16 + 16).unwrap_or(&[0; 16]);
            let (c0, c1) = c.split_at(8);
            for half in 0..2 {
                let mut ins = [0i32; 8];
                for (j, b) in frame[1 + half * 4..5 + half * 4].iter().enumerate() {
                    ins[j * 2] = (((*b as u16 & 0xF0) << 8) as i16 >> rshift) as i32;
                    ins[j * 2 + 1] = (((*b as u16 & 0x0F) << 12) as i16 >> rshift) as i32;
                }
                let prev1 = o[pos - 1] as i32;
                let prev2 = o[pos - 2] as i32;
                for j in 0..8 {
                    let mut acc = c0[j] as i32 * prev2 + c1[j] as i32 * prev1 + (ins[j] << 11);
                    for k in 0..j {
                        acc += c1[j - k - 1] as i32 * ins[k];
                    }
                    o[pos + j] = clamp16(acc >> 11);
                }
                pos += 8;
            }
        }
        state.copy_from_slice(&o[pos - 16..pos]);
    }

    /// `A_RESAMPLE`: 184 output samples at `out` from `input`, through the
    /// 64-phase, 4-tap `table`. Writes its 4 history samples just below
    /// `input`, as the microcode does.
    pub fn resample(
        &mut self,
        flags: u32,
        pitch: u16,
        state: &mut State16,
        input: usize,
        out: usize,
        table: &[[i16; 4]; 64],
    ) {
        let mut tmp: State16 = *state;
        if flags & A_INIT != 0 {
            tmp[..5].fill(0);
        }
        let mut inp = input / 2;
        if flags & A_LOOP != 0 {
            self.w[inp - 8..inp].copy_from_slice(&tmp[8..16]);
            inp = (inp as isize - (tmp[5] as isize / 2)) as usize;
        }
        inp -= 4;
        let start = inp;
        let mut acc = tmp[4] as u16 as u32;
        self.w[inp..inp + 4].copy_from_slice(&tmp[..4]);
        let incr = (pitch as u32) << 1;
        let o = out / 2;
        // Indices wrap at the (power-of-two) workspace size instead of being
        // bounds-checked per sample; in range they are the plain offsets.
        const MASK: usize = WORDS - 1;
        let w = &mut self.w;
        for i in 0..COUNT / 2 {
            let t = &table[((acc * 64) >> 16) as usize & 63];
            let v = (w[inp & MASK] as i32 * t[0] as i32
                + w[(inp + 1) & MASK] as i32 * t[1] as i32
                + w[(inp + 2) & MASK] as i32 * t[2] as i32
                + w[(inp + 3) & MASK] as i32 * t[3] as i32)
                >> 15;
            w[(o + i) & MASK] = clamp16(v);
            acc += incr;
            inp += (acc >> 16) as usize;
            acc &= 0xFFFF;
        }
        state[4] = acc as i16;
        state[..4].copy_from_slice(&self.w[inp..inp + 4]);
        let mut a = (inp - start) & 7;
        inp -= a;
        if a != 0 {
            a = (-8 - a as isize) as usize;
        }
        state[5] = a as i16;
        state[8..16].copy_from_slice(&self.w[inp..inp + 8]);
    }

    /// `A_ENVMIXER` (N_MICRO): mixes the 184 samples at [`TEMP_0`] into the
    /// dry and wet buses with per-sample Q16.16 volume ramps.
    pub fn envmix(&mut self, init: Option<&EnvMixInit>, state: &mut EnvMixState) {
        let mut st = match init {
            Some(i) => {
                let rate = |k: usize| ((i.rate_m[k] as i32) << 16) | i.rate_l[k] as i32;
                EnvMixState {
                    wet: i.wet,
                    dry: i.dry,
                    value: [(i.vol[0] as i32) << 16, (i.vol[1] as i32) << 16],
                    target: [(i.tgt[0] as i32) << 16, (i.tgt[1] as i32) << 16],
                    step: [rate(0) / 8, rate(1) / 8],
                }
            }
            None => *state,
        };
        let (dry, wet) = (st.dry as i32, st.wet as i32);
        let (head, rest) = self.w.split_at_mut(MAIN_L / 2);
        let input = &head[..COUNT / 2];
        let (main, aux) = rest.split_at_mut((AUX_L - MAIN_L) / 2);
        let (ml, mr) = main.split_at_mut(COUNT / 2);
        let (al, ar) = aux.split_at_mut(COUNT / 2);
        let (mr, al, ar) = (&mut mr[..COUNT / 2], &mut al[..], &mut ar[..COUNT / 2]);
        let n = COUNT / 2;
        for i in 0..n {
            let s = input[i] as i32;
            let lv = ramp(&mut st, 0) as i32;
            let rv = ramp(&mut st, 1) as i32;
            let ld = clamp16((lv * dry + 0x4000) >> 15) as i32;
            let rd = clamp16((rv * dry + 0x4000) >> 15) as i32;
            let lw = clamp16((lv * wet + 0x4000) >> 15) as i32;
            let rw = clamp16((rv * wet + 0x4000) >> 15) as i32;
            ml[i] = clamp16(ml[i] as i32 + ((s * ld) >> 15));
            mr[i] = clamp16(mr[i] as i32 + ((s * rd) >> 15));
            al[i] = clamp16(al[i] as i32 + ((s * lw) >> 15));
            ar[i] = clamp16(ar[i] as i32 + ((s * rw) >> 15));
        }
        *state = st;
    }

    /// `A_MIX` over 184 samples: `out += (in * gain) >> 15`, saturating;
    /// gain -0x8000 is a plain subtract.
    pub fn mix(&mut self, gain: i16, input: usize, out: usize) {
        let (i, o) = (input / 2, out / 2);
        let n = COUNT / 2;
        if gain == i16::MIN {
            for k in 0..n {
                self.w[o + k] = clamp16(self.w[o + k] as i32 - self.w[i + k] as i32);
            }
            return;
        }
        let g = gain as i32;
        if i + n <= o || o + n <= i {
            let (a, b) = if i < o {
                let (lo, hi) = self.w.split_at_mut(o);
                (&lo[i..i + n], &mut hi[..n])
            } else {
                let (lo, hi) = self.w.split_at_mut(i);
                (&hi[..n], &mut lo[o..o + n])
            };
            for (d, s) in b.iter_mut().zip(a) {
                *d = clamp16(*d as i32 + ((*s as i32 * g) >> 15));
            }
        } else {
            for k in 0..n {
                self.w[o + k] = clamp16(self.w[o + k] as i32 + ((self.w[i + k] as i32 * g) >> 15));
            }
        }
    }

    /// `A_INTERLEAVE` (N_MICRO): MAIN_L and MAIN_R into L,R pairs at
    /// [`TEMP_0`], 184 frames.
    pub fn interleave(&mut self) {
        let mut l = [0i16; COUNT / 2];
        let mut r = [0i16; COUNT / 2];
        l.copy_from_slice(&self.w[MAIN_L / 2..MAIN_L / 2 + COUNT / 2]);
        r.copy_from_slice(&self.w[MAIN_R / 2..MAIN_R / 2 + COUNT / 2]);
        for (d, (a, b)) in self.w[..COUNT]
            .as_chunks_mut::<2>()
            .0
            .iter_mut()
            .zip(l.iter().zip(&r))
        {
            d[0] = *a;
            d[1] = *b;
        }
    }

    /// `A_POLEF` (the pole variant, `table[0..2] == 0`): in place on the 184
    /// samples at `addr`. `coefs` is what `A_LOADADPCM` loaded (32 bytes).
    pub fn polef(
        &mut self,
        flags: u32,
        gain: u16,
        coefs: &[i16; 16],
        state: &mut [i16; 4],
        addr: usize,
    ) {
        let h1 = &coefs[..8];
        let h2_before = &coefs[8..];
        let g = gain as i32;
        let mut h2 = [0i16; 8];
        for (d, s) in h2.iter_mut().zip(h2_before) {
            *d = ((*s as i32 * g) >> 14) as i16;
        }
        let (mut l1, mut l2) = if flags & A_INIT != 0 {
            (0, 0)
        } else {
            (state[2] as i32, state[3] as i32)
        };
        let buf = &mut self.w[addr / 2..addr / 2 + COUNT / 2];
        for chunk in buf.as_chunks_mut::<8>().0 {
            let mut frame = [0i32; 8];
            for (f, s) in frame.iter_mut().zip(chunk.iter()) {
                *f = *s as i32;
            }
            for i in 0..8 {
                let mut acc = frame[i] * g + h1[i] as i32 * l1 + h2_before[i] as i32 * l2;
                for k in 0..i {
                    acc += h2[i - 1 - k] as i32 * frame[k];
                }
                chunk[i] = clamp16(acc >> 14);
            }
            l1 = chunk[6] as i32;
            l2 = chunk[7] as i32;
        }
        state.copy_from_slice(&buf[COUNT / 2 - 4..]);
    }
}

/// One envmixer ramp step: `value += step` saturating, snapping to the
/// target once reached (BattleShip's `ramp_step`).
#[inline(always)]
fn ramp(st: &mut EnvMixState, k: usize) -> i16 {
    let step = st.step[k];
    let v = st.value[k].saturating_add(step);
    let reached = if step <= 0 {
        v <= st.target[k]
    } else {
        v >= st.target[k]
    };
    if reached {
        st.value[k] = st.target[k];
        st.step[k] = 0;
    } else {
        st.value[k] = v;
    }
    (st.value[k] >> 16) as i16
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(header: u8, nibbles: [u8; 16]) -> [u8; 9] {
        let mut f = [header, 0, 0, 0, 0, 0, 0, 0, 0];
        for (k, n) in nibbles.as_chunks::<2>().0.iter().enumerate() {
            f[1 + k] = (n[0] << 4) | n[1];
        }
        f
    }

    #[test]
    fn adpcm_scale12_is_full_scale() {
        let mut d = Dmem::default();
        let mut st = [0i16; 16];
        let book = [0i16; 16];
        // Scale 12 shifts by 0 (x4096), not to silence; scale 11 halves it.
        let mut nib = [1u8; 16];
        nib[1] = 7;
        nib[2] = 8;
        nib[3] = 0xF;
        let mut input = frame(0xC0, nib).to_vec();
        input.extend_from_slice(&frame(0xB0, [1; 16]));
        d.adpcm(A_INIT, &mut st, &[0; 16], &book, &input, TEMP_1, 64);
        let out = d.get(TEMP_1, 96);
        assert_eq!(&out[..16], &[0; 16]);
        assert_eq!(&out[16..20], &[4096, 28672, -32768, -4096]);
        assert_eq!(out[32], 2048);
        assert_eq!(&st[..], &out[32..48]);
    }

    #[test]
    fn adpcm_predictor_uses_history_and_inputs() {
        let mut d = Dmem::default();
        // c1[0] = 1.0 (Q11): out[0] = prev1 + in[0]; out[1] = in[1] + in[0].
        let mut book = [0i16; 32];
        book[16 + 8] = 2048;
        let mut st = [0i16; 16];
        st[15] = 100;
        // Scale 11, predictor 1.
        let input = frame((11 << 4) | 1, [2; 16]);
        d.adpcm(0, &mut st, &[0; 16], &book, &input, TEMP_1, 32);
        let out = d.get(TEMP_1, 64);
        assert_eq!(out[15], 100);
        // 2 << 12 >> 1 = 4096.
        assert_eq!(out[16], 100 + 4096);
        assert_eq!(out[17], 4096 + 4096);
    }

    #[test]
    fn resample_unity_is_the_filtered_input() {
        let mut d = Dmem::default();
        let mut table = [[0i16; 4]; 64];
        table[0] = [3129, 26285, 3398, -33];
        for (i, s) in d.w[TEMP_1 / 2..TEMP_1 / 2 + 200].iter_mut().enumerate() {
            *s = if i == 10 { 10000 } else { 0 };
        }
        let mut st = [0i16; 16];
        d.resample(A_INIT, 0x8000, &mut st, TEMP_1, TEMP_0, &table);
        let out = d.get(TEMP_0, COUNT);
        // An impulse comes out as the filter row reversed, delayed.
        let expect: [i16; 4] =
            [-33, 3398, 26285, 3129].map(|t: i16| ((10000 * t as i32) >> 15) as i16);
        assert_eq!(&out[11..15], &expect);
        // 184 inputs consumed: the state holds the last four.
        assert_eq!(st[4], 0);
        // Constant input stays constant (the taps sum to ~1.0).
        d.w[TEMP_1 / 2 - 4..TEMP_1 / 2 + 200].fill(1000);
        let mut st = [0i16; 16];
        d.resample(A_INIT, 0x8000, &mut st, TEMP_1, TEMP_0, &table);
        assert!(d.get(TEMP_0, COUNT)[8..].iter().all(|&s| s == 1000));
    }

    #[test]
    fn envmix_ramps_reach_their_targets() {
        let mut d = Dmem::default();
        d.w[..COUNT / 2].fill(0x4000);
        let mut st = EnvMixState::default();
        // Count 0 (`_getRate`'s 0x7FFF/0xFFFF): jump to the target at once.
        let init = EnvMixInit {
            vol: [1, 1],
            tgt: [0x7FFF, 0x4000],
            rate_m: [0x7FFF, 0x7FFF],
            rate_l: [0xFFFF, 0xFFFF],
            dry: 0x7FFF,
            wet: 0,
        };
        d.envmix(Some(&init), &mut st);
        assert_eq!(st.value, [0x7FFF << 16, 0x4000 << 16]);
        assert_eq!(st.step, [0, 0]);
        // The step is rate/8 per sample: the left ramp saturates at its
        // 8th sample.
        let l = d.get(MAIN_L, COUNT)[7] as i32;
        let r = d.get(MAIN_R, COUNT)[7] as i32;
        assert!(d.get(MAIN_L, COUNT)[6] < l as i16);
        assert_eq!(l, (0x4000 * ((0x7FFF * 0x7FFF + 0x4000) >> 15)) >> 15);
        assert_eq!(r, (0x4000 * ((0x4000 * 0x7FFF + 0x4000) >> 15)) >> 15);
        assert!(d.get(AUX_L, COUNT).iter().all(|&s| s == 0));
        // A slow ramp continues across calls from the saved state.
        let init = EnvMixInit {
            vol: [0, 0],
            tgt: [1000, 1000],
            rate_m: [0, 0],
            rate_l: [0x8000, 0x8000],
            dry: 0x7FFF,
            wet: 0x7FFF,
        };
        d.envmix(Some(&init), &mut st);
        assert_eq!(st.value[0], 184 * (0x8000 / 8));
        d.envmix(None, &mut st);
        assert_eq!(st.value[0], 2 * 184 * (0x8000 / 8));
    }

    #[test]
    fn mix_saturates() {
        let mut d = Dmem::default();
        d.w[MAIN_L / 2..MAIN_L / 2 + 184].fill(30000);
        d.w[AUX_L / 2..AUX_L / 2 + 184].fill(30000);
        d.mix(0x7FFF, AUX_L, MAIN_L);
        assert!(d.get(MAIN_L, COUNT).iter().all(|&s| s == 32767));
        d.mix(i16::MIN, AUX_L, MAIN_L);
        assert!(d.get(MAIN_L, COUNT).iter().all(|&s| s == 2767));
    }

    #[test]
    fn polef_is_a_one_pole_low_pass() {
        let mut d = Dmem::default();
        // `_init_lpfilter` for fc 0x5000: fc' = 0x5000*16384 >> 15.
        let fc: i16 = 0x2800;
        let mut coefs = [0i16; 16];
        coefs[8] = fc;
        let mut p = fc as f64 / 16384.0;
        for c in &mut coefs[9..] {
            p *= fc as f64 / 16384.0;
            *c = (p * 16384.0) as i16;
        }
        d.w[..184].fill(8000);
        let mut st = [0i16; 4];
        d.polef(A_INIT, (16384 - fc as i32) as u16, &coefs, &mut st, TEMP_0);
        let out = d.get(TEMP_0, COUNT);
        // y = g*x + fc*y[-1]: rises monotonically towards the input.
        assert_eq!(out[0], ((8000 * (16384 - fc as i32)) >> 14) as i16);
        // Truncation leaves a 1-LSB wobble once settled.
        assert!(out[..16].windows(2).all(|w| w[1] >= w[0]));
        assert!((out[183] as i32 - 8000).abs() < 8);
        assert_eq!(&st[..], &out[180..184]);
    }
}
