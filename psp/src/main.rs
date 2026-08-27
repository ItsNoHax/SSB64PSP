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
mod timing;

use core::f32::consts::PI;

use psp::Align16;

use ssb_engine::coord;
use ssb_engine::input::{Input, N64Buttons};
use ssb_engine::renderer::Color;
use ssb_engine::timing::{Clock, FixedClock, FRAME_BUDGET_US};


use ssb_rom::pack::Pack;

use gu::{Gpu, GuVertex};
use input::PspInput;
use timing::{PspClock, Stopwatch};

psp::module!("ssb64_psp", 1, 0);

/// Scratch for the texture-inspection quad. Align16 because the GE DMAs it.
static mut TEX_QUAD: Align16<[meshdraw::TexQuadVertex; 6]> =
    Align16([meshdraw::TexQuadVertex { u: 0.0, v: 0.0, color: 0, x: 0.0, y: 0.0, z: 0.0 }; 6]);

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
    let pack = pack_buf.and_then(|b| Pack::open(b.as_slice()).ok());
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
    let mut cam_distance = 200.0f32;
    let mut draw_state = meshdraw::DrawState::default();
    // Texture inspection mode (C_UP toggles). Proven working, so the mesh
    // view is the default; kept because it cleanly separates an upload bug
    // from a mesh-data bug.
    let mut tex_view = false;
    let mut tex_index: u32 = 0;
    let tex_count = pack.as_ref().map_or(0, |p| p.texture_count());

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
            if mesh_count > 0 {
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
            // The stick spins the model so geometry can be inspected.
            spin += 0.02 + state.stick_x as f32 * 0.0005;
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
                            mn[0] as i32, mn[1] as i32, mn[2] as i32,
                            mx[0] as i32, mx[1] as i32, mx[2] as i32,
                        ];
                    }
                    dbg_radius = radius;
                    dbg_cam = centre[2] + radius * cam_distance / 100.0;

                    gpu.model_transform(
                        [-centre[0], -centre[1], -centre[2] - radius * cam_distance / 100.0],
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

        const WHITE: u32 = 0xFFFF_FFFF;
        gpu.debug_text(
            8,
            8,
            WHITE,
            format_args!(
                "SSB64-PSP  M3 mesh viewer\n\
                 pack: {}\n\
                 \n\
                 mesh {}/{}  file {}  @0x{:X}\n\
                 tris {}  verts {}  prims {}\n\
                 draws {}  state changes {}\n\
                 \n\
                 cpu {}us / budget {}us\n\
                 frame {}us  tick {}\n\
                 \n\
                 tex {}  bb {} {} {} .. {} {} {}\n\
                 TEXVIEW {}  tex {}/{}\n\
                 cam {} r {}\n\
                 \n\
                 dpad: browse   A/Z: zoom   nub: spin",
                pack_path,
                mesh_index,
                mesh_count,
                pack.as_ref()
                    .and_then(|p| p.mesh(mesh_index))
                    .map_or(0, |m| m.source_file),
                pack.as_ref()
                    .and_then(|p| p.mesh(mesh_index))
                    .map_or(0, |m| m.source_offset),
                shown.0,
                shown.1,
                shown.2,
                draw_state.draws,
                draw_state.state_changes,
                cpu_us,
                FRAME_BUDGET_US,
                last_frame_us,
                sim.tick,
                dbg_tex,
                dbg_bb[0], dbg_bb[1], dbg_bb[2], dbg_bb[3], dbg_bb[4], dbg_bb[5],
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
