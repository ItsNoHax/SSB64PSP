//! The RDP colour combiner's `NOISE` input, for combiners the GE cannot
//! express (the select screens' locked-portrait shadows,
//! `mnPlayersVSPortraitProcDisplay`).
//!
//! RDP model (angrylion-rdp-plus `dither.c` `get_dither_noise`,
//! `combiner.c` `color_combiner_equation`):
//! - one 3-bit random per pixel, `((rand & 7) << 6) | 0x20`, shared by R,
//!   G and B, drawn anew each frame;
//! - read as a 9-bit combiner input: levels 0x1A0 and 0x1E0 sign-extend to
//!   -96 and -32 (`special_9bit_exttable`);
//! - `((A - B) * C + (D << 8) + 0x80) >> 8`, clamped to 0..=255.
//!
//! BattleShip's shader approximates `NOISE` as a continuous uniform 0..1;
//! this module keeps the RDP's eight levels (mean 27, not 24, under the
//! shadows' `PRIM` 0x30 share).

/// The eight `NOISE` levels as signed 9-bit combiner inputs, by the
/// random's low three bits.
pub const LEVELS: [i32; 8] = [32, 96, 160, 224, 288, 352, -96, -32];

/// `(NOISE - TEXEL0) * share + TEXEL0` for one channel: the RDP's 1-cycle
/// colour combiner with `NOISE` level `noise` and an 8-bit `share`.
#[inline]
pub fn combine(texel: u8, noise: i32, share: u8) -> u8 {
    let t = i32::from(texel);
    let v = ((noise - t) * i32::from(share) + (t << 8) + 0x80) >> 8;
    v.clamp(0, 255) as u8
}

/// The per-pixel random source: xorshift32, three bits a pixel. Not the
/// game's `syUtilsRandom` stream, which the RDP never touches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoiseRng(u32);

impl NoiseRng {
    /// A generator from a non-zero `seed` (zero is replaced).
    pub const fn new(seed: u32) -> Self {
        NoiseRng(if seed == 0 { 0x2545_F491 } else { seed })
    }

    /// The next pixel's `NOISE` level, as an index into [`LEVELS`].
    #[inline]
    pub fn level_index(&mut self) -> usize {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        (x >> 29) as usize
    }

    /// The next pixel's `NOISE` level.
    #[inline]
    pub fn level(&mut self) -> i32 {
        LEVELS[self.level_index()]
    }
}

/// [`combine`] for one `share`, tabled by level and texel channel: a
/// texel's three channels are three loads.
#[derive(Clone)]
pub struct Lut {
    share: u8,
    table: [[u8; 256]; 8],
}

impl Lut {
    pub fn new(share: u8) -> Self {
        let mut table = [[0u8; 256]; 8];
        for (row, &n) in table.iter_mut().zip(&LEVELS) {
            for (t, v) in row.iter_mut().enumerate() {
                *v = combine(t as u8, n, share);
            }
        }
        Lut { share, table }
    }

    /// The `share` the table was built for.
    pub fn share(&self) -> u8 {
        self.share
    }

    /// [`texel_rgba8888`] through the table.
    #[inline]
    pub fn texel_rgba8888(&self, word: u32, rng: &mut NoiseRng) -> u32 {
        let row = &self.table[rng.level_index()];
        u32::from(row[(word & 0xFF) as usize])
            | u32::from(row[(word >> 8 & 0xFF) as usize]) << 8
            | u32::from(row[(word >> 16 & 0xFF) as usize]) << 16
            | (word & 0xFF00_0000)
    }
}

/// One RGBA8888 texel word (`[r, g, b, a]` little-endian) through
/// `(NOISE - TEXEL0) * share + TEXEL0` in colour and `TEXEL0` in alpha.
#[inline]
pub fn texel_rgba8888(word: u32, share: u8, rng: &mut NoiseRng) -> u32 {
    let [r, g, b, a] = word.to_le_bytes();
    let n = rng.level();
    u32::from_le_bytes([
        combine(r, n, share),
        combine(g, n, share),
        combine(b, n, share),
        a,
    ])
}

#[cfg(test)]
#[path = "rdp_noise_tests.rs"]
mod tests;
