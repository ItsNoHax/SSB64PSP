//! The stage select's presentation, from `mn/mnmaps/mnmaps.c` (RE-419):
//! the stone wallpaper (`mnMapsMakeWallpaper`), the preview's backing,
//! tiles and wallpaper (`mnMapsMakePreviewWallpaper`), the preview's stage
//! model and camera (`mnMapsMakeModel`, `mnMapsMakePreviewCamera`), the
//! icons (`mnMapsMakeIcons`), the cursor (`mnMapsMakeCursor`), the wooden
//! plaque (`mnMapsMakePlaque`), the labels (`mnMapsMakeLabels`) and the
//! stage's name and emblem (`mnMapsMakeNameAndEmblem`).
//!
//! [`Layer::tick`] runs after [`StageSelect::tick`]: a moved cursor remakes
//! the preview (`mnMapsMakePreview`), and the preview camera's thread
//! (`mnMapsPreviewCameraThreadUpdate`) bobs its target. [`Layer::visit`]
//! yields what draws, back to front in the cameras' order: the wallpaper
//! (priority 80), the preview wallpaper (70), the model (65), the icons
//! (60), the cursor (50), the plaque (40), the labels (30), then the name
//! and emblem (20). Every camera's viewport is `(10, 10)` to `(310, 230)`.
//! The sprites name `dMNMapsFileIDs`' files; the PSP layer looks each up in
//! the pack.
//!
//! The US build's `mnMapsMakeSubtitle` is empty, so no subtitle is drawn.

use ssb_engine::math::{self, Vec3};

use crate::results_scene::Camera;
use crate::stage_select::{gkind, slot_gkind, StageSelect, RANDOM_SLOT};
use crate::wallpaper;

/// `llMNMapsFileID`.
pub const MN_MAPS: u32 = 30;
/// `llMNSelectCommonFileID`.
pub const SELECT_COMMON: u32 = 21;
/// `llFTEmblemSpritesFileID`.
pub const EMBLEM_SPRITES: u32 = 20;

/// `llMNSelectCommonStoneBackgroundSprite`.
const STONE_BACKGROUND: u32 = 0x440;
/// `llMNMaps*` sprites (`reloc_data.us.h`).
const CURSOR: u32 = 0x1AB8;
const QUESTION_MARK: u32 = 0x1DD8;
const STAGE_SELECT_TEXT: u32 = 0x26A0;
const WOODEN_CIRCLE: u32 = 0x3840;
const PLATE_RIGHT: u32 = 0x3C68;
const PLATE_MIDDLE: u32 = 0x3D68;
const PLATE_LEFT: u32 = 0x3FA8;
const TILES: u32 = 0xC728;
const RANDOM_SMALL: u32 = 0xCB10;
const RANDOM_BIG: u32 = 0xDE30;

/// `mnMapsMakeIcons`' `offsets`, by `GRKind`.
const ICONS: [u32; 9] = [
    0x4D88, // PeachsCastle
    0x5B68, // SectorZ
    0x6948, // CongoJungle
    0x7728, // PlanetZebes
    0x8508, // HyruleCastle
    0x92E8, // YoshisIsland
    0xBC88, // DreamLand
    0xA0C8, // SaffronCity
    0xAEA8, // MushroomKingdom
];

/// `mnMapsMakeName`'s `offsets`, by `GRKind`.
const NAMES: [u32; 9] = [
    0x1F8,  // PeachsCastleText
    0x438,  // SectorZText
    0x678,  // CongoJungleText
    0x8B8,  // PlanetZebesText
    0xB10,  // HyruleCastleText
    0xD58,  // YoshisIslandText
    0x1418, // DreamLandText
    0xF98,  // SaffronCityText
    0x11D8, // MushroomKingdomText
];

/// `mnMapsMakeEmblem`'s `offsets` into `FTEmblemSprites`, by `GRKind`:
/// Mario, Fox, Donkey, Metroid, Zelda, Yoshi, Kirby, PMonsters and Mario.
const EMBLEMS: [u32; 9] = [
    0x618, 0x1938, 0xC78, 0x12D8, 0x25F8, 0x2C58, 0x1F98, 0x3918, 0x618,
];

