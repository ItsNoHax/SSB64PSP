//! The `LBParticle` runtime: `lb/lbparticle.c`'s particle, generator and
//! transform pools, run the way `efParticleInitAll` sets them up for a
//! match (`ef/efparticle.c`).
//!
//! - **Pools.** 112 particles, 24 generators and 80 transforms, each a
//!   fixed array with the source's free lists (`lbParticleAllocStructs`,
//!   `lbParticleAllocGenerators`, `lbParticleAllocTransforms`). A make that
//!   finds its pool empty returns [`NIL`], as the source returns `NULL`.
//! - **Links.** A particle's `bank_id` is a bank in its low three bits and
//!   one of 16 linked lists above them (`LBPARTICLE_MASK_GENLINK`). A root
//!   particle goes to the head of its list; a child is spliced in after its
//!   parent (`lbParticleMakeStruct`).
//! - **Frame.** [`struct_func_run`] is `lbParticleStructFuncRun` and
//!   [`generator_func_run`] `lbParticleGeneratorFuncRun`, the two
//!   `func_run`s of link 0, which run before every process of the frame.
//! - **Bytecode.** [`update_struct`] is `lbParticleUpdateStruct`: the wait
//!   and opcode loop, a child's immediate first update, the size and colour
//!   blends, the lifetime, and gravity, friction and velocity.
//! - **Transforms.** An `LBTransform` places a particle's position when it
//!   is drawn. Its users count the particles and generators that share it;
//!   the last one to go ejects it and runs its `proc_dead`
//!   ([`ProcDead`]), synchronously, as the effect manager's makers rely on.
//!
//! Script data comes through [`Banks`]; randomness through the game's one
//! [`crate::rng`] sequence, as `syUtilsRandFloat` does.
//!
//! Not modelled, each unused by any bank script a match reaches (RE-413):
//! the vortex generator (`kind` 2) and `LBPARTICLE_FLAG_VORTEX`'s motion,
//! the attach and distance opcodes' `DObj` (`sLBParticleAttachDObjs`,
//! empty in a match), and a generator's `dobj`.

use ssb_engine::math::{sin_cos, sqrt, Mat4, Vec3};

use crate::rng;

/// `lbParticleAllocStructs(112)`.
pub const STRUCTS_NUM: usize = 112;
/// `lbParticleAllocGenerators(24)`.
pub const GENERATORS_NUM: usize = 24;
/// `lbParticleAllocTransforms(80, ...)`.
pub const TRANSFORMS_NUM: usize = 80;
/// `sLBParticleStructsAllocLinks`.
pub const LINKS_NUM: usize = 16;
/// `LBPARTICLE_BANKS_NUM_MAX`.
pub const BANKS_NUM_MAX: u8 = 8;
/// The null index of every pool.
pub const NIL: u8 = u8::MAX;

/// `LBPARTICLE_MASK_GENLINK(id)`: OR'd into a bank id, puts its particles
/// on list `id + 1`.
pub const fn genlink(id: u8) -> u8 {
    (id + 1) * 8
}

/// `LBPARTICLE_FLAG_*` (`lb/lbdef.h`).
pub mod flag {
    pub const GRAVITY: u16 = 0x1;
    pub const FRICTION: u16 = 0x2;
    pub const VORTEX: u16 = 0x4;
    pub const SHAREDPAL: u16 = 0x10;
    pub const MASKS: u16 = 0x20;
    pub const MASKT: u16 = 0x40;
    pub const ENVCOLOR: u16 = 0x80;
    pub const NOISE: u16 = 0x100;
    pub const ALPHABLEND: u16 = 0x200;
    pub const DITHER: u16 = 0x400;
    pub const PAUSE: u16 = 0x800;
    pub const ATTACH: u16 = 0x8000;
}

/// `LBScript`: a script's fixed header and its bytecode.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Script<'a> {
    pub kind: u16,
    pub texture_id: u16,
    pub generator_lifetime: u16,
    pub particle_lifetime: u16,
    pub flags: u32,
    pub gravity: f32,
    pub friction: f32,
    pub vel: Vec3,
    /// `unk_script_0x20`: a generator's spread radius.
    pub unk_20: f32,
    /// `unk_script_0x24`: a generator's cone angle.
    pub unk_24: f32,
    pub update_rate: f32,
    pub size: f32,
    pub bytecode: &'a [u8],
}

/// The loaded banks (`sLBParticleScriptBanks`, `sLBParticleTextureBanks`),
/// by bank id.
pub trait Banks {
    /// `sLBParticleScriptBanksNum[bank]`; 0 for a bank not loaded.
    fn script_count(&self, bank: u8) -> u16;
    fn script(&self, bank: u8, id: u16) -> Option<Script<'_>>;
    /// `LBTexture::flags` (bit 0: one shared palette).
    fn texture_flags(&self, bank: u8, texture: u16) -> u32;
}

/// `LBTransformStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum TransformStatus {
    /// The matrix is rebuilt at every draw.
    #[default]
    Default = 0,
    /// Built at the first draw, then kept.
    Ready = 1,
    Finished = 2,
}

/// `LBTransform`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    next: u8,
    pub translate: Vec3,
    pub rotate: Vec3,
    pub scale: Vec3,
    pub status: TransformStatus,
    pub users_num: u16,
    /// `syMatrixTraRotRpyRScaF`, row-vector (`affine[3]` is the
    /// translation), as the last draw built it.
    pub affine: [[f32; 4]; 4],
    pub generator_id: u16,
    /// Whether `proc_dead` is set; [`Transform::owner`] says whose.
    pub has_proc_dead: bool,
    /// `effect_gobj`: the owner's handle, for [`ProcDead`].
    pub owner: u8,
}

impl Transform {
    const EMPTY: Transform = Transform {
        next: NIL,
        translate: Vec3::ZERO,
        rotate: Vec3::ZERO,
        scale: Vec3::splat(1.0),
        status: TransformStatus::Default,
        users_num: 0,
        affine: IDENTITY,
        generator_id: 0,
        has_proc_dead: false,
        owner: NIL,
    };
}

const IDENTITY: [[f32; 4]; 4] = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];

/// `LBParticle`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Particle {
    next: u8,
    pub generator_id: u16,
    pub flags: u16,
    pub bank_id: u8,
    loop_count: u8,
    pub texture_id: u8,
    pub frame_id: u8,
    bytecode_timer: u16,
    size_target_length: u16,
    primcolor_target_length: u16,
    envcolor_target_length: u16,
    /// Which script of bank `bank_id & 7` holds `bytecode`.
    script: u16,
    bytecode_csr: u16,
    return_ptr: u16,
    loop_ptr: u16,
    /// Ends at 1, as in the source.
    pub lifetime: u16,
    pub pos: Vec3,
    pub vel: Vec3,
    pub gravity: f32,
    pub friction: f32,
    pub size: f32,
    size_target: f32,
    pub primcolor: [u8; 4],
    target_primcolor: [u8; 4],
    pub envcolor: [u8; 4],
    target_envcolor: [u8; 4],
    pub gn: u8,
    pub xf: u8,
}

