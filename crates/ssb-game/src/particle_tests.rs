use super::*;
use ssb_engine::math::{sy_arc_tan as arc_tan, sy_arc_tan2 as arc_tan2};

/// A one-bank test double: scripts with their bytecode.
pub(crate) struct TestBank {
    pub scripts: std::vec::Vec<(Script<'static>, bool)>,
}

impl TestBank {
    /// A particle script: `lifetime` frames of `bytecode`.
    pub fn particle(lifetime: u16, size: f32, bytecode: &'static [u8]) -> Script<'static> {
        Script {
            kind: 0,
            texture_id: 0,
            generator_lifetime: 0,
            particle_lifetime: lifetime,
            flags: 0,
            gravity: 0.0,
            friction: 1.0,
            vel: Vec3::ZERO,
            unk_20: 0.0,
            unk_24: 0.0,
            update_rate: 0.0,
            size,
            bytecode,
        }
    }

    pub fn new(scripts: &[Script<'static>]) -> TestBank {
        TestBank {
            scripts: scripts.iter().map(|&s| (s, false)).collect(),
        }
    }
}

impl Banks for TestBank {
    fn script_count(&self, bank: u8) -> u16 {
        if bank == 0 {
            self.scripts.len() as u16
        } else {
            0
        }
    }

    fn script(&self, bank: u8, id: u16) -> Option<Script<'_>> {
        (bank == 0)
            .then(|| self.scripts.get(usize::from(id)).map(|s| s.0))
            .flatten()
    }

    fn texture_flags(&self, _: u8, _: u16) -> u32 {
        0
    }
}

/// Counts the `proc_dead` calls by transform.
#[derive(Default)]
struct CountDead {
    calls: std::vec::Vec<u8>,
}

impl ProcDead for CountDead {
    fn proc_dead(&mut self, _: &mut Particles, _: &dyn Banks, xf: u8) {
        self.calls.push(xf);
    }
}

fn boxed() -> std::boxed::Box<Particles> {
    std::boxed::Box::new(Particles::new())
}

/// `END` at once: the particle lives its script's lifetime only if the
/// bytecode waits.
const WAIT_THEN_END: &[u8] = &[0x05, 0xFF];

#[test]
fn a_root_goes_to_its_lists_head_and_a_child_after_its_parent() {
    let bank = TestBank::new(&[TestBank::particle(10, 1.0, WAIT_THEN_END)]);
    let mut p = boxed();
    let a = make_script_id(&mut p, &bank, 0, 0);
    let b = make_script_id(&mut p, &bank, 0, 0);
    let c = make_child_script_id(&mut p, &bank, a, 0, 0);
    let order: std::vec::Vec<u8> = p.list(0).map(|(i, _)| i).collect();
    assert_eq!(order, [b, a, c]);
    // `LBPARTICLE_MASK_GENLINK(1)` puts a particle on list 2.
    let d = make_script_id(&mut p, &bank, genlink(1), 0);
    assert_eq!(p.list(2).map(|(i, _)| i).collect::<std::vec::Vec<_>>(), [d]);
    assert_eq!(p.used_num, 4);
}

#[test]
fn the_pool_holds_112_particles_and_a_full_pool_makes_none() {
    let bank = TestBank::new(&[TestBank::particle(10, 1.0, WAIT_THEN_END)]);
    let mut p = boxed();
    for _ in 0..STRUCTS_NUM {
        assert_ne!(make_script_id(&mut p, &bank, 0, 0), NIL);
    }
    assert_eq!(make_script_id(&mut p, &bank, 0, 0), NIL);
    assert_eq!(usize::from(p.used_max), STRUCTS_NUM);
    // A script past the bank's count makes nothing either.
    let mut q = boxed();
    assert_eq!(make_script_id(&mut q, &bank, 0, 1), NIL);
}

#[test]
fn a_particle_ends_after_its_lifetime_and_frees_its_slot() {
    // A wait longer than the life: the lifetime ends it.
    let bank = TestBank::new(&[TestBank::particle(3, 1.0, &[0x1F, 0xFF])]);
    let mut p = boxed();
    let mut dead = NoProcDead;
    let pc = make_script_id(&mut p, &bank, 0, 0);
    // `lifetime + 1`, less one per update: made, then three frames.
    for frame in 0..3 {
        assert_eq!(p.used_num, 1, "frame {frame}");
        run(&mut p, &bank, &mut dead);
    }
    // Still there after three updates; the fourth takes it to zero.
    assert_eq!(p.used_num, 1);
    run(&mut p, &bank, &mut dead);
    assert_eq!(p.used_num, 0);
    assert_eq!(p.list(0).count(), 0);
    // Its slot is the next one made.
    assert_eq!(make_script_id(&mut p, &bank, 0, 0), pc);
}

#[test]
fn end_kills_on_the_update_that_reads_it() {
    // `DEAD`: lifetime 1, so the same update frees it.
    let bank = TestBank::new(&[TestBank::particle(100, 1.0, &[0xFE])]);
    let mut p = boxed();
    let mut dead = NoProcDead;
    let pc = make_script_id(&mut p, &bank, 0, 0);
    process_struct(&mut p, &bank, &mut dead, pc);
    assert_eq!(p.used_num, 0);
}

#[test]
fn size_and_colour_blend_step_as_the_source_does() {
    // SETSIZELERP over 4 (stored 3) to 10; SETPRIMBLEND r over 2 (stored
    // 1) to 0; wait 10.
    const CODE: &[u8] = &[
        0xA0, 0x03, 0x41, 0x20, 0x00, 0x00, 0xC1, 0x01, 0x00, 0x0A, 0xFF,
    ];
    let bank = TestBank::new(&[TestBank::particle(100, 2.0, CODE)]);
    let mut p = boxed();
    let mut dead = NoProcDead;
    let pc = make_script_id(&mut p, &bank, 0, 0);
    process_struct(&mut p, &bank, &mut dead, pc);
    // Size: 2 + (10 - 2) / 4 = 4.
    assert_eq!(p.particle(pc).size, 4.0);
    // Red: ((255 << 16) + (0 - 255) * (65536 / 2)) >> 16 = 127.
    assert_eq!(p.particle(pc).primcolor, [127, 0xFF, 0xFF, 0xFF]);
    run(&mut p, &bank, &mut dead);
    assert_eq!(p.particle(pc).size, 4.0 + (10.0 - 4.0) / 3.0);
    assert_eq!(p.particle(pc).primcolor[0], 0);
}

#[test]
fn gravity_and_friction_move_it_each_frame() {
    // SETVEL y 10; SETGRAVITY 1; SETFRICTION 0.5; wait 10.
    const CODE: &[u8] = &[
        0x92, 0x41, 0x20, 0x00, 0x00, 0xA2, 0x3F, 0x80, 0x00, 0x00, 0xA3, 0x3F, 0x00, 0x00, 0x00,
        0x0A, 0xFF,
    ];
    let bank = TestBank::new(&[TestBank::particle(100, 1.0, CODE)]);
    let mut p = boxed();
    let mut dead = NoProcDead;
    let pc = make_script_id(&mut p, &bank, 0, 0);
    process_struct(&mut p, &bank, &mut dead, pc);
    // (10 - 1) * 0.5.
    assert_eq!(p.particle(pc).vel.y, 4.5);
    assert_eq!(p.particle(pc).pos.y, 4.5);
    run(&mut p, &bank, &mut dead);
    assert_eq!(p.particle(pc).vel.y, 1.75);
    assert_eq!(p.particle(pc).pos.y, 6.25);
}

#[test]
fn a_transform_runs_its_proc_dead_once_when_its_last_user_goes() {
    // The parent makes a child (MAKESCRIPT 1) that shares its transform;
    // the child outlives it.
    const PARENT: &[u8] = &[0xA4, 0x00, 0x01, 0x02, 0xFF];
    const CHILD: &[u8] = &[0x1F, 0xFF];
    let bank = TestBank::new(&[
        TestBank::particle(100, 1.0, PARENT),
        TestBank::particle(5, 1.0, CHILD),
    ]);
    let mut p = boxed();
    let mut dead = CountDead::default();
    let pc = make_script_id(&mut p, &bank, 0, 0);
    let xf = p.add_transform_for_struct(pc, TransformStatus::Default);
    p.transform_mut(xf).has_proc_dead = true;
    process_struct(&mut p, &bank, &mut dead, pc);
    assert_eq!(p.used_num, 2);
    assert_eq!(p.transform(xf).users_num, 2);
    for _ in 0..3 {
        run(&mut p, &bank, &mut dead);
    }
    // The parent is gone; the child keeps the transform.
    assert_eq!(p.used_num, 1);
    assert!(dead.calls.is_empty());
    for _ in 0..10 {
        run(&mut p, &bank, &mut dead);
    }
    assert_eq!(p.used_num, 0);
    assert_eq!(dead.calls, [xf]);
    assert_eq!(p.xf_used_num, 0);
}

#[test]
fn a_generator_emits_at_its_rate_and_ends_with_its_lifetime() {
    // Kind 1 (a line): every frame (rate -1), for 4 frames.
    let mut gen = TestBank::particle(20, 1.0, &[0x1F, 0xFF]);
    gen.kind = 1;
    gen.update_rate = -1.0;
    gen.generator_lifetime = 4;
    gen.vel = Vec3::new(100.0, 0.0, 0.0);
    let bank = TestBank::new(&[gen]);
    let mut p = boxed();
    let mut dead = NoProcDead;
    let gn = make_generator(&mut p, &bank, 0, 0);
    assert_ne!(gn, NIL);
    for _ in 0..4 {
        generator_func_run(&mut p, &bank, &mut dead);
    }
    assert_eq!(p.used_num, 4);
    assert_eq!(p.gen_used_num, 0);
    // Emitted along the line from the origin to `vel`, then moved once
    // by `vel` in their first update.
    for (_, pc) in p.list(0) {
        assert!((100.0..=200.0).contains(&pc.pos.x), "{}", pc.pos.x);
    }
}

#[test]
fn arc_tan2_follows_the_rational_approximation() {
    for &(y, x) in &[
        (1.0f32, 1.0f32),
        (-2.0, 0.5),
        (0.3, -4.0),
        (-1.0, -1.0),
        (3.0, 0.0),
    ] {
        let got = arc_tan2(y, x);
        assert!((got - y.atan2(x)).abs() < 1e-4, "{y} {x}: {got}");
    }
}

#[test]
fn a_particle_projects_to_its_size_over_depth() {
    // A camera 1000 units back looking down -z, 90 degrees vertical.
    let view = Mat4::look_at(Vec3::new(0.0, 0.0, 1000.0), Vec3::ZERO, Vec3::Y);
    let proj = Mat4::perspective(core::f32::consts::FRAC_PI_2, 1.0, 1.0, 10_000.0);
    let mut pc = Particle::EMPTY;
    pc.size = 50.0;
    let got = project(&pc, None, &view, &proj, BATTLE_PLANES).unwrap();
    assert!(got.center[0].abs() < 1e-5 && got.center[1].abs() < 1e-5);
    assert!((got.half[0] - 0.05).abs() < 1e-5, "{:?}", got.half);
    assert!((got.depth - 1000.0).abs() < 1e-3);
    // A transform scaled by 2 and flipped in x doubles it and flips s.
    let mut xf = Transform::EMPTY;
    xf.affine = tra_rot_rpy_r_sca(Vec3::ZERO, Vec3::ZERO, Vec3::new(-2.0, 2.0, 2.0));
    let got = project(&pc, Some(&xf), &view, &proj, BATTLE_PLANES).unwrap();
    assert!((got.half[0] - 0.1).abs() < 1e-5);
    assert!(got.flip_s && !got.flip_t);
    // Nearer than the original camera's depth window: culled.
    let near = Mat4::look_at(Vec3::new(0.0, 0.0, 300.0), Vec3::ZERO, Vec3::Y);
    assert_eq!(project(&pc, None, &near, &proj, BATTLE_PLANES), None);
}

#[test]
fn the_effect_camera_reads_quarter_pixels_of_its_viewport() {
    let proj = screen_projection([10.0, 10.0, 310.0, 230.0]);
    let mut pc = Particle::EMPTY;
    pc.size = 40.0;
    // Pixel (55, 210): a stock snap's start.
    pc.pos = Vec3::new(55.0 * 4.0, 210.0 * 4.0, 0.0);
    let got = project_screen(&pc, None, &proj).unwrap();
    // NDC over the 300 x 220 viewport: x (55 - 160) / 150, y up.
    assert!(
        (got.center[0] - (55.0 - 160.0) / 150.0).abs() < 1e-5,
        "{:?}",
        got.center
    );
    assert!(
        (got.center[1] - (120.0 - 210.0) / 110.0).abs() < 1e-5,
        "{:?}",
        got.center
    );
    // A size of 40 quarter pixels is 10 pixels: 10 / 150 of the half-width.
    assert!((got.half[0] - 10.0 / 150.0).abs() < 1e-5, "{:?}", got.half);
    // A score's transform squashes y by 4.
    let mut xf = Transform::EMPTY;
    xf.affine = tra_rot_rpy_r_sca(Vec3::ZERO, Vec3::ZERO, Vec3::new(1.0, 0.25, 1.0));
    let got = project_screen(&pc, Some(&xf), &proj).unwrap();
    assert!(
        (got.half[1] - 0.25 * 10.0 / 110.0).abs() < 1e-5,
        "{:?}",
        got.half
    );
    // Off the viewport: culled.
    pc.pos = Vec3::new(5.0 * 4.0, 100.0 * 4.0, 0.0);
    assert_eq!(project_screen(&pc, None, &proj), None);
}

#[test]
fn make_pos_vel_places_the_particle_and_updates_it_once() {
    let mut script = TestBank::particle(10, 5.0, WAIT_THEN_END);
    script.vel = Vec3::new(9.0, 9.0, 9.0);
    let bank = TestBank::new(&[script]);
    let mut p = boxed();
    let mut dead = CountDead::default();
    let pc = make_pos_vel(
        &mut p,
        &bank,
        &mut dead,
        genlink(2),
        0,
        Vec3::new(4.0, 8.0, 0.0),
        Vec3::ZERO,
    );
    assert_ne!(pc, NIL);
    let got = p.particle(pc);
    // Its own velocity (zero), not the script's.
    assert_eq!(got.vel, Vec3::ZERO);
    assert_eq!(got.pos, Vec3::new(4.0, 8.0, 0.0));
    assert_eq!(p.list(3).count(), 1, "GENLINK(2) is list 3");
}

/// RE-469: the KO explosion's generators have no velocity, so the
/// arctangents see `(0, 0)`. Under the traps a PSP enables (divide by zero,
/// invalid, overflow) they must still return 0 and raise nothing. glibc's
/// `feenableexcept` sets the same traps on the host; a trapped operation
/// kills the test with SIGFPE.
#[cfg(all(
    target_os = "linux",
    target_env = "gnu",
    any(target_arch = "x86_64", target_arch = "x86")
))]
#[test]
fn arctangents_of_a_zero_vector_raise_no_fpu_exception() {
    extern "C" {
        fn feenableexcept(excepts: i32) -> i32;
        fn fedisableexcept(excepts: i32) -> i32;
    }
    let traps = 0x01 | 0x04 | 0x08;
    let (y, x) = core::hint::black_box((0.0f32, 0.0f32));
    unsafe { feenableexcept(traps) };
    let angles = (
        arc_tan2(y, x),
        arc_tan(x),
        ssb_engine::math::div_nonzero(y, x),
    );
    unsafe { fedisableexcept(traps) };
    assert_eq!(angles, (0.0, 0.0, 0.0));
    assert_eq!(arc_tan2(1.0, 0.0), core::f32::consts::FRAC_PI_2);
    assert!((arc_tan2(1.0, 1.0) - core::f32::consts::FRAC_PI_4).abs() < 1e-5);
    assert!((arc_tan(2.0) - 2.0f32.atan()).abs() < 1e-5);
}

