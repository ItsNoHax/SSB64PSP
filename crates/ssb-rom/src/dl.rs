//! F3DEX2 display list parsing.
//!
//! Smash 64's mesh geometry is not authored as a mesh format — it is stored in
//! the ROM as **ready-made F3DEX2 display lists**. `objdisplay.c` only wraps
//! them: it pushes matrices, sets material state, then `gSPDisplayList`s
//! straight into ROM data. So the way to get geometry out of this game is to
//! parse the display lists themselves.
//!
//! We are *not* emulating the RDP. This parser walks a display list and
//! produces a neutral command stream that the build-time converter lowers into
//! PSP vertex/index buffers and `sceGu` state (`docs/rendering.md`).
//!
//! Opcode values are taken from the decompilation's `include/PR/gbi.h`
//! (F3DEX2 branch — `taskman.c` registers `gspF3DEX2_fifo`). Note F3DEX2
//! renumbers the SP opcodes relative to F3DEX: `G_VTX` is `0x01`, not `0x04`.

use alloc::vec::Vec;

use crate::{Error, Result};

/// Bytes per display list command. Every F3DEX2 command is one 64-bit word.
pub const CMD_SIZE: usize = 8;

// ---- SP opcodes -----------------------------------------------------------
pub const G_NOOP: u8 = 0x00;
pub const G_VTX: u8 = 0x01;
pub const G_MODIFYVTX: u8 = 0x02;
pub const G_CULLDL: u8 = 0x03;
pub const G_BRANCH_Z: u8 = 0x04;
pub const G_TRI1: u8 = 0x05;
pub const G_TRI2: u8 = 0x06;
pub const G_QUAD: u8 = 0x07;
pub const G_LINE3D: u8 = 0x08;
pub const G_TEXTURE: u8 = 0xD7;
pub const G_POPMTX: u8 = 0xD8;
pub const G_GEOMETRYMODE: u8 = 0xD9;
pub const G_MTX: u8 = 0xDA;
pub const G_MOVEWORD: u8 = 0xDB;
pub const G_MOVEMEM: u8 = 0xDC;
pub const G_DL: u8 = 0xDE;
pub const G_ENDDL: u8 = 0xDF;
pub const G_SPNOOP: u8 = 0xE0;

// ---- DP opcodes -----------------------------------------------------------
pub const G_SETOTHERMODE_L: u8 = 0xE2;
pub const G_SETOTHERMODE_H: u8 = 0xE3;
pub const G_TEXRECT: u8 = 0xE4;
pub const G_RDPLOADSYNC: u8 = 0xE6;
pub const G_RDPPIPESYNC: u8 = 0xE7;
pub const G_RDPTILESYNC: u8 = 0xE8;
pub const G_RDPFULLSYNC: u8 = 0xE9;
pub const G_SETSCISSOR: u8 = 0xED;
pub const G_LOADTLUT: u8 = 0xF0;
pub const G_SETTILESIZE: u8 = 0xF2;
pub const G_LOADBLOCK: u8 = 0xF3;
pub const G_LOADTILE: u8 = 0xF4;
pub const G_SETTILE: u8 = 0xF5;
pub const G_FILLRECT: u8 = 0xF6;
pub const G_SETFOGCOLOR: u8 = 0xF8;
pub const G_SETBLENDCOLOR: u8 = 0xF9;
pub const G_SETPRIMCOLOR: u8 = 0xFA;
pub const G_SETENVCOLOR: u8 = 0xFB;
pub const G_SETCOMBINE: u8 = 0xFC;
pub const G_SETTIMG: u8 = 0xFD;
/// Sets the RDP's output colour image (render target). Real, legitimate
/// F3DEX2 opcode, but Smash's per-object display lists never emit it — only
/// the graphics-task setup code that frames the whole scene does. Seeing it
/// while walking an object's mesh list is a strong sign the walk has left
/// real display-list data (see `Cmd::Other`'s doc comment, RE-283).
pub const G_SETCIMG: u8 = 0xFF;

/// An N64 segmented address: the top byte selects a segment, the rest is an
/// offset into it. Resolving one needs the segment table that was live at the
/// time, which the caller supplies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SegAddr(pub u32);

impl SegAddr {
    pub fn segment(self) -> u8 {
        (self.0 >> 24) as u8
    }
    pub fn offset(self) -> u32 {
        self.0 & 0x00FF_FFFF
    }
}

