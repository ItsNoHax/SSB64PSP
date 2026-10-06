//! The 1P Game's last scenes' assets (US): the ending movie
//! (`mvending.c`), the staff roll (`scstaffroll.c`), the congratulations
//! picture (`mncongra.c`), the challenger warning (`sc1pchallenger.c`) and
//! the unlock message (`mnmessage.c`). Offsets are from
//! `include/reloc_data.us.h`; no ROM data lives here.
use crate::archive::File;
use crate::sprite::{Sprite, SpriteError, SpriteFile};
use crate::texture::{self, BitSize, Format, Rgba8};

/// File 9, `MNMessage`: the seven unlock messages in `nLBBackupUnlock`
/// order, then `DecalExclaim`.
pub const MESSAGE: SpriteFile = SpriteFile {
    file: 9,
    offsets: &[
        0x9e0, 0x1148, 0x1f50, 0x2e58, 0x3458, 0x4180, 0x4eb0, 0x5300,
    ],
};
pub const MESSAGE_EXCLAIM: u32 = 0x5300;

/// File 0, `MNCommon`: `llMNCommonSmashBrosCollageSprite`, the message's
/// wallpaper.
pub const MESSAGE_COLLAGE: SpriteFile = SpriteFile {
    file: 0,
    offsets: &[0x18000],
};
pub const COLLAGE: u32 = 0x18000;

/// File 10, `SC1PChallenger`: `ChallengerText`, `ApproachingText`,
/// `WarningText`, `DecalExclaim`.
pub const CHALLENGER: SpriteFile = SpriteFile {
    file: 10,
    offsets: &[0x1f8, 0x488, 0x968, 0xdb0],
};
pub const CHALLENGER_TEXT: u32 = 0x1f8;
pub const APPROACHING_TEXT: u32 = 0x488;
pub const WARNING_TEXT: u32 = 0x968;
pub const CHALLENGER_EXCLAIM: u32 = 0xdb0;

/// Every congratulations picture is one 300 x 110 sprite at this offset of
/// its own file (`llMNCongra*{Bottom,Top}Sprite`).
pub const CONGRA_SPRITE: u32 = 0x20718;
/// The pack keys a picture's columns from [`CONGRA_SPLIT`] on under this
/// offset (no real sprite starts at an odd offset); [`CONGRA_SPRITE`] keys
/// the columns before it.
pub const CONGRA_SPRITE_RIGHT: u32 = CONGRA_SPRITE | 1;
/// Where a congratulations picture is cut: a power of two, so the left
/// part needs no padding.
pub const CONGRA_SPLIT: u16 = 256;

/// One part of a congratulations picture: columns `0..CONGRA_SPLIT`
/// (`part` 0) or the rest (`part` 1).
pub fn congra_part(s: &Sprite, part: usize) -> Sprite {
    let (from, to) = if part == 0 {
        (0, CONGRA_SPLIT.min(s.width))
    } else {
        (CONGRA_SPLIT.min(s.width), s.width)
    };
    let width = to - from;
    let mut image = Rgba8::new(u32::from(width), u32::from(s.height));
    for y in 0..usize::from(s.height) {
        for x in 0..usize::from(width) {
            image.put(
                y * usize::from(width) + x,
                s.image
                    .get(y * usize::from(s.width) + usize::from(from) + x),
            );
        }
    }
    Sprite {
        width,
        image,
        ..s.clone()
    }
}

/// `dMNCongraPictures`' files by `FTKind` (Mario .. Ness): bottom, top
/// (`llMNCongra*FileID`).
pub const CONGRA_FILES: [[u32; 2]; 12] = [
    [0xba, 0xbb], // Mario
    [0xbe, 0xbf], // Fox
    [0xb8, 0xb9], // Donkey
    [0xb0, 0xb1], // Samus
    [0xbc, 0xbd], // Luigi
    [0xb2, 0xb3], // Link
    [0xac, 0xad], // Yoshi
    [0xb6, 0xb7], // Captain
    [0xaa, 0xab], // Kirby
    [0xae, 0xaf], // Pikachu
    [0xb4, 0xb5], // Purin
    [0xc0, 0xc1], // Ness
];

