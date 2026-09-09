//! Decoder for SSB64's position-independent `LBParticle` banks.
//!
//! These banks live outside `relocData`: each script/texture pointer is a
//! bank-relative offset fixed up by `lbParticleSetupBankID`. The US ROM owns
//! nine script/texture pairs (160 scripts and 65 texture series total).

use alloc::vec::Vec;

use crate::texture::{BitSize, Format};

/// One source-named particle bank pair in the supported US ROM.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BankSpec {
    pub name: &'static str,
    pub scripts_lo: usize,
    pub scripts_hi: usize,
    pub textures_lo: usize,
    pub textures_hi: usize,
}

pub const BANKS: &[BankSpec] = &[
    BankSpec {
        name: "efcommon",
        scripts_lo: 0xAC7340,
        scripts_hi: 0xAC9DE0,
        textures_lo: 0xAC9DE0,
        textures_hi: 0xB16C80,
    },
    BankSpec {
        name: "particles_unk0",
        scripts_lo: 0xB16C80,
        scripts_hi: 0xB17060,
        textures_lo: 0xB17060,
        textures_hi: 0xB174A0,
    },
    BankSpec {
        name: "particles_unk1",
        scripts_lo: 0xB174A0,
        scripts_hi: 0xB176A0,
        textures_lo: 0xB176A0,
        textures_hi: 0xB19700,
    },
    BankSpec {
        name: "particles_unk2",
        scripts_lo: 0xB19700,
        scripts_hi: 0xB19850,
        textures_lo: 0xB19850,
        textures_hi: 0xB1BCA0,
    },
    BankSpec {
        name: "itcommon",
        scripts_lo: 0xB1BCA0,
        scripts_hi: 0xB1BDE0,
        textures_lo: 0xB1BDE0,
        textures_hi: 0xB1E640,
    },
    BankSpec {
        name: "grpupupu",
        scripts_lo: 0xB1E640,
        scripts_hi: 0xB1E7E0,
        textures_lo: 0xB1E7E0,
        textures_hi: 0xB1F960,
    },
    BankSpec {
        name: "grhyrule",
        scripts_lo: 0xB1F960,
        scripts_hi: 0xB1FC80,
        textures_lo: 0xB1FC80,
        textures_hi: 0xB22980,
    },
    BankSpec {
        name: "gryoster",
        scripts_lo: 0xB22980,
        scripts_hi: 0xB22A00,
        textures_lo: 0xB22A00,
        textures_hi: 0xB22C30,
    },
    BankSpec {
        name: "mntitle",
        scripts_lo: 0xB22C30,
        scripts_hi: 0xB22D40,
        textures_lo: 0xB22D40,
        textures_hi: 0xB277B0,
    },
];

#[derive(Debug, Clone, PartialEq)]
pub struct Script<'a> {
    pub kind: u16,
    pub texture_id: u16,
    pub generator_lifetime: u16,
    pub particle_lifetime: u16,
    pub flags: u32,
    pub gravity: f32,
    pub friction: f32,
    pub velocity: [f32; 3],
    pub unknown_20: f32,
    pub unknown_24: f32,
    pub update_rate: f32,
    pub size: f32,
    /// Includes alignment bytes after the terminating `DEAD`/`END` command.
    pub bytecode: &'a [u8],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Texture<'a> {
    pub format: Format,
    pub size: BitSize,
    pub width: u32,
    pub height: u32,
    pub flags: u32,
    pub images: Vec<&'a [u8]>,
    /// Empty for non-CI, one entry for shared-palette CI, otherwise one per image.
    pub palettes: Vec<&'a [u8]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BytecodeSummary {
    pub instructions: usize,
    pub used_bytes: usize,
    pub terminator: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParticleError {
    Truncated,
    BadCount(u32),
    BadOffset(u32),
    BadFormat(u32),
    BadSize(u32),
    BadDimensions(u32, u32),
    MissingTerminator,
    UnknownOpcode(u8),
}

fn be_u16(data: &[u8], at: usize) -> Result<u16, ParticleError> {
    let b = data.get(at..at + 2).ok_or(ParticleError::Truncated)?;
    Ok(u16::from_be_bytes([b[0], b[1]]))
}

fn be_u32(data: &[u8], at: usize) -> Result<u32, ParticleError> {
    let b = data.get(at..at + 4).ok_or(ParticleError::Truncated)?;
    Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
}

fn be_f32(data: &[u8], at: usize) -> Result<f32, ParticleError> {
    Ok(f32::from_bits(be_u32(data, at)?))
}

fn table_offsets(data: &[u8]) -> Result<Vec<usize>, ParticleError> {
    let count = be_u32(data, 0)?;
    if count == 0 || count > 4096 {
        return Err(ParticleError::BadCount(count));
    }
    let table_end = 4usize
        .checked_add(count as usize * 4)
        .ok_or(ParticleError::BadCount(count))?;
    if table_end > data.len() {
        return Err(ParticleError::Truncated);
    }
    let mut out = Vec::with_capacity(count as usize);
    for i in 0..count as usize {
        let raw = be_u32(data, 4 + i * 4)?;
        let offset = raw as usize;
        if offset < table_end || offset >= data.len() || out.last().is_some_and(|&p| offset <= p) {
            return Err(ParticleError::BadOffset(raw));
        }
        out.push(offset);
    }
    Ok(out)
}

pub fn decode_scripts(data: &[u8]) -> Result<Vec<Script<'_>>, ParticleError> {
    let offsets = table_offsets(data)?;
    let mut scripts = Vec::with_capacity(offsets.len());
    for (i, &at) in offsets.iter().enumerate() {
        let end = offsets.get(i + 1).copied().unwrap_or(data.len());
        if end < at + 0x30 {
            return Err(ParticleError::Truncated);
        }
        let bytecode = &data[at + 0x30..end];
        inspect_bytecode(bytecode)?;
        scripts.push(Script {
            kind: be_u16(data, at)?,
            texture_id: be_u16(data, at + 2)?,
            generator_lifetime: be_u16(data, at + 4)?,
            particle_lifetime: be_u16(data, at + 6)?,
            flags: be_u32(data, at + 8)?,
            gravity: be_f32(data, at + 0x0C)?,
            friction: be_f32(data, at + 0x10)?,
            velocity: [
                be_f32(data, at + 0x14)?,
                be_f32(data, at + 0x18)?,
                be_f32(data, at + 0x1C)?,
            ],
            unknown_20: be_f32(data, at + 0x20)?,
            unknown_24: be_f32(data, at + 0x24)?,
            update_rate: be_f32(data, at + 0x28)?,
            size: be_f32(data, at + 0x2C)?,
            bytecode,
        });
    }
    Ok(scripts)
}