impl Particle {
    const EMPTY: Particle = Particle {
        next: NIL,
        generator_id: 0,
        flags: 0,
        bank_id: 0,
        loop_count: 0,
        texture_id: 0,
        frame_id: 0,
        bytecode_timer: 0,
        size_target_length: 0,
        primcolor_target_length: 0,
        envcolor_target_length: 0,
        script: 0,
        bytecode_csr: 0,
        return_ptr: 0,
        loop_ptr: 0,
        lifetime: 0,
        pos: Vec3::ZERO,
        vel: Vec3::ZERO,
        gravity: 0.0,
        friction: 0.0,
        size: 0.0,
        size_target: 0.0,
        primcolor: [0; 4],
        target_primcolor: [0; 4],
        envcolor: [0; 4],
        target_envcolor: [0; 4],
        gn: NIL,
        xf: NIL,
    };

    /// The list it is on (`bank_id >> 3`).
    pub fn link(&self) -> usize {
        usize::from(self.bank_id >> 3)
    }
}

/// `LBGenerator`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Generator {
    next: u8,
    pub generator_id: u16,
    pub flags: u16,
    pub kind: u8,
    pub bank_id: u8,
    pub texture_id: u16,
    particle_lifetime: u16,
    pub generator_lifetime: u16,
    script: u16,
    pub pos: Vec3,
    pub vel: Vec3,
    gravity: f32,
    friction: f32,
    size: f32,
    unk_38: f32,
    unk_3c: f32,
    update_rate: f32,
    frame: f32,
    pub xf: u8,
    rotate_base: f32,
    rotate_target: f32,
    move_to: Vec3,
    vortex_lifetime: u16,
}

impl Generator {
    const EMPTY: Generator = Generator {
        next: NIL,
        generator_id: 0,
        flags: 0,
        kind: 0,
        bank_id: 0,
        texture_id: 0,
        particle_lifetime: 0,
        generator_lifetime: 0,
        script: 0,
        pos: Vec3::ZERO,
        vel: Vec3::ZERO,
        gravity: 0.0,
        friction: 0.0,
        size: 0.0,
        unk_38: 0.0,
        unk_3c: 0.0,
        update_rate: 0.0,
        frame: 0.0,
        xf: NIL,
        rotate_base: 0.0,
        rotate_target: 0.0,
        move_to: Vec3::ZERO,
        vortex_lifetime: 0,
    };
}

/// A transform's `proc_dead`, run when its last user goes.
pub trait ProcDead {
    fn proc_dead(&mut self, particles: &mut Particles, banks: &dyn Banks, xf: u8);
}

/// No transform with a `proc_dead`.
pub struct NoProcDead;

impl ProcDead for NoProcDead {
    fn proc_dead(&mut self, _: &mut Particles, _: &dyn Banks, _: u8) {}
}

/// The three pools and the lists over them.
pub struct Particles {
    structs: [Particle; STRUCTS_NUM],
    free: u8,
    links: [u8; LINKS_NUM],
    pub used_num: u16,
    /// `D_ovl0_800D644E`: the most particles live at once.
    pub used_max: u16,
    gens: [Generator; GENERATORS_NUM],
    gen_free: u8,
    gen_queued: u8,
    /// `sLBParticleGeneratorsLastProcessed`.
    gen_last: u8,
    pub gen_used_num: u16,
    xfs: [Transform; TRANSFORMS_NUM],
    xf_free: u8,
    pub xf_used_num: u16,
    current_generator_id: u16,
    /// The particle and generator `GObj`s' skip bits (`flags >> 16`):
    /// list `i` does not run while bit `i` is set
    /// (`efParticleGObjSetSkipID`).
    pub skip: u16,
}

impl Particles {
    /// A fresh runtime by value: some 30 KB, for the host.
    pub fn new() -> Particles {
        let mut p = Particles {
            structs: [Particle::EMPTY; STRUCTS_NUM],
            free: NIL,
            links: [NIL; LINKS_NUM],
            used_num: 0,
            used_max: 0,
            gens: [Generator::EMPTY; GENERATORS_NUM],
            gen_free: NIL,
            gen_queued: NIL,
            gen_last: NIL,
            gen_used_num: 0,
            xfs: [Transform::EMPTY; TRANSFORMS_NUM],
            xf_free: NIL,
            xf_used_num: 0,
            current_generator_id: 0,
            skip: 0,
        };
        p.reset();
        p
    }

    /// Builds a fresh runtime in place, element by element, so a caller
    /// with a small stack can put it straight on the heap.
    ///
    /// # Safety
    ///
    /// `dst` must be valid for writes and aligned; it is fully initialised
    /// on return.
    pub unsafe fn write_new(dst: *mut Particles) {
        use core::ptr::addr_of_mut;
        for i in 0..STRUCTS_NUM {
            addr_of_mut!((*dst).structs[i]).write(Particle::EMPTY);
        }
        for i in 0..GENERATORS_NUM {
            addr_of_mut!((*dst).gens[i]).write(Generator::EMPTY);
        }
        for i in 0..TRANSFORMS_NUM {
            addr_of_mut!((*dst).xfs[i]).write(Transform::EMPTY);
        }
        addr_of_mut!((*dst).free).write(NIL);
        addr_of_mut!((*dst).links).write([NIL; LINKS_NUM]);
        addr_of_mut!((*dst).used_num).write(0);
        addr_of_mut!((*dst).used_max).write(0);
        addr_of_mut!((*dst).gen_free).write(NIL);
        addr_of_mut!((*dst).gen_queued).write(NIL);
        addr_of_mut!((*dst).gen_last).write(NIL);
        addr_of_mut!((*dst).gen_used_num).write(0);
        addr_of_mut!((*dst).xf_free).write(NIL);
        addr_of_mut!((*dst).xf_used_num).write(0);
        addr_of_mut!((*dst).current_generator_id).write(0);
        addr_of_mut!((*dst).skip).write(0);
        (*dst).reset();
    }

    /// `efParticleInitAll`: every pool free, every list empty.
    pub fn reset(&mut self) {
        // `lbParticleAllocStructs`: pushed from the last, so the free list
        // runs 0, 1, 2, ...
        self.free = NIL;
        for i in (0..STRUCTS_NUM).rev() {
            self.structs[i].next = self.free;
            self.free = i as u8;
        }
        self.links = [NIL; LINKS_NUM];
        self.used_num = 0;
        self.used_max = 0;
        self.gen_free = NIL;
        self.gen_queued = NIL;
        self.gen_last = NIL;
        for i in (0..GENERATORS_NUM).rev() {
            self.gens[i].next = self.gen_free;
            self.gen_free = i as u8;
        }
        self.gen_used_num = 0;
        // `lbParticleAllocTransforms` pushes from the first.
        self.xf_free = NIL;
        for i in 0..TRANSFORMS_NUM {
            self.xfs[i].next = self.xf_free;
            self.xf_free = i as u8;
        }
        self.xf_used_num = 0;
        // `dLBParticleCurrentGeneratorID`.
        self.current_generator_id = 0;
        self.skip = 0;
    }

