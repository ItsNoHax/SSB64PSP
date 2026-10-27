//! The opening's scenes as their cameras and display objects: what each
//! `gcMakeCameraGObj` and display `GObj` is, which display link it sits on
//! and what its processes have played so far.
//!
//! A scene keeps a [`World`] that its tick updates as the source's
//! `func_run` and processes do; the host draws it as `gcDrawAll` does
//! (cameras from the highest `dl_link_priority` down, each camera's links
//! from 0 up, the head-0 lists of all of them before any head-1 list) and
//! plays each object's animation scripts up to the plays the world counts.
//! The world names its ROM data by file and offset (`ssb_rom::opening`).

use alloc::vec::Vec;

use crate::fighter::FighterKind;
use crate::menu::Piece;
use ssb_engine::math::Vec3;
use ssb_engine::memory::Scope;

/// `COBJ_MASK_DLLINK(link)`.
pub const fn link(link: u8) -> u32 {
    1 << link
}

/// `syRdpSetViewport(&cobj->viewport, 10, 10, 310, 230)`, every scene's.
pub const FULL: [f32; 4] = [10.0, 10.0, 310.0, 230.0];

/// A camera animation's baked plays (`gcAddCObjCamAnimJoint`): its slot in
/// the scene pack and how many plays `gcPlayCamAnim` has run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CamAnim {
    pub slot: u32,
    pub plays: u32,
}

/// How a 3D camera builds its view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    /// `nGCMatrixKindLookAt` (6): `syMatrixLookAt` with the up vector
    /// `(up.x, 1, 0)`.
    LookAt,
    /// `syMatrixModLookAt` (8): `up.x` rolls the view about its look
    /// direction.
    Roll,
    /// `syMatrixModLookAtReflect` (14): [`Self::Roll`]'s view, and the
    /// camera loads a texgen `LookAt` (the room's scene-1 main camera).
    RollReflect,
}

impl View {
    /// `gcPrepCameraMatrix` loads a `LookAt` for this camera: kinds 12 to
    /// 17 only.
    pub const fn loads_look_at(self) -> bool {
        matches!(self, View::RollReflect)
    }
}

/// The texgen `LookAt` world-space right and up the RSP holds at a task's
/// start: F3DEX2 reloads its DMEM data every task, and its default `LookAt`
/// is zero, so texgen reads one texel (N64 ParaLLEl, RE-458).
pub const TASK_LOOK_AT: ([f32; 3], [f32; 3]) = ([0.0; 3], [0.0; 3]);

/// Tracks the `LookAt` the RSP holds across one frame's cameras in draw
/// order: [`TASK_LOOK_AT`] until a reflecting camera loads its own, which
/// stays for the cameras after it.
#[derive(Debug, Clone, Copy, Default)]
pub struct LookAtTracker {
    loaded: Option<([f32; 3], [f32; 3])>,
}

impl LookAtTracker {
    /// Camera `view` draws next; `own` is its view's first two rows (world
    /// right and up), `syMatrixModLookAtReflectF`'s `l[0]` and `l[1]` when
    /// it loads them. Returns the `LookAt` texgen reads under it.
    pub fn camera(&mut self, view: View, own: ([f32; 3], [f32; 3])) -> ([f32; 3], [f32; 3]) {
        if view.loads_look_at() {
            self.loaded = Some(own);
        }
        self.loaded.unwrap_or(TASK_LOOK_AT)
    }
}

/// `func_80017EC0`'s camera.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Persp {
    pub eye: Vec3,
    pub at: Vec3,
    pub up_x: f32,
    pub view: View,
    pub fovy: f32,
    pub aspect: f32,
    pub near: f32,
    pub far: f32,
    /// `nGCMatrixKindPerspF` (`syMatrixPerspF`, an exact cotangent) rather
    /// than `PerspFastF` (the sine table's): the cliff's cameras only.
    pub persp_f: bool,
    /// The camera animation that writes `eye`, `at`, `up.x` and `fovy`
    /// (its plays override the fields above).
    pub anim: Option<CamAnim>,
}

