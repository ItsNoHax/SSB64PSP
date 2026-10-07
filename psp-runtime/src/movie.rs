//! The opening movie's scenes on the PSP (RE-467): plays each
//! [`ssb_game::opening::movie::World`] object's scripts up to the plays the
//! world counts, and draws the world as `gcDrawAll` does.
//!
//! Cameras draw from the highest `dl_link_priority` down. A sprite camera
//! draws its links' sprites and fills in screen space; a 3D camera sets its
//! viewport, projection and view, then draws its links' head-0 lists and
//! after them their head-1 lists, as `func_80017EC0` gathers each camera's
//! display-list heads. Models and fighters keep their joint poses here:
//! a model's `AnimJoint` scripts play from the scene's script files, its
//! `MatAnimJoint` scripts through the pack's material table, and a demo
//! fighter's status clip through its skeleton.

use alloc::{boxed::Box, vec::Vec};

use ssb_engine::math::Vec3;
use ssb_game::fighter::FighterKind;
use ssb_game::menu::Piece;
use ssb_game::opening::movie::{
    Camera, CameraKind, Fighter, Head, Model, Object, ObjectKind, Persp, View, World,
};
use ssb_rom::figatree::JointPose;
use ssb_rom::objanim::StageJoint;
use ssb_rom::pack::{ObjectDesc, Pack, SpriteDesc, MODEL_SCALE};
use ssb_rom::scene::Mat4;
use ssb_rom::skeleton::{EffectMaterialAnimator, Skeleton};

use crate::assets::AlignedBuf;
use crate::gu::Gpu;
use crate::meshdraw::{self, DrawState};

/// The most nodes an opening tree has (the room's background: 53; the
/// Yoshis' nest: 73).
const MAX_NODES: usize = 80;

/// The opening's own models (`MenuScene::OpeningModels`), held from the
/// room to the title as the opening's files stay loaded across its scenes.
static mut MODELS: Option<AlignedBuf> = None;

/// Loads the opening's models pack beside `pack_path` when `want` and not
/// yet held, or drops it when not `want`.
pub fn hold_models(pack_path: Option<&str>, want: bool) {
    // SAFETY: the game runs on one thread; no `Pack` borrowed from the
    // buffer outlives the frame that opened it.
    let held = unsafe { &mut *core::ptr::addr_of_mut!(MODELS) };
    if !want {
        *held = None;
    } else if held.is_none() {
        *held = pack_path.and_then(|p| {
            crate::assets::load_menu_pack(p, ssb_rom::menu_pack::MenuScene::OpeningModels).ok()
        });
    }
}

/// The opening's models pack, while held.
pub fn models() -> Option<Pack<'static>> {
    // SAFETY: as [`hold_models`]; the buffer is only dropped between
    // frames, when no pack opened from it is alive.
    let held = unsafe { &*core::ptr::addr_of!(MODELS) };
    held.as_ref().and_then(|b| {
        let bytes: &'static [u8] =
            unsafe { core::slice::from_raw_parts(b.as_slice().as_ptr(), b.as_slice().len()) };
        Pack::open(bytes).ok()
    })
}

/// The packs a scene reads: its own (sprites, scripts, cameras), the
/// opening's models, then the resident one.
#[derive(Clone, Copy)]
pub struct Assets<'a, 'b> {
    pub main: &'a Pack<'b>,
    pub scene: Option<&'a Pack<'b>>,
    pub models: Option<&'a Pack<'b>>,
}

impl<'a, 'b> Assets<'a, 'b> {
    /// The pack holding `file`'s models: the opening's own for its files
    /// (`ssb_rom::opening::is_model_file`), else the resident one.
    pub fn model_pack(&self, file: u32) -> &'a Pack<'b> {
        match self.models {
            Some(m) if ssb_rom::opening::is_model_file(file) => m,
            _ => self.main,
        }
    }

    fn effect_blob(pack: &'a Pack<'b>, slot: u32) -> Option<&'a [u8]> {
        let a = pack.effect_anim(slot)?;
        pack.anim_script(&a)
    }

    /// The bytes of `file`, which its joint scripts play from: the scene
    /// pack's copy, or the resident `MVCommon` (the ending's room scripts).
    pub fn script_data(&self, file: u32) -> Option<&'a [u8]> {
        if let Some(d) = self
            .scene
            .and_then(|p| Self::effect_blob(p, ssb_rom::opening::blob_slot(file)))
        {
            return Some(d);
        }
        if file == ssb_rom::opening::file::MV_COMMON {
            return Self::effect_blob(self.main, ssb_rom::ending::ROOM_SCRIPTS_SLOT);
        }
        None
    }

    /// A camera animation's baked plays.
    pub fn camera(&self, slot: u32) -> Option<&'a [u8]> {
        Self::effect_blob(self.scene?, slot)
    }

    /// A sprite from the scene pack or the resident one.
    pub fn sprite(&self, file: u32, offset: u32) -> Option<(&'a Pack<'b>, SpriteDesc)> {
        [self.scene, Some(self.main)]
            .into_iter()
            .flatten()
            .find_map(|p| Some((p, p.sprite(file, offset)?)))
    }
}