    pub fn particle(&self, i: u8) -> &Particle {
        &self.structs[usize::from(i)]
    }

    pub fn particle_mut(&mut self, i: u8) -> &mut Particle {
        &mut self.structs[usize::from(i)]
    }

    pub fn transform(&self, i: u8) -> &Transform {
        &self.xfs[usize::from(i)]
    }

    pub fn transform_mut(&mut self, i: u8) -> &mut Transform {
        &mut self.xfs[usize::from(i)]
    }

    pub fn generator(&self, i: u8) -> &Generator {
        &self.gens[usize::from(i)]
    }

    pub fn generator_mut(&mut self, i: u8) -> &mut Generator {
        &mut self.gens[usize::from(i)]
    }

    /// List `link`'s particles, head first.
    pub fn list(&self, link: usize) -> ListIter<'_> {
        ListIter {
            p: self,
            cur: self.links[link],
        }
    }

    /// The live generators, queue order.
    pub fn generators(&self) -> impl Iterator<Item = &Generator> + '_ {
        let mut cur = self.gen_queued;
        core::iter::from_fn(move || {
            if cur == NIL {
                return None;
            }
            let g = &self.gens[usize::from(cur)];
            cur = g.next;
            Some(g)
        })
    }

    /// `lbParticleGetTransform`.
    pub fn get_transform(&mut self, status: TransformStatus, generator_id: u16) -> u8 {
        let xf = self.xf_free;
        if xf == NIL {
            return NIL;
        }
        let t = &mut self.xfs[usize::from(xf)];
        self.xf_free = t.next;
        t.users_num = 1;
        t.has_proc_dead = false;
        t.owner = NIL;
        t.translate = Vec3::ZERO;
        t.rotate = Vec3::ZERO;
        t.scale = Vec3::splat(1.0);
        t.status = status;
        t.generator_id = generator_id;
        self.xf_used_num += 1;
        xf
    }

    /// `lbParticleAddTransformForStruct`.
    pub fn add_transform_for_struct(&mut self, pc: u8, status: TransformStatus) -> u8 {
        let generator_id = self.structs[usize::from(pc)].generator_id;
        let xf = self.get_transform(status, generator_id);
        self.structs[usize::from(pc)].xf = xf;
        xf
    }

    /// Readies each drawn transform the way `lbParticleDrawTextures` does
    /// before it projects: a transform not yet finished takes its matrix
    /// (`syMatrixTraRotRpyRScaF`), and a ready one is then finished. Only
    /// particles of the lists in `links` with a size count, as the draw
    /// skips a zero size. Run once a frame, before the draw.
    pub fn prepare_draw(&mut self, links: u16) {
        for link in 0..LINKS_NUM {
            if links & (1 << link) == 0 {
                continue;
            }
            let mut cur = self.links[link];
            while cur != NIL {
                let pc = self.structs[usize::from(cur)];
                if pc.size != 0.0 && pc.xf != NIL {
                    let xf = &mut self.xfs[usize::from(pc.xf)];
                    if xf.status != TransformStatus::Finished {
                        xf.affine = tra_rot_rpy_r_sca(xf.translate, xf.rotate, xf.scale);
                    }
                    if xf.status == TransformStatus::Ready {
                        xf.status = TransformStatus::Finished;
                    }
                }
                cur = pc.next;
            }
        }
    }
}

impl Default for Particles {
    fn default() -> Self {
        Particles::new()
    }
}

/// A list's particles with their indices.
pub struct ListIter<'a> {
    p: &'a Particles,
    cur: u8,
}

