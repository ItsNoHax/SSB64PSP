//! A host model of how a 2D sprite (`SObj`) looks on the N64 and on the
//! PSP, for choosing how the PSP filters and stores sprites (RE-517).
//!
//! The N64 draws a sprite with `gSPTextureRectangle` one texel per pixel,
//! so its `G_TF_BILERP` lands on texel centres and the frame holds the
//! texels exactly, alpha-compared at the blend colour's alpha 8
//! (`lbCommonStartSprite`) and blended `G_RM_XLU_SURF` over what is behind
//! (`lbCommonPrepSObjAttr`). The PSP draws the same sprite
//! [`SCREEN_SCALE`](ssb_engine::coord::SCREEN_SCALE) times larger, a
//! non-integer 1.236, so every PSP pixel samples between texels.
//!
//! The targets are the N64 frame seen at PSP resolution, three ways:
//! [`Target::Bilinear`] (the frame resampled; the primary target, D-047),
//! [`Target::Nearest`] (each PSP pixel takes its N64 pixel) and
//! [`Target::Area`] (each PSP pixel averages the N64 pixels it covers).
//! [`draw_psp`] models the GE's draw: its rectangle coverage, bilinear
//! sampling through [`n64_filter::sample_bilinear`], the texture function
//! and the `SrcAlpha`/`OneMinusSrcAlpha` blend.
//!
//! Nothing here reads the ROM; callers pass decoded sprites.

use crate::n64_filter;
use crate::texture::Rgba8;
use ssb_engine::coord::{n64_to_psp_x, n64_to_psp_y, SCREEN_SCALE};

/// The N64 combiner a sprite is drawn through (`lbCommonPrepSObjAttr`),
/// mirrored on the PSP by the texture function.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Combine {
    /// `G_CC_DECALRGBA` (RGBA, CI): the texel as it is; PSP `Modulate`
    /// against white.
    Decal,
    /// I and IA: `(PRIM - ENV) * TEXEL0 + ENV` in colour, `TEXEL0 * PRIM`
    /// in alpha; PSP `Blend` with the vertex colour `env` and the texture
    /// environment colour `prim`.
    Tint { prim: [u8; 4], env: [u8; 3] },
}

impl Combine {
    /// The combiner's output for one unfiltered texel.
    pub fn apply(self, t: [u8; 4]) -> [u8; 4] {
        match self {
            Combine::Decal => t,
            Combine::Tint { prim, env } => {
                let mix = |c: usize| {
                    let (e, p, k) = (u32::from(env[c]), u32::from(prim[c]), u32::from(t[c]));
                    ((e * (255 - k) + p * k + 127) / 255) as u8
                };
                [
                    mix(0),
                    mix(1),
                    mix(2),
                    ((u32::from(t[3]) * u32::from(prim[3]) + 127) / 255) as u8,
                ]
            }
        }
    }
}

/// The GE's texture filter for the sprite.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Filter {
    Linear,
    Nearest,
}

/// One way the PSP could draw a sprite.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PspDraw {
    pub filter: Filter,
    /// Texels premultiplied by their alpha (after the combiner) and blended
    /// `One`/`OneMinusSrcAlpha`. Not shippable with the `Blend` texture
    /// function; the ideal that bilinear-of-the-frame equals.
    pub premultiplied: bool,
    /// The rectangle's origin rounded to a whole PSP pixel.
    pub snap: bool,
}

/// How the N64 frame is resampled to PSP pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    Bilinear,
    Nearest,
    Area,
}

/// A background behind the sprite, as a colour per N64 pixel.
pub trait Background {
    fn at(&self, x: i32, y: i32) -> [u8; 3];
}

/// One colour everywhere.
pub struct Flat(pub [u8; 3]);

impl Background for Flat {
    fn at(&self, _: i32, _: i32) -> [u8; 3] {
        self.0
    }
}

/// A smooth colour gradient across the N64 screen.
pub struct Gradient;

