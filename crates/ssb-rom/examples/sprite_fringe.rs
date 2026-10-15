//! RE-517: the 2D sprite edge fringe, measured. Prints numbers only, never
//! ROM images (D-048).
//!
//! SSB64_ROM=... cargo run --release -p ssb-rom --example sprite_fringe -- raw
//! SSB64_ROM=... cargo run --release -p ssb-rom --example sprite_fringe -- calib PIXELS
//! SSB64_ROM=... cargo run --release -p ssb-rom --example sprite_fringe -- measure
//!
//! `raw`: what the ROM stores in each sprite's transparent texels, from the
//! raw bitmap and TLUT bytes. `calib`: the GE rectangle convention fitted to
//! a PPSSPP capture of the Training stock icon (`PIXELS`: `x y r g b` lines,
//! PSP pixels). `measure`: every candidate against the N64 frame.
use ssb_rom::archive::{Archive, File};
use ssb_rom::sprite::{self, Sprite, SpriteFile};
use ssb_rom::sprite_fringe::{
    score, Background, Combine, Filter, Flat, Gradient, PspDraw, Score, Target,
};
use ssb_rom::texture::{Format, Rgba8};
use std::collections::{BTreeMap, HashSet};

struct Item {
    class: &'static str,
    file: u32,
    at: u32,
    tlut: Option<(u32, u32)>,
    sprite: Sprite,
}

fn be16(d: &[u8], at: usize) -> usize {
    i16::from_be_bytes([d[at], d[at + 1]]).max(0) as usize
}

fn pointer(file: &File, at: u32) -> Option<usize> {
    file.intern_relocs
        .iter()
        .find(|r| r.at == at)
        .map(|r| r.target as usize)
}

fn gather(archive: &Archive) -> Vec<Item> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    let mut push = |class: &'static str, file: &File, at: u32, tlut: Option<(u32, u32)>| {
        if !seen.insert((file.id, at, tlut)) {
            return;
        }
        let s = match tlut {
            Some((lf, la)) => {
                let lut = archive.load(lf).unwrap();
                let bytes = &lut.data[la as usize..la as usize + 32];
                sprite::decode_with_tlut(file, at, &ssb_rom::texture::parse_tlut(bytes))
            }
            None => sprite::decode(file, at),
        };
        if let Ok(sprite) = s {
            out.push(Item {
                class,
                file: file.id,
                at,
                tlut,
                sprite,
            });
        }
    };
    for e in ssb_rom::fighter::FIGHTER_FILES.iter().take(12) {
        let main = archive.load(e.file).unwrap();
        let s = sprite::fighter_sprites(&main, e.offset).unwrap();
        if let Some((f, at)) = s.stock {
            let file = archive.load(f).unwrap();
            for &lut in &s.stock_luts {
                push("stock", &file, at, Some(lut));
            }
        }
        if let Some((f, at)) = s.emblem {
            push("emblem", &archive.load(f).unwrap(), at, None);
        }
    }
    let class_of = |f: &SpriteFile| -> &'static str {
        if f.file == sprite::PLAYER_DAMAGE.file {
            "damage"
        } else if f.file == sprite::TIMER.file {
            "timer"
        } else if [
            sprite::GAME_STATUS.file,
            sprite::ANNOUNCE_COMMON.file,
            sprite::BATTLE_PAUSE.file,
            sprite::PLAYER_TAGS.file,
            sprite::ITEM_ARROW.file,
            sprite::STOCKS_ZAKO.file,
        ]
        .contains(&f.file)
        {
            "hud"
        } else {
            "menu"
        }
    };
    for f in sprite::FILES {
        let file = archive.load(f.file).unwrap();
        for &at in f.offsets {
            push(class_of(f), &file, at, None);
        }
    }
    for scene in ssb_rom::menu_pack::MenuScene::ALL {
        for f in scene.sprites() {
            let file = archive.load(f.file).unwrap();
            for &at in f.offsets {
                push("menu", &file, at, None);
            }
        }
    }
    // Drawn `SP_FASTCOPY`, opaque, at runtime (`grWallpaperMakeDecideKind`).
    for f in sprite::TRAINING_WALLPAPER_FILES {
        push(
            "wallpaper",
            &archive.load(f).unwrap(),
            sprite::TRAINING_WALLPAPER_SPRITE,
            None,
        );
    }
    let training = archive.load(sprite::TRAINING_FILE).unwrap();
    for slot in sprite::training_sprite_slots() {
        if let Ok((f, at)) = sprite::training_sprite_ref(&training, slot) {
            push("training", &archive.load(f).unwrap(), at, None);
        }
    }
    out
}

