//! Layer C: the PSP executable.
//!
//! Milestone M1 — the platform baseline the rest of the port is built on. It
//! proves the things that must work before any Smash code is worth writing:
//! the toolchain produces a bootable EBOOT, the display and GE initialise, the
//! GPU draws, the controller reads, and the fixed 60 Hz clock ticks correctly
//! against real hardware timers.
//!
//! What is on screen is a testbed, not the game. It runs the real
//! [`ssb_engine`] clock and the real [`ssb_game`] physics so that both are
//! exercised on-device from day one, rather than only in host tests.

#![no_std]
#![no_main]

// The asset pack is loaded into a heap buffer; `psp` provides the allocator.
extern crate alloc;

mod assets;
mod gu;
mod input;
mod meshdraw;
mod play;
mod timing;

use core::f32::consts::PI;

use psp::Align16;

use ssb_engine::coord;
use ssb_engine::input::{Input, N64Buttons};
use ssb_engine::renderer::Color;
use ssb_engine::timing::{Clock, FixedClock, FRAME_BUDGET_US};

use ssb_rom::pack::{Pack, PackError};

use gu::{Gpu, GuVertex};
use input::PspInput;
use timing::{PspClock, Stopwatch};

/// R0.17's deterministic capture mode. Every per-frame mutation (physics,
/// skeleton/stage/material animation) reads its own wall-clock-independent
/// tick count and no randomness anywhere in the sim, so two runs that reach
/// the same tick count are byte-identical -- *if* both are stopped there.
/// Left running, they are not: an idle animation loops, so which tick a
/// screenshot lands on still depends on host speed and OS scheduling jitter
/// between the capture script's `--seconds N` and the emulator's own frame
/// pacing. Freezing every mutation past a fixed tick count removes that
/// dependency entirely -- a screenshot at tick 240 and one at tick 600 are
/// the same PNG, so the capture script's timing no longer has to be exact,
/// only "late enough".
#[cfg(feature = "regression_capture")]
mod regression {
    /// 4 real seconds at the sim's fixed 60 Hz -- comfortably past Mario
    /// landing from Dream Land's spawn height (RE-098's own costume-cycle
    /// testing never saw a fall take more than ~30 ticks).
    pub const TARGET_TICKS: u64 = 240;
}

/// `true` once `regression_capture` has frozen the sim; always `false`
/// otherwise, so callers need one guard, not a cfg per call site.
#[inline]
fn regression_frozen(sim_frame_index: u64) -> bool {
    #[cfg(feature = "regression_capture")]
    {
        sim_frame_index >= regression::TARGET_TICKS
    }
    #[cfg(not(feature = "regression_capture"))]
    {
        let _ = sim_frame_index;
        false
    }
}

psp::module!("ssb64_psp", 1, 0);

/// Scratch for the texture-inspection quad. Align16 because the GE DMAs it.
static mut TEX_QUAD: Align16<[meshdraw::TexQuadVertex; 6]> = Align16(
    [meshdraw::TexQuadVertex {
        u: 0.0,
        v: 0.0,
        color: 0,
        x: 0.0,
        y: 0.0,
        z: 0.0,
    }; 6],
);

/// A unit tetrahedron, vertex-coloured. Stands in for a fighter until real
/// geometry conversion lands.
///
/// `Align16` matters: the GE DMAs vertex data directly and requires 16-byte
/// alignment.
static TRIANGLE: Align16<[GuVertex; 12]> = Align16([
    // front
    GuVertex::new(0.0, 1.0, 0.0, 0.0, 0.0, 0xFF00_00FF),
    GuVertex::new(-1.0, -1.0, 1.0, 0.0, 0.0, 0xFF00_FF00),
    GuVertex::new(1.0, -1.0, 1.0, 0.0, 0.0, 0xFFFF_0000),
    // right
    GuVertex::new(0.0, 1.0, 0.0, 0.0, 0.0, 0xFF00_00FF),
    GuVertex::new(1.0, -1.0, 1.0, 0.0, 0.0, 0xFFFF_0000),
    GuVertex::new(0.0, -1.0, -1.0, 0.0, 0.0, 0xFFFF_FF00),
    // left
    GuVertex::new(0.0, 1.0, 0.0, 0.0, 0.0, 0xFF00_00FF),
    GuVertex::new(0.0, -1.0, -1.0, 0.0, 0.0, 0xFFFF_FF00),
    GuVertex::new(-1.0, -1.0, 1.0, 0.0, 0.0, 0xFF00_FF00),
    // bottom
    GuVertex::new(-1.0, -1.0, 1.0, 0.0, 0.0, 0xFF00_FF00),
    GuVertex::new(0.0, -1.0, -1.0, 0.0, 0.0, 0xFFFF_FF00),
    GuVertex::new(1.0, -1.0, 1.0, 0.0, 0.0, 0xFFFF_0000),
]);

/// Starts animation `index` and returns the object its joints drive.
///
/// The pairing is stored, not searched for: a joint names an absolute node, and
/// a node belongs to exactly one object. Finding that object is the one hop the
/// pack does not store, and it is a scan over 363 objects done once per
/// selection rather than per frame.
fn start_anim(
    pack: &ssb_rom::pack::Pack<'_>,
    index: u32,
    skeleton: &mut ssb_rom::skeleton::Skeleton,
) -> Option<u32> {
    let anim = pack.anim(index)?;
    skeleton.start(pack, &anim, 0.0, 1.0);
    let node = (0..anim.joint_count)
        .filter_map(|i| pack.anim_joint(anim.first_joint + i))
        .map(|j| j.node)
        .find(|&n| n != ssb_rom::pack::AnimJoint::NO_NODE)?;
    (0..pack.object_count()).find(|&i| {
        pack.object(i)
            .is_some_and(|o| node >= o.first_node && node < o.first_node + o.node_count)
    })
}