impl Background for Gradient {
    fn at(&self, x: i32, y: i32) -> [u8; 3] {
        let x = x.clamp(0, 319) as u32;
        let y = y.clamp(0, 239) as u32;
        [
            (x * 255 / 319) as u8,
            (y * 255 / 239) as u8,
            (255 - (x + y) * 255 / 558) as u8,
        ]
    }
}

fn blend8(s: u8, d: u8, a: u8) -> u8 {
    ((u32::from(s) * u32::from(a) + u32::from(d) * (255 - u32::from(a)) + 127) / 255) as u8
}

/// The N64 frame around a sprite drawn at the N64 pixel `(x, y)`: the
/// combined texel blended by its alpha over `bg`, texels with alpha under
/// 8 discarded by the alpha compare.
pub struct N64Frame {
    /// The frame's N64 origin and size, two pixels past the sprite.
    pub x0: i32,
    pub y0: i32,
    pub w: i32,
    pub h: i32,
    pub rgb: Vec<[u8; 3]>,
    /// The drawn alpha (0 where the alpha compare discards).
    pub alpha: Vec<u8>,
}

const MARGIN: i32 = 2;

impl N64Frame {
    pub fn new(image: &Rgba8, combine: Combine, x: i32, y: i32, bg: &dyn Background) -> Self {
        let (iw, ih) = (image.width as i32, image.height as i32);
        let (x0, y0) = (x - MARGIN, y - MARGIN);
        let (w, h) = (iw + 2 * MARGIN, ih + 2 * MARGIN);
        let mut rgb = Vec::with_capacity((w * h) as usize);
        let mut alpha = Vec::with_capacity((w * h) as usize);
        for j in 0..h {
            for i in 0..w {
                let b = bg.at(x0 + i, y0 + j);
                let (u, v) = (i - MARGIN, j - MARGIN);
                if u < 0 || v < 0 || u >= iw || v >= ih {
                    rgb.push(b);
                    alpha.push(0);
                    continue;
                }
                let t = combine.apply(image.get((v * iw + u) as usize));
                if t[3] < 8 {
                    rgb.push(b);
                    alpha.push(0);
                } else {
                    rgb.push([0, 1, 2].map(|c| blend8(t[c], b[c], t[3])));
                    alpha.push(t[3]);
                }
            }
        }
        N64Frame {
            x0,
            y0,
            w,
            h,
            rgb,
            alpha,
        }
    }

    fn texel(&self, i: i32, j: i32, bg: &dyn Background) -> ([f64; 3], f64) {
        let (li, lj) = (i - self.x0, j - self.y0);
        if li < 0 || lj < 0 || li >= self.w || lj >= self.h {
            let b = bg.at(i, j);
            return (b.map(f64::from), 0.0);
        }
        let k = (lj * self.w + li) as usize;
        (self.rgb[k].map(f64::from), f64::from(self.alpha[k]))
    }