fn fmt_name(s: &Sprite) -> String {
    format!("{:?}{}", s.format, s.size.bits())
}

/// Every transparent texel's stored colour, from the raw bytes.
fn raw(archive: &Archive, items: &[Item]) {
    // format -> (transparent texels, of them nonzero colour, sprites with any nonzero)
    let mut sum: BTreeMap<String, (u64, u64, u64, u64)> = BTreeMap::new();
    let mut nonzero_examples: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for it in items {
        let file = archive.load(it.file).unwrap();
        let d = &file.data;
        let base = it.at as usize;
        let nbitmaps = be16(d, base + 40);
        let (fmt, siz) = (d[base + 48], d[base + 49]);
        let format = Format::from_raw(fmt).unwrap();
        let tlut: Vec<u16> = if format == Format::Ci {
            let (lbytes, at) = match it.tlut {
                Some((lf, la)) => (archive.load(lf).unwrap().data, la as usize),
                None => (d.clone(), pointer(&file, it.at + 32).unwrap()),
            };
            let n = if siz == 0 || siz == 4 { 16 } else { 256 };
            (0..n)
                .map(|i| u16::from_be_bytes([lbytes[at + 2 * i], lbytes[at + 2 * i + 1]]))
                .collect()
        } else {
            Vec::new()
        };
        let key = format!("{:?}{}{}", format, siz, if siz == 4 { "c" } else { "" });
        let bitmaps = pointer(&file, it.at + 52).unwrap();
        let (mut transparent, mut nonzero) = (0u64, 0u64);
        let mut nonzero_values = HashSet::new();
        for strip in 0..nbitmaps {
            let bm = bitmaps + strip * sprite::BITMAP_SIZE;
            let width_img = be16(d, bm + 2);
            let rows = be16(d, bm + 12);
            let buf = pointer(&file, (bm + 8) as u32).unwrap();
            let bits = match siz {
                0 | 4 => 4,
                1 => 8,
                2 => 16,
                _ => 32,
            };
            let texels = width_img * rows;
            let bytes = if siz == 4 {
                &d[buf..buf + (width_img / 2) * rows / 2]
            } else {
                &d[buf..buf + (width_img * bits).div_ceil(8) * rows]
            };
            // (alpha zero, stored colour) per texel; 4c packs 2-bit texels.
            let texel = |i: usize| -> (bool, u32) {
                let nib = |i: usize| {
                    if siz == 4 {
                        // LEVEL: 2-bit -> 4-bit, `lbCommonDecodeBitmapSiz4b`.
                        let b = bytes[i / 4];
                        let v = (b >> (6 - 2 * (i % 4))) & 3;
                        [0u8, 5, 10, 15][v as usize]
                    } else {
                        let b = bytes[i / 2];
                        if i.is_multiple_of(2) {
                            b >> 4
                        } else {
                            b & 0xF
                        }
                    }
                };
                match (format, bits) {
                    (Format::Rgba, 16) => {
                        let w = u16::from_be_bytes([bytes[2 * i], bytes[2 * i + 1]]);
                        (w & 1 == 0, u32::from(w >> 1))
                    }
                    (Format::Rgba, 32) => {
                        let p = &bytes[4 * i..4 * i + 4];
                        (p[3] == 0, u32::from_be_bytes([0, p[0], p[1], p[2]]))
                    }
                    (Format::Ci, 4) => {
                        let w = tlut[usize::from(nib(i))];
                        (w & 1 == 0, u32::from(w >> 1))
                    }
                    (Format::Ci, 8) => {
                        let w = tlut[usize::from(bytes[i])];
                        (w & 1 == 0, u32::from(w >> 1))
                    }
                    (Format::Ia, 4) => {
                        let n = nib(i);
                        (n & 1 == 0, u32::from(n >> 1))
                    }
                    (Format::Ia, 8) => (bytes[i] & 0xF == 0, u32::from(bytes[i] >> 4)),
                    (Format::Ia, 16) => (bytes[2 * i + 1] == 0, u32::from(bytes[2 * i])),
                    // I has no alpha channel apart from its intensity.
                    (Format::I, 4) => (nib(i) == 0, 0),
                    (Format::I, 8) => (bytes[i] == 0, 0),
                    _ => (false, 0),
                }
            };
            let n = if siz == 4 {
                (bytes.len() * 4).min(texels)
            } else {
                texels
            };
            for i in 0..n {
                let (clear, colour) = texel(i);
                if clear {
                    transparent += 1;
                    if colour != 0 {
                        nonzero += 1;
                        nonzero_values.insert(colour);
                    }
                }
            }
        }
        let e = sum.entry(key.clone()).or_default();
        e.0 += 1;
        e.1 += transparent;
        e.2 += nonzero;
        if nonzero > 0 {
            e.3 += 1;
            let ex = nonzero_examples.entry(key).or_default();
            if ex.len() < 6 {
                let mut v: Vec<_> = nonzero_values.into_iter().collect();
                v.sort();
                v.truncate(4);
                ex.push(format!(
                    "{}:{:#x}{} {}x{} {}/{} values {:x?}",
                    it.file,
                    it.at,
                    it.tlut
                        .map(|t| format!(" lut {}:{:#x}", t.0, t.1))
                        .unwrap_or_default(),
                    it.sprite.width,
                    it.sprite.height,
                    nonzero,
                    transparent,
                    v
                ));
            }
        }
    }
    println!("format(raw siz)  sprites  transparent-texels  nonzero-colour  sprites-with-nonzero");
    for (k, (s, t, nz, snz)) in &sum {
        println!("{k:16} {s:7} {t:19} {nz:15} {snz:21}");
    }
    for (k, ex) in nonzero_examples {
        for e in ex {
            println!("  {k}: {e}");
        }
    }
}

