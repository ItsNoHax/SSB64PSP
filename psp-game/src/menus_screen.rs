//! The options and data menus (`ssb_game::menu`, RE-461): Option, Screen
//! Adjust, Backup Clear, Data, VS Record and Characters.
//!
//! Each scene's sprites come from its own pack in `ssb64-menus.pak`
//! (`ssb_rom::menu_pack`), read when the scene starts and dropped when it
//! ends, as the original loads each scene's files with it; the fonts, the
//! portraits, the fighters and the series emblems stay in the resident
//! pack. Characters' fighter plays its motion's clip under the scene's
//! camera and light, and its series emblem holds its material colour.

use alloc::boxed::Box;

use ssb_game::backup::{Backup, Selections};
use ssb_game::fighter::FighterKind;
use ssb_game::menu::backup_clear::BackupClear;
use ssb_game::menu::characters::{self, CharactersMenu, Motion};
use ssb_game::menu::data::DataMenu;
use ssb_game::menu::option::OptionMenu;
use ssb_game::menu::screen_adjust::ScreenAdjust;
use ssb_game::menu::vs_record::VsRecordMenu;
use ssb_game::menu::{Draw, Pad, Piece, Scene, VIEWPORT};
use ssb_game::results_scene::Camera;
use ssb_psp_runtime::assets::{self, AlignedBuf};
use ssb_psp_runtime::gu::Gpu;
use ssb_psp_runtime::meshdraw;
use ssb_rom::menu_pack::MenuScene;
use ssb_rom::pack::Pack;
use ssb_rom::skeleton::Skeleton;

/// The running menu scene.
pub(crate) enum Active {
    Option(OptionMenu),
    ScreenAdjust(ScreenAdjust),
    BackupClear(BackupClear),
    Data(DataMenu),
    VsRecord(Box<VsRecordMenu>),
    Characters(Box<CharactersMenu>),
}

/// Where the menus leave to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Exit {
    /// `nSCKindTitle`: the port's intro screen.
    Title,
    /// `nSCKindModeSelect`: the port's main menu.
    ModeSelect,
}

/// Characters' fighter pose.
struct FighterModel {
    serial: u32,
    kind: FighterKind,
    object: u32,
    slot: Option<u32>,
    skips_scales: bool,
    skeleton: Skeleton,
    demo: ssb_game::modelpart::DemoParts,
}

/// The menus' state and the running scene's sprite pack.
pub(crate) struct Menus {
    pub active: Option<Active>,
    sprites: Option<AlignedBuf>,
    fighter: Option<Box<FighterModel>>,
    emblem: Option<Box<(ssb_rom::pack::ObjectDesc, ssb_rom::skeleton::EffectMaterialAnimator)>>,
    /// A capture's stand-in for `osGetTime`'s low byte.
    clock: u8,
}

/// What the menus read and write in the session.
pub(crate) struct Host<'h, 'p> {
    pub pack: Option<&'h Pack<'p>>,
    pub pack_path: Option<&'static str>,
    pub backup: &'h mut Backup,
    pub selections: &'h mut Selections,
    /// `dSYAudioSoundQuality`: 1 stereo, 0 mono.
    pub sound_quality: &'h mut u8,
    /// `gSYVideoOffsetLeft`/`Top` as `syVideoSetCenterOffsets` last set them.
    pub video_offsets: &'h mut (i16, i16),
    pub capture: bool,
}

impl Menus {
    pub(crate) fn new() -> Menus {
        Menus {
            active: None,
            sprites: None,
            fighter: None,
            emblem: None,
            clock: 0,
        }
    }