/// Which way to turn a model so it faces the way the fighter does.
///
/// Fighter models are authored facing `+Z` — shoulders spanning X — while a
/// match runs along X, so every one of them is a quarter turn off (RE-038).
fn facing_turn(facing: ssb_game::fighter::Facing) -> f32 {
    match facing {
        ssb_game::fighter::Facing::Right => core::f32::consts::FRAC_PI_2,
        ssb_game::fighter::Facing::Left => -core::f32::consts::FRAC_PI_2,
    }
}

fn psp_main() {
    psp::enable_home_button();
    unsafe { run() }
}

unsafe fn run() -> ! {
    let mut gpu = Gpu::init();
    let mut pad = PspInput::init();
    let clock = PspClock;

    // Load the converted asset pack. Held for the whole program: the GE reads
    // vertex and texture data out of it by DMA every frame.
    let loaded = assets::load_pack();
    let (pack_buf, pack_path) = match &loaded {
        Ok((b, p)) => (Some(b), *p),
        Err(e) => (None, e.as_str()),
    };
    // A pack that loads from disk but fails to *parse* is a different problem
    // from having no pack, and discarding the error made the two look
    // identical: a stale pack from an older format version silently showed the
    // fallback tetrahedron, exactly like a first run with no assets.
    let opened = pack_buf.map(|b| Pack::open(b.as_slice()));
    let pack = match &opened {
        Some(Ok(p)) => Some(p),
        _ => None,
    };
    let pack_status: &str = match &opened {
        Some(Ok(_)) | None => pack_path,
        Some(Err(PackError::BadVersion(_))) => "REJECTED: stale pack, re-run romtool pack",
        Some(Err(PackError::BadMagic(_))) => "REJECTED: not a pack file",
        Some(Err(PackError::TooSmall)) => "REJECTED: truncated",
        Some(Err(PackError::OutOfBounds)) => "REJECTED: descriptor out of bounds",
    };
    let mesh_count = pack.as_ref().map_or(0, |p| p.mesh_count());

    // Which mesh is on screen, and how far back the camera sits.
    // Start on the mesh with the most triangles, so the first frame shows
    // something substantial rather than a two-triangle sliver.
    // Start on the best *textured* mesh, so the first frame exercises the
    // texture path -- the part of the pipeline with the least evidence behind
    // it. Falls back to the densest mesh if nothing is textured.
    let mut mesh_index: u32 = pack
        .as_ref()
        .map(|p| {
            let mut best_textured = (0u32, 0u32);
            let mut best_any = (0u32, 0u32);
            for i in 0..p.mesh_count() {
                let Some(m) = p.mesh(i) else { continue };
                let mut tris = 0u32;
                let mut textured = false;
                for k in 0..m.prim_count {
                    let Some(pr) = p.prim(m.first_prim + k) else {
                        continue;
                    };
                    tris += pr.index_count / 3;
                    textured |= pr.texture != ssb_rom::pack::PrimDesc::NO_TEXTURE;
                }
                if tris > best_any.1 {
                    best_any = (i, tris);
                }
                if textured && tris > best_textured.1 {
                    best_textured = (i, tris);
                }
            }
            if best_textured.1 > 0 {
                best_textured.0
            } else {
                best_any.0
            }
        })
        .unwrap_or(0);
    /// Zoom at which a stage exactly fits the view. Below it the camera is
    /// closer than the stage is wide, so there is a fighter to follow.
    const CAM_FIT: f32 = 200.0;
    let mut cam_distance = CAM_FIT;
    let mut draw_state = meshdraw::DrawState::default();
    // Texture inspection mode (C_UP toggles). Proven working, so the mesh
    // view is the default; kept because it cleanly separates an upload bug
    // from a mesh-data bug.
    let mut tex_view = false;
    let mut tex_index: u32 = 0;
    let tex_count = pack.as_ref().map_or(0, |p| p.texture_count());

    // Object view: a whole DObjDesc hierarchy assembled from its baked node
    // transforms, rather than one mesh floating at the origin. This is the
    // default because it is the only mode that shows geometry where the game
    // actually puts it. C_DOWN falls back to single-mesh browsing.
    let object_count = pack.as_ref().map_or(0, |p| p.object_count());
    let mut object_view = object_count > 0;
    // Start on the deepest hierarchy, tie-broken by triangle count.
    //
    // Three rankings were tried before this one and all picked something that
    // misrepresents the mode. Most nodes got a 51-node skybox whose panels are
    // two triangles each; most triangles got an `LBTransition` screen wipe,
    // 1000 triangles of subdivided flat plane on a single node; most *placed*
    // nodes got `MVCommon`, 38 cutscene panels that fill the screen with
    // texture and look exactly like a rendering bug.
    //
    // Depth is the metric that means "assembled hierarchy" -- the one thing
    // this mode shows that the mesh view does not. A fighter is seven joints
    // deep; a panel mosaic is one.
    let mut object_index: u32 = pack
        .as_ref()
        .map(|p| {
            let mut best = (0u32, 0u32, 0u32);
            for i in 0..p.object_count() {
                let Some(o) = p.object(i) else { continue };
                let mut depth = 0u32;
                let mut tris = 0u32;
                for k in 0..o.node_count {
                    let Some(n) = p.node(o.first_node + k) else {
                        continue;
                    };
                    // Node parents always precede their children, so walking up
                    // terminates; cap anyway rather than trust the data.
                    let mut d = 0u32;
                    let mut at = n.parent;
                    while at != ssb_rom::pack::NodeDesc::NO_PARENT && d < 32 {
                        d += 1;
                        let Some(up) = p.node(at) else { break };
                        at = up.parent;
                    }
                    depth = depth.max(d);

                    if n.mesh == ssb_rom::pack::NodeDesc::NO_MESH {
                        continue;
                    }
                    if let Some(m) = p.mesh(n.mesh) {
                        for j in 0..m.prim_count {
                            if let Some(pr) = p.prim(m.first_prim + j) {
                                tris += pr.index_count / 3;
                            }
                        }
                    }
                }
                if (depth, tris) > (best.1, best.2) {
                    best = (i, depth, tris);
                }
            }
            best.0
        })
        .unwrap_or(0);
    // RE-098: no real costume-selection game system exists yet, so the only
    // way to see whether a fighter's alternate costumes actually render is
    // the same debug-viewer-cycle precedent `RE-095`'s `MaterialAnimator`
    // verification used. `L` steps through `0..costume_count(object_index)`;
    // out-of-range values are impossible by construction (`% ...max(1)`), and
    // a fresh object with fewer costumes than the previous one silently
    // clamps rather than drawing with a stale, meaningless index.
    let mut costume_index: u32 = 0;

    // Stage view: a whole stage -- its render layers assembled, with its
    // collision polylines drawn over them. This is the default when the pack
    // has stages, because it is the only view that shows a *place* rather than
    // an asset, and because the overlay is the check that geometry and
    // collision agree about where that place is. `romtool collide` proves the
    // spawns land; nothing but looking at it proves they land in the right
    // spot on screen.
    let stage_count = pack.as_ref().map_or(0, |p| p.stage_count());
    let mut stage_view = stage_count > 0;
    let mut stage_index: u32 = 0;
    // Stage scenery animation (RE-051). Restarted whenever the stage changes,
    // and ticked once per frame beside the fighter's own skeleton.
    let mut stage_anim = ssb_rom::skeleton::StageAnimator::new();
    let mut stage_anim_loaded: Option<u32> = None;
    let mut show_collision = true;

    // Material animation (RE-089-095): a `MatAnimDesc` entry is a property of
    // a *texture*, not a stage layer or a fighter, so unlike `stage_anim`
    // above there is no per-object "start" boundary to restart on -- it loads
    // once here, when the pack loads, and ticks every frame for as long as
    // the pack is loaded, independent of which stage or fighter is shown.
    let mut material_anim = ssb_rom::skeleton::MaterialAnimator::new();
    if let Some(p) = &pack {
        material_anim.start(p);
    }

    // The gameplay slice: one fighter, placed at the stage's first spawn and
    // ticked against its collision every simulation step. This is the join
    // between the ported physics and the ported collision, and running it here
    // is the only way to see it happen at 60 Hz rather than in a test.
    let mut sim_fighter = true;
    let mut player = match (&pack, stage_count > 0) {
        (Some(p), true) => p
            .stage(stage_index)
            .map(|s| play::Play::at_spawn(p, &s)),
        _ => None,
    };

    // The animation viewer. Pack version 6 stores, per animation, each joint's
    // script and the node it drives (RE-036), so playing one is: start a
    // skeleton, tick it, recompose the object's matrices. Browsing it here is
    // the only way to see whether a pose is right -- a host test can say the
    // numbers match the ROM, which they do, and still not say the fighter
    // looks like it is running.
    let anim_count = pack.as_ref().map_or(0, |p| p.anim_count());
    let mut anim_index: u32 = 0;
    let mut anim_playing = false;
    let mut skeleton = ssb_rom::skeleton::Skeleton::new();
    let mut posed = [ssb_rom::scene::Mat4::IDENTITY; ssb_rom::skeleton::MAX_NODES];
    let mut posed_len = 0usize;

    let mut sim = FixedClock::new(clock.now_us());

    let (_vx, _, vw, vh) = coord::pillarboxed_viewport();
    let aspect = vw as f32 / vh as f32;

    let mut spin = 0.0f32;
    let mut last_frame_us = 0u32;
    let mut dbg_cam = 1000.0f32;
    // Simulation-tick count since boot, independent of wall-clock/frame
    // pacing. Only consulted by `regression_frozen` (R0.17); harmless to
    // maintain unconditionally.
    let mut sim_frame_index = 0u64;

    loop {
        let frame = Stopwatch::start();

        // ---- simulate: fixed 60 Hz, decoupled from display ----------------
        let ticks = sim.advance(clock.now_us());

        for _ in 0..ticks {
            sim_frame_index = sim_frame_index.saturating_add(1);
            pad.poll();
            let state = pad.state(0);

            // D-pad steps through the pack; held Z zooms out, A zooms in.
            let prev = pad.previous(0).buttons;
            let pressed = ssb_engine::input::newly_pressed(prev, state.buttons);
            if pressed.contains(N64Buttons::START) && stage_count > 0 {
                stage_view = !stage_view;
            }
            if pressed.contains(N64Buttons::C_DOWN) && object_count > 0 {
                object_view = !object_view;
            }
            // B in the object view starts and stops animation playback.
            if pressed.contains(N64Buttons::B) && object_view && anim_count > 0 {
                anim_playing = !anim_playing;
                if anim_playing {
                    if let Some(p) = &pack {
                        object_index =
                            start_anim(p, anim_index, &mut skeleton).unwrap_or(object_index);
                    }
                } else {
                    posed_len = 0;
                }
            }

            // One tick of every joint, at the simulation rate rather than the
            // frame rate -- animation timing is gameplay timing (RE-035).
            if anim_playing && object_view && !regression_frozen(sim_frame_index) {
                if let Some(p) = &pack {
                    if skeleton.ended() {
                        start_anim(p, anim_index, &mut skeleton);
                    }
                    if let Some(a) = p.anim(anim_index) {
                        if let Some(script) = p.anim_script(&a) {
                            let _ = skeleton.tick(script);
                        }
                    }
                }
            }
            if stage_view && stage_count > 0 {
                let was = stage_index;
                if pressed.contains(N64Buttons::D_RIGHT) {
                    stage_index = (stage_index + 1) % stage_count;
                }
                if pressed.contains(N64Buttons::D_LEFT) {
                    stage_index = (stage_index + stage_count - 1) % stage_count;
                }
                // Toggling the overlay is what separates "the collision is
                // wrong" from "the geometry is wrong": with it off you see
                // only the stage, with it on you see only whether they line up.
                if pressed.contains(N64Buttons::B) {
                    show_collision = !show_collision;
                }
                if pressed.contains(N64Buttons::C_UP) {
                    sim_fighter = !sim_fighter;
                }
                // Walking off a ledge is a *correct* outcome, so there has to
                // be a way back onto the stage without restarting.
                let respawn = pressed.contains(N64Buttons::C_RIGHT) || was != stage_index;
                if respawn {
                    if let Some(p) = &pack {
                        player = p
                            .stage(stage_index)
                            .map(|s| play::Play::at_spawn(p, &s));
                    }
                }

                // The tick itself. Ordered after input so a respawn this frame
                // starts falling this frame rather than next.
                if sim_fighter && !regression_frozen(sim_frame_index) {
                    if let (Some(p), Some(pl)) = (&pack, player.as_mut()) {
                        if let Some(s) = p.stage(stage_index) {
                            // C-left jumps. The original uses any C-button, but
                            // three of the four are taken by the viewer's own
                            // controls and remapping them would make the stage
                            // browser worse to use than the fighter is to play.
                            let jump = state.buttons.contains(N64Buttons::C_LEFT);
                            pl.tick(p, &s, state, jump);
                        }
                    }
                }
            } else if object_view && object_count > 0 && anim_playing {
                // While an animation is playing the d-pad browses animations
                // rather than objects: the object is whichever one the
                // animation drives, so stepping it independently would only
                // ever break the pairing.
                let was = anim_index;
                if pressed.contains(N64Buttons::D_RIGHT) {
                    anim_index = (anim_index + 1) % anim_count;
                }
                if pressed.contains(N64Buttons::D_LEFT) {
                    anim_index = (anim_index + anim_count - 1) % anim_count;
                }
                // Whole fighters rather than one slot at a time.
                let slots = anim_count / 27.max(1);
                if pressed.contains(N64Buttons::D_UP) {
                    anim_index = (anim_index + slots.max(1)) % anim_count;
                }
                if pressed.contains(N64Buttons::D_DOWN) {
                    anim_index = (anim_index + anim_count - slots.max(1)) % anim_count;
                }
                if was != anim_index || skeleton.ended() {
                    if let Some(p) = &pack {
                        object_index =
                            start_anim(p, anim_index, &mut skeleton).unwrap_or(object_index);
                    }
                }
            } else if object_view && object_count > 0 {
                let was_object = object_index;
                if pressed.contains(N64Buttons::D_RIGHT) {
                    object_index = (object_index + 1) % object_count;
                }
                if pressed.contains(N64Buttons::D_LEFT) {
                    object_index = (object_index + object_count - 1) % object_count;
                }
                if pressed.contains(N64Buttons::D_UP) {
                    object_index = (object_index + 10) % object_count;
                }
                if pressed.contains(N64Buttons::D_DOWN) {
                    object_index = (object_index + object_count - 10) % object_count;
                }
                // 363 objects but only 134 source files, and neighbouring
                // objects almost always come from the same one. Stepping by
                // file is how you get from a stage to a fighter without
                // holding right for a minute.
                let step_file = |from: u32, dir: u32| -> u32 {
                    let Some(p) = pack.as_ref() else { return from };
                    let of = |i: u32| p.object(i).map(|o| o.source_file);
                    let start = of(from);
                    let mut i = from;
                    for _ in 0..object_count {
                        i = (i + dir) % object_count;
                        if of(i) != start {
                            break;
                        }
                    }
                    // Landing on a file's *last* object when stepping back is
                    // disorienting; walk to its first.
                    if dir != 1 {
                        let f = of(i);
                        while of((i + object_count - 1) % object_count) == f {
                            i = (i + object_count - 1) % object_count;
                        }
                    }
                    i
                };
                if pressed.contains(N64Buttons::R) {
                    object_index = step_file(object_index, 1);
                }
                if pressed.contains(N64Buttons::C_UP) {
                    object_index = step_file(object_index, object_count - 1);
                }
                // RE-098: `L` (mapped from the PSP's otherwise-idle SELECT
                // button) cycles which baked costume variant this object
                // draws with -- the only way to see a fighter's alternate
                // costumes render at all before any real costume-selection
                // game system exists, mirroring the debug-viewer-cycle
                // precedent `RE-095`'s `MaterialAnimator` verification used.
                let costume_count = pack
                    .as_ref()
                    .and_then(|p| p.object(object_index).map(|o| p.object_costume_count(&o)))
                    .unwrap_or(1);
                if was_object != object_index {
                    // A different object may have fewer costumes than the
                    // one just left; a stale index would silently draw
                    // whatever the last override in the table happens to be.
                    costume_index = 0;
                } else if pressed.contains(N64Buttons::L) {
                    costume_index = (costume_index + 1) % costume_count.max(1);
                }
            } else if mesh_count > 0 {
                if pressed.contains(N64Buttons::D_RIGHT) {
                    mesh_index = (mesh_index + 1) % mesh_count;
                }
                if pressed.contains(N64Buttons::D_LEFT) {
                    mesh_index = (mesh_index + mesh_count - 1) % mesh_count;
                }
                if pressed.contains(N64Buttons::D_UP) {
                    mesh_index = (mesh_index + 50) % mesh_count;
                }
                if pressed.contains(N64Buttons::D_DOWN) {
                    mesh_index = (mesh_index + mesh_count - 50) % mesh_count;
                }
            }
            if pressed.contains(N64Buttons::C_UP) {
                tex_view = !tex_view;
            }
            if tex_view {
                if pressed.contains(N64Buttons::D_RIGHT) {
                    tex_index = (tex_index + 1) % tex_count.max(1);
                }
                if pressed.contains(N64Buttons::D_LEFT) {
                    tex_index = (tex_index + tex_count.max(1) - 1) % tex_count.max(1);
                }
            }
            if state.buttons.contains(N64Buttons::Z) {
                cam_distance *= 1.03;
            }
            if state.buttons.contains(N64Buttons::A) {
                cam_distance *= 0.97;
            }
            // The stick spins the model so geometry can be inspected -- except
            // while a fighter is being driven, where it is the fighter's input
            // and a camera that swung with it would be unreadable.
            let stick_drives_fighter = stage_view && sim_fighter && player.is_some();
            // The slow drift exists so a *static* model can be seen from all
            // sides without touching anything. While an animation is playing it
            // makes the thing being judged unjudgeable: two captures seconds
            // apart differ by most of a turn, and the difference reads as the
            // pose having changed. That cost real time (RE-038), so playback
            // holds the angle still and leaves the stick in charge.
            if !anim_playing {
                spin += 0.02;
            }
            spin += if stick_drives_fighter {
                0.0
            } else {
                state.stick_x as f32 * 0.0005
            };
            if spin > 2.0 * PI {
                spin -= 2.0 * PI;
            }
        }

        let cpu = Stopwatch::start();

        // ---- render: display cadence -------------------------------------
        gpu.begin_frame(Some(Color::rgba(0x20, 0x28, 0x38, 0xFF)));
        // Far plane follows the camera: meshes range from a few units across to
        // tens of thousands, and a fixed far plane clips the large ones entirely.
        //
        // 38 degrees, not an arbitrary round number: `refs/ssb-decomp-re/src/
        // gm/gmcamera.c:1191` sets `gGMCameraStruct.fovy = 38.0F` as the
        // default battle camera's FOV, and every normal camera-behavior
        // function that resets it (`gmCameraAdjustFOV(38.0F)`, four call
        // sites) targets the same value; only two special modes (a
        // KO/photo-finish player-zoom and a player-follow camera) take a
        // different, situational FOV from their own caller (RE-084). This
        // viewer has no real camera system yet, so this is the closest
        // still-correct thing to reproduce: the original's own default,
        // not this project's previous unsourced guess of `60.0`.
        gpu.set_perspective(38.0, aspect, 1.0, (dbg_cam * 4.0).max(10_000.0));
        gpu.reset_modelview();
        draw_state.begin_frame();
        // RE-115: the object-view inspection camera has no guarantee it
        // views authored geometry from its intended front side (unlike a
        // real game camera, which always does) -- a one-sided plane, viewed
        // from behind, is real geometry rendering nothing, not a bug in the
        // material/UV/texture pipeline. Object view exists specifically to
        // inspect whatever is there, so it must not hide half of it.
        draw_state.force_no_cull = object_view;
        // RE-131: reset every frame, then set for real inside `stage_view`'s
        // own real-camera branch below -- every other mode's view matrix is
        // identity, so `None` (leave the billboard basis at whatever
        // `sceGumLoadIdentity` set) is correct for them, matching this
        // code's own behaviour before the real camera existed.
        draw_state.billboard_camera = None;
        if let Some(p) = &pack {
            if !regression_frozen(sim_frame_index) {
                material_anim.tick(p);
            }
        }

        let mut shown = (0u32, 0u32, 0u32); // tris, verts, prims
        let mut dbg_bb = [0i32; 6];
        let mut dbg_tex = 0u32;

        let mut dbg_radius = 0.0f32;
        match &pack {
            Some(p) if tex_view => {
                // Flat orthographic-ish view of one texture.
                gpu.model_transform([0.0, 0.0, -2.2], [0.0, 0.0, 0.0], 1.0);
                meshdraw::draw_texture_quad(p, tex_index, &mut TEX_QUAD.0);
            }
            Some(p) if stage_view => {
                if let Some(stage) = p.stage(stage_index) {
                    // Frame the stage from its collision *and* its geometry:
                    // collision alone misses the scenery, geometry alone can
                    // be swamped by a skybox.
                    let (centre, radius) = match meshdraw::stage_bounds(p, &stage) {
                        Some((min, max)) => {
                            let c = [
                                (min[0] + max[0]) * 0.5,
                                (min[1] + max[1]) * 0.5,
                                (min[2] + max[2]) * 0.5,
                            ];
                            let e = ssb_engine::math::Vec3 {
                                x: max[0] - min[0],
                                y: max[1] - min[1],
                                z: max[2] - min[2],
                            };
                            let s = meshdraw::MODEL_SCALE;
                            (
                                [c[0] * s, c[1] * s, c[2] * s],
                                (e.length() * 0.5 * s).max(1.0),
                            )
                        }
                        None => ([0.0; 3], 1000.0),
                    };
                    // d = r / tan(19) ~= 2.90 * r, matching the 38-degree FOV
                    // above (half-angle 19). Was 1.733 (`1/tan(30)`) for the
                    // FOV's old, unsourced 60-degree guess -- keeps "whole
                    // stage fits at the default zoom" true after RE-084's fix
                    // narrowed the FOV, rather than silently cropping every
                    // stage.
                    const FIT: f32 = 2.904;
                    let dist = radius * FIT * cam_distance / 200.0;
                    dbg_radius = radius;
                    dbg_cam = centre[2] + dist;

                    // Once zoomed in past the whole-stage framing, hand
                    // framing to the real battle camera (RE-131) instead of
                    // the debug viewer's own fixed, face-on one. The stage's
                    // bounding centre is the right thing to look at while
                    // inspecting a stage, but it sits up in the scenery -- on
                    // Dream Land it is inside the tree -- so a naive
                    // "look at the fighter" would walk it off the bottom of
                    // the screen; the real camera's own framing formula
                    // (`gmCameraUpdateInterests`) already accounts for this
                    // via `cam_offset_y`. At the default zoom the stage still
                    // wins, because that whole-stage framing is what makes
                    // the collision overlay legible -- a real per-object
                    // camera is only meaningful once there is an object (the
                    // fighter) to actually watch.
                    let use_real_camera = sim_fighter && cam_distance < CAM_FIT && player.is_some();
                    let cam = if let (true, Some(pl)) = (use_real_camera, &player) {
                        // The real camera's own `eye`/`at` are a genuine
                        // `look_at`, not the "translate the world" trick the
                        // whole-stage framing below still uses -- it can pan
                        // and (via `light_angle.z`) tilt, which a pure
                        // translation cannot represent.
                        gpu.set_view(&ssb_engine::math::Mat4::look_at(
                            pl.camera.eye,
                            pl.camera.at,
                            ssb_engine::math::Vec3::Y,
                        ));
                        // RE-131: `Kind46`'s billboards (RE-048/049) need the
                        // real camera's own right/up to stay screen-aligned
                        // now that the view matrix actually rotates -- see
                        // `DrawState::billboard_camera`'s own doc comment.
                        let forward = (pl.camera.at - pl.camera.eye).normalized();
                        let right = forward.cross(ssb_engine::math::Vec3::Y).normalized();
                        let up = right.cross(forward);
                        // RE-132: `Kind48`'s own camera-pitch-locked basis --
                        // collapse the camera's real X/Z position into a
                        // single horizontal distance first (`objdisplay.c`
                        // case 48's own `eye_z = sqrt(dx^2 + dz^2)`), so the
                        // resulting `right` is always a world horizontal axis
                        // (invariant to yaw) while `up`/the implied forward
                        // still tilt with the camera's real vertical angle.
                        let dx = pl.camera.at.x - pl.camera.eye.x;
                        let dz = pl.camera.at.z - pl.camera.eye.z;
                        let dist_xz = ssb_engine::math::sqrt(dx * dx + dz * dz);
                        let pitch_forward = ssb_engine::math::Vec3::new(
                            0.0,
                            pl.camera.at.y - pl.camera.eye.y,
                            -dist_xz,
                        )
                        .normalized();
                        let pitch_right =
                            pitch_forward.cross(ssb_engine::math::Vec3::Y).normalized();
                        let pitch_up = pitch_right.cross(pitch_forward);
                        draw_state.billboard_camera = Some(meshdraw::BillboardCamera {
                            screen: (right.to_array(), up.to_array()),
                            pitch_locked: (pitch_right.to_array(), pitch_up.to_array()),
                        });
                        dbg_cam = pl.camera.eye.z;
                        [0.0, 0.0, 0.0]
                    } else {
                        // A stage is a place, not an object: spinning it
                        // would make the collision overlay impossible to
                        // read against the geometry. Face-on, always.
                        [-centre[0], -centre[1], -centre[2] - dist]
                    };
                    gpu.model_transform(cam, [0.0, 0.0, 0.0], meshdraw::MODEL_SCALE);
                    let base = gpu.model_matrix();
                    // (Re)load when the stage changes, then tick and draw.
                    if stage_anim_loaded != Some(stage_index) {
                        stage_anim_loaded = Some(stage_index);
                        match p.stage_anim(stage_index) {
                            Some(a) => stage_anim.start(p, &a),
                            None => stage_anim = ssb_rom::skeleton::StageAnimator::new(),
                        }
                    }
                    let animated = p.stage_anim(stage_index).and_then(|a| {
                        let script = p.anim_script(&a)?;
                        // A script that desynchronises stops the scenery rather
                        // than posing it from a garbage stream.
                        if !regression_frozen(sim_frame_index) {
                            stage_anim.tick(script).ok()?;
                        }
                        Some(())
                    });
                    let scenery = animated.map(|()| &stage_anim);
                    let (mut tris, layers) = meshdraw::draw_stage_animated(
                        p,
                        &stage,
                        &base,
                        scenery,
                        &mut draw_state,
                        Some(&material_anim),
                    );
                    let segments = if show_collision {
                        meshdraw::draw_collision(p, &stage, &base, &mut gpu, &mut draw_state)
                    } else {
                        0
                    };
                    // Drawn last so the marker is never hidden behind a layer
                    // it is standing in front of.
                    if sim_fighter {
                        if let Some(pl) = &player {
                            // The model, posed by whatever animation the
                            // fighter's status is playing, placed at its
                            // simulated position. The collision diamond is
                            // still drawn over it, because the point of this
                            // view is whether the two agree.
                            if let Some(obj) = p.object(pl.object) {
                                let n = pl.skeleton.compose(p, &obj, &mut posed);
                                let sc = meshdraw::MODEL_SCALE;
                                gpu.model_transform(
                                    [
                                        cam[0] + pl.fighter.pos.x,
                                        cam[1] + pl.fighter.pos.y,
                                        cam[2],
                                    ],
                                    // The models face +Z; a fighter faces
                                    // along X, so it is turned a quarter turn
                                    // one way or the other (RE-038).
                                    [0.0, facing_turn(pl.fighter.facing), 0.0],
                                    sc,
                                );
                                let m = gpu.model_matrix();
                                tris += meshdraw::draw_object_posed(
                                    p,
                                    &obj,
                                    &m,
                                    &posed[..n],
                                    None,
                                    &mut draw_state,
                                    Some(&material_anim),
                                    // No costume-selection game system exists
                                    // yet (R0.11); the simulated fighter here
                                    // always draws costume 0.
                                    0,
                                );
                                gpu.model_transform(cam, [0.0, 0.0, 0.0], sc);
                            }
                            meshdraw::draw_fighter(
                                pl.fighter.pos.to_array(),
                                &pl.fighter.coll,
                                pl.fighter.is_grounded(),
                                &base,
                                &mut gpu,
                                &mut draw_state,
                            );
                        }
                    }
                    shown = (tris, layers, segments);
                }
            }
            Some(p) if object_view => {
                if let Some(obj) = p.object(object_index) {
                    // Frame the whole hierarchy, not one node: an object's
                    // nodes are spread over the stage, so bounding only the
                    // first would put the camera inside the scene.
                    let (centre, radius) = match meshdraw::object_bounds(p, &obj) {
                        Some((min, max)) => {
                            let c = [
                                (min[0] + max[0]) * 0.5,
                                (min[1] + max[1]) * 0.5,
                                (min[2] + max[2]) * 0.5,
                            ];
                            let e = ssb_engine::math::Vec3 {
                                x: max[0] - min[0],
                                y: max[1] - min[1],
                                z: max[2] - min[2],
                            };
                            // object_bounds works in normalised units; the
                            // camera works in game units, like the mesh path.
                            let s = meshdraw::MODEL_SCALE;
                            // Bounding *sphere*, not the largest extent: the
                            // model spins, so a box that fits axis-aligned
                            // swings outside the frame a quarter turn later.
                            // That is what kept clipping Samus's head.
                            (
                                [c[0] * s, c[1] * s, c[2] * s],
                                (e.length() * 0.5 * s).max(1.0),
                            )
                        }
                        None => ([0.0; 3], 100.0),
                    };
                    // Fit the radius to the 38-degree vertical FOV (RE-084):
                    // d = r / tan 19 ~= 2.90 * r. The mesh path's 2x-extent
                    // rule leaves an object filling a ninth of the frame,
                    // which for a stage whose parts are spread over 33000
                    // units means specks.
                    const FIT: f32 = 2.904;
                    let dist = radius * FIT * cam_distance / 200.0;
                    dbg_radius = radius;
                    dbg_cam = centre[2] + dist;

                    gpu.model_transform(
                        [-centre[0], -centre[1], -centre[2] - dist],
                        [0.0, spin, 0.0],
                        meshdraw::MODEL_SCALE,
                    );
                    let base = gpu.model_matrix();
                    posed_len = if anim_playing {
                        skeleton.compose(p, &obj, &mut posed)
                    } else {
                        0
                    };
                    let tris = meshdraw::draw_object_posed(
                        p,
                        &obj,
                        &base,
                        &posed[..posed_len],
                        None,
                        &mut draw_state,
                        Some(&material_anim),
                        costume_index,
                    );
                    let placed = (0..obj.node_count)
                        .filter_map(|k| p.node(obj.first_node + k))
                        .filter(|n| n.mesh != ssb_rom::pack::NodeDesc::NO_MESH)
                        .count() as u32;
                    shown = (tris, obj.node_count, placed);
                }
            }
            Some(p) => {
                if let Some(desc) = p.mesh(mesh_index) {
                    // Frame the mesh: Smash's models range from a few dozen
                    // units across to several hundred, so a fixed camera
                    // distance would show either nothing or one huge polygon.
                    let bb = meshdraw::bounds(p, &desc);
                    let (centre, radius) = match bb {
                        Some((min, max)) => {
                            let c = [
                                (min[0] + max[0]) * 0.5,
                                (min[1] + max[1]) * 0.5,
                                (min[2] + max[2]) * 0.5,
                            ];
                            let e = [max[0] - min[0], max[1] - min[1], max[2] - min[2]];
                            let r = e[0].max(e[1]).max(e[2]).max(1.0);
                            (c, r)
                        }
                        None => ([0.0; 3], 100.0),
                    };

                    if let Some((mn, mx)) = bb {
                        dbg_bb = [
                            mn[0] as i32,
                            mn[1] as i32,
                            mn[2] as i32,
                            mx[0] as i32,
                            mx[1] as i32,
                            mx[2] as i32,
                        ];
                    }
                    dbg_radius = radius;
                    dbg_cam = centre[2] + radius * cam_distance / 100.0;

                    gpu.model_transform(
                        [
                            -centre[0],
                            -centre[1],
                            -centre[2] - radius * cam_distance / 100.0,
                        ],
                        [0.0, spin, 0.0],
                        meshdraw::MODEL_SCALE,
                    );
                    dbg_tex = (0..desc.prim_count)
                        .filter_map(|k| p.prim(desc.first_prim + k))
                        .filter(|pr| pr.texture != ssb_rom::pack::PrimDesc::NO_TEXTURE)
                        .count() as u32;
                    meshdraw::draw_mesh(p, &desc, &mut draw_state, Some(&material_anim));
                    shown = (draw_state.triangles, desc.vertex_count, desc.prim_count);
                }
            }
            None => {
                // No pack: fall back to the built-in tetrahedron so the
                // platform baseline is still visible and testable.
                gpu.model_transform([0.0, 0.0, -8.0], [spin * 0.4, spin, 0.0], 1.0);
                gpu.draw_triangles(&TRIANGLE.0);
            }
        }

        let cpu_us = cpu.elapsed_us();

        // Which browser is driving, so the readout describes what is on screen
        // rather than whichever index happens to be highest.
        let (mode, index, count) = if stage_view {
            ("stage", stage_index, stage_count)
        } else if object_view {
            ("obj  ", object_index, object_count)
        } else {
            ("mesh ", mesh_index, mesh_count)
        };
        let (src_file, src_offset) = pack
            .as_ref()
            .and_then(|p| {
                if stage_view {
                    p.stage(stage_index).map(|s| (s.source_file, s.source_offset))
                } else if object_view {
                    p.object(object_index).map(|o| (o.source_file, o.source_offset))
                } else {
                    p.mesh(mesh_index).map(|m| (m.source_file, m.source_offset))
                }
            })
            .unwrap_or((0, 0));

        // The fighter's own line. Positions are cast to integers because the
        // debug text has no float formatting, and a unit of game space is far
        // below what the overlay could show anyway.
        let (ft_state, ft_x, ft_y, ft_line, ft_mat, ft_air) = match &player {
            Some(pl) if sim_fighter => (
                if !pl.placed {
                    "no-spawn"
                } else {
                    // The status, not just ground/air: which of the two a
                    // fighter is in follows from the status rather than the
                    // other way round, and "walk-fst" says more than "ground".
                    pl.status_name()
                },
                pl.fighter.pos.x as i32,
                pl.fighter.pos.y as i32,
                pl.fighter.floor.map_or(-1, |f| f.line as i32),
                pl.material().unwrap_or(0),
                pl.airborne_ticks,
            ),
            _ => ("off     ", 0, 0, -1, 0, 0),
        };

        // The constants the fighter is actually running under, so a stale pack
        // shows up as "attrs built-in" rather than as mysteriously wrong
        // physics. Gravity is shown in tenths because the debug text has no
        // float formatting; Mario's 2.4 reads as 24.
        let (ft_src, ft_grav, ft_tvel, ft_bw, ft_bh, ft_dash, ft_land) = match &player {
            Some(pl) if sim_fighter => (
                if pl.from_pack { "pack    " } else { "built-in" },
                (pl.fighter.attributes.gravity * 10.0) as i32,
                pl.fighter.attributes.tvel_base as i32,
                pl.fighter.coll.width as i32,
                pl.fighter.coll.top as i32,
                pl.fighter.anim.dash as i32,
                pl.fighter.anim.landing as i32,
            ),
            _ => ("-       ", 0, 0, 0, 0, 0, 0),
        };

        // Which animation the skeleton is on, for the overlay. Read from the
        // pack rather than tracked separately so a wrong index shows up as a
        // wrong name instead of agreeing with itself.
        let (anim_fighter, anim_slot) = match pack.as_ref().and_then(|p| p.anim(anim_index)) {
            Some(a) => (a.fighter, a.slot),
            None => (0, 0),
        };

        // RE-098: only meaningful in object view (a stage or a loose mesh has
        // no costume table), but always computed and shown so a wrong index
        // surviving a mode switch would be visible rather than hidden.
        let costume_count_shown = pack
            .as_ref()
            .and_then(|p| p.object(object_index).map(|o| p.object_costume_count(&o)))
            .unwrap_or(1);

        const WHITE: u32 = 0xFFFF_FFFF;
        // Never drawn at all under `regression_capture`, from frame 0 --
        // not just hidden once frozen. Two narrower fixes were tried and
        // both failed for reasons specific to `sceGuDebugPrint` (RE-123,
        // RE-125): it is a PPSSPP-only debug overlay, not real GE drawing,
        // and it does not fully clear between calls. Pinning `cpu`/`frame`/
        // `tick` to `0` once frozen left old, longer digits ghosted behind
        // a shorter string; pinning them to the real last-seen values fixed
        // the width but not the content, since `cpu`/`frame` are genuine
        // wall-clock timing measurements that differ between runs by
        // design; and a hardcoded, safely-wide sentinel still ghosted,
        // meaning the corruption is not simply about string width at all.
        // Never calling `sceGuDebugPrint` in a `regression_capture` build
        // sidesteps whatever PPSSPP-internal state causes it, rather than
        // trying to out-guess it, and a developer diagnostic overlay was
        // never part of the golden scene R0.17 wants captured anyway.
        if !cfg!(feature = "regression_capture") {
            gpu.debug_text(
                8,
                8,
                WHITE,
                format_args!(
                    "SSB64-PSP  M4 scene viewer\n\
                     pack: {}\n\
                     \n\
                     {} {}/{}  file {}  @0x{:X}  costume {}/{}\n\
                     fighter {}  x {} y {}  line {}  mat {}  air {}\n\
                     attrs {}  grav {}/10  tvel {}  body {}w {}h\n\
                     anim dash {}f  land {}f\n\
                     play {} {}/{}  fighter {} slot {}  joints {}  f {}\n\
                     tris {}  {} {}  {} {}\n\
                     draws {}  state changes {}\n\
                     \n\
                     cpu {}us / budget {}us\n\
                     frame {}us  tick {}\n\
                     \n\
                     tex {}  bb {} {} {} .. {} {} {}\n\
                     TEXVIEW {}  tex {}/{}\n\
                     cam {} r {}\n\
                     \n\
                     dpad: browse  start: stage  B: collision\n\
                     stick: move  C-left: jump  C-right: respawn\n\
                     C-up: fighter sim on/off\n\
                     R/C-up: file  C-dn: obj/mesh  A/Z: zoom\n\
                     in obj view -- B: animate  dpad: anim/fighter  L: costume",
                    pack_status,
                    mode,
                    index,
                    count,
                    src_file,
                    src_offset,
                    costume_index,
                    costume_count_shown,
                    ft_state,
                    ft_x,
                    ft_y,
                    ft_line,
                    ft_mat,
                    ft_air,
                    ft_src,
                    ft_grav,
                    ft_tvel,
                    ft_bw,
                    ft_bh,
                    ft_dash,
                    ft_land,
                    if anim_playing { "on " } else { "off" },
                    anim_index,
                    anim_count,
                    anim_fighter,
                    anim_slot,
                    skeleton.joint_count(),
                    skeleton.frame() as i32,
                    shown.0,
                    if stage_view {
                        "layers"
                    } else if object_view {
                        "nodes"
                    } else {
                        "verts"
                    },
                    shown.1,
                    if stage_view {
                        "coll-segs"
                    } else if object_view {
                        "placed"
                    } else {
                        "prims"
                    },
                    shown.2,
                    draw_state.draws,
                    draw_state.state_changes,
                    cpu_us,
                    FRAME_BUDGET_US,
                    last_frame_us,
                    sim.tick,
                    dbg_tex,
                    dbg_bb[0],
                    dbg_bb[1],
                    dbg_bb[2],
                    dbg_bb[3],
                    dbg_bb[4],
                    dbg_bb[5],
                    tex_view,
                    tex_index,
                    tex_count,
                    dbg_cam as i32,
                    dbg_radius as i32,
                ),
            );
        }

        gpu.end_frame();
        last_frame_us = frame.elapsed_us();
    }
}