impl<'a> Iterator for ListIter<'a> {
    type Item = (u8, &'a Particle);

    fn next(&mut self) -> Option<Self::Item> {
        if self.cur == NIL {
            return None;
        }
        let i = self.cur;
        let pc = &self.p.structs[usize::from(i)];
        self.cur = pc.next;
        Some((i, pc))
    }
}

/// `syMatrixTraRotRpyRScaF`: `syMatrixRotRpyRF`, the translation in row 3,
/// then `syMatrixRowscaleF`.
pub fn tra_rot_rpy_r_sca(t: Vec3, r: Vec3, s: Vec3) -> [[f32; 4]; 4] {
    let (sinr, cosr) = sin_cos(r.x);
    let (sinp, cosp) = sin_cos(r.y);
    let (siny, cosy) = sin_cos(r.z);
    let mut m = [
        [cosp * cosy, cosp * siny, -sinp, 0.0],
        [
            sinr * sinp * cosy - cosr * siny,
            sinr * sinp * siny + cosr * cosy,
            sinr * cosp,
            0.0,
        ],
        [
            cosr * sinp * cosy + sinr * siny,
            cosr * sinp * siny - sinr * cosy,
            cosr * cosp,
            0.0,
        ],
        [t.x, t.y, t.z, 1.0],
    ];
    for (row, k) in m.iter_mut().zip([s.x, s.y, s.z]) {
        for v in row.iter_mut() {
            if *v != 0.0 {
                *v *= k;
            }
        }
    }
    m
}

/// `syUtilsArcTan` (`sys/utils.c`): a rational approximation.
pub fn arc_tan(div: f32) -> f32 {
    use core::f32::consts::FRAC_PI_2;
    if div == 0.0 {
        return 0.0;
    }
    let (d, kind) = if div > 1.0 {
        (1.0 / div, 1)
    } else if div < -1.0 {
        (1.0 / div, 2)
    } else {
        (div, 0)
    };
    let c = d * d;
    let a = c / -0.108_106_75 + -44.571_92;
    let b = c / a + -0.161_908_1;
    let e = c / b + -15.774_018;
    let f = c / e + -0.555_569_77;
    let g = c / f + -3.000_003;
    let result = (c / g + 1.0) * d;
    match kind {
        1 => FRAC_PI_2 - result,
        2 => -FRAC_PI_2 - result,
        _ => result,
    }
}

/// `syUtilsArcTan2`.
pub fn arc_tan2(y: f32, x: f32) -> f32 {
    use core::f32::consts::{FRAC_PI_2, PI};
    if x > 0.0 {
        arc_tan(y / x)
    } else if x < 0.0 {
        let sign = if y < 0.0 { -1.0 } else { 1.0 };
        (PI - arc_tan((y / x).abs())) * sign
    } else if y != 0.0 {
        (if y < 0.0 { -1.0 } else { 1.0 }) * FRAC_PI_2
    } else {
        0.0
    }
}

const TAU: f32 = core::f32::consts::TAU;

/// `lbParticleMakeStruct`: takes a particle from the free list, puts it at
/// the head of its list (`this_pc` [`NIL`]) or after `this_pc`, and fills
/// it. The fields it leaves (`loop_*`, `size_target`, the target colours)
/// keep what the slot last held.
#[allow(clippy::too_many_arguments)]
pub fn make_struct(
    p: &mut Particles,
    this_pc: u8,
    bank_id: u8,
    flags: u32,
    texture_id: u16,
    script: u16,
    lifetime: u16,
    pos: Vec3,
    vel: Vec3,
    size: f32,
    gravity: f32,
    friction: f32,
    texture_flags: u32,
    gn: u8,
) -> u8 {
    let new_pc = p.free;
    if new_pc == NIL {
        return NIL;
    }
    p.used_num += 1;
    if p.used_max < p.used_num {
        p.used_max = p.used_num;
    }
    let generator_id = if gn != NIL {
        p.gens[usize::from(gn)].generator_id
    } else {
        p.current_generator_id = p.current_generator_id.wrapping_add(1);
        p.current_generator_id
    };
    let xf = if gn != NIL {
        let xf = p.gens[usize::from(gn)].xf;
        if xf != NIL {
            p.xfs[usize::from(xf)].users_num += 1;
        }
        xf
    } else {
        NIL
    };
    p.free = p.structs[usize::from(new_pc)].next;
    if this_pc == NIL {
        let link = usize::from(bank_id >> 3);
        p.structs[usize::from(new_pc)].next = p.links[link];
        p.links[link] = new_pc;
    } else {
        p.structs[usize::from(new_pc)].next = p.structs[usize::from(this_pc)].next;
        p.structs[usize::from(this_pc)].next = new_pc;
    }
    let s = &mut p.structs[usize::from(new_pc)];
    s.generator_id = generator_id;
    s.xf = xf;
    s.bank_id = bank_id;
    s.flags = flags as u16;
    s.texture_id = texture_id as u8;
    s.pos = pos;
    s.vel = vel;
    s.size = size;
    s.gravity = gravity;
    s.friction = friction;
    s.lifetime = lifetime.wrapping_add(1);
    s.bytecode_csr = 0;
    s.return_ptr = 0;
    s.script = script;
    if texture_flags != 0 {
        s.flags |= flag::SHAREDPAL;
    }
    // A bank script's bytecode is never `NULL`.
    s.bytecode_timer = 1;
    s.frame_id = 0;
    s.primcolor = [0xFF; 4];
    s.envcolor = [0; 4];
    s.size_target_length = 0;
    s.primcolor_target_length = 0;
    s.envcolor_target_length = 0;
    s.gn = gn;
    new_pc
}

/// `lbParticleMakeChildScriptID`: a particle from script `script_id` of
/// bank `bank_id & 7`, at the origin, after `pc` (or at its list's head).
pub fn make_child_script_id(
    p: &mut Particles,
    banks: &dyn Banks,
    pc: u8,
    bank_id: u8,
    script_id: u16,
) -> u8 {
    let id = bank_id & 7;
    if id >= BANKS_NUM_MAX || script_id >= banks.script_count(id) {
        return NIL;
    }
    let Some(script) = banks.script(id, script_id) else {
        return NIL;
    };
    make_struct(
        p,
        pc,
        bank_id,
        script.flags,
        script.texture_id,
        script_id,
        script.particle_lifetime,
        Vec3::ZERO,
        script.vel,
        script.size,
        script.gravity,
        script.friction,
        banks.texture_flags(id, script.texture_id),
        NIL,
    )
}

/// `lbParticleMakeScriptID`: a root particle, not yet updated.
pub fn make_script_id(p: &mut Particles, banks: &dyn Banks, bank_id: u8, script_id: u16) -> u8 {
    make_child_script_id(p, banks, NIL, bank_id, script_id)
}

/// `lbParticleMakeCommon`: a root particle, updated once.
pub fn make_common(
    p: &mut Particles,
    banks: &dyn Banks,
    dead: &mut dyn ProcDead,
    bank_id: u8,
    script_id: u16,
) -> u8 {
    let pc = make_child_script_id(p, banks, NIL, bank_id, script_id);
    if pc != NIL {
        update_struct(p, banks, dead, pc, NIL, usize::from(bank_id >> 3));
    }
    pc
}

/// `LBParticleProcessStruct`: one update of a particle, as the head of its
/// list.
pub fn process_struct(p: &mut Particles, banks: &dyn Banks, dead: &mut dyn ProcDead, pc: u8) {
    if pc != NIL {
        let link = p.structs[usize::from(pc)].link();
        update_struct(p, banks, dead, pc, NIL, link);
    }
}

/// `lbParticleEjectTransform`.
pub fn eject_transform(p: &mut Particles, banks: &dyn Banks, dead: &mut dyn ProcDead, xf: u8) {
    if p.xfs[usize::from(xf)].has_proc_dead {
        dead.proc_dead(p, banks, xf);
    }
    p.xfs[usize::from(xf)].next = p.xf_free;
    p.xf_free = xf;
    p.xf_used_num -= 1;
}

/// Drops one user of `xf`, ejecting it with the last.
fn release_transform(
    p: &mut Particles,
    banks: &dyn Banks,
    dead: &mut dyn ProcDead,
    xf: u8,
) -> bool {
    if xf == NIL {
        return false;
    }
    let t = &mut p.xfs[usize::from(xf)];
    t.users_num -= 1;
    if t.users_num == 0 {
        eject_transform(p, banks, dead, xf);
        return true;
    }
    false
}

/// `lbParticleEjectStruct`: unlinks one particle wherever it is on its list.
pub fn eject_struct(p: &mut Particles, banks: &dyn Banks, dead: &mut dyn ProcDead, this_pc: u8) {
    let link = p.structs[usize::from(this_pc)].link();
    let mut prev = NIL;
    let mut cur = p.links[link];
    while cur != NIL {
        if cur == this_pc {
            let next = p.structs[usize::from(cur)].next;
            if prev == NIL {
                p.links[link] = next;
            } else {
                p.structs[usize::from(prev)].next = next;
            }
            vortex_release(p, this_pc);
            let xf = p.structs[usize::from(this_pc)].xf;
            release_transform(p, banks, dead, xf);
            p.structs[usize::from(cur)].next = p.free;
            p.free = cur;
            p.used_num -= 1;
            break;
        }
        prev = cur;
        cur = p.structs[usize::from(cur)].next;
    }
}

/// `gn->generator_vars.vortex.lifetime--` for a vortex particle.
fn vortex_release(p: &mut Particles, pc: u8) {
    let s = p.structs[usize::from(pc)];
    if s.gn != NIL && s.flags & flag::VORTEX != 0 && p.gens[usize::from(s.gn)].kind == 2 {
        let g = &mut p.gens[usize::from(s.gn)];
        g.vortex_lifetime = g.vortex_lifetime.wrapping_sub(1);
    }
}

/// Bytecode reads. A read past the end yields `END` or zero; no bank
/// script reaches one.
struct Csr<'a> {
    data: &'a [u8],
    at: usize,
}

impl Csr<'_> {
    fn u8(&mut self) -> u8 {
        let v = self.data.get(self.at).copied().unwrap_or(0xFF);
        self.at += 1;
        v
    }

    fn u16(&mut self) -> u16 {
        let hi = u16::from(self.u8());
        (hi << 8) + u16::from(self.u8())
    }

    /// `lbParticleReadFloatBigEnd`.
    fn f32(&mut self) -> f32 {
        let b = [self.u8(), self.u8(), self.u8(), self.u8()];
        f32::from_bits(u32::from_be_bytes(b))
    }

    /// `lbParticleReadUShort`: one or two bytes, stored minus one.
    fn var_u16(&mut self) -> u16 {
        let first = u16::from(self.u8());
        let v = if first & 0x80 != 0 {
            ((first & 0x7F) << 8) + u16::from(self.u8())
        } else {
            first
        };
        v.wrapping_add(1)
    }
}