/// `mnMapsSetLogoPosition`'s `positions`, offsets from the plaque at
/// (189, 124).
const LOGO_OFFSETS: [(f32, f32); 9] = [
    (3.0, 19.0),
    (3.0, 19.0),
    (3.0, 20.0),
    (2.0, 20.0),
    (3.0, 17.0),
    (-1.0, 19.0),
    (1.0, 20.0),
    (1.0, 20.0),
    (3.0, 19.0),
];

/// `mnMapsMakeEmblem`'s brown primitive, for the emblem and question mark.
const EMBLEM_PRIM: [u8; 3] = [0x5C, 0x22, 0x00];

/// Every camera's `syRdpSetViewport`.
pub const VIEWPORT: [f32; 4] = [10.0, 10.0, 310.0, 230.0];

/// `mnMapsSetPreviewCameraPosition`'s target per `GRKind`.
const PREVIEW_AT: [(f32, f32); 9] = [
    (1700.0, 1800.0),
    (1600.0, 1600.0),
    (1600.0, 1600.0),
    (1600.0, 1600.0),
    (1600.0, 1500.0),
    (1600.0, 1600.0),
    (1600.0, 1500.0),
    (1600.0, 1600.0),
    (1200.0, 1600.0),
];

/// The preview camera's eye.
pub const PREVIEW_EYE: Vec3 = Vec3 {
    x: -3000.0,
    y: 3000.0,
    z: 9000.0,
};

/// `mnMapsMakeLayer`'s `scales`: the root `DObj`'s scale, per `GRKind`.
const MODEL_SCALES: [f32; 9] = [0.5, 0.2, 0.6, 0.5, 0.3, 0.6, 0.5, 0.4, 0.2];

/// Where a sprite comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// A file's sprite at an offset.
    File { file: u32, offset: u32 },
    /// The VS stage's `MPGroundData.wallpaper`.
    StageWallpaper(u8),
}

/// One `SObj`: its sprite, top-left corner on the 320 x 240 screen, scale
/// and colours.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Piece {
    pub source: Source,
    pub x: f32,
    pub y: f32,
    pub scale: f32,
    /// `sprite.red`, `.green`, `.blue`; `None` keeps the ROM's.
    pub prim: Option<[u8; 3]>,
    /// `sobj->envcolor`; black where the source leaves it unset.
    pub env: [u8; 3],
    /// `SP_TRANSPARENT` set (and `SP_FASTCOPY` cleared).
    pub transparent: bool,
}

impl Piece {
    fn at(file: u32, offset: u32, x: f32, y: f32) -> Self {
        Piece {
            source: Source::File { file, offset },
            x,
            y,
            scale: 1.0,
            prim: None,
            env: [0; 3],
            transparent: false,
        }
    }

    fn transparent(mut self) -> Self {
        self.transparent = true;
        self
    }

    fn prim(mut self, prim: [u8; 3]) -> Self {
        self.prim = Some(prim);
        self
    }
}

/// What draws, back to front.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Draw {
    /// `lbCommonSetSpriteScissor`-like viewport, `[x0, y0, x1, y1]`.
    Scissor([f32; 4]),
    /// A wrapping `SObj` over `size` from the piece's corner.
    Tiled {
        piece: Piece,
        size: [f32; 2],
    },
    /// A one-cycle `gDPFillRectangle` over `[x0, y0, x1, y1)`, blended by
    /// the alpha (`G_RM_AA_XLU_SURF`).
    Fill {
        rect: [f32; 4],
        rgba: [u8; 4],
    },
    Sprite(Piece),
    /// The preview's stage model under [`Layer::camera`].
    Model,
}

