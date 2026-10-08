//! The spans a framebuffer depth mask draws: an RGBA5551 colour image the
//! RDP reads as depth, scaled through the 3-point filter onto the screen's
//! pixels (the magnifiers, RE-470, RE-471).
//!
//! [`rows`] gives every row's maximal runs of equal depth in pixel order,
//! exactly as sampling each pixel through
//! [`n64_filter::sample_3point_quad`](crate::n64_filter::sample_3point_quad)
//! and merging equal neighbours would ([`rows_per_pixel`], the reference).
//! It samples per texel instead: a run of pixels whose 2 × 2 quad holds four
//! equal texels blends to that texel whatever its fraction, so its depth is
//! looked up once ([`Tables`]); only pixels on a quad of unequal texels are
//! filtered one by one.

use alloc::vec::Vec;

use crate::n64_filter::sample_3point_quad;
use crate::texture::Rgba8;

/// Where a mask lands, in screen pixels: its top-left corner `(x0, y0)`,
/// its far edges `(x1, y1)`, the screen pixels per N64 pixel `k`, and its
/// N64 size.
#[derive(Clone, Copy, Debug)]
pub struct Placement {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
    pub k: f32,
    pub width: f32,
    pub height: f32,
}

/// A row's spans: `(start, end, depth)`, end exclusive, in GE depth (the
/// RDP's 18-bit Z, inverted into 16 bits).
pub type Span = (i32, i32, u16);

/// The most spans [`rows`] hands over at once.
pub const SPANS_PER_CALL: usize = 128;

/// `v` rounded up, as the GE's span end.
fn ceil(v: f32) -> i32 {
    let i = v as i32;
    if v > i as f32 {
        i + 1
    } else {
        i
    }
}

/// A filtered sample's depth: the RGBA5551 word the colour packs to, read
/// as compressed Z, in the PSP's inverted 16-bit range.
fn depth_of(rgba: [u8; 4]) -> u16 {
    let packed = (u16::from(rgba[0] >> 3) << 11)
        | (u16::from(rgba[1] >> 3) << 6)
        | (u16::from(rgba[2] >> 3) << 1)
        | u16::from(rgba[3] >= 128);
    u16::MAX - (crate::n64_depth::decode(packed) >> 2) as u16
}

/// The bits of a colour [`depth_of`] reads: red's and green's top five,
/// blue's top four (the RGBA5551 word less its two low bits).
fn depth_bits(c: [u8; 4]) -> [u8; 3] {
    [c[0] >> 3, c[1] >> 3, c[2] >> 4]
}

/// Per quad of a mask image, its depth when every sample of its four
/// (edge-clamped) texels has the same depth, else its texels. Quads are indexed by their top-left texel
/// from `-1` to `width - 1` (and `height - 1`): every quad further out
/// clamps to the same four texels as the one at the edge.
pub struct Tables {
    width: i32,
    height: i32,
    /// A uniform quad's depth, or `!i` for `mixed[i]`.
    quads: Vec<i32>,
    /// The texels of the quads that are not uniform: `c00`, `c10`, `c01`,
    /// `c11`.
    mixed: Vec<[[u8; 4]; 4]>,
}

impl Tables {
    pub fn new(image: &Rgba8) -> Self {
        let (w, h) = (image.width as i32, image.height as i32);
        let texel = |x: i32, y: i32| -> [u8; 4] {
            let (x, y) = (x.clamp(0, w - 1), y.clamp(0, h - 1));
            image.get((y * w + x) as usize)
        };
        let mut quads = Vec::with_capacity(((w + 1) * (h + 1)) as usize);
        let mut mixed = Vec::new();
        for t0 in -1..h {
            for s0 in -1..w {
                let q = [
                    texel(s0, t0),
                    texel(s0 + 1, t0),
                    texel(s0, t0 + 1),
                    texel(s0 + 1, t0 + 1),
                ];
                // The depth reads red's and green's top five bits and
                // blue's top four (`depth_of`). Each channel of a 3-point
                // blend lies between its three texels' (a convex weighting,
                // rounded to nearest), so when the four texels agree in
                // those bits every sample does: the quad has one depth.
                if q[1..].iter().all(|&c| depth_bits(c) == depth_bits(q[0])) {
                    quads.push(i32::from(depth_of(q[0])));
                } else {
                    quads.push(!(mixed.len() as i32));
                    mixed.push(q);
                }
            }
        }
        Self {
            width: w,
            height: h,
            quads,
            mixed,
        }
    }

    /// The quad at texel `(s0, t0)`: a uniform quad's depth, or `!i` for
    /// `mixed[i]`.
    #[inline(always)]
    fn quad(&self, s0: i32, t0: i32) -> i32 {
        let s = (s0.clamp(-1, self.width - 1) + 1) as usize;
        let t = (t0.clamp(-1, self.height - 1) + 1) as usize;
        self.quads[t * (self.width + 1) as usize + s]
    }
}

