//! `romtool matcolors`: stage material colour tracks against an independent
//! decomp-semantics reference (RE-322).
//!
//! The reference below is written from `objanim.c`'s
//! `gcParseMObjMatAnimJoint`/`gcPlayMObjMatAnim` directly, including the
//! first-frame `AOBJ_ANIM_CHANGED` path and the `length = -anim_wait -
//! anim_speed` convention, and shares no code with
//! `ssb_rom::matanim::MaterialJoint`. It models only the colour-window
//! commands stage colour scripts use and declines anything else.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use ssb_rom::archive::Archive;
use ssb_rom::pack::{flags, Pack};

type Res = Result<(), Box<dyn std::error::Error>>;

const TRACK_NAMES: [&str; 5] = [
    "PrimColor",
    "EnvColor",
    "BlendColor",
    "Light1Color",
    "Light2Color",
];

/// How far the runtime engine leads the decomp: its tick `n` equals the
/// decomp's frame `n + PHASE` (RE-322).
const PHASE: u32 = 2;

#[derive(Clone, Copy, Default)]
struct RefTrack {
    live: bool,
    base: u32,
    target: u32,
    length: f32,
    invert: f32,
}

/// One `MObj`'s colour window, frame by frame, as the decomp computes it.
struct Reference<'a> {
    data: &'a [u8],
    pc: usize,
    /// `None` is `AOBJ_ANIM_CHANGED`, the state before the first parse.
    anim_wait: Option<f32>,
    ended: bool,
    tracks: [RefTrack; 5],
}

impl<'a> Reference<'a> {
    fn new(data: &'a [u8], script: u32) -> Self {
        Reference {
            data,
            pc: script as usize,
            anim_wait: None,
            ended: false,
            tracks: [RefTrack::default(); 5],
        }
    }

    fn word(&self, at: usize) -> Result<u32, String> {
        self.data
            .get(at..at + 4)
            .map(|b| u32::from_be_bytes(b.try_into().unwrap()))
            .ok_or_else(|| format!("script ran off its file at 0x{at:X}"))
    }

    /// One frame: `gcParseMObjMatAnimJoint` then `gcPlayMObjMatAnim`, with
    /// `anim_speed` 1. Returns each track's colour as `gcPlayMObjMatAnim`
    /// writes it into `mobj->sub`.
    fn frame(&mut self) -> Result<[Option<[u8; 4]>; 5], String> {
        const SPEED: f32 = 1.0;
        let mut wait = match self.anim_wait {
            None => 0.0,
            Some(w) => w - SPEED,
        };
        if !self.ended && wait <= 0.0 {
            for _ in 0..4096 {
                let word = self.word(self.pc)?;
                let (opcode, flags, payload) =
                    (word >> 25, (word >> 15) & 0x3FF, (word & 0x7FFF) as f32);
                self.pc += 4;
                match opcode {
                    0 => {
                        self.ended = true;
                        break;
                    }
                    // Jump / SetAnim: the next word is the target.
                    1 | 14 => self.pc = self.word(self.pc)? as usize,
                    // Wait.
                    2 => wait += payload,
                    // SetExtValBlock / SetExtVal: linear colour keys.
                    20 | 21 => {
                        for (i, t) in self.tracks.iter_mut().enumerate() {
                            if flags & (1 << i) == 0 {
                                continue;
                            }
                            let value = u32::from_be_bytes(
                                self.data
                                    .get(self.pc..self.pc + 4)
                                    .ok_or("truncated value")?
                                    .try_into()
                                    .unwrap(),
                            );
                            self.pc += 4;
                            t.live = true;
                            t.base = t.target;
                            t.target = value;
                            t.length = -wait - SPEED;
                            if payload != 0.0 {
                                t.invert = 1.0 / payload;
                            }
                        }
                        if opcode == 20 {
                            wait += payload;
                        }
                    }
                    other => {
                        return Err(format!("opcode {other} is outside the reference's model"))
                    }
                }
                if wait > 0.0 {
                    break;
                }
            }
        }
        self.anim_wait = Some(wait);
        let mut out = [None; 5];
        for (i, t) in self.tracks.iter_mut().enumerate() {
            if !t.live {
                continue;
            }
            if !self.ended {
                t.length += SPEED;
            }
            // `objanim.c:1352-1386`: the packed two-lane multiply is a
            // per-byte `((256 - interp) * base + interp * target) >> 8`.
            let interp = ((t.length * t.invert * 256.0) as i32).clamp(0, 256) as u32;
            let (b, g) = (t.base.to_be_bytes(), t.target.to_be_bytes());
            out[i] = Some(core::array::from_fn(|k| {
                (((256 - interp) * b[k] as u32 + interp * g[k] as u32) >> 8) as u8
            }));
        }
        Ok(out)
    }
}

