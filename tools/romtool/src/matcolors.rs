//! `romtool matcolors`: every material animation track against an
//! independent decomp-semantics reference (RE-322, RE-324).
//!
//! The reference below is written from `objanim.c`'s
//! `gcParseMObjMatAnimJoint`/`gcPlayMObjMatAnim` directly, including the
//! first-frame `AOBJ_ANIM_CHANGED` path and the `length = -anim_wait -
//! anim_speed` convention, and shares no code with
//! `ssb_rom::matanim::MaterialJoint`. It models all fifteen tracks: the ten
//! material tracks (texture ids, UV, scroll, `SetLFrac`, `PaletteID`) as the
//! `f32` `gcPlayMObjMatAnim` writes, and the five colour tracks as packed
//! RGBA. An opcode the decomp's switch has no case for would hang the
//! original (`default` does not advance the script), so the reference
//! declines it rather than guessing.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use ssb_rom::archive::Archive;
use ssb_rom::pack::{flags, Pack};

type Res = Result<(), Box<dyn std::error::Error>>;

const TRACK_NAMES: [&str; TRACKS] = [
    "TextureIDCurrent",
    "TraU",
    "TraV",
    "ScaU",
    "ScaV",
    "TextureIDNext",
    "ScrU",
    "ScrV",
    "SetLFrac",
    "PaletteID",
    "PrimColor",
    "EnvColor",
    "BlendColor",
    "Light1Color",
    "Light2Color",
];
/// Ten material tracks, then five colour tracks.
const TRACKS: usize = 15;
/// First colour track (`nGCAnimTrackMaterialSubStart`).
const EXT: usize = 10;

/// One frame's output: a material track's `f32` bits, or a colour track's
/// RGBA as a big-endian word. `None` while the track's kind is
/// `nGCAnimKindNone`, when `gcPlayMObjMatAnim` writes nothing.
type Frame = [Option<u32>; TRACKS];

#[derive(Clone, Copy, PartialEq)]
enum RefKind {
    None,
    Linear,
    Cubic,
    Step,
}

#[derive(Clone, Copy)]
struct RefTrack {
    kind: RefKind,
    /// Raw words: a colour track's are RGBA bytes, never arithmetic.
    base: u32,
    target: u32,
    rate_base: f32,
    rate_target: f32,
    length: f32,
    invert: f32,
}

impl Default for RefTrack {
    /// `gcAddAObjForMObj` (`objman.c:1230-1237`).
    fn default() -> Self {
        RefTrack {
            kind: RefKind::None,
            base: 0,
            target: 0,
            rate_base: 0.0,
            rate_target: 0.0,
            length: 0.0,
            invert: 1.0,
        }
    }
}

impl RefTrack {
    /// `gcPlayMObjMatAnim`'s material branch.
    fn material(&self) -> Option<u32> {
        let (base, target) = (f32::from_bits(self.base), f32::from_bits(self.target));
        let value = match self.kind {
            RefKind::None => return None,
            RefKind::Linear => base + self.length * self.rate_base,
            RefKind::Cubic => {
                let f16 = self.invert * self.invert;
                let f12 = self.length * self.length;
                let f18 = self.invert * f12;
                let f14 = self.length * f12 * f16;
                let f20 = 2.0 * f14 * self.invert;
                let f22 = 3.0 * f12 * f16;
                let f24 = f14 - f18;
                base * ((f20 - f22) + 1.0)
                    + target * (f22 - f20)
                    + self.rate_base * ((f24 - f18) + self.length)
                    + self.rate_target * f24
            }
            RefKind::Step => {
                if self.invert <= self.length {
                    target
                } else {
                    base
                }
            }
        };
        Some(value.to_bits())
    }

    /// `gcPlayMObjMatAnim`'s colour branch. The colour opcodes never make
    /// a cubic track, so only linear and step write.
    fn colour(&self) -> Option<u32> {
        match self.kind {
            RefKind::Linear => {
                // `objanim.c:1352-1386`: the packed two-lane multiply is a
                // per-byte `((256 - interp) * base + interp * target) >> 8`.
                let interp = ((self.length * self.invert * 256.0) as i32).clamp(0, 256) as u32;
                let (b, g) = (self.base.to_be_bytes(), self.target.to_be_bytes());
                Some(u32::from_be_bytes(core::array::from_fn(|k| {
                    (((256 - interp) * b[k] as u32 + interp * g[k] as u32) >> 8) as u8
                })))
            }
            RefKind::Step => Some(if self.invert <= self.length {
                self.target
            } else {
                self.base
            }),
            _ => None,
        }
    }
}

