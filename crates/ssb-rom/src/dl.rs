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
    /// A command we decode but do not model yet.
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
        G_SETOTHERMODE_H => Cmd::SetOtherModeH {
            len: ((w0 & 0xFF) + 1) as u8,
            shift: (32 - ((w0 >> 8) & 0xFF) as u8 - (((w0 & 0xFF) + 1) as u8)),
            data: w1,
        },
        G_SETOTHERMODE_L => Cmd::SetOtherModeL {
            len: ((w0 & 0xFF) + 1) as u8,
            shift: (32 - ((w0 >> 8) & 0xFF) as u8 - (((w0 & 0xFF) + 1) as u8)),
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
pub fn decode_list(data: &[u8]) -> Result<Vec<Cmd>> {
    let mut out = Vec::new();
    for raw in data.chunks_exact(CMD_SIZE) {
        let cmd = decode(raw)?;
        let end = cmd == Cmd::End;
        out.push(cmd);
        if end {
            break;
        }
    }
    Ok(out)
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
    fn decodes_tri2_as_two_triangles() {
        assert_eq!(
            cmd(0x0600_0204, 0x0006_080A),
            Cmd::Tri2([0, 1, 2], [3, 4, 5])
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
}