/// One decoded display list command.
///
/// Only the commands Smash actually uses are broken out; everything else is
/// preserved verbatim as `Other` so a converter can decide whether it matters
/// rather than silently dropping it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cmd {
    /// Load `count` vertices into the vertex cache starting at index
    /// `dest_index`, from `addr`.
    Vtx {
        count: u8,
        dest_index: u8,
        addr: SegAddr,
    },
    /// One triangle, by vertex-cache index.
    Tri1([u8; 3]),
    /// Two triangles in one command.
    Tri2([u8; 3], [u8; 3]),
    /// Call another display list.
    Call(SegAddr),
    /// Jump to another display list without returning.
    Branch(SegAddr),
    /// End of this display list.
    End,
    /// Load a matrix.
    Mtx {
        params: u8,
        addr: SegAddr,
    },
    /// Pop `count` matrices off the modelview stack.
    PopMtx {
        count: u32,
    },
    /// Set geometry mode bits (lighting, culling, fog, ...).
    GeometryMode {
        clear: u32,
        set: u32,
    },
    /// Texture scale/enable.
    Texture {
        level: u8,
        tile: u8,
        on: bool,
        scale_s: u16,
        scale_t: u16,
    },
    /// Set the texture image source address.
    SetTimg {
        format: u8,
        size: u8,
        width: u16,
        addr: SegAddr,
        /// File-relative byte offset of the address word itself.
        ///
        /// Needed because the address may be *absent*: the archive leaves a
        /// pointer into another file zeroed and records it as an extern
        /// relocation instead, which is keyed by this offset. Without it there
        /// is no way to tell "no texture" from "the texture is over there"
        /// (RE-037). Zero when the list was decoded without a base offset.
        slot: u32,
    },
    SetTile {
        format: u8,
        size: u8,
        line: u16,
        tmem: u16,
        tile: u8,
        palette: u8,
        cm_s: u8,
        cm_t: u8,
        mask_s: u8,
        mask_t: u8,
        shift_s: u8,
        shift_t: u8,
    },
    /// Tile bounds, in 10.2 fixed point.
    SetTileSize {
        tile: u8,
        uls: u16,
        ult: u16,
        lrs: u16,
        lrt: u16,
    },
    LoadBlock {
        tile: u8,
        uls: u16,
        ult: u16,
        lrs: u16,
        dxt: u16,
    },
    LoadTlut {
        tile: u8,
        count: u16,
    },
    SetPrimColor {
        m: u8,
        l: u8,
        rgba: [u8; 4],
    },
    SetEnvColor([u8; 4]),
    SetBlendColor([u8; 4]),
    SetFogColor([u8; 4]),
    SetCombine {
        hi: u32,
        lo: u32,
    },
    SetOtherModeH {
        shift: u8,
        len: u8,
        data: u32,
    },
    SetOtherModeL {
        shift: u8,
        len: u8,
        data: u32,
    },
    /// A sync or no-op with no effect on geometry conversion.
    Sync(u8),
    /// `G_MOVEWORD`: writes one 32-bit RSP data word (light colours/
    /// directions, fog range, clip planes, ...) named by `index` (`G_MW_*`)
    /// and `offset` (`G_MWO_*`) within it. `index == G_MW_LIGHTCOL` (RE-105)
    /// is the one this converter acts on -- see `mesh.rs`.
    MoveWord {
        index: u8,
        offset: u16,
        data: u32,
    },
    /// Draws a debug/wireframe line between two vertex-cache indices.
    /// `width` is added to the RDP's 1.5px minimum, in half-pixel units.
    /// F3DEX2's `gSPLine3D`/`gSPLineW3D`: unlike every other command here,
    /// the vertex/width fields live entirely in `w0` and `w1` is unused
    /// (always 0). Not known to be emitted by any of Smash's per-object
    /// meshes; modeled mainly so it stops showing up as `Other` and getting
    /// mistaken for a desync signal (RE-283 mistook this opcode for an
    /// unimplemented matrix-load command before it was decoded here).
    Line3D {
        v0: u8,
        v1: u8,
        width: u8,
    },
    /// A command we decode but do not model yet.
    ///
    /// `decode` never fails: every possible opcode byte maps to *some*
    /// `Cmd`, either a modeled one or this. That makes `Other` load-bearing
    /// for two different situations that must not be confused:
    ///
    /// 1. A *known* F3DEX2/RDP opcode this crate has not bothered to give a
    ///    dedicated variant to (e.g. `G_SETSCISSOR`, `G_FILLRECT`,
    ///    `G_TEXRECT`) because `mesh::convert_sequence` has no use for it.
    ///    This is benign and expected in real display lists.
    /// 2. A byte pattern with no defined F3DEX2 meaning at all, or a real
    ///    opcode (like `G_SETCIMG`, the RDP's set-render-target command)
    ///    that is never legitimately used inside one of Smash's per-object
    ///    display lists. Seeing this while decoding what is assumed to be
    ///    object geometry means the walk is very likely no longer looking
    ///    at real display-list data — either the start offset was wrong, or
    ///    it read past the list's true end into unrelated bytes that happen
    ///    to decode into opcode-shaped words anyway.
    ///
    /// `Cmd` alone cannot distinguish the two, and neither `decode` nor
    /// [`decode_list_at`] treats case 2 as an error — they keep decoding at
    /// the fixed `CMD_SIZE` stride regardless, because F3DEX2 commands are
    /// always one 8-byte word each (there is no variable-length command to
    /// "lose sync" on). What actually happens in case 2 is every command
    /// *after* the bad one is still a real decode of real bytes, just bytes
    /// that were never meant to be interpreted as a display list — so they
    /// come out looking like perfectly plausible `SetTimg`/`Tri1`/`Tri2`
    /// commands (valid-looking addresses, in-range-looking vertex indices)
    /// while actually being garbage.
    ///
    /// RE-283 lost a full session to exactly this: a scratch raw-decode
    /// tool hit `Other{opcode: 255}`/`Other{opcode: 8}` partway through a
    /// list and kept trusting everything decoded afterward, including a
    /// `SetTimg` address that looked real. **Any consumer of `decode_list`/
    /// [`decode_list_at`] that sees an `Other` whose `opcode` is not one of
    /// this file's named-but-unmodeled constants must treat every command
    /// after it in that same decode as unverified**, and must cross-check
    /// texture/vertex attribution against an authoritative resolution path
    /// (`mesh::convert_sequence`, or the real `pack()`/`plan_draw_order`
    /// traversal) before drawing conclusions from it.
    Other {
        opcode: u8,
        w0: u32,
        w1: u32,
    },
}