impl Persp {
    /// `gcAddXObjForCamera`'s defaults: `dGCPerspDefault` and
    /// `dGCCObjVecDefault`.
    /// Both are generated from the ROM ([`crate::d_arrays`]).
    pub const DEFAULT: Persp = {
        use crate::d_arrays as d;
        let (eye, at) = (d::GC_COBJ_VEC_DEFAULT_EYE, d::GC_COBJ_VEC_DEFAULT_AT);
        Persp {
            eye: Vec3 {
                x: eye[0],
                y: eye[1],
                z: eye[2],
            },
            at: Vec3 {
                x: at[0],
                y: at[1],
                z: at[2],
            },
            up_x: d::GC_COBJ_VEC_DEFAULT_UP[0],
            view: View::LookAt,
            fovy: d::GC_PERSP_DEFAULT_FOVY[0],
            aspect: d::GC_PERSP_DEFAULT_ASPECT[0],
            near: d::GC_PERSP_DEFAULT_NEAR[0],
            far: d::GC_PERSP_DEFAULT_FAR[0],
            persp_f: false,
            anim: None,
        }
    };

    /// The camera playing `slot`'s baked animation from its start.
    pub fn animated(slot: u32, view: View, near: f32, far: f32) -> Persp {
        Persp {
            view,
            near,
            far,
            anim: Some(CamAnim { slot, plays: 0 }),
            ..Persp::DEFAULT
        }
    }

    /// `gcPlayCamAnim`, once.
    pub fn play(&mut self) {
        if let Some(a) = self.anim.as_mut() {
            a.plays += 1;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CameraKind {
    /// `gcMakeDefaultCameraGObj`: `COBJ_FLAG_FILLCOLOR` fills the frame
    /// with `fill`, `COBJ_FLAG_ZBUFFER` clears depth.
    Default {
        fill: Option<[u8; 4]>,
        zbuffer: bool,
    },
    /// `lbCommonDrawSprite`: its links' sprites and fills in screen space.
    Sprite,
    /// `func_80017EC0`: a 3D camera; `zbuffer` (`COBJ_FLAG_ZBUFFER`)
    /// clears depth in its viewport first.
    Persp { persp: Persp, zbuffer: bool },
    /// `lbFadeMakeActor`'s display: `color` over the frame at `alpha`.
    Fade { color: [u8; 3], alpha: u8 },
}

/// One camera `GObj`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    pub id: u16,
    /// `dl_link_priority`: the cameras draw from the highest down, those
    /// of equal priority in the order they were made.
    pub priority: u32,
    /// `COBJ_MASK_DLLINK` bits.
    pub links: u32,
    pub viewport: [f32; 4],
    pub kind: CameraKind,
}

impl Camera {
    pub fn persp(&self) -> Option<&Persp> {
        match &self.kind {
            CameraKind::Persp { persp, .. } => Some(persp),
            _ => None,
        }
    }

    pub fn persp_mut(&mut self) -> Option<&mut Persp> {
        match &mut self.kind {
            CameraKind::Persp { persp, .. } => Some(persp),
            _ => None,
        }
    }
}

/// Which display-list head an object's own drawing goes to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Head {
    Zero,
    One,
    /// A tree drawn through its `DObjDLLink`s (`gcDrawDObjTreeDLLinksForGObj`):
    /// each list goes to the head its link names.
    Links,
}

/// A joint animation (`gcAddAnimJointAll` with a table, or
/// `gcAddDObjAnimJoint` with one script on the root) and its plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Joints {
    pub file: u32,
    pub script: u32,
    /// `script` is an `AObjEvent32 **` table, one script per node.
    pub table: bool,
    pub plays: u32,
}

impl Joints {
    pub const fn table(file: u32, table: u32) -> Joints {
        Joints {
            file,
            script: table,
            table: true,
            plays: 0,
        }
    }

    pub const fn root(file: u32, script: u32) -> Joints {
        Joints {
            file,
            script,
            table: false,
            plays: 0,
        }
    }
}

/// A `DObjDesc` tree or a one-`DObj` display list, as the pack keys it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Model {
    pub file: u32,
    pub graph: u32,
    pub joints: Option<Joints>,
    /// `gcAddMatAnimJointAll`'s plays: its material scripts from frame 0.
    pub mats: Option<u32>,
    /// The root `DObj`'s transform where the scene sets it; `None` keeps
    /// the tree's own (or its animation's).
    pub translate: Option<Vec3>,
    pub rotate: Option<Vec3>,
    pub scale: Option<Vec3>,
    pub head: Head,
}