    /// A random byte: the system clock's low byte, or a counter in a capture.
    fn rand(&mut self, capture: bool) -> impl FnMut() -> u8 + '_ {
        move || {
            if capture {
                self.clock = self.clock.wrapping_add(97);
                self.clock
            } else {
                unsafe { psp::sys::sceKernelGetSystemTimeLow() as u8 }
            }
        }
    }

    /// Starts `scene`, entered from `prev`: the previous scene's sprites are
    /// freed before this one's load.
    pub(crate) fn enter(&mut self, scene: Scene, prev: Scene, host: &mut Host<'_, '_>) {
        self.sprites = None;
        self.fighter = None;
        self.emblem = None;
        let (pack_scene, active) = match scene {
            Scene::Option => (
                MenuScene::Option,
                Active::Option(OptionMenu::new(prev, *host.sound_quality, host.backup)),
            ),
            Scene::ScreenAdjust => {
                let (h, v) = *host.video_offsets;
                (MenuScene::ScreenAdjust, Active::ScreenAdjust(ScreenAdjust::new(h, v)))
            }
            Scene::BackupClear => (MenuScene::BackupClear, Active::BackupClear(BackupClear::new())),
            Scene::VsRecord => (
                MenuScene::VsRecord,
                Active::VsRecord(Box::new(VsRecordMenu::new(host.backup))),
            ),
            Scene::Characters => {
                let capture = host.capture;
                let backup: &Backup = host.backup;
                let menu = CharactersMenu::new(backup, &mut self.rand(capture));
                (MenuScene::Characters, Active::Characters(Box::new(menu)))
            }
            // The Data menu; Sound Test is not ported, so leaving for it
            // comes back here with its tab chosen.
            _ => (MenuScene::Data, Active::Data(DataMenu::new(prev, host.backup))),
        };
        self.sprites = host
            .pack_path
            .and_then(|path| assets::load_menu_pack(path, pack_scene).ok());
        if let (Active::Characters(m), Some(p)) = (&active, host.pack) {
            self.emblem = make_emblem(p, m.kind());
            self.fighter = Some(make_fighter(p, &m.fighter));
        }
        self.active = Some(active);
    }

    /// Leaves the menus, freeing the scene's sprites.
    pub(crate) fn leave(&mut self) {
        self.active = None;
        self.sprites = None;
        self.fighter = None;
        self.emblem = None;
    }

    /// One frame of the running scene. `Some` when the menus are left.
    pub(crate) fn frame(&mut self, pad: &Pad, host: &mut Host<'_, '_>) -> Option<Exit> {
        let capture = host.capture;
        let (from, next) = if matches!(self.active, Some(Active::Characters(_))) {
            (Scene::Characters, self.characters_frame(pad, host, capture))
        } else {
            self.menu_frame(pad, host)?
        };
        let next = next?;
        match next {
            Scene::Title | Scene::AutoDemo => {
                self.leave();
                Some(Exit::Title)
            }
            Scene::ModeSelect => {
                self.leave();
                Some(Exit::ModeSelect)
            }
            scene => {
                self.enter(scene, from, host);
                None
            }
        }
    }

    /// Every scene's frame but Characters'.
    fn menu_frame(&mut self, pad: &Pad, host: &mut Host<'_, '_>) -> Option<(Scene, Option<Scene>)> {
        Some(match self.active.as_mut()? {
            Active::Option(m) => {
                let next = m.tick(pad, host.backup);
                // `syAudioSetQuality`.
                *host.sound_quality = m.sound_quality();
                (Scene::Option, next)
            }
            Active::ScreenAdjust(m) => {
                let next = m.tick(pad, host.backup);
                *host.video_offsets = m.video_offsets();
                (Scene::ScreenAdjust, next)
            }
            Active::BackupClear(m) => {
                let next = m.tick(pad, host.backup, host.selections);
                if m.apply_options {
                    // `lbBackupApplyOptions`.
                    *host.sound_quality = host.backup.sound_mono_or_stereo;
                    *host.video_offsets = (host.backup.screen_adjust_h, host.backup.screen_adjust_v);
                }
                (Scene::BackupClear, next)
            }
            Active::Data(m) => (Scene::Data, m.tick(pad)),
            Active::VsRecord(m) => (Scene::VsRecord, m.tick(pad, host.backup)),
            Active::Characters(_) => return None,
        })
    }

    /// `mnCharactersFuncRun`, the fighter's clip, then
    /// `mnCharactersFighterProcUpdate`; a status set plays its first frame
    /// at once.
    fn characters_frame(&mut self, pad: &Pad, host: &mut Host<'_, '_>, capture: bool) -> Option<Scene> {
        let mut clock = self.clock;
        let mut rand = || {
            if capture {
                clock = clock.wrapping_add(97);
                clock
            } else {
                unsafe { psp::sys::sceKernelGetSystemTimeLow() as u8 }
            }
        };
        let Some(Active::Characters(m)) = self.active.as_mut() else {
            return None;
        };
        let kind = m.kind();
        let next = m.tick(pad, host.backup, &mut rand);
        let mut anim_end = false;
        if let (Some(p), Some(f)) = (host.pack, self.fighter.as_deref_mut()) {
            if f.serial == m.fighter.serial && f.kind == m.fighter.kind {
                play(p, f);
                f.demo.tick();
                anim_end = f.skeleton.frame() == 0.0;
            }
        }
        m.tick_fighter(anim_end, &mut rand);
        if let Some(p) = host.pack {
            let fresh = self
                .fighter
                .as_ref()
                .is_none_or(|f| f.serial != m.fighter.serial || f.kind != m.fighter.kind);
            if fresh {
                self.fighter = Some(make_fighter(p, &m.fighter));
            }
            if m.kind() != kind {
                self.emblem = make_emblem(p, m.kind());
            }
        }
        self.clock = clock;
        next
    }

    /// The running scene back to front.
    pub(crate) unsafe fn draw(
        &self,
        gpu: &mut Gpu,
        pack: Option<&Pack<'_>>,
        draw_state: &mut meshdraw::DrawState,
        backup: &Backup,
    ) {
        gpu.set_viewport_fullscreen();
        gpu.begin_frame(Some(ssb_engine::renderer::Color::rgba(0, 0, 0, 255)));
        let Some(active) = self.active.as_ref() else {
            return;
        };
        let sprites = self.sprites.as_ref().and_then(|b| Pack::open(b.as_slice()).ok());
        let packs = Packs {
            menu: sprites.as_ref(),
            main: pack,
        };
        gpu.set_viewport_n64(VIEWPORT);
        let mut f = |d: Draw| match d {
            Draw::Sprite(piece) => draw_piece(&packs, draw_state, &piece, None),
            Draw::Tiled { piece, size } => draw_piece(&packs, draw_state, &piece, Some(size)),
            Draw::Fill { rect, rgba } => meshdraw::fill_rect_n64(rect, rgba, draw_state),
            Draw::Fighter => {
                if let (Some(p), Active::Characters(m)) = (pack, active) {
                    draw_fighter(gpu, p, draw_state, m, self.fighter.as_deref());
                }
            }
            Draw::Emblem => {
                if let Some(p) = pack {
                    draw_emblem(gpu, p, draw_state, self.emblem.as_deref());
                }
            }
        };
        match active {
            Active::Option(m) => m.visit(&mut f),
            Active::ScreenAdjust(m) => m.visit(&mut f),
            Active::BackupClear(m) => m.visit(&mut f),
            Active::Data(m) => m.visit(&mut f),
            Active::VsRecord(m) => m.visit(backup, &mut f),
            Active::Characters(m) => m.visit(&mut f),
        }
        gpu.set_viewport_fullscreen();
    }
}