/// The congratulations picture files as sprite files, bottom then top.
pub const CONGRA: [SpriteFile; 24] = [
    SpriteFile {
        file: 0xba,
        offsets: &[CONGRA_SPRITE],
    },
    SpriteFile {
        file: 0xbb,
        offsets: &[CONGRA_SPRITE],
    },
    SpriteFile {
        file: 0xbe,
        offsets: &[CONGRA_SPRITE],
    },
    SpriteFile {
        file: 0xbf,
        offsets: &[CONGRA_SPRITE],
    },
    SpriteFile {
        file: 0xb8,
        offsets: &[CONGRA_SPRITE],
    },
    SpriteFile {
        file: 0xb9,
        offsets: &[CONGRA_SPRITE],
    },
    SpriteFile {
        file: 0xb0,
        offsets: &[CONGRA_SPRITE],
    },
    SpriteFile {
        file: 0xb1,
        offsets: &[CONGRA_SPRITE],
    },
    SpriteFile {
        file: 0xbc,
        offsets: &[CONGRA_SPRITE],
    },
    SpriteFile {
        file: 0xbd,
        offsets: &[CONGRA_SPRITE],
    },
    SpriteFile {
        file: 0xb2,
        offsets: &[CONGRA_SPRITE],
    },
    SpriteFile {
        file: 0xb3,
        offsets: &[CONGRA_SPRITE],
    },
    SpriteFile {
        file: 0xac,
        offsets: &[CONGRA_SPRITE],
    },
    SpriteFile {
        file: 0xad,
        offsets: &[CONGRA_SPRITE],
    },
    SpriteFile {
        file: 0xb6,
        offsets: &[CONGRA_SPRITE],
    },
    SpriteFile {
        file: 0xb7,
        offsets: &[CONGRA_SPRITE],
    },
    SpriteFile {
        file: 0xaa,
        offsets: &[CONGRA_SPRITE],
    },
    SpriteFile {
        file: 0xab,
        offsets: &[CONGRA_SPRITE],
    },
    SpriteFile {
        file: 0xae,
        offsets: &[CONGRA_SPRITE],
    },
    SpriteFile {
        file: 0xaf,
        offsets: &[CONGRA_SPRITE],
    },
    SpriteFile {
        file: 0xb4,
        offsets: &[CONGRA_SPRITE],
    },
    SpriteFile {
        file: 0xb5,
        offsets: &[CONGRA_SPRITE],
    },
    SpriteFile {
        file: 0xc0,
        offsets: &[CONGRA_SPRITE],
    },
    SpriteFile {
        file: 0xc1,
        offsets: &[CONGRA_SPRITE],
    },
];

/// File 195, `SCStaffroll`.
pub const STAFFROLL_FILE: u32 = 0xc3;

/// `dSCStaffrollNameAndJobSpriteInfo`'s images by name/job font index:
/// A-Z, a-z, period, comma, apostrophe, 4. Each is a raw I4 image whose
/// rows are its width rounded up to 16 texels
/// (`scStaffrollInitNameAndJobDisplayLists`'s `gDPLoadTextureBlock_4b`).
pub const NAME_IMAGES: [u32; 56] = [
    0x8, 0x218, 0x398, 0x4f8, 0x728, 0x888, 0xa08, 0xc38, 0xe68, 0xfe8, 0x1188, 0x13b8, 0x1538,
    0x17d8, 0x19e8, 0x1c88, 0x1e08, 0x2038, 0x2198, 0x22f8, 0x2478, 0x25d8, 0x27e8, 0x2a88, 0x2c98,
    0x2e18, 0x178, 0x2d8, 0x458, 0x668, 0x7e8, 0x948, 0xb78, 0xda8, 0xf28, 0x10a8, 0x12f8, 0x1478,
    0x16a8, 0x1948, 0x1b58, 0x1d48, 0x1f78, 0x20f8, 0x2258, 0x23b8, 0x2538, 0x2748, 0x2958, 0x2bf8,
    0x2d58, 0x2f88, 0x3078, 0x3018, 0x30b8, 0x3118,
];

