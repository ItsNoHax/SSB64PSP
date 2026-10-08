//! The sequence player's tremolo and vibrato oscillators (`src/sys/audio.c:509-753`:
//! `syAudioDepth2Cents`, `syAudioInitOsc`, `syAudioUpdateOsc`,
//! `syAudioStopOsc`), `alCents2Ratio` (`src/libultra/audio/cents2ratio.c`)
//! and libultra's `sinf` (`src/libultra/gu/sinf.c`).
//!
//! The 32 oscillator states (`SYAUDIO_OSC_STATES_NUM`) are a LIFO free list
//! linked in index order (`sSYAudioOscStatesAllocFree`, audio.c:959-964).

/// `TREMELO_SIN` .. `VIBRATO_ASC_SAW` (`src/sys/audio.h:14-21`).
pub const TREMELO_SIN: u8 = 1;
pub const TREMELO_SQR: u8 = 2;
pub const TREMELO_DSC_SAW: u8 = 3;
pub const TREMELO_ASC_SAW: u8 = 4;
pub const VIBRATO_SIN: u8 = 128;
pub const VIBRATO_SQR: u8 = 129;
pub const VIBRATO_DSC_SAW: u8 = 130;
pub const VIBRATO_ASC_SAW: u8 = 131;
const OSC_HIGH: u8 = 0;
const OSC_LOW: u8 = 1;

/// `SYAUDIO_OSC_STATES_NUM`.
pub const OSC_STATES: usize = 32;
/// `AL_USEC_PER_FRAME`.
const AL_USEC_PER_FRAME: i32 = 16000;
/// `AL_VOL_FULL`.
const AL_VOL_FULL: u8 = 127;
/// `F_CST_DTOR32(360.0F)`: `(float)(360 * DTOR64)`.
const TWO_PI: f32 = (360.0 * (core::f64::consts::PI / 180.0)) as f32;

const NIL: u8 = 0xFF;

