//! `romtool matcolors`: every material animation track, and the resolvers
//! built on them, against an independent decomp-semantics reference
//! (RE-322, RE-324, RE-325, RE-326).
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
//!
//! [`check_resolvers`] then feeds `gcDrawMObjForDObj`'s texture, palette,
//! tile-window and `PRIM_LOD_FRAC` logic from those tracks and the ROM's
//! own `MObjSub`, and compares `MaterialAnimator`'s resolvers with it.
//!
//! [`crate::matsample`] then checks what the primitives sample: packed
//! texels against the ROM images, and every material-animated vertex's GE
//! texel against the RDP's (RE-326).

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use ssb_rom::archive::Archive;
use ssb_rom::pack::{flags, Pack};
use ssb_rom::skeleton::{LodBlendState, MaterialUv};

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

/// What `MaterialAnimator`'s resolvers returned for one entry on one frame.
#[derive(Clone, Copy)]
struct Resolved {
    texture: Option<u32>,
    palette: Option<u32>,
    uv: Option<MaterialUv>,
    lod: Option<LodBlendState>,
}

const MOBJ_FLAG_ALPHA: u16 = 1 << 0;
const MOBJ_FLAG_SPLIT: u16 = 1 << 1;
const MOBJ_FLAG_PALETTE: u16 = 1 << 2;
const MOBJ_FLAG_FRAC: u16 = 1 << 4;
const MOBJ_FLAG_TILE0: u16 = 0x20;
const MOBJ_FLAG_TILE1: u16 = 0x40;
const MOBJ_FLAG_TEXTURE: u16 = 1 << 7;

/// The `MObjSub` fields `gcDrawMObjForDObj` reads for textures, palettes
/// and tile windows (`objtypes.h`'s layout), read straight from the ROM.
struct RomSub {
    flags: u16,
    unk08: u16,
    unk0a: u16,
    unk0c: u16,
    unk0e: u16,
    unk10: i32,
    /// `unk24`, `unk28`, `unk44`: the `unk10 == 1` inputs (RE-327).
    half: [f32; 3],
    /// `trau`, `trav`, `scau`, `scav`.
    uv: [f32; 4],
    unk38: u16,
    unk3a: u16,
    /// `scrollu`, `scrollv`.
    scroll: [f32; 2],
    prim_l: u8,
}

impl RomSub {
    fn read(data: &[u8], at: u32) -> Result<Self, String> {
        let at = at as usize;
        let bytes = |off: usize, n: usize| {
            data.get(at + off..at + off + n)
                .ok_or_else(|| format!("MObjSub at 0x{at:X} runs off its file"))
        };
        let u16_ = |off| bytes(off, 2).map(|b| u16::from_be_bytes([b[0], b[1]]));
        let u32_ = |off| bytes(off, 4).map(|b| u32::from_be_bytes(b.try_into().unwrap()));
        let f32_ = |off| u32_(off).map(f32::from_bits);
        let flags = u16_(0x30)?;
        Ok(RomSub {
            // `MOBJ_FLAG_NONE` draws as `TEXTURE | 0x20 | ALPHA`.
            flags: if flags == 0 {
                MOBJ_FLAG_TEXTURE | MOBJ_FLAG_TILE0 | MOBJ_FLAG_ALPHA
            } else {
                flags
            },
            unk08: u16_(0x08)?,
            unk0a: u16_(0x0A)?,
            unk0c: u16_(0x0C)?,
            unk0e: u16_(0x0E)?,
            unk10: u32_(0x10)? as i32,
            half: [f32_(0x24)?, f32_(0x28)?, f32_(0x44)?],
            uv: [f32_(0x14)?, f32_(0x18)?, f32_(0x1C)?, f32_(0x20)?],
            unk38: u16_(0x38)?,
            unk3a: u16_(0x3A)?,
            scroll: [f32_(0x3C)?, f32_(0x40)?],
            prim_l: bytes(0x54, 1)?[0],
        })
    }
}