/// The scene's sprite pack, then the resident one.
struct Packs<'a, 'b> {
    menu: Option<&'a Pack<'b>>,
    main: Option<&'a Pack<'b>>,
}

impl<'a, 'b> Packs<'a, 'b> {
    fn sprite(&self, piece: &Piece) -> Option<(&'a Pack<'b>, ssb_rom::pack::SpriteDesc)> {
        [self.menu, self.main].into_iter().flatten().find_map(|p| {
            let s = match piece.lut {
                Some(lut) => p.sprite_lut(piece.file, piece.offset, lut),
                None => p.sprite(piece.file, piece.offset),
            }?;
            Some((p, s))
        })
    }
}

/// One `SObj` through `lbCommonDrawSObjAttr`; a wrapping one over `size`.
unsafe fn draw_piece(packs: &Packs<'_, '_>, draw_state: &mut meshdraw::DrawState, piece: &Piece, size: Option<[f32; 2]>) {
    const SP_FASTCOPY: u16 = 0x0020;
    let Some((p, s)) = packs.sprite(piece) else {
        return;
    };
    let [r, g, b, a] = s.color;
    let prim = piece.prim.map_or([r, g, b, a], |c| [c[0], c[1], c[2], a]);
    let attr = if piece.transparent {
        (s.attr & !SP_FASTCOPY) | ssb_rom::sprite::SP_TRANSPARENT
    } else {
        s.attr & !ssb_rom::sprite::SP_TRANSPARENT
    };
    let d = meshdraw::SObjDraw {
        x: piece.x,
        y: piece.y,
        scale: 1.0,
        prim,
        env: piece.env,
        solid: false,
        attr,
    };
    match size {
        Some(size) => meshdraw::draw_sprite_tiled(p, &s, &d, size, draw_state),
        None => meshdraw::draw_sprite(p, &s, &d, draw_state),
    }
}