/// `alCents2Ratio`: square and multiply in f32. The constants are the C's
/// literals, rounded to f32 as the compiler does.
#[allow(clippy::excessive_precision)]
pub fn cents2ratio(cents: i32) -> f32 {
    let mut ratio = 1.0f32;
    let (mut x, mut c) = if cents >= 0 {
        (1.000_577_79_f32, cents as u32)
    } else {
        (0.999_422_544_1_f32, cents.unsigned_abs())
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

/// `syAudioDepth2Cents`: `1.03099303^depth` by square and multiply (the
/// result is used as cents, as the original's name says).
#[allow(clippy::excessive_precision)]
pub fn depth2cents(depth: u8) -> f32 {
    let mut x = 1.030_993_03_f32;
    let mut cents = 1.0f32;
    let mut d = depth;
    while d != 0 {
        if d & 1 != 0 {
            cents *= x;
        }
        x *= x;
        d >>= 1;
    }
    cents
}

/// The polynomial of libultra's `sinf` (`P[]`, IEEE doubles), rounded to
/// f32 at compile time.
const P1: f32 = f64::from_bits(0xbfc5_5554_bc83_656d) as f32;
const P2: f32 = f64::from_bits(0x3f81_10ed_3804_c2a0) as f32;
const P3: f32 = f64::from_bits(0xbf29_f6ff_eea5_6814) as f32;
const P4: f32 = f64::from_bits(0x3ec5_dbdf_0e31_4bfe) as f32;
/// `rpi` = 1/pi.
const RPI: f32 = f64::from_bits(0x3fd4_5f30_6dc9_c883) as f32;
/// Cody-Waite split of pi for f32: `PIHI` has 8 significant bits, so
/// `n * PIHI` is exact for the small `n` here.
const PIHI: f32 = 3.140_625;
const PILO: f32 = (core::f64::consts::PI - 3.140_625) as f32;

/// libultra's `sinf` (`fsin`), evaluated in f32. The original computes the
/// same range reduction and polynomial in double precision and rounds once;
/// the PSP has no double-precision FPU, so this port evaluates it in f32
/// (D-049 deviation: the result can differ from the N64 in the last bits;
/// see `sinf_matches_libultra_in_vibrato_use`).
pub fn sinf(x: f32) -> f32 {
    let ix = x.to_bits() as i32;
    let xpt = (ix >> 22) & 0x1FF;
    let poly = |dx: f32| {
        let xsq = dx * dx;
        let poly = ((P4 * xsq + P3) * xsq + P2) * xsq + P1;
        dx + (dx * xsq) * poly
    };
    if xpt < 0xFF {
        // |x| < 1.5
        if xpt >= 0xE6 {
            poly(x)
        } else {
            x
        }
    } else if xpt < 0x136 {
        // |x| < 2^28
        let dn = x * RPI;
        // `ROUND`: half away from zero.
        let n = if dn >= 0.0 {
            (dn + 0.5) as i32
        } else {
            (dn - 0.5) as i32
        };
        let dn = n as f32;
        let dx = x - dn * PIHI - dn * PILO;
        let r = poly(dx);
        if n & 1 == 0 {
            r
        } else {
            -r
        }
    } else if x.is_nan() {
        x
    } else {
        0.0
    }
}

/// The type-specific part of `SYAudioOsc` (`src/sys/audio.h:97-147`).
#[derive(Debug, Clone, Copy)]
enum OscData {
    None,
    TremSin { halfdepth: u8, base_vol: u8 },
    TremSqr { hi_val: u8, lo_val: u8 },
    TremSaw { base_vol: u8, depth: u8 },
    VibSin { depthcents: f32 },
    VibSqr { lo_ratio: f32, hi_ratio: f32 },
    VibDscSaw { hicents: i32, centsrange: i32 },
    VibAscSaw { locents: i32, centsrange: i32 },
}

/// `SYAudioOsc`.
#[derive(Debug, Clone, Copy)]
struct Osc {
    next: u8,
    kind: u8,
    state_flags: u8,
    max_count: u16,
    cur_count: u16,
    data: OscData,
}

/// What `syAudioInitOsc` hands back.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OscInit {
    /// The return value: `oscDelay * 0x4000` microseconds, 0 when no state
    /// was free (or the delay is 0: "don't run the osc").
    pub delta: i32,
    /// `*oscState`: the state taken from the free list, if any. A state is
    /// taken even when `delta` is 0 (zero delay); the caller then never
    /// posts an event for it and it is never returned to the list, exactly
    /// as on the N64.
    pub state: Option<u8>,
    /// `*initVal`, written only for the eight known types when a state was
    /// free.
    pub value: Option<f32>,
}

/// The oscillator pool and the three callbacks.
pub struct OscPool {
    oscs: [Osc; OSC_STATES],
    free: u8,
    /// `syAudioInitOsc` calls that found no free state.
    pub exhausted: u32,
}

impl Default for OscPool {
    fn default() -> Self {
        Self::new()
    }
}

impl OscPool {
    /// audio.c:959-964: states linked in index order, state 0 first.
    pub fn new() -> Self {
        let mut oscs = [Osc {
            next: NIL,
            kind: 0,
            state_flags: 0,
            max_count: 0,
            cur_count: 0,
            data: OscData::None,
        }; OSC_STATES];
        for (i, o) in oscs.iter_mut().enumerate() {
            o.next = if i + 1 < OSC_STATES { i as u8 + 1 } else { NIL };
        }
        Self {
            oscs,
            free: 0,
            exhausted: 0,
        }
    }

    /// States on the free list (diagnostic).
    pub fn free_count(&self) -> usize {
        let mut n = 0;
        let mut at = self.free;
        while at != NIL {
            n += 1;
            at = self.oscs[at as usize].next;
        }
        n
    }

    /// `syAudioInitOsc`.
    pub fn init(&mut self, osc_type: u8, rate: u8, depth: u8, delay: u8) -> OscInit {
        let s = self.free;
        if s == NIL {
            self.exhausted += 1;
            return OscInit {
                delta: 0,
                state: None,
                value: None,
            };
        }
        self.free = self.oscs[s as usize].next;
        let o = &mut self.oscs[s as usize];
        o.kind = osc_type;
        let delta = delay as i32 * 0x4000;
        let value = match osc_type {
            TREMELO_SIN => {
                o.cur_count = 0;
                o.max_count = 259 - rate as u16;
                let halfdepth = depth >> 1;
                let base_vol = AL_VOL_FULL.wrapping_sub(halfdepth);
                o.data = OscData::TremSin {
                    halfdepth,
                    base_vol,
                };
                Some(base_vol as f32)
            }
            TREMELO_SQR => {
                o.max_count = 256 - rate as u16;
                o.cur_count = o.max_count;
                o.state_flags = OSC_HIGH;
                o.data = OscData::TremSqr {
                    lo_val: AL_VOL_FULL.wrapping_sub(depth),
                    hi_val: AL_VOL_FULL,
                };
                Some(AL_VOL_FULL as f32)
            }
            TREMELO_DSC_SAW | TREMELO_ASC_SAW => {
                o.max_count = 256 - rate as u16;
                o.cur_count = 0;
                let base_vol = if osc_type == TREMELO_DSC_SAW {
                    AL_VOL_FULL
                } else {
                    AL_VOL_FULL.wrapping_sub(depth)
                };
                o.data = OscData::TremSaw { base_vol, depth };
                Some(base_vol as f32)
            }
            VIBRATO_SIN => {
                o.data = OscData::VibSin {
                    depthcents: depth2cents(depth),
                };
                o.cur_count = 0;
                o.max_count = 259 - rate as u16;
                Some(1.0)
            }
            VIBRATO_SQR => {
                o.max_count = 256 - rate as u16;
                o.cur_count = o.max_count;
                o.state_flags = OSC_HIGH;
                let cents = depth2cents(depth) as i32;
                let hi_ratio = cents2ratio(cents);
                o.data = OscData::VibSqr {
                    lo_ratio: cents2ratio(cents.wrapping_neg()),
                    hi_ratio,
                };
                Some(hi_ratio)
            }
            VIBRATO_DSC_SAW => {
                o.max_count = 256 - rate as u16;
                o.cur_count = o.max_count;
                let cents = depth2cents(depth) as i32;
                o.data = OscData::VibDscSaw {
                    hicents: cents,
                    centsrange: cents.wrapping_mul(2),
                };
                Some(cents2ratio(cents))
            }
            VIBRATO_ASC_SAW => {
                o.max_count = 256 - rate as u16;
                o.cur_count = o.max_count;
                let cents = depth2cents(depth) as i32;
                o.data = OscData::VibAscSaw {
                    locents: cents.wrapping_neg(),
                    centsrange: cents.wrapping_mul(2),
                };
                Some(cents2ratio(cents.wrapping_neg()))
            }
            _ => {
                o.data = OscData::None;
                None
            }
        };
        OscInit {
            delta,
            state: Some(s),
            value,
        }
    }

    /// `syAudioUpdateOsc`: the microseconds to the next update and the new
    /// value (`None` for an unknown type, whose `*updateVal` the C leaves
    /// as the caller's stack garbage).
    pub fn update(&mut self, state: u8) -> (i32, Option<f32>) {
        let o = &mut self.oscs[state as usize];
        let mut delta = AL_USEC_PER_FRAME;
        let value = match o.data {
            OscData::TremSin {
                halfdepth,
                base_vol,
            } => {
                o.cur_count += 1;
                if o.cur_count >= o.max_count {
                    o.cur_count = 0;
                }
                let t = o.cur_count as f32 / o.max_count as f32;
                let t = sinf(t * TWO_PI) * halfdepth as f32;
                Some(base_vol as f32 + t)
            }
            OscData::TremSqr { hi_val, lo_val } => {
                let v = if o.state_flags == OSC_HIGH {
                    o.state_flags = OSC_LOW;
                    lo_val
                } else {
                    o.state_flags = OSC_HIGH;
                    hi_val
                };
                delta = delta.wrapping_mul(o.max_count as i32);
                Some(v as f32)
            }
            OscData::TremSaw { base_vol, depth } => {
                o.cur_count += 1;
                if o.cur_count > o.max_count {
                    o.cur_count = 0;
                }
                let t = o.cur_count as f32 / o.max_count as f32 * depth as f32;
                Some(if o.kind == TREMELO_DSC_SAW {
                    base_vol as f32 - t
                } else {
                    base_vol as f32 + t
                })
            }
            OscData::VibSin { depthcents } => {
                o.cur_count += 1;
                if o.cur_count >= o.max_count {
                    o.cur_count = 0;
                }
                let t = o.cur_count as f32 / o.max_count as f32;
                let t = sinf(t * TWO_PI) * depthcents;
                Some(cents2ratio(t as i32))
            }
            OscData::VibSqr { lo_ratio, hi_ratio } => {
                let v = if o.state_flags == OSC_HIGH {
                    o.state_flags = OSC_LOW;
                    lo_ratio
                } else {
                    o.state_flags = OSC_HIGH;
                    hi_ratio
                };
                delta = delta.wrapping_mul(o.max_count as i32);
                Some(v)
            }
            OscData::VibDscSaw {
                hicents,
                centsrange,
            } => {
                o.cur_count += 1;
                if o.cur_count > o.max_count {
                    o.cur_count = 0;
                }
                let t = o.cur_count as f32 / o.max_count as f32 * centsrange as f32;
                Some(cents2ratio((hicents as f32 - t) as i32))
            }
            OscData::VibAscSaw {
                locents,
                centsrange,
            } => {
                o.cur_count += 1;
                if o.cur_count > o.max_count {
                    o.cur_count = 0;
                }
                let t = o.cur_count as f32 / o.max_count as f32 * centsrange as f32;
                Some(cents2ratio((t + locents as f32) as i32))
            }
            OscData::None => None,
        };
        (delta, value)
    }

    /// `syAudioStopOsc`: back on the head of the free list.
    pub fn stop(&mut self, state: u8) {
        self.oscs[state as usize].next = self.free;
        self.free = state;
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;

    /// libultra's `fsin` exactly as written (double precision), as the
    /// test oracle.
    fn libultra_sinf(x: f32) -> f32 {
        let p = [
            f64::from_bits(0xbfc5_5554_bc83_656d),
            f64::from_bits(0x3f81_10ed_3804_c2a0),
            f64::from_bits(0xbf29_f6ff_eea5_6814),
            f64::from_bits(0x3ec5_dbdf_0e31_4bfe),
        ];
        let rpi = f64::from_bits(0x3fd4_5f30_6dc9_c883);
        let pihi = f64::from_bits(0x4009_21fb_5000_0000);
        let pilo = f64::from_bits(0x3e61_10b4_611a_6263);
        let ix = x.to_bits() as i32;
        let xpt = (ix >> 22) & 0x1FF;
        let poly = |dx: f64| {
            let xsq = dx * dx;
            let poly = ((p[3] * xsq + p[2]) * xsq + p[1]) * xsq + p[0];
            dx + (dx * xsq) * poly
        };
        if xpt < 0xFF {
            if xpt >= 0xE6 {
                poly(x as f64) as f32
            } else {
                x
            }
        } else if xpt < 0x136 {
            let dn = x as f64 * rpi;
            let n = if dn >= 0.0 {
                (dn + 0.5) as i32
            } else {
                (dn - 0.5) as i32
            };
            let dn = n as f64;
            let dx = x as f64 - dn * pihi - dn * pilo;
            let r = poly(dx);
            if n & 1 == 0 {
                r as f32
            } else {
                -(r as f32)
            }
        } else {
            0.0
        }
    }

    /// Over every input `syAudioUpdateOsc` can pass (`cur / max * 2pi`,
    /// `max` 4..=259), the f32 port stays within a few ulp of the
    /// double-precision original (absolute 1e-6 near the zeros), and the
    /// vibrato's `(s32)(sin * depthcents)` rarely differs.
    #[test]
    fn sinf_matches_libultra_in_vibrato_use() {
        let mut worst = 0f32;
        let mut cents_diff = 0;
        let mut total = 0;
        for max in 4u16..=259 {
            for cur in 0..max {
                let x = cur as f32 / max as f32 * TWO_PI;
                let (a, b) = (sinf(x), libultra_sinf(x));
                worst = worst.max((a - b).abs());
                for depth in [50u8, 75, 100, 127] {
                    let dc = depth2cents(depth);
                    total += 1;
                    if (a * dc) as i32 != (b * dc) as i32 {
                        cents_diff += 1;
                    }
                }
            }
        }
        std::eprintln!(
            "sinf: worst abs error {worst:e}, vibrato cents differ in {cents_diff}/{total}"
        );
        assert!(worst < 4e-7, "worst {worst:e}");
        assert!(cents_diff * 1000 <= total, "{cents_diff}/{total}");
    }

    #[test]
    fn cents2ratio_octaves() {
        assert_eq!(cents2ratio(0), 1.0);
        assert!((cents2ratio(1200) - 2.0).abs() < 1e-4);
        assert!((cents2ratio(-1200) - 0.5).abs() < 1e-4);
    }

    #[test]
    #[allow(clippy::excessive_precision)]
    fn depth2cents_is_a_power() {
        assert_eq!(depth2cents(0), 1.0);
        assert_eq!(depth2cents(1), 1.030_993_03);
        let d = depth2cents(100);
        assert!((d - 1.030_993_03_f32.powi(100)).abs() / d < 1e-4, "{d}");
    }

    /// The pool is LIFO from state 0; an empty pool returns delta 0 and
    /// leaves the init value alone; a zero delay still takes a state.
    #[test]
    fn osc_pool_and_values() {
        let mut p = OscPool::new();
        let a = p.init(VIBRATO_SIN, 242, 100, 1);
        assert_eq!(a.state, Some(0));
        assert_eq!(a.delta, 0x4000);
        assert_eq!(a.value, Some(1.0));
        let b = p.init(VIBRATO_SIN, 242, 100, 0);
        assert_eq!((b.state, b.delta), (Some(1), 0));
        p.stop(0);
        assert_eq!(p.init(TREMELO_SQR, 200, 20, 1).state, Some(0));
        for _ in 2..OSC_STATES {
            assert!(p.init(TREMELO_SIN, 0, 0, 1).state.is_some());
        }
        let none = p.init(VIBRATO_SIN, 242, 100, 5);
        assert_eq!(
            none,
            OscInit {
                delta: 0,
                state: None,
                value: None
            }
        );
        assert_eq!(p.exhausted, 1);
        // Square tremolo: low first, period scaled by maxCount.
        assert_eq!(p.update(0), (16000 * 56, Some(107.0)));
        assert_eq!(p.update(0), (16000 * 56, Some(127.0)));
        // Vibrato sine (state 1, maxCount 17): the first update is count 1.
        let (d, v) = p.update(1);
        assert_eq!(d, 16000);
        let want = cents2ratio((sinf(1.0 / 17.0 * TWO_PI) * depth2cents(100)) as i32);
        assert_eq!(v, Some(want));
    }
}