/// One `MObj`'s fifteen tracks, frame by frame, as the decomp computes them.
struct Reference<'a> {
    data: &'a [u8],
    pc: usize,
    /// `None` is `AOBJ_ANIM_CHANGED`, the state before the first parse.
    anim_wait: Option<f32>,
    ended: bool,
    tracks: [RefTrack; TRACKS],
    /// `ANIM_CMD_22` commands run: they write `MObjSub` fields no track
    /// covers, so a script using one is only partly checked.
    cmd22: usize,
    /// Commands run, by opcode.
    ops: [usize; 23],
}

impl<'a> Reference<'a> {
    fn new(data: &'a [u8], script: u32) -> Self {
        Reference {
            data,
            pc: script as usize,
            anim_wait: None,
            ended: false,
            tracks: [RefTrack::default(); TRACKS],
            cmd22: 0,
            ops: [0; 23],
        }
    }

    fn word(&self, at: usize) -> Result<u32, String> {
        self.data
            .get(at..at + 4)
            .map(|b| u32::from_be_bytes(b.try_into().unwrap()))
            .ok_or_else(|| format!("script ran off its file at 0x{at:X}"))
    }

    fn next(&mut self) -> Result<u32, String> {
        let w = self.word(self.pc)?;
        self.pc += 4;
        Ok(w)
    }

    /// One frame: `gcParseMObjMatAnimJoint` then `gcPlayMObjMatAnim`, with
    /// `anim_speed` 1 and `anim_frame` 0.
    fn frame(&mut self) -> Result<Frame, String> {
        const SPEED: f32 = 1.0;
        let mut wait = match self.anim_wait {
            None => 0.0,
            Some(w) => w - SPEED,
        };
        if !self.ended && wait <= 0.0 {
            let mut steps = 0;
            loop {
                steps += 1;
                if steps > 4096 {
                    return Err("script did not block within 4096 commands".into());
                }
                let at = self.pc;
                let word = self.next()?;
                let (opcode, flags, payload) =
                    (word >> 25, (word >> 15) & 0x3FF, (word & 0x7FFF) as f32);
                if let Some(n) = self.ops.get_mut(opcode as usize) {
                    *n += 1;
                }
                // Tracks a keyed command names, in flag-bit order.
                let named = |window: usize, count: usize| {
                    (0..count)
                        .filter(move |i| flags & (1 << i) != 0)
                        .map(move |i| window + i)
                };
                match opcode {
                    0 => {
                        for t in self.tracks.iter_mut().filter(|t| t.kind != RefKind::None) {
                            t.length += SPEED + wait;
                        }
                        self.ended = true;
                        break;
                    }
                    // Jump / SetAnim: the next word is the target.
                    1 | 14 => self.pc = self.word(self.pc)? as usize,
                    2 => wait += payload,
                    // SetValBlock / SetVal.
                    3 | 4 => {
                        for i in named(0, EXT) {
                            let v = self.next()?;
                            let t = &mut self.tracks[i];
                            t.base = t.target;
                            t.target = v;
                            t.kind = RefKind::Linear;
                            if payload != 0.0 {
                                t.rate_base =
                                    (f32::from_bits(t.target) - f32::from_bits(t.base)) / payload;
                            }
                            t.length = -wait - SPEED;
                            t.rate_target = 0.0;
                        }
                        if opcode == 3 {
                            wait += payload;
                        }
                    }
                    // SetValRateBlock / SetValRate.
                    5 | 6 => {
                        for i in named(0, EXT) {
                            let v = self.next()?;
                            let r = f32::from_bits(self.next()?);
                            let t = &mut self.tracks[i];
                            t.base = t.target;
                            t.target = v;
                            t.rate_base = t.rate_target;
                            t.rate_target = r;
                            t.kind = RefKind::Cubic;
                            if payload != 0.0 {
                                t.invert = 1.0 / payload;
                            }
                            t.length = -wait - SPEED;
                        }
                        if opcode == 5 {
                            wait += payload;
                        }
                    }
                    // SetTargetRate: the rate only.
                    7 => {
                        for i in named(0, EXT) {
                            self.tracks[i].rate_target = f32::from_bits(self.next()?);
                        }
                    }
                    // SetVal0RateBlock / SetVal0Rate.
                    8 | 9 => {
                        for i in named(0, EXT) {
                            let v = self.next()?;
                            let t = &mut self.tracks[i];
                            t.base = t.target;
                            t.target = v;
                            t.rate_base = t.rate_target;
                            t.rate_target = 0.0;
                            t.kind = RefKind::Cubic;
                            if payload != 0.0 {
                                t.invert = 1.0 / payload;
                            }
                            t.length = -wait - SPEED;
                        }
                        if opcode == 8 {
                            wait += payload;
                        }
                    }
                    // SetValAfterBlock / SetValAfter.
                    10 | 11 => {
                        for i in named(0, EXT) {
                            let v = self.next()?;
                            let t = &mut self.tracks[i];
                            t.base = t.target;
                            t.target = v;
                            t.kind = RefKind::Step;
                            t.invert = payload;
                            t.length = -wait - SPEED;
                            t.rate_target = 0.0;
                        }
                        if opcode == 10 {
                            wait += payload;
                        }
                    }
                    // `ANIM_CMD_12`: lengthen the named material tracks.
                    12 => {
                        for i in named(0, EXT) {
                            self.tracks[i].length += payload;
                        }
                    }
                    // SetExtValAfterBlock / SetExtValAfter.
                    18 | 19 => {
                        for i in named(EXT, TRACKS - EXT) {
                            let v = self.next()?;
                            let t = &mut self.tracks[i];
                            t.base = t.target;
                            t.target = v;
                            t.kind = RefKind::Step;
                            t.invert = payload;
                            t.length = -wait - SPEED;
                        }
                        if opcode == 18 {
                            wait += payload;
                        }
                    }
                    // SetExtValBlock / SetExtVal.
                    20 | 21 => {
                        for i in named(EXT, TRACKS - EXT) {
                            let v = self.next()?;
                            let t = &mut self.tracks[i];
                            t.base = t.target;
                            t.target = v;
                            t.kind = RefKind::Linear;
                            if payload != 0.0 {
                                t.invert = 1.0 / payload;
                            }
                            t.length = -wait - SPEED;
                        }
                        if opcode == 20 {
                            wait += payload;
                        }
                    }
                    // `ANIM_CMD_22`: sets the wait outright and writes up to
                    // five `MObjSub` words outside the track model.
                    22 => {
                        wait = payload;
                        for _ in 0..(flags & 0x1F).count_ones() {
                            self.next()?;
                        }
                        self.cmd22 += 1;
                    }
                    other => {
                        return Err(format!(
                            "opcode {other} at 0x{at:X} has no case in gcParseMObjMatAnimJoint"
                        ))
                    }
                }
                if wait > 0.0 {
                    break;
                }
            }
        }
        self.anim_wait = Some(wait);
        let mut out = [None; TRACKS];
        for (i, t) in self.tracks.iter_mut().enumerate() {
            if t.kind == RefKind::None {
                continue;
            }
            if !self.ended {
                t.length += SPEED;
            }
            out[i] = if i < EXT { t.material() } else { t.colour() };
        }
        Ok(out)
    }
}

