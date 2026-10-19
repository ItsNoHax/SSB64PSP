//! The decomp's small `d*` data arrays (D-048): each one array of a single
//! scalar type, flat or two-dimensional, or one field of an array of
//! records, read at its symbol's run-time address in the overlay its file's
//! `.data` sits in (`refs/ssb-decomp-re/smashbrothers.us.yaml`).
//!
//! What is committed is the layout ([`DArray`]): the symbol and its address
//! (`refs/ssb-decomp-re/symbols/symbols_us.txt`), the overlay, the element
//! type and the shape. The generated constants keep the symbol's name in
//! `UPPER_SNAKE` without the `d`: `dIFCommonPlayerDamageDigitWidths` is
//! `IF_COMMON_PLAYER_DAMAGE_DIGIT_WIDTHS`, written into `OUT_DIR` as
//! `d_arrays.rs` for `ssb-game` ([`game_a`], [`game_b`]), `ssb-rom`
//! ([`rom`]) and `ssb-engine` ([`engine`]).
//!
//! Arrays declared inside a decompiled function ([`DArray::local`]) are
//! ROM tables too: IDO compiles each initialised local aggregate into an
//! anonymous template in its file's `.data`, which the function copies.
//! They have no symbol, so `tools/local_arrays.py` finds each one's address
//! (source-order layout fitted on the ROM) and prints the list entries;
//! [`local`] holds one list file per decomp file. Their constants are named
//! `<FUNCTION>_<VARIABLE>` (`mnVSResultsMakeString`'s `widths` is
//! `MN_VS_RESULTS_MAKE_STRING_WIDTHS`).

use std::collections::HashSet;
use std::fmt::Write;

use crate::emit::{self, f32_lit};
use crate::source::{Overlay, OverlayData};
use crate::{Generated, Result, Source};

mod engine;
mod game_a;
mod game_b;
pub(crate) mod local;
mod rom;

/// An element type and how it is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Elem {
    U8,
    U16,
    U32,
    I32,
    /// A `u32` written in hex (file offsets, IDs read as addresses).
    X32,
    F32,
    /// A `char *` to a NUL-terminated string in the same overlay (its
    /// `.rodata`), written as `&'static str`; `""` for a `NULL` and in the
    /// stub tables.
    CStr,
}

impl Elem {
    fn size(self) -> u32 {
        match self {
            Elem::U8 => 1,
            Elem::U16 => 2,
            Elem::U32 | Elem::I32 | Elem::X32 | Elem::F32 | Elem::CStr => 4,
        }
    }

    fn rust(self) -> &'static str {
        match self {
            Elem::U8 => "u8",
            Elem::U16 => "u16",
            Elem::U32 | Elem::X32 => "u32",
            Elem::I32 => "i32",
            Elem::F32 => "f32",
            Elem::CStr => "&str",
        }
    }

    fn read(self, d: &OverlayData, vram: u32) -> Result<String> {
        Ok(match self {
            Elem::U8 => d.u8(vram)?.to_string(),
            Elem::U16 => d.u16(vram)?.to_string(),
            Elem::U32 => d.u32(vram)?.to_string(),
            Elem::I32 => d.i32(vram)?.to_string(),
            Elem::X32 => format!("0x{:X}", d.u32(vram)?),
            Elem::F32 => f32_lit(d.f32(vram)?),
            Elem::CStr => {
                if d.rom.is_none() {
                    return Ok("\"\"".to_string());
                }
                let mut at = d.u32(vram)?;
                if at == 0 {
                    return Ok("\"\"".to_string());
                }
                let mut text = String::new();
                loop {
                    let b = d.u8(at)?;
                    if b == 0 {
                        break;
                    }
                    assert!(
                        b.is_ascii() && !b.is_ascii_control(),
                        "0x{vram:08X} points at a byte 0x{b:02X} that is not printable ASCII"
                    );
                    text.push(b as char);
                    at += 1;
                }
                format!("{text:?}")
            }
        })
    }
}