/// How `mnMapsMakeModel` hides part of a preview.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hide {
    None,
    /// Saffron City: layer 3's root's child's child.
    GrandchildOfLayer3,
    /// Yoshi's Island: layer 0's 15th and 17th `DObj`s in tree order, the
    /// root being the first.
    Layer0TreeIndices([u32; 2]),
}

/// The preview's model: which stage, the root's scale and what it hides.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Model {
    pub gkind: u8,
    pub scale: f32,
    pub hide: Hide,
}

/// `mnMaps`' presentation state.
#[derive(Debug, Clone, PartialEq)]
pub struct Layer {
    /// `sMNMapsIsTrainingMode`.
    pub is_training: bool,
    /// The slot the name, cursor and preview were last made for.
    slot: usize,
    /// Bumped each time the preview is made again, so the host restarts
    /// the model's animation.
    pub preview_serial: u32,
    /// The preview camera's target.
    at: Vec3,
    /// The target's `y` when the camera's thread first ran.
    bob_base_y: f32,
    /// The thread's angle, in degrees.
    bob_deg: f32,
}

/// `mnMapsSetPreviewCameraPosition`'s target; the random slot shows Peach's
/// Castle's.
fn preview_at(gkind: u8) -> Vec3 {
    let k = if gkind == gkind::RANDOM {
        gkind::CASTLE
    } else {
        gkind
    };
    let (x, y) = PREVIEW_AT[usize::from(k).min(8)];
    Vec3::new(x, y, 0.0)
}

impl Layer {
    /// `mnMapsFuncStart`: the preview of the cursor's slot, and the camera
    /// on its stage.
    pub fn new(select: &StageSelect, is_training: bool) -> Self {
        let at = preview_at(slot_gkind(select.cursor_slot));
        Layer {
            is_training,
            slot: select.cursor_slot,
            preview_serial: 0,
            at,
            bob_base_y: at.y,
            bob_deg: 0.0,
        }
    }

    /// One tick after [`StageSelect::tick`]: a cursor on a new slot remakes
    /// the name, emblem and preview; `mnMapsMakePreview` moves the camera
    /// for any stage but the random slot. Then the camera's thread sets
    /// the target's `y` to `sin(deg) * 40` about the `y` it read when it
    /// first ran, and advances `deg` by 2. That base is never read again,
    /// so a later stage's own target height is overwritten at once.
    pub fn tick(&mut self, select: &StageSelect) {
        if select.cursor_slot != self.slot {
            self.slot = select.cursor_slot;
            self.preview_serial = self.preview_serial.wrapping_add(1);
            let k = slot_gkind(self.slot);
            if k != gkind::RANDOM {
                self.at = preview_at(k);
            }
        }
        let (sin, _) = math::sin_cos(self.bob_deg.to_radians());
        self.at.y = sin * 40.0 + self.bob_base_y;
        self.bob_deg = if self.bob_deg + 2.0 > 360.0 {
            self.bob_deg + 2.0 - 360.0
        } else {
            self.bob_deg + 2.0
        };
    }

    /// The preview's `GRKind` (`gkind::RANDOM` for the random slot).
    pub fn preview_gkind(&self) -> u8 {
        slot_gkind(self.slot)
    }

    /// `mnMapsMakePreviewCamera`: `dGCPerspDefault`'s 30 degrees and 4:3,
    /// near 100, and `far` raised to 16384.
    pub fn camera(&self) -> Camera {
        Camera {
            eye: PREVIEW_EYE,
            at: self.at,
            up: Vec3::new(0.0, 1.0, 0.0),
            fovy: 30.0,
            aspect: 4.0 / 3.0,
            near: 100.0,
            far: 16384.0,
            viewport: VIEWPORT,
        }
    }

    /// The preview's model: none for the random slot.
    pub fn model(&self) -> Option<Model> {
        let k = self.preview_gkind();
        if k == gkind::RANDOM || k > gkind::INISHIE {
            return None;
        }
        let hide = match k {
            gkind::YAMABUKI => Hide::GrandchildOfLayer3,
            gkind::YOSTER => Hide::Layer0TreeIndices([15, 17]),
            _ => Hide::None,
        };
        Some(Model {
            gkind: k,
            scale: MODEL_SCALES[usize::from(k)],
            hide,
        })
    }

