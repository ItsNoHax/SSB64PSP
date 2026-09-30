//! The in-game particle runtime's PSP half (RE-413): the packed banks as
//! `ssb_game::particle::Banks`, and `lbParticleDrawTextures` through the
//! battle camera.

use ssb_engine::math::{Mat4, Vec3};
use ssb_game::particle::{self as lb, flag, Banks, Particles, Script};
use ssb_rom::pack::{Pack, ParticleBankDesc, ParticleTextureDesc};

use crate::meshdraw::{self, DrawState, ParticleRect};

/// The banks a match loads: runtime bank 0 is `efcommon` (pack bank 0),
/// which `efDisplayInitAll` loads. The stages' own banks are not loaded.
pub struct PackBanks<'p, 'a> {
    pack: &'p Pack<'a>,
    common: ParticleBankDesc,
}

impl<'p, 'a> PackBanks<'p, 'a> {
    pub fn new(pack: &'p Pack<'a>) -> Option<PackBanks<'p, 'a>> {
        Some(PackBanks {
            pack,
            common: pack.particle_bank(0)?,
        })
    }

    /// The pack texture of a particle's frame.
    fn frame_texture(&self, pc: &lb::Particle) -> Option<(u32, ParticleTextureDesc)> {
        if pc.bank_id & 7 != 0 || u32::from(pc.texture_id) >= self.common.texture_count {
            return None;
        }
        let t = self
            .pack
            .particle_texture(self.common.first_texture + u32::from(pc.texture_id))?;
        if t.first_frame == ParticleTextureDesc::NO_FRAME || t.frame_count == 0 {
            return None;
        }
        Some((t.first_frame + u32::from(pc.frame_id).min(t.frame_count - 1), t))
    }
}

impl Banks for PackBanks<'_, '_> {
    fn script_count(&self, bank: u8) -> u16 {
        if bank == 0 {
            self.common.script_count as u16
        } else {
            0
        }
    }

    fn script(&self, bank: u8, id: u16) -> Option<Script<'_>> {
        if bank != 0 || u32::from(id) >= self.common.script_count {
            return None;
        }
        let s = self.pack.particle_script(self.common.first_script + u32::from(id))?;
        Some(Script {
            kind: s.kind,
            texture_id: s.texture_id,
            generator_lifetime: s.generator_lifetime,
            particle_lifetime: s.particle_lifetime,
            flags: s.flags,
            gravity: s.gravity,
            friction: s.friction,
            vel: Vec3::new(s.velocity[0], s.velocity[1], s.velocity[2]),
            unk_20: s.unknown_20,
            unk_24: s.unknown_24,
            update_rate: s.update_rate,
            size: s.size,
            bytecode: self.pack.particle_bytecode(&s).unwrap_or(&[]),
        })
    }

    fn texture_flags(&self, bank: u8, texture: u16) -> u32 {
        if bank != 0 {
            return 0;
        }
        self.pack
            .particle_texture(self.common.first_texture + u32::from(texture))
            .map_or(0, |t| t.flags)
    }
}

/// The lists in the order the battle's display links draw them
/// (`efDisplayInitAll`): list 4 at link 10, list 1 at 15, lists 0 and 2
/// at 18, and list 3 at the interface's 25.
pub const DRAW_ORDER: [usize; 5] = [4, 1, 0, 2, 3];

/// The list on DL link 10, in the fighters' camera pass (RE-422).
pub const LINK10_LISTS: [usize; 1] = [4];
/// The list on DL link 15, drawn before stage layer 3 (RE-422).
pub const LINK15_LISTS: [usize; 1] = [1];
/// The lists on DL link 18, drawn after stage layer 3 (RE-422).
pub const LINK18_LISTS: [usize; 2] = [0, 2];
/// The list on the interface's link 25.
pub const LINK25_LISTS: [usize; 1] = [3];

/// The lists `efDisplayZPerspAAXLUProcDisplay` draws depth-tested
/// (`G_RM_AA_ZB_XLU_SURF`): list 4, at link 10. The others draw
/// `G_RM_CLD_SURF` or `G_RM_XLU_SURF`, untested.
pub const DEPTH_TESTED: u16 = 1 << 4;

/// A power of two from 2 to 256: an axis `lbParticleDrawTextures` can mask.
fn maskable(n: u16) -> bool {
    n.is_power_of_two() && (2..=256).contains(&n)
}