/// [`sample_3point_addressed`](crate::n64_filter::sample_3point_addressed)'s
/// blend of a quad of unequal texels, `[c00, c10, c01, c11]`. Each channel
/// is its texels weighted convexly and rounded to nearest, so it lies
/// between them and the hardware's clamp to 0..255 never acts.
#[inline(always)]
fn blend(q: &[[u8; 4]; 4], sfrac: i32, tfrac: i32) -> [u8; 4] {
    let [c00, c10, c01, c11] = q.map(|c| c.map(i32::from));
    let mut out = [0u8; 4];
    if sfrac + tfrac < 32 {
        for c in 0..4 {
            let v = c00[c] + ((sfrac * (c10[c] - c00[c]) + tfrac * (c01[c] - c00[c]) + 0x10) >> 5);
            debug_assert!((0..=255).contains(&v));
            out[c] = v as u8;
        }
    } else {
        let (invs, invt) = (32 - sfrac, 32 - tfrac);
        for c in 0..4 {
            let v = c11[c] + ((invs * (c01[c] - c11[c]) + invt * (c10[c] - c11[c]) + 0x10) >> 5);
            debug_assert!((0..=255).contains(&v));
            out[c] = v as u8;
        }
    }
    out
}

/// A pixel column's `s` (1/32 texel).
fn s_at(p: &Placement, image: &Rgba8, px: i32) -> i32 {
    (((px as f32 + 0.5 - p.x0) / p.k) * image.width as f32 / p.width * 32.0) as i32
}

/// A pixel row's `t` (1/32 texel).
fn t_at(p: &Placement, image: &Rgba8, py: i32) -> i32 {
    (((py as f32 + 0.5 - p.y0) / p.k) * image.height as f32 / p.height * 32.0) as i32
}

/// Gathers a row's spans, merging equal neighbours, and hands them to
/// `emit` [`SPANS_PER_CALL`] at a time. One buffer serves every row.
struct Row<'a, F: FnMut(i32, &[Span])> {
    py: i32,
    spans: &'a mut Vec<Span>,
    open: Option<Span>,
    emit: &'a mut F,
}

impl<F: FnMut(i32, &[Span])> Row<'_, F> {
    /// Pixels `start..end` have `depth`.
    #[inline(always)]
    fn put(&mut self, start: i32, end: i32, depth: u16) {
        match &mut self.open {
            Some(open) if open.2 == depth => open.1 = end,
            open => {
                if let Some(done) = open.replace((start, end, depth)) {
                    self.spans.push(done);
                    if self.spans.len() == SPANS_PER_CALL {
                        (self.emit)(self.py, self.spans);
                        self.spans.clear();
                    }
                }
            }
        }
    }

    fn finish(&mut self) {
        if let Some(done) = self.open.take() {
            self.spans.push(done);
        }
        if !self.spans.is_empty() {
            (self.emit)(self.py, self.spans);
            self.spans.clear();
        }
    }
}

/// Every row's spans of `image` placed at `p`, top to bottom, through
/// `emit(row, spans)`; a row's spans may come in several calls, in order.
pub fn rows(image: &Rgba8, tables: &Tables, p: &Placement, mut emit: impl FnMut(i32, &[Span])) {
    let first = p.x0 as i32;
    let end = ceil(p.x1);
    if first >= end {
        return;
    }
    // The columns' `s`, and their runs of one integer texel `s0`: the same
    // on every row.
    let columns: Vec<i32> = (first..end).map(|px| s_at(p, image, px)).collect();
    let mut runs: Vec<(i32, usize, usize)> = Vec::new();
    for (i, &s) in columns.iter().enumerate() {
        let s0 = s.div_euclid(32);
        match runs.last_mut() {
            Some(run) if run.0 == s0 => run.2 = i + 1,
            _ => runs.push((s0, i, i + 1)),
        }
    }
    // Per band of rows on one integer texel `t0`, the row's segments:
    // `(first column, end column, quad)`, uniform neighbours of one depth
    // merged. Built when a row first reaches the band.
    let bands = (tables.height + 1) as usize;
    let mut segments: Vec<(usize, usize, i32)> = Vec::new();
    let mut band_at: Vec<Option<(usize, usize)>> = alloc::vec![None; bands];
    let mut spans = Vec::with_capacity(SPANS_PER_CALL);
    for py in (p.y0 as i32)..ceil(p.y1) {
        let t = t_at(p, image, py);
        let t0 = t.div_euclid(32);
        let band = (t0.clamp(-1, tables.height - 1) + 1) as usize;
        let (lo, hi) = *band_at[band].get_or_insert_with(|| {
            let lo = segments.len();
            for &(s0, a, b) in &runs {
                let quad = tables.quad(s0, t0);
                let len = segments.len();
                match segments.last_mut() {
                    Some(last) if lo < len && quad >= 0 && last.2 == quad => last.1 = b,
                    _ => segments.push((a, b, quad)),
                }
            }
            (lo, segments.len())
        });
        let mut row = Row {
            py,
            spans: &mut spans,
            open: None,
            emit: &mut emit,
        };
        let tfrac = t.rem_euclid(32);
        for &(a, b, quad) in &segments[lo..hi] {
            if quad >= 0 {
                row.put(first + a as i32, first + b as i32, quad as u16);
            } else {
                let q = &tables.mixed[!quad as usize];
                for (i, &s) in columns[a..b].iter().enumerate() {
                    let px = first + (a + i) as i32;
                    row.put(px, px + 1, depth_of(blend(q, s.rem_euclid(32), tfrac)));
                }
            }
        }
        row.finish();
    }
}

