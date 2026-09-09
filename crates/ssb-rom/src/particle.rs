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
}