    /// The frame (and its drawn alpha) seen at the PSP pixel `(px, py)`.
    pub fn sample(&self, t: Target, px: i32, py: i32, bg: &dyn Background) -> ([u8; 3], f64) {
        let k = f64::from(SCREEN_SCALE);
        let cx = f64::from(n64_to_psp_x(160.0));
        let cy = f64::from(n64_to_psp_y(120.0));
        let x = 160.0 + (f64::from(px) + 0.5 - cx) / k;
        let y = 120.0 + (f64::from(py) + 0.5 - cy) / k;
        let (c, a) = match t {
            Target::Nearest => self.texel(x.floor() as i32, y.floor() as i32, bg),
            Target::Bilinear => {
                let (sx, sy) = (x - 0.5, y - 0.5);
                let (i0, j0) = (sx.floor(), sy.floor());
                let (fx, fy) = (sx - i0, sy - j0);
                let (i0, j0) = (i0 as i32, j0 as i32);
                let mut c = [0.0; 3];
                let mut a = 0.0;
                for (di, dj, wgt) in [
                    (0, 0, (1.0 - fx) * (1.0 - fy)),
                    (1, 0, fx * (1.0 - fy)),
                    (0, 1, (1.0 - fx) * fy),
                    (1, 1, fx * fy),
                ] {
                    let (tc, ta) = self.texel(i0 + di, j0 + dj, bg);
                    for ch in 0..3 {
                        c[ch] += tc[ch] * wgt;
                    }
                    a += ta * wgt;
                }
                (c, a)
            }
            Target::Area => {
                let half = 0.5 / k;
                let (xa, xb, ya, yb) = (x - half, x + half, y - half, y + half);
                let mut c = [0.0; 3];
                let mut a = 0.0;
                let mut total = 0.0;
                for j in ya.floor() as i32..=yb.floor() as i32 {
                    let wy = (yb.min(f64::from(j + 1)) - ya.max(f64::from(j))).max(0.0);
                    for i in xa.floor() as i32..=xb.floor() as i32 {
                        let wx = (xb.min(f64::from(i + 1)) - xa.max(f64::from(i))).max(0.0);
                        let wgt = wx * wy;
                        let (tc, ta) = self.texel(i, j, bg);
                        for ch in 0..3 {
                            c[ch] += tc[ch] * wgt;
                        }
                        a += ta * wgt;
                        total += wgt;
                    }
                }
                (c.map(|v| v / total), a / total)
            }
        };
        (c.map(|v| v.round().clamp(0.0, 255.0) as u8), a)
    }

    /// The PSP pixels the frame covers, `[x0, y0, x1, y1)`.
    pub fn psp_bounds(&self) -> [i32; 4] {
        [
            n64_to_psp_x(self.x0 as f32).floor() as i32,
            n64_to_psp_y(self.y0 as f32).floor() as i32,
            n64_to_psp_x((self.x0 + self.w) as f32).ceil() as i32,
            n64_to_psp_y((self.y0 + self.h) as f32).ceil() as i32,
        ]
    }
}

/// The GE's vertex positions: 12.4 fixed point, truncated.
fn ge_vertex(v: f64) -> f64 {
    (v * 16.0).floor() / 16.0
}

/// The image the PSP samples for `draw`: the combiner applied first when
/// premultiplying (the texture function cannot follow a premultiplied
/// texel), else the image as given.
fn psp_texture(image: &Rgba8, combine: Combine, draw: PspDraw) -> Rgba8 {
    if !draw.premultiplied {
        return image.clone();
    }
    let mut out = image.clone();
    for p in out.pixels.as_chunks_mut::<4>().0 {
        let t = combine.apply(*p);
        let pm = |c: u8| ((u32::from(c) * u32::from(t[3]) + 127) / 255) as u8;
        *p = [pm(t[0]), pm(t[1]), pm(t[2]), t[3]];
    }
    out
}

/// The PSP's draw of an image at an N64 pixel, as `draw_sprite_xy` submits
/// it: one `GU_SPRITES` rectangle from `n64_to_psp_*` of the corner,
/// `SCREEN_SCALE` texels wide per texel. The GE convention is fitted to a
/// PPSSPP (software GE) capture of the Training stock icon, 180 of 180
/// pixels within 1 level (RE-517): the corners truncate to 1/16 pixel, a
/// pixel is drawn when its centre lies in `[x0, x1)`, and its UV is
/// interpolated between the truncated corners at the centre.
pub struct PspSprite {
    tex: Rgba8,
    combine: Combine,
    draw: PspDraw,
    /// The rectangle's corners as the GE holds them.
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    /// The texture's size: the UVs at `(x1, y1)`.
    tw: f64,
    th: f64,
}