pub fn decode_textures(data: &[u8]) -> Result<Vec<Texture<'_>>, ParticleError> {
    let offsets = table_offsets(data)?;
    let mut textures = Vec::with_capacity(offsets.len());
    for &at in &offsets {
        let count = be_u32(data, at)?;
        // EFCommon texture 6 is an authored zero-frame placeholder. It has a
        // valid header and no pointer array; retain it because script texture
        // IDs index the table including that slot.
        if count > 4096 {
            return Err(ParticleError::BadCount(count));
        }
        let format_raw = be_u32(data, at + 4)?;
        let size_raw = be_u32(data, at + 8)?;
        let format = Format::from_raw(format_raw as u8)
            .filter(|_| format_raw <= u8::MAX as u32)
            .ok_or(ParticleError::BadFormat(format_raw))?;
        let size = BitSize::from_raw(size_raw as u8)
            .filter(|_| size_raw <= u8::MAX as u32)
            .ok_or(ParticleError::BadSize(size_raw))?;
        let width = be_u32(data, at + 0x0C)?;
        let height = be_u32(data, at + 0x10)?;
        if width == 0 || height == 0 || width > 4096 || height > 4096 {
            return Err(ParticleError::BadDimensions(width, height));
        }
        let flags = be_u32(data, at + 0x14)?;
        let image_len = (width as usize)
            .checked_mul(height as usize)
            .and_then(|n| n.checked_mul(size.bits()))
            .map(|bits| bits.div_ceil(8))
            .ok_or(ParticleError::BadDimensions(width, height))?;
        let palette_count = if format == Format::Ci {
            if flags & 1 != 0 {
                1
            } else {
                count as usize
            }
        } else {
            0
        };
        let pointer_count = count as usize + palette_count;
        data.get(at + 0x18..at + 0x18 + pointer_count * 4)
            .ok_or(ParticleError::Truncated)?;

        let mut images = Vec::with_capacity(count as usize);
        for i in 0..count as usize {
            let raw = be_u32(data, at + 0x18 + i * 4)?;
            let offset = raw as usize;
            images.push(
                data.get(offset..offset + image_len)
                    .ok_or(ParticleError::BadOffset(raw))?,
            );
        }
        let palette_len = match size {
            BitSize::Bits4 => 16 * 2,
            BitSize::Bits8 => 256 * 2,
            _ => 0,
        };
        if format == Format::Ci && palette_len == 0 {
            return Err(ParticleError::BadSize(size_raw));
        }
        let mut palettes = Vec::with_capacity(palette_count);
        for i in 0..palette_count {
            let raw = be_u32(data, at + 0x18 + (count as usize + i) * 4)?;
            let offset = raw as usize;
            palettes.push(
                data.get(offset..offset + palette_len)
                    .ok_or(ParticleError::BadOffset(raw))?,
            );
        }
        textures.push(Texture {
            format,
            size,
            width,
            height,
            flags,
            images,
            palettes,
        });
    }
    Ok(textures)
}

fn take(data: &[u8], cursor: &mut usize, len: usize) -> Result<(), ParticleError> {
    *cursor = cursor.checked_add(len).ok_or(ParticleError::Truncated)?;
    if *cursor > data.len() {
        Err(ParticleError::Truncated)
    } else {
        Ok(())
    }
}

fn take_var_u16(data: &[u8], cursor: &mut usize) -> Result<(), ParticleError> {
    let first = *data.get(*cursor).ok_or(ParticleError::Truncated)?;
    take(data, cursor, if first & 0x80 != 0 { 2 } else { 1 })
}

/// Validates operand widths exactly as `lbParticleUpdateStruct` consumes them.
pub fn inspect_bytecode(data: &[u8]) -> Result<BytecodeSummary, ParticleError> {
    let mut cursor = 0usize;
    let mut instructions = 0usize;
    while cursor < data.len() {
        let command = data[cursor];
        cursor += 1;
        instructions += 1;
        if command < 0x80 {
            if command & 0x20 != 0 {
                take(data, &mut cursor, 1)?;
            }
            if command & 0x40 != 0 {
                take(data, &mut cursor, 1)?;
            }
            continue;
        }
        let grouped = command & 0xF8;
        if matches!(grouped, 0x80 | 0x88 | 0x90 | 0x98) {
            take(data, &mut cursor, (command & 7).count_ones() as usize * 4)?;
            continue;
        }
        match command {
            0xA0 => {
                take_var_u16(data, &mut cursor)?;
                take(data, &mut cursor, 4)?;
            }
            0xA1 => take(data, &mut cursor, 1)?,
            0xA2 | 0xA3 | 0xA9 | 0xAB => take(data, &mut cursor, 4)?,
            0xA4 | 0xA5 | 0xB9 => take(data, &mut cursor, 2)?,
            0xA6 | 0xAA => take(data, &mut cursor, 4)?,
            0xA7 | 0xB7 | 0xBF | 0xFA => take(data, &mut cursor, 1)?,
            0xA8 | 0xBE => take(data, &mut cursor, 12)?,
            0xAC => {
                take_var_u16(data, &mut cursor)?;
                take(data, &mut cursor, 8)?;
            }
            0xAD..=0xB6 | 0xFB..=0xFD => {}
            0xB8 => take(data, &mut cursor, 5)?,
            0xBA | 0xBB => take(data, &mut cursor, 4)?,
            0xBC => take(data, &mut cursor, 2)?,
            0xBD => take(data, &mut cursor, 8)?,
            0xC0..=0xDF => {
                take_var_u16(data, &mut cursor)?;
                take(data, &mut cursor, (command & 0x0F).count_ones() as usize)?;
            }
            0xFE | 0xFF => {
                return Ok(BytecodeSummary {
                    instructions,
                    used_bytes: cursor,
                    terminator: command,
                })
            }
            _ => return Err(ParticleError::UnknownOpcode(command)),
        }
    }
    Err(ParticleError::MissingTerminator)
}

pub fn decode_bank<'a>(
    rom: &'a [u8],
    spec: BankSpec,
) -> Result<(Vec<Script<'a>>, Vec<Texture<'a>>), ParticleError> {
    let scripts = rom
        .get(spec.scripts_lo..spec.scripts_hi)
        .ok_or(ParticleError::Truncated)?;
    let textures = rom
        .get(spec.textures_lo..spec.textures_hi)
        .ok_or(ParticleError::Truncated)?;
    let scripts = decode_scripts(scripts)?;
    let textures = decode_textures(textures)?;
    if scripts
        .iter()
        .any(|script| script.texture_id as usize >= textures.len())
    {
        return Err(ParticleError::BadOffset(textures.len() as u32));
    }
    Ok((scripts, textures))
}

// ============================================================================
// Deterministic single-particle bytecode playback.
//
// Reproduces `lbParticleUpdateStruct` (`refs/ssb-decomp-re/src/lb/
// lbparticle.c:707-1415`) for one `LBParticle` instance: the wait/opcode
// dispatch loop, the size/color lerps, and the gravity/friction/position
// integration that runs unconditionally after it.
//
// Scope, measured archive-wide against the real US ROM (all 160 scripts,
// `SSB64_ROM`-gated census, since reverted):
//   - `LBPARTICLE_FLAG_VORTEX` is set by zero script headers and zero
//     `SETFLAG` operands. Its physics branch (`lbparticle.c:1347-1386`) reads
//     an `LBGenerator`'s own vortex table, which this module does not model;
//     [`SimError::VortexUnsupported`] fails loudly instead of guessing if a
//     future script ever sets it.
//   - `SETDISTVEL`/`ADDDISTVELMAG`/`SETATTACHID`'s write-back all require a
//     live `DObj` (`sLBParticleAttachDObjs`). Zero real scripts use any of
//     the three. Operands are still decoded (cursor stays in sync); no state
//     mutation is attempted.
//   - `MAKESCRIPT`/`MAKERAND`/`MAKEID` (25/0/2 real uses) and `MAKEGENERATOR`
//     (103 real uses) are common. Reproducing them exactly requires
//     replicating `lbParticleStructFuncRun`'s node-splice walk, which visits
//     a newly spawned child a second time within its own spawning frame
//     (`lbparticle.c:1426-1447`) on top of the spawning opcode's own
//     immediate recursive tick (`lbparticle.c:895`) -- a real double-tick
//     quirk, not yet reproduced here. Operands decode correctly and are
//     reported as [`SpawnRequest`]s instead of being executed, so cursor
//     sync and every other opcode's correctness do not depend on it.
//
// Everything else -- position/velocity set/add, size lerp, primitive/
// environment color lerp, gravity, friction, lifetime, loop/return control
// flow, and `SETVELANGLE`'s rotation (which needs `syUtilsArcTan2`, ported
// below) -- is reproduced exactly.

