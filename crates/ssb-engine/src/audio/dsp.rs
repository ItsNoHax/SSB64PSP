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

use super::mem;

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
        mem::zero(self.words(addr, n));
    }

    /// `A_DMEMMOVE` (memmove semantics).
    pub fn dmem_move(&mut self, src: usize, dst: usize, nbytes: usize) {
        let n = round_up(nbytes, 16) / 2;
        mem::copy_within(&mut self.w, src / 2, dst / 2, n);
    }

    /// `A_LOADBUFF` from an `s16` DRAM buffer (the reverb delay line);
    /// `src` holds exactly the words to load.
    pub fn load(&mut self, dst: usize, src: &[i16]) {
        mem::copy(&mut self.w[dst / 2..dst / 2 + src.len()], src);
    }

    /// `A_SAVEBUFF` to an `s16` DRAM buffer.
    pub fn save(&self, src: usize, dst: &mut [i16]) {
        mem::copy(dst, &self.w[src / 2..src / 2 + dst.len()]);
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
            let mut local = [0u8; 9];
            let frame: &[u8; 9] = match input.get(f * 9..f * 9 + 9) {
                Some(src) => src.as_chunks::<9>().0.first().unwrap_or(&local),
                None => {
                    if let Some(src) = input.get(f * 9..) {
                        local[..src.len()].copy_from_slice(src);
                    }
                    &local
                }
            };
            let shift = (frame[0] >> 4) as u32;
            let pred = (frame[0] & 15) as usize;
            let c: &[i16; 16] = match book.get(pred * 16..pred * 16 + 16) {
                Some(c) => c.as_chunks::<16>().0.first().unwrap_or(&ZERO_BOOK),
                None => &ZERO_BOOK,
            };
            #[cfg(target_arch = "mips")]
            {
                // A nibble scaled by `>> (12 - shift)` from `<< 12` is the
                // signed nibble `<< min(shift, 12)`.
                (prev2, prev1) = allegrex::adpcm_16(frame, c, shift.min(12), prev2, prev1, out);
            }
            #[cfg(not(target_arch = "mips"))]
            {
                // Scale 12 means x4096 (rshift 0), not silence.
                let rshift = 12u32.saturating_sub(shift);
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
                    adpcm_8(c, &ins, prev2, prev1, out);
                    prev2 = out[6] as i32;
                    prev1 = out[7] as i32;
                }
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
        #[cfg(target_arch = "mips")]
        let in_range = inp + ((acc as usize + (COUNT / 2) * incr as usize) >> 16) + 4 <= WORDS
            && o + COUNT / 2 <= WORDS;
        #[cfg(not(target_arch = "mips"))]
        let in_range = false;
        if in_range {
            #[cfg(target_arch = "mips")]
            {
                (inp, acc) = allegrex::resample(w, inp, o, COUNT / 2, acc, incr, table);
            }
        } else {
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
        // Each side's volume (`value >> 16`) per sample: plain sums of the
        // step for the ramp's first samples, then constant (`Ramp`).
        let l = ramp_run(&mut st, 0);
        let r = ramp_run(&mut st, 1);
        #[cfg(target_arch = "mips")]
        self.envmix_allegrex(l, r, dry, wet);
        #[cfg(not(target_arch = "mips"))]
        self.envmix_buses(l, r, dry, wet);
        *state = st;
    }

    /// The envmixer's four bus passes: each bus is independent, and a zero
    /// amount (gain 0 on every sample) adds nothing.
    #[cfg(not(target_arch = "mips"))]
    fn envmix_buses(&mut self, l: Ramp, r: Ramp, dry: i32, wet: i32) {
        let (head, rest) = self.w.split_at_mut(MAIN_L / 2);
        let input = &head[..COUNT / 2];
        let (main, aux) = rest.split_at_mut((AUX_L - MAIN_L) / 2);
        let (ml, mr) = main.split_at_mut(COUNT / 2);
        let (al, ar) = aux.split_at_mut(COUNT / 2);
        let n = COUNT / 2;
        for (bus, ramp, amt) in [
            (&mut ml[..n], l, dry),
            (&mut mr[..n], r, dry),
            (&mut al[..n], l, wet),
            (&mut ar[..n], r, wet),
        ] {
            if amt != 0 {
                env_bus(bus, &input[..n], ramp, amt);
            }
        }
    }

    /// [`Dmem::envmix_buses`] as Allegrex kernels that run all four buses
    /// per sample: while both volumes ramp, while one does (the other held
    /// at its final volume, a step-0 ramp), then with constant gains.
    #[cfg(target_arch = "mips")]
    fn envmix_allegrex(&mut self, l: Ramp, r: Ramp, dry: i32, wet: i32) {
        const _: () = assert!(
            MAIN_R - MAIN_L == COUNT && AUX_L - MAIN_L == 2 * COUNT && AUX_R - MAIN_L == 3 * COUNT
        );
        let n = COUNT / 2;
        let gain = |v: i32, amt: i32| clamp16((v * amt + 0x4000) >> 15) as i32;
        let both = l.sums.min(r.sums).min(n);
        let one = l.sums.max(r.sums).min(n);
        // SAFETY: the input (TEMP_0) and the four buses are disjoint ranges
        // of `w` (the layout is asserted above); every range is in bounds.
        unsafe {
            let input = self.w.as_ptr();
            let bus = self.w.as_mut_ptr().add(MAIN_L / 2);
            let (mut lv, mut rv) =
                allegrex::env4_ramp(input, bus, both, l.value, l.step, r.value, r.step, dry, wet);
            if one > both {
                let (ls, rs) = (
                    if l.sums > both { l.step } else { 0 },
                    if r.sums > both { r.step } else { 0 },
                );
                if ls == 0 {
                    lv = l.then << 16;
                }
                if rs == 0 {
                    rv = r.then << 16;
                }
                let _ = allegrex::env4_ramp(
                    input.add(both),
                    bus.add(both),
                    one - both,
                    lv,
                    ls,
                    rv,
                    rs,
                    dry,
                    wet,
                );
            }
            let g = [
                gain(l.then, dry),
                gain(r.then, dry),
                gain(l.then, wet),
                gain(r.then, wet),
            ];
            allegrex::env4_const(input.add(one), bus.add(one), n - one, g);
        }
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
            mix_gain(b, a, g);
        } else {
            for k in 0..n {
                self.w[o + k] = clamp16(self.w[o + k] as i32 + ((self.w[i + k] as i32 * g) >> 15));
            }
        }
    }

    /// `A_INTERLEAVE` (N_MICRO): MAIN_L and MAIN_R into L,R pairs at
    /// [`TEMP_0`], 184 frames.
    pub fn interleave(&mut self) {
        let (out, rest) = self.w.split_at_mut(MAIN_L / 2);
        let l = &rest[..COUNT / 2];
        let r = &rest[(MAIN_R - MAIN_L) / 2..(MAIN_R - MAIN_L) / 2 + COUNT / 2];
        for (d, (a, b)) in out[..COUNT]
            .as_chunks_mut::<2>()
            .0
            .iter_mut()
            .zip(l.iter().zip(r))
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
#[cfg(not(target_arch = "mips"))]
#[inline(always)]
#[rustfmt::skip]
fn adpcm_8(c: &[i16; 16], i: &[i32; 8], p2: i32, p1: i32, out: &mut [i16; 8]) {
    let c0: [i32; 8] = core::array::from_fn(|j| c[j] as i32);
    let c1: [i32; 8] = core::array::from_fn(|j| c[8 + j] as i32);
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
pub(crate) fn mix_gain(out: &mut [i16], input: &[i16], g: i32) {
    #[cfg(target_arch = "mips")]
    return allegrex::mix_gain(out, input, g);
    #[cfg(not(target_arch = "mips"))]
    for (d, s) in out.iter_mut().zip(input) {
        *d = clamp16(*d as i32 + ((*s as i32 * g) >> 15));
    }
}

/// One envmixer bus: `bus += (in * clamp16((v * amt + 0x4000) >> 15)) >>
/// 15`, saturating, with `v` per sample from `ramp`.
#[cfg(not(target_arch = "mips"))]
#[inline(always)]
fn env_bus(bus: &mut [i16], input: &[i16], ramp: Ramp, amt: i32) {
    let gain = |v: i32| clamp16((v * amt + 0x4000) >> 15) as i32;
    let n = bus.len().min(input.len());
    let sums = ramp.sums.min(n);
    let mut val = ramp.value;
    for k in 0..sums {
        val += ramp.step;
        let g = gain(val >> 16);
        bus[k] = clamp16(bus[k] as i32 + ((input[k] as i32 * g) >> 15));
    }
    if sums < n {
        let g = gain(ramp.then);
        if g != 0 {
            mix_gain(&mut bus[sums..n], &input[sums..n], g);
        }
    }
}

/// A ramp over one envmixer call: `sums` samples of `value + k * step`
/// (`k` from 1), then `then` (a volume, `>> 16` applied) for the rest.
#[derive(Clone, Copy)]
struct Ramp {
    value: i32,
    step: i32,
    sums: usize,
    then: i32,
}

/// Runs ramp `k` over a whole call ([`ramp`] once per sample), updating
/// the state, and returns the volumes it gave.
///
/// A ramp adds `step` until a sum reaches the target (`>=` for a positive
/// step, `<=` otherwise), which snaps it to the target with step 0; a
/// saturating sum always reaches it. So the sample that snaps is the first
/// `m >= 1` with `value + m * step` past the target in exact arithmetic,
/// and the samples before it are plain sums.
#[inline(always)]
fn ramp_run(st: &mut EnvMixState, k: usize) -> Ramp {
    let n = COUNT / 2;
    let (value, step, target) = (st.value[k], st.step[k], st.target[k]);
    // The distance and the step as non-negative i64s; i64 division is a
    // library call, so it only runs when the snap could fall in this call.
    let to_snap = |dist: i64, s: i64| -> usize {
        if dist <= 0 {
            1
        } else if dist > s * n as i64 {
            n + 1
        } else {
            ((dist + s - 1) / s) as usize
        }
    };
    let m = if step > 0 {
        to_snap(target as i64 - value as i64, step as i64)
    } else if step < 0 {
        to_snap(value as i64 - target as i64, -(step as i64))
    } else if value <= target {
        1
    } else {
        // Step 0 above the target: never moves.
        return Ramp {
            value,
            step: 0,
            sums: 0,
            then: value >> 16,
        };
    };
    // Samples 1..m-1 are sums; sample m snaps.
    let sums = (m - 1).min(n);
    if sums < n {
        st.value[k] = target;
        st.step[k] = 0;
    } else {
        st.value[k] = value.wrapping_add(step.wrapping_mul(n as i32));
    }
    Ramp {
        value,
        step,
        sums,
        then: target >> 16,
    }
}

/// One envmixer ramp step: `value += step` saturating, snapping to the
/// target once reached (BattleShip's `ramp_step`). [`ramp_run`] is the
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

/// Allegrex kernels for the hottest DSP loops (PSP only). Each computes
/// exactly what the portable loop beside it computes, and the golden
/// scenario checks the two agree on the target (`audio::golden`). The
/// target CPU is MIPS II, so the Allegrex's `max`/`min` (SPECIAL funct
/// 0x2C/0x2D) are emitted as `.word`s on fixed registers.
#[cfg(target_arch = "mips")]
pub(crate) mod allegrex {
    use core::arch::asm;

    /// `out[k] = clamp16(out[k] + ((input[k] * g) >> 15))`.
    #[inline(always)]
    pub fn mix_gain(out: &mut [i16], input: &[i16], g: i32) {
        let n = out.len().min(input.len());
        if n == 0 {
            return;
        }
        // SAFETY: reads `input[..n]`, writes `out[..n]`; registers named.
        unsafe {
            let o = out.as_mut_ptr();
            asm!(
                ".set push",
                ".set noreorder",
                "1:",
                "lh $8, 0($4)",
                "lh $9, 0($5)",
                "mult $8, $7",
                "addiu $4, $4, 2",
                "addiu $5, $5, 2",
                "mflo $8",
                "sra $8, $8, 15",
                "addu $9, $9, $8",
                ".word 0x012a482c", // max $9, $9, $10
                ".word 0x012b482d", // min $9, $9, $11
                "bne $5, $6, 1b",
                "sh $9, -2($5)",
                ".set pop",
                inout("$4") input.as_ptr() => _,
                inout("$5") o => _,
                in("$6") o.add(n),
                in("$7") g,
                out("$8") _,
                out("$9") _,
                in("$10") -32768,
                in("$11") 32767,
                options(nostack),
            );
        }
    }

    /// One 16-sample `A_ADPCM` frame: the eight nibble bytes after the
    /// header, each a signed nibble `<< lshift`, through the predictor
    /// (`adpcm_8`, twice). Returns the last two outputs.
    #[inline(always)]
    pub fn adpcm_16(
        frame: &[u8; 9],
        c: &[i16; 16],
        lshift: u32,
        p2: i32,
        p1: i32,
        out: &mut [i16; 16],
    ) -> (i32, i32) {
        let (a, b): (i32, i32);
        // SAFETY: reads `frame` and `c`, writes `out`; registers named.
        unsafe {
            asm!(
                "lb $8, 1($4)",
                "sll $9, $8, 28",
                "sra $8, $8, 4",
                "sra $9, $9, 28",
                "sllv $8, $8, $19",
                "sllv $9, $9, $19",
                "lb $10, 2($4)",
                "sll $11, $10, 28",
                "sra $10, $10, 4",
                "sra $11, $11, 28",
                "sllv $10, $10, $19",
                "sllv $11, $11, $19",
                "lb $12, 3($4)",
                "sll $13, $12, 28",
                "sra $12, $12, 4",
                "sra $13, $13, 28",
                "sllv $12, $12, $19",
                "sllv $13, $13, $19",
                "lb $14, 4($4)",
                "sll $15, $14, 28",
                "sra $14, $14, 4",
                "sra $15, $15, 28",
                "sllv $14, $14, $19",
                "sllv $15, $15, $19",
                "lh $2, 0($18)",
                "lh $3, 16($18)",
                "mult $2, $6",
                ".word 0x0067001c", // madd $3, $7
                "sll $17, $8, 11",
                "mflo $16",
                "addu $16, $16, $17",
                "sra $16, $16, 11",
                ".word 0x0218802c", // max $16, $16, $24
                ".word 0x0219802d", // min $16, $16, $25
                "sh $16, 0($5)",
                "lh $2, 2($18)",
                "lh $3, 18($18)",
                "mult $2, $6",
                "lh $2, 16($18)",
                ".word 0x0067001c", // madd $3, $7
                ".word 0x0048001c", // madd $2, $8
                "sll $17, $9, 11",
                "mflo $16",
                "addu $16, $16, $17",
                "sra $16, $16, 11",
                ".word 0x0218802c", // max $16, $16, $24
                ".word 0x0219802d", // min $16, $16, $25
                "sh $16, 2($5)",
                "lh $3, 4($18)",
                "lh $2, 20($18)",
                "mult $3, $6",
                "lh $3, 18($18)",
                ".word 0x0047001c", // madd $2, $7
                "lh $2, 16($18)",
                ".word 0x0068001c", // madd $3, $8
                ".word 0x0049001c", // madd $2, $9
                "sll $17, $10, 11",
                "mflo $16",
                "addu $16, $16, $17",
                "sra $16, $16, 11",
                ".word 0x0218802c", // max $16, $16, $24
                ".word 0x0219802d", // min $16, $16, $25
                "sh $16, 4($5)",
                "lh $3, 6($18)",
                "lh $2, 22($18)",
                "mult $3, $6",
                "lh $3, 20($18)",
                ".word 0x0047001c", // madd $2, $7
                "lh $2, 18($18)",
                ".word 0x0068001c", // madd $3, $8
                "lh $3, 16($18)",
                ".word 0x0049001c", // madd $2, $9
                ".word 0x006a001c", // madd $3, $10
                "sll $17, $11, 11",
                "mflo $16",
                "addu $16, $16, $17",
                "sra $16, $16, 11",
                ".word 0x0218802c", // max $16, $16, $24
                ".word 0x0219802d", // min $16, $16, $25
                "sh $16, 6($5)",
                "lh $2, 8($18)",
                "lh $3, 24($18)",
                "mult $2, $6",
                "lh $2, 22($18)",
                ".word 0x0067001c", // madd $3, $7
                "lh $3, 20($18)",
                ".word 0x0048001c", // madd $2, $8
                "lh $2, 18($18)",
                ".word 0x0069001c", // madd $3, $9
                "lh $3, 16($18)",
                ".word 0x004a001c", // madd $2, $10
                ".word 0x006b001c", // madd $3, $11
                "sll $17, $12, 11",
                "mflo $16",
                "addu $16, $16, $17",
                "sra $16, $16, 11",
                ".word 0x0218802c", // max $16, $16, $24
                ".word 0x0219802d", // min $16, $16, $25
                "sh $16, 8($5)",
                "lh $2, 10($18)",
                "lh $3, 26($18)",
                "mult $2, $6",
                "lh $2, 24($18)",
                ".word 0x0067001c", // madd $3, $7
                "lh $3, 22($18)",
                ".word 0x0048001c", // madd $2, $8
                "lh $2, 20($18)",
                ".word 0x0069001c", // madd $3, $9
                "lh $3, 18($18)",
                ".word 0x004a001c", // madd $2, $10
                "lh $2, 16($18)",
                ".word 0x006b001c", // madd $3, $11
                ".word 0x004c001c", // madd $2, $12
                "sll $17, $13, 11",
                "mflo $16",
                "addu $16, $16, $17",
                "sra $16, $16, 11",
                ".word 0x0218802c", // max $16, $16, $24
                ".word 0x0219802d", // min $16, $16, $25
                "sh $16, 10($5)",
                "lh $3, 12($18)",
                "lh $2, 28($18)",
                "mult $3, $6",
                "lh $3, 26($18)",
                ".word 0x0047001c", // madd $2, $7
                "lh $2, 24($18)",
                ".word 0x0068001c", // madd $3, $8
                "lh $3, 22($18)",
                ".word 0x0049001c", // madd $2, $9
                "lh $2, 20($18)",
                ".word 0x006a001c", // madd $3, $10
                "lh $3, 18($18)",
                ".word 0x004b001c", // madd $2, $11
                "lh $2, 16($18)",
                ".word 0x006c001c", // madd $3, $12
                ".word 0x004d001c", // madd $2, $13
                "sll $17, $14, 11",
                "mflo $16",
                "addu $16, $16, $17",
                "sra $16, $16, 11",
                ".word 0x0218802c", // max $16, $16, $24
                ".word 0x0219802d", // min $16, $16, $25
                "sh $16, 12($5)",
                "move $20, $16",
                "lh $3, 14($18)",
                "lh $2, 30($18)",
                "mult $3, $6",
                "lh $3, 28($18)",
                ".word 0x0047001c", // madd $2, $7
                "lh $2, 26($18)",
                ".word 0x0068001c", // madd $3, $8
                "lh $3, 24($18)",
                ".word 0x0049001c", // madd $2, $9
                "lh $2, 22($18)",
                ".word 0x006a001c", // madd $3, $10
                "lh $3, 20($18)",
                ".word 0x004b001c", // madd $2, $11
                "lh $2, 18($18)",
                ".word 0x006c001c", // madd $3, $12
                "lh $3, 16($18)",
                ".word 0x004d001c", // madd $2, $13
                ".word 0x006e001c", // madd $3, $14
                "sll $17, $15, 11",
                "mflo $16",
                "addu $16, $16, $17",
                "sra $16, $16, 11",
                ".word 0x0218802c", // max $16, $16, $24
                ".word 0x0219802d", // min $16, $16, $25
                "sh $16, 14($5)",
                "move $6, $20",
                "move $7, $16",
                "lb $8, 5($4)",
                "sll $9, $8, 28",
                "sra $8, $8, 4",
                "sra $9, $9, 28",
                "sllv $8, $8, $19",
                "sllv $9, $9, $19",
                "lb $10, 6($4)",
                "sll $11, $10, 28",
                "sra $10, $10, 4",
                "sra $11, $11, 28",
                "sllv $10, $10, $19",
                "sllv $11, $11, $19",
                "lb $12, 7($4)",
                "sll $13, $12, 28",
                "sra $12, $12, 4",
                "sra $13, $13, 28",
                "sllv $12, $12, $19",
                "sllv $13, $13, $19",
                "lb $14, 8($4)",
                "sll $15, $14, 28",
                "sra $14, $14, 4",
                "sra $15, $15, 28",
                "sllv $14, $14, $19",
                "sllv $15, $15, $19",
                "lh $2, 0($18)",
                "lh $3, 16($18)",
                "mult $2, $6",
                ".word 0x0067001c", // madd $3, $7
                "sll $17, $8, 11",
                "mflo $16",
                "addu $16, $16, $17",
                "sra $16, $16, 11",
                ".word 0x0218802c", // max $16, $16, $24
                ".word 0x0219802d", // min $16, $16, $25
                "sh $16, 16($5)",
                "lh $2, 2($18)",
                "lh $3, 18($18)",
                "mult $2, $6",
                "lh $2, 16($18)",
                ".word 0x0067001c", // madd $3, $7
                ".word 0x0048001c", // madd $2, $8
                "sll $17, $9, 11",
                "mflo $16",
                "addu $16, $16, $17",
                "sra $16, $16, 11",
                ".word 0x0218802c", // max $16, $16, $24
                ".word 0x0219802d", // min $16, $16, $25
                "sh $16, 18($5)",
                "lh $3, 4($18)",
                "lh $2, 20($18)",
                "mult $3, $6",
                "lh $3, 18($18)",
                ".word 0x0047001c", // madd $2, $7
                "lh $2, 16($18)",
                ".word 0x0068001c", // madd $3, $8
                ".word 0x0049001c", // madd $2, $9
                "sll $17, $10, 11",
                "mflo $16",
                "addu $16, $16, $17",
                "sra $16, $16, 11",
                ".word 0x0218802c", // max $16, $16, $24
                ".word 0x0219802d", // min $16, $16, $25
                "sh $16, 20($5)",
                "lh $3, 6($18)",
                "lh $2, 22($18)",
                "mult $3, $6",
                "lh $3, 20($18)",
                ".word 0x0047001c", // madd $2, $7
                "lh $2, 18($18)",
                ".word 0x0068001c", // madd $3, $8
                "lh $3, 16($18)",
                ".word 0x0049001c", // madd $2, $9
                ".word 0x006a001c", // madd $3, $10
                "sll $17, $11, 11",
                "mflo $16",
                "addu $16, $16, $17",
                "sra $16, $16, 11",
                ".word 0x0218802c", // max $16, $16, $24
                ".word 0x0219802d", // min $16, $16, $25
                "sh $16, 22($5)",
                "lh $2, 8($18)",
                "lh $3, 24($18)",
                "mult $2, $6",
                "lh $2, 22($18)",
                ".word 0x0067001c", // madd $3, $7
                "lh $3, 20($18)",
                ".word 0x0048001c", // madd $2, $8
                "lh $2, 18($18)",
                ".word 0x0069001c", // madd $3, $9
                "lh $3, 16($18)",
                ".word 0x004a001c", // madd $2, $10
                ".word 0x006b001c", // madd $3, $11
                "sll $17, $12, 11",
                "mflo $16",
                "addu $16, $16, $17",
                "sra $16, $16, 11",
                ".word 0x0218802c", // max $16, $16, $24
                ".word 0x0219802d", // min $16, $16, $25
                "sh $16, 24($5)",
                "lh $2, 10($18)",
                "lh $3, 26($18)",
                "mult $2, $6",
                "lh $2, 24($18)",
                ".word 0x0067001c", // madd $3, $7
                "lh $3, 22($18)",
                ".word 0x0048001c", // madd $2, $8
                "lh $2, 20($18)",
                ".word 0x0069001c", // madd $3, $9
                "lh $3, 18($18)",
                ".word 0x004a001c", // madd $2, $10
                "lh $2, 16($18)",
                ".word 0x006b001c", // madd $3, $11
                ".word 0x004c001c", // madd $2, $12
                "sll $17, $13, 11",
                "mflo $16",
                "addu $16, $16, $17",
                "sra $16, $16, 11",
                ".word 0x0218802c", // max $16, $16, $24
                ".word 0x0219802d", // min $16, $16, $25
                "sh $16, 26($5)",
                "lh $3, 12($18)",
                "lh $2, 28($18)",
                "mult $3, $6",
                "lh $3, 26($18)",
                ".word 0x0047001c", // madd $2, $7
                "lh $2, 24($18)",
                ".word 0x0068001c", // madd $3, $8
                "lh $3, 22($18)",
                ".word 0x0049001c", // madd $2, $9
                "lh $2, 20($18)",
                ".word 0x006a001c", // madd $3, $10
                "lh $3, 18($18)",
                ".word 0x004b001c", // madd $2, $11
                "lh $2, 16($18)",
                ".word 0x006c001c", // madd $3, $12
                ".word 0x004d001c", // madd $2, $13
                "sll $17, $14, 11",
                "mflo $16",
                "addu $16, $16, $17",
                "sra $16, $16, 11",
                ".word 0x0218802c", // max $16, $16, $24
                ".word 0x0219802d", // min $16, $16, $25
                "sh $16, 28($5)",
                "move $20, $16",
                "lh $3, 14($18)",
                "lh $2, 30($18)",
                "mult $3, $6",
                "lh $3, 28($18)",
                ".word 0x0047001c", // madd $2, $7
                "lh $2, 26($18)",
                ".word 0x0068001c", // madd $3, $8
                "lh $3, 24($18)",
                ".word 0x0049001c", // madd $2, $9
                "lh $2, 22($18)",
                ".word 0x006a001c", // madd $3, $10
                "lh $3, 20($18)",
                ".word 0x004b001c", // madd $2, $11
                "lh $2, 18($18)",
                ".word 0x006c001c", // madd $3, $12
                "lh $3, 16($18)",
                ".word 0x004d001c", // madd $2, $13
                ".word 0x006e001c", // madd $3, $14
                "sll $17, $15, 11",
                "mflo $16",
                "addu $16, $16, $17",
                "sra $16, $16, 11",
                ".word 0x0218802c", // max $16, $16, $24
                ".word 0x0219802d", // min $16, $16, $25
                "sh $16, 30($5)",
                "move $6, $20",
                "move $7, $16",
                in("$4") frame.as_ptr(),
                in("$5") out.as_mut_ptr(),
                inout("$6") p2 => a,
                inout("$7") p1 => b,
                out("$8") _,
                out("$9") _,
                out("$10") _,
                out("$11") _,
                out("$12") _,
                out("$13") _,
                out("$14") _,
                out("$15") _,
                in("$18") c.as_ptr(),
                in("$19") lshift,
                out("$20") _,
                in("$24") -32768,
                in("$25") 32767,
                out("$2") _,
                out("$3") _,
                out("$16") _,
                out("$17") _,
                options(nostack),
            );
        }
        (a, b)
    }

    /// The `A_RESAMPLE` loop over `n` outputs from `w[inp..]` to `w[o..]`
    /// (`Dmem::resample`); returns the final `(inp, acc)`. The caller has
    /// checked that every index stays inside `w`.
    #[inline(always)]
    pub fn resample(
        w: &mut [i16],
        inp: usize,
        o: usize,
        n: usize,
        mut acc: u32,
        incr: u32,
        table: &[[i16; 4]; 64],
    ) -> (usize, u32) {
        if n == 0 {
            return (inp, acc);
        }
        let base = w.as_mut_ptr();
        let mut ip: *mut i16;
        // SAFETY: the caller checked the ranges; registers named.
        unsafe {
            let op = base.add(o);
            asm!(
                ".set push",
                ".set noreorder",
                "1:",
                "srl $10, $8, 7",
                "lh $11, 0($4)",
                "andi $10, $10, 0x1F8",
                "addu $10, $10, $7",
                "lh $12, 0($10)",
                "lh $13, 2($4)",
                "lh $14, 2($10)",
                "mult $11, $12",
                "lh $11, 4($4)",
                "lh $12, 4($10)",
                "addu $8, $8, $9",
                "mflo $15",
                "mult $13, $14",
                "lh $13, 6($4)",
                "lh $14, 6($10)",
                "srl $25, $8, 16",
                "mflo $24",
                "mult $11, $12",
                "addu $15, $15, $24",
                "sll $25, $25, 1",
                "andi $8, $8, 0xFFFF",
                "mflo $24",
                "mult $13, $14",
                "addu $15, $15, $24",
                "addu $4, $4, $25",
                "addiu $5, $5, 2",
                "mflo $24",
                "addu $15, $15, $24",
                "sra $15, $15, 15",
                ".word 0x01e2782c", // max $15, $15, $2
                ".word 0x01e3782d", // min $15, $15, $3
                "bne $5, $6, 1b",
                "sh $15, -2($5)",
                ".set pop",
                inout("$4") base.add(inp) => ip,
                inout("$5") op => _,
                in("$6") op.add(n),
                in("$7") table.as_ptr(),
                inout("$8") acc,
                in("$9") incr,
                out("$10") _,
                out("$11") _,
                out("$12") _,
                out("$13") _,
                out("$14") _,
                out("$15") _,
                out("$24") _,
                out("$25") _,
                in("$2") -32768,
                in("$3") 32767,
                options(nostack),
            );
            ((ip.offset_from(base)) as usize, acc)
        }
    }

    /// `A_ENVMIXER` over `n` samples with constant gains `g` (left dry,
    /// right dry, left wet, right wet): `bus += (in * g) >> 15`, saturating,
    /// on the four buses at `bus`, `bus + 184`, `bus + 368`, `bus + 552`
    /// (MAIN_L, MAIN_R, AUX_L, AUX_R).
    ///
    /// # Safety
    /// `input[..n]` and the four bus ranges are valid and disjoint.
    #[inline(always)]
    pub unsafe fn env4_const(input: *const i16, bus: *mut i16, n: usize, g: [i32; 4]) {
        if n == 0 {
            return;
        }
        unsafe {
            asm!(
                ".set push",
                ".set noreorder",
                "1:",
                "lh $12, 0($4)",
                "lh $13, 0($5)",
                "mult $12, $7",
                "lh $14, 368($5)",
                "lh $15, 736($5)",
                "lh $24, 1104($5)",
                "mflo $25",
                "mult $12, $8",
                "sra $25, $25, 15",
                "addu $13, $13, $25",
                ".word 0x01a2682c", // max $13, $13, $2
                ".word 0x01a3682d", // min $13, $13, $3
                "sh $13, 0($5)",
                "mflo $25",
                "mult $12, $9",
                "sra $25, $25, 15",
                "addu $14, $14, $25",
                ".word 0x01c2702c", // max $14, $14, $2
                ".word 0x01c3702d", // min $14, $14, $3
                "sh $14, 368($5)",
                "mflo $25",
                "mult $12, $10",
                "sra $25, $25, 15",
                "addu $15, $15, $25",
                ".word 0x01e2782c", // max $15, $15, $2
                ".word 0x01e3782d", // min $15, $15, $3
                "sh $15, 736($5)",
                "addiu $4, $4, 2",
                "mflo $25",
                "sra $25, $25, 15",
                "addu $24, $24, $25",
                ".word 0x0302c02c", // max $24, $24, $2
                ".word 0x0303c02d", // min $24, $24, $3
                "addiu $5, $5, 2",
                "bne $4, $6, 1b",
                "sh $24, 1102($5)",
                ".set pop",
                inout("$4") input => _,
                inout("$5") bus => _,
                in("$6") input.add(n),
                in("$7") g[0],
                in("$8") g[1],
                in("$9") g[2],
                in("$10") g[3],
                out("$12") _,
                out("$13") _,
                out("$14") _,
                out("$15") _,
                out("$24") _,
                out("$25") _,
                in("$2") -32768,
                in("$3") 32767,
                options(nostack),
            );
        }
    }

    /// `A_ENVMIXER` over `n` samples with ramping volumes: per sample
    /// `lval += lstep`, `rval += rstep`, then each bus's gain is
    /// `clamp16(((val >> 16) * amt + 0x4000) >> 15)` and the bus gets
    /// `(in * gain) >> 15`, saturating (bus layout as [`env4_const`]).
    /// Returns the final `(lval, rval)`.
    ///
    /// # Safety
    /// As [`env4_const`].
    #[allow(clippy::too_many_arguments)]
    #[inline(always)]
    pub unsafe fn env4_ramp(
        input: *const i16,
        bus: *mut i16,
        n: usize,
        lval: i32,
        lstep: i32,
        rval: i32,
        rstep: i32,
        dry: i32,
        wet: i32,
    ) -> (i32, i32) {
        if n == 0 {
            return (lval, rval);
        }
        let (l, r): (i32, i32);
        unsafe {
            asm!(
                ".set push",
                ".set noreorder",
                "1:",
                "addu $7, $7, $8",
                "addu $9, $9, $10",
                "sra $14, $7, 16",
                "mult $14, $11",
                "sra $15, $9, 16",
                "lh $13, 0($4)",
                "lh $24, 0($5)",
                "mflo $16",
                "mult $14, $12",
                "addiu $16, $16, 0x4000",
                "sra $16, $16, 15",
                ".word 0x0202802c", // max $16, $16, $2
                ".word 0x0203802d", // min $16, $16, $3
                "mflo $17",
                "mult $13, $16",
                "addiu $17, $17, 0x4000",
                "sra $17, $17, 15",
                ".word 0x0222882c", // max $17, $17, $2
                ".word 0x0223882d", // min $17, $17, $3
                "mflo $25",
                "mult $15, $11",
                "sra $25, $25, 15",
                "addu $24, $24, $25",
                ".word 0x0302c02c", // max $24, $24, $2
                ".word 0x0303c02d", // min $24, $24, $3
                "sh $24, 0($5)",
                "lh $24, 736($5)",
                "mflo $16",
                "mult $13, $17",
                "addiu $16, $16, 0x4000",
                "sra $16, $16, 15",
                ".word 0x0202802c", // max $16, $16, $2
                ".word 0x0203802d", // min $16, $16, $3
                "mflo $25",
                "mult $15, $12",
                "sra $25, $25, 15",
                "addu $24, $24, $25",
                ".word 0x0302c02c", // max $24, $24, $2
                ".word 0x0303c02d", // min $24, $24, $3
                "sh $24, 736($5)",
                "lh $24, 368($5)",
                "mflo $17",
                "mult $13, $16",
                "addiu $17, $17, 0x4000",
                "sra $17, $17, 15",
                ".word 0x0222882c", // max $17, $17, $2
                ".word 0x0223882d", // min $17, $17, $3
                "mflo $25",
                "mult $13, $17",
                "sra $25, $25, 15",
                "addu $24, $24, $25",
                ".word 0x0302c02c", // max $24, $24, $2
                ".word 0x0303c02d", // min $24, $24, $3
                "sh $24, 368($5)",
                "lh $24, 1104($5)",
                "addiu $4, $4, 2",
                "addiu $5, $5, 2",
                "mflo $25",
                "sra $25, $25, 15",
                "addu $24, $24, $25",
                ".word 0x0302c02c", // max $24, $24, $2
                ".word 0x0303c02d", // min $24, $24, $3
                "bne $4, $6, 1b",
                "sh $24, 1102($5)",
                ".set pop",
                inout("$4") input => _,
                inout("$5") bus => _,
                in("$6") input.add(n),
                inout("$7") lval => l,
                in("$8") lstep,
                inout("$9") rval => r,
                in("$10") rstep,
                in("$11") dry,
                in("$12") wet,
                out("$13") _,
                out("$14") _,
                out("$15") _,
                out("$16") _,
                out("$17") _,
                out("$24") _,
                out("$25") _,
                in("$2") -32768,
                in("$3") 32767,
                options(nostack),
            );
        }
        (l, r)
    }

    /// [`section`] two samples per iteration, the second sample's multiplies
    /// filling the first's latencies; `n` even.
    ///
    /// # Safety
    /// `x[..n]`, `y[..n]` and `out[..n]` are valid and pairwise disjoint.
    #[inline(always)]
    unsafe fn section_pairs(x: *mut i16, y: *mut i16, out: *mut i16, n: usize, c: [i32; 3]) {
        unsafe {
            asm!(
                ".set push",
                ".set noreorder",
                "1:",
                "lh $8, 0($4)",
                "lh $9, 0($5)",
                "lh $2, 2($4)",
                "mult $8, $12",
                "lh $3, 2($5)",
                "lh $10, 0($6)",
                "lh $25, 2($6)",
                "addiu $4, $4, 4",
                "addiu $5, $5, 4",
                "mflo $11",
                "mult $2, $12",
                "sra $11, $11, 15",
                "addu $9, $9, $11",
                ".word 0x012f482c", // max $9, $9, $15
                ".word 0x0138482d", // min $9, $9, $24
                "sh $9, -4($5)",
                "addiu $6, $6, 4",
                "mflo $16",
                "mult $9, $13",
                "sra $16, $16, 15",
                "addu $3, $3, $16",
                ".word 0x006f182c", // max $3, $3, $15
                ".word 0x0078182d", // min $3, $3, $24
                "sh $3, -2($5)",
                "mflo $11",
                "mult $3, $13",
                "sra $11, $11, 15",
                "addu $8, $8, $11",
                ".word 0x010f402c", // max $8, $8, $15
                ".word 0x0118402d", // min $8, $8, $24
                "sh $8, -4($4)",
                "mflo $16",
                "mult $9, $14",
                "sra $16, $16, 15",
                "addu $2, $2, $16",
                ".word 0x004f102c", // max $2, $2, $15
                ".word 0x0058102d", // min $2, $2, $24
                "sh $2, -2($4)",
                "mflo $11",
                "mult $3, $14",
                "sra $11, $11, 15",
                "addu $10, $10, $11",
                ".word 0x014f502c", // max $10, $10, $15
                ".word 0x0158502d", // min $10, $10, $24
                "sh $10, -4($6)",
                "mflo $16",
                "sra $16, $16, 15",
                "addu $25, $25, $16",
                ".word 0x032fc82c", // max $25, $25, $15
                ".word 0x0338c82d", // min $25, $25, $24
                "bne $6, $7, 1b",
                "sh $25, -2($6)",
                ".set pop",
                inout("$4") x => _,
                inout("$5") y => _,
                inout("$6") out => _,
                in("$7") out.add(n),
                out("$2") _,
                out("$3") _,
                out("$8") _,
                out("$9") _,
                out("$10") _,
                out("$11") _,
                in("$12") c[0],
                in("$13") c[1],
                in("$14") c[2],
                in("$15") -32768,
                out("$16") _,
                in("$24") 32767,
                out("$25") _,
                options(nostack),
            );
        }
    }

    /// One plain reverb section (`reverb::section`): `y += x * ff`, then
    /// `x += y * fb`, then `out += y * gain`, saturating, per sample.
    #[inline(always)]
    pub fn section(x: &mut [i16], y: &mut [i16], out: &mut [i16], ff: i32, fb: i32, gain: i32) {
        let n = out.len().min(x.len()).min(y.len());
        if n == 0 {
            return;
        }
        if ff == 0 && gain == 0 {
            // `y` and `out` stay as they are: only `x += y * fb`.
            mix_gain(&mut x[..n], &y[..n], fb);
            return;
        }
        let pairs = n & !1;
        // SAFETY: three disjoint slices of at least `n` samples.
        unsafe {
            if pairs > 0 {
                section_pairs(
                    x.as_mut_ptr(),
                    y.as_mut_ptr(),
                    out.as_mut_ptr(),
                    pairs,
                    [ff, fb, gain],
                );
            }
            if n > pairs {
                let k = n - 1;
                section_one(
                    x.as_mut_ptr().add(k),
                    y.as_mut_ptr().add(k),
                    out.as_mut_ptr().add(k),
                    1,
                    [ff, fb, gain],
                );
            }
        }
    }

    /// [`section`] one sample per iteration.
    ///
    /// # Safety
    /// As [`section_pairs`]; `n > 0`.
    #[inline(always)]
    unsafe fn section_one(x: *mut i16, y: *mut i16, o: *mut i16, n: usize, c: [i32; 3]) {
        let (ff, fb, gain) = (c[0], c[1], c[2]);
        unsafe {
            asm!(
                ".set push",
                ".set noreorder",
                "1:",
                "lh $8, 0($4)",
                "lh $9, 0($5)",
                "mult $8, $12",
                "lh $10, 0($6)",
                "addiu $4, $4, 2",
                "addiu $5, $5, 2",
                "mflo $11",
                "sra $11, $11, 15",
                "addu $9, $9, $11",
                ".word 0x012f482c", // max $9, $9, $15
                ".word 0x0138482d", // min $9, $9, $24
                "mult $9, $13",
                "sh $9, -2($5)",
                "addiu $6, $6, 2",
                "mflo $11",
                "sra $11, $11, 15",
                "addu $8, $8, $11",
                ".word 0x010f402c", // max $8, $8, $15
                ".word 0x0118402d", // min $8, $8, $24
                "mult $9, $14",
                "sh $8, -2($4)",
                "mflo $11",
                "sra $11, $11, 15",
                "addu $10, $10, $11",
                ".word 0x014f502c", // max $10, $10, $15
                ".word 0x0158502d", // min $10, $10, $24
                "bne $6, $7, 1b",
                "sh $10, -2($6)",
                ".set pop",
                inout("$4") x => _,
                inout("$5") y => _,
                inout("$6") o => _,
                in("$7") o.add(n),
                out("$8") _,
                out("$9") _,
                out("$10") _,
                out("$11") _,
                in("$12") ff,
                in("$13") fb,
                in("$14") gain,
                in("$15") -32768,
                in("$24") 32767,
                options(nostack),
            );
        }
    }
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

    /// `ramp_run` equals `ramp` sample by sample, saturation included.
    #[test]
    fn ramp_run_matches_per_sample_ramp() {
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
            let r = ramp_run(&mut st, 0);
            let mut val = r.value;
            for k in 0..COUNT / 2 {
                let got = if k < r.sums {
                    val += r.step;
                    val >> 16
                } else {
                    r.then
                };
                assert_eq!(got, ramp(&mut want, 0) as i32, "{k} {a} {b} {c}");
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