    /// The screen back to front.
    pub fn visit(&self, select: &StageSelect, mut f: impl FnMut(Draw)) {
        f(Draw::Scissor(VIEWPORT));
        // Camera 80, link 0: the stone tile, wrapped over 300 x 220.
        f(Draw::Tiled {
            piece: Piece::at(SELECT_COMMON, STONE_BACKGROUND, 10.0, 10.0),
            size: [300.0, 220.0],
        });

        // Camera 70, link 7: `mnMapsPreviewWallpaperProcDisplay`'s shade,
        // then the tiles and the wallpaper or random picture.
        f(Draw::Fill {
            rect: [43.0, 130.0, 152.0, 211.0],
            rgba: [0x00, 0x00, 0x00, 0x73],
        });
        let mut x = 43.0;
        while x < 155.0 {
            f(Draw::Sprite(Piece::at(MN_MAPS, TILES, x, 130.0)));
            x += 16.0;
        }
        let k = self.preview_gkind();
        if k == gkind::RANDOM {
            f(Draw::Sprite(Piece::at(MN_MAPS, RANDOM_BIG, 40.0, 127.0)));
        } else {
            let source = match wallpaper::training_wallpaper(k) {
                Some(t) if self.is_training => Source::File {
                    file: t.file,
                    offset: wallpaper::TRAINING_WALLPAPER_SPRITE,
                },
                _ => Source::StageWallpaper(k),
            };
            f(Draw::Sprite(Piece {
                source,
                x: 40.0,
                y: 127.0,
                scale: 0.37,
                prim: None,
                env: [0; 3],
                transparent: false,
            }));
        }

        // Camera 65, link 3: the model.
        if self.model().is_some() {
            f(Draw::Model);
            f(Draw::Scissor(VIEWPORT));
        }

        // Camera 60, link 1: an icon per unlocked slot.
        for slot in 0..=RANDOM_SLOT {
            let k = slot_gkind(slot);
            if select.is_locked(k) {
                continue;
            }
            let offset = if slot == RANDOM_SLOT {
                RANDOM_SMALL
            } else {
                ICONS[usize::from(k)]
            };
            let x = (slot * 50) as f32;
            let (x, y) = if slot < 5 {
                (x + 30.0, 30.0)
            } else {
                (x - 220.0, 68.0)
            };
            f(Draw::Sprite(Piece::at(MN_MAPS, offset, x, y)));
        }

        // Camera 50, link 5: the cursor, red.
        let (cx, cy) = cursor_position(self.slot);
        f(Draw::Sprite(
            Piece::at(MN_MAPS, CURSOR, cx, cy)
                .transparent()
                .prim([0xFF, 0x00, 0x00]),
        ));

        // Camera 40, link 6: the wooden plaque.
        f(Draw::Sprite(
            Piece::at(MN_MAPS, WOODEN_CIRCLE, 189.0, 124.0).transparent(),
        ));

        // Camera 30, link 4: `mnMapsLabelsProcDisplay`'s two fills, then
        // "Stage Select" and the name plate.
        f(Draw::Fill {
            rect: [160.0, 128.0, 320.0, 134.0],
            rgba: [0x57, 0x60, 0x88, 0xFF],
        });
        f(Draw::Fill {
            rect: [194.0, 189.0, 268.0, 193.0],
            rgba: [0x00, 0x00, 0x00, 0x33],
        });
        f(Draw::Sprite(
            Piece::at(MN_MAPS, STAGE_SELECT_TEXT, 172.0, 122.0)
                .transparent()
                .prim([0xAF, 0xB1, 0xCC]),
        ));
        f(Draw::Sprite(
            Piece::at(MN_MAPS, PLATE_LEFT, 174.0, 191.0).transparent(),
        ));
        let mut x = 186.0;
        while x < 262.0 {
            // The middle pieces keep the file's `SP_FASTCOPY`.
            f(Draw::Sprite(Piece::at(MN_MAPS, PLATE_MIDDLE, x, 191.0)));
            x += 4.0;
        }
        f(Draw::Sprite(
            Piece::at(MN_MAPS, PLATE_RIGHT, 262.0, 191.0).transparent(),
        ));

        // Camera 20, link 2: the emblem (or question mark) and the name.
        if k == gkind::RANDOM {
            f(Draw::Sprite(
                Piece::at(MN_MAPS, QUESTION_MARK, 223.0, 144.0)
                    .transparent()
                    .prim(EMBLEM_PRIM),
            ));
        } else {
            let i = usize::from(k);
            let (dx, dy) = LOGO_OFFSETS[i];
            f(Draw::Sprite(
                Piece::at(EMBLEM_SPRITES, EMBLEMS[i], dx + 189.0, dy + 124.0)
                    .transparent()
                    .prim(EMBLEM_PRIM),
            ));
            // `mnMapsSetNamePosition`: the US build puts every name at
            // (183, 196).
            f(Draw::Sprite(
                Piece::at(MN_MAPS, NAMES[i], 183.0, 196.0)
                    .transparent()
                    .prim([0x00, 0x00, 0x00]),
            ));
        }
    }
}