/// One colour channel's step: `((c << 16) + (t - c) * (65536 / n)) >> 16`,
/// narrowed to `u8`.
fn blend_channel(c: u8, t: u8, n: u16) -> u8 {
    let c = i32::from(c);
    (((c << 16) + (i32::from(t) - c) * (65536 / i32::from(n))) >> 16) as u8
}

/// A float added to a `u8` field and stored back (`+= x * rand`).
fn add_u8(v: u8, x: f32) -> u8 {
    (f32::from(v) + x) as i32 as u8
}

/// `lbParticleRotateVel`.
fn rotate_vel(pc: &mut Particle, angle: f32) {
    let v = pc.vel;
    let pitch = arc_tan2(v.y, v.z);
    let (sin_pitch, cos_pitch) = sin_cos(pitch);
    let yaw = arc_tan2(v.x, v.y * sin_pitch + v.z * cos_pitch);
    let (sin_yaw, cos_yaw) = sin_cos(yaw);
    let magnitude = sqrt(v.x * v.x + v.y * v.y + v.z * v.z);
    let spin = rng::rand_float() * TAU;
    let (sin_a, cos_a) = sin_cos(angle);
    let sin_angle = sin_a * magnitude;
    let vz = sin_yaw;
    let (spin_sin, spin_cos) = sin_cos(spin);
    let vx = spin_cos * sin_angle;
    let vy = spin_sin * sin_angle;
    let cos_angle = cos_a * magnitude;
    pc.vel.x = vx * cos_yaw + cos_angle * sin_yaw;
    pc.vel.y = (-vx * sin_pitch * sin_yaw + vy * cos_pitch) + cos_angle * sin_pitch * cos_yaw;
    pc.vel.z = (-vx * cos_pitch * vz - vy * sin_pitch) + cos_angle * cos_pitch * cos_yaw;
}

/// Spawns a `MAKESCRIPT`/`MAKERAND`/`MAKEID` child of `this`: it takes the
/// parent's position (and velocity for `MAKEID`), generator and transform,
/// then updates once in place.
fn make_child(
    p: &mut Particles,
    banks: &dyn Banks,
    dead: &mut dyn ProcDead,
    this: u8,
    script_id: u16,
    inherit_vel: bool,
) {
    let parent = p.structs[usize::from(this)];
    let child = make_child_script_id(p, banks, this, parent.bank_id, script_id);
    if child == NIL {
        return;
    }
    {
        let c = &mut p.structs[usize::from(child)];
        c.pos = parent.pos;
        if inherit_vel {
            c.vel = parent.vel;
        }
        c.generator_id = parent.generator_id;
        c.gn = parent.gn;
        c.xf = parent.xf;
    }
    if parent.xf != NIL {
        p.xfs[usize::from(parent.xf)].users_num += 1;
    }
    update_struct(p, banks, dead, child, this, parent.link());
}

/// `lbParticleUpdateStruct`: one frame of particle `this` on list `link`,
/// whose predecessor is `other` ([`NIL`] at the head). Returns the next
/// particle to update.
pub fn update_struct(
    p: &mut Particles,
    banks: &dyn Banks,
    dead: &mut dyn ProcDead,
    this: u8,
    other: u8,
    link: usize,
) -> u8 {
    let t = usize::from(this);
    if p.structs[t].flags & flag::PAUSE != 0 {
        return p.structs[t].next;
    }
    if p.structs[t].bytecode_timer != 0 {
        p.structs[t].bytecode_timer -= 1;
        if p.structs[t].bytecode_timer == 0 {
            let s = p.structs[t];
            let data = banks
                .script(s.bank_id & 7, s.script)
                .map_or(&[][..], |sc| sc.bytecode);
            let mut csr = Csr {
                data,
                at: usize::from(s.bytecode_csr),
            };
            let timer = run_bytecode(p, banks, dead, this, &mut csr);
            let s = &mut p.structs[t];
            s.bytecode_csr = csr.at as u16;
            s.bytecode_timer = timer;
        }
    }
    {
        let s = &mut p.structs[t];
        if s.size_target_length != 0 {
            s.size += (s.size_target - s.size) / f32::from(s.size_target_length);
            s.size_target_length -= 1;
        }
        if s.primcolor_target_length != 0 {
            for i in 0..4 {
                s.primcolor[i] = blend_channel(
                    s.primcolor[i],
                    s.target_primcolor[i],
                    s.primcolor_target_length,
                );
            }
            s.primcolor_target_length -= 1;
        }
        if s.envcolor_target_length != 0 {
            for i in 0..4 {
                s.envcolor[i] = blend_channel(
                    s.envcolor[i],
                    s.target_envcolor[i],
                    s.envcolor_target_length,
                );
            }
            s.envcolor_target_length -= 1;
        }
        s.lifetime = s.lifetime.wrapping_sub(1);
    }
    if p.structs[t].lifetime == 0 {
        let mut next = p.structs[t].next;
        if other == NIL {
            p.links[link] = next;
        } else {
            p.structs[usize::from(other)].next = next;
        }
        vortex_release(p, this);
        let xf = p.structs[t].xf;
        if release_transform(p, banks, dead, xf) && other == NIL && next != p.links[link] {
            // The `proc_dead` made particles at the head: go on from there.
            next = p.links[link];
        }
        p.structs[t].next = p.free;
        p.free = this;
        p.used_num -= 1;
        return next;
    }
    let s = &mut p.structs[t];
    if s.flags & flag::VORTEX == 0 {
        if s.flags & flag::GRAVITY != 0 {
            s.vel.y -= s.gravity;
        }
        if s.flags & flag::FRICTION != 0 {
            s.vel.x *= s.friction;
            s.vel.y *= s.friction;
            s.vel.z *= s.friction;
        }
        s.pos.x += s.vel.x;
        s.pos.y += s.vel.y;
        s.pos.z += s.vel.z;
    }
    s.next
}