/// Where one array is and its shape.
#[derive(Debug, Clone, Copy)]
pub struct DArray {
    /// The generated constant.
    pub name: &'static str,
    /// The decomp symbol (and field) it is read from, for the doc comment;
    /// for a [`DArray::local`], the function and the variable.
    pub symbol: &'static str,
    pub overlay: Overlay,
    /// The first element's run-time address: the symbol's, plus the field's
    /// offset for a field of a record array.
    pub vram: u32,
    pub elem: Elem,
    /// Dimensions, outermost first: `&[10]` is `[T; 10]`, `&[5, 3]` is
    /// `[[T; 3]; 5]`.
    pub dims: &'static [usize],
    /// Bytes from one outermost row to the next; 0 when the rows are packed
    /// (the record size for a field of a record array).
    pub stride: u32,
    /// Declared inside a function (no symbol of its own).
    pub local: bool,
}

impl DArray {
    /// A packed array.
    pub const fn new(
        name: &'static str,
        symbol: &'static str,
        overlay: Overlay,
        vram: u32,
        elem: Elem,
        dims: &'static [usize],
    ) -> Self {
        Self {
            name,
            symbol,
            overlay,
            vram,
            elem,
            dims,
            stride: 0,
            local: false,
        }
    }

    /// An array declared inside a decompiled function: `function` is
    /// `"<function> <variable>"`, `vram` the address `tools/local_arrays.py`
    /// prints for it.
    pub const fn local(
        name: &'static str,
        function: &'static str,
        overlay: Overlay,
        vram: u32,
        elem: Elem,
        dims: &'static [usize],
    ) -> Self {
        Self {
            local: true,
            ..Self::new(name, function, overlay, vram, elem, dims)
        }
    }

    /// The same array read with `stride` bytes between outermost rows.
    pub const fn stride(self, stride: u32) -> Self {
        Self { stride, ..self }
    }

    fn rust_type(&self) -> String {
        let mut ty = self.elem.rust().to_string();
        for n in self.dims.iter().rev() {
            ty = format!("[{ty}; {n}]");
        }
        ty
    }

    /// A one-dimensional array of 4-byte elements as raw words, for a
    /// generator that indexes it; zeros for a stub.
    pub fn words(&self, rom: Option<&Source>) -> Result<Vec<u32>> {
        assert!(
            self.dims.len() == 1 && self.elem.size() == 4,
            "{} is not a 1-D word array",
            self.name
        );
        let d = OverlayData::new(rom, self.overlay);
        (0..self.dims[0] as u32)
            .map(|i| d.u32(self.vram + i * self.row_bytes()))
            .collect()
    }

    /// Bytes per outermost row.
    fn row_bytes(&self) -> u32 {
        if self.stride != 0 {
            self.stride
        } else {
            self.dims[1..].iter().product::<usize>() as u32 * self.elem.size()
        }
    }

    fn value(&self, d: &OverlayData, vram: u32, dims: &[usize]) -> Result<String> {
        let Some((&n, rest)) = dims.split_first() else {
            return self.elem.read(d, vram);
        };
        let step = if dims.len() == self.dims.len() {
            self.row_bytes()
        } else {
            rest.iter().product::<usize>() as u32 * self.elem.size()
        };
        let cells: Vec<String> = (0..n as u32)
            .map(|i| self.value(d, vram + i * step, rest))
            .collect::<Result<_>>()?;
        Ok(format!("[{}]", cells.join(", ")))
    }

    fn emit(&self, rom: Option<&Source>, out: &mut String) -> Result<()> {
        let d = OverlayData::new(rom, self.overlay);
        // The last element lies in the overlay, even for a stub.
        let last = self.vram
            + (self.dims[0] as u32 - 1) * self.row_bytes()
            + (self.dims[1..].iter().product::<usize>() as u32 - 1) * self.elem.size();
        d.bytes(last, self.elem.size())?;
        let value = self.value(&d, self.vram, self.dims)?;
        let doc = match self.symbol.split_once(' ') {
            Some((function, var)) if self.local => {
                format!("`{function}`'s local `{var}` (at 0x{:08X}).", self.vram)
            }
            _ => format!("`{}`.", self.symbol),
        };
        let _ = writeln!(
            out,
            "/// {doc}\npub const {}: {} = {value};",
            self.name,
            self.rust_type()
        );
        Ok(())
    }
}