/// Resolver mismatches, by resolver.
#[derive(Default)]
struct ResolverTally {
    /// Frames the decomp or the runtime drew something through each
    /// resolver: texture, palette, tile 0, two-tile blend.
    checked: [usize; 4],
    bad: [usize; 4],
    /// Static checks of the packed tables against the ROM's arrays.
    table_bad: usize,
    /// Entries whose `MObjSub` loads a second tile the packer did not lower.
    lod_not_lowered: usize,
    /// Packed textures compared texel for texel with their `sprites[]`
    /// image, and those that differ (RE-326).
    textures_checked: usize,
    /// Of those, read through a mirrored or repeated period (RE-327).
    textures_folded: usize,
    textures_bad: usize,
    /// Static CLUTs compared with `palettes[0]`, and those that differ.
    static_palettes_checked: usize,
    static_palettes_bad: usize,
    /// GE against RDP texel coordinates (RE-326).
    uv: crate::matsample::UvTally,
    /// Primitives naming an entry whose texture, palette or window track
    /// moves, by whether their `MObj` still owns that state (RE-326):
    /// `[owned, not owned]` for image, TLUT and tile 0.
    ownership: [[usize; 2]; 3],
    /// Source files of the entries with primitives in `ownership[_][1]`.
    replaced_files: BTreeSet<u32>,
}

const RESOLVERS: [&str; 4] = ["texture", "palette", "tile0 UV", "two-tile blend"];

/// Compares a resolved window with the fields `gcDrawMObjForDObj` reads:
/// live and rest `Tra*`/`Sca*`, form and tile parameters. `None` when equal.
fn uv_diff(
    got: &MaterialUv,
    live: [f32; 4],
    rest: [f32; 4],
    mode: u32,
    params: [u16; 3],
    half: [f32; 2],
) -> Option<String> {
    let want = [
        live[0], live[1], live[2], live[3], rest[0], rest[1], rest[2], rest[3],
    ];
    let have = [
        got.trau,
        got.trav,
        got.scau,
        got.scav,
        got.base_trau,
        got.base_trav,
        got.base_scau,
        got.base_scav,
    ];
    let params_have = [got.tile_bias, got.tile_width, got.tile_height];
    let params_want = params.map(f32::from);
    if want.map(f32::to_bits) == have.map(f32::to_bits)
        && got.mode == mode
        && params_have.map(f32::to_bits) == params_want.map(f32::to_bits)
        && (mode & 8 == 0 || got.half.map(f32::to_bits) == half.map(f32::to_bits))
    {
        return None;
    }
    Some(format!(
        "want {want:?} mode {mode} params {params_want:?}, got {have:?} mode {} params {params_have:?}",
        got.mode
    ))
}

