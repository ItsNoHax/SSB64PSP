//! The options and data menus' sprites, packed per scene (RE-461).
//!
//! The original loads each menu's files when its scene starts
//! (`lbRelocLoadFilesListed`) and frees them with the scene. The resident
//! pack has no room for them all (some 5 MB in 8888), so `romtool pack`
//! writes them beside it as `ssb64-menus.pak`: an index, then one ordinary
//! [`crate::pack`] per [`MenuScene`] holding only that scene's sprites. The
//! host reads the index and the one scene's pack when the scene starts and
//! drops it when the scene ends. Sprites a scene shares with the resident
//! pack (the fonts and the portraits) stay there.
//!
//! The index is little-endian: [`MAGIC`], the pack [`crate::pack::VERSION`],
//! the scene count, then each scene's byte offset and length. Every
//! scene's pack starts on a 64-byte boundary.

use crate::sprite::SpriteFile;

/// `"SMNU"`.
pub const MAGIC: u32 = u32::from_le_bytes(*b"SMNU");
/// The file the host loads beside `ssb64.pak`.
pub const FILE_NAME: &str = "ssb64-menus.pak";
/// Each scene's pack is aligned to this.
pub const ALIGN: usize = 64;

/// The scenes with their own sprites.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuScene {
    Option = 0,
    ScreenAdjust = 1,
    BackupClear = 2,
    Data = 3,
    VsRecord = 4,
    Characters = 5,
}

impl MenuScene {
    pub const ALL: [MenuScene; 6] = [
        MenuScene::Option,
        MenuScene::ScreenAdjust,
        MenuScene::BackupClear,
        MenuScene::Data,
        MenuScene::VsRecord,
        MenuScene::Characters,
    ];

    /// The scene's sprites (`dMN*FileIDs`, those it draws).
    pub fn sprites(self) -> &'static [SpriteFile] {
        match self {
            MenuScene::Option => &[COMMON_TABS, COMMON_DECALS, COMMON_SLASH, OPTION],
            MenuScene::ScreenAdjust => &[SCREEN_ADJUST],
            MenuScene::BackupClear => &[BACKUP_CLEAR, BACKUP_CLEAR_HEADER],
            MenuScene::Data => &[COMMON_TABS, COMMON_DECALS, DATA],
            MenuScene::VsRecord => &[VS_RECORD, DATA_COMMON],
            MenuScene::Characters => &[CHARACTERS, DATA_COMMON],
        }
    }

    /// Sprites the scene draws through a swapped `sprite.LUT`: `(file,
    /// sprite, palettes)`, each palette's variant packed as `ROLE_LUT`
    /// with its index.
    pub fn lut_sprites(self) -> &'static [(u32, u32, &'static [u32])] {
        match self {
            MenuScene::BackupClear => &[
                (FILE_BACKUP_CLEAR, BACKUP_CLEAR_YES, &BACKUP_CLEAR_YES_LUTS),
                (FILE_BACKUP_CLEAR, BACKUP_CLEAR_NO, &BACKUP_CLEAR_NO_LUTS),
            ],
            _ => &[],
        }
    }
}

const FILE_BACKUP_CLEAR: u32 = 0x4D;
/// `llMNBackupClearOptionYesSprite` and `...NoSprite` (CI4).
const BACKUP_CLEAR_YES: u32 = 0x7580;
const BACKUP_CLEAR_NO: u32 = 0x7AB8;
/// `OptionYesHighlightPalette`, `OptionYesNotPalette`,
/// `OptionConfirmPalette`.
const BACKUP_CLEAR_YES_LUTS: [u32; 3] = [0x7500, 0x7528, 0x7550];
/// `OptionNoHighlightPalette`, `OptionNoNotPalette`.
const BACKUP_CLEAR_NO_LUTS: [u32; 2] = [0x7A60, 0x7A88];

/// File 0, `MNCommon`: `OptionTabLeft`, `...Middle`, `...Right`.
pub const COMMON_TABS: SpriteFile = SpriteFile {
    file: 0x00,
    offsets: &[0x1E8, 0x330, 0x568],
};