/// The fighter's clip slot: a battle status's, or a demo status's.
fn clip(motion: Motion) -> (Option<u32>, usize, bool) {
    match motion {
        Motion::Null => (None, 0, false),
        Motion::Status(s) => (Some(s.anim_slot() as u32), 0, false),
        Motion::Demo(d) => (Some((ssb_rom::anim::SLOT_WIN1 + d.index()) as u32), 1 + d.index(), true),
    }
}

/// `ftManagerMakeFighter` or `ftMainSetStatus`: the clip from its first
/// frame, played at once.
fn make_fighter(p: &Pack<'_>, f: &characters::Fighter) -> Box<FighterModel> {
    let (slot, row, demo) = clip(f.motion);
    let mut m = Box::new(FighterModel {
        serial: f.serial,
        kind: f.kind,
        object: ssb_psp_runtime::scene::fighter_object(p, f.kind as u32).unwrap_or(u32::MAX),
        slot: None,
        skips_scales: match f.motion {
            Motion::Demo(d) => d.skips_translate_scales(f.kind),
            _ => false,
        },
        skeleton: Skeleton::new(),
        // A battle status's model and texture part events are not run
        // (RE-461): those statuses show the make's parts.
        demo: ssb_game::modelpart::DemoParts::start(f.kind, if demo { row } else { 0 }),
    });
    if let Some(anim) = slot.and_then(|slot| p.fighter_anim(f.kind as u32, slot)) {
        m.skeleton.start(p, &anim, 0.0, 1.0);
        m.slot = slot;
        play(p, &mut m);
    }
    m
}

/// One frame of the clip, through the fighter's translation scales.
fn play(p: &Pack<'_>, m: &mut FighterModel) {
    let Some(anim) = m.slot.and_then(|slot| p.fighter_anim(m.kind as u32, slot)) else {
        return;
    };
    let Some(script) = p.anim_script(&anim) else {
        return;
    };
    let scales = if m.skips_scales {
        None
    } else {
        p.fighter_translate_scales(m.kind as u32)
    };
    let first_node = p.object(m.object).map_or(0, |o| o.first_node);
    let _ = m.skeleton.tick_scaled(script, scales, first_node);
}