/// `dSCStaffrollTextBoxSpriteInfo`'s sprites by text font index: A-Z, a-z,
/// colon, 9 to 0, period, dash, comma, ampersand, quote, slash, apostrophe,
/// question mark, brackets, e-acute.
pub const TEXT_SPRITES: [u32; 74] = [
    0x3258, 0x33e8, 0x3588, 0x3718, 0x38b8, 0x3a48, 0x3be8, 0x3d78, 0x3f18, 0x40b8, 0x4258, 0x43f8,
    0x4598, 0x4728, 0x48b8, 0x4a48, 0x4bd8, 0x4d68, 0x4ef8, 0x5088, 0x5228, 0x53b8, 0x5548, 0x56d8,
    0x5868, 0x59f8, 0x3310, 0x34b0, 0x3640, 0x37e0, 0x3970, 0x3b10, 0x3ca8, 0x3e40, 0x3fe0, 0x4188,
    0x4320, 0x44c0, 0x4650, 0x47e0, 0x4970, 0x4b08, 0x4c98, 0x4e20, 0x4fb0, 0x5150, 0x52e0, 0x5470,
    0x5600, 0x5790, 0x5928, 0x5ab0, 0x5b70, 0x6468, 0x6398, 0x62c8, 0x61f8, 0x6128, 0x6058, 0x5f88,
    0x5eb8, 0x5de8, 0x6538, 0x5c90, 0x5d18, 0x5c00, 0x6698, 0x65c0, 0x6758, 0x67e0, 0x68b8, 0x6988,
    0x6a58, 0x6b20,
];

/// A name/job letter image as a sprite: the `width` x `height` texels the
/// letter's quad samples (`tc` from 0 to `width` and `height`), cut from
/// rows `width` rounded up to 16 wide. The rows are linear, as
/// `gDPLoadTextureBlock_4b` loads them.
pub fn name_glyph(file: &File, at: u32, width: u16, height: u16) -> Result<Sprite, SpriteError> {
    let row = u32::from(width).next_multiple_of(16);
    let len = (row * u32::from(height) / 2) as usize;
    let bytes = file
        .data
        .get(at as usize..at as usize + len)
        .ok_or(SpriteError::Truncated)?;
    let full = texture::decode(
        bytes,
        row,
        u32::from(height),
        Format::I,
        BitSize::Bits4,
        None,
    )
    .map_err(SpriteError::Texture)?;
    let mut image = Rgba8::new(u32::from(width), u32::from(height));
    for y in 0..usize::from(height) {
        for x in 0..usize::from(width) {
            image.put(y * usize::from(width) + x, full.get(y * row as usize + x));
        }
    }
    Ok(Sprite {
        width,
        height,
        attr: crate::sprite::SP_TRANSPARENT,
        color: [0xFF; 4],
        format: Format::I,
        size: BitSize::Bits4,
        image,
    })
}

pub const CROSSHAIR: u32 = 0x6d58;
pub const BRACKET_LEFT: u32 = 0x6f98;
pub const BRACKET_RIGHT: u32 = 0x71d8;

/// The text box font, then the crosshair and the brackets.
pub const STAFFROLL: SpriteFile = SpriteFile {
    file: STAFFROLL_FILE,
    offsets: &[
        0x3258,
        0x33e8,
        0x3588,
        0x3718,
        0x38b8,
        0x3a48,
        0x3be8,
        0x3d78,
        0x3f18,
        0x40b8,
        0x4258,
        0x43f8,
        0x4598,
        0x4728,
        0x48b8,
        0x4a48,
        0x4bd8,
        0x4d68,
        0x4ef8,
        0x5088,
        0x5228,
        0x53b8,
        0x5548,
        0x56d8,
        0x5868,
        0x59f8,
        0x3310,
        0x34b0,
        0x3640,
        0x37e0,
        0x3970,
        0x3b10,
        0x3ca8,
        0x3e40,
        0x3fe0,
        0x4188,
        0x4320,
        0x44c0,
        0x4650,
        0x47e0,
        0x4970,
        0x4b08,
        0x4c98,
        0x4e20,
        0x4fb0,
        0x5150,
        0x52e0,
        0x5470,
        0x5600,
        0x5790,
        0x5928,
        0x5ab0,
        0x5b70,
        0x6468,
        0x6398,
        0x62c8,
        0x61f8,
        0x6128,
        0x6058,
        0x5f88,
        0x5eb8,
        0x5de8,
        0x6538,
        0x5c90,
        0x5d18,
        0x5c00,
        0x6698,
        0x65c0,
        0x6758,
        0x67e0,
        0x68b8,
        0x6988,
        0x6a58,
        0x6b20,
        CROSSHAIR,
        BRACKET_LEFT,
        BRACKET_RIGHT,
    ],
};