/// A vortex generator script: every frame (rate -1) for `life` frames,
/// particles of `particle_life`, as grhyrule's (gravity is the spin,
/// `unk_20` the radius, `unk_24` the pitch).
fn vortex_script(life: u16, particle_life: u16, unk_20: f32, unk_24: f32) -> Script<'static> {
    let mut gen = TestBank::particle(particle_life, 1.0, &[0x1F, 0xFF]);
    gen.kind = 2;
    gen.update_rate = -1.0;
    gen.generator_lifetime = life;
    gen.vel = Vec3::new(0.0, 25.0, 0.0);
    gen.gravity = 0.5;
    gen.friction = 0.999;
    gen.unk_20 = unk_20;
    gen.unk_24 = unk_24;
    gen
}

#[test]
fn a_vortex_generator_emits_vortex_particles_on_its_axis() {
    let bank = TestBank::new(&[vortex_script(3, 10, 100.0, 0.698)]);
    let mut p = boxed();
    let mut dead = NoProcDead;
    rng::set_seed(1);
    let gn = make_generator(&mut p, &bank, 0, 0);
    p.gens[usize::from(gn)].pos = Vec3::new(10.0, 20.0, 30.0);
    // The draws: the pre-loop angle, then per particle the radius
    // fraction before the angle.
    let pre = rng::rand_float();
    let pv1 = rng::rand_float();
    let pv0 = rng::rand_float() * TAU;
    let after = rng::seed();
    rng::set_seed(1);
    generator_func_run(&mut p, &bank, &mut dead);
    assert_eq!(rng::seed(), after, "three draws: {pre}");
    assert_eq!(p.used_num, 1);
    let g = p.gens[usize::from(gn)];
    assert_eq!(g.vortex_lifetime, 1);
    assert_eq!(g.vortex_f, 25.0);
    let (_, pc) = p.list(0).next().unwrap();
    assert_eq!(pc.flags & flag::VORTEX, flag::VORTEX);
    // The axis angles: the velocity (0, 25, 0) points along +y.
    assert_eq!(pc.gravity, sy_arc_tan2(25.0, 0.0));
    assert_eq!(pc.friction, sy_arc_tan2(0.0, 25.0 * sin_cos(pc.gravity).0));
    // Updated once: the angle turned by the spin, risen once.
    assert_eq!(pc.vel, Vec3::new(pv0 + 0.5, pv1, 25.0));
    let mut want = Particle::EMPTY;
    want.vel = Vec3::new(pv0, pv1, 0.0);
    want.gravity = pc.gravity;
    want.friction = pc.friction;
    vortex_step(&mut want, &g);
    assert_eq!(pc.pos, want.pos);
}