/// Fits the GE rectangle convention to a capture of the Training stock
/// icon (Mario, costume 0, N64 (27, 185)) over a black background.
fn calib(items: &[Item], path: &str) {
    let icon = items
        .iter()
        .find(|i| i.class == "stock" && i.file == 296)
        .expect("Mario's stock icon");
    let img = sprite::combined_image(&icon.sprite);
    let pixels: Vec<(i32, i32, [u8; 3])> = std::fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter_map(|l| {
            let v: Vec<i32> = l.split_whitespace().map(|t| t.parse().unwrap()).collect();
            (v.len() == 5).then(|| (v[0], v[1], [v[2] as u8, v[3] as u8, v[4] as u8]))
        })
        .collect();
    let k = f64::from(ssb_engine::coord::SCREEN_SCALE);
    let ox0 = f64::from(ssb_engine::coord::n64_to_psp_x(27.0));
    let oy0 = f64::from(ssb_engine::coord::n64_to_psp_y(185.0));
    let snap16 = |o: f64| (o * 16.0).floor() / 16.0;
    let round16 = |o: f64| (o * 16.0).round() / 16.0;
    for (name, mode) in [
        ("centre, origin 1/16 floor", 0),
        ("centre, both ends 1/16 floor", 1),
        ("centre, both ends 1/16 round", 2),
        ("centre, unsnapped", 3),
    ] {
        let snap = |o: f64| match mode {
            2 => round16(o),
            3 => o,
            _ => snap16(o),
        };
        let (x0, y0) = (snap(ox0), snap(oy0));
        let (x1, y1) = if mode == 0 {
            (x0 + 8.0 * k, y0 + 10.0 * k)
        } else {
            (snap(ox0 + 8.0 * k), snap(oy0 + 10.0 * k))
        };
        let mut st = ssb_rom::sprite_fringe::Stats::default();
        let mut worst = Vec::new();
        for &(px, py, got) in &pixels {
            let (cx, cy) = (f64::from(px) + 0.5, f64::from(py) + 0.5);
            let want = if cx >= x0 && cx < x1 && cy >= y0 && cy < y1 {
                let (u, v) = (8.0 * (cx - x0) / (x1 - x0), 10.0 * (cy - y0) / (y1 - y0));
                let s = ssb_rom::n64_filter::sample_bilinear(
                    &img,
                    ((u - 0.5) * 32.0).floor() as i32,
                    ((v - 0.5) * 32.0).floor() as i32,
                );
                [0, 1, 2].map(|c| ((u32::from(s[c]) * u32::from(s[3]) + 127) / 255) as u8)
            } else {
                [0, 0, 0]
            };
            st.add(got, want);
            let d = (0..3).map(|c| got[c].abs_diff(want[c])).max().unwrap();
            if d > 2 {
                worst.push((px, py, got, want));
            }
        }
        println!(
            "{name:20} n {} within2 {} ge8 {} max {} mae {:.2}",
            st.pixels,
            st.within2,
            st.ge8,
            st.max,
            st.mae()
        );
        for w in worst.iter().take(6) {
            println!("    {w:?}");
        }
    }
}