/// Draws the live particles of `lists` as `lbParticleDrawTextures` does,
/// through `view` and `proj` onto the pillarboxed viewport, list 4
/// depth-tested ([`draw_lists`]). The battle draws each link's lists at
/// that link's place in its camera passes (RE-422); [`DRAW_ORDER`] is all
/// of them.
///
/// # Safety
///
/// Between `begin_frame` and `end_frame`; the pack must outlive the frame.
pub unsafe fn draw(
    banks: &PackBanks<'_, '_>,
    particles: &mut Particles,
    view: &Mat4,
    proj: &Mat4,
    lists: &[usize],
    draw_state: &mut DrawState,
) {
    let (vx, vy, vw, vh) = ssb_engine::coord::pillarboxed_viewport();
    let camera = Camera {
        view,
        proj,
        planes: lb::BATTLE_PLANES,
        ge_planes: (ssb_game::camera::DEFAULT_NEAR, ssb_game::camera::DEFAULT_FAR),
        rect: [vx as f32, vy as f32, vw as f32, vh as f32],
    };
    draw_lists(banks, particles, &camera, lists, DEPTH_TESTED, draw_state);
}

/// The camera a particle pass draws under: its view, projection and
/// `near`/`far`, and its viewport on the PSP screen as `[x, y, w, h]`.
pub struct Camera<'m> {
    pub view: &'m Mat4,
    pub proj: &'m Mat4,
    pub planes: (f32, f32),
    /// The planes of the GE projection `proj` was built with, for a
    /// depth-tested list's GE depth.
    pub ge_planes: (f32, f32),
    pub rect: [f32; 4],
}

/// `lbParticleDrawTextures` for `lists`, in that order. A list in
/// `depth_tested` (a bit per list) draws under `G_RM_AA_ZB_XLU_SURF` with
/// `G_ZS_PRIM` at the particle's projected depth: tested against what the
/// camera drew, never written (`efDisplayZPerspAAXLUProcDisplay`, RE-420).
///
/// # Safety
///
/// As [`draw`].
pub unsafe fn draw_lists(
    banks: &PackBanks<'_, '_>,
    particles: &mut Particles,
    camera: &Camera<'_>,
    lists: &[usize],
    depth_tested: u16,
    draw_state: &mut DrawState,
) {
    let links = lists.iter().fold(0u16, |m, &l| m | (1 << l));
    particles.prepare_draw(links);
    let [vx, vy, vw, vh] = camera.rect;
    let (hw, hh) = (vw * 0.5, vh * 0.5);
    for &link in lists {
        let tested = depth_tested & (1 << link) != 0;
        for (_, pc) in particles.list(link) {
            let xf = (pc.xf != lb::NIL).then(|| particles.transform(pc.xf));
            let Some(at) = lb::project(pc, xf, camera.view, camera.proj, camera.planes) else {
                continue;
            };
            let Some((texture, desc)) = banks.frame_texture(pc) else {
                continue;
            };
            let cx = vx + (at.center[0] + 1.0) * hw;
            let cy = vy + (1.0 - at.center[1]) * hh;
            let (dx, dy) = (at.half[0] * hw, at.half[1] * hh);
            let alpha_ref = if pc.flags & flag::DITHER != 0 {
                None
            } else if pc.flags & flag::ALPHABLEND != 0 {
                Some(pc.envcolor[3])
            } else {
                Some(0x08)
            };
            let rect = ParticleRect {
                x0: cx - dx,
                y0: cy - dy,
                x1: cx + dx,
                y1: cy + dy,
                flip_s: at.flip_s,
                flip_t: at.flip_t,
                mirror_s: pc.flags & flag::MASKS != 0 && maskable(desc.width),
                mirror_t: pc.flags & flag::MASKT != 0 && maskable(desc.height),
                prim: pc.primcolor,
                env: (pc.flags & flag::ENVCOLOR != 0).then_some(pc.envcolor),
                alpha_ref,
                depth: tested.then(|| ge_depth(at.depth, camera.ge_planes)),
            };
            meshdraw::draw_particle_rect(banks.pack, texture, &rect, draw_state);
        }
    }
}

/// The GE depth a vertex at eye-space `depth` gets under a projection with
/// planes `(n, f)` and `Gpu::init`'s `sceGuDepthRange(65535, 0)`:
/// `32767.5 * (1 - z)` for the normalised `z = ((f + n) d - 2 f n) /
/// ((f - n) d)`, 65535 at `n`.
fn ge_depth(depth: f32, (n, f): (f32, f32)) -> f32 {
    let z = ((f + n) * depth - 2.0 * f * n) / ((f - n) * depth);
    (32767.5 * (1.0 - z)).clamp(0.0, 65535.0)
}