impl Model {
    pub const fn new(file: u32, graph: u32, head: Head) -> Model {
        Model {
            file,
            graph,
            joints: None,
            mats: None,
            translate: None,
            rotate: None,
            scale: None,
            head,
        }
    }

    pub const fn joints(mut self, joints: Joints) -> Model {
        self.joints = Some(joints);
        self
    }

    pub const fn mats(mut self) -> Model {
        self.mats = Some(0);
        self
    }

    pub fn at(mut self, translate: Vec3) -> Model {
        self.translate = Some(translate);
        self
    }

    /// `gcPlayAnimAll`, once.
    pub fn play(&mut self) {
        if let Some(j) = self.joints.as_mut() {
            j.plays += 1;
        }
        if let Some(m) = self.mats.as_mut() {
            *m += 1;
        }
    }
}

/// A demo fighter (`ftManagerMakeFighter` with `dFTManagerDefaultFighterDesc`,
/// then `scSubsysFighterSetStatus`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fighter {
    pub kind: FighterKind,
    pub costume: u8,
    /// The demo status's submotion row (`status - nFTDemoStatusNull`).
    pub row: u8,
    /// Bumped by every `scSubsysFighterSetStatus`, so setting a status
    /// again restarts its clip.
    pub serial: u16,
    /// `scSubsysFighterProcUpdate`s since the status was set.
    pub plays: u32,
    /// The fighter's root `DObj`: its translation, rotation and scale.
    pub translate: Vec3,
    pub rotate: Vec3,
    pub scale: Vec3,
    /// `mvOpeningRunFighterProcUpdate`: a proxy `DObj`'s animation that
    /// moves the root each tick.
    pub proxy: Option<Joints>,
    /// `scSubsysFighterOpeningProcUpdate`: the fighter (an object id) whose
    /// `joint_itemheavy_id` joint holds this one, its TransN joint in the
    /// hand (the room's pulled figure).
    pub attach: Option<u16>,
}

impl Fighter {
    pub fn new(kind: FighterKind, color: usize, row: u8) -> Fighter {
        Fighter {
            kind,
            costume: crate::costume::costume_common_id(kind, color),
            row,
            serial: 0,
            plays: 0,
            translate: Vec3::ZERO,
            rotate: Vec3::ZERO,
            scale: Vec3::new(1.0, 1.0, 1.0),
            proxy: None,
            attach: None,
        }
    }

    /// `scSubsysFighterSetStatus`.
    pub fn set_status(&mut self, row: u8) {
        self.row = row;
        self.serial = self.serial.wrapping_add(1);
        self.plays = 0;
    }
}

/// `gDPFillRectangle` over `rect` (screen pixels) in `rgba`; below 0xFF
/// alpha it blends.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fill {
    pub rect: [f32; 4],
    pub rgba: [u8; 4],
}

#[derive(Debug, Clone, PartialEq)]
pub enum ObjectKind {
    Model(Model),
    Fighter(Fighter),
    /// `SObj`s through `lbCommonDrawSObjAttr`, in their chain's order.
    Sprites(Vec<Piece>),
    /// A custom display's rectangles.
    Fills(Vec<Fill>),
    /// Drawn by the scene's own host code: the scene's display id.
    Host(u16),
    /// A model whose scripts play as any model's but whose display is the
    /// scene's own (a custom render mode around `gcDrawDObjDLHead0`): the
    /// host draws it with its display id.
    HostModel(u16, Model),
    /// One of `efDisplayInitAll`'s particle displays: the particle lists
    /// it draws, in order. The host owns the particles.
    Particles(&'static [usize]),
}

/// `efDisplayInitAll`'s displays in creation order: link and lists.
const PARTICLE_DISPLAYS: [(u8, &[usize]); 4] = [(18, &[0, 2]), (15, &[1]), (25, &[3]), (10, &[4])];

/// One display `GObj`.
#[derive(Debug, Clone, PartialEq)]
pub struct Object {
    pub id: u16,
    /// Its display link.
    pub link: u8,
    pub head: Head,
    pub kind: ObjectKind,
    /// `GOBJ_FLAG_HIDDEN`.
    pub hidden: bool,
}

/// A scene's cameras and display objects.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct World {
    pub cameras: Vec<Camera>,
    pub objects: Vec<Object>,
    next_id: u16,
    /// `scSubsysFighterSetLightParams`' angles.
    pub light: [f32; 2],
}