const CANDIDATES: &[&str] = &[
    "A0 linear",
    "B1 bleed-near",
    "B2 bleed-mean",
    "P nearest",
    "S snap+bleed-near",
    "PM premult",
];

fn psp_variant(c: &str, img: &Rgba8, exact_5551: bool) -> (Rgba8, PspDraw) {
    let mut d = PspDraw {
        filter: Filter::Linear,
        premultiplied: false,
        snap: false,
    };
    let mut out = img.clone();
    match c {
        "B1 bleed-near" => sprite::bleed_transparent_rgb(&mut out),
        "B2 bleed-mean" => bleed_mean(&mut out, exact_5551),
        "P nearest" => d.filter = Filter::Nearest,
        "S snap+bleed-near" => {
            sprite::bleed_transparent_rgb(&mut out);
            d.snap = true;
        }
        "PM premult" => d.premultiplied = true,
        _ => {}
    }
    (out, d)
}

/// The alpha-weighted mean of a transparent texel's eight neighbours,
/// orthogonal ones counted twice; requantized to 5 bits when the sprite
/// is stored 5551.
fn bleed_mean(img: &mut Rgba8, q5: bool) {
    let (w, h) = (img.width as i32, img.height as i32);
    let src = img.clone();
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) as usize;
            if src.get(i)[3] != 0 {
                continue;
            }
            let mut acc = [0u32; 3];
            let mut wsum = 0u32;
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let (nx, ny) = (x + dx, y + dy);
                    if (dx, dy) == (0, 0) || nx < 0 || ny < 0 || nx >= w || ny >= h {
                        continue;
                    }
                    let p = src.get((ny * w + nx) as usize);
                    let wt = u32::from(p[3]) * if dx == 0 || dy == 0 { 2 } else { 1 };
                    for c in 0..3 {
                        acc[c] += u32::from(p[c]) * wt;
                    }
                    wsum += wt;
                }
            }
            if wsum == 0 {
                continue;
            }
            let mut c = acc.map(|v| ((v + wsum / 2) / wsum) as u8);
            if q5 {
                c = c.map(|v| {
                    let r = ((u32::from(v) * 31 + 127) / 255) as u8;
                    (r << 3) | (r >> 2)
                });
            }
            img.put(i, [c[0], c[1], c[2], 0]);
        }
    }
}