impl PspSprite {
    pub fn new(image: &Rgba8, combine: Combine, draw: PspDraw, x: f32, y: f32) -> Self {
        let (mut ox, mut oy) = (f64::from(n64_to_psp_x(x)), f64::from(n64_to_psp_y(y)));
        if draw.snap {
            ox = ox.round();
            oy = oy.round();
        }
        let k = f64::from(SCREEN_SCALE);
        let (tw, th) = (f64::from(image.width), f64::from(image.height));
        PspSprite {
            tex: psp_texture(image, combine, draw),
            combine,
            draw,
            x0: ge_vertex(ox),
            y0: ge_vertex(oy),
            x1: ge_vertex(ox + tw * k),
            y1: ge_vertex(oy + th * k),
            tw,
            th,
        }
    }

    pub fn draw_over(&self, px: i32, py: i32, dest: [u8; 3]) -> [u8; 3] {
        let (cx, cy) = (f64::from(px) + 0.5, f64::from(py) + 0.5);
        if cx < self.x0 || cy < self.y0 || cx >= self.x1 || cy >= self.y1 {
            return dest;
        }
        // UVs interpolate between the corners at the pixel's centre.
        let u = self.tw * (cx - self.x0) / (self.x1 - self.x0);
        let v = self.th * (cy - self.y0) / (self.y1 - self.y0);
        let s = match self.draw.filter {
            Filter::Linear => n64_filter::sample_bilinear(
                &self.tex,
                ((u - 0.5) * 32.0).floor() as i32,
                ((v - 0.5) * 32.0).floor() as i32,
            ),
            Filter::Nearest => {
                let i = (u.floor() as i32).clamp(0, self.tex.width as i32 - 1);
                let j = (v.floor() as i32).clamp(0, self.tex.height as i32 - 1);
                self.tex.get((j * self.tex.width as i32 + i) as usize)
            }
        };
        if self.draw.premultiplied {
            return [0, 1, 2].map(|c| {
                let d = (u32::from(dest[c]) * (255 - u32::from(s[3])) + 127) / 255;
                (u32::from(s[c]) + d).min(255) as u8
            });
        }
        let t = self.combine.apply(s);
        [0, 1, 2].map(|c| blend8(t[c], dest[c], t[3]))
    }
}

/// Differences between a PSP draw and a target over a set of pixels.
#[derive(Debug, Clone, Copy, Default)]
pub struct Stats {
    pub pixels: u64,
    /// Sum over pixels of the mean absolute channel difference.
    pub abs_sum: f64,
    pub ge8: u64,
    pub ge32: u64,
    pub within2: u64,
    pub max: u8,
}

impl Stats {
    pub fn add(&mut self, a: [u8; 3], b: [u8; 3]) {
        let d = [0, 1, 2].map(|c| a[c].abs_diff(b[c]));
        let m = d.into_iter().max().unwrap_or(0);
        self.pixels += 1;
        self.abs_sum += d.iter().map(|&v| f64::from(v)).sum::<f64>() / 3.0;
        self.ge8 += u64::from(m >= 8);
        self.ge32 += u64::from(m >= 32);
        self.within2 += u64::from(m <= 2);
        self.max = self.max.max(m);
    }

    pub fn merge(&mut self, o: &Stats) {
        self.pixels += o.pixels;
        self.abs_sum += o.abs_sum;
        self.ge8 += o.ge8;
        self.ge32 += o.ge32;
        self.within2 += o.within2;
        self.max = self.max.max(o.max);
    }

    pub fn mae(&self) -> f64 {
        if self.pixels == 0 {
            0.0
        } else {
            self.abs_sum / self.pixels as f64
        }
    }
}

/// All pixels and the edge pixels (drawn alpha strictly between 0 and 255
/// in the bilinear target) of one comparison.
#[derive(Debug, Clone, Copy, Default)]
pub struct Score {
    pub all: Stats,
    pub edge: Stats,
}

impl Score {
    pub fn merge(&mut self, o: &Score) {
        self.all.merge(&o.all);
        self.edge.merge(&o.edge);
    }
}