/// `gcAddAnimJointAll`/`gcAddDObjAnimJoint`'s joints and the poses they
/// write.
struct ModelState {
    key: (u32, u32),
    object: ObjectDesc,
    poses: Vec<JointPose>,
    joints: Vec<Option<StageJoint>>,
    joints_file: u32,
    played: u32,
    mats: Option<Box<EffectMaterialAnimator>>,
    mats_played: u32,
}

/// A demo fighter's skeleton on its status's clip.
struct FighterState {
    kind: FighterKind,
    serial: u16,
    row: u8,
    object: u32,
    skeleton: Box<Skeleton>,
    slot: Option<u32>,
    demo: ssb_game::modelpart::DemoParts,
    played: u32,
    /// The proxy `DObj`'s joint (`mvOpeningRunFighterProcUpdate`).
    proxy: Option<(StageJoint, JointPose, u32)>,
    /// The root matrix `scSubsysFighterOpeningProcUpdate` last gave it on
    /// another fighter's hand; its TopN keeps the place once let go.
    held: Option<Mat4>,
    /// `scSubsysFighterApplyVelTransN` (rows 9 and 10, whose clips detach
    /// TransN, `is_use_transn_joint`): TransN's translate when the status
    /// began, from which TopN follows its moves.
    transn_base: Option<[f32; 3]>,
    /// What earlier such statuses moved TopN by.
    transn_carried: [f32; 3],
}

/// The demo rows whose `proc_update` is `scSubsysFighterApplyVelTransN`
/// (`D_ovl1_80390BE8`): `FigureDropped` and `FigureStand`.
fn applies_vel_transn(row: u8) -> bool {
    matches!(row, 9 | 10)
}

enum State {
    Model(Box<ModelState>),
    Fighter(Box<FighterState>),
}

/// The scene's live animation state, by object id.
#[derive(Default)]
pub struct Runtime {
    states: Vec<(u16, State)>,
}

fn rest_poses(pack: &Pack<'_>, object: &ObjectDesc) -> Vec<JointPose> {
    let count = (object.node_count as usize).min(MAX_NODES);
    (0..count)
        .map(|i| {
            pack.node(object.first_node + i as u32)
                .map_or(JointPose::default(), |node| JointPose {
                    rotate: node.rest_rotate,
                    translate: node.rest_translate,
                    scale: node.rest_scale,
                })
        })
        .collect()
}

/// Each node's matrix, parent first, in pack units.
fn compose(pack: &Pack<'_>, object: &ObjectDesc, poses: &[JointPose], out: &mut [Mat4]) -> usize {
    let count = poses.len().min(out.len());
    for i in 0..count {
        let pose = poses[i];
        let t = pose.translate.map(|v| v / MODEL_SCALE);
        let local = Mat4::from_trs(t, pose.rotate, pose.scale);
        let parent = pack
            .node(object.first_node + i as u32)
            .and_then(|n| n.parent.checked_sub(object.first_node))
            .map(|p| p as usize)
            .filter(|&p| p < i);
        out[i] = match parent {
            Some(p) => out[p].mul(&local),
            None => local,
        };
    }
    count
}

/// The submotion row's clip slot (`D_ovl1_80390BE8` and the fighter's
/// opening table): row 0 `Wait`, 1 to 5 the results' and selects', then
/// the opening's and the 1P cards'.
pub fn demo_row_slot(row: u8) -> Option<usize> {
    use ssb_rom::anim as a;
    Some(match row {
        0 => a::SLOT_WAIT,
        1..=5 => a::SLOT_WIN1 + usize::from(row) - 1,
        6 => a::SLOT_DEMO_RUN,
        7 => a::SLOT_DEMO_JUMP,
        8 => a::SLOT_FIGURE_PULLED,
        9 => a::SLOT_FIGURE_DROPPED,
        10 => a::SLOT_FIGURE_STAND,
        11 => a::SLOT_CLASH,
        12 => a::SLOT_STANCE,
        13 => a::SLOT_INTRO_L,
        14 => a::SLOT_INTRO_R,
        15..=23 => a::SLOT_OPENING1 + usize::from(row) - 15,
        _ => return None,
    })
}