fn is_5551(img: &Rgba8) -> bool {
    img.pixels.as_chunks::<4>().0.iter().all(|p| {
        let c = |v: u8| (v >> 3) << 3 | v >> 5;
        c(p[0]) == p[0] && c(p[1]) == p[1] && c(p[2]) == p[2] && (p[3] == 0 || p[3] == 0xFF)
    })
}

type Table = BTreeMap<(String, &'static str, String, &'static str), Score>;

struct Work {
    img: Rgba8,
    combine: Combine,
    groups: Vec<String>,
    steps: Vec<(i32, i32)>,
}

fn work_items(items: &[Item], only: Option<&str>) -> Vec<Work> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for it in items {
        let s = &it.sprite;
        let img = sprite::combined_image(s);
        if !seen.insert((img.pixels.clone(), s.width, s.format as u8, s.color)) {
            continue;
        }
        let has_alpha = img.pixels.as_chunks::<4>().0.iter().any(|p| p[3] < 255);
        let combine = if sprite::uses_prim_color(s.format) {
            Combine::Tint {
                prim: [s.color[0], s.color[1], s.color[2], 255],
                env: [0, 0, 0],
            }
        } else {
            Combine::Decal
        };
        let mut groups = vec![it.class.to_string(), fmt_name(s)];
        // The file's attribute; most SObjs clear it before drawing.
        if s.attr & sprite::SP_FASTCOPY != 0 {
            groups.push("rom-fastcopy".into());
        }
        if s.format == Format::Ia {
            groups.push("ia-blend".into());
        }
        groups.push(if has_alpha { "alpha" } else { "opaque" }.into());
        groups.push("ALL".into());
        if let Some(o) = only {
            if !groups.iter().any(|g| g == o) {
                continue;
            }
        }
        let area = u32::from(s.width) * u32::from(s.height);
        // Placements: x and y phases swept independently for small
        // sprites, together for larger ones.
        let steps: Vec<(i32, i32)> = if area <= 1024 {
            (0..7).flat_map(|i| (0..7).map(move |j| (i, j))).collect()
        } else if area <= 16384 {
            (0..7).map(|i| (i, (i * 3) % 7)).collect()
        } else {
            (0..3).map(|i| (i, (i * 2) % 3)).collect()
        };
        out.push(Work {
            img,
            combine,
            groups,
            steps,
        });
    }
    out
}