fn file(what: &str, lists: &[&[DArray]], rom: Option<&Source>) -> Result<String> {
    let mut out = emit::header(what);
    let mut seen = HashSet::new();
    for a in lists.iter().flat_map(|l| l.iter()) {
        assert!(seen.insert(a.name), "duplicate d_arrays name {}", a.name);
        assert!(!a.dims.is_empty(), "{} has no dimensions", a.name);
        a.emit(rom, &mut out)?;
    }
    Ok(out)
}

/// `ssb-game`'s `d_arrays.rs`.
pub fn generate_game(rom: Option<&Source>) -> Result<Generated> {
    Ok(Generated {
        name: "d_arrays.rs",
        text: file(
            "the decomp's small `d*` arrays",
            &[&[game_a::LIST, game_b::LIST], local::GAME].concat(),
            rom,
        )?,
    })
}

/// `ssb-rom`'s `d_arrays.rs`.
pub fn generate_rom(rom: Option<&Source>) -> Result<Generated> {
    Ok(Generated {
        name: "d_arrays.rs",
        text: file(
            "the decomp's small `d*` arrays",
            &[&[rom::LIST], local::ROM].concat(),
            rom,
        )?,
    })
}

/// `ssb-engine`'s `d_arrays.rs`.
pub fn generate_engine(rom: Option<&Source>) -> Result<Generated> {
    Ok(Generated {
        name: "d_arrays.rs",
        text: file("the decomp's small `d*` arrays", &[engine::LIST], rom)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::OVL2;

    #[test]
    fn shapes_and_strides() {
        let a = DArray::new("A", "dA", OVL2, 0x8012_E870, Elem::U8, &[2, 3]);
        assert_eq!(a.rust_type(), "[[u8; 3]; 2]");
        assert_eq!(a.row_bytes(), 3);
        let b = DArray::new("B", "dB", OVL2, 0x8012_E870, Elem::F32, &[4]).stride(12);
        assert_eq!(b.row_bytes(), 12);
        let mut out = String::new();
        a.emit(None, &mut out).unwrap();
        assert!(out.contains("pub const A: [[u8; 3]; 2] = [[0, 0, 0], [0, 0, 0]];"));
        let mut out = String::new();
        b.emit(None, &mut out).unwrap();
        assert!(out.contains("pub const B: [f32; 4] = [0.0, 0.0, 0.0, 0.0];"));
    }

    #[test]
    fn local_doc_and_string_stub() {
        let a = DArray::local("S", "fnName names", OVL2, 0x8012_E870, Elem::CStr, &[2]);
        let mut out = String::new();
        a.emit(None, &mut out).unwrap();
        assert_eq!(
            out,
            "/// `fnName`'s local `names` (at 0x8012E870).\n\
             pub const S: [&str; 2] = [\"\", \"\"];\n"
        );
    }

    #[test]
    fn every_list_emits_its_stub() {
        let game = generate_game(None).unwrap().text;
        let rom = generate_rom(None).unwrap().text;
        let n = |t: &str| t.matches("pub const ").count();
        let locals = |l: &[&[DArray]]| l.iter().map(|l| l.len()).sum::<usize>();
        assert_eq!(
            n(&game),
            game_a::LIST.len() + game_b::LIST.len() + locals(local::GAME)
        );
        assert_eq!(n(&rom), rom::LIST.len() + locals(local::ROM));
        let engine = generate_engine(None).unwrap().text;
        assert_eq!(n(&engine), engine::LIST.len());
    }
}