impl ModelState {
    fn new(a: &Assets<'_, '_>, m: &Model) -> Option<ModelState> {
        let pack = a.model_pack(m.file);
        let object = crate::scene::object_keyed(pack, (m.file, m.graph))?;
        let mut poses = rest_poses(pack, &object);
        if let Some(root) = poses.first_mut() {
            if let Some(t) = m.translate {
                root.translate = [t.x, t.y, t.z];
            }
            if let Some(r) = m.rotate {
                root.rotate = [r.x, r.y, r.z];
            }
            if let Some(s) = m.scale {
                root.scale = [s.x, s.y, s.z];
            }
        }
        let (joints, joints_file) = match m.joints {
            Some(j) => {
                let data = a.script_data(j.file).unwrap_or(&[]);
                let scripts: Vec<Option<u32>> = if j.table {
                    ssb_rom::objanim::joint_scripts(data, j.script, poses.len())
                } else {
                    alloc::vec![Some(j.script)]
                };
                let joints = scripts
                    .into_iter()
                    .map(|s| s.map(|s| StageJoint::start_changed(s, 0.0)))
                    .collect();
                (joints, j.file)
            }
            None => (Vec::new(), 0),
        };
        let mats = m.mats.map(|_| {
            let mut mats = Box::new(EffectMaterialAnimator::new());
            let prims = (0..object.node_count)
                .filter_map(|n| pack.node(object.first_node + n))
                .filter_map(|node| pack.mesh(node.mesh))
                .flat_map(|mesh| (0..mesh.prim_count).map(move |j| mesh.first_prim + j))
                .filter_map(|i| pack.prim(i))
                .map(|prim| prim.mat_anim);
            mats.start(pack, prims);
            mats
        });
        Some(ModelState {
            key: (m.file, m.graph),
            object,
            poses,
            joints,
            joints_file,
            played: 0,
            mats,
            mats_played: 0,
        })
    }

    /// `DOBJ_FLAG_HIDDEN` (bit 0) on the node, or `DOBJ_FLAG_HIDETREE`
    /// (bit 1) on it or an ancestor, as its joint's `SetFlags` left them.
    fn hidden(&self, pack: &Pack<'_>, global: u32) -> bool {
        let flags = |i: usize| {
            self.joints
                .get(i)
                .and_then(|j| j.as_ref())
                .map_or(0, |j| j.flags)
        };
        let Some(mut i) = global
            .checked_sub(self.object.first_node)
            .map(|i| i as usize)
        else {
            return false;
        };
        if flags(i) & 1 != 0 {
            return true;
        }
        for _ in 0..self.poses.len() {
            if flags(i) & 2 != 0 {
                return true;
            }
            match pack
                .node(self.object.first_node + i as u32)
                .and_then(|n| n.parent.checked_sub(self.object.first_node))
            {
                Some(p) if (p as usize) < i => i = p as usize,
                _ => return false,
            }
        }
        false
    }

    fn catch_up(&mut self, a: &Assets<'_, '_>, m: &Model) {
        let target = m.joints.map_or(0, |j| j.plays);
        if self.played < target {
            let data = a.script_data(self.joints_file).unwrap_or(&[]);
            while self.played < target {
                for (joint, pose) in self.joints.iter_mut().zip(self.poses.iter_mut()) {
                    if let Some(j) = joint.as_mut() {
                        let _ = j.tick(data, 1.0, pose);
                    }
                }
                self.played += 1;
            }
        }
        let target = m.mats.unwrap_or(0);
        if let Some(mats) = self.mats.as_mut() {
            while self.mats_played < target {
                mats.tick(a.model_pack(self.key.0));
                self.mats_played += 1;
            }
        }
    }
}

impl FighterState {
    fn new(a: &Assets<'_, '_>, f: &Fighter) -> Option<FighterState> {
        let object = crate::scene::fighter_object(a.main, f.kind as u32)?;
        let mut s = FighterState {
            kind: f.kind,
            serial: f.serial,
            row: f.row,
            object,
            skeleton: Box::new(Skeleton::new()),
            slot: None,
            demo: ssb_game::modelpart::DemoParts::start(f.kind, usize::from(f.row)),
            played: 0,
            proxy: f.proxy.map(|j| {
                (
                    StageJoint::start_changed(j.script, 0.0),
                    JointPose::default(),
                    0,
                )
            }),
            held: None,
            transn_base: None,
            transn_carried: [0.0; 3],
        };
        s.set_status(a.main, f.row);
        Some(s)
    }

    /// `ftMainSetStatus`: the clip from its first frame, played at once.
    fn set_status(&mut self, p: &Pack<'_>, row: u8) {
        if let Some(base) = self.transn_base {
            let now = self.child_translate();
            for i in 0..3 {
                self.transn_carried[i] += now[i] - base[i];
            }
        }
        self.row = row;
        self.demo = ssb_game::modelpart::DemoParts::start(self.kind, usize::from(row));
        self.slot = demo_row_slot(row).map(|s| s as u32);
        *self.skeleton = Skeleton::new();
        if let Some(anim) = self
            .slot
            .and_then(|slot| p.fighter_anim(self.kind as u32, slot))
        {
            self.skeleton.start(p, &anim, 0.0, 1.0);
            self.play(p);
        } else {
            self.slot = None;
        }
        self.transn_base = applies_vel_transn(row).then(|| self.child_translate());
        self.played = 0;
    }