/// The wait and opcode loop. Returns the wait left.
fn run_bytecode(
    p: &mut Particles,
    banks: &dyn Banks,
    dead: &mut dyn ProcDead,
    this: u8,
    csr: &mut Csr<'_>,
) -> u16 {
    let t = usize::from(this);
    loop {
        let command = csr.u8();
        if command < 0x80 {
            let mut timer = u16::from(command & 0x1F);
            if command & 0x20 != 0 {
                timer = u16::from(csr.u8()) + (timer << 8);
            }
            if command & 0xC0 == 0x40 {
                p.structs[t].frame_id = csr.u8();
            }
            if timer != 0 {
                return timer;
            }
            continue;
        }
        let mut opcode = command & 0xF8;
        if opcode > 0x98 {
            opcode = command & 0xF0;
            if opcode != 0xC0 && opcode != 0xD0 {
                opcode = command;
            }
        }
        match opcode {
            0x80 | 0x88 | 0x90 | 0x98 => {
                let s = &mut p.structs[t];
                let v = if opcode < 0x90 {
                    &mut s.pos
                } else {
                    &mut s.vel
                };
                let add = opcode & 8 != 0;
                if command & 1 != 0 {
                    let f = csr.f32();
                    v.x = if add { v.x + f } else { f };
                }
                if command & 2 != 0 {
                    let f = csr.f32();
                    v.y = if add { v.y + f } else { f };
                }
                if command & 4 != 0 {
                    let f = csr.f32();
                    v.z = if add { v.z + f } else { f };
                }
            }
            // SETSIZELERP
            0xA0 => {
                let s = &mut p.structs[t];
                s.size_target_length = csr.var_u16();
                s.size_target = csr.f32();
                if s.size_target_length == 1 {
                    s.size = s.size_target;
                    s.size_target_length = 0;
                }
            }
            // SETFLAG
            0xA1 => p.structs[t].flags = u16::from(csr.u8()),
            // SETGRAVITY
            0xA2 => {
                let s = &mut p.structs[t];
                s.gravity = csr.f32();
                if s.gravity == 0.0 {
                    s.flags &= !flag::GRAVITY;
                } else {
                    s.flags |= flag::GRAVITY;
                }
            }
            // SETFRICTION
            0xA3 => {
                let s = &mut p.structs[t];
                s.friction = csr.f32();
                if s.friction == 1.0 {
                    s.flags &= !flag::FRICTION;
                } else {
                    s.flags |= flag::FRICTION;
                }
            }
            // MAKESCRIPT
            0xA4 => {
                let id = csr.u16();
                make_child(p, banks, dead, this, id, false);
            }
            // MAKEGENERATOR
            0xA5 => {
                let id = csr.u16();
                let parent = p.structs[t];
                let gn = make_generator(p, banks, parent.bank_id, id);
                if gn != NIL {
                    let g = &mut p.gens[usize::from(gn)];
                    g.pos = parent.pos;
                    g.generator_id = parent.generator_id;
                    g.xf = parent.xf;
                    if parent.xf != NIL {
                        p.xfs[usize::from(parent.xf)].users_num += 1;
                    }
                }
            }
            // SETLIFERAND
            0xA6 => {
                let base = i32::from(csr.u16());
                let range = i32::from(csr.u16());
                p.structs[t].lifetime = (base + (range as f32 * rng::rand_float()) as i32) as u16;
            }
            // TRYDEADRAND: the roll is truncated to an integer.
            0xA7 => {
                let percent = i32::from(csr.u8());
                let roll = (rng::rand_float() * 100.0) as i32;
                if percent >= roll {
                    p.structs[t].lifetime = 1;
                    return 0;
                }
            }
            // ADDVELRAND: adds to the position.
            0xA8 => {
                let s = &mut p.structs[t];
                let x = csr.f32();
                s.pos.x += x * rng::rand_float();
                let y = csr.f32();
                s.pos.y += y * rng::rand_float();
                let z = csr.f32();
                s.pos.z += z * rng::rand_float();
            }
            // SETVELANGLE
            0xA9 => {
                let angle = csr.f32();
                rotate_vel(&mut p.structs[t], angle);
            }
            // MAKERAND
            0xAA => {
                let base = i32::from(csr.u16());
                let range = i32::from(csr.u16());
                let id = base + (range as f32 * rng::rand_float()) as i32;
                make_child(p, banks, dead, this, id as u16, false);
            }
            // MULVELUFORM
            0xAB => {
                let k = csr.f32();
                let s = &mut p.structs[t];
                s.vel.x *= k;
                s.vel.y *= k;
                s.vel.z *= k;
            }
            // SETSIZERAND
            0xAC => {
                let s = &mut p.structs[t];
                s.size_target_length = csr.var_u16();
                s.size_target = csr.f32();
                let range = csr.f32();
                s.size_target += range * rng::rand_float();
                if s.size_target_length == 1 {
                    s.size = s.size_target;
                    s.size_target_length = 0;
                }
            }
            0xAD => p.structs[t].flags |= flag::ENVCOLOR,
            0xAE => p.structs[t].flags &= !(flag::MASKT | flag::MASKS),
            0xAF => {
                let s = &mut p.structs[t];
                s.flags &= !flag::MASKT;
                s.flags |= flag::MASKS;
            }
            0xB0 => {
                let s = &mut p.structs[t];
                s.flags &= !flag::MASKS;
                s.flags |= flag::MASKT;
            }
            0xB1 => p.structs[t].flags |= flag::MASKT | flag::MASKS,
            0xB2 => p.structs[t].flags |= flag::ALPHABLEND,
            0xB3 => p.structs[t].flags &= !flag::DITHER,
            0xB4 => p.structs[t].flags |= flag::DITHER,
            // `NONOISE` sets the flag and `NOISE` clears it, as written.
            0xB5 => p.structs[t].flags |= flag::NOISE,
            0xB6 => p.structs[t].flags &= !flag::NOISE,
            // SETDISTVEL, ADDDISTVELMAG: no attach `DObj` in a match.
            0xB7 => {
                csr.u8();
            }
            0xB8 => {
                csr.u8();
                csr.f32();
            }
            // MAKEID
            0xB9 => {
                let id = csr.u16();
                make_child(p, banks, dead, this, id, true);
            }
            // PRIMBLENDRAND, ENVBLENDRAND
            0xBA | 0xBB => {
                let s = &mut p.structs[t];
                let (target, len) = if opcode == 0xBA {
                    (&mut s.target_primcolor, s.primcolor_target_length)
                } else {
                    (&mut s.target_envcolor, s.envcolor_target_length)
                };
                for c in target.iter_mut() {
                    let k = f32::from(csr.u8());
                    *c = add_u8(*c, k * rng::rand_float());
                }
                let target = *target;
                if len == 0 {
                    if opcode == 0xBA {
                        s.primcolor = target;
                    } else {
                        s.envcolor = target;
                    }
                }
            }
            0xBC => {
                let s = &mut p.structs[t];
                s.frame_id = csr.u8();
                let k = f32::from(csr.u8());
                s.frame_id = add_u8(s.frame_id, k * rng::rand_float());
            }
            // SETVELMAG
            0xBD => {
                let base = csr.f32();
                let range = csr.f32();
                let s = &mut p.structs[t];
                let target = base + range * rng::rand_float();
                let mag = sqrt(s.vel.x * s.vel.x + s.vel.y * s.vel.y + s.vel.z * s.vel.z);
                let k = target / mag;
                s.vel.x *= k;
                s.vel.y *= k;
                s.vel.z *= k;
            }
            // MULVELAXIS
            0xBE => {
                let s = &mut p.structs[t];
                s.vel.x *= csr.f32();
                s.vel.y *= csr.f32();
                s.vel.z *= csr.f32();
            }
            // SETATTACHID
            0xBF => {
                let id = u16::from(csr.u8().wrapping_sub(1));
                p.structs[t].flags |= flag::ATTACH | (id << 0xC);
            }
            // SETPRIMBLEND, SETENVBLEND
            0xC0 | 0xD0 => {
                let s = &mut p.structs[t];
                let len = csr.var_u16();
                let (color, target, length) = if opcode == 0xC0 {
                    (
                        &mut s.primcolor,
                        &mut s.target_primcolor,
                        &mut s.primcolor_target_length,
                    )
                } else {
                    (
                        &mut s.envcolor,
                        &mut s.target_envcolor,
                        &mut s.envcolor_target_length,
                    )
                };
                *length = len;
                *target = *color;
                for (i, c) in target.iter_mut().enumerate() {
                    if command & (1 << i) != 0 {
                        *c = csr.u8();
                    }
                }
                if *length == 1 {
                    *color = *target;
                    *length = 0;
                }
            }
            // SETLOOP
            0xFA => {
                let s = &mut p.structs[t];
                s.loop_count = csr.u8();
                s.loop_ptr = csr.at as u16;
            }
            // LOOP
            0xFB => {
                let s = &mut p.structs[t];
                s.loop_count = s.loop_count.wrapping_sub(1);
                if s.loop_count != 0 {
                    csr.at = usize::from(s.loop_ptr);
                }
            }
            // SETRETURN
            0xFC => p.structs[t].return_ptr = csr.at as u16,
            // RETURN
            0xFD => csr.at = usize::from(p.structs[t].return_ptr),
            // DEAD, END
            0xFE | 0xFF => {
                p.structs[t].lifetime = 1;
                return 0;
            }
            // Not an opcode: the source's switch does nothing.
            _ => {}
        }
    }
}