impl World {
    pub fn new() -> World {
        World::default()
    }

    fn id(&mut self) -> u16 {
        self.next_id = self.next_id.wrapping_add(1);
        self.next_id
    }

    /// `gcMakeCameraGObj`: returns its id.
    pub fn camera(
        &mut self,
        priority: u32,
        links: u32,
        viewport: [f32; 4],
        kind: CameraKind,
    ) -> u16 {
        let id = self.id();
        self.cameras.push(Camera {
            id,
            priority,
            links,
            viewport,
            kind,
        });
        id
    }

    /// `gcMakeGObjSPAfter` with a display at `link`: returns its id.
    pub fn object(&mut self, link: u8, head: Head, kind: ObjectKind) -> u16 {
        let id = self.id();
        self.objects.push(Object {
            id,
            link,
            head,
            kind,
            hidden: false,
        });
        id
    }

    /// `efDisplayInitAll` (`efManagerInitEffects`): the particle displays,
    /// lists 0 and 2 on link 18, 1 on 15, 3 on 25 and 4 on 10
    /// (`efDisplayZPerspAAXLUProcDisplay`, depth-tested).
    pub fn particle_displays(&mut self) {
        for (link, lists) in PARTICLE_DISPLAYS {
            self.object(link, Head::Zero, ObjectKind::Particles(lists));
        }
    }

    /// `gcEjectGObj`.
    pub fn eject(&mut self, id: u16) {
        self.cameras.retain(|c| c.id != id);
        self.objects.retain(|o| o.id != id);
    }

    pub fn get(&self, id: u16) -> Option<&Object> {
        self.objects.iter().find(|o| o.id == id)
    }

    pub fn get_mut(&mut self, id: u16) -> Option<&mut Object> {
        self.objects.iter_mut().find(|o| o.id == id)
    }

    pub fn camera_mut(&mut self, id: u16) -> Option<&mut Camera> {
        self.cameras.iter_mut().find(|c| c.id == id)
    }

    pub fn model_mut(&mut self, id: u16) -> Option<&mut Model> {
        match self.get_mut(id).map(|o| &mut o.kind) {
            Some(ObjectKind::Model(m)) | Some(ObjectKind::HostModel(_, m)) => Some(m),
            _ => None,
        }
    }

    pub fn fighter_mut(&mut self, id: u16) -> Option<&mut Fighter> {
        match self.get_mut(id).map(|o| &mut o.kind) {
            Some(ObjectKind::Fighter(f)) => Some(f),
            _ => None,
        }
    }

    pub fn sprites_mut(&mut self, id: u16) -> Option<&mut Vec<Piece>> {
        match self.get_mut(id).map(|o| &mut o.kind) {
            Some(ObjectKind::Sprites(s)) => Some(s),
            _ => None,
        }
    }

    pub fn fills_mut(&mut self, id: u16) -> Option<&mut Vec<Fill>> {
        match self.get_mut(id).map(|o| &mut o.kind) {
            Some(ObjectKind::Fills(f)) => Some(f),
            _ => None,
        }
    }