    /// `scSubsysFighterProcUpdate`'s clip play.
    fn play(&mut self, p: &Pack<'_>) {
        let Some(anim) = self
            .slot
            .and_then(|slot| p.fighter_anim(self.kind as u32, slot))
        else {
            return;
        };
        let Some(script) = p.anim_script(&anim) else {
            return;
        };
        let scales = if self.kind == FighterKind::Luigi {
            None
        } else {
            p.fighter_translate_scales(self.kind as u32)
        };
        let first = p.object(self.object).map_or(0, |o| o.first_node);
        let _ = self.skeleton.tick_scaled(script, scales, first);
    }

    fn catch_up(&mut self, a: &Assets<'_, '_>, f: &Fighter) {
        if f.serial != self.serial {
            self.serial = f.serial;
            self.set_status(a.main, f.row);
        }
        while self.played < f.plays {
            self.play(a.main);
            self.demo.tick();
            self.played += 1;
        }
        if let (Some((joint, pose, played)), Some(j)) = (self.proxy.as_mut(), f.proxy) {
            let data = a.script_data(j.file).unwrap_or(&[]);
            while *played < j.plays {
                let _ = joint.tick(data, 1.0, pose);
                *played += 1;
            }
        }
    }

    /// The model's matrices under the clip's TransN: a figatree whose first
    /// joint has no packed node (`NO_NODE`, Donkey Kong's clash) moves the
    /// whole model through it, which a demo status draws as it is rather
    /// than handing it to the physics (as `FighterScene::compose_model`
    /// does for `CapturePulled`).
    fn compose(&self, p: &Pack<'_>, object: &ObjectDesc, out: &mut [Mat4]) -> usize {
        let count = self.skeleton.compose(p, object, out);
        // A detached TransN draws nothing; TopN carries its moves.
        if self.transn_base.is_none() && self.skeleton.joint_node(0).is_none() {
            if let Some(pose) = self.skeleton.pose(0) {
                let t = pose.translate.map(|v| v / MODEL_SCALE);
                let transn = Mat4::from_trs(t, pose.rotate, pose.scale);
                for m in &mut out[..count] {
                    *m = transn.mul(m);
                }
            }
        }
        count
    }

    /// The fighter's root matrix (world units, `MODEL_SCALE` in its
    /// scale): the proxy's transform where it has one, the hand's while
    /// held, and the hand's last place with its own rotation once let go.
    fn root(&self, f: &Fighter) -> Mat4 {
        let k = MODEL_SCALE;
        let s = [f.scale.x * k, f.scale.y * k, f.scale.z * k];
        let r = [f.rotate.x, f.rotate.y, f.rotate.z];
        let mut m = match (&self.proxy, self.held) {
            (_, Some(held)) if f.attach.is_some() => held,
            (_, Some(held)) => Mat4::from_trs(held.translation(), r, s),
            (Some((_, pose, _)), None) => Mat4::from_trs(pose.translate, pose.rotate, s),
            (None, None) => Mat4::from_trs([f.translate.x, f.translate.y, f.translate.z], r, s),
        };
        let now = self.child_translate();
        for i in 0..3 {
            m.0[12 + i] += self.transn_carried[i] + self.transn_base.map_or(0.0, |b| now[i] - b[i]);
        }
        m
    }

    /// TopN's child's translate (world units): TransN's, where the clip
    /// moves it.
    fn child_translate(&self) -> [f32; 3] {
        if self.skeleton.joint_node(0).is_none() {
            if let Some(pose) = self.skeleton.pose(0) {
                return pose.translate;
            }
        }
        [0.0; 3]
    }
}

/// `FTAttributes::joint_itemheavy_id` as a node of the fighter's packed
/// object (joint `n + 4`): Master Hand's is joint 5 (file 250's attributes
/// at 0xE8 + 0x334).
fn itemheavy_node(kind: FighterKind) -> Option<usize> {
    match kind {
        FighterKind::Boss => Some(1),
        k => ssb_game::grab::itemheavy_joint(k).and_then(|j| j.checked_sub(4)),
    }
}

/// `m`'s rotation alone: its axes normalised.
fn rotation_of(m: &Mat4) -> Mat4 {
    use ssb_engine::math::sqrt;
    let mut out = Mat4::IDENTITY;
    for c in 0..3 {
        let v = [m.0[c * 4], m.0[c * 4 + 1], m.0[c * 4 + 2]];
        let len = sqrt(v[0] * v[0] + v[1] * v[1] + v[2] * v[2]);
        if len > 0.0 {
            for r in 0..3 {
                out.0[c * 4 + r] = v[r] / len;
            }
        }
    }
    out
}

impl Runtime {
    pub fn new() -> Runtime {
        Runtime::default()
    }