/// [`rows`] the slow way, one filtered sample per pixel: the reference the
/// tests hold it to.
pub fn rows_per_pixel(image: &Rgba8, p: &Placement, mut emit: impl FnMut(i32, &[Span])) {
    let mut spans = Vec::with_capacity(SPANS_PER_CALL);
    for py in (p.y0 as i32)..ceil(p.y1) {
        let t = t_at(p, image, py);
        let mut row = Row {
            py,
            spans: &mut spans,
            open: None,
            emit: &mut emit,
        };
        for px in (p.x0 as i32)..ceil(p.x1) {
            let s = s_at(p, image, px);
            row.put(px, px + 1, depth_of(sample_3point_quad(image, s, t).0));
        }
        row.finish();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A mask like the magnifier's: a disc of near depth with a soft edge,
    /// far outside, and a few odd texels.
    fn disc(size: u32) -> Rgba8 {
        let mut img = Rgba8::new(size, size);
        let c = size as f32 / 2.0 - 0.5;
        for y in 0..size {
            for x in 0..size {
                let d = ((x as f32 - c).powi(2) + (y as f32 - c).powi(2)).sqrt();
                let v: [u8; 4] = if d < c - 2.0 {
                    [0x10, 0x20, 0x30, 0xFF]
                } else if d < c - 1.0 {
                    [0x80, 0x40, 0x20, 0xFF]
                } else if d < c {
                    [0xC0, 0x70, 0x18, 0x40]
                } else {
                    [0xFF, 0xFF, 0xFF, 0xFF]
                };
                let i = ((y * size + x) * 4) as usize;
                img.pixels[i..i + 4].copy_from_slice(&v);
            }
        }
        // Texels the depth cannot tell from the disc (alpha, blue's low
        // bit) and ones it can (blue's bit 4, green's bit 3).
        if size >= 16 {
            for (x, y, v) in [
                (8, 8, [0x10, 0x20, 0x30, 0x00]),
                (9, 8, [0x10, 0x20, 0x38, 0xFF]),
                (12, 9, [0x10, 0x20, 0x20, 0xFF]),
                (9, 12, [0x10, 0x28, 0x30, 0xFF]),
                (14, 14, [0x17, 0x27, 0x3F, 0x80]),
            ] {
                let i = ((y * size + x) * 4) as usize;
                img.pixels[i..i + 4].copy_from_slice(&v);
            }
        }
        img.pixels[0..4].copy_from_slice(&[1, 2, 3, 4]);
        let last = img.pixels.len() - 4;
        img.pixels[last..].copy_from_slice(&[9, 9, 9, 9]);
        img
    }

    fn collect(f: impl FnOnce(&mut dyn FnMut(i32, &[Span]))) -> Vec<(i32, Span)> {
        let mut out = Vec::new();
        f(&mut |py, spans: &[Span]| out.extend(spans.iter().map(|&s| (py, s))));
        out
    }

    fn placements() -> impl Iterator<Item = Placement> {
        use ssb_engine::coord::{n64_to_psp_x, n64_to_psp_y, SCREEN_SCALE};
        let k = SCREEN_SCALE;
        [0.4f32, 0.75, 1.0, 1.3, 2.0, 2.37, 3.9, 6.0]
            .into_iter()
            .flat_map(move |scale| {
                [
                    (-20.3f32, 3.7f32),
                    (10.0, 10.0),
                    (100.55, 50.123),
                    (300.0, 200.9),
                ]
                .into_iter()
                .map(move |(x, y)| {
                    let (w, h) = (32.0 * scale, 32.0 * scale);
                    let x0 = n64_to_psp_x(x);
                    let y0 = n64_to_psp_y(y);
                    Placement {
                        x0,
                        y0,
                        x1: x0 + w * k,
                        y1: y0 + h * k,
                        k,
                        width: w,
                        height: h,
                    }
                })
            })
    }

    #[test]
    fn rows_match_sampling_every_pixel() {
        for img in [disc(32), disc(7), Rgba8::new(1, 1)] {
            let tables = Tables::new(&img);
            for p in placements() {
                let fast = collect(|e| rows(&img, &tables, &p, e));
                let slow = collect(|e| rows_per_pixel(&img, &p, e));
                assert!(!slow.is_empty());
                assert_eq!(fast, slow, "{p:?}");
            }
        }
    }

    #[test]
    fn spans_are_maximal() {
        let img = disc(32);
        let tables = Tables::new(&img);
        for p in placements() {
            let spans = collect(|e| rows(&img, &tables, &p, e));
            for w in spans.windows(2) {
                let ((ya, a), (yb, b)) = (w[0], w[1]);
                if ya == yb {
                    assert_eq!(a.1, b.0);
                    assert_ne!(a.2, b.2);
                }
            }
        }
    }
}
