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
    let mut show_collision = true;

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

    let mut sim = FixedClock::new(clock.now_us());

    let (_vx, _, vw, vh) = coord::pillarboxed_viewport();
    let aspect = vw as f32 / vh as f32;

    let mut spin = 0.0f32;
    let mut last_frame_us = 0u32;
    let mut dbg_cam = 1000.0f32;

    loop {
        let frame = Stopwatch::start();

        // ---- simulate: fixed 60 Hz, decoupled from display ----------------
        let ticks = sim.advance(clock.now_us());

        for _ in 0..ticks {
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
                if sim_fighter {
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
            } else if object_view && object_count > 0 {
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
                // R-trigger and C-up: the only inputs the default mapping
                // leaves free here (L-trigger is Z, which zooms).
                if pressed.contains(N64Buttons::R) {
                    object_index = step_file(object_index, 1);
                }
                if pressed.contains(N64Buttons::C_UP) {
                    object_index = step_file(object_index, object_count - 1);
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
            spin += 0.02
                + if stick_drives_fighter {
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
        gpu.set_perspective(60.0, aspect, 1.0, (dbg_cam * 4.0).max(10_000.0));
        gpu.reset_modelview();
        draw_state.begin_frame();

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
                    const FIT: f32 = 1.733;
                    let dist = radius * FIT * cam_distance / 200.0;
                    dbg_radius = radius;
                    dbg_cam = centre[2] + dist;

                    // Once zoomed in past the whole-stage framing, follow the
                    // fighter. The stage's bounding centre is the right thing
                    // to look at while inspecting a stage, but it sits up in
                    // the scenery -- on Dream Land it is inside the tree -- so
                    // zooming in on it walks the fighter off the bottom of the
                    // screen. At the default zoom the stage still wins, because
                    // that framing is what makes the collision overlay legible.
                    let centre = match (sim_fighter && cam_distance < CAM_FIT, &player) {
                        (true, Some(pl)) => [
                            pl.fighter.pos.x,
                            // Look at the middle of the body rather than the
                            // feet, so a fighter standing on a floor is not
                            // pinned to the centre of the screen with the
                            // stage below it out of view.
                            pl.fighter.pos.y + pl.fighter.coll.center,
                            centre[2],
                        ],
                        _ => centre,
                    };

                    // A stage is a place, not an object: spinning it would
                    // make the collision overlay impossible to read against
                    // the geometry. Face-on, always.
                    gpu.model_transform(
                        [-centre[0], -centre[1], -centre[2] - dist],
                        [0.0, 0.0, 0.0],
                        meshdraw::MODEL_SCALE,
                    );
                    let base = gpu.model_matrix();
                    let (tris, layers) = meshdraw::draw_stage(p, &stage, &base, &mut draw_state);
                    let segments = if show_collision {
                        meshdraw::draw_collision(p, &stage, &base, &mut gpu)
                    } else {
                        0
                    };
                    // Drawn last so the marker is never hidden behind a layer
                    // it is standing in front of.
                    if sim_fighter {
                        if let Some(pl) = &player {
                            meshdraw::draw_fighter(
                                pl.fighter.pos.to_array(),
                                &pl.fighter.coll,
                                pl.fighter.is_grounded(),
                                &base,
                                &mut gpu,
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
                    // Fit the radius to the 60-degree vertical FOV:
                    // d = r / tan 30 ~= 1.73 * r. The mesh path's 2x-extent
                    // rule leaves an object filling a ninth of the frame,
                    // which for a stage whose parts are spread over 33000
                    // units means specks.
                    const FIT: f32 = 1.733;
                    let dist = radius * FIT * cam_distance / 200.0;
                    dbg_radius = radius;
                    dbg_cam = centre[2] + dist;

                    gpu.model_transform(
                        [-centre[0], -centre[1], -centre[2] - dist],
                        [0.0, spin, 0.0],
                        meshdraw::MODEL_SCALE,
                    );
                    let base = gpu.model_matrix();
                    let tris = meshdraw::draw_object(p, &obj, &base, &mut draw_state);
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
                    meshdraw::draw_mesh(p, &desc, &mut draw_state);
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

        const WHITE: u32 = 0xFFFF_FFFF;
        gpu.debug_text(
            8,
            8,
            WHITE,
            format_args!(
                "SSB64-PSP  M4 scene viewer\n\
                 pack: {}\n\
                 \n\
                 {} {}/{}  file {}  @0x{:X}\n\
                 fighter {}  x {} y {}  line {}  mat {}  air {}\n\
                 attrs {}  grav {}/10  tvel {}  body {}w {}h\n\
                 anim dash {}f  land {}f\n\
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
                 R/C-up: file  C-dn: obj/mesh  A/Z: zoom",
                pack_status,
                mode,
                index,
                count,
                src_file,
                src_offset,
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

        gpu.end_frame();
        last_frame_us = frame.elapsed_us();
    }
}