    /// Makes, drops and plays each object's state to match `world`.
    pub fn sync(&mut self, world: &World, a: &Assets<'_, '_>) {
        self.states
            .retain(|(id, _)| world.objects.iter().any(|o| o.id == *id));
        for o in &world.objects {
            let at = self.states.iter().position(|(id, _)| *id == o.id);
            match &o.kind {
                ObjectKind::Model(m) | ObjectKind::HostModel(_, m) => {
                    let stale = at.is_some_and(|i| match &self.states[i].1 {
                        State::Model(s) => s.key != (m.file, m.graph),
                        _ => true,
                    });
                    if stale {
                        self.states.retain(|(id, _)| *id != o.id);
                    }
                    if at.is_none() || stale {
                        if let Some(s) = ModelState::new(a, m) {
                            self.states.push((o.id, State::Model(Box::new(s))));
                        }
                    }
                    if let Some((_, State::Model(s))) =
                        self.states.iter_mut().find(|(id, _)| *id == o.id)
                    {
                        s.catch_up(a, m);
                    }
                }
                ObjectKind::Fighter(f) => {
                    if at.is_none() {
                        if let Some(s) = FighterState::new(a, f) {
                            self.states.push((o.id, State::Fighter(Box::new(s))));
                        }
                    }
                    if let Some((_, State::Fighter(s))) =
                        self.states.iter_mut().find(|(id, _)| *id == o.id)
                    {
                        s.catch_up(a, f);
                    }
                }
                _ => {}
            }
        }
        // `scSubsysFighterOpeningProcUpdate`: a held fighter's TopN takes
        // the holder's `joint_itemheavy_id` rotation, and its place less
        // TopN's child's translate.
        for o in &world.objects {
            let ObjectKind::Fighter(f) = &o.kind else {
                continue;
            };
            let Some(holder) = f.attach else {
                continue;
            };
            let hand = world.get(holder).and_then(|h| match &h.kind {
                ObjectKind::Fighter(hf) => {
                    let node = itemheavy_node(hf.kind)?;
                    let State::Fighter(hs) = self.state(holder)? else {
                        return None;
                    };
                    let posed = self.node_matrix(a, holder, node)?;
                    Some(hs.root(hf).mul(&posed))
                }
                _ => None,
            });
            let Some(hand) = hand else {
                continue;
            };
            if let Some((_, State::Fighter(s))) = self.states.iter_mut().find(|(id, _)| *id == o.id)
            {
                let c = s.child_translate().map(|v| -v / MODEL_SCALE);
                let t = hand.transform_point(c);
                let k = MODEL_SCALE;
                let scale = Mat4::from_trs(
                    [0.0; 3],
                    [0.0; 3],
                    [f.scale.x * k, f.scale.y * k, f.scale.z * k],
                );
                let mut held = rotation_of(&hand).mul(&scale);
                held.0[12] = t[0];
                held.0[13] = t[1];
                held.0[14] = t[2];
                s.held = Some(held);
            }
        }
    }

    fn state(&self, id: u16) -> Option<&State> {
        self.states.iter().find(|(i, _)| *i == id).map(|(_, s)| s)
    }

    /// Node `node`'s world matrix (pack units under a `MODEL_SCALE` base)
    /// of model or fighter `id`.
    pub fn node_matrix(&self, a: &Assets<'_, '_>, id: u16, node: usize) -> Option<Mat4> {
        match self.state(id)? {
            State::Model(s) => {
                let mut posed = [Mat4::IDENTITY; MAX_NODES];
                let n = compose(a.model_pack(s.key.0), &s.object, &s.poses, &mut posed);
                (node < n).then(|| posed[node])
            }
            State::Fighter(s) => {
                let object = a.main.object(s.object)?;
                let mut posed = [Mat4::IDENTITY; ssb_rom::skeleton::MAX_NODES];
                let n = s.compose(a.main, &object, &mut posed);
                (node < n).then(|| posed[node])
            }
        }
    }