/// `syUtilsRandFloat`'s LCG (`refs/ssb-decomp-re/src/sys/utils.c:172-178`).
///
/// The real seed is a single global shared by the whole game's frame loop;
/// there is no way to recover *which* value it held at a given script's
/// real spawn moment without a full game-state replay. Callers supply their
/// own seed and get an exactly reproducible sequence from it -- this proves
/// the interpreter is deterministic, not that it replays a specific real
/// playthrough's random draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rng(i32);

impl Rng {
    pub fn new(seed: i32) -> Self {
        Rng(seed)
    }

    /// `[0.0, 1.0)`, matching `syUtilsRandFloat` bit-for-bit.
    pub fn next_float(&mut self) -> f32 {
        let step = self.0.wrapping_mul(214013).wrapping_add(2531011);
        self.0 = step;
        (((step >> 16) & 0xFFFF) as u32 as f32) / 65536.0
    }
}

/// Newton-Raphson square root. `core` has no `sqrt` without `std`/`libm`;
/// mirrors the no-dependency convention `ssb_engine::math::sqrt` already
/// uses for the same reason.
fn sqrt(v: f32) -> f32 {
    if v <= 0.0 {
        return 0.0;
    }
    let mut x = f32::from_bits((v.to_bits() >> 1) + 0x1FC0_0000);
    for _ in 0..4 {
        x = 0.5 * (x + v / x);
    }
    x
}

/// `syUtilsArcTan` (`refs/ssb-decomp-re/src/sys/utils.c:25-77`): an exact,
/// dependency-free rational-polynomial approximation, not a table lookup, so
/// it ports directly.
fn arctan(div: f32) -> f32 {
    const M_PI_F: f32 = core::f32::consts::PI;
    let (div, kind) = if div == 0.0 {
        return 0.0;
    } else if div > 1.0 {
        (1.0 / div, 1)
    } else if div < -1.0 {
        (1.0 / div, 2)
    } else {
        (div, 0)
    };
    let d2 = div * div;
    let result = (d2
        / (d2 / (d2 / (d2 / (d2 / -0.10810675 + -44.57192) + -0.1619081) + -15.774018)
            + -0.55556977)
        + -3.000003
        + 1.0)
        * div;
    match kind {
        1 => M_PI_F / 2.0 - result,
        2 => -(M_PI_F / 2.0) - result,
        _ => result,
    }
}

/// `syUtilsArcTan2` (`refs/ssb-decomp-re/src/sys/utils.c:79-100`).
fn arctan2(y: f32, x: f32) -> f32 {
    const M_PI_F: f32 = core::f32::consts::PI;
    if x > 0.0 {
        arctan(y / x)
    } else if x < 0.0 {
        let sign = if y < 0.0 { -1.0 } else { 1.0 };
        (M_PI_F - arctan((y / x).abs())) * sign
    } else if y != 0.0 {
        (if y < 0.0 { -1.0 } else { 1.0 }) * (M_PI_F / 2.0)
    } else {
        0.0
    }
}

/// `LBPARTICLE_FLAG_*` (`refs/ssb-decomp-re/src/lb/lbdef.h:11-22`).
pub mod flag {
    pub const GRAVITY: u32 = 0x1;
    pub const FRICTION: u32 = 0x2;
    pub const VORTEX: u32 = 0x4;
    pub const SHAREDPAL: u32 = 0x10;
    pub const MASKS: u32 = 0x20;
    pub const MASKT: u32 = 0x40;
    pub const ENVCOLOR: u32 = 0x80;
    pub const NOISE: u32 = 0x100;
    pub const ALPHABLEND: u32 = 0x200;
    pub const DITHER: u32 = 0x400;
    pub const PAUSE: u32 = 0x800;
    pub const ATTACH: u32 = 0x8000;
}

/// A decoded but unexecuted `MAKESCRIPT`/`MAKERAND`/`MAKEID`/`MAKEGENERATOR`
/// call. See the module-level scope note for why these are reported rather
/// than spawned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpawnRequest {
    pub script_id: u16,
    pub inherit_velocity: bool,
    pub is_generator: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SimError {
    Truncated,
    /// A script set `LBPARTICLE_FLAG_VORTEX`; see the module-level scope
    /// note. Zero real US ROM scripts do this today.
    VortexUnsupported,
}

/// One live `LBParticle` instance's simulated state, mirroring `struct
/// LBParticle` (`refs/ssb-decomp-re/src/lb/lbtypes.h:200-230`) minus the
/// fields the real engine only needs for its own allocator bookkeeping
/// (`next`, `bank_id`, `generator_id`, `gn`, `xf`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParticleState {
    pub pos: [f32; 3],
    pub vel: [f32; 3],
    pub size: f32,
    pub size_target: f32,
    pub size_target_length: u16,
    pub gravity: f32,
    pub friction: f32,
    pub flags: u32,
    pub texture_id: u16,
    pub frame_id: u8,
    pub primcolor: [u8; 4],
    pub target_primcolor: [u8; 4],
    pub primcolor_target_length: u16,
    pub envcolor: [u8; 4],
    pub target_envcolor: [u8; 4],
    pub envcolor_target_length: u16,
    /// Ends at 1, not 0 -- matches `lbtypes.h`'s own comment on this field.
    pub lifetime: u16,
    pub bytecode_csr: u16,
    pub bytecode_timer: u16,
    pub return_ptr: u16,
    pub loop_ptr: u16,
    pub loop_count: u8,
    pub alive: bool,
}

impl ParticleState {
    /// Whether a draw call this frame would put anything on screen: real
    /// hardware's `lbParticleDrawTextures` (`lbparticle.c:1450-2118`) skips
    /// the draw entirely when `size <= 0`, and a zero-frame texture series
    /// has nothing to bind either. Shared by the PSP viewer
    /// (`psp/src/main.rs`'s `particle_view`) and `romtool`'s frame-4 census
    /// so both ask the identical question instead of two hand-written copies
    /// silently drifting apart.
    pub fn visible(&self, frame_count: u32) -> bool {
        frame_count > 0 && self.size > 0.0
    }
}

fn read_u8(data: &[u8], cursor: &mut usize) -> Result<u8, SimError> {
    let v = *data.get(*cursor).ok_or(SimError::Truncated)?;
    *cursor += 1;
    Ok(v)
}

