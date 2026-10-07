//! The N64 logo and the opening movie (`mnStartup`, `mvOpening*`,
//! `ssb_game::opening`, RE-467).
//!
//! The scenes' logic keeps their worlds; this plays and draws them
//! (`ssb_psp_runtime::movie`) with each scene's own pack from
//! `ssb64-menus.pak`, read when the scene starts. A scene whose start
//! waits for the music's tic count ([`ssb_game::opening::Kind::start_tic`])
//! is held: the last picture stays up, drawn again unchanged, until the
//! count reaches it.

use alloc::boxed::Box;

use ssb_game::fighter::FighterKind;
use ssb_game::menu::Scene as MScene;
use ssb_game::opening::movie::{Camera, Head, Object, World};
use ssb_game::opening::{
    clash, cliff, newcomers, portraits, room, run, sector, standoff, startup, yamabuki, yoster,
    Clock, Exit, Kind,
};
use ssb_psp_runtime::gu::Gpu;
use ssb_psp_runtime::meshdraw::DrawState;
use ssb_psp_runtime::movie::{self, Assets, Runtime};
use ssb_rom::pack::Pack;

/// The running scene's logic.
pub(crate) enum Logic {
    Startup(Box<startup::Startup>),
    Room(Box<room::Room>),
    Portraits(Box<portraits::Portraits>),
    Cliff(Box<cliff::Cliff>),
    Run(Box<run::Run>),
    Yamabuki(Box<yamabuki::Yamabuki>),
    Yoster(Box<yoster::Yoster>),
    Sector(Box<sector::Sector>),
    Standoff(Box<standoff::Standoff>),
    Clash(Box<clash::Clash>),
    Newcomers(Box<newcomers::Newcomers>),
}

impl Logic {
    fn world(&self) -> &World {
        match self {
            Logic::Startup(s) => &s.world,
            Logic::Room(s) => &s.world,
            Logic::Portraits(s) => &s.world,
            Logic::Cliff(s) => &s.world,
            Logic::Run(s) => &s.world,
            Logic::Yamabuki(s) => &s.world,
            Logic::Yoster(s) => &s.world,
            Logic::Sector(s) => &s.world,
            Logic::Standoff(s) => &s.world,
            Logic::Clash(s) => &s.world,
            Logic::Newcomers(s) => &s.world,
        }
    }

    fn tick(&mut self, tapped: bool) -> Option<Exit> {
        match self {
            Logic::Startup(s) => s.tick(tapped),
            Logic::Room(s) => s.tick(tapped),
            Logic::Portraits(s) => s.tick(tapped),
            Logic::Cliff(s) => s.tick(tapped),
            Logic::Run(s) => s.tick(tapped),
            Logic::Yamabuki(s) => s.tick(tapped),
            Logic::Yoster(s) => s.tick(tapped),
            Logic::Sector(s) => s.tick(tapped),
            Logic::Standoff(s) => s.tick(tapped),
            Logic::Clash(s) => s.tick(tapped),
            Logic::Newcomers(s) => s.tick(tapped),
        }
    }
}

/// The opening's host state: the music clock, the running scene and the
/// one waiting to start.
pub(crate) struct Opening {
    pub clock: Clock,
    logic: Option<Logic>,
    runtime: Runtime,
    /// A scene whose start waits for [`Clock`].
    pending: Option<Kind>,
    /// `gSCManagerBackupData.fighter_mask`, which the newcomers read; the
    /// eight starters until the host sets the save's.
    pub fighter_mask: u16,
    /// The room's last picture is held in the wallpaper snapshot: the
    /// default camera stopped filling at tic 1037.
    frozen: core::cell::Cell<bool>,
    /// The next room's figures, picked ahead to read their files
    /// ([`room_figures`], RE-476).
    room_figures: Option<(FighterKind, FighterKind)>,
}

impl Opening {
    pub(crate) fn new() -> Opening {
        Opening {
            clock: Clock::default(),
            logic: None,
            runtime: Runtime::new(),
            pending: None,
            fighter_mask: STARTERS_MASK,
            frozen: core::cell::Cell::new(false),
            room_figures: None,
        }
    }

    /// The scene's sprites, cameras and scripts: its files in the pack
    /// (RE-475).
    fn scene_pack(&self) -> Option<&'static Pack<'static>> {
        ssb_psp_runtime::scene_files::pack()
    }
}

/// `LBBACKUP_MASK_FIGHTER` of the eight starters: a new save's mask.
const STARTERS_MASK: u16 = 0x036F;

/// The next room's figures (`mvOpeningRoomInitVars`), picked now if they
/// have not been: the room's file list names them
/// (`scene_load::set_room_figures`), so they can be read before the room
/// starts. The room that starts next takes them. The pick reads the
/// clock's low byte as the N64's does, earlier: a random draw either way.
pub(crate) fn room_figures(o: &mut Opening) -> (FighterKind, FighterKind) {
    let figures = *o
        .room_figures
        .get_or_insert_with(|| room::Room::pick_figures(&mut room_time()));
    crate::scene_load::set_room_figures([figures.0 as u32, figures.1 as u32]);
    figures
}