    /// The world as `gcDrawAll` draws it. `host` draws the scene's own
    /// displays ([`ObjectKind::Host`]) for the camera and head it is given.
    ///
    /// # Safety
    ///
    /// Between `begin_frame` and `end_frame`.
    pub unsafe fn draw(
        &self,
        world: &World,
        gpu: &mut Gpu,
        st: &mut DrawState,
        a: &Assets<'_, '_>,
        host: &mut dyn FnMut(&mut Gpu, &mut DrawState, &Camera, &Object, Head),
    ) {
        for cam in world.draw_order() {
            gpu.set_viewport_n64(cam.viewport);
            match &cam.kind {
                CameraKind::Default { fill, zbuffer } => {
                    if let Some(rgba) = fill {
                        meshdraw::fill_rect_n64(cam.viewport, *rgba, st);
                    }
                    if *zbuffer {
                        gpu.clear_depth();
                    }
                }
                CameraKind::Fade { color, alpha } => {
                    meshdraw::fill_rect_n64(
                        cam.viewport,
                        [color[0], color[1], color[2], *alpha],
                        st,
                    );
                }
                CameraKind::Sprite => {
                    for head in [Head::Zero, Head::One] {
                        for o in world.drawn_by(cam) {
                            if !admits(o.head, head) {
                                continue;
                            }
                            match &o.kind {
                                ObjectKind::Sprites(pieces) => {
                                    for p in pieces {
                                        draw_piece(a, st, p);
                                    }
                                }
                                ObjectKind::Fills(fills) => {
                                    for f in fills {
                                        meshdraw::fill_rect_n64(f.rect, f.rgba, st);
                                    }
                                }
                                ObjectKind::Host(_) => host(gpu, st, cam, o, head),
                                _ => {}
                            }
                        }
                    }
                }
                CameraKind::Persp { persp, zbuffer } => {
                    if *zbuffer {
                        gpu.clear_depth();
                    }
                    self.view(gpu, st, a, persp);
                    st.set_stage_light(world.light);
                    for head in [Head::Zero, Head::One] {
                        st.heads = match head {
                            Head::One => meshdraw::Heads::Head1,
                            _ => meshdraw::Heads::Head0,
                        };
                        for o in world.drawn_by(cam) {
                            if o.head != Head::Links && !admits(o.head, head) {
                                continue;
                            }
                            match &o.kind {
                                // A tree drawn whole into one head
                                // (`gcDrawDObjDLHead0/1`,
                                // `gcDrawDObjTreeForGObj`) draws every list
                                // it has there, whatever links they carry.
                                ObjectKind::Model(m) if o.head != Head::Links => {
                                    st.heads = meshdraw::Heads::Both;
                                    self.draw_model(gpu, st, a, o.id, m);
                                    st.heads = match head {
                                        Head::One => meshdraw::Heads::Head1,
                                        _ => meshdraw::Heads::Head0,
                                    };
                                }
                                ObjectKind::Model(m) => self.draw_model(gpu, st, a, o.id, m),
                                ObjectKind::Fighter(f) if head == Head::Zero => {
                                    st.heads = meshdraw::Heads::Both;
                                    self.draw_fighter(gpu, st, a, o.id, f, world.light);
                                    st.heads = meshdraw::Heads::Head0;
                                }
                                ObjectKind::Host(_) | ObjectKind::HostModel(..) => {
                                    host(gpu, st, cam, o, head)
                                }
                                _ => {}
                            }
                        }
                    }
                    st.heads = meshdraw::Heads::Both;
                }
            }
        }
        gpu.set_viewport_pillarboxed();
    }

    /// The camera's view and projection: its animation's play where it has
    /// one.
    unsafe fn view(&self, gpu: &mut Gpu, st: &mut DrawState, a: &Assets<'_, '_>, p: &Persp) {
        let mut eye = p.eye;
        let mut at = p.at;
        let mut up_x = p.up_x;
        let mut fovy = p.fovy;
        if let Some(anim) = p.anim {
            if let Some(f) = a.camera(anim.slot).and_then(|b| {
                ssb_rom::camanim::frame_at(b, (anim.plays as usize).saturating_sub(1))
            }) {
                if anim.plays > 0 {
                    eye = Vec3::new(f[0], f[1], f[2]);
                    at = Vec3::new(f[3], f[4], f[5]);
                    up_x = f[6];
                    fovy = f[7];
                }
            }
        }
        gpu.set_perspective(fovy, p.aspect, p.near, p.far);
        gpu.reset_modelview();
        st.begin_frame();
        gpu.set_view(&view_matrix(eye, at, up_x, p.view));
    }

    unsafe fn draw_model(
        &self,
        gpu: &mut Gpu,
        st: &mut DrawState,
        a: &Assets<'_, '_>,
        id: u16,
        m: &Model,
    ) {
        let Some(State::Model(s)) = self.state(id) else {
            return;
        };
        let pack = a.model_pack(s.key.0);
        let mut posed = [Mat4::IDENTITY; MAX_NODES];
        let n = compose(pack, &s.object, &s.poses, &mut posed);
        gpu.model_transform([0.0; 3], [0.0; 3], MODEL_SCALE);
        let base = gpu.model_matrix();
        let _ = m;
        if s.joints.iter().flatten().any(|j| j.flags & 3 != 0) {
            meshdraw::draw_object_posed_hiding(
                pack,
                &s.object,
                &base,
                &posed[..n],
                st,
                s.mats.as_deref(),
                &|g| s.hidden(pack, g),
            );
            return;
        }
        meshdraw::draw_object_posed(
            pack,
            &s.object,
            &base,
            &posed[..n],
            None,
            st,
            None,
            s.mats.as_deref(),
            0,
        );
    }

    /// Model `id` drawn under the current camera, for a host's own display
    /// of an [`ObjectKind::HostModel`].
    ///
    /// # Safety
    ///
    /// Between `begin_frame` and `end_frame`.
    pub unsafe fn draw_host_model(
        &self,
        gpu: &mut Gpu,
        st: &mut DrawState,
        a: &Assets<'_, '_>,
        id: u16,
        m: &Model,
    ) {
        self.draw_model(gpu, st, a, id, m);
    }