/// Frames a colour script's key segments start at, from the reference's own
/// replay: where any track's target changes.
fn key_frames(reference: &mut Reference<'_>, frames: u32) -> Result<Vec<u32>, String> {
    let mut starts = Vec::new();
    let mut last = [0u32; 5];
    for f in 1..=frames {
        reference.frame()?;
        let targets = reference.tracks.map(|t| t.target);
        if f == 1 || targets != last {
            starts.push(f);
        }
        last = targets;
    }
    Ok(starts)
}

fn hex(c: Option<[u8; 4]>) -> String {
    c.map_or("-".into(), |c| {
        format!("{:02X}{:02X}{:02X}{:02X}", c[0], c[1], c[2], c[3])
    })
}

pub fn matcolors(rom_path: &Path, opts: &[&str]) -> Res {
    let mut pack_path: Option<PathBuf> = None;
    let mut it = opts.iter();
    while let Some(o) = it.next() {
        match *o {
            "--pack" => pack_path = it.next().map(PathBuf::from),
            other => return Err(format!("unknown option {other}").into()),
        }
    }
    let pack_path = pack_path.ok_or("--pack <pack.pak> is required")?;
    let rom = fs::read(rom_path)?;
    let info = ssb_rom::rom::identify(&rom)?;
    let archive = Archive::open(&rom, info.region)?;
    let pack_bytes = fs::read(&pack_path)?;
    let pack = Pack::open(&pack_bytes).map_err(|e| format!("{e:?}"))?;

    // Stage render-layer graph files: the census scope (`romtool stages`).
    let stage_files: BTreeSet<u32> = super::load_all(&archive)
        .stages
        .iter()
        .flat_map(|s| s.layers.iter().map(|l| l.graph.0))
        .collect();

    let mut animator = ssb_rom::skeleton::MaterialAnimator::new();
    animator.start(&pack);
    const TICKS: u32 = 600;
    let mut runtime: Vec<Vec<ssb_rom::skeleton::EffectColors>> = vec![Vec::new(); animator.len()];
    for _ in 0..TICKS {
        animator.tick(&pack);
        for (i, frames) in runtime.iter_mut().enumerate() {
            frames.push(animator.resolved_colors(i as u32).unwrap_or_default());
        }
    }

    let (mut entries, mut mismatches) = (0usize, 0usize);
    let mut track_counts = [0usize; 5];
    for i in 0..pack.mat_anim_count() {
        let a = pack.mat_anim(i).ok_or("missing MatAnimDesc")?;
        if !stage_files.contains(&a.source_file) {
            continue;
        }
        let colours = runtime[i as usize].iter().any(|c| {
            [c.prim, c.env, c.blend, c.light1, c.light2]
                .iter()
                .any(Option::is_some)
        });
        if !colours {
            continue;
        }
        entries += 1;
        let data = pack.mat_anim_file(&a).ok_or("missing script bytes")?;
        let rom_file = archive.load(a.source_file)?;
        if rom_file.data.as_slice() != data {
            return Err(format!(
                "MatAnimDesc {i}: packed file {} differs from the ROM",
                a.source_file
            )
            .into());
        }

        let mut reference = Reference::new(data, a.script);
        let mut expected = Vec::new();
        for _ in 0..TICKS + PHASE {
            expected.push(reference.frame()?);
        }
        let live: Vec<usize> = (0..5)
            .filter(|&t| expected.iter().any(|e| e[t].is_some()))
            .collect();
        for &t in &live {
            track_counts[t] += 1;
        }
        let mut bad = 0usize;
        for n in 1..=TICKS {
            let got = runtime[i as usize][n as usize - 1];
            let got = [got.prim, got.env, got.blend, got.light1, got.light2];
            if got != expected[(n + PHASE) as usize - 1] {
                bad += 1;
            }
        }
        mismatches += bad;

        let prims: Vec<(u32, u32)> = (0..pack.prim_count())
            .filter_map(|p| pack.prim(p).map(|d| (p, d)))
            .filter(|(_, d)| d.mat_anim == i)
            .map(|(p, d)| (p, d.flags))
            .collect();
        println!(
            "MatAnimDesc {i}: file {} MObjSub 0x{:X} script 0x{:X}  tracks {}",
            a.source_file,
            a.source_offset,
            a.script,
            live.iter()
                .map(|&t| TRACK_NAMES[t])
                .collect::<Vec<_>>()
                .join(" ")
        );
        for (p, f) in &prims {
            let names: Vec<&str> = [
                (flags::PRIM_ANIM, "PRIM_ANIM"),
                (flags::LIGHT1_ANIM, "LIGHT1_ANIM"),
                (flags::LIGHT2_ANIM, "LIGHT2_ANIM"),
                (flags::LIT, "LIT"),
                (flags::TEXTURE_BLEND, "TEXTURE_BLEND"),
                (flags::TRANSLUCENT, "TRANSLUCENT"),
                (flags::ALPHA_BLEND, "ALPHA_BLEND"),
            ]
            .iter()
            .filter(|(bit, _)| f & bit != 0)
            .map(|&(_, n)| n)
            .collect();
            println!("  prim {p}: {}", names.join(" "));
        }
        let mut keys = Reference::new(data, a.script);
        let starts = key_frames(&mut keys, TICKS)?;
        let loop_at = starts.iter().skip(1).find(|&&f| {
            f > 1
                && (f..f + 20)
                    .all(|g| expected.get(g as usize - 1) == expected.get((g - f) as usize))
        });
        let mut samples: BTreeSet<u32> = BTreeSet::new();
        samples.insert(1);
        for w in starts.windows(2).take(8) {
            samples.insert(w[0]);
            samples.insert((w[0] + w[1]) / 2);
            samples.insert(w[1] - 1);
        }
        if let Some(&l) = loop_at {
            samples.insert(l - 1);
            samples.insert(l);
            samples.insert(l + 1);
        }
        println!(
            "  period {}  key starts {:?}",
            loop_at.map_or("-".into(), |l| (l - 1).to_string()),
            &starts[..starts.len().min(12)]
        );
        println!("  decomp frame  reference                         runtime tick  runtime");
        for f in samples.into_iter().filter(|&f| f > PHASE) {
            let e = expected[f as usize - 1];
            let n = f - PHASE;
            let g = runtime[i as usize][n as usize - 1];
            let g = [g.prim, g.env, g.blend, g.light1, g.light2];
            let fmt = |c: [Option<[u8; 4]>; 5]| {
                live.iter()
                    .map(|&t| hex(c[t]))
                    .collect::<Vec<_>>()
                    .join(" ")
            };
            println!(
                "  {f:12}  {:<32}  {n:12}  {}{}",
                fmt(e),
                fmt(g),
                if e == g { "" } else { "  MISMATCH" }
            );
        }
        println!(
            "  {} of {TICKS} ticks differ from the reference at phase {PHASE}",
            bad
        );
    }
    println!(
        "\n{entries} stage colour MatAnimDesc(s): {}; {mismatches} mismatching tick(s)",
        TRACK_NAMES
            .iter()
            .zip(track_counts)
            .filter(|(_, c)| *c > 0)
            .map(|(n, c)| format!("{n} {c}"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    if mismatches > 0 {
        return Err("runtime colour tracks disagree with the decomp reference".into());
    }
    Ok(())
}