/// `llSCStaffrollInterpolation`: the names' path (`SYInterpDesc`).
pub const STAFFROLL_INTERPOLATION: u32 = 0x7304;
/// `llSCStaffrollAnimJoint`: the names' tilt.
pub const STAFFROLL_ANIM_JOINT: u32 = 0x7338;
/// `llSCStaffrollDObjDesc`: the highlighted name's frame.
pub const STAFFROLL_FRAME_GRAPH: u32 = 0x78c0;

/// `ovl59`'s ROM start and VRAM base (`smashbrothers.us.yaml`); its
/// `.data` begins with the credits tables
/// (`ssb_game::spgame::staffroll::DATA_VRAM`).
pub const OVL59_ROM: usize = 0x17f200;
pub const OVL59_VRAM: u32 = 0x8013_1b00;

/// The staff roll's credits tables in a US ROM.
pub fn credits_bytes(rom: &[u8]) -> Option<&[u8]> {
    use ssb_game_layout::{DATA_LEN, DATA_VRAM};
    let at = OVL59_ROM + (DATA_VRAM - OVL59_VRAM) as usize;
    rom.get(at..at + DATA_LEN)
}

/// The tables' place, as `ssb_game::spgame::staffroll` reads them; kept
/// here as plain numbers so this crate does not depend on the game.
mod ssb_game_layout {
    pub const DATA_VRAM: u32 = 0x8013_5260;
    pub const DATA_LEN: usize = 0x8013_a184 - 0x8013_5260;
}

/// File 52, `MVCommon`: the room the ending's figure drops into, as
/// `mvEndingMakeRoom*` builds it.
pub const ROOM_FILE: u32 = 0x34;
pub const ROOM_BACKGROUND: u32 = 0x7e98;
pub const ROOM_BACKGROUND_MAT_ANIM_JOINT: u32 = 0x8788;
pub const ROOM_DESK: u32 = 0x8df8;
pub const ROOM_BOOKS: u32 = 0xa6f8;
pub const ROOM_BOOKS_ANIM_JOINT: u32 = 0xa7b0;
pub const ROOM_PENCILS: u32 = 0xaeb8;
pub const ROOM_PENCILS_ANIM_JOINT: u32 = 0xaf70;
pub const ROOM_LAMP: u32 = 0xbdc0;
pub const ROOM_LAMP_ANIM_JOINT: u32 = 0xbea0;
/// `llMVCommonRoomTissuesDisplayList`: one list on one `DObj`.
pub const ROOM_TISSUES: u32 = 0xc690;
pub const ROOM_TISSUES_ANIM_JOINT: u32 = 0xc884;

/// File 76, `MVEnding`: `llMVEndingOperatorCamAnimJoint`.
pub const ENDING_FILE: u32 = 0x4c;
pub const ENDING_CAMERA: u32 = 0x0;

/// Reserved `AnimDesc::EFFECT` slots: the credits tables, the ending's
/// baked camera, and the room's and the staff roll's whole files (their
/// joint scripts and the names' path).
pub const CREDITS_SLOT: u32 = 0xF103;
pub const ENDING_CAMERA_SLOT: u32 = 0xF104;
pub const ROOM_SCRIPTS_SLOT: u32 = 0xF105;
pub const STAFFROLL_SCRIPTS_SLOT: u32 = 0xF106;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_staffroll_sprite_list_is_the_text_font_then_three() {
        let mut font: Vec<u32> = TEXT_SPRITES.to_vec();
        font.sort_unstable();
        font.dedup();
        assert_eq!(font.len(), 74);
        assert_eq!(STAFFROLL.offsets.len(), 74 + 3);
        for at in TEXT_SPRITES {
            assert!(STAFFROLL.offsets.contains(&at));
        }
    }

    #[test]
    fn the_congratulations_files_cover_the_twenty_four_pictures() {
        for (i, pair) in CONGRA_FILES.iter().enumerate() {
            assert_eq!([CONGRA[2 * i].file, CONGRA[2 * i + 1].file], *pair);
        }
        let mut files: Vec<u32> = CONGRA.iter().map(|f| f.file).collect();
        files.sort_unstable();
        assert_eq!(files, (0xaa..=0xc1).collect::<Vec<_>>());
    }
}