#[test]
fn a_vortex_generator_lingers_until_its_particles_die() {
    let bank = TestBank::new(&[vortex_script(3, 10, -50.0, -0.1)]);
    let mut p = boxed();
    let mut dead = NoProcDead;
    let gn = make_generator(&mut p, &bank, 0, 0);
    for _ in 0..3 {
        run(&mut p, &bank, &mut dead);
    }
    // Its life is over, three particles live: it stays, emitting nothing.
    assert_eq!(p.used_num, 3);
    assert_eq!(p.gen_used_num, 1);
    let g = p.gens[usize::from(gn)];
    assert_eq!((g.vortex_lifetime, g.generator_lifetime), (3, 1));
    assert_eq!(g.update_rate, 0.0);
    // Negative radius and pitch: no draws, full radius, even steps.
    for (_, pc) in p.list(0) {
        assert_eq!(pc.vel.y, 1.0);
    }
    let mut frames = 0;
    while p.gen_used_num != 0 {
        run(&mut p, &bank, &mut dead);
        frames += 1;
        assert_eq!(p.gens[usize::from(gn)].vortex_lifetime, p.used_num);
        assert!(frames < 20);
    }
    assert_eq!(p.used_num, 0);
}

#[test]
fn eject_struct_id_ends_an_effects_particles_and_generators() {
    // Two vortex generators on list 1, one ejected by id: its particles go
    // at once, and with none left it goes too.
    let bank = TestBank::new(&[vortex_script(0, 30, 100.0, 0.698)]);
    let mut p = boxed();
    let mut dead = NoProcDead;
    let a = make_generator(&mut p, &bank, genlink(0), 0);
    let b = make_generator(&mut p, &bank, genlink(0), 0);
    for _ in 0..4 {
        run(&mut p, &bank, &mut dead);
    }
    assert_eq!(p.used_num, 8);
    let id = p.gens[usize::from(a)].generator_id;
    eject_struct_id(&mut p, &bank, &mut dead, id, 1);
    assert_eq!(p.used_num, 4);
    assert_eq!(p.gen_used_num, 1);
    assert_eq!(p.gens[usize::from(b)].vortex_lifetime, 4);
    // Ejecting a vortex generator whose particles are on another list
    // only stops it: it lingers until they die.
    let id = p.gens[usize::from(b)].generator_id;
    eject_struct_id(&mut p, &bank, &mut dead, id, 0);
    assert_eq!(p.gen_used_num, 1);
    assert_eq!(p.gens[usize::from(b)].update_rate, 0.0);
    assert_eq!(p.gens[usize::from(b)].generator_lifetime, 1);
}