/// Splits a 64-bit command into its two big-endian halves.
fn words(raw: &[u8]) -> (u32, u32) {
    (
        u32::from_be_bytes([raw[0], raw[1], raw[2], raw[3]]),
        u32::from_be_bytes([raw[4], raw[5], raw[6], raw[7]]),
    )
}

fn rgba(w: u32) -> [u8; 4] {
    w.to_be_bytes()
}

fn other_mode(opcode: u8, w0: u32, w1: u32) -> Cmd {
    let len = ((w0 & 0xFF) + 1) as u8;
    let encoded_shift = ((w0 >> 8) & 0xFF) as u8;
    let Some(shift) = 32u8
        .checked_sub(encoded_shift)
        .and_then(|remaining| remaining.checked_sub(len))
    else {
        // Blind display-list discovery can encounter opcode-shaped data.
        // Impossible F3DEX2 fields are not a command and must not inherit
        // release-mode integer wrapping (RE-147).
        return Cmd::Other { opcode, w0, w1 };
    };
    if opcode == G_SETOTHERMODE_H {
        Cmd::SetOtherModeH {
            shift,
            len,
            data: w1,
        }
    } else {
        Cmd::SetOtherModeL {
            shift,
            len,
            data: w1,
        }
    }
}

/// Decodes a single F3DEX2 command.
pub fn decode(raw: &[u8]) -> Result<Cmd> {
    if raw.len() < CMD_SIZE {
        return Err(Error::OutOfBounds {
            offset: 0,
            len: CMD_SIZE,
        });
    }
    let (w0, w1) = words(raw);
    let opcode = (w0 >> 24) as u8;

    Ok(match opcode {
        G_NOOP | G_SPNOOP | G_RDPLOADSYNC | G_RDPPIPESYNC | G_RDPTILESYNC | G_RDPFULLSYNC => {
            Cmd::Sync(opcode)
        }

        // gSPVertex(pkt, v, n, v0) => gDma0p(G_VTX, v, ((n)<<12) | (((v0)+(n))*2))
        //
        // The low byte holds `(v0 + n) * 2` — the *end* of the destination
        // range, not its start. So `v0 = (w0 & 0xFF) / 2 - n`.
        //
        // Getting this wrong is quiet and costly: an earlier version computed
        // `(v0 + n) / 2`, which is only correct when `v0 == n`, so triangles
        // indexed vertex-cache slots that were never filled. Verified against a
        // real list (file 105, offset 0xCDA0: `01004008` is n=4, v0=0).
        G_VTX => {
            let count = ((w0 >> 12) & 0xFF) as u8;
            let end = ((w0 & 0xFF) >> 1) as u8;
            Cmd::Vtx {
                count,
                dest_index: end.saturating_sub(count),
                addr: SegAddr(w1),
            }
        }

        // Triangle indices are stored as index*2.
        G_TRI1 => Cmd::Tri1(tri(w0 >> 16, w0 >> 8, w0)),
        G_TRI2 => Cmd::Tri2(tri(w0 >> 16, w0 >> 8, w0), tri(w1 >> 16, w1 >> 8, w1)),

        // gSPLine3D/gSPLineW3D: vertex indices are index*2, same encoding as
        // Tri1/Tri2; width is the raw low byte. w1 is unused by this opcode.
        G_LINE3D => Cmd::Line3D {
            v0: ((w0 >> 16) & 0xFF) as u8 / 2,
            v1: ((w0 >> 8) & 0xFF) as u8 / 2,
            width: (w0 & 0xFF) as u8,
        },

        G_DL => {
            // The low byte of w0 selects call (0) vs branch (1).
            if (w0 >> 16) & 0x01 != 0 {
                Cmd::Branch(SegAddr(w1))
            } else {
                Cmd::Call(SegAddr(w1))
            }
        }
        G_ENDDL => Cmd::End,

        G_MTX => Cmd::Mtx {
            // F3DEX2 inverts the parameter byte relative to F3DEX.
            params: !((w0 >> 16) as u8),
            addr: SegAddr(w1),
        },
        G_POPMTX => Cmd::PopMtx { count: w1 / 64 },

        G_GEOMETRYMODE => Cmd::GeometryMode {
            clear: !w0 & 0x00FF_FFFF,
            set: w1,
        },

        G_TEXTURE => Cmd::Texture {
            level: ((w0 >> 11) & 0x07) as u8,
            tile: ((w0 >> 8) & 0x07) as u8,
            on: (w0 & 0xFF) != 0,
            scale_s: (w1 >> 16) as u16,
            scale_t: w1 as u16,
        },

        G_SETTIMG => Cmd::SetTimg {
            format: ((w0 >> 21) & 0x07) as u8,
            size: ((w0 >> 19) & 0x03) as u8,
            width: ((w0 & 0xFFF) + 1) as u16,
            addr: SegAddr(w1),
            slot: 0,
        },

        G_SETTILE => Cmd::SetTile {
            format: ((w0 >> 21) & 0x07) as u8,
            size: ((w0 >> 19) & 0x03) as u8,
            line: ((w0 >> 9) & 0x1FF) as u16,
            tmem: (w0 & 0x1FF) as u16,
            tile: ((w1 >> 24) & 0x07) as u8,
            palette: ((w1 >> 20) & 0x0F) as u8,
            cm_t: ((w1 >> 18) & 0x03) as u8,
            mask_t: ((w1 >> 14) & 0x0F) as u8,
            shift_t: ((w1 >> 10) & 0x0F) as u8,
            cm_s: ((w1 >> 8) & 0x03) as u8,
            mask_s: ((w1 >> 4) & 0x0F) as u8,
            shift_s: (w1 & 0x0F) as u8,
        },

        G_SETTILESIZE => Cmd::SetTileSize {
            tile: ((w1 >> 24) & 0x07) as u8,
            uls: ((w0 >> 12) & 0xFFF) as u16,
            ult: (w0 & 0xFFF) as u16,
            lrs: ((w1 >> 12) & 0xFFF) as u16,
            lrt: (w1 & 0xFFF) as u16,
        },

        G_LOADBLOCK => Cmd::LoadBlock {
            tile: ((w1 >> 24) & 0x07) as u8,
            uls: ((w0 >> 12) & 0xFFF) as u16,
            ult: (w0 & 0xFFF) as u16,
            lrs: ((w1 >> 12) & 0xFFF) as u16,
            dxt: (w1 & 0xFFF) as u16,
        },

        G_LOADTLUT => Cmd::LoadTlut {
            tile: ((w1 >> 24) & 0x07) as u8,
            // Stored as (count-1) << 2 in the high 12 bits of the low word.
            count: (((w1 >> 14) & 0x3FF) + 1) as u16,
        },

        G_SETPRIMCOLOR => Cmd::SetPrimColor {
            m: ((w0 >> 8) & 0xFF) as u8,
            l: (w0 & 0xFF) as u8,
            rgba: rgba(w1),
        },
        G_SETENVCOLOR => Cmd::SetEnvColor(rgba(w1)),
        G_SETBLENDCOLOR => Cmd::SetBlendColor(rgba(w1)),
        G_SETFOGCOLOR => Cmd::SetFogColor(rgba(w1)),
        G_SETCOMBINE => Cmd::SetCombine {
            hi: w0 & 0x00FF_FFFF,
            lo: w1,
        },

        // F3DEX2 encodes these as (32 - shift - len) and (len - 1).
        G_SETOTHERMODE_H | G_SETOTHERMODE_L => other_mode(opcode, w0, w1),

        // F3DEX2's `gMoveWd`/`gDma1p(pkt, G_MOVEWORD, data, offset, index)`:
        // `w0 = (G_MOVEWORD << 24) | (index << 16) | offset`, `w1 = data`.
        // Verified against `refs/ssb-decomp-re/include/PR/gbi.h`'s
        // `G_MWO_aLIGHT_1`/`bLIGHT_1`/`aLIGHT_2`/`bLIGHT_2` (0x00/0x04/0x18/
        // 0x1c) against a real four-command run (file 313, offset 0x1AB0)
        // that writes exactly those four offsets under `index = G_MW_LIGHTCOL`
        // (0x0a) -- RE-105.
        G_MOVEWORD => Cmd::MoveWord {
            index: ((w0 >> 16) & 0xFF) as u8,
            offset: (w0 & 0xFFFF) as u16,
            data: w1,
        },

        _ => Cmd::Other { opcode, w0, w1 },
    })
}