/// `mnCharactersMakeEmblem`: the series tree, its material scripts started
/// at frame 4 and played once.
fn make_emblem(
    p: &Pack<'_>,
    kind: FighterKind,
) -> Option<Box<(ssb_rom::pack::ObjectDesc, ssb_rom::skeleton::EffectMaterialAnimator)>> {
    let series = ssb_rom::effect::EMBLEM_BY_FIGHTER.get(kind as usize)?;
    let desc = ssb_rom::effect::EMBLEMS.get(usize::from(*series))?;
    let object = ssb_psp_runtime::scene::object_keyed(p, (ssb_rom::effect::EMBLEM_FILE, desc.dobjdesc))?;
    let mut mats = ssb_rom::skeleton::EffectMaterialAnimator::new();
    let prims = (0..object.node_count)
        .filter_map(|n| p.node(object.first_node + n))
        .filter_map(|node| p.mesh(node.mesh))
        .flat_map(|m| (0..m.prim_count).map(move |j| m.first_prim + j))
        .filter_map(|i| p.prim(i))
        .map(|prim| prim.mat_anim);
    mats.start_at(p, prims, characters::EMBLEM_COLOR_FRAME);
    mats.tick(p);
    Some(Box::new((object, mats)))
}

fn camera_view(gpu: &mut Gpu, draw_state: &mut meshdraw::DrawState, cam: &Camera) {
    gpu.set_viewport_n64(cam.viewport);
    gpu.set_perspective(cam.fovy, cam.aspect, cam.near, cam.far);
    gpu.reset_modelview();
    draw_state.begin_frame();
    gpu.set_view(&ssb_engine::math::Mat4::look_at(cam.eye, cam.at, cam.up));
}

/// `mnCharactersMakeEmblemCamera`'s emblem, lit by the scene's fighter
/// light.
unsafe fn draw_emblem(
    gpu: &mut Gpu,
    p: &Pack<'_>,
    draw_state: &mut meshdraw::DrawState,
    emblem: Option<&(ssb_rom::pack::ObjectDesc, ssb_rom::skeleton::EffectMaterialAnimator)>,
) {
    let Some((object, mats)) = emblem else {
        return;
    };
    camera_view(gpu, draw_state, &characters::EMBLEM_CAMERA);
    let t = characters::EMBLEM_TRANSLATE;
    let k = meshdraw::MODEL_SCALE;
    let s = characters::EMBLEM_SCALE;
    gpu.model_transform_xyz([t.x, t.y, t.z], [0.0; 3], [s * k, s * k, k]);
    let base = gpu.model_matrix();
    let posed = crate::results_screen::rooted_rest_pose(p, object);
    draw_state.configure_fighter_light(characters::LIGHT_ANGLE);
    meshdraw::draw_object_posed(p, object, &base, &posed[..object.node_count as usize], None, draw_state, None, Some(mats), 0);
    draw_state.finish_fighter_light();
    gpu.set_viewport_n64(VIEWPORT);
}

/// `mnCharactersMakeFighterCamera`'s fighter at (0, -100, 0), turned by
/// its `rotate.y`, at its select scale.
unsafe fn draw_fighter(
    gpu: &mut Gpu,
    p: &Pack<'_>,
    draw_state: &mut meshdraw::DrawState,
    menu: &CharactersMenu,
    model: Option<&FighterModel>,
) {
    let Some(m) = model else {
        return;
    };
    let Some(obj) = p.object(m.object) else {
        return;
    };
    camera_view(gpu, draw_state, &menu.camera);
    let mut posed = [ssb_rom::scene::Mat4::IDENTITY; ssb_rom::skeleton::MAX_NODES];
    let n = m.skeleton.compose(p, &obj, &mut posed);
    gpu.model_transform(
        characters::FIGHTER_POSITION,
        [0.0, menu.fighter.rotate_y, 0.0],
        meshdraw::MODEL_SCALE * menu.fighter.scale(),
    );
    let base = gpu.model_matrix();
    draw_state.configure_fighter_light(characters::LIGHT_ANGLE);
    let parts = m.demo.parts.draw_parts();
    meshdraw::draw_fighter_posed(
        p,
        &obj,
        &base,
        &posed[..n],
        draw_state,
        meshdraw::Look {
            costume: 0,
            textures: m.demo.parts.draw_textures(),
            parts: parts.as_ref().map(|p| &p[..]),
            accessory_before: m.kind == FighterKind::Purin,
        },
    );
    draw_state.finish_fighter_light();
    gpu.set_viewport_n64(VIEWPORT);
}
