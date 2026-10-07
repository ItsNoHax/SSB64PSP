//! The menu, title, How to Play and opening scenes' own content (RE-461,
//! RE-475): which sprites, whole files and baked cameras each scene
//! draws. `romtool pack` adds them to the pack, where each belongs to its
//! archive file and loads with the scene that lists it
//! (`ssb_rom::scene_roots`). Before RE-475 each scene's lay in a pack of
//! its own in `ssb64-menus.pak`.

use crate::opening as op;
use crate::sprite::SpriteFile;

/// The scenes with their own sprites.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuScene {
    Option = 0,
    ScreenAdjust = 1,
    BackupClear = 2,
    Data = 3,
    VsRecord = 4,
    Characters = 5,
    Title = 6,
    ModeSelect = 7,
    OnePMode = 8,
    VsMode = 9,
    VsOptions = 10,
    VsItemSwitch = 11,
    Players1PBonus = 12,
    /// How to Play (`scExplain`): its window's sprites, the stick's,
    /// spark's and overlay's textures and the scene's data blobs
    /// ([`crate::explain`]).
    Explain = 13,
    /// The N64 logo (`mnStartup`).
    Startup = 14,
    /// The opening movie's scenes ([`crate::opening`]): each one's sprites,
    /// baked cameras and script files. The eight fighter scenes share one.
    OpeningRoom = 15,
    OpeningPortraits = 16,
    OpeningFighters = 17,
    OpeningRun = 18,
    OpeningCliff = 19,
    OpeningYamabuki = 20,
    OpeningJungle = 21,
    OpeningYoster = 22,
    OpeningSector = 23,
    OpeningStandoff = 24,
    OpeningClash = 25,
    OpeningNewcomers = 26,
    /// The opening's own models (`ssb_rom::opening::is_model_file`): their
    /// objects, textures and material scripts in a pack of their own, loaded
    /// for the whole opening rather than held in the resident pack.
    OpeningModels = 27,
}

