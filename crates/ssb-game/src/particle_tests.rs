use super::*;

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