/// Scores `image` drawn by the PSP as `draw` (from `psp_image`, which may
/// differ from `image` by a pack-time transform) against the N64 frame of
/// `image`, at the N64 pixel `(x, y)` over `bg`, under `target`.
#[allow(clippy::too_many_arguments)]
pub fn score(
    image: &Rgba8,
    psp_image: &Rgba8,
    combine: Combine,
    draw: PspDraw,
    target: Target,
    x: i32,
    y: i32,
    bg: &dyn Background,
) -> Score {
    let frame = N64Frame::new(image, combine, x, y, bg);
    let empty = Rgba8::new(1, 1);
    let bg_frame = N64Frame::new(&empty, Combine::Decal, x, y, bg);
    let sprite = PspSprite::new(psp_image, combine, draw, x as f32, y as f32);
    let [bx0, by0, bx1, by1] = frame.psp_bounds();
    let mut s = Score::default();
    for py in by0..by1 {
        for px in bx0..bx1 {
            let (want, _) = frame.sample(target, px, py, bg);
            let (_, cover) = frame.sample(Target::Bilinear, px, py, bg);
            let (dest, _) = bg_frame.sample(target, px, py, bg);
            let got = sprite.draw_over(px, py, dest);
            s.all.add(got, want);
            if cover > 0.5 && cover < 254.5 {
                s.edge.add(got, want);
            }
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 4 x 4 white square inside a transparent black 8 x 8 image.
    fn square(rgb: [u8; 3]) -> Rgba8 {
        let mut img = Rgba8::new(8, 8);
        for y in 2..6 {
            for x in 2..6 {
                img.put(y * 8 + x, [rgb[0], rgb[1], rgb[2], 255]);
            }
        }
        img
    }

    #[test]
    fn premultiplied_linear_tracks_the_bilinear_target() {
        let img = square([255, 255, 255]);
        let draw = PspDraw {
            filter: Filter::Linear,
            premultiplied: true,
            snap: false,
        };
        let s = score(
            &img,
            &img,
            Combine::Decal,
            draw,
            Target::Bilinear,
            40,
            40,
            &Flat([255; 3]),
        );
        // Only the GE's 4-bit weights separate it from the ideal.
        assert!(s.all.max <= 16, "{s:?}");
        assert!(s.all.mae() < 2.0, "{s:?}");
    }

    const LINEAR: PspDraw = PspDraw {
        filter: Filter::Linear,
        premultiplied: false,
        snap: false,
    };
    const NEAREST: PspDraw = PspDraw {
        filter: Filter::Nearest,
        premultiplied: false,
        snap: false,
    };
    const PREMULTIPLIED: PspDraw = PspDraw {
        filter: Filter::Linear,
        premultiplied: true,
        snap: false,
    };

    /// `image` stored as `stored` and drawn as `draw`, at 7 x 7 sub-pixel
    /// phases over black, grey, white and a gradient.
    fn sweep(image: &Rgba8, stored: &Rgba8, combine: Combine, draw: PspDraw, t: Target) -> Score {
        let bgs: [&dyn Background; 4] =
            [&Flat([0; 3]), &Flat([128; 3]), &Flat([255; 3]), &Gradient];
        let mut s = Score::default();
        for i in 0..7 {
            for j in 0..7 {
                for bg in bgs {
                    s.merge(&score(
                        image,
                        stored,
                        combine,
                        draw,
                        t,
                        20 + 5 * i,
                        16 + 5 * j,
                        bg,
                    ));
                }
            }
        }
        s
    }

    fn bled(image: &Rgba8) -> Rgba8 {
        let mut out = image.clone();
        crate::sprite::bleed_transparent_rgb(&mut out);
        out
    }

    #[test]
    fn bleeding_removes_the_black_fringe_of_a_filtered_sprite() {
        // Break the bleed (keep the black) and the edge error comes back:
        // the test can tell the two apart.
        let img = square([240, 200, 40]);
        let t = Target::Bilinear;
        let black = sweep(&img, &img, Combine::Decal, LINEAR, t);
        let bleed = sweep(&img, &bled(&img), Combine::Decal, LINEAR, t);
        let ideal = sweep(&img, &img, Combine::Decal, PREMULTIPLIED, t);
        let nearest = sweep(&img, &img, Combine::Decal, NEAREST, t);
        assert!(
            bleed.edge.mae() < 0.5 * black.edge.mae(),
            "{bleed:?} {black:?}"
        );
        assert!(bleed.all.ge32 < black.all.ge32, "{bleed:?} {black:?}");
        // The bled texture comes within a level of the premultiplied ideal.
        assert!(
            bleed.all.mae() < ideal.all.mae() + 1.0,
            "{bleed:?} {ideal:?}"
        );
        // Point sampling is further from the resampled N64 frame still.
        assert!(
            nearest.all.mae() > 2.0 * bleed.all.mae(),
            "{nearest:?} {bleed:?}"
        );
    }

    /// RE-517 on the ROM: every fighter's stock icons in every costume,
    /// against the N64 frame resampled bilinearly (the primary target).
    #[test]
    fn bled_stock_icons_beat_black_ones_on_the_rom() {
        let Some(path) = std::env::var_os("SSB64_ROM") else {
            return;
        };
        let data = std::fs::read(path).unwrap();
        let info = crate::rom::identify(&data).unwrap();
        let archive = crate::archive::Archive::open(&data, info.region).unwrap();
        let (mut black, mut bleed, mut nearest) =
            (Score::default(), Score::default(), Score::default());
        for e in crate::fighter::FIGHTER_FILES.iter().take(12) {
            let main = archive.load(e.file).unwrap();
            let s = crate::sprite::fighter_sprites(&main, e.offset).unwrap();
            let (file, at) = s.stock.unwrap();
            let model = archive.load(file).unwrap();
            // The default costume keeps the test quick.
            for &(lf, la) in s.stock_luts.iter().take(1) {
                let lut = archive.load(lf).unwrap();
                let tlut = crate::texture::parse_tlut(&lut.data[la as usize..la as usize + 32]);
                let sprite = crate::sprite::decode_with_tlut(&model, at, &tlut).unwrap();
                let img = crate::sprite::combined_image(&sprite);
                assert_eq!(crate::sprite::psp_image(&sprite), bled(&img));
                let t = Target::Bilinear;
                black.merge(&sweep(&img, &img, Combine::Decal, LINEAR, t));
                bleed.merge(&sweep(&img, &bled(&img), Combine::Decal, LINEAR, t));
                nearest.merge(&sweep(&img, &img, Combine::Decal, NEAREST, t));
            }
        }
        // Measured over all 87 icons: mean 3.38 -> 1.49, edge 21.3 -> 8.7,
        // >= 32 on 6.6% -> 1.9% of pixels; point sampling 6.70.
        assert!(
            bleed.all.mae() < 0.6 * black.all.mae(),
            "{bleed:?} {black:?}"
        );
        assert!(
            bleed.edge.mae() < 0.6 * black.edge.mae(),
            "{bleed:?} {black:?}"
        );
        assert!(bleed.all.ge32 * 2 < black.all.ge32, "{bleed:?} {black:?}");
        assert!(
            nearest.all.mae() > 2.0 * bleed.all.mae(),
            "{nearest:?} {bleed:?}"
        );
    }

    #[test]
    fn nearest_matches_the_nearest_target_exactly() {
        let img = square([200, 40, 90]);
        let draw = PspDraw {
            filter: Filter::Nearest,
            premultiplied: false,
            snap: false,
        };
        for x in 30..40 {
            let s = score(
                &img,
                &img,
                Combine::Decal,
                draw,
                Target::Nearest,
                x,
                50,
                &Gradient,
            );
            assert_eq!(s.all.max, 0, "x {x}: {s:?}");
        }
    }
}