/// `mnMapsSetCursorPosition`.
pub fn cursor_position(slot: usize) -> (f32, f32) {
    if slot < 5 {
        ((slot * 50 + 23) as f32, 23.0)
    } else {
        ((slot * 50) as f32 - 250.0 + 23.0, 61.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ssb_engine::input::{ControllerState, N64Buttons};

    fn draws(layer: &Layer, select: &StageSelect) -> Vec<Draw> {
        let mut out = Vec::new();
        layer.visit(select, |d| out.push(d));
        out
    }

    fn sprites(layer: &Layer, select: &StageSelect) -> Vec<Piece> {
        draws(layer, select)
            .into_iter()
            .filter_map(|d| match d {
                Draw::Sprite(p) => Some(p),
                _ => None,
            })
            .collect()
    }

    fn file_piece(pieces: &[Piece], offset: u32) -> Vec<Piece> {
        pieces
            .iter()
            .copied()
            .filter(|p| matches!(p.source, Source::File { offset: o, .. } if o == offset))
            .collect()
    }

    fn tap(select: &mut StageSelect, layer: &mut Layer, buttons: u16) {
        let pad = ControllerState {
            buttons: N64Buttons(buttons),
            stick_x: 0,
            stick_y: 0,
            connected: true,
        };
        select.tick(pad, N64Buttons(buttons));
        layer.tick(select);
        let idle = ControllerState {
            buttons: N64Buttons(0),
            stick_x: 0,
            stick_y: 0,
            connected: true,
        };
        select.tick(idle, N64Buttons(0));
        layer.tick(select);
    }

    fn settled(gkind: u8) -> (StageSelect, Layer) {
        let mut select = StageSelect::new(gkind, 0);
        let mut layer = Layer::new(&select, true);
        for _ in 0..10 {
            select.tick(ControllerState::default(), N64Buttons(0));
            layer.tick(&select);
        }
        (select, layer)
    }

    #[test]
    fn the_icons_skip_the_locked_stage() {
        let select = StageSelect::new(gkind::CASTLE, 0);
        let layer = Layer::new(&select, true);
        let pieces = sprites(&layer, &select);
        let icons: Vec<(f32, f32)> = pieces
            .iter()
            .filter(|p| {
                matches!(p.source, Source::File { offset, .. }
                    if ICONS.contains(&offset) || offset == RANDOM_SMALL)
            })
            .map(|p| (p.x, p.y))
            .collect();
        // Mushroom Kingdom (slot 4) is locked with no save data.
        assert_eq!(
            icons,
            [
                (30.0, 30.0),
                (80.0, 30.0),
                (130.0, 30.0),
                (180.0, 30.0),
                (30.0, 68.0),
                (80.0, 68.0),
                (130.0, 68.0),
                (180.0, 68.0),
                (230.0, 68.0),
            ]
        );
        let unlocked = StageSelect::new(gkind::CASTLE, crate::stage_select::UNLOCK_MASK_INISHIE);
        let n = sprites(&layer, &unlocked)
            .iter()
            .filter(|p| matches!(p.source, Source::File { offset, .. } if ICONS.contains(&offset)))
            .count();
        assert_eq!(n, 9);
    }

    #[test]
    fn the_cursor_name_and_emblem_follow_the_slot() {
        let (mut select, mut layer) = settled(gkind::CASTLE);
        let p = sprites(&layer, &select);
        let cursor = file_piece(&p, CURSOR)[0];
        assert_eq!(
            (cursor.x, cursor.y, cursor.prim),
            (23.0, 23.0, Some([0xFF, 0, 0]))
        );
        assert_eq!(file_piece(&p, NAMES[0])[0].x, 183.0);
        let emblem = p
            .iter()
            .find(|p| {
                p.source
                    == Source::File {
                        file: EMBLEM_SPRITES,
                        offset: 0x618,
                    }
            })
            .unwrap();
        assert_eq!((emblem.x, emblem.y), (192.0, 143.0));

        // Down to Yoshi's Island: the cursor's second row, the Yoshi emblem.
        tap(&mut select, &mut layer, N64Buttons::D_DOWN);
        assert_eq!(select.cursor_slot, 5);
        let p = sprites(&layer, &select);
        assert_eq!(
            (file_piece(&p, CURSOR)[0].x, file_piece(&p, CURSOR)[0].y),
            (23.0, 61.0)
        );
        let emblem = p
            .iter()
            .find(|p| {
                matches!(
                    p.source,
                    Source::File {
                        file: EMBLEM_SPRITES,
                        ..
                    }
                )
            })
            .unwrap();
        assert_eq!(
            emblem.source,
            Source::File {
                file: EMBLEM_SPRITES,
                offset: 0x2C58
            }
        );
        assert_eq!((emblem.x, emblem.y), (188.0, 143.0));
        assert_eq!(file_piece(&p, NAMES[5]).len(), 1);
    }

    #[test]
    fn the_random_slot_shows_its_picture_and_question_mark_without_a_model() {
        let (mut select, mut layer) = settled(gkind::YOSTER);
        tap(&mut select, &mut layer, N64Buttons::D_LEFT);
        assert_eq!(select.cursor_slot, RANDOM_SLOT);
        let p = sprites(&layer, &select);
        assert_eq!(file_piece(&p, RANDOM_BIG).len(), 1);
        let q = file_piece(&p, QUESTION_MARK)[0];
        assert_eq!((q.x, q.y, q.prim), (223.0, 144.0, Some(EMBLEM_PRIM)));
        assert!(NAMES.iter().all(|&n| file_piece(&p, n).is_empty()));
        assert_eq!(layer.model(), None);
        assert!(!draws(&layer, &select).contains(&Draw::Model));
    }

    #[test]
    fn the_preview_wallpaper_is_trainings_or_the_stages() {
        let select = StageSelect::new(gkind::SECTOR, 0);
        let training = Layer::new(&select, true);
        let wp = sprites(&training, &select)
            .into_iter()
            .find(|p| p.scale == 0.37)
            .unwrap();
        assert_eq!(
            wp.source,
            Source::File {
                file: 0x1A,
                offset: wallpaper::TRAINING_WALLPAPER_SPRITE
            }
        );
        assert_eq!((wp.x, wp.y), (40.0, 127.0));
        let vs = Layer::new(&select, false);
        let wp = sprites(&vs, &select)
            .into_iter()
            .find(|p| p.scale == 0.37)
            .unwrap();
        assert_eq!(wp.source, Source::StageWallpaper(gkind::SECTOR));
    }

    #[test]
    fn the_labels_draw_the_plate_in_twenty_one_pieces() {
        let select = StageSelect::new(gkind::CASTLE, 0);
        let layer = Layer::new(&select, true);
        let p = sprites(&layer, &select);
        let middles: Vec<f32> = file_piece(&p, PLATE_MIDDLE).iter().map(|p| p.x).collect();
        assert_eq!(middles.len(), 19);
        assert_eq!((middles[0], middles[18]), (186.0, 258.0));
        assert!(!file_piece(&p, PLATE_MIDDLE)[0].transparent);
        assert!(file_piece(&p, PLATE_LEFT)[0].transparent);
        assert_eq!(file_piece(&p, TILES).len(), 7);
    }

    #[test]
    fn the_draw_order_follows_the_camera_priorities() {
        let select = StageSelect::new(gkind::CASTLE, 0);
        let layer = Layer::new(&select, true);
        let d = draws(&layer, &select);
        let at = |pred: &dyn Fn(&Draw) -> bool| d.iter().position(pred).unwrap();
        let stone = at(&|x| matches!(x, Draw::Tiled { .. }));
        let model = at(&|x| *x == Draw::Model);
        let icon = at(
            &|x| matches!(x, Draw::Sprite(p) if p.source == Source::File { file: MN_MAPS, offset: ICONS[0] }),
        );
        let cursor = at(
            &|x| matches!(x, Draw::Sprite(p) if p.source == Source::File { file: MN_MAPS, offset: CURSOR }),
        );
        let plaque = at(
            &|x| matches!(x, Draw::Sprite(p) if p.source == Source::File { file: MN_MAPS, offset: WOODEN_CIRCLE }),
        );
        let label = at(
            &|x| matches!(x, Draw::Sprite(p) if p.source == Source::File { file: MN_MAPS, offset: STAGE_SELECT_TEXT }),
        );
        let name = at(
            &|x| matches!(x, Draw::Sprite(p) if p.source == Source::File { file: MN_MAPS, offset: NAMES[0] }),
        );
        assert!(stone < model && model < icon && icon < cursor && cursor < plaque);
        assert!(plaque < label && label < name);
    }

    #[test]
    fn the_preview_camera_bobs_about_the_first_stages_height() {
        let select = StageSelect::new(gkind::CASTLE, 0);
        let mut layer = Layer::new(&select, true);
        assert_eq!(layer.camera().at, Vec3::new(1700.0, 1800.0, 0.0));
        layer.tick(&select);
        assert_eq!(layer.camera().at.y, 1800.0, "sin 0 on the first run");
        for _ in 0..44 {
            layer.tick(&select);
        }
        // The 46th run is at 90 degrees.
        layer.tick(&select);
        assert!((layer.camera().at.y - 1840.0).abs() < 1e-3);

        // Moving to Hyrule (target 1600, 1500) keeps Peach's 1800 base.
        let (mut select, mut layer) = settled(gkind::CASTLE);
        tap(&mut select, &mut layer, N64Buttons::D_RIGHT);
        tap(&mut select, &mut layer, N64Buttons::D_RIGHT);
        assert_eq!(select.cursor_slot, 2);
        let at = layer.camera().at;
        assert_eq!(at.x, 1600.0);
        assert!((at.y - 1800.0).abs() <= 40.0);
        assert_eq!(layer.model().unwrap().scale, 0.3);
        assert_eq!(layer.preview_serial, 2);
    }

    #[test]
    fn the_model_hides_what_the_source_hides() {
        let select = StageSelect::new(gkind::YAMABUKI, 0);
        let layer = Layer::new(&select, true);
        assert_eq!(layer.model().unwrap().hide, Hide::GrandchildOfLayer3);
        let select = StageSelect::new(gkind::YOSTER, 0);
        let layer = Layer::new(&select, true);
        assert_eq!(
            layer.model().unwrap().hide,
            Hide::Layer0TreeIndices([15, 17])
        );
        assert_eq!(layer.model().unwrap().scale, 0.6);
    }
}