fn read_be_u16(data: &[u8], cursor: &mut usize) -> Result<u16, SimError> {
    let hi = read_u8(data, cursor)? as u16;
    let lo = read_u8(data, cursor)? as u16;
    Ok((hi << 8) | lo)
}

fn read_be_f32(data: &[u8], cursor: &mut usize) -> Result<f32, SimError> {
    let bits = u32::from_be_bytes([
        read_u8(data, cursor)?,
        read_u8(data, cursor)?,
        read_u8(data, cursor)?,
        read_u8(data, cursor)?,
    ]);
    Ok(f32::from_bits(bits))
}

/// `lbParticleReadUShort` (`lbparticle.c:600-611`): value is stored `+1`.
fn read_var_u16(data: &[u8], cursor: &mut usize) -> Result<u16, SimError> {
    let first = read_u8(data, cursor)? as u16;
    let value = if first & 0x80 != 0 {
        let lo = read_u8(data, cursor)? as u16;
        ((first & 0x7F) << 8) + lo
    } else {
        first
    };
    Ok(value + 1)
}

/// `((current << 16) + (target - current) * (65536 / length)) >> 16`,
/// truncated back into a `u8` exactly as the C narrowing assignment does
/// (`lbparticle.c:1278-1288`).
fn lerp_channel(current: u8, target: u8, length: u16) -> u8 {
    let cur = current as i32;
    let step = 65536i32 / length as i32;
    let value = (cur << 16) + (target as i32 - cur) * step;
    (value >> 16) as u8
}

/// One live `LBParticle` plus the bytecode it's executing.
#[derive(Debug, Clone, PartialEq)]
pub struct Particle<'a> {
    pub bytecode: &'a [u8],
    pub state: ParticleState,
}

impl<'a> Particle<'a> {
    /// `lbParticleMakeChildScriptID` + `lbParticleMakeStruct`
    /// (`lbparticle.c:267-369,372-403`) with `pos = (0,0,0)` -- the real
    /// engine's caller (`lbParticleMakeParam`/a parent's own position)
    /// fills that in; texture-bank `SHAREDPAL` is a draw-time detail this
    /// state-only simulator does not model.
    pub fn spawn(script: &Script<'a>) -> Self {
        Particle {
            bytecode: script.bytecode,
            state: ParticleState {
                pos: [0.0; 3],
                vel: script.velocity,
                size: script.size,
                size_target: 0.0,
                size_target_length: 0,
                gravity: script.gravity,
                friction: script.friction,
                flags: script.flags,
                texture_id: script.texture_id,
                frame_id: 0,
                primcolor: [0xFF; 4],
                target_primcolor: [0xFF; 4],
                primcolor_target_length: 0,
                envcolor: [0, 0, 0, 0],
                target_envcolor: [0, 0, 0, 0],
                envcolor_target_length: 0,
                lifetime: script.particle_lifetime.wrapping_add(1),
                bytecode_csr: 0,
                bytecode_timer: u16::from(!script.bytecode.is_empty()),
                return_ptr: 0,
                loop_ptr: 0,
                loop_count: 0,
                alive: true,
            },
        }
    }

    /// One frame: `lbParticleUpdateStruct`'s pause check, bytecode dispatch,
    /// and unconditional size/color lerp + physics + lifetime tail
    /// (`lbparticle.c:734-1414`). Returns any `MAKESCRIPT`/`MAKERAND`/
    /// `MAKEID`/`MAKEGENERATOR` calls seen this frame, decoded but not
    /// executed (see the module-level scope note).
    pub fn tick(&mut self, rng: &mut Rng) -> Result<Vec<SpawnRequest>, SimError> {
        let mut spawns = Vec::new();
        if !self.state.alive || self.state.flags & flag::PAUSE != 0 {
            return Ok(spawns);
        }
        if self.state.bytecode_timer != 0 {
            self.state.bytecode_timer -= 1;
            if self.state.bytecode_timer == 0 {
                self.run_bytecode(rng, &mut spawns)?;
            }
        }
        self.post_bytecode_update()?;
        Ok(spawns)
    }

    fn run_bytecode(
        &mut self,
        rng: &mut Rng,
        spawns: &mut Vec<SpawnRequest>,
    ) -> Result<(), SimError> {
        let data = self.bytecode;
        let mut cursor = self.state.bytecode_csr as usize;
        let mut timer;
        loop {
            let command = read_u8(data, &mut cursor)?;
            if command < 0x80 {
                timer = (command & 0x1F) as u16;
                if command & 0x20 != 0 {
                    timer = read_u8(data, &mut cursor)? as u16 + (timer << 8);
                }
                if command & 0xC0 == 0x40 {
                    self.state.frame_id = read_u8(data, &mut cursor)?;
                }
            } else {
                timer = 0;
                if !self.dispatch_opcode(command, data, &mut cursor, rng, spawns)? {
                    break; // DEAD/END/TRYDEADRAND-kill: matches `goto loop_break`.
                }
            }
            if timer != 0 {
                break;
            }
        }
        self.state.bytecode_csr = cursor as u16;
        self.state.bytecode_timer = timer;
        Ok(())
    }