/// Frames a script's key segments start at, from the reference's own
/// replay: where any track's target changes.
fn key_frames(reference: &mut Reference<'_>, frames: u32) -> Result<Vec<u32>, String> {
    let mut starts = Vec::new();
    let mut last = [0u32; TRACKS];
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

fn show(track: usize, v: Option<u32>) -> String {
    match v {
        None => "-".into(),
        Some(w) if track >= EXT => format!("{w:08X}"),
        Some(w) => format!("{}", f32::from_bits(w)),
    }
}

/// The runtime joint's fifteen tracks, in the reference's [`Frame`] form.
fn runtime_frame(j: &ssb_rom::matanim::MaterialJoint) -> Frame {
    core::array::from_fn(|i| {
        if i < EXT {
            j.track_value(i).map(f32::to_bits)
        } else {
            j.track_color(i).map(u32::from_be_bytes)
        }
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

    // The stage clock. Effect joints start and tick the same way from
    // their spawn, so one replay from frame 0 covers both.
    let mut animator = ssb_rom::skeleton::MaterialAnimator::new();
    animator.start(&pack);
    const TICKS: u32 = 600;
    let mut runtime: Vec<Vec<Frame>> = vec![Vec::new(); animator.len()];
    for _ in 0..TICKS {
        animator.tick(&pack);
        for (i, frames) in runtime.iter_mut().enumerate() {
            let j = animator.joint(i as u32).ok_or("animator lost a joint")?;
            frames.push(runtime_frame(j));
        }
    }

    let (mut entries, mut stage_entries, mut mismatches) = (0usize, 0usize, 0usize);
    let mut track_counts = [0usize; TRACKS];
    let mut track_bad = [0usize; TRACKS];
    let mut declined = Vec::new();
    let mut partial = 0usize;
    let mut ops = [0usize; 23];
    for i in 0..pack.mat_anim_count() {
        let a = pack.mat_anim(i).ok_or("missing MatAnimDesc")?;
        let data = pack.mat_anim_file(&a).ok_or("missing script bytes")?;
        let rom_file = archive.load(a.source_file)?;
        if rom_file.data.as_slice() != data {
            return Err(format!(
                "MatAnimDesc {i}: packed file {} differs from the ROM",
                a.source_file
            )
            .into());
        }
        let stage = stage_files.contains(&a.source_file);

        let mut reference = Reference::new(data, a.script);
        let mut expected = Vec::new();
        let replay = (0..TICKS).try_for_each(|_| {
            expected.push(reference.frame()?);
            Ok::<_, String>(())
        });
        if let Err(e) = replay {
            println!(
                "MatAnimDesc {i}: file {} script 0x{:X}  DECLINED: {e}",
                a.source_file, a.script
            );
            declined.push(i);
            continue;
        }
        if reference.cmd22 > 0 {
            partial += 1;
        }
        for (total, n) in ops.iter_mut().zip(reference.ops) {
            *total += n;
        }
        entries += 1;
        stage_entries += stage as usize;
        let live: Vec<usize> = (0..TRACKS)
            .filter(|&t| expected.iter().any(|e| e[t].is_some()))
            .collect();
        for &t in &live {
            track_counts[t] += 1;
        }
        let got = &runtime[i as usize];
        let mut bad = 0usize;
        let mut first_bad: Vec<(usize, u32)> = Vec::new();
        for n in 0..TICKS as usize {
            if got[n] != expected[n] {
                bad += 1;
            }
            for t in 0..TRACKS {
                if got[n][t] != expected[n][t] {
                    track_bad[t] += 1;
                    if !first_bad.iter().any(|&(u, _)| u == t) {
                        first_bad.push((t, n as u32 + 1));
                    }
                }
            }
        }
        mismatches += bad;

        println!(
            "MatAnimDesc {i}: file {} MObjSub 0x{:X} script 0x{:X} {}  tracks {}{}",
            a.source_file,
            a.source_offset,
            a.script,
            if stage { "stage" } else { "other" },
            live.iter()
                .map(|&t| TRACK_NAMES[t])
                .collect::<Vec<_>>()
                .join(" "),
            if reference.cmd22 > 0 {
                "  (+ANIM_CMD_22)"
            } else {
                ""
            }
        );
        for &(t, f) in &first_bad {
            println!(
                "  {} first differs at frame {f}: reference {} runtime {}",
                TRACK_NAMES[t],
                show(t, expected[f as usize - 1][t]),
                show(t, got[f as usize - 1][t])
            );
        }
        if bad > 0 {
            println!("  {bad} of {TICKS} ticks differ from the reference");
        }
        // Stage colour entries keep RE-322's sampled table.
        if !stage || !live.iter().any(|&t| t >= EXT) {
            continue;
        }
        let prims: Vec<(u32, u32)> = (0..pack.prim_count())
            .filter_map(|p| pack.prim(p).map(|d| (p, d)))
            .filter(|(_, d)| d.mat_anim == i)
            .map(|(p, d)| (p, d.flags))
            .collect();
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
        println!("  frame  reference                         runtime");
        for f in samples.into_iter().filter(|&f| f <= TICKS) {
            let e = expected[f as usize - 1];
            let g = got[f as usize - 1];
            let fmt = |c: Frame| {
                live.iter()
                    .filter(|&&t| t >= EXT)
                    .map(|&t| show(t, c[t]))
                    .collect::<Vec<_>>()
                    .join(" ")
            };
            println!(
                "  {f:5}  {:<32}  {}{}",
                fmt(e),
                fmt(g),
                if e == g { "" } else { "  MISMATCH" }
            );
        }
    }
    println!(
        "\n{entries} MatAnimDesc(s) replayed ({stage_entries} stage), {} declined, \
         {partial} with ANIM_CMD_22",
        declined.len()
    );
    for t in 0..TRACKS {
        if track_counts[t] > 0 || track_bad[t] > 0 {
            println!(
                "  {:<16} {:4} entries  {:6} mismatching track-ticks",
                TRACK_NAMES[t], track_counts[t], track_bad[t]
            );
        }
    }
    println!(
        "  commands run, by opcode: {}",
        ops.iter()
            .enumerate()
            .filter(|(_, &n)| n > 0)
            .map(|(op, n)| format!("{op}:{n}"))
            .collect::<Vec<_>>()
            .join(" ")
    );
    println!("{mismatches} mismatching tick(s) over {TICKS} per entry");
    if !declined.is_empty() {
        return Err(format!("the reference declined MatAnimDesc(s) {declined:?}").into());
    }
    if mismatches > 0 {
        return Err("runtime material tracks disagree with the decomp reference".into());
    }
    Ok(())
}
