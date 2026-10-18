//! The sprite manifests (`reloc_data.us.h` offsets, in the manifests'
//! order) that `ssb-tablegen` indexes the decomp's `d*` sprite tables by.
//! `sprite.rs` wraps each in its `SpriteFile`; `ssb-tablegen` compiles this
//! file in so that the generators and the pack read one list.

/// File 82, `IFCommonGameStatus` (`gGMCommonFiles[1]`): `OrangeLetterG`,
/// `...O`, `OrangeExclamationMark`, the blue letters `T I M E U P S A G`,
/// `Rod`, `Frame`, `RodShadow`, then the lamps `{Red,Yellow,Blue}Dim`,
/// `...Light` and `...Contour`.
pub const GAME_STATUS: &[u32] = &[
    0x4D78, 0xA730, 0xC370, 0xE4A8, 0xF740, 0x127E0, 0x144E0, 0x16EB8, 0x18FE8, 0x1B5F8, 0x1DE68,
    0x20788, 0x20990, 0x21760, 0x21878, 0x21950, 0x21A10, 0x21BA8, 0x22128, 0x22588, 0x22F18,
    0x23A28, 0x24620, 0x25290,
];

/// File 37, `IFCommonAnnounceCommon` (`gGMCommonFiles[7]`): `LetterA` to
/// `LetterZ`, then `SymbolExclaim` and `SymbolPeriod`.
pub const ANNOUNCE_COMMON: &[u32] = &[
    0x5E0, 0x9A8, 0xD80, 0x1268, 0x1628, 0x1A00, 0x1F08, 0x2408, 0x26B8, 0x2A90, 0x2F98, 0x3358,
    0x3980, 0x3E88, 0x44B0, 0x4890, 0x4F10, 0x5418, 0x57F0, 0x5BD0, 0x60D8, 0x65D8, 0x6C00, 0x7108,
    0x7608, 0x7AE8, 0x7D98, 0x7E50,
];

/// File 197, `IFCommonBattlePause` (`gGMCommonFiles[5]`): `PlayerNum1P` to
/// `...4P`, then the decals `Pause`, `Plus`, `Reset`, `SmashBall`, `Retry`,
/// `AButton`, `BButton`, `ZTrigger`, `RTrigger`, `Arrows`, `ControlStick`
/// and `LTrigger`.
pub const BATTLE_PAUSE: &[u32] = &[
    0x78, 0x138, 0x1F8, 0x2B8, 0x438, 0x4D8, 0x610, 0x6D8, 0x828, 0x958, 0xA88, 0xBD8, 0xCF8,
    0x1538, 0x17A8, 0x18C8,
];