    /// Returns `false` for the `goto loop_break` cases (`DEAD`/`END`, or a
    /// losing `TRYDEADRAND`), matching `lbparticle.c:750-1244`.
    fn dispatch_opcode(
        &mut self,
        command: u8,
        data: &'a [u8],
        cursor: &mut usize,
        rng: &mut Rng,
        spawns: &mut Vec<SpawnRequest>,
    ) -> Result<bool, SimError> {
        let s = &mut self.state;
        match command {
            0x80..=0x87 => {
                if command & 1 != 0 {
                    s.pos[0] = read_be_f32(data, cursor)?;
                }
                if command & 2 != 0 {
                    s.pos[1] = read_be_f32(data, cursor)?;
                }
                if command & 4 != 0 {
                    s.pos[2] = read_be_f32(data, cursor)?;
                }
            }
            0x88..=0x8F => {
                if command & 1 != 0 {
                    s.pos[0] += read_be_f32(data, cursor)?;
                }
                if command & 2 != 0 {
                    s.pos[1] += read_be_f32(data, cursor)?;
                }
                if command & 4 != 0 {
                    s.pos[2] += read_be_f32(data, cursor)?;
                }
            }
            0x90..=0x97 => {
                if command & 1 != 0 {
                    s.vel[0] = read_be_f32(data, cursor)?;
                }
                if command & 2 != 0 {
                    s.vel[1] = read_be_f32(data, cursor)?;
                }
                if command & 4 != 0 {
                    s.vel[2] = read_be_f32(data, cursor)?;
                }
            }
            0x98..=0x9F => {
                if command & 1 != 0 {
                    s.vel[0] += read_be_f32(data, cursor)?;
                }
                if command & 2 != 0 {
                    s.vel[1] += read_be_f32(data, cursor)?;
                }
                if command & 4 != 0 {
                    s.vel[2] += read_be_f32(data, cursor)?;
                }
            }
            0xA0 => {
                // SETSIZELERP
                s.size_target_length = read_var_u16(data, cursor)?;
                s.size_target = read_be_f32(data, cursor)?;
                if s.size_target_length == 1 {
                    s.size = s.size_target;
                    s.size_target_length = 0;
                }
            }
            0xA1 => s.flags = read_u8(data, cursor)? as u32, // SETFLAG
            0xA2 => {
                // SETGRAVITY
                s.gravity = read_be_f32(data, cursor)?;
                if s.gravity == 0.0 {
                    s.flags &= !flag::GRAVITY;
                } else {
                    s.flags |= flag::GRAVITY;
                }
            }
            0xA3 => {
                // SETFRICTION
                s.friction = read_be_f32(data, cursor)?;
                if s.friction == 1.0 {
                    s.flags &= !flag::FRICTION;
                } else {
                    s.flags |= flag::FRICTION;
                }
            }
            0xA4 => {
                // MAKESCRIPT
                let script_id = read_be_u16(data, cursor)?;
                spawns.push(SpawnRequest {
                    script_id,
                    inherit_velocity: false,
                    is_generator: false,
                });
            }
            0xA5 => {
                // MAKEGENERATOR -- decoded, never executed; see scope note.
                let script_id = read_be_u16(data, cursor)?;
                spawns.push(SpawnRequest {
                    script_id,
                    inherit_velocity: false,
                    is_generator: true,
                });
            }
            0xA6 => {
                // SETLIFERAND
                let base = read_be_u16(data, cursor)? as i32;
                let range = read_be_u16(data, cursor)? as i32;
                s.lifetime = (base + (range as f32 * rng.next_float()) as i32) as u16;
            }
            0xA7 => {
                // TRYDEADRAND
                let percent = read_u8(data, cursor)? as f32;
                let roll = rng.next_float() * 100.0;
                if percent < roll {
                    // Survives; falls through to the next instruction.
                } else {
                    s.lifetime = 1;
                    return Ok(false);
                }
            }
            0xA8 => {
                // ADDVELRAND -- misnamed; modifies position, not velocity.
                let rx = read_be_f32(data, cursor)?;
                s.pos[0] += rx * rng.next_float();
                let ry = read_be_f32(data, cursor)?;
                s.pos[1] += ry * rng.next_float();
                let rz = read_be_f32(data, cursor)?;
                s.pos[2] += rz * rng.next_float();
            }
            0xA9 => {
                // SETVELANGLE
                let angle = read_be_f32(data, cursor)?;
                rotate_vel(s, angle, rng);
            }
            0xAA => {
                // MAKERAND
                let base = read_be_u16(data, cursor)? as i32;
                let range = read_be_u16(data, cursor)? as i32;
                let script_id = (base + (range as f32 * rng.next_float()) as i32) as u16;
                spawns.push(SpawnRequest {
                    script_id,
                    inherit_velocity: false,
                    is_generator: false,
                });
            }
            0xAB => {
                // MULVELUFORM
                let scale = read_be_f32(data, cursor)?;
                s.vel[0] *= scale;
                s.vel[1] *= scale;
                s.vel[2] *= scale;
            }
            0xAC => {
                // SETSIZERAND
                s.size_target_length = read_var_u16(data, cursor)?;
                s.size_target = read_be_f32(data, cursor)?;
                let range = read_be_f32(data, cursor)?;
                s.size_target += range * rng.next_float();
                if s.size_target_length == 1 {
                    s.size = s.size_target;
                    s.size_target_length = 0;
                }
            }
            0xAD => s.flags |= flag::ENVCOLOR,
            0xAE => s.flags &= !(flag::MASKT | flag::MASKS),
            0xAF => {
                s.flags &= !flag::MASKT;
                s.flags |= flag::MASKS;
            }
            0xB0 => {
                s.flags &= !flag::MASKS;
                s.flags |= flag::MASKT;
            }
            0xB1 => s.flags |= flag::MASKT | flag::MASKS,
            0xB2 => s.flags |= flag::ALPHABLEND,
            0xB3 => s.flags &= !flag::DITHER,
            0xB4 => s.flags |= flag::DITHER,
            // Naming in lbdef.h is inverted relative to behaviour: NONOISE
            // *sets* the flag, NOISE *clears* it (matches lbparticle.c:1046-1052).
            0xB5 => s.flags |= flag::NOISE,
            0xB6 => s.flags &= !flag::NOISE,
            0xB7 | 0xB8 => {
                // SETDISTVEL / ADDDISTVELMAG -- need a live DObj; see scope
                // note. Zero real scripts use either. Decode only.
                read_u8(data, cursor)?;
                if command == 0xB8 {
                    read_be_f32(data, cursor)?;
                }
            }
            0xB9 => {
                // MAKEID -- also inherits velocity.
                let script_id = read_be_u16(data, cursor)?;
                spawns.push(SpawnRequest {
                    script_id,
                    inherit_velocity: true,
                    is_generator: false,
                });
            }
            0xBA => {
                // PRIMBLENDRAND (zero real uses; best-effort port, see scope note)
                blend_rand(&mut s.target_primcolor, data, cursor, rng)?;
                if s.primcolor_target_length == 0 {
                    s.primcolor = s.target_primcolor;
                }
            }
            0xBB => {
                // ENVBLENDRAND (zero real uses; best-effort port, see scope note)
                blend_rand(&mut s.target_envcolor, data, cursor, rng)?;
                if s.envcolor_target_length == 0 {
                    s.envcolor = s.target_envcolor;
                }
            }
            0xBC => {
                // Unnamed in lbdef.h; frame_id randomization.
                let base = read_u8(data, cursor)? as i32;
                let range = read_u8(data, cursor)? as f32;
                s.frame_id = (base + (range * rng.next_float()) as i32) as u8;
            }
            0xBD => {
                // SETVELMAG
                let base = read_be_f32(data, cursor)?;
                let range = read_be_f32(data, cursor)?;
                let target_mag = base + range * rng.next_float();
                let mag = sqrt(s.vel[0] * s.vel[0] + s.vel[1] * s.vel[1] + s.vel[2] * s.vel[2]);
                let scale = target_mag / mag;
                s.vel[0] *= scale;
                s.vel[1] *= scale;
                s.vel[2] *= scale;
            }
            0xBE => {
                // MULVELAXIS
                s.vel[0] *= read_be_f32(data, cursor)?;
                s.vel[1] *= read_be_f32(data, cursor)?;
                s.vel[2] *= read_be_f32(data, cursor)?;
            }
            0xBF => {
                // SETATTACHID -- the flag-set half needs no live DObj; the
                // per-frame write-back it enables does (see scope note).
                let id = read_u8(data, cursor)? as u32 - 1;
                s.flags |= flag::ATTACH | (id << 0xC);
            }
            0xC0..=0xCF => {
                // SETPRIMBLEND
                s.primcolor_target_length = read_var_u16(data, cursor)?;
                s.target_primcolor = s.primcolor;
                if command & 1 != 0 {
                    s.target_primcolor[0] = read_u8(data, cursor)?;
                }
                if command & 2 != 0 {
                    s.target_primcolor[1] = read_u8(data, cursor)?;
                }
                if command & 4 != 0 {
                    s.target_primcolor[2] = read_u8(data, cursor)?;
                }
                if command & 8 != 0 {
                    s.target_primcolor[3] = read_u8(data, cursor)?;
                }
                if s.primcolor_target_length == 1 {
                    s.primcolor = s.target_primcolor;
                    s.primcolor_target_length = 0;
                }
            }
            0xD0..=0xDF => {
                // SETENVBLEND
                s.envcolor_target_length = read_var_u16(data, cursor)?;
                s.target_envcolor = s.envcolor;
                if command & 1 != 0 {
                    s.target_envcolor[0] = read_u8(data, cursor)?;
                }
                if command & 2 != 0 {
                    s.target_envcolor[1] = read_u8(data, cursor)?;
                }
                if command & 4 != 0 {
                    s.target_envcolor[2] = read_u8(data, cursor)?;
                }
                if command & 8 != 0 {
                    s.target_envcolor[3] = read_u8(data, cursor)?;
                }
                if s.envcolor_target_length == 1 {
                    s.envcolor = s.target_envcolor;
                    s.envcolor_target_length = 0;
                }
            }
            0xFA => {
                // SETLOOP
                s.loop_count = read_u8(data, cursor)?;
                s.loop_ptr = *cursor as u16;
            }
            0xFB => {
                // LOOP
                s.loop_count = s.loop_count.wrapping_sub(1);
                if s.loop_count != 0 {
                    *cursor = s.loop_ptr as usize;
                }
            }
            0xFC => s.return_ptr = *cursor as u16, // SETRETURN
            0xFD => *cursor = s.return_ptr as usize, // RETURN
            0xFE | 0xFF => {
                // DEAD / END
                s.lifetime = 1;
                return Ok(false);
            }
            _ => return Err(SimError::Truncated),
        }
        Ok(true)
    }

