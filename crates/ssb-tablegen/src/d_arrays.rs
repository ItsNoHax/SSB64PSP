//! The decomp's small `d*` data arrays (D-048): each one array of a single
//! scalar type, flat or two-dimensional, or one field of an array of
//! records, read at its symbol's run-time address in the overlay its file's
//! `.data` sits in (`refs/ssb-decomp-re/smashbrothers.us.yaml`).
//!
//! What is committed is the layout ([`DArray`]): the symbol and its address
//! (`refs/ssb-decomp-re/symbols/symbols_us.txt`), the overlay, the element
//! type and the shape. The values go to the pack's `TABLES` section
//! ([`crate::tables`]): one group table per element kind ([`Group`]: every
//! array of that kind, `ssb-game`'s then `ssb-rom`'s, each 4-byte aligned)
//! and the strings' blob. Each crate's `d_arrays.rs` in `OUT_DIR` holds
//! only the layout ([`layout_rs`]): a `ssb_tables::DArr` view per array at
//! its group offset, named after the symbol in `UPPER_SNAKE` without the
//! `d` (`dIFCommonPlayerDamageDigitWidths` is
//! `IF_COMMON_PLAYER_DAMAGE_DIGIT_WIDTHS`), for `ssb-game` ([`game_a`],
//! [`game_b`]) and `ssb-rom` ([`rom`]). `ssb-engine`'s ([`engine`]) are
//! tables of their own.
//!
//! Arrays declared inside a decompiled function ([`DArray::local`]) are
//! ROM tables too: IDO compiles each initialised local aggregate into an
//! anonymous template in its file's `.data`, which the function copies.
//! They have no symbol, so `tools/local_arrays.py` finds each one's address
//! (source-order layout fitted on the ROM) and prints the list entries;
//! [`local`] holds one list file per decomp file. Their constants are named
//! `<FUNCTION>_<VARIABLE>` (`mnVSResultsMakeString`'s `widths` is
//! `MN_VS_RESULTS_MAKE_STRING_WIDTHS`).

#[cfg(feature = "std")]
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;
#[cfg(feature = "std")]
use core::fmt::Write;

use crate::source::Overlay;
use crate::source::OverlayData;
#[cfg(feature = "std")]
use crate::Generated;
use crate::{Result, Source};

mod engine;
mod game_a;
mod game_b;
pub(crate) mod local;
mod rom;

/// An element type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Elem {
    U8,
    U16,
    U32,
    I32,
    /// A `u32` the code reads as an address or offset (a `u32`).
    X32,
    F32,
    /// A `char *` to a NUL-terminated string in the same overlay (its
    /// `.rodata`): its offset in the strings' blob (`DArrStr`), read as
    /// `&'static str` (`ssb_tables::Strs`); `""` for a `NULL` and in the
    /// stub.
    CStr,
}

/// The group table an array's elements go to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    U8,
    U16,
    /// `u32`, the addresses, and the strings' offsets.
    U32,
    I32,
    F32,
}

impl Group {
    /// The group's `TableId` variant.
    #[cfg(feature = "std")]
    pub const fn table(self) -> &'static str {
        match self {
            Group::U8 => "DArrU8",
            Group::U16 => "DArrU16",
            Group::U32 => "DArrU32",
            Group::I32 => "DArrI32",
            Group::F32 => "DArrF32",
        }
    }
}

impl Elem {
    const fn size(self) -> u32 {
        match self {
            Elem::U8 => 1,
            Elem::U16 => 2,
            Elem::U32 | Elem::I32 | Elem::X32 | Elem::F32 | Elem::CStr => 4,
        }
    }

    const fn group(self) -> Group {
        match self {
            Elem::U8 => Group::U8,
            Elem::U16 => Group::U16,
            Elem::U32 | Elem::X32 | Elem::CStr => Group::U32,
            Elem::I32 => Group::I32,
            Elem::F32 => Group::F32,
        }
    }

