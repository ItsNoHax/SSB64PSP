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

/// Saturates to 16 bits.
#[cfg(not(target_arch = "mips"))]
#[inline(always)]
pub(crate) fn clamp16(v: i32) -> i16 {
    v.clamp(i16::MIN as i32, i16::MAX as i32) as i16
}

/// Saturates to 16 bits with the Allegrex's `max` and `min` (no branch;
/// LLVM's MIPS II code for a clamp is two compare-and-branch pairs). The
/// target CPU is MIPS II, whose assembler has no mnemonic for them, so they
/// are encoded: `max $8, $8, $9` (SPECIAL funct 0x2C) and
/// `min $8, $8, $10` (funct 0x2D).
#[cfg(target_arch = "mips")]
#[inline(always)]
pub(crate) fn clamp16(v: i32) -> i16 {
    let r: i32;
    // SAFETY: two register-only ALU instructions on the named registers.
    unsafe {
        core::arch::asm!(
            ".word 0x0109402C",
            ".word 0x010A402D",
            inout("$8") v => r,
            in("$9") i16::MIN as i32,
            in("$10") i16::MAX as i32,
            options(pure, nomem, nostack, preserves_flags),
        );
    }
    r as i16
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
        let (hist, body) = o.split_at_mut(16);
        let mut prev2 = hist[14] as i32;
        let mut prev1 = hist[15] as i32;
        for (f, out) in body.as_chunks_mut::<16>().0.iter_mut().enumerate() {
            let mut frame = [0u8; 9];
            if let Some(src) = input.get(f * 9..f * 9 + 9) {
                frame = src.try_into().unwrap_or([0; 9]);
            } else if let Some(src) = input.get(f * 9..) {
                frame[..src.len()].copy_from_slice(src);
            }
            let shift = (frame[0] >> 4) as u32;
            let pred = (frame[0] & 15) as usize;
            // Scale 12 means x4096 (rshift 0), not silence.
            let rshift = 12u32.saturating_sub(shift);
            let c: &[i16; 16] = match book.get(pred * 16..pred * 16 + 16) {
                Some(c) => c.as_chunks::<16>().0.first().unwrap_or(&ZERO_BOOK),
                None => &ZERO_BOOK,
            };
            let c0: [i32; 8] = core::array::from_fn(|j| c[j] as i32);
            let c1: [i32; 8] = core::array::from_fn(|j| c[8 + j] as i32);
            for (half, out) in out.as_chunks_mut::<8>().0.iter_mut().enumerate() {
                let bytes = &frame[1 + half * 4..5 + half * 4];
                let ins: [i32; 8] = core::array::from_fn(|j| {
                    let b = bytes[j / 2] as u16;
                    let nib = if j % 2 == 0 {
                        (b & 0xF0) << 8
                    } else {
                        (b & 0x0F) << 12
                    };
                    (nib as i16 >> rshift) as i32
                });
                adpcm_8(&c0, &c1, &ins, prev2, prev1, out);
                prev2 = out[6] as i32;
                prev1 = out[7] as i32;
            }
        }
        let pos = 16 + frames * 16;
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
        // Each side's per-sample volume (`value >> 16`) for this call: it
        // varies over the first `len` samples and is constant after them.
        let mut lv = [0i32; COUNT / 2];
        let mut rv = [0i32; COUNT / 2];
        let llen = ramp_fill(&mut st, 0, &mut lv);
        let rlen = ramp_fill(&mut st, 1, &mut rv);
        // The four buses are independent: each gets its own pass, and a
        // zero amount (gain 0 on every sample) adds nothing.
        for (bus, v, len, amt) in [
            (&mut ml[..n], &lv, llen, dry),
            (&mut mr[..n], &rv, rlen, dry),
            (&mut al[..n], &lv, llen, wet),
            (&mut ar[..n], &rv, rlen, wet),
        ] {
            if amt != 0 {
                env_bus(bus, &input[..n], v, len, amt);
            }
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

/// The predictor row a frame naming a missing predictor reads.
static ZERO_BOOK: [i16; 16] = [0; 16];

/// Eight `A_ADPCM` outputs: the predictor over the two previous outputs
/// and the eight scaled nibbles (`acc` in Q11).
#[inline(always)]
#[rustfmt::skip]
fn adpcm_8(c0: &[i32; 8], c1: &[i32; 8], i: &[i32; 8], p2: i32, p1: i32, out: &mut [i16; 8]) {
    // Written out: the triangle `sum(c1[j - k - 1] * in[k], k < j)` with
    // constant indices, so nothing is a loop or a bounds check.
    out[0] = clamp16((c0[0] * p2 + c1[0] * p1 + (i[0] << 11)) >> 11);
    out[1] = clamp16((c0[1] * p2 + c1[1] * p1 + (i[1] << 11) + c1[0] * i[0]) >> 11);
    out[2] = clamp16((c0[2] * p2 + c1[2] * p1 + (i[2] << 11) + c1[1] * i[0] + c1[0] * i[1]) >> 11);
    out[3] = clamp16((c0[3] * p2 + c1[3] * p1 + (i[3] << 11) + c1[2] * i[0] + c1[1] * i[1] + c1[0] * i[2]) >> 11);
    out[4] = clamp16((c0[4] * p2 + c1[4] * p1 + (i[4] << 11) + c1[3] * i[0] + c1[2] * i[1] + c1[1] * i[2] + c1[0] * i[3]) >> 11);
    out[5] = clamp16((c0[5] * p2 + c1[5] * p1 + (i[5] << 11) + c1[4] * i[0] + c1[3] * i[1] + c1[2] * i[2] + c1[1] * i[3] + c1[0] * i[4]) >> 11);
    out[6] = clamp16((c0[6] * p2 + c1[6] * p1 + (i[6] << 11) + c1[5] * i[0] + c1[4] * i[1] + c1[3] * i[2] + c1[2] * i[3] + c1[1] * i[4] + c1[0] * i[5]) >> 11);
    out[7] = clamp16((c0[7] * p2 + c1[7] * p1 + (i[7] << 11) + c1[6] * i[0] + c1[5] * i[1] + c1[4] * i[2] + c1[3] * i[3] + c1[2] * i[4] + c1[1] * i[5] + c1[0] * i[6]) >> 11);
}

/// `out += (in * g) >> 15`, saturating.
#[inline(always)]
fn mix_gain(out: &mut [i16], input: &[i16], g: i32) {
    for (d, s) in out.iter_mut().zip(input) {
        *d = clamp16(*d as i32 + ((*s as i32 * g) >> 15));
    }
}

/// One envmixer bus: `bus += (in * clamp16((v * amt + 0x4000) >> 15)) >>
/// 15`, saturating, with `v` constant from `len` on.
#[inline(always)]
fn env_bus(bus: &mut [i16], input: &[i16], v: &[i32; COUNT / 2], len: usize, amt: i32) {
    let gain = |v: i32| clamp16((v * amt + 0x4000) >> 15) as i32;
    let n = bus.len().min(input.len());
    let len = len.min(n);
    for k in 0..len {
        let g = gain(v[k]);
        bus[k] = clamp16(bus[k] as i32 + ((input[k] as i32 * g) >> 15));
    }
    if len < n {
        let g = gain(v[len]);
        if g != 0 {
            mix_gain(&mut bus[len..n], &input[len..n], g);
        }
    }
}

/// Runs ramp `k` over a whole call ([`ramp`] once per sample), writing
/// each sample's `value >> 16` to `out`. Returns how many leading samples
/// may differ from the last one; the rest equal it.
///
/// A ramp adds `step` until a sum reaches the target (`>=` for a positive
/// step, `<=` otherwise), which snaps it to the target with step 0; a
/// saturating sum always reaches it. So the sample that snaps is the first
/// `m >= 1` with `value + m * step` past the target in exact arithmetic,
/// and the samples before it are plain sums.
#[inline(always)]
fn ramp_fill(st: &mut EnvMixState, k: usize, out: &mut [i32; COUNT / 2]) -> usize {
    let (v, step, t) = (st.value[k] as i64, st.step[k] as i64, st.target[k] as i64);
    let n = out.len();
    let m = if step > 0 {
        if t > v {
            (t - v + step - 1) / step
        } else {
            1
        }
    } else if step < 0 {
        if v > t {
            (v - t + (-step) - 1) / (-step)
        } else {
            1
        }
    } else if v <= t {
        1
    } else {
        // Step 0 above the target: never moves.
        let c = st.value[k] >> 16;
        out.fill(c);
        return 0;
    };
    // Samples 1..m-1 (indices 0..m-2) are sums; sample m snaps.
    let sums = (m - 1).min(n as i64) as usize;
    let mut val = st.value[k];
    let step = st.step[k];
    for o in &mut out[..sums] {
        val += step;
        *o = val >> 16;
    }
    if sums < n {
        st.value[k] = st.target[k];
        st.step[k] = 0;
        out[sums..].fill(st.target[k] >> 16);
        sums + 1
    } else {
        st.value[k] = val;
        n
    }
}

/// One envmixer ramp step: `value += step` saturating, snapping to the
/// target once reached (BattleShip's `ramp_step`). [`ramp_fill`] is the
/// same over a whole call.
#[cfg(test)]
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

    /// `ramp_fill` equals `ramp` sample by sample, saturation included.
    #[test]
    fn ramp_fill_matches_per_sample_ramp() {
        let mut x: u32 = 1;
        let mut rnd = || {
            x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            x
        };
        let pick = |r: u32, small: i32| -> i32 {
            match r % 6 {
                0 => i32::MAX - (r >> 8) as i32 % 1000,
                1 => i32::MIN + (r >> 8) as i32 % 1000,
                2 => 0,
                3 => ((r >> 4) as i32 % small) - small / 2,
                _ => (r as i32) >> (r % 16),
            }
        };
        for _ in 0..200_000 {
            let (a, b, c) = (rnd(), rnd(), rnd());
            let mut st = EnvMixState {
                value: [pick(a, 1 << 20), 0],
                target: [pick(b, 1 << 20), 0],
                step: [pick(c, 1 << 16), 0],
                ..Default::default()
            };
            let mut want = st;
            let mut out = [0i32; COUNT / 2];
            let len = ramp_fill(&mut st, 0, &mut out);
            for (k, o) in out.iter().enumerate() {
                assert_eq!(*o, ramp(&mut want, 0) as i32, "{k} {a} {b} {c}");
                if k >= len {
                    assert_eq!(*o, out[len.min(COUNT / 2 - 1)]);
                }
            }
            assert_eq!((st.value, st.step), (want.value, want.step));
        }
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