    /// The unconditional tail of `lbParticleUpdateStruct`
    /// (`lbparticle.c:1270-1414`): size/color lerp, then lifetime, then
    /// gravity/friction/position (or vortex, which this module declines).
    fn post_bytecode_update(&mut self) -> Result<(), SimError> {
        let s = &mut self.state;
        if s.size_target_length != 0 {
            s.size += (s.size_target - s.size) / s.size_target_length as f32;
            s.size_target_length -= 1;
        }
        if s.primcolor_target_length != 0 {
            for i in 0..4 {
                s.primcolor[i] = lerp_channel(
                    s.primcolor[i],
                    s.target_primcolor[i],
                    s.primcolor_target_length,
                );
            }
            s.primcolor_target_length -= 1;
        }
        if s.envcolor_target_length != 0 {
            for i in 0..4 {
                s.envcolor[i] = lerp_channel(
                    s.envcolor[i],
                    s.target_envcolor[i],
                    s.envcolor_target_length,
                );
            }
            s.envcolor_target_length -= 1;
        }
        s.lifetime = s.lifetime.wrapping_sub(1);
        if s.lifetime == 0 {
            s.alive = false;
            return Ok(());
        }
        if s.flags & flag::VORTEX != 0 {
            return Err(SimError::VortexUnsupported);
        }
        if s.flags & flag::GRAVITY != 0 {
            s.vel[1] -= s.gravity;
        }
        if s.flags & flag::FRICTION != 0 {
            s.vel[0] *= s.friction;
            s.vel[1] *= s.friction;
            s.vel[2] *= s.friction;
        }
        s.pos[0] += s.vel[0];
        s.pos[1] += s.vel[1];
        s.pos[2] += s.vel[2];
        // ATTACH's write-back into a live DObj is not modeled; see scope note.
        Ok(())
    }
}

/// `lbParticleRotateVel` (`lbparticle.c:614-654`).
fn rotate_vel(s: &mut ParticleState, angle: f32, rng: &mut Rng) {
    let (vx, vy, vz) = (s.vel[0], s.vel[1], s.vel[2]);
    let pitch = arctan2(vy, vz);
    let (sin_pitch, cos_pitch) = crate::scene::sin_cos(pitch);
    let yaw = arctan2(vx, vy * sin_pitch + vz * cos_pitch);
    let (sin_yaw, cos_yaw) = crate::scene::sin_cos(yaw);
    let magnitude = sqrt(vx * vx + vy * vy + vz * vz);

    let spin = rng.next_float() * (360.0f32.to_radians());
    let sin_angle = {
        let (sa, _) = crate::scene::sin_cos(angle);
        sa * magnitude
    };
    let cos_angle = {
        let (_, ca) = crate::scene::sin_cos(angle);
        ca * magnitude
    };

    let new_vz = sin_yaw;
    let (spin_sin, spin_cos) = crate::scene::sin_cos(spin);
    let new_vx = spin_cos * sin_angle;
    let new_vy = spin_sin * sin_angle;

    s.vel[0] = new_vx * cos_yaw + cos_angle * sin_yaw;
    s.vel[1] =
        (-new_vx * sin_pitch * sin_yaw + new_vy * cos_pitch) + cos_angle * sin_pitch * cos_yaw;
    s.vel[2] =
        (-new_vx * cos_pitch * new_vz - new_vy * sin_pitch) + cos_angle * cos_pitch * cos_yaw;
}