/// File 0: `DecalPaper` and `SmashLogo`. The `SmashBrosCollage` behind
/// them is already resident (`crate::ending::MESSAGE_COLLAGE`).
pub const COMMON_DECALS: SpriteFile = SpriteFile {
    file: 0x00,
    offsets: &[0x2A30, 0x31F8],
};

/// File 0: `Slash`.
pub const COMMON_SLASH: SpriteFile = SpriteFile {
    file: 0x00,
    offsets: &[0xBA28],
};

/// File 4, `MNOption`: `StereoText`, `MonoText`, `SoundText`,
/// `ScreenAdjustText`, `BackupClearText`, `OptionText`,
/// `SettingsIconDark`.
pub const OPTION: SpriteFile = SpriteFile {
    file: 0x04,
    offsets: &[0x71F8, 0x73A8, 0x7628, 0x8138, 0x8780, 0x9288, 0xB958],
};

/// File 5, `MNData`: `CharactersText`, `VSRecordText`, `SoundTestText`,
/// `DataText`, `DataIconDark`.
pub const DATA: SpriteFile = SpriteFile {
    file: 0x05,
    offsets: &[0x14E0, 0x1900, 0x1D20, 0x23A8, 0x4A78],
};

/// File 0xF, `MNScreenAdjust`: `Instruction`, `Guide`.
pub const SCREEN_ADJUST: SpriteFile = SpriteFile {
    file: 0x0F,
    offsets: &[0x918, 0x98A0],
};

/// File 0x4D, `MNBackupClear`: `HeaderBackupClear`, the six options,
/// `OptionCircle`, `IsOkayText`, `AreYouSureText` (the Yes and No sprites
/// come through their LUTs).
pub const BACKUP_CLEAR: SpriteFile = SpriteFile {
    file: FILE_BACKUP_CLEAR,
    offsets: &[
        0xB60, 0x3A00, 0x4050, 0x46A0, 0x5340, 0x5990, 0x7020, 0x5DB8, 0x63C8, 0x69D8,
    ],
};

/// File 0x4E, `MNBackupClearHeaderOption`: `HeaderOption`.
pub const BACKUP_CLEAR_HEADER: SpriteFile = SpriteFile {
    file: 0x4E,
    offsets: &[0xB40],
};

/// File 0x20, `MNDataCommon`: `DataHeader`, `ArrowL`, `ArrowR`.
pub const DATA_COMMON: SpriteFile = SpriteFile {
    file: 0x20,
    offsets: &[0xB40, 0xBE0, 0xC80],
};

/// File 0x1F, `MNVSRecordMain`: every sprite `mnvsrecord.c` names.
pub const VS_RECORD: SpriteFile = SpriteFile {
    file: 0x1F,
    offsets: &[
        0x70, 0x258, 0x2F0, 0x390, 0x430, 0x4D0, 0x570, 0x610, 0x6B0, 0x750, 0x7F0, 0x890, 0x910,
        0xA08, 0xAF8, 0xBE8, 0xCD8, 0xE10, 0xF08, 0x1008, 0x1140, 0x11D0, 0x1318, 0x1458, 0x15D0,
        0x1668, 0x17A8, 0x1918, 0x1A98, 0x1CA8, 0x1E88, 0x2008, 0x2178, 0x2370, 0x2540, 0x2698,
        0x27C8, 0x2930, 0x2B30, 0x2D18, 0x2EF8, 0x3198, 0x3438, 0x3618, 0x37F8, 0x3A38, 0x3CD8,
        0x3EB8, 0x4098, 0x4308, 0x45A8, 0x4D30, 0x5428, 0x54C0,
    ],
};