    #[allow(clippy::too_many_arguments)]
    unsafe fn draw_fighter(
        &self,
        _gpu: &mut Gpu,
        st: &mut DrawState,
        a: &Assets<'_, '_>,
        id: u16,
        f: &Fighter,
        light: [f32; 2],
    ) {
        let Some(State::Fighter(s)) = self.state(id) else {
            return;
        };
        let Some(object) = a.main.object(s.object) else {
            return;
        };
        let mut posed = [Mat4::IDENTITY; ssb_rom::skeleton::MAX_NODES];
        let n = s.compose(a.main, &object, &mut posed);
        let base = psp_matrix(&s.root(f));
        st.configure_fighter_light(light);
        let parts = s.demo.parts.draw_parts();
        meshdraw::draw_fighter_posed(
            a.main,
            &object,
            &base,
            &posed[..n],
            st,
            meshdraw::Look {
                costume: u32::from(f.costume),
                textures: s.demo.parts.draw_textures(),
                parts: parts.as_ref().map(|p| &p[..]),
                accessory_before: f.kind == FighterKind::Purin,
            },
        );
        st.finish_fighter_light();
    }
}

fn admits(object: Head, pass: Head) -> bool {
    match object {
        Head::Links => true,
        h => h == pass,
    }
}

/// A scene matrix as the GE's.
pub fn psp_matrix(m: &Mat4) -> psp::sys::ScePspFMatrix4 {
    use psp::sys::{ScePspFMatrix4, ScePspFVector4};
    let c = |i: usize| ScePspFVector4 {
        x: m.0[i * 4],
        y: m.0[i * 4 + 1],
        z: m.0[i * 4 + 2],
        w: m.0[i * 4 + 3],
    };
    ScePspFMatrix4 {
        x: c(0),
        y: c(1),
        z: c(2),
        w: c(3),
    }
}

/// `syVectorRotateAbout3D`: `dst` turned by `angle` about `dir`.
fn rotate_about(dst: [f32; 3], dir: [f32; 3], angle: f32) -> [f32; 3] {
    use ssb_engine::math::{sin_cos, sqrt};
    let mag_yz = sqrt(dir[1] * dir[1] + dir[2] * dir[2]);
    let (s, c) = sin_cos(angle);
    let (rot_x, rot_y, rot_z, ratio_y, ratio_z);
    if mag_yz != 0.0 {
        ratio_z = dir[2] / mag_yz;
        ratio_y = dir[1] / mag_yz;
        rot_x = dst[0];
        rot_y = dst[1] * ratio_z - dst[2] * ratio_y;
        rot_z = dst[1] * ratio_y + dst[2] * ratio_z;
    } else {
        ratio_z = 0.0;
        ratio_y = 0.0;
        rot_x = dst[0];
        rot_y = dst[1];
        rot_z = dst[2];
    }
    let iz = rot_x * mag_yz - rot_z * dir[0];
    let ix = rot_x * dir[0] + rot_z * mag_yz;
    let rx = iz * c - rot_y * s;
    let ry = iz * s + rot_y * c;
    let iz = rx * mag_yz + ix * dir[0];
    let ix = -rx * dir[0] + ix * mag_yz;
    if mag_yz != 0.0 {
        [
            iz,
            ry * ratio_z + ix * ratio_y,
            -ry * ratio_y + ix * ratio_z,
        ]
    } else {
        [iz, ry, ix]
    }
}

/// The camera's view: `syMatrixLookAt` with up `(up.x, 1, 0)`, or
/// `syMatrixModLookAtF` rolled by `up.x` about the look direction.
pub fn view_matrix(eye: Vec3, at: Vec3, up_x: f32, view: View) -> ssb_engine::math::Mat4 {
    match view {
        View::LookAt => ssb_engine::math::Mat4::look_at(eye, at, Vec3::new(up_x, 1.0, 0.0)),
        View::Roll => {
            use ssb_engine::math::sqrt;
            let mut look = [at.x - eye.x, at.y - eye.y, at.z - eye.z];
            let len = -1.0 / sqrt(look[0] * look[0] + look[1] * look[1] + look[2] * look[2]);
            look = look.map(|v| v * len);
            let up = [0.0f32, 1.0, 0.0];
            let mut right = [
                up[1] * look[2] - up[2] * look[1],
                up[2] * look[0] - up[0] * look[2],
                up[0] * look[1] - up[1] * look[0],
            ];
            let len = 1.0 / sqrt(right[0] * right[0] + right[1] * right[1] + right[2] * right[2]);
            right = right.map(|v| v * len);
            right = rotate_about(right, look, up_x);
            let mut u = [
                look[1] * right[2] - look[2] * right[1],
                look[2] * right[0] - look[0] * right[2],
                look[0] * right[1] - look[1] * right[0],
            ];
            let len = 1.0 / sqrt(u[0] * u[0] + u[1] * u[1] + u[2] * u[2]);
            u = u.map(|v| v * len);
            let e = [eye.x, eye.y, eye.z];
            let dot = |a: [f32; 3]| -(e[0] * a[0] + e[1] * a[1] + e[2] * a[2]);
            ssb_engine::math::Mat4 {
                cols: [
                    [right[0], u[0], look[0], 0.0],
                    [right[1], u[1], look[1], 0.0],
                    [right[2], u[2], look[2], 0.0],
                    [dot(right), dot(u), dot(look), 1.0],
                ],
            }
        }
    }
}