impl MenuScene {
    pub const ALL: [MenuScene; 28] = [
        MenuScene::Option,
        MenuScene::ScreenAdjust,
        MenuScene::BackupClear,
        MenuScene::Data,
        MenuScene::VsRecord,
        MenuScene::Characters,
        MenuScene::Title,
        MenuScene::ModeSelect,
        MenuScene::OnePMode,
        MenuScene::VsMode,
        MenuScene::VsOptions,
        MenuScene::VsItemSwitch,
        MenuScene::Players1PBonus,
        MenuScene::Explain,
        MenuScene::Startup,
        MenuScene::OpeningRoom,
        MenuScene::OpeningPortraits,
        MenuScene::OpeningFighters,
        MenuScene::OpeningRun,
        MenuScene::OpeningCliff,
        MenuScene::OpeningYamabuki,
        MenuScene::OpeningJungle,
        MenuScene::OpeningYoster,
        MenuScene::OpeningSector,
        MenuScene::OpeningStandoff,
        MenuScene::OpeningClash,
        MenuScene::OpeningNewcomers,
        MenuScene::OpeningModels,
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
            MenuScene::Title => &[TITLE, TITLE_FIRE],
            MenuScene::ModeSelect => &[MAIN],
            MenuScene::OnePMode => &[COMMON_TABS, COMMON_DECALS, COMMON_GAME_MODE, ONE_P],
            MenuScene::VsMode => &[
                COMMON_TABS,
                COMMON_DECALS,
                COMMON_GAME_MODE,
                COMMON_ARROWS,
                VS_MODE,
            ],
            MenuScene::VsOptions => &[COMMON_OPTIONS, COMMON_SLASH, VS_OPTIONS],
            MenuScene::VsItemSwitch => &[VS_ITEM_SWITCH],
            MenuScene::Players1PBonus => &[BONUS_GAME_MODES, BONUS_RECORDS],
            MenuScene::Explain => &[crate::explain::SPRITES],
            MenuScene::Startup => &[op::STARTUP_SPRITES],
            MenuScene::OpeningPortraits => {
                &[op::PORTRAITS_SET1_SPRITES, op::PORTRAITS_SET2_SPRITES]
            }
            MenuScene::OpeningRun => &[op::RUN_SPRITES],
            MenuScene::OpeningCliff | MenuScene::OpeningStandoff => {
                &[op::STANDOFF_WALLPAPER_SPRITES]
            }
            MenuScene::OpeningYamabuki => &[op::YAMABUKI_SPRITES],
            MenuScene::OpeningSector => &[op::SECTOR_SPRITES, op::SECTOR_WALLPAPER_SPRITES],
            MenuScene::OpeningRoom
            | MenuScene::OpeningFighters
            | MenuScene::OpeningJungle
            | MenuScene::OpeningYoster
            | MenuScene::OpeningClash
            | MenuScene::OpeningNewcomers
            | MenuScene::OpeningModels => &[],
        }
    }

    /// Whole files the scene's joint scripts play from, each under
    /// [`crate::opening::blob_slot`]. The room's `MVCommon` is resident
    /// (the ending's `ROOM_SCRIPTS_SLOT`).
    pub fn blobs(self) -> &'static [u32] {
        use op::file as f;
        match self {
            MenuScene::Title => &[crate::title::FILE],
            MenuScene::OpeningRoom => &[f::ROOM_TRANSITION],
            MenuScene::OpeningRun => &[f::RUN],
            MenuScene::OpeningCliff => &[f::CLIFF],
            MenuScene::OpeningYamabuki => &[f::YAMABUKI],
            MenuScene::OpeningYoster => &[f::YOSTER],
            MenuScene::OpeningSector => &[f::SECTOR],
            MenuScene::OpeningStandoff => &[f::STANDOFF],
            MenuScene::OpeningClash => &[f::CLASH_WALLPAPER],
            MenuScene::OpeningNewcomers => &[f::NEWCOMERS1, f::NEWCOMERS2],
            _ => &[],
        }
    }

    /// The scene's camera animations, baked one play per frame under
    /// [`crate::opening::CamAnim::slot`].
    pub fn cameras(self) -> &'static [op::CamAnim] {
        match self {
            MenuScene::OpeningRoom => &op::ROOM_CAMERAS,
            MenuScene::OpeningFighters => &op::COMMON_CAMERAS,
            MenuScene::OpeningRun => &[op::RUN_CAMERA],
            MenuScene::OpeningCliff => &[op::CLIFF_CAMERA],
            MenuScene::OpeningYamabuki => &[op::YAMABUKI_CAMERA],
            MenuScene::OpeningJungle => &[op::JUNGLE_CAMERA],
            MenuScene::OpeningYoster => &[op::YOSTER_CAMERA],
            MenuScene::OpeningSector => &[op::SECTOR_CAMERA],
            MenuScene::OpeningStandoff => &[op::STANDOFF_CAMERA],
            MenuScene::OpeningClash => &[op::CLASH_FIGHTERS_CAMERA, op::CLASH_WALLPAPER_CAMERA],
            _ => &[],
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

/// File 0xA7, `MNTitle`: `LogoAnimFull`, `BorderUpper`, `Cutout`,
/// `TMUnk`, `Copyright`, `PressStart`, `Super`, `Smash`, `Bros` (the US
/// title's sprites), then the opening layout's `LogoAnimCutout`,
/// `...StrikeV` and `...StrikeH` (RE-467).
/// The scene's pack also holds the labels' and "Press Start"'s baked
/// animation plays (`crate::title`).
pub const TITLE: SpriteFile = SpriteFile {
    file: crate::title::FILE,
    offsets: &[
        0xBBB0, 0xC208, 0x11988, 0x11AA8, 0x15320, 0x15A48, 0x16728, 0x245C8, 0x25188, 0x8FC8,
        0x97E8, 0x9B48,
    ],
};

/// File 0xA8, `MNTitleFireAnim`: `Frame1Sprite` to `Frame30Sprite`.
pub const TITLE_FIRE: SpriteFile = SpriteFile {
    file: crate::title::FIRE_FILE,
    offsets: &[
        0x1018, 0x2078, 0x30D8, 0x4138, 0x5198, 0x61F8, 0x7258, 0x82B8, 0x9318, 0xA378, 0xB3D8,
        0xC438, 0xD498, 0xE4F8, 0xF558, 0x105B8, 0x11618, 0x12678, 0x136D8, 0x14738, 0x15798,
        0x167F8, 0x17858, 0x188B8, 0x19918, 0x1A978, 0x1B9D8, 0x1CA38, 0x1DA98, 0x1EAF8,
    ],
};

/// File 1, `MNMain`: the mode select's icons, their dark versions, the
/// labels, `ModeSelectText`, the decal bar and `SmashLogo` (the US build's
/// sprites; not the Japanese subtitles).
pub const MAIN: SpriteFile = SpriteFile {
    file: 0x01,
    offsets: &[
        0x1990, 0x2520, 0x30B0, 0x40F0, 0x4C80, 0x5570, 0x57E0, 0x5980, 0x6048, 0x6708, 0x6DC8,
        0x72E8, 0x7AA8, 0x7C38, 0x82F8, 0x84F8,
    ],
};

/// File 2, `MN1P`: `OptionTab`, `1PGameText`, `ControllerIconDark`,
/// `1PText`, `TrainingModeText`, `Bonus1PracticeText`,
/// `Bonus2PracticeText`.
pub const ONE_P: SpriteFile = SpriteFile {
    file: 0x02,
    offsets: &[0x1108, 0x2A28, 0x50F8, 0x5338, 0x5AC8, 0x5F28, 0x6388],
};

/// File 0: `ArrowL`, `ArrowR`, `Infinity`.
pub const COMMON_ARROWS: SpriteFile = SpriteFile {
    file: 0x00,
    offsets: &[0xDE30, 0xDD90, 0xDC48],
};

/// File 0: `OnText`, `OffText`, `AutoText`, `Percentage`.
pub const COMMON_OPTIONS: SpriteFile = SpriteFile {
    file: 0x00,
    offsets: &[0xB818, 0xB958, 0xDF48, 0xDB30],
};

/// File 6, `MNVSMode`: every sprite `mnvsmode.c` draws in the US build.
pub const VS_MODE: SpriteFile = SpriteFile {
    file: 0x06,
    offsets: &[
        0x5EB0, 0x6118, 0x24C8, 0x2748, 0x28E0, 0x2A80, 0x2C20, 0x2EC8, 0x2FC8, 0x3248, 0x3828,
    ],
};

/// File 7, `MNVSOptions`: every sprite `mnvsoptions.c` draws in the US
/// build.
pub const VS_OPTIONS: SpriteFile = SpriteFile {
    file: 0x07,
    offsets: &[
        0x5F60, 0x2668, 0x33D8, 0x3690, 0x3968, 0x3CF8, 0x3FC8, 0x4228,
    ],
};

/// File 8, `MNVSItemSwitch`: every sprite `mnvsitemswitch.c` draws.
pub const VS_ITEM_SWITCH: SpriteFile = SpriteFile {
    file: 0x08,
    offsets: &[
        0x9A8, 0xB20, 0xCE8, 0xEA8, 0xF98, 0x10D0, 0x11E8, 0x13A8, 0x1488, 0x1568, 0x1608, 0x3430,
        0x5E60, 0x63A8,
    ],
};

/// File 18, `MNPlayersGameModes`: `Bonus1BreakTheTargetsText` and
/// `Bonus2BoardThePlatformsText` (the Bonus Practice select's titles).
pub const BONUS_GAME_MODES: SpriteFile = SpriteFile {
    file: 18,
    offsets: &[0xBD8, 0x1058],
};

/// File 23, `MNPlayers1PMode`: the Bonus Practice select's records,
/// `BestTimeText`, `TotalBestTimeText`, `TargetsText`, `PlatformsText`,
/// `Sec` and `CSec`.
pub const BONUS_RECORDS: SpriteFile = SpriteFile {
    file: 23,
    offsets: &[0x12E0, 0x1410, 0x1658, 0x1898, 0x1F48, 0x1FC8],
};

/// File 0: `GameModeText`.
pub const COMMON_GAME_MODE: SpriteFile = SpriteFile {
    file: 0x00,
    offsets: &[0xD240],
};

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