/// File 0x10, `MNCharacters`: every sprite `mncharacters.c` names.
pub const CHARACTERS: SpriteFile = SpriteFile {
    file: 0x10,
    offsets: &[
        0x630, 0x1230, 0x28F0, 0x2F98, 0x33A0, 0x4290, 0x4910, 0x4F78, 0x5398, 0x58F8, 0x6828,
        0x6E48, 0x7628, 0x82E0, 0x8828, 0xACA8, 0xD128, 0xF5A8, 0x11A28, 0x13EA8, 0x16328, 0x187A8,
        0x1AC28, 0x1D0A8, 0x1F528, 0x219A8, 0x23E28, 0x25058, 0x25AB8, 0x26518, 0x26F78, 0x279D8,
        0x28438, 0x28E98, 0x298F8, 0x2A358, 0x2ADB8, 0x2B818, 0x2C278, 0x2CCD8, 0x2CDA8, 0x2CE78,
        0x2CF48, 0x2D088, 0x2D1C8, 0x2D308, 0x2D448, 0x2D588, 0x2D6C8, 0x2D808, 0x2D948, 0x2DA88,
        0x2DBC8, 0x2DD08, 0x2DE48, 0x2DF88, 0x2E0C8, 0x2E208, 0x2E348, 0x2E488, 0x2E5C8, 0x2E740,
        0x2E888, 0x2E9C8, 0x2EB08, 0x2EC48, 0x2ED88, 0x2EEC8, 0x2F008, 0x2F148, 0x2F288, 0x2F3C8,
        0x2F508, 0x2F648, 0x2F788, 0x2F8C8, 0x2FA08, 0x30888,
    ],
};

fn u32_at(b: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(at..at + 4)?.try_into().ok()?))
}

/// The index's length for `count` scenes.
pub fn index_len(count: usize) -> usize {
    12 + 8 * count
}

/// The scene's `(offset, len)` from the file's first bytes, if the index
/// is this build's.
pub fn locate(index: &[u8], scene: MenuScene) -> Option<(u32, u32)> {
    if u32_at(index, 0)? != MAGIC || u32_at(index, 4)? != crate::pack::VERSION {
        return None;
    }
    let count = u32_at(index, 8)? as usize;
    let i = scene as usize;
    if i >= count {
        return None;
    }
    Some((u32_at(index, 12 + 8 * i)?, u32_at(index, 16 + 8 * i)?))
}

/// The file: the index, then each scene's pack in [`MenuScene::ALL`]'s
/// order.
#[cfg(feature = "std")]
pub fn build(packs: &[Vec<u8>]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&MAGIC.to_le_bytes());
    out.extend_from_slice(&crate::pack::VERSION.to_le_bytes());
    out.extend_from_slice(&(packs.len() as u32).to_le_bytes());
    out.resize(index_len(packs.len()), 0);
    let mut at = index_len(packs.len()).next_multiple_of(ALIGN);
    for (i, p) in packs.iter().enumerate() {
        out[12 + 8 * i..16 + 8 * i].copy_from_slice(&(at as u32).to_le_bytes());
        out[16 + 8 * i..20 + 8 * i].copy_from_slice(&(p.len() as u32).to_le_bytes());
        at = (at + p.len()).next_multiple_of(ALIGN);
    }
    for p in packs {
        out.resize(out.len().next_multiple_of(ALIGN), 0);
        out.extend_from_slice(p);
    }
    out
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;

    #[test]
    fn the_index_finds_each_scene() {
        let packs = vec![vec![1u8; 10], vec![2u8; 70], vec![3u8; 3]];
        let file = build(&packs);
        for (i, scene) in [
            MenuScene::Option,
            MenuScene::ScreenAdjust,
            MenuScene::BackupClear,
        ]
        .into_iter()
        .enumerate()
        {
            let (at, len) = locate(&file, scene).unwrap();
            assert_eq!(at as usize % ALIGN, 0);
            assert_eq!(&file[at as usize..(at + len) as usize], &packs[i][..]);
        }
        assert_eq!(locate(&file, MenuScene::Characters), None);
        assert_eq!(locate(&file[4..], MenuScene::Option), None);
    }
}