/// `lbCommonPrepSObjDraw`'s whole-pixel corner.
fn corner(v: f32) -> f32 {
    if v < 0.0 {
        (v - 0.9999) as i32 as f32
    } else {
        v as i32 as f32
    }
}

/// One `SObj` through `lbCommonDrawSObjAttr`.
///
/// # Safety
///
/// Between `begin_frame` and `end_frame`.
pub unsafe fn draw_piece(a: &Assets<'_, '_>, st: &mut DrawState, piece: &Piece) {
    const SP_FASTCOPY: u16 = 0x0020;
    let Some((p, s)) = a.sprite(piece.file, piece.offset) else {
        return;
    };
    let [r, g, b, alpha] = s.color;
    let alpha = piece.alpha.unwrap_or(alpha);
    let prim = piece
        .prim
        .map_or([r, g, b, alpha], |c| [c[0], c[1], c[2], alpha]);
    let [sx, sy] = piece.scale;
    if sx < 0.0001 || sy < 0.0001 {
        return;
    }
    let (mut x, mut y) = (piece.x, piece.y);
    if piece.centred {
        x -= f32::from(s.width) * sx * 0.5;
        y -= f32::from(s.height) * sy * 0.5;
    }
    let attr = if piece.transparent {
        (s.attr & !SP_FASTCOPY) | ssb_rom::sprite::SP_TRANSPARENT
    } else {
        s.attr & !SP_FASTCOPY
    };
    let d = meshdraw::SObjDraw {
        x: corner(x),
        y: corner(y),
        scale: 1.0,
        prim,
        env: piece.env,
        solid: piece.solid,
        attr,
    };
    meshdraw::draw_sprite_xy(p, &s, &d, [sx, sy], st);
}

/// One `SObj` through `lbCommonDrawSObjNoAttr` behind a custom display's
/// `G_CC(0, 0, 0, TEXEL0, 0, 0, 0, PRIMITIVE)` and `G_RM_AA_XLU_SURF`:
/// the texel's colour, blended at the primitive alpha `alpha` (Sector Z's
/// cockpit). `TexFunc::Blend` from black to white gives the texel's colour
/// and modulates its alpha by `alpha`; the N64 ignores the texel's alpha,
/// so a sprite with partly transparent texels draws them fainter here.
///
/// # Safety
///
/// Between `begin_frame` and `end_frame`.
pub unsafe fn draw_piece_texel_at_alpha(
    a: &Assets<'_, '_>,
    st: &mut DrawState,
    piece: &Piece,
    alpha: u8,
) {
    const SP_FASTCOPY: u16 = 0x0020;
    let Some((p, s)) = a.sprite(piece.file, piece.offset) else {
        return;
    };
    let [sx, sy] = piece.scale;
    if sx < 0.0001 || sy < 0.0001 {
        return;
    }
    let mut tinted = s;
    tinted.flags |= SpriteDesc::TINTED;
    let d = meshdraw::SObjDraw {
        x: corner(piece.x),
        y: corner(piece.y),
        scale: 1.0,
        prim: [0xFF, 0xFF, 0xFF, alpha],
        env: [0, 0, 0],
        solid: false,
        attr: (s.attr & !SP_FASTCOPY) | ssb_rom::sprite::SP_TRANSPARENT,
    };
    meshdraw::draw_sprite_xy(p, &tinted, &d, [sx, sy], st);
}

/// Draws model `m` alone under the current camera: a scene's own display
/// (e.g. the cliff's hills, with no depth test).
///
/// # Safety
///
/// Between `begin_frame` and `end_frame`.
pub unsafe fn draw_static_model(gpu: &mut Gpu, st: &mut DrawState, a: &Assets<'_, '_>, m: &Model) {
    let pack = a.model_pack(m.file);
    let Some(object) = crate::scene::object_keyed(pack, (m.file, m.graph)) else {
        return;
    };
    gpu.model_transform([0.0; 3], [0.0; 3], MODEL_SCALE);
    let base = gpu.model_matrix();
    meshdraw::draw_object(pack, &object, &base, st, None, 0);
}

impl Runtime {
    /// The joints' file a model plays from (for the host's own displays).
    pub fn has(&self, id: u16) -> bool {
        self.state(id).is_some()
    }
}