fn tri(a: u32, b: u32, c: u32) -> [u8; 3] {
    [
        ((a & 0xFF) / 2) as u8,
        ((b & 0xFF) / 2) as u8,
        ((c & 0xFF) / 2) as u8,
    ]
}

/// Decodes commands from `data` until `G_ENDDL` or the buffer runs out.
///
/// This does *not* follow `Call`/`Branch` — resolving a segmented address
/// needs a segment table the caller owns. `romtool` drives the traversal.
///
/// `base` is where `data` starts within its file, and is only used to fill in
/// [`Cmd::SetTimg::slot`]. Pass the real offset whenever the caller has it:
/// a texture that lives in another file can only be found through that slot.
///
/// This never returns an error for bad opcode data — see [`Cmd::Other`] for
/// why that matters. If `data` does not actually start at a real display
/// list (or if `base` is wrong), the result can decode all the way through
/// to a `G_ENDDL`-shaped word without ever coming back as `Err`, and every
/// command in it looks individually plausible. Callers doing exploratory or
/// blind decoding (not driven by a known-good object/scene graph) must scan
/// the result for `Cmd::Other` first and treat an unexpected one as a sign
/// the whole decode may be untrustworthy, per [`Cmd::Other`]'s doc comment.
pub fn decode_list_at(data: &[u8], base: u32) -> Result<Vec<Cmd>> {
    let mut out = Vec::new();
    for (i, raw) in data.as_chunks::<CMD_SIZE>().0.iter().enumerate() {
        let mut cmd = decode(raw)?;
        if let Cmd::SetTimg { slot, .. } = &mut cmd {
            // The address is `w1`, the second word of the command.
            *slot = base + (i * CMD_SIZE) as u32 + 4;
        }
        let end = cmd == Cmd::End;
        out.push(cmd);
        if end {
            break;
        }
    }
    Ok(out)
}

