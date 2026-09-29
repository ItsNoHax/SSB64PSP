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

/// A power of two from 2 to 256: an axis `lbParticleDrawTextures` can mask.
fn maskable(n: u16) -> bool {
    n.is_power_of_two() && (2..=256).contains(&n)
}

/// Draws every live particle as `lbParticleDrawTextures` does, through
/// `view` and `proj` onto the pillarboxed viewport.
///
/// # Safety
///
/// Between `begin_frame` and `end_frame`; the pack must outlive the frame.
pub unsafe fn draw(
    banks: &PackBanks<'_, '_>,
    particles: &mut Particles,
    view: &Mat4,
    proj: &Mat4,
    draw_state: &mut DrawState,
) {
    let links = DRAW_ORDER.iter().fold(0u16, |m, &l| m | (1 << l));
    particles.prepare_draw(links);
    let (vx, vy, vw, vh) = ssb_engine::coord::pillarboxed_viewport();
    let (vx, vy, hw, hh) = (vx as f32, vy as f32, vw as f32 * 0.5, vh as f32 * 0.5);
    for &link in &DRAW_ORDER {
        for (_, pc) in particles.list(link) {
            let xf = (pc.xf != lb::NIL).then(|| particles.transform(pc.xf));
            let Some(at) = lb::project(pc, xf, view, proj) else {
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
            };
            meshdraw::draw_particle_rect(banks.pack, texture, &rect, draw_state);
        }
    }
}