/// `tools/r4300/vortex_hashes.py`'s inputs: (axis angles, vel, (spin,
/// radius, pitch, rise), generator position).
#[allow(clippy::type_complexity)]
fn vortex_inputs() -> std::vec::Vec<([f32; 2], [f32; 3], [f32; 4], [f32; 3])> {
    let axes = [
        [0.0, 0.0],
        [0.3, 0.698],
        [-1.2, -0.5],
        [1.570_796_4, 2.2],
        [3.0, -6.0],
        [-std::f32::consts::PI, 0.1745],
        [7.0, 1.0],
    ];
    let gens = [
        ([0.5, 100.0, 0.698, 25.0], [10.0, -20.0, 30.0]),
        ([0.0, 50.0, 0.0, 40.0], [0.0, 0.0, 0.0]),
        ([-0.32, -60.0, -0.1745, 12.5], [1000.5, 200.25, -3.0]),
        ([0.05, 0.0, 0.3, 0.0], [-512.0, 64.0, 7.5]),
        ([3.0, 75.0, 1.2, 7.0], [1.0, 2.0, 3.0]),
    ];
    let vels = [
        [0.0, 1.0, 0.0],
        [2.5, 0.37, 0.0],
        [6.0, 0.9, 13.0],
        [-1.0, 0.5, -4.0],
    ];
    let mut out = std::vec::Vec::new();
    for a in axes {
        for (g, pos) in gens {
            for v in vels {
                out.push((a, v, g, pos));
            }
        }
    }
    out
}