/// The scene's logic, if it is ported; `fighter_mask` is the backup's.
fn make(o: &mut Opening, kind: Option<Kind>) -> Option<Logic> {
    let fighter_mask = o.fighter_mask;
    Some(match kind {
        None => Logic::Startup(Box::new(startup::Startup::new())),
        Some(Kind::Room) => {
            let (pulled, dropped) = room_figures(o);
            o.room_figures = None;
            Logic::Room(Box::new(room::Room::with_figures(pulled, dropped)))
        }
        Some(Kind::Portraits) => Logic::Portraits(Box::new(portraits::Portraits::new())),
        Some(Kind::Run) => Logic::Run(Box::new(run::Run::new())),
        Some(Kind::Cliff) => Logic::Cliff(Box::new(cliff::Cliff::new())),
        Some(Kind::Yamabuki) => Logic::Yamabuki(Box::new(yamabuki::Yamabuki::new())),
        Some(Kind::Yoster) => Logic::Yoster(Box::new(yoster::Yoster::new())),
        Some(Kind::Sector) => Logic::Sector(Box::new(sector::Sector::new())),
        Some(Kind::Standoff) => Logic::Standoff(Box::new(standoff::Standoff::new())),
        Some(Kind::Clash) => Logic::Clash(Box::new(clash::Clash::new())),
        Some(Kind::Newcomers) => {
            Logic::Newcomers(Box::new(newcomers::Newcomers::new(fighter_mask)))
        }
        _ => return None,
    })
}

/// `syTaskmanSetLoadScene` into `nSCKindStartup` (`kind` `None`) or an
/// opening scene. Returns `false` for a scene this host does not run.
pub(crate) fn start(o: &mut Opening, kind: Option<Kind>) -> bool {
    let Some(logic) = make(o, kind) else {
        return false;
    };
    if kind == Some(Kind::Room) {
        // `mvOpeningRoomFuncStart`'s `sySchedulerSetTicCount(0)`.
        o.clock.reset();
    }
    o.runtime = Runtime::new();
    o.logic = Some(logic);
    o.pending = None;
    o.frozen.set(false);
    true
}

/// Reads opening scene `kind`'s files as it starts, after any hold
/// (whose picture still draws the last scene's), and starts reading the
/// next scene's in the background: the opening's order is fixed
/// (RE-476).
pub(crate) fn load(o: &mut Opening, kind: Kind) {
    if kind == Kind::Room {
        room_figures(o);
    }
    let scene = MScene::Opening(kind);
    crate::scene_load::scene(crate::scene_load::name(scene), crate::scene_load::scene_roots(scene));
    let next = kind.next().map_or(MScene::Title, MScene::Opening);
    if next == MScene::Opening(Kind::Room) {
        room_figures(o);
    }
    crate::scene_load::prefetch(crate::scene_load::name(next), crate::scene_load::scene_roots(next));
}

/// Drops the running scene: a hold then shows black.
pub(crate) fn clear(o: &mut Opening) {
    o.logic = None;
    o.runtime = Runtime::new();
}

/// Whether `kind` runs on this screen (rather than as a battle).
pub(crate) fn runs(kind: Option<Kind>) -> bool {
    kind.is_none_or(|k| !k.is_battle())
}

/// `osGetTime`'s low byte for `syUtilsRandTimeUCharRange`. A capture build
/// draws the N64 reference's figures (Pikachu pulled, Donkey Kong dropped)
/// so its frames compare.
fn room_time() -> impl FnMut() -> u8 {
    let mut n = 0usize;
    move || {
        n += 1;
        if cfg!(feature = "headless_capture") {
            [0xE0, 0x40][(n - 1).min(1)]
        } else {
            unsafe { psp::sys::sceKernelGetSystemTimeLow() as u8 }
        }
    }
}

/// One frame: the tic, then a held scene's start or the running scene's
/// tick. Returns the scene to load.
pub(crate) fn frame(
    o: &mut Opening,
    pack: Option<&Pack<'_>>,
    tapped: bool,
) -> Option<MScene> {
    o.clock.retrace();
    if let Some(kind) = o.pending {
        if !o.clock.ready(kind) {
            return None;
        }
        o.pending = None;
        load(o, kind);
        start(o, Some(kind));
    }
    let exit = o.logic.as_mut()?.tick(tapped);
    if let Some(p) = pack {
        let a = Assets {
            main: p,
            scene: o.scene_pack(),
        };
        if let Some(l) = o.logic.as_ref() {
            o.runtime.sync(l.world(), &a);
        }
    }
    match exit? {
        Exit::Title => Some(MScene::Title),
        Exit::Next(kind) => Some(MScene::Opening(kind)),
    }
}