/// `lbParticleStructFuncRun`: every list not skipped, head to tail.
pub fn struct_func_run(p: &mut Particles, banks: &dyn Banks, dead: &mut dyn ProcDead) {
    for link in 0..LINKS_NUM {
        if p.skip & (1 << link) != 0 {
            continue;
        }
        let mut prev = NIL;
        let mut cur = p.links[link];
        while cur != NIL {
            let next = update_struct(p, banks, dead, cur, prev, link);
            // A particle that died now heads the free list, so its `next`
            // is no longer this one.
            if p.structs[usize::from(cur)].next == next {
                prev = cur;
            }
            cur = next;
        }
    }
}

/// `lbParticleGetGenerator`: to the head of the queue.
fn get_generator(p: &mut Particles) -> u8 {
    let gn = p.gen_free;
    if gn == NIL {
        return NIL;
    }
    p.gen_used_num += 1;
    p.gen_free = p.gens[usize::from(gn)].next;
    p.gens[usize::from(gn)].next = p.gen_queued;
    p.gen_queued = gn;
    if p.gen_last == NIL {
        p.gen_last = gn;
    }
    p.current_generator_id = p.current_generator_id.wrapping_add(1);
    let g = &mut p.gens[usize::from(gn)];
    g.generator_id = p.current_generator_id;
    g.xf = NIL;
    gn
}

/// `lbParticleMakeGenerator`: a generator from script `script_id`'s header,
/// at the origin.
pub fn make_generator(p: &mut Particles, banks: &dyn Banks, bank_id: u8, script_id: u16) -> u8 {
    let id = bank_id & 7;
    if id >= BANKS_NUM_MAX || script_id >= banks.script_count(id) {
        return NIL;
    }
    let Some(s) = banks.script(id, script_id) else {
        return NIL;
    };
    let gn = get_generator(p);
    if gn == NIL {
        return NIL;
    }
    let shared = banks.texture_flags(id, s.texture_id) != 0;
    let g = &mut p.gens[usize::from(gn)];
    g.kind = s.kind as u8;
    g.bank_id = bank_id;
    g.flags = s.flags as u16;
    g.texture_id = s.texture_id;
    g.particle_lifetime = s.particle_lifetime;
    g.generator_lifetime = s.generator_lifetime;
    g.pos = Vec3::ZERO;
    g.vel = s.vel;
    g.gravity = s.gravity;
    g.friction = s.friction;
    g.size = s.size;
    g.script = script_id;
    g.unk_38 = s.unk_20;
    g.unk_3c = s.unk_24;
    g.update_rate = s.update_rate;
    g.frame = 0.0;
    if shared {
        g.flags |= flag::SHAREDPAL;
    }
    match g.kind {
        0 | 3 | 4 => {
            g.rotate_base = 0.0;
            g.rotate_target = TAU;
        }
        // Taken before the caller places it, so from the origin.
        1 => g.move_to = g.pos + g.vel,
        2 => g.vortex_lifetime = 0,
        _ => {}
    }
    gn
}

/// `lbParticleMakeParam`: a generator's particle, updated once.
fn make_param(
    p: &mut Particles,
    banks: &dyn Banks,
    dead: &mut dyn ProcDead,
    gn: u8,
    pos: Vec3,
    vel: Vec3,
) {
    let g = p.gens[usize::from(gn)];
    let pc = make_struct(
        p,
        NIL,
        g.bank_id,
        u32::from(g.flags),
        g.texture_id,
        g.script,
        g.particle_lifetime,
        pos,
        vel,
        g.size,
        g.gravity,
        g.friction,
        0,
        gn,
    );
    if pc != NIL {
        update_struct(p, banks, dead, pc, NIL, usize::from(g.bank_id >> 3));
    }
}