#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn the_vortex_motion_matches_the_rom() {
    // The hash of the ROM's own `lbParticleUpdateStruct` (0x800CEF4C) on
    // a vortex particle, 140 cases of 12 updates, run by
    // `tools/r4300/vortex_hashes.py`: the f32 bits of pos and vel after
    // every update.
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    let inputs = vortex_inputs();
    assert_eq!(inputs.len(), 140);
    for (a, v, gv, gpos) in inputs {
        let mut pc = Particle::EMPTY;
        pc.flags = flag::VORTEX;
        pc.vel = Vec3::new(v[0], v[1], v[2]);
        pc.gravity = a[0];
        pc.friction = a[1];
        let mut g = Generator::EMPTY;
        g.kind = 2;
        g.gravity = gv[0];
        g.unk_38 = gv[1];
        g.unk_3c = gv[2];
        g.vortex_f = gv[3];
        g.pos = Vec3::new(gpos[0], gpos[1], gpos[2]);
        for _ in 0..12 {
            vortex_step(&mut pc, &g);
            for f in [pc.pos.x, pc.pos.y, pc.pos.z, pc.vel.x, pc.vel.y, pc.vel.z] {
                for b in f.to_bits().to_le_bytes() {
                    h = (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3);
                }
            }
        }
    }
    assert_eq!(h, 0x046f_7167_8688_c5e8);
}