/// Checks one entry's resolved texture, palette, tile-0 window and
/// two-tile blend against `gcDrawMObjForDObj` fed by the reference's
/// tracks and the ROM's `MObjSub`. Returns the first difference per
/// resolver.
fn check_resolvers(
    archive: &Archive,
    pack: &Pack<'_>,
    i: u32,
    expected: &[Frame],
    got: &[Resolved],
    tally: &mut ResolverTally,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let a = pack.mat_anim(i).ok_or("missing MatAnimDesc")?;
    let rom_file = archive.load(a.source_file)?;
    let sub = RomSub::read(&rom_file.data, a.source_offset)?;
    let lod = pack.lod_blend(i);
    let f = sub.flags;
    let mut notes: Vec<String> = Vec::new();
    let mut note = |notes: &mut Vec<String>, kind: usize, msg: String| {
        tally.bad[kind] += 1;
        if !notes.iter().any(|n| n.starts_with(RESOLVERS[kind])) {
            notes.push(format!("{}: {msg}", RESOLVERS[kind]));
        }
    };

    // Static: the packed tables follow the ROM's arrays. Equal sprite
    // pointers must share a packed texture and distinct ones must not;
    // palettes must hold the ROM's RGBA5551 entries.
    let same_shape = |ptrs: &[ssb_rom::mobj::Ptr], packed: &[u32]| {
        (0..ptrs.len())
            .all(|x| (0..ptrs.len()).all(|y| (ptrs[x] == ptrs[y]) == (packed[x] == packed[y])))
    };
    let mut table_notes = Vec::new();
    if a.texture_count > 0 {
        let n = a.texture_count as usize;
        match ssb_rom::mobj::read_sprites(&rom_file, a.source_offset, n) {
            Some(ptrs) if same_shape(&ptrs, &a.textures[..n]) => {}
            Some(_) => table_notes.push("textures[] do not follow sprites[]".to_string()),
            None => table_notes.push("sprites[] unreadable".to_string()),
        }
    }
    if let Some(l) = lod {
        let n = l.next_count as usize;
        match ssb_rom::mobj::read_sprites(&rom_file, a.source_offset, n) {
            Some(ptrs) if same_shape(&ptrs, &l.next_textures[..n]) => {}
            Some(_) => table_notes.push("next_textures[] do not follow sprites[]".to_string()),
            None => table_notes.push("sprites[] unreadable".to_string()),
        }
    }
    if a.palette_count > 0 {
        let ptrs =
            ssb_rom::mobj::read_palettes(&rom_file, a.source_offset, a.palette_count as usize)
                .ok_or("palettes[] unreadable")?;
        for (k, p) in ptrs.iter().enumerate() {
            let packed = pack
                .mat_anim_palette(a.first_palette + k as u32)
                .ok_or("missing packed palette")?;
            let words = pack
                .mat_anim_palette_data(&packed)
                .ok_or("missing palette bytes")?;
            let home = match p.file {
                Some(id) => archive.load(u32::from(id))?,
                None => rom_file.clone(),
            };
            let n = packed.palette_len as usize;
            let rom = home
                .data
                .get(p.offset as usize..p.offset as usize + 2 * n)
                .ok_or("palette runs off its file")?;
            let matches = (0..n).all(|e| {
                let v = u16::from_be_bytes([rom[2 * e], rom[2 * e + 1]]);
                let w = u32::from_le_bytes(words[4 * e..4 * e + 4].try_into().unwrap());
                let top = |shift: u32| (w >> shift) as u16 & 0xF8;
                top(0) >> 3 == (v >> 11) & 0x1F
                    && top(8) >> 3 == (v >> 6) & 0x1F
                    && top(16) >> 3 == (v >> 1) & 0x1F
                    && (w >> 24 != 0) == (v & 1 != 0)
            });
            if !matches {
                table_notes.push(format!("palette {k} differs from palettes[{k}]"));
            }
        }
    }
    tally.table_bad += table_notes.len();
    notes.extend(table_notes.into_iter().map(|n| format!("table: {n}")));

    // Texels (RE-326): every packed texture the entry can bind against the
    // image it stands for. A primitive's own texture is `sprites[0]` when
    // the `MObj` sets the image (`texture_id_curr` starts at 0), and its own
    // CLUT is `palettes[0]` under `PALETTE` (`palette_id` starts at 0).
    // Only primitives whose `MObj` still owns the image or TLUT (RE-326).
    let owned = |bit: u32| -> BTreeSet<u32> {
        (0..pack.prim_count())
            .filter_map(|p| pack.prim(p))
            .filter(|p| {
                p.mat_anim == i
                    && p.flags & bit != 0
                    && p.texture != ssb_rom::pack::TextureDesc::NO_ANIM
            })
            .map(|p| p.texture)
            .collect()
    };
    // A v34 runtime applied each moving track to every primitive naming the
    // entry; count the ones that do not own the state it moves.
    for (slot, (bit, tracks)) in [
        (flags::IMAGE_ANIM, &[0usize][..]),
        (flags::PALETTE_ANIM, &[9][..]),
        (flags::TILE0_ANIM, &[1, 2, 3, 4][..]),
    ]
    .into_iter()
    .enumerate()
    {
        if !expected
            .iter()
            .any(|e| tracks.iter().any(|&t| e[t].is_some()))
        {
            continue;
        }
        for p in (0..pack.prim_count()).filter_map(|p| pack.prim(p)) {
            if p.mat_anim == i {
                tally.ownership[slot][(p.flags & bit == 0) as usize] += 1;
                if p.flags & bit == 0 {
                    tally.replaced_files.insert(a.source_file);
                }
            }
        }
    }
    let statics = owned(flags::IMAGE_ANIM);
    let palette_statics = owned(flags::PALETTE_ANIM);
    let mut bound: Vec<(String, u32, usize)> = Vec::new();
    if f & (MOBJ_FLAG_FRAC | MOBJ_FLAG_ALPHA) != 0 {
        bound.extend(statics.iter().map(|&t| ("static".to_string(), t, 0)));
    }
    for (k, &t) in a.textures[..a.texture_count as usize].iter().enumerate() {
        bound.push((format!("textures[{k}]"), t, k));
    }
    if let Some(l) = lod {
        for (k, &t) in l.next_textures[..l.next_count as usize].iter().enumerate() {
            bound.push((format!("next_textures[{k}]"), t, k));
        }
    }
    let sprite_count = bound.iter().map(|b| b.2 + 1).max().unwrap_or(0);
    if sprite_count > 0 {
        let sprites = ssb_rom::mobj::read_sprites(&rom_file, a.source_offset, sprite_count)
            .ok_or("sprites[] unreadable")?;
        for (name, t, k) in &bound {
            let desc = pack.texture(*t).ok_or("missing packed texture")?;
            tally.textures_checked += 1;
            if desc.tile_mirror != 0 || [desc.width, desc.height] != desc.tile_period {
                tally.textures_folded += 1;
            }
            let r = crate::matsample::compare_texture(archive, &rom_file, pack, &desc, sprites[*k]);
            match r {
                Ok(r) if r.bad == 0 => {}
                Ok(r) => {
                    tally.textures_bad += 1;
                    let (x, y, rom, got) = r.first.unwrap();
                    notes.push(format!(
                        "texels: {name} (texture {t}, {}x{} psm {}, tile {:?} period {:?} mirror {}) \
                         differs from sprites[{k}] in {} of {} texels; first ({x},{y}) rom {rom} packed {got}",
                        desc.width,
                        desc.height,
                        desc.psm,
                        desc.tile_format(),
                        desc.tile_period,
                        desc.tile_mirror,
                        r.bad,
                        r.texels
                    ));
                }
                Err(e) => {
                    tally.textures_bad += 1;
                    notes.push(format!("texels: {name} (texture {t}): {e}"));
                }
            }
        }
    }
    if f & MOBJ_FLAG_PALETTE != 0 {
        let first = ssb_rom::mobj::read_palettes(&rom_file, a.source_offset, 1)
            .and_then(|p| p.first().copied())
            .ok_or("palettes[0] unreadable")?;
        for &t in &palette_statics {
            let desc = pack.texture(t).ok_or("missing packed texture")?;
            let Some(words) = pack.palette_data(&desc) else {
                continue;
            };
            tally.static_palettes_checked += 1;
            if !crate::matsample::palette_matches(archive, &rom_file, words, first)? {
                tally.static_palettes_bad += 1;
                notes.push(format!(
                    "texels: texture {t}'s packed CLUT differs from palettes[0]"
                ));
            }
        }
    }

    let loads_next =
        f & (MOBJ_FLAG_FRAC | MOBJ_FLAG_SPLIT) != 0 && f & (MOBJ_FLAG_FRAC | MOBJ_FLAG_ALPHA) != 0;
    if loads_next && lod.is_none() && expected.iter().any(|e| e[5].is_some() || e[8].is_some()) {
        tally.lod_not_lowered += 1;
    }
    let rest_uv = sub.uv;
    let tile0_mode = if f & MOBJ_FLAG_TILE0 == 0 {
        0
    } else if sub.unk10 == 2 {
        2
    } else {
        1
    } | if f & MOBJ_FLAG_TEXTURE != 0 { 4 } else { 0 };
    // RE-327: `unk10 == 1` halves the U window and scale of both tiles.
    let half_bit = if sub.unk10 == 1 { 8 } else { 0 };
    let tile0_mode = if tile0_mode == 0 {
        0
    } else {
        tile0_mode | half_bit
    };

    let window = crate::matsample::SubWindow {
        flags: f,
        unk08: sub.unk08,
        unk0a: sub.unk0a,
        unk0c: sub.unk0c,
        unk0e: sub.unk0e,
        unk10: sub.unk10,
        half: sub.half,
        rest: sub.uv,
    };
    let live_uv: Vec<[f32; 4]> = expected
        .iter()
        .map(|e| core::array::from_fn(|k| e[1 + k].map_or(sub.uv[k], f32::from_bits)))
        .collect();
    let got_uv: Vec<Option<MaterialUv>> = got.iter().map(|g| g.uv).collect();
    crate::matsample::check_uv(
        pack,
        i,
        &window,
        &live_uv,
        &got_uv,
        &mut tally.uv,
        &mut notes,
    );
    if lod.is_some() {
        let scroll: Vec<[f32; 2]> = expected
            .iter()
            .map(|e| core::array::from_fn(|k| e[6 + k].map_or(sub.scroll[k], f32::from_bits)))
            .collect();
        let got_lod: Vec<Option<LodBlendState>> = got.iter().map(|g| g.lod).collect();
        let tile1 = crate::matsample::Tile1 {
            unk38: sub.unk38,
            unk3a: sub.unk3a,
        };
        crate::matsample::check_uv_tile1(
            pack,
            i,
            &window,
            &tile1,
            &live_uv,
            &scroll,
            &got_lod,
            &mut tally.uv,
            &mut notes,
        );
    }

    for (n, (e, g)) in expected.iter().zip(got).enumerate() {
        let frame = n + 1;
        let live = |t: usize| e[t].map(f32::from_bits);
        // `MObj` state after `gcPlayMObjMatAnim`, from `gcAddMObjForDObj`'s
        // initial values (`objman.c:1321-1328`) where no track has written.
        let lfrac = live(8).unwrap_or(sub.prim_l as f32 / 255.0);
        let (curr, next) = if f & MOBJ_FLAG_FRAC != 0 {
            let trunc = lfrac as i32;
            (Some(trunc as u16), (trunc + 1) as u16)
        } else {
            (
                live(0).map(|v| v as i32 as u16),
                live(5).map_or(0, |v| v as i32 as u16),
            )
        };
        let uv: [f32; 4] = core::array::from_fn(|k| live(1 + k).unwrap_or(rest_uv[k]));
        let scroll: [f32; 2] = core::array::from_fn(|k| live(6 + k).unwrap_or(sub.scroll[k]));

        // Texture: `sprites[texture_id_curr]` under `FRAC | ALPHA`. Only a
        // moving index needs the resolver; index 0 is the packed texture.
        let want_tex = curr.filter(|_| f & (MOBJ_FLAG_FRAC | MOBJ_FLAG_ALPHA) != 0);
        if want_tex.is_some() || g.texture.is_some() {
            tally.checked[0] += 1;
            let want = want_tex.map(|k| {
                a.textures
                    .get(k as usize)
                    .copied()
                    .filter(|_| u32::from(k) < a.texture_count)
            });
            let ok = match (want, g.texture) {
                (Some(Some(t)), Some(h)) => t == h,
                // A NO_ANIM slot resolves to the packed texture.
                (Some(Some(t)), None) => t == ssb_rom::pack::TextureDesc::NO_ANIM,
                _ => false,
            };
            if !ok {
                note(
                    &mut notes,
                    0,
                    format!(
                        "frame {frame}: index {want_tex:?} (table {:?}), runtime {:?}",
                        &a.textures[..a.texture_count as usize],
                        g.texture
                    ),
                );
            }
        }

        // Palette: `palettes[(s32)palette_id]` under `PALETTE`.
        let want_pal = live(9)
            .map(|v| v as i32)
            .filter(|_| f & MOBJ_FLAG_PALETTE != 0);
        if want_pal.is_some() || g.palette.is_some() {
            tally.checked[1] += 1;
            let ok = match (want_pal, g.palette) {
                (Some(k), Some(p)) => {
                    k >= 0 && (k as u32) < a.palette_count && p == a.first_palette + k as u32
                }
                // No packed table: one palette, the packed texture's.
                (Some(0), None) => a.palette_count == 0,
                _ => false,
            };
            if !ok {
                note(
                    &mut notes,
                    1,
                    format!(
                        "frame {frame}: index {want_pal:?} of {}, runtime {:?} (first {})",
                        a.palette_count, g.palette, a.first_palette
                    ),
                );
            }
        }

        // Tile 0 and `gSPTexture`.
        if f & (MOBJ_FLAG_TILE0 | MOBJ_FLAG_TEXTURE) != 0 {
            let moved = uv.map(f32::to_bits) != rest_uv.map(f32::to_bits);
            if moved || g.uv.is_some() {
                tally.checked[2] += 1;
                let diff = match &g.uv {
                    None => Some("runtime draws the rest window".to_string()),
                    Some(got) => uv_diff(
                        got,
                        uv,
                        rest_uv,
                        tile0_mode,
                        [sub.unk0a, sub.unk0c, sub.unk0e],
                        [sub.half[0], sub.half[1]],
                    ),
                };
                if let Some(d) = diff {
                    note(&mut notes, 2, format!("frame {frame}: {d}"));
                }
            }
        }

        // Two-tile blend: `sprites[texture_id_next]`, `PRIM_LOD_FRAC` and
        // tile 1, for the entries the packer lowered.
        if let Some(l) = lod {
            tally.checked[3] += 1;
            let frac = if f & MOBJ_FLAG_FRAC != 0 {
                ((lfrac - (lfrac as i32) as f32) * 256.0) as u32 as u8
            } else {
                (lfrac * 255.0) as u32 as u8
            };
            let texture = (u32::from(next) < l.next_count).then(|| l.next_textures[next as usize]);
            let tile1_moved = [uv[2], uv[3], scroll[0], scroll[1]].map(f32::to_bits)
                != [rest_uv[2], rest_uv[3], sub.scroll[0], sub.scroll[1]].map(f32::to_bits);
            let diff = match &g.lod {
                None => Some(format!("runtime has no blend (next {next}, frac {frac})")),
                Some(got) if Some(got.texture) != texture || got.frac != frac => Some(format!(
                    "next {next} texture {texture:?} frac {frac}, runtime texture {} frac {}",
                    got.texture, got.frac
                )),
                Some(got) => match (&got.uv, f & MOBJ_FLAG_TILE1 != 0 && tile1_moved) {
                    (None, false) => None,
                    (None, true) => Some("runtime draws the rest tile-1 window".to_string()),
                    (Some(w), _) => uv_diff(
                        w,
                        [scroll[0], scroll[1], uv[2], uv[3]],
                        [sub.scroll[0], sub.scroll[1], rest_uv[2], rest_uv[3]],
                        1 | if f & MOBJ_FLAG_TEXTURE != 0 { 4 } else { 0 } | half_bit,
                        [sub.unk0a, sub.unk38, sub.unk3a],
                        [sub.half[2], sub.half[1]],
                    ),
                },
            };
            if let Some(d) = diff {
                note(&mut notes, 3, format!("frame {frame}: {d}"));
            }
        }
    }
    Ok(notes)
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
    let mut resolved: Vec<Vec<Resolved>> = vec![Vec::new(); animator.len()];
    for _ in 0..TICKS {
        animator.tick(&pack);
        for (i, (frames, res)) in runtime.iter_mut().zip(&mut resolved).enumerate() {
            let i = i as u32;
            let j = animator.joint(i).ok_or("animator lost a joint")?;
            frames.push(runtime_frame(j));
            res.push(Resolved {
                texture: animator.resolved_texture(&pack, i),
                palette: animator.resolved_palette(&pack, i),
                uv: animator.resolved_uv(&pack, i),
                lod: animator.resolved_lod_blend(&pack, i),
            });
        }
    }
    let mut tally = ResolverTally::default();

    // Effect players restart per spawn but resolve through the same rules;
    // started at frame 0 they must agree with the stage clock tick for tick.
    let mut effect_bad = 0usize;
    let all: Vec<u32> = (0..pack.mat_anim_count()).collect();
    for chunk in all.chunks(ssb_rom::skeleton::MAX_EFFECT_MAT_ANIMS) {
        let mut effect = ssb_rom::skeleton::EffectMaterialAnimator::new();
        effect.start(&pack, chunk.iter().copied());
        #[allow(clippy::needless_range_loop)] // `n` indexes each entry's frames
        for n in 0..TICKS as usize {
            effect.tick(&pack);
            for &i in chunk {
                let stage = &resolved[i as usize][n];
                // RE-327: the window and the two-tile blend as well.
                if effect.resolved_texture(&pack, i) != stage.texture
                    || effect.resolved_palette(&pack, i) != stage.palette
                    || effect.resolved_uv(&pack, i) != stage.uv
                    || effect.resolved_lod_blend(&pack, i) != stage.lod
                {
                    effect_bad += 1;
                }
            }
        }
    }

    // RE-327: entries a manager effect's primitives name, and how many of
    // them resolve a texture, palette, window or blend that changes over
    // time. On the stage clock those drew the value at the frame since pack
    // load, not since the spawn.
    let effect_entries: BTreeSet<u32> = ssb_rom::effect::MANAGER_EFFECT_KEYS
        .iter()
        .filter_map(|&(file, offset)| {
            (0..pack.object_count())
                .filter_map(|o| pack.object(o))
                .find(|o| o.source_file == file && o.source_offset == offset)
        })
        .flat_map(|o| super::object_mat_anims(&pack, &o))
        .collect();
    let varies = |i: u32, f: &dyn Fn(&Resolved) -> String| {
        let r = &resolved[i as usize];
        r.iter().any(|x| f(x) != f(&r[0]))
    };
    let effect_moving: [usize; 4] = [
        effect_entries
            .iter()
            .filter(|&&i| varies(i, &|r| format!("{:?}", r.texture)))
            .count(),
        effect_entries
            .iter()
            .filter(|&&i| varies(i, &|r| format!("{:?}", r.palette)))
            .count(),
        effect_entries
            .iter()
            .filter(|&&i| varies(i, &|r| format!("{:?}", r.uv)))
            .count(),
        effect_entries
            .iter()
            .filter(|&&i| varies(i, &|r| format!("{:?}", r.lod)))
            .count(),
    ];

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
        for n in check_resolvers(
            &archive,
            &pack,
            i,
            &expected,
            &resolved[i as usize],
            &mut tally,
        )? {
            println!("  {n}");
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
    println!("Resolvers against gcDrawMObjForDObj:");
    for (k, name) in RESOLVERS.iter().enumerate() {
        println!(
            "  {name:<16} {:6} frames checked  {:6} mismatching",
            tally.checked[k], tally.bad[k]
        );
    }
    println!(
        "  {} packed-table difference(s); {} entr(ies) load a second tile the packer did not lower",
        tally.table_bad, tally.lod_not_lowered
    );
    println!("  {effect_bad} effect-player tick(s) resolve differently from the stage player");
    println!(
        "  manager-effect entries: {}; moving over {TICKS} ticks: texture {}, palette {}, window {}, blend {}",
        effect_entries.len(),
        effect_moving[0],
        effect_moving[1],
        effect_moving[2],
        effect_moving[3],
    );
    for (k, name) in ["image", "TLUT", "tile 0"].iter().enumerate() {
        println!(
            "Ownership: {:4} primitive(s) whose MObj owns the {name} its entry moves, \
             {:4} whose display list replaced it",
            tally.ownership[k][0], tally.ownership[k][1]
        );
    }
    println!("  replaced in files {:?}", tally.replaced_files);
    // A texture-keyed palette (`TextureDesc::mat_anim`, one per texture)
    // would give these primitives another `MObj`'s palette cycle.
    let shared: Vec<u32> = (0..pack.prim_count())
        .filter(|&p| {
            pack.prim(p).is_some_and(|d| {
                d.palette_anim()
                    .is_some_and(|e| pack.texture(d.texture).is_some_and(|t| t.mat_anim != e))
            })
        })
        .collect();
    println!(
        "  {} primitive(s) cycle a palette other than their texture's mat_anim: {shared:?}",
        shared.len()
    );
    let u = &tally.uv;
    println!(
        "Sampling: {} primitive(s) ({} texgen, {} over a list's own origin skipped)\n  \
         moving: {} vertex-axis samples, {} beyond 1/64 texel, max {:.4} texel\n  \
         rest: {} samples, {} beyond 1/64 texel, max {:.4} texel\n  \
         tile 1: {} primitive(s), {} samples, {} beyond 1/64 texel, max {:.4} texel",
        u.prims,
        u.texgen,
        u.dl_origin,
        u.samples,
        u.bad,
        u.max_err,
        u.rest_samples,
        u.rest_bad,
        u.rest_max,
        u.tile1_prims,
        u.tile1_samples,
        u.tile1_bad,
        u.tile1_max
    );
    println!(
        "Texels: {} packed texture(s) against sprites[] through their recorded tile, {} differ \
         ({} read through a mirrored or repeated period); \
         {} static CLUT(s) against palettes[0], {} differ",
        tally.textures_checked,
        tally.textures_bad,
        tally.textures_folded,
        tally.static_palettes_checked,
        tally.static_palettes_bad
    );
    let resolver_bad = tally.bad.iter().sum::<usize>()
        + tally.table_bad
        + effect_bad
        + tally.textures_bad
        + tally.static_palettes_bad
        + tally.uv.bad
        + tally.uv.rest_bad
        + tally.uv.tile1_bad;
    if !declined.is_empty() {
        return Err(format!("the reference declined MatAnimDesc(s) {declined:?}").into());
    }
    if mismatches > 0 {
        return Err("runtime material tracks disagree with the decomp reference".into());
    }
    if resolver_bad > 0 {
        return Err("material resolvers disagree with the decomp draw path".into());
    }
    Ok(())
}