fn measure_one(w: &Work, table: &mut Table) {
    let bgs: [(&'static str, Box<dyn Background>); 4] = [
        ("black", Box::new(Flat([0, 0, 0]))),
        ("grey", Box::new(Flat([128, 128, 128]))),
        ("white", Box::new(Flat([255, 255, 255]))),
        ("gradient", Box::new(Gradient)),
    ];
    let exact = is_5551(&w.img);
    let variants: Vec<(&'static str, Rgba8, PspDraw)> = CANDIDATES
        .iter()
        .map(|&c| {
            let (v, d) = psp_variant(c, &w.img, exact);
            (c, v, d)
        })
        .collect();
    for &(i, j) in &w.steps {
        // N64 positions 5 apart step the PSP phase by 0.18 pixel.
        let (x, y) = (20 + 5 * i, 16 + 5 * j);
        for (bname, bg) in &bgs {
            for t in [Target::Bilinear, Target::Nearest, Target::Area] {
                for (c, v, d) in &variants {
                    let sc = score(&w.img, v, w.combine, *d, t, x, y, bg.as_ref());
                    for g in &w.groups {
                        for b in [*bname, "all-bg"] {
                            table
                                .entry((g.clone(), *c, format!("{t:?}"), b))
                                .or_default()
                                .merge(&sc);
                        }
                    }
                }
            }
        }
    }
}

fn measure(items: &[Item], only: Option<&str>) {
    let work = work_items(items, only);
    let mut counted: BTreeMap<String, u32> = BTreeMap::new();
    for w in &work {
        for g in &w.groups {
            *counted.entry(g.clone()).or_default() += 1;
        }
    }
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let next = std::sync::atomic::AtomicUsize::new(0);
    let tables: Vec<Table> = std::thread::scope(|sc| {
        let hs: Vec<_> = (0..threads)
            .map(|_| {
                sc.spawn(|| {
                    let mut t = Table::new();
                    loop {
                        let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let Some(w) = work.get(i) else { break };
                        measure_one(w, &mut t);
                    }
                    t
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let mut table = Table::new();
    for t in tables {
        for (k, v) in t {
            table.entry(k).or_default().merge(&v);
        }
    }
    println!("unique sprites per group: {counted:?}");
    println!(
        "{:12} {:18} {:8} {:8} | {:>9} {:>7} {:>8} {:>7} {:>6} {:>4} | {:>8} {:>6} {:>7} {:>6} {:>4}",
        "group", "candidate", "target", "bg", "pixels", "mae", "ge8", "ge32", "in2%", "max", "edge-px",
        "e-mae", "e-ge8", "e-ge32", "emax"
    );
    for ((g, c, t, b), s) in &table {
        println!(
            "{:12} {:18} {:8} {:8} | {:9} {:7.3} {:8} {:7} {:6.2} {:4} | {:8} {:6.2} {:7} {:6} {:4}",
            g,
            c,
            t,
            b,
            s.all.pixels,
            s.all.mae(),
            s.all.ge8,
            s.all.ge32,
            100.0 * s.all.within2 as f64 / s.all.pixels.max(1) as f64,
            s.all.max,
            s.edge.pixels,
            s.edge.mae(),
            s.edge.ge8,
            s.edge.ge32,
            s.edge.max
        );
    }
}

/// Packed bytes (texels plus CLUT) of every unique sprite, black against
/// bled, per class: 8888 as `add_sprite_texture` stores them.
fn sizes(items: &[Item]) {
    use ssb_rom::psp_texture::{pack_rgba, palettize_lossless, Psm};
    let bytes = |img: &Rgba8| {
        let t = pack_rgba(img, Psm::Psm8888, true);
        let t = palettize_lossless(&t).unwrap_or(t);
        (t.data.len() + 4 * t.palette.len(), t.format as u8)
    };
    let mut seen = HashSet::new();
    let mut sum: BTreeMap<&str, (usize, usize, usize, u32)> = BTreeMap::new();
    for it in items {
        let img = sprite::combined_image(&it.sprite);
        if !seen.insert(img.pixels.clone()) {
            continue;
        }
        let (a, fa) = bytes(&img);
        let (b, fb) = bytes(&sprite::psp_image(&it.sprite));
        for c in [it.class, "ALL"] {
            let e = sum.entry(c).or_default();
            e.0 += 1;
            e.1 += a;
            e.2 += b;
            e.3 += u32::from(fa != fb);
        }
    }
    println!("class      sprites  black-bytes  bled-bytes  format-changed");
    for (c, (n, a, b, f)) in sum {
        println!("{c:10} {n:7} {a:12} {b:11} {f:15}");
    }
}

fn main() {
    let rom = std::env::var_os("SSB64_ROM").expect("SSB64_ROM");
    let data = std::fs::read(rom).unwrap();
    let info = ssb_rom::rom::identify(&data).unwrap();
    let archive = Archive::open(&data, info.region).unwrap();
    let items = gather(&archive);
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        Some("raw") => raw(&archive, &items),
        Some("calib") => calib(&items, &args.next().expect("PIXELS")),
        Some("measure") => measure(&items, args.next().as_deref()),
        Some("sizes") => sizes(&items),
        _ => eprintln!("raw | calib PIXELS | measure [GROUP]"),
    }
}