    /// Every camera in `gcDrawAll`'s order: highest priority first, ties in
    /// creation order. The list is per-frame scratch in `scope` (the
    /// frame arena), so the draw takes nothing from the heap; `None` when
    /// it does not fit.
    pub fn draw_order_in<'w: 's, 's>(
        &'w self,
        scope: &mut Scope<'s>,
    ) -> Option<&'s mut [&'w Camera]> {
        let order = scope.alloc_from_iter(self.cameras.iter())?;
        // Stable insertion sort (no buffer): a camera moves only past a
        // lower priority, so ties keep creation order.
        for i in 1..order.len() {
            let mut j = i;
            while j > 0 && order[j - 1].priority < order[j].priority {
                order.swap(j - 1, j);
                j -= 1;
            }
        }
        Some(order)
    }

    /// [`World::draw_order_in`] on the heap, for tests.
    #[cfg(test)]
    pub(crate) fn draw_order(&self) -> Vec<&Camera> {
        let mut v: Vec<&Camera> = self.cameras.iter().collect();
        v.sort_by_key(|c| core::cmp::Reverse(c.priority));
        v
    }

    /// The objects camera `cam` draws, link by link from 0.
    pub fn drawn_by<'a>(&'a self, cam: &'a Camera) -> impl Iterator<Item = &'a Object> + 'a {
        (0..32u8)
            .filter(move |l| cam.links & link(*l) != 0)
            .flat_map(move |l| {
                self.objects
                    .iter()
                    .filter(move |o| o.link == l && !o.hidden)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Kinds 6 and 8 (the opening's posed and movie cameras) load no
    /// `LookAt`: texgen reads the task's zeroed default until a reflecting
    /// camera loads one (N64 ParaLLEl, opening Link's sword, RE-458). Room
    /// scene 1's kind 14 loads its own rolled right and up, which stay
    /// (N64 ParaLLEl, the room's window at tic 330, RE-458).
    #[test]
    fn only_a_reflecting_camera_replaces_the_task_look_at() {
        assert_eq!(TASK_LOOK_AT, ([0.0; 3], [0.0; 3]));
        let a = ([0.6, 0.0, -0.8], [0.1, 0.99, 0.0]);
        let b = ([1.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
        let c = ([0.0, 1.0, 0.0], [1.0, 0.0, 0.0]);
        let mut t = LookAtTracker::default();
        assert_eq!(
            [(View::LookAt, a), (View::Roll, b), (View::LookAt, c)].map(|(v, o)| t.camera(v, o)),
            [TASK_LOOK_AT; 3]
        );
        let mut t = LookAtTracker::default();
        assert_eq!(
            [
                (View::LookAt, a),
                (View::RollReflect, b),
                (View::Roll, c),
                (View::RollReflect, a)
            ]
            .map(|(v, o)| t.camera(v, o)),
            [TASK_LOOK_AT, b, b, a]
        );
    }

    #[test]
    fn cameras_draw_from_the_highest_priority_and_links_from_zero() {
        let mut w = World::new();
        let low = w.camera(10, link(26) | link(9), FULL, CameraKind::Sprite);
        let high = w.camera(80, link(27), FULL, CameraKind::Sprite);
        let mid = w.camera(10, link(28), FULL, CameraKind::Sprite);
        let order: Vec<u16> = w.draw_order().iter().map(|c| c.id).collect();
        assert_eq!(order, [high, low, mid]);
        {
            // The frame-arena order is the same, ties included.
            let top = w.camera(80, link(29), FULL, CameraKind::Sprite);
            let last = w.camera(5, link(30), FULL, CameraKind::Sprite);
            let mut buf = [0u8; 256];
            let mut arena = ssb_engine::memory::Arena::new(&mut buf);
            let mut frame = arena.begin();
            let ids: Vec<u16> = w
                .draw_order_in(&mut frame)
                .unwrap()
                .iter()
                .map(|c| c.id)
                .collect();
            assert_eq!(ids, [high, top, low, mid, last]);
            let heap: Vec<u16> = w.draw_order().iter().map(|c| c.id).collect();
            assert_eq!(ids, heap);
            // The scope borrows the world's cameras until it drops.
            drop(frame);
            w.cameras.retain(|c| c.id != top && c.id != last);
        }
        let a = w.object(26, Head::Zero, ObjectKind::Host(1));
        let b = w.object(9, Head::Zero, ObjectKind::Host(2));
        let c = w.object(26, Head::Zero, ObjectKind::Host(3));
        let cam = *w.cameras.iter().find(|c| c.id == low).unwrap();
        let drawn: Vec<u16> = w.drawn_by(&cam).map(|o| o.id).collect();
        assert_eq!(drawn, [b, a, c]);
        w.eject(a);
        assert_eq!(w.drawn_by(&cam).count(), 2);
    }
}