/// [`decode_list_at`] for a caller that does not know where the list sits.
///
/// Any texture the list reaches through a cross-file pointer is unresolvable
/// this way; use `decode_list_at` where the offset is known.
pub fn decode_list(data: &[u8]) -> Result<Vec<Cmd>> {
    decode_list_at(data, 0)
}

/// An N64 vertex as stored in ROM (`Vtx_t`), 16 bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Vtx {
    /// Object-space position, in the game's integer coordinate units.
    pub pos: [i16; 3],
    /// Texture coordinates in S10.5 fixed point.
    pub uv: [i16; 2],
    /// Vertex colour, or a packed normal when lighting is enabled — the
    /// geometry mode in force at draw time decides which.
    pub rgba: [u8; 4],
}

impl Vtx {
    pub const SIZE: usize = 16;

    pub fn parse(raw: &[u8]) -> Result<Vtx> {
        if raw.len() < Vtx::SIZE {
            return Err(Error::OutOfBounds {
                offset: 0,
                len: Vtx::SIZE,
            });
        }
        let i16at = |o: usize| i16::from_be_bytes([raw[o], raw[o + 1]]);
        Ok(Vtx {
            pos: [i16at(0), i16at(2), i16at(4)],
            // bytes 6..8 are padding / flag
            uv: [i16at(8), i16at(10)],
            rgba: [raw[12], raw[13], raw[14], raw[15]],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cmd(w0: u32, w1: u32) -> Cmd {
        let mut raw = [0u8; 8];
        raw[..4].copy_from_slice(&w0.to_be_bytes());
        raw[4..].copy_from_slice(&w1.to_be_bytes());
        decode(&raw).unwrap()
    }

    #[test]
    fn decodes_enddl() {
        assert_eq!(cmd(0xDF00_0000, 0), Cmd::End);
    }

    #[test]
    fn decodes_vtx_count_and_dest() {
        // gSPVertex(v, n=8, v0=8): low byte is (v0 + n) * 2 = 32.
        let w0 = 0x0100_0000 | (8 << 12) | 32;
        assert_eq!(
            cmd(w0, 0x0600_1234),
            Cmd::Vtx {
                count: 8,
                dest_index: 8,
                addr: SegAddr(0x0600_1234)
            }
        );
    }

    /// Regression: `dest_index` must not be derived as `(v0 + n) / 2`, which
    /// coincidentally matches only when `v0 == n`.
    #[test]
    fn decodes_vtx_when_dest_differs_from_count() {
        // Real command from file 105 @ 0xCDA0: n = 4, v0 = 0.
        assert_eq!(
            cmd(0x0100_4008, 0x0000_CD20),
            Cmd::Vtx {
                count: 4,
                dest_index: 0,
                addr: SegAddr(0x0000_CD20)
            }
        );

        // gSPVertex(v, n=2, v0=6): low byte = (6 + 2) * 2 = 16.
        assert_eq!(
            cmd(0x0100_0000 | (2 << 12) | 16, 0),
            Cmd::Vtx {
                count: 2,
                dest_index: 6,
                addr: SegAddr(0)
            }
        );
    }

    #[test]
    fn decodes_tri1_halving_indices() {
        // Vertices 0, 1, 2 are encoded as 0, 2, 4.
        assert_eq!(cmd(0x0500_0204, 0), Cmd::Tri1([0, 1, 2]));
    }

    #[test]
    fn decodes_line3d_halving_indices() {
        // Vertices 0, 3 with width 5, encoded as 0, 6 (index*2).
        assert_eq!(
            cmd(0x0800_0605, 0),
            Cmd::Line3D {
                v0: 0,
                v1: 3,
                width: 5
            }
        );
    }

    #[test]
    fn decodes_tri2_as_two_triangles() {
        assert_eq!(
            cmd(0x0600_0204, 0x0006_080A),
            Cmd::Tri2([0, 1, 2], [3, 4, 5])
        );
    }

    #[test]
    fn decodes_moveword_lightcol_from_a_real_rom_sample() {
        // RE-105: file 313 (Fox), offset 0x1AB0, four consecutive
        // `gMoveWd` commands writing `G_MW_LIGHTCOL`'s (0x0a) `aLIGHT_1`/
        // `bLIGHT_1`/`aLIGHT_2`/`bLIGHT_2` offsets (0x00/0x04/0x18/0x1c),
        // verified against `refs/ssb-decomp-re/include/PR/gbi.h`'s
        // `G_MWO_*` constants.
        assert_eq!(
            cmd(0xDB0A_0000, 0xFFFF_FF00),
            Cmd::MoveWord {
                index: 0x0a,
                offset: 0x00,
                data: 0xFFFF_FF00,
            }
        );
        assert_eq!(
            cmd(0xDB0A_0004, 0xFFFF_FF00),
            Cmd::MoveWord {
                index: 0x0a,
                offset: 0x04,
                data: 0xFFFF_FF00,
            }
        );
        assert_eq!(
            cmd(0xDB0A_0018, 0x4C4C_4C00),
            Cmd::MoveWord {
                index: 0x0a,
                offset: 0x18,
                data: 0x4C4C_4C00,
            }
        );
        assert_eq!(
            cmd(0xDB0A_001C, 0x4C4C_4C00),
            Cmd::MoveWord {
                index: 0x0a,
                offset: 0x1c,
                data: 0x4C4C_4C00,
            }
        );
    }

    #[test]
    fn distinguishes_call_from_branch() {
        assert_eq!(
            cmd(0xDE00_0000, 0x0700_0010),
            Cmd::Call(SegAddr(0x0700_0010))
        );
        assert_eq!(
            cmd(0xDE01_0000, 0x0700_0010),
            Cmd::Branch(SegAddr(0x0700_0010))
        );
    }

    #[test]
    fn segmented_address_splits() {
        let a = SegAddr(0x0601_2345);
        assert_eq!(a.segment(), 6);
        assert_eq!(a.offset(), 0x012345);
    }

    #[test]
    fn unknown_opcode_is_preserved_not_dropped() {
        assert_eq!(
            cmd(0xAB00_0000, 0xDEAD_BEEF),
            Cmd::Other {
                opcode: 0xAB,
                w0: 0xAB00_0000,
                w1: 0xDEAD_BEEF
            }
        );
    }

    #[test]
    fn impossible_othermode_fields_are_preserved_not_wrapped() {
        // Encoded shift 31 plus length 2 cannot fit in a 32-bit mode word.
        // This shape occurs when blind scanning encounters ordinary data.
        assert_eq!(
            cmd(0xE300_1F01, 0xDEAD_BEEF),
            Cmd::Other {
                opcode: G_SETOTHERMODE_H,
                w0: 0xE300_1F01,
                w1: 0xDEAD_BEEF,
            }
        );
    }

    #[test]
    fn decode_list_stops_at_end() {
        let mut data = alloc::vec![0u8; 24];
        data[0] = G_TRI1;
        data[8] = G_ENDDL;
        data[16] = G_TRI1; // must not be reached
        let cmds = decode_list(&data).unwrap();
        assert_eq!(cmds.len(), 2);
        assert_eq!(cmds[1], Cmd::End);
    }

    #[test]
    fn parses_vertex() {
        let raw = [
            0x00, 0x0A, 0xFF, 0xF6, 0x00, 0x14, 0x00, 0x00, 0x00, 0x20, 0x00, 0x40, 0x11, 0x22,
            0x33, 0x44,
        ];
        let v = Vtx::parse(&raw).unwrap();
        assert_eq!(v.pos, [10, -10, 20]);
        assert_eq!(v.uv, [32, 64]);
        assert_eq!(v.rgba, [0x11, 0x22, 0x33, 0x44]);
    }

    #[test]
    fn decode_list_at_records_where_each_texture_address_word_sits() {
        // Two SETTIMGs in one list. The slot is the *second* word of the
        // command, which is where the archive keys its relocation -- pointing
        // at the command instead would miss by four bytes and resolve nothing.
        let mut data = Vec::new();
        data.extend_from_slice(&[0xE1, 0, 0, 0, 0, 0, 0, 0]); // G_RDPLOADSYNC
        data.extend_from_slice(&[G_SETTIMG, 0x10, 0, 0, 0, 0, 0, 0]);
        data.extend_from_slice(&[G_SETTIMG, 0x10, 0, 0, 0, 0, 0, 0]);
        data.extend_from_slice(&[G_ENDDL, 0, 0, 0, 0, 0, 0, 0]);

        let slots: Vec<u32> = decode_list_at(&data, 0x1000)
            .unwrap()
            .iter()
            .filter_map(|c| match c {
                Cmd::SetTimg { slot, .. } => Some(*slot),
                _ => None,
            })
            .collect();
        assert_eq!(slots, [0x100C, 0x1014]);

        // Without a base the slots are still consistent, just file-relative
        // to nothing -- which is why the offset should always be passed.
        let bare: Vec<u32> = decode_list(&data)
            .unwrap()
            .iter()
            .filter_map(|c| match c {
                Cmd::SetTimg { slot, .. } => Some(*slot),
                _ => None,
            })
            .collect();
        assert_eq!(bare, [0x0C, 0x14]);
    }
}