    #[cfg(feature = "std")]
    fn rust(self) -> &'static str {
        match self {
            Elem::U8 => "u8",
            Elem::U16 => "u16",
            Elem::U32 | Elem::X32 | Elem::CStr => "u32",
            Elem::I32 => "i32",
            Elem::F32 => "f32",
        }
    }
}

/// Where one array is and its shape.
#[derive(Debug, Clone, Copy)]
#[cfg_attr(not(feature = "std"), allow(dead_code))]
pub struct DArray {
    /// The generated view.
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

    /// Elements.
    const fn count(&self) -> usize {
        let mut n = 1;
        let mut i = 0;
        while i < self.dims.len() {
            n *= self.dims[i];
            i += 1;
        }
        n
    }

    /// Bytes in its group table.
    const fn bytes(&self) -> usize {
        self.count() * self.elem.size() as usize
    }

    #[cfg(feature = "std")]
    fn rust_type(&self) -> String {
        let mut ty = String::from(self.elem.rust());
        for n in self.dims.iter().rev() {
            ty = alloc::format!("[{ty}; {n}]");
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

    /// Each element's run-time address, in row-major order.
    fn addresses(&self) -> Vec<u32> {
        let inner = self.dims[1..].iter().product::<usize>() as u32;
        (0..self.dims[0] as u32)
            .flat_map(|r| (0..inner).map(move |i| (r, i)))
            .map(|(r, i)| self.vram + r * self.row_bytes() + i * self.elem.size())
            .collect()
    }
}

/// Every array, `ssb-game`'s ([`game_a`], [`game_b`], [`local::GAME`]) then
/// `ssb-rom`'s ([`rom`], [`local::ROM`]): the group tables' order.
const SETS: [&[&[DArray]]; 4] = [
    &[game_a::LIST, game_b::LIST],
    local::GAME,
    &[rom::LIST],
    local::ROM,
];
/// [`SETS`] that are `ssb-game`'s; the rest are `ssb-rom`'s.
#[cfg(feature = "std")]
const GAME_SETS: usize = 2;

/// The bytes of `group`'s table: its arrays in [`SETS`] order, each at a
/// 4-byte aligned offset (as [`layout`] places them).
const fn walk(group: Group) -> usize {
    let mut at = 0;
    let mut s = 0;
    while s < SETS.len() {
        let mut l = 0;
        while l < SETS[s].len() {
            let list = SETS[s][l];
            let mut i = 0;
            while i < list.len() {
                let a = &list[i];
                if a.elem.group() as u8 == group as u8 {
                    at = (at + a.bytes()).next_multiple_of(4);
                }
                i += 1;
            }
            l += 1;
        }
        s += 1;
    }
    at
}

/// The bytes of `group`'s table (layout).
pub const fn group_bytes(group: Group) -> usize {
    walk(group)
}

/// Every array with its set index and its offset in its group table.
fn layout() -> Vec<(usize, &'static DArray, usize)> {
    let mut at = [0usize; 5];
    let mut out = Vec::new();
    for (s, set) in SETS.iter().enumerate() {
        for a in set.iter().flat_map(|l| l.iter()) {
            assert!(!a.dims.is_empty(), "{} has no dimensions", a.name);
            let g = a.elem.group() as usize;
            out.push((s, a, at[g]));
            at[g] = (at[g] + a.bytes()).next_multiple_of(4);
        }
    }
    out
}

/// `group`'s table read from `rom`: each array's elements, little-endian,
/// at its offset; a string array's elements are its strings' offsets in
/// [`strings`]'s blob.
pub fn group(group: Group, rom: &Source) -> Result<Vec<u8>> {
    let mut out = vec![0u8; group_bytes(group)];
    let mut blob = Blob::default();
    for (_, a, at) in layout() {
        if a.elem.group() != group && a.elem != Elem::CStr {
            continue;
        }
        let d = OverlayData::new(Some(rom), a.overlay);
        let mut cell = at;
        for vram in a.addresses() {
            let n = a.elem.size() as usize;
            let bytes: [u8; 4] = match a.elem {
                Elem::U8 => [d.u8(vram)?, 0, 0, 0],
                Elem::U16 => {
                    let v = d.u16(vram)?.to_le_bytes();
                    [v[0], v[1], 0, 0]
                }
                Elem::U32 | Elem::X32 | Elem::I32 | Elem::F32 => d.u32(vram)?.to_le_bytes(),
                Elem::CStr => blob.add(&d, d.u32(vram)?)?.to_le_bytes(),
            };
            if a.elem.group() == group {
                out[cell..cell + n].copy_from_slice(&bytes[..n]);
            }
            cell += n;
        }
    }
    Ok(out)
}

/// The strings' blob (`DArrStr`): a NUL (a `NULL`'s string), then each
/// string array's strings, NUL-terminated, in [`group`]'s order.
pub fn strings(rom: &Source) -> Result<Vec<u8>> {
    let mut blob = Blob::default();
    for (_, a, _) in layout()
        .into_iter()
        .filter(|(_, a, _)| a.elem == Elem::CStr)
    {
        let d = OverlayData::new(Some(rom), a.overlay);
        for vram in a.addresses() {
            blob.add(&d, d.u32(vram)?)?;
        }
    }
    Ok(blob.bytes())
}

/// The strings' blob as [`group`] and [`strings`] build it.
#[derive(Default)]
struct Blob(Vec<u8>);

impl Blob {
    /// Appends the string at `at` (`0`: `NULL`, offset 0); returns its
    /// offset.
    fn add(&mut self, d: &OverlayData, mut at: u32) -> Result<u32> {
        if self.0.is_empty() {
            self.0.push(0);
        }
        if at == 0 {
            return Ok(0);
        }
        let start = self.0.len() as u32;
        loop {
            let b = d.u8(at)?;
            if b == 0 {
                break;
            }
            assert!(
                b.is_ascii() && !b.is_ascii_control(),
                "0x{at:08X} is a byte 0x{b:02X} that is not printable ASCII"
            );
            self.0.push(b);
            at += 1;
        }
        self.0.push(0);
        Ok(start)
    }

    fn bytes(mut self) -> Vec<u8> {
        if self.0.is_empty() {
            self.0.push(0);
        }
        self.0
    }
}

/// A crate's `d_arrays.rs`: its arrays' views (layout only, no ROM data).
#[cfg(feature = "std")]
fn layout_rs(game: bool) -> String {
    let mut out = String::from(
        "// Generated by crates/ssb-tablegen (d_arrays): the views of the decomp's\n\
         // small `d*` arrays in the pack's TABLES section. Layout only; never edit.\n\n",
    );
    for (s, a, at) in layout() {
        let viewed = !game && local::ROM_VIEWS.contains(&a.name);
        if (s < GAME_SETS) != game && !viewed {
            continue;
        }
        let doc = match a.symbol.split_once(' ') {
            Some((function, var)) if a.local => {
                alloc::format!("`{function}`'s local `{var}` (at 0x{:08X}).", a.vram)
            }
            _ => alloc::format!("`{}`.", a.symbol),
        };
        let table = a.elem.group().table();
        let _ = if a.elem == Elem::CStr {
            assert_eq!(a.dims.len(), 1, "{}: a string array is 1-D", a.name);
            writeln!(
                out,
                "/// {doc}\npub static {}: ssb_tables::Strs<{}> = ssb_tables::Strs::new(ssb_tables::TableId::{table}, {at}, ssb_tables::TableId::DArrStr);",
                a.name, a.dims[0]
            )
        } else {
            writeln!(
                out,
                "/// {doc}\npub static {}: ssb_tables::DArr<{}> = ssb_tables::DArr::new(ssb_tables::TableId::{table}, {at});",
                a.name,
                a.rust_type()
            )
        };
    }
    out
}

/// `ssb-game`'s `d_arrays.rs`.
#[cfg(feature = "std")]
pub fn generate_game() -> Generated {
    Generated {
        name: "d_arrays.rs",
        text: layout_rs(true),
    }
}

/// `ssb-rom`'s `d_arrays.rs`.
#[cfg(feature = "std")]
pub fn generate_rom() -> Generated {
    Generated {
        name: "d_arrays.rs",
        text: layout_rs(false),
    }
}

/// `ssb-engine`'s array `name` ([`engine::LIST`]) as raw words, for the
/// pack's `TABLES` section ([`crate::tables`]); zeros for a stub.
pub fn engine_words(name: &str, rom: Option<&Source>) -> Result<Vec<u32>> {
    list_words(engine::LIST, name, rom)
}

/// `ssb-rom`'s array `name` ([`rom::LIST`]) as raw words, for a table
/// read from it ([`crate::ground`]).
pub fn rom_words(name: &str, rom: &Source) -> Result<Vec<u32>> {
    list_words(rom::LIST, name, Some(rom))
}

fn list_words(list: &[DArray], name: &str, rom: Option<&Source>) -> Result<Vec<u32>> {
    let a = list
        .iter()
        .find(|a| a.name == name)
        .unwrap_or_else(|| panic!("no d_array {name}"));
    let d = OverlayData::new(rom, a.overlay);
    // The last element lies in the overlay, even for a stub.
    d.bytes(
        a.vram + (a.dims[0] as u32 - 1) * a.row_bytes(),
        a.elem.size(),
    )?;
    a.words(rom)
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;
    use crate::source::OVL2;
    use std::collections::HashSet;

    #[test]
    fn shapes_strides_and_addresses() {
        let a = DArray::new("A", "dA", OVL2, 0x8012_E870, Elem::U8, &[2, 3]);
        assert_eq!(a.rust_type(), "[[u8; 3]; 2]");
        assert_eq!(a.row_bytes(), 3);
        assert_eq!(a.bytes(), 6);
        let b = DArray::new("B", "dB", OVL2, 0x8012_E870, Elem::F32, &[4, 2]).stride(12);
        assert_eq!(b.row_bytes(), 12);
        assert_eq!(
            b.addresses(),
            [0, 4, 12, 16, 24, 28, 36, 40].map(|o| 0x8012_E870 + o)
        );
    }

    #[test]
    fn every_array_has_a_distinct_name_and_lies_in_its_overlay() {
        // Distinct within each crate's module (`ssb-game`'s sets, then
        // `ssb-rom`'s, with its `ROM_VIEWS`); both crates may view one array.
        let mut seen = HashSet::new();
        for (s, a, _) in layout() {
            let krate = s < GAME_SETS;
            assert!(
                seen.insert((krate, a.name)),
                "duplicate d_arrays name {}",
                a.name
            );
            if krate && local::ROM_VIEWS.contains(&a.name) {
                assert!(
                    seen.insert((false, a.name)),
                    "{} is viewed and listed in ssb-rom",
                    a.name
                );
            }
            let d = OverlayData::new(None, a.overlay);
            let last = *a.addresses().last().unwrap();
            d.bytes(last, a.elem.size()).unwrap();
        }
    }

    #[test]
    fn the_views_fill_each_group_in_order() {
        let game = generate_game().text;
        let rom = generate_rom().text;
        let n = |t: &str| t.matches("pub static ").count();
        let lists = |sets: &[&[&[DArray]]]| -> usize {
            sets.iter().flat_map(|s| s.iter()).map(|l| l.len()).sum()
        };
        assert_eq!(n(&game), lists(&SETS[..GAME_SETS]));
        assert_eq!(n(&rom), lists(&SETS[GAME_SETS..]) + local::ROM_VIEWS.len());
        for g in [Group::U8, Group::U16, Group::U32, Group::I32, Group::F32] {
            let end = layout()
                .iter()
                .filter(|(_, a, _)| a.elem.group() == g)
                .map(|(_, a, at)| (at + a.bytes()).next_multiple_of(4))
                .max()
                .unwrap_or(0);
            assert_eq!(end, group_bytes(g), "{g:?}");
        }
        let engine = engine_words("LB_COMMON_SIN_LOOKUP", None).unwrap();
        assert_eq!(engine, vec![0; 1024]);
    }
}