/// Holds the current picture until `kind`'s start tic, then starts it.
/// `false` when `kind` is not run here (the host loads it itself).
pub(crate) fn hold(o: &mut Opening, kind: Kind) -> bool {
    if !runs(Some(kind)) {
        return false;
    }
    o.pending = Some(kind);
    true
}

/// The running scene's picture.
///
/// # Safety
///
/// Between `begin_frame` and `end_frame`.
pub(crate) unsafe fn draw(o: &Opening, gpu: &mut Gpu, pack: Option<&Pack<'_>>, st: &mut DrawState) {
    // `mvOpeningRoomFuncRun` at tic 1037: the default camera stops filling,
    // and the transition opens over the picture left on screen.
    let freeze = matches!(&o.logic, Some(Logic::Room(r)) if r.tics() >= 1037);
    if freeze && !o.frozen.get() {
        gpu.capture_campaign_wallpaper();
        o.frozen.set(true);
    }
    gpu.set_viewport_fullscreen();
    gpu.begin_frame(Some(ssb_engine::renderer::Color::rgba(0, 0, 0, 255)));
    gpu.set_viewport_pillarboxed();
    if freeze {
        gpu.draw_frozen_picture();
        st.invalidate_all();
    }
    let (Some(p), Some(logic)) = (pack, o.logic.as_ref()) else {
        return;
    };
    let a = Assets {
        main: p,
        scene: o.scene_pack(),
    };
    let mut host = |gpu: &mut Gpu, st: &mut DrawState, _cam: &Camera, obj: &Object, head: Head| {
        host_draw(logic, &o.runtime, gpu, st, &a, obj, head);
    };
    o.runtime.draw(logic.world(), gpu, st, &a, &mut host);
    gpu.set_viewport_fullscreen();
}

/// The scenes' own displays.
unsafe fn host_draw(
    logic: &Logic,
    runtime: &Runtime,
    gpu: &mut Gpu,
    st: &mut DrawState,
    a: &Assets<'_, '_>,
    obj: &Object,
    head: Head,
) {
    use ssb_game::opening::movie::ObjectKind;
    match logic {
        // `mvOpeningRoomTransitionOutlineProcDisplay`: the depth image
        // filled black (nearest), then the outline's list in colour with no
        // depth test.
        Logic::Room(_)
            if matches!(
                obj.kind,
                ObjectKind::HostModel(room::HOST_TRANSITION_OUTLINE, _)
            ) =>
        {
            let ObjectKind::HostModel(_, m) = &obj.kind else {
                return;
            };
            gpu.fill_depth_near();
            st.depth_off = true;
            st.invalidate_all();
            runtime.draw_host_model(gpu, st, a, obj.id, m);
            st.depth_off = false;
            st.invalidate_all();
        }
        // `mvOpeningRoomTransitionOverlayProcDisplay`: the overlay's list
        // into the depth image in white (farthest), so what follows draws
        // only inside it.
        Logic::Room(_)
            if matches!(
                obj.kind,
                ObjectKind::HostModel(room::HOST_TRANSITION_OVERLAY, _)
            ) =>
        {
            let ObjectKind::HostModel(_, m) = &obj.kind else {
                return;
            };
            gpu.depth_far_only(true);
            st.depth_write_only = true;
            st.invalidate_all();
            runtime.draw_host_model(gpu, st, a, obj.id, m);
            st.depth_write_only = false;
            gpu.depth_far_only(false);
            st.invalidate_all();
        }
        // `mvOpeningRoomWallpaperProcDisplay`: the sprite at a fixed depth,
        // tested against the transition's depth.
        Logic::Room(_)
            if obj.kind == ObjectKind::Host(room::HOST_WALLPAPER) && head == Head::Zero =>
        {
            st.sprite_depth_test = true;
            movie::draw_piece(a, st, &room::Room::wallpaper());
            st.sprite_depth_test = false;
        }
        Logic::Portraits(p) if obj.id == p.cover_id() && head == Head::Zero => {
            for f in p.cover_fills() {
                ssb_psp_runtime::meshdraw::fill_rect_n64(f.rect, f.rgba, st);
            }
            movie::draw_piece(a, st, &p.cover_sprite());
        }
        Logic::Cliff(c) if obj.id == c.hills_id() && head == Head::Zero => {
            // `G_RM_AA_OPA_SURF`: no depth test or write.
            st.depth_off = true;
            st.invalidate_all();
            movie::draw_static_model(gpu, st, a, &cliff::Cliff::hills());
            st.depth_off = false;
            st.invalidate_all();
        }
        Logic::Sector(s) if Some(obj.id) == s.cockpit_id() && head == Head::Zero => {
            if let Some(p) = s.cockpit() {
                movie::draw_piece_texel_at_alpha(a, st, &p, s.cockpit_alpha());
            }
        }
        _ => {}
    }
}