/// `lbParticleGeneratorFuncRun`: each queued generator accumulates its rate
/// and emits a particle for every whole frame, then counts down its life.
pub fn generator_func_run(p: &mut Particles, banks: &dyn Banks, dead: &mut dyn ProcDead) {
    let mut gn = p.gen_queued;
    p.gen_last = NIL;
    while gn != NIL {
        let gi = usize::from(gn);
        let g = p.gens[gi];
        if p.skip & (1 << (g.bank_id >> 3)) != 0 || g.flags & flag::PAUSE != 0 {
            p.gen_last = gn;
            gn = g.next;
            continue;
        }
        {
            let g = &mut p.gens[gi];
            if g.update_rate < 0.0 {
                g.frame -= g.update_rate;
            } else {
                g.frame += rng::rand_float() * g.update_rate;
            }
        }
        let g = p.gens[gi];
        let vel = g.vel;
        let (mut pv0, mut spb8) = (0.0, 0.0);
        if g.frame >= 1.0 {
            match g.kind {
                0 | 3 | 4 => {
                    let span = g.rotate_target - g.rotate_base;
                    pv0 = g.rotate_base + rng::rand_float() * span;
                    spb8 = span / (g.frame as i32) as f32;
                }
                _ => {
                    pv0 = TAU * rng::rand_float();
                    spb8 = TAU / (g.frame as i32) as f32;
                }
            }
        }
        while p.gens[gi].frame >= 1.0 {
            let g = p.gens[gi];
            match g.kind {
                0 | 3 | 4 => {
                    let angle1 = arc_tan2(vel.y, vel.z);
                    let (sin1, cos1) = sin_cos(angle1);
                    let angle2 = arc_tan2(vel.x, vel.y * sin1 + vel.z * cos1);
                    let (sin2, cos2) = sin_cos(angle2);
                    let magnitude = sqrt(vel.x * vel.x + vel.y * vel.y + vel.z * vel.z);
                    let (mut pv1, vmag, var_f20);
                    if g.unk_38 < 0.0 {
                        pv1 = 1.0;
                        vmag = 1.0;
                        var_f20 = -g.unk_38;
                    } else {
                        let mut r = rng::rand_float();
                        if g.kind != 0 {
                            r = sqrt(r);
                        }
                        pv1 = r;
                        vmag = r;
                        var_f20 = g.unk_38 * r;
                    }
                    if g.unk_3c < 0.0 {
                        pv0 += spb8;
                        pv1 = -g.unk_3c;
                    } else {
                        pv0 = g.rotate_base + rng::rand_float() * (g.rotate_target - g.rotate_base);
                        pv1 *= g.unk_3c;
                    }
                    let (sin0, cos0) = sin_cos(pv0);
                    let (sinp1, cosp1) = sin_cos(pv1);
                    let spec = cos0 * var_f20;
                    let f26 = sin0 * var_f20;
                    let pm1 = sinp1 * magnitude;
                    let (vx, vy, vz) = (cos0 * pm1, sin0 * pm1, cosp1 * magnitude);
                    let pos = Vec3::new(
                        spec * cos2 + g.pos.x,
                        -spec * sin1 * sin2 + f26 * cos1 + g.pos.y,
                        -spec * cos1 * sin2 - f26 * sin1 + g.pos.z,
                    );
                    let mut v = Vec3::new(
                        vx * cos2 + vz * sin2,
                        -vx * sin1 * sin2 + vy * cos1 + vz * sin1 * cos2,
                        -vx * cos1 * sin2 - vy * sin1 + vz * cos1 * cos2,
                    );
                    if g.kind == 3 {
                        v *= vmag;
                    }
                    make_param(p, banks, dead, gn, pos, v);
                }
                1 => {
                    let r = rng::rand_float();
                    let pos = g.pos + (g.move_to - g.pos) * r;
                    make_param(p, banks, dead, gn, pos, vel);
                }
                // The vortex (RE-413): not modelled, emits nothing.
                _ => {}
            }
            p.gens[gi].frame -= 1.0;
        }
        let g = &mut p.gens[gi];
        if g.generator_lifetime != 0 {
            g.generator_lifetime -= 1;
            if g.generator_lifetime == 0 {
                if g.kind == 2 && g.vortex_lifetime != 0 {
                    g.update_rate = 0.0;
                    g.generator_lifetime = 1;
                } else {
                    let next = g.next;
                    let xf = g.xf;
                    if p.gen_last == NIL {
                        p.gen_queued = next;
                    } else {
                        p.gens[usize::from(p.gen_last)].next = next;
                    }
                    release_transform(p, banks, dead, xf);
                    p.gens[gi].next = p.gen_free;
                    p.gen_free = gn;
                    p.gen_used_num -= 1;
                    gn = next;
                    continue;
                }
            }
        }
        p.gen_last = gn;
        gn = p.gens[gi].next;
    }
}

/// One frame of the runtime: the particles, then the generators, as the
/// two link-0 `GObj`s run.
pub fn run(p: &mut Particles, banks: &dyn Banks, dead: &mut dyn ProcDead) {
    struct_func_run(p, banks, dead);
    generator_func_run(p, banks, dead);
}

/// Where `lbParticleDrawTextures` puts one particle: its rectangle's
/// centre and half extents in normalised device coordinates, and whether
/// the texture is flipped (`s`/`t` start at the far edge).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Projected {
    pub center: [f32; 2],
    pub half: [f32; 2],
    /// Eye-space depth (distance in front of the camera).
    pub depth: f32,
    pub flip_s: bool,
    pub flip_t: bool,
}

/// The original battle camera's `near` and `far` (`dGMCameraPerspDefault`),
/// for the draw's depth cull in a battle.
pub const CULL_NEAR: f32 = crate::camera::DEFAULT_NEAR;
pub const CULL_FAR: f32 = crate::camera::DEFAULT_FAR;
/// [`CULL_NEAR`] and [`CULL_FAR`].
pub const BATTLE_PLANES: (f32, f32) = (CULL_NEAR, CULL_FAR);

/// Projects one particle as `lbParticleDrawTextures` does: its position
/// through its transform's matrix, `view` and `proj`, the half extents
/// `size / w` times the lengths of the composed matrix's first two columns
/// (the gradients of clip x and y).
///
/// The source culls a point outside `[-1, 1]` in x or y, or outside
/// `[0, 1]` in its own depth range; the depth test uses the current
/// camera's `near` and `far` (`planes`): [`BATTLE_PLANES`] in a battle.
pub fn project(
    pc: &Particle,
    xf: Option<&Transform>,
    view: &Mat4,
    proj: &Mat4,
    planes: (f32, f32),
) -> Option<Projected> {
    if pc.size == 0.0 {
        return None;
    }
    // A row-vector matrix's rows are a column-vector matrix's columns.
    let a = Mat4 {
        cols: xf.map_or(IDENTITY, |x| x.affine),
    };
    let va = view.multiply(&a);
    let m = proj.multiply(&va);
    let p = pc.pos;
    let row = |m: &Mat4, r: usize| {
        m.cols[0][r] * p.x + m.cols[1][r] * p.y + m.cols[2][r] * p.z + m.cols[3][r]
    };
    let (cx, cy, cw) = (row(&m, 0), row(&m, 1), row(&m, 3));
    if cw == 0.0 {
        return None;
    }
    let depth = -row(&va, 2);
    let tm = 1.0 / cw;
    let (tx, ty) = (cx * tm, cy * tm);
    // N64 depth `((f + n) d - 2 f n) / ((f - n) d)` in `[0, 1]`.
    let (n, f) = planes;
    let tz = if depth != 0.0 {
        ((f + n) * depth - 2.0 * f * n) / ((f - n) * depth)
    } else {
        -1.0
    };
    if !(-1.0..=1.0).contains(&tx) || !(-1.0..=1.0).contains(&ty) || !(0.0..=1.0).contains(&tz) {
        return None;
    }
    let c = &m.cols;
    let mx = sqrt(c[0][0] * c[0][0] + c[1][0] * c[1][0] + c[2][0] * c[2][0]);
    let my = sqrt(c[0][1] * c[0][1] + c[1][1] * c[1][1] + c[2][1] * c[2][1]);
    let k = (tm * pc.size).abs();
    let (flip_s, flip_t) = match xf {
        Some(x) => (x.affine[0][0] < 0.0, x.affine[1][1] < 0.0),
        None => (false, false),
    };
    Some(Projected {
        center: [tx, ty],
        half: [k * mx, k * my],
        depth,
        flip_s,
        flip_t,
    })
}

#[cfg(test)]
#[path = "particle_tests.rs"]
pub(crate) mod tests;