/// `PRIMBLENDRAND`/`ENVBLENDRAND` (`lbparticle.c:1092-1124`). Zero real US
/// ROM scripts use either opcode; this follows the formula as written
/// (`target += byte * rand()`, truncated back into the `u8` channel) without
/// independent on-device verification.
fn blend_rand(
    target: &mut [u8; 4],
    data: &[u8],
    cursor: &mut usize,
    rng: &mut Rng,
) -> Result<(), SimError> {
    for channel in target.iter_mut() {
        let byte = read_u8(data, cursor)? as f32;
        *channel = (*channel as i32 + (byte * rng.next_float()) as i32) as u8;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn bytecode_widths_and_padding_are_identified() {
        let code = [0x81, 0, 0, 0, 0, 0xCF, 0, 1, 2, 3, 4, 0xFF, 0, 0];
        assert_eq!(
            inspect_bytecode(&code),
            Ok(BytecodeSummary {
                instructions: 3,
                used_bytes: 12,
                terminator: 0xFF
            })
        );
    }

    #[test]
    fn long_wait_consumes_timer_and_frame() {
        assert_eq!(
            inspect_bytecode(&[0x60, 0x20, 7, 0xFE]).unwrap().used_bytes,
            4
        );
    }

    #[test]
    fn synthetic_script_bank_decodes_header() {
        let mut data = vec![0u8; 4 + 4 + 0x30 + 4];
        data[3] = 1;
        data[7] = 8;
        data[8 + 3] = 2;
        data[8 + 7] = 9;
        data[8 + 0x30] = 0xFF;
        let scripts = decode_scripts(&data).unwrap();
        assert_eq!(scripts.len(), 1);
        assert_eq!(scripts[0].texture_id, 2);
        assert_eq!(scripts[0].particle_lifetime, 9);
    }

    #[test]
    fn synthetic_texture_bank_decodes_frame() {
        let mut data = vec![0u8; 0x30];
        data[3] = 1; // texture count
        data[7] = 8; // texture header offset
        data[8 + 3] = 1; // frame count
        data[8 + 8 + 3] = 0; // 4bpp
        data[8 + 0x0C + 3] = 2;
        data[8 + 0x10 + 3] = 2;
        data[8 + 0x14 + 3] = 1;
        data[8 + 0x18 + 3] = 0x2C;
        data[0x2C..0x2E].copy_from_slice(&[0x12, 0x34]);
        let textures = decode_textures(&data).unwrap();
        assert_eq!(textures.len(), 1);
        assert_eq!(textures[0].format, Format::Rgba);
        assert_eq!(textures[0].images[0], [0x12, 0x34]);
    }

    #[test]
    fn source_bank_inventory_is_stable() {
        assert_eq!(BANKS.len(), 9);
        assert_eq!(BANKS[0].scripts_hi, BANKS[0].textures_lo);
        assert_eq!(BANKS.last().unwrap().textures_hi, 0xB277B0);
    }

    #[test]
    fn real_rom_has_160_scripts_and_65_textures() {
        let Some(path) = std::env::var_os("SSB64_ROM") else {
            return;
        };
        let rom = std::fs::read(path).unwrap();
        let mut scripts = 0;
        let mut textures = 0;
        for &bank in BANKS {
            let (s, t) = decode_bank(&rom, bank).unwrap();
            scripts += s.len();
            textures += t.len();
        }
        assert_eq!((scripts, textures), (160, 65));
    }

    /// RE-184: locks in the frame-4 (RE-183's own settle point) visibility
    /// census `romtool particles` reports. 148 of 160 real scripts resolve a
    /// texture frame at that instant; the other 12 are explained by name in
    /// `docs/reverse-engineering.md` RE-184, not just counted. A drop in
    /// `visible_count` below 148 without a matching RE update means either a
    /// real regression or an unreviewed decoder change.
    #[test]
    fn real_rom_frame_4_particle_visibility_census_is_148_of_160() {
        let Some(path) = std::env::var_os("SSB64_ROM") else {
            return;
        };
        let rom = std::fs::read(path).unwrap();
        let mut visible_count = 0usize;
        let mut total = 0usize;
        for &bank in BANKS {
            let (scripts, textures) = decode_bank(&rom, bank).unwrap();
            for script in &scripts {
                total += 1;
                let mut particle = Particle::spawn(script);
                let mut rng = Rng::new(1);
                for _ in 0..4 {
                    let _ = particle.tick(&mut rng);
                }
                let frame_count = textures
                    .get(particle.state.texture_id as usize)
                    .map_or(0, |t| t.images.len() as u32);
                if particle.state.visible(frame_count) {
                    visible_count += 1;
                }
            }
        }
        assert_eq!((visible_count, total), (148, 160));
    }

    /// RE-186: locks in the archive-wide combine-mode census `romtool
    /// particles` reports for `lbparticle.c:2057-2099`'s three-way branch.
    /// Among the 148 visible scripts, 90 set `ENVCOLOR` (the already-shipped
    /// `(PRIM-ENV)*TEXEL+ENV` path, `psp/src/meshdraw.rs::draw_particle`) and
    /// zero set `NOISE`, `DITHER` or `ALPHABLEND` — this project's own
    /// declined combine/alpha-compare paths (RE-183's doc comment) are
    /// confirmed unreachable by any real script at its own frame-4 settle
    /// point, not merely unimplemented. A nonzero count here without a
    /// matching RE update means a real script now reaches a declined path.
    #[test]
    fn real_rom_frame_4_combine_mode_census_is_envcolor_90_noise_dither_alphablend_0() {
        let Some(path) = std::env::var_os("SSB64_ROM") else {
            return;
        };
        let rom = std::fs::read(path).unwrap();
        let mut envcolor = 0usize;
        let mut noise = 0usize;
        let mut dither = 0usize;
        let mut alphablend = 0usize;
        let mut visible_count = 0usize;
        for &bank in BANKS {
            let (scripts, textures) = decode_bank(&rom, bank).unwrap();
            for script in &scripts {
                let mut particle = Particle::spawn(script);
                let mut rng = Rng::new(1);
                for _ in 0..4 {
                    let _ = particle.tick(&mut rng);
                }
                let frame_count = textures
                    .get(particle.state.texture_id as usize)
                    .map_or(0, |t| t.images.len() as u32);
                if !particle.state.visible(frame_count) {
                    continue;
                }
                visible_count += 1;
                let flags = particle.state.flags;
                if flags & flag::ENVCOLOR != 0 {
                    envcolor += 1;
                }
                if flags & flag::NOISE != 0 {
                    noise += 1;
                }
                if flags & flag::DITHER != 0 {
                    dither += 1;
                }
                if flags & flag::ALPHABLEND != 0 {
                    alphablend += 1;
                }
            }
        }
        assert_eq!(visible_count, 148);
        assert_eq!((envcolor, noise, dither, alphablend), (90, 0, 0, 0));
    }
}

#[cfg(test)]
mod sim_tests {
    use super::*;

    fn script_with(flags: u32, bytecode: &'static [u8]) -> Script<'static> {
        Script {
            kind: 0,
            texture_id: 0,
            generator_lifetime: 0,
            particle_lifetime: 100,
            flags,
            gravity: 0.0,
            friction: 1.0,
            velocity: [0.0, 0.0, 0.0],
            unknown_20: 0.0,
            unknown_24: 0.0,
            update_rate: 0.0,
            size: 0.0,
            bytecode,
        }
    }

    #[test]
    fn visible_requires_positive_size_and_a_real_frame() {
        let script = script_with(0, &[0xFF]);
        let particle = Particle::spawn(&script);
        assert!(!particle.state.visible(4)); // spawn() defaults size to 0.0
        let mut visible_state = particle.state;
        visible_state.size = 1.0;
        assert!(visible_state.visible(4));
        assert!(!visible_state.visible(0)); // zero-frame texture series
    }

    #[test]
    fn rng_matches_syutilsrandfloat_seed_1() {
        // step = 1*214013 + 2531011 = 2745024; (step >> 16) & 0xFFFF = 41.
        let mut rng = Rng::new(1);
        assert_eq!(rng.next_float(), 41.0 / 65536.0);
    }

    #[test]
    fn setpos_addpos_setvel_addvel_apply_only_selected_axes() {
        // SETPOS(x,z)=[2.0, _, 5.0]; ADDPOS(y)+=1.5; SETVEL(y)=3.0; END.
        let code: &[u8] = &[
            0x85, 0x40, 0x00, 0x00, 0x00, 0x40, 0xA0, 0x00, 0x00, // SETPOS x=2.0 z=5.0
            0x8A, 0x3F, 0xC0, 0x00, 0x00, // ADDPOS y+=1.5
            0x92, 0x40, 0x40, 0x00, 0x00, // SETVEL y=3.0
            0xFF,
        ];
        let script = script_with(0, code);
        let mut particle = Particle::spawn(&script);
        let mut rng = Rng::new(1);
        particle.tick(&mut rng).unwrap();
        assert_eq!(particle.state.pos, [2.0, 1.5, 5.0]);
        assert_eq!(particle.state.vel, [0.0, 3.0, 0.0]);
        assert!(!particle.state.alive); // END fires lifetime->1->0 same frame.
    }

    #[test]
    fn size_lerp_reaches_exact_target_after_exact_frame_count() {
        // SETSIZELERP length=4 target=8.0, then a real wait so each tick
        // only advances the lerp once and doesn't re-enter the opcode.
        let code: &[u8] = &[0xA0, 0x03, 0x41, 0x00, 0x00, 0x00, 0x1F, 0xFF];
        let script = script_with(0, code);
        let mut particle = Particle::spawn(&script);
        let mut rng = Rng::new(1);
        particle.tick(&mut rng).unwrap(); // parses SETSIZELERP + the wait(31).
        assert_eq!(particle.state.size_target_length, 3);
        for _ in 0..3 {
            particle.tick(&mut rng).unwrap();
        }
        assert_eq!(particle.state.size, 8.0);
        assert_eq!(particle.state.size_target_length, 0);
    }

    #[test]
    fn gravity_and_friction_integrate_in_source_order() {
        // SETGRAVITY(0.5); SETFRICTION(0.9); SETVEL y=10; wait(31); END.
        let code: &[u8] = &[
            0xA2, 0x3F, 0x00, 0x00, 0x00, // SETGRAVITY 0.5
            0xA3, 0x3F, 0x66, 0x66, 0x66, // SETFRICTION 0.9
            0x92, 0x41, 0x20, 0x00, 0x00, // SETVEL y=10.0
            0x1F, // wait(31)
            0xFF,
        ];
        let script = script_with(0, code);
        let mut particle = Particle::spawn(&script);
        let mut rng = Rng::new(1);
        particle.tick(&mut rng).unwrap();

        let gravity = 0.5f32;
        let friction = 0.9f32;
        let mut vel_y = 10.0f32;
        vel_y -= gravity; // vel.y -= gravity happens first
        vel_y *= friction; // then vel *= friction
        assert_eq!(particle.state.vel[1], vel_y);
        assert_eq!(particle.state.pos[1], vel_y); // pos starts at 0, += vel once
        assert_ne!(particle.state.flags & flag::GRAVITY, 0);
        assert_ne!(particle.state.flags & flag::FRICTION, 0);
    }

    #[test]
    fn setgravity_zero_clears_the_gravity_flag() {
        let code: &[u8] = &[0xA2, 0x00, 0x00, 0x00, 0x00, 0xFF];
        let script = script_with(flag::GRAVITY, code);
        let mut particle = Particle::spawn(&script);
        let mut rng = Rng::new(1);
        particle.tick(&mut rng).unwrap();
        assert_eq!(particle.state.flags & flag::GRAVITY, 0);
    }

    #[test]
    fn loop_reruns_body_exactly_count_minus_one_times_then_falls_through() {
        // SETLOOP(3); ADDPOS(x)+=1.0; wait(1); LOOP; END.
        let code: &[u8] = &[
            0xFA, 0x03, // SETLOOP count=3, loop_ptr -> byte 2
            0x89, 0x3F, 0x80, 0x00, 0x00, // ADDPOS x += 1.0
            0x01, // wait(1)
            0xFB, // LOOP
            0xFF, // END
        ];
        let script = script_with(0, code);
        let mut particle = Particle::spawn(&script);
        let mut rng = Rng::new(1);

        particle.tick(&mut rng).unwrap(); // SETLOOP, ADDPOS(->1.0), wait breaks
        assert_eq!(particle.state.pos[0], 1.0);
        particle.tick(&mut rng).unwrap(); // LOOP(3->2) jumps back, ADDPOS(->2.0)
        assert_eq!(particle.state.pos[0], 2.0);
        particle.tick(&mut rng).unwrap(); // LOOP(2->1) jumps back, ADDPOS(->3.0)
        assert_eq!(particle.state.pos[0], 3.0);
        assert!(particle.state.alive);
        particle.tick(&mut rng).unwrap(); // LOOP(1->0) falls through into END
        assert_eq!(particle.state.pos[0], 3.0);
        assert!(!particle.state.alive);
    }

    #[test]
    fn setreturn_and_return_jump_back_to_the_saved_cursor() {
        // ADDPOS(x)+=1.0; SETRETURN; ADDPOS(x)+=1.0; wait(1); RETURN; END
        // (never reached: RETURN always jumps back before END).
        let code: &[u8] = &[
            0x89, 0x3F, 0x80, 0x00, 0x00, // ADDPOS x += 1.0
            0xFC, // SETRETURN -> return_ptr points at the next ADDPOS
            0x89, 0x3F, 0x80, 0x00, 0x00, // ADDPOS x += 1.0
            0x01, // wait(1)
            0xFD, // RETURN
            0xFF, // END (unreachable in this trace)
        ];
        let script = script_with(0, code);
        let mut particle = Particle::spawn(&script);
        let mut rng = Rng::new(1);

        particle.tick(&mut rng).unwrap();
        assert_eq!(particle.state.pos[0], 2.0);
        particle.tick(&mut rng).unwrap();
        assert_eq!(particle.state.pos[0], 3.0);
        assert!(particle.state.alive);
    }

    #[test]
    fn makescript_and_makeid_decode_as_spawn_requests_without_executing() {
        // MAKESCRIPT(0x0007); MAKEID(0x0003); END.
        let code: &[u8] = &[0xA4, 0x00, 0x07, 0xB9, 0x00, 0x03, 0xFF];
        let script = script_with(0, code);
        let mut particle = Particle::spawn(&script);
        let mut rng = Rng::new(1);
        let spawns = particle.tick(&mut rng).unwrap();
        assert_eq!(
            spawns,
            [
                SpawnRequest {
                    script_id: 7,
                    inherit_velocity: false,
                    is_generator: false
                },
                SpawnRequest {
                    script_id: 3,
                    inherit_velocity: true,
                    is_generator: false
                },
            ]
        );
        // No child was actually created: this particle's own state is all
        // that changed (it died from the trailing END).
        assert!(!particle.state.alive);
    }

    #[test]
    fn makegenerator_decodes_as_a_generator_spawn_request() {
        let code: &[u8] = &[0xA5, 0x00, 0x2A, 0xFF];
        let script = script_with(0, code);
        let mut particle = Particle::spawn(&script);
        let mut rng = Rng::new(1);
        let spawns = particle.tick(&mut rng).unwrap();
        assert_eq!(
            spawns,
            [SpawnRequest {
                script_id: 0x2A,
                inherit_velocity: false,
                is_generator: true
            }]
        );
    }

    #[test]
    fn setvelangle_preserves_speed() {
        // SETVELANGLE 0.5 rad; wait(1); END. The starting velocity comes
        // from the script header, exactly as a real spawn provides it.
        let code: &[u8] = &[0xA9, 0x3F, 0x00, 0x00, 0x00, 0x01, 0xFF];
        let mut script = script_with(0, code);
        script.velocity = [1.0, 2.0, 3.0];
        let mut particle = Particle::spawn(&script);
        let mut rng = Rng::new(1);
        let before = particle.state.vel;
        let before_mag =
            sqrt(before[0] * before[0] + before[1] * before[1] + before[2] * before[2]);
        particle.tick(&mut rng).unwrap();
        let after = particle.state.vel;
        let after_mag = sqrt(after[0] * after[0] + after[1] * after[1] + after[2] * after[2]);
        assert!(
            (before_mag - after_mag).abs() < 1e-3,
            "rotation must preserve speed: {before_mag} vs {after_mag}"
        );
    }

    #[test]
    fn vortex_flag_reports_unsupported_instead_of_guessing() {
        // wait(1); no opcode runs this frame, but the physics tail still
        // checks the flag every frame regardless of what bytecode did.
        let vortex_script = script_with(flag::VORTEX, &[0x01]);
        let mut vortex = Particle::spawn(&vortex_script);
        let mut rng = Rng::new(1);
        assert_eq!(vortex.tick(&mut rng), Err(SimError::VortexUnsupported));
    }
}
