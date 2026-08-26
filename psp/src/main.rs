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

mod gu;
mod input;
mod timing;

use core::f32::consts::PI;

use psp::Align16;

use ssb_engine::coord;
use ssb_engine::input::{Input, N64Buttons};
use ssb_engine::renderer::Color;
use ssb_engine::timing::{Clock, FixedClock, FRAME_BUDGET_US};

use ssb_game::fighter::{Fighter, FighterKind};
use ssb_game::physics;

use gu::{Gpu, GuVertex};
use input::PspInput;
use timing::{PspClock, Stopwatch};

psp::module!("ssb64_psp", 1, 0);

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

    let mut sim = FixedClock::new(clock.now_us());

    // A fighter driven by the real physics module, so gravity, drift and the
    // depth clamp are all exercised on hardware.
    let mut fighter = Fighter::new(FighterKind::Mario, 0, 3);
    fighter.pos = ssb_engine::math::Vec3::new(0.0, 0.0, 0.0);

    let (vx, _, vw, vh) = coord::pillarboxed_viewport();
    let aspect = vw as f32 / vh as f32;

    let mut spin = 0.0f32;
    let mut last_frame_us = 0u32;

    loop {
        let frame = Stopwatch::start();

        // ---- simulate: fixed 60 Hz, decoupled from display ----------------
        let ticks = sim.advance(clock.now_us());

        for _ in 0..ticks {
            pad.poll();
            let state = pad.state(0);

            physics::apply_air_drift(&mut fighter.physics, &fighter.attributes, state.stick_x);
            physics::apply_gravity_default(&mut fighter.physics, &fighter.attributes);

            // Cross re-launches the test object so gravity is visibly re-run.
            if state.buttons.contains(N64Buttons::A) {
                fighter.physics.vel_air.y = 3.0;
            }
            // A crude floor, so it does not fall forever.
            fighter.integrate();
            if fighter.pos.y < -3.0 {
                fighter.land(-3.0);
                fighter.become_airborne();
            }

            spin += 0.02;
            if spin > 2.0 * PI {
                spin -= 2.0 * PI;
            }
        }

        let cpu = Stopwatch::start();

        // ---- render: display cadence -------------------------------------
        gpu.begin_frame(Some(Color::rgba(0x20, 0x28, 0x38, 0xFF)));
        gpu.set_perspective(60.0, aspect, 0.5, 1000.0);
        gpu.reset_modelview();
        gpu.model_transform(
            [fighter.pos.x, fighter.pos.y, -8.0],
            [spin * 0.4, spin, 0.0],
        );
        gpu.draw_triangles(&TRIANGLE.0);

        let cpu_us = cpu.elapsed_us();

        // ---- on-screen diagnostics ---------------------------------------
        // One call, newline-separated: see Gpu::debug_text for why multiple
        // calls per frame would render garbage.
        //
        // Frame time is carried over from the *previous* iteration on purpose —
        // measuring it after end_frame would include the vblank wait and always
        // report ~16.7 ms, which tells us nothing about headroom.
        //
        // Positions and velocities are printed as integers scaled by 100:
        // no_std float formatting is slow, and this runs every frame.
        const WHITE: u32 = 0xFFFF_FFFF;
        let s = &pad.state(0);
        gpu.debug_text(
            8,
            8,
            WHITE,
            format_args!(
                "SSB64-PSP  M1 baseline\n\
                 \n\
                 frame {}  tick {}\n\
                 ticks/frame {}  dropped {}\n\
                 cpu {}us / budget {}us\n\
                 frame {}us  view {}x{}\n\
                 \n\
                 pos  x{} y{} z{}   (x100)\n\
                 vel  x{} y{}       (x100)\n\
                 stick {}  buttons {:04X}\n\
                 \n\
                 nub: drift    X: jump",
                gpu.frame_count(),
                sim.tick,
                ticks,
                sim.dropped,
                cpu_us,
                FRAME_BUDGET_US,
                last_frame_us,
                vw,
                vh,
                (fighter.pos.x * 100.0) as i32,
                (fighter.pos.y * 100.0) as i32,
                (fighter.pos.z * 100.0) as i32,
                (fighter.physics.vel_air.x * 100.0) as i32,
                (fighter.physics.vel_air.y * 100.0) as i32,
                s.stick_x,
                s.buttons.0,
            ),
        );

        gpu.end_frame();
        last_frame_us = frame.elapsed_us();
    }
}
