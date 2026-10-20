//! A demo fighter (`nFTPlayerKindDemo`, `dFTManagerDefaultFighterDesc`):
//! the selects', the results', Characters', the 1P scenes' and the movies'
//! fighters, which run only
//! `scSubsysFighterProcUpdate` (`ftMainPlayAnimEventsAll`, then
//! `ftMainRunUpdateColAnim`). Its status scripts run through the battle
//! motion runner ([`crate::motion::start_demo`], [`crate::motion::advance`])
//! on a [`Fighter`] marked [`Fighter::is_demo`], so their effects queue on
//! the fighter as a battle's do and the host makes them
//! ([`crate::fteffect::flush`]) where it holds its particle world.
//!
//! The host owns the clip; each frame it hands the clip's frame
//! (`fighter_gobj->anim_frame`, which wraps when the clip loops) to
//! [`DemoFighter::tick`] as the script's clock (`ftMainUpdateMotionEventsAll`'s
//! pause and async waits read it).

use alloc::boxed::Box;

use crate::fighter::{Facing, Fighter, FighterKind};

/// `FTSTATUS_PRESERVE_*` (ftdef.h): the bits a demo fighter's
/// `ftMainSetStatus` reads.
pub mod preserve {
    pub const NONE: u32 = 0;
    pub const COLANIM: u32 = 1 << 1;
    pub const EFFECT: u32 = 1 << 2;
    pub const MODELPART: u32 = 1 << 5;
    pub const TEXTUREPART: u32 = 1 << 7;
}

/// `ftMainSetStatus`'s resets, ahead of the new status's script: the
/// parts back to the base detail, a modified model or texture part set
/// (RE-426), an unlocked colour animation and the attached effects,
/// unless `flags` keeps them.
fn on_set_status(f: &mut Fighter, flags: u32) {
    let base = f.model_parts.detail_base;
    f.model_parts.set_detail_all(base);
    if flags & preserve::MODELPART == 0 && f.model_parts.is_modify {
        f.model_parts.reset_all();
    }
    if flags & preserve::TEXTUREPART == 0 && f.model_parts.texture.is_modify {
        f.model_parts.reset_textures();
    }
    crate::colanim::on_set_status(f, flags & preserve::COLANIM != 0);
    if flags & preserve::EFFECT == 0 {
        crate::fteffect::stop_on_set_status(f);
    }
}

/// A demo fighter. Boxed: a [`Fighter`] is several KB and the menu hosts
/// keep up to four.
#[derive(Debug, Clone)]
pub struct DemoFighter {
    pub fighter: Box<Fighter>,
}

impl DemoFighter {
    /// `ftManagerMakeFighter(&desc)` with `dFTManagerDefaultFighterDesc`'s
    /// defaults: `lr` right, `pkind` demo, unmuted; TopN's `attr->size`
    /// for the effects' `is_scale_pos` offsets. `port` names the fighter
    /// to the effect world (its attached effects).
    pub fn new(kind: FighterKind, port: u8, costume: u8) -> Self {
        let mut f = Box::new(Fighter::new(kind, port, 0));
        f.is_demo = true;
        f.facing = Facing::Right;
        f.topn_lr = 1.0;
        f.costume = costume;
        f.attributes.size = crate::motion::combat_attrs(kind)
            .or_else(|| crate::motion::combat_attrs(crate::grab::base_kind(kind)))
            .map_or(1.0, |a| a.size);
        DemoFighter { fighter: f }
    }

    /// [`Self::new`], then [`Self::set_status`].
    pub fn start(kind: FighterKind, port: u8, costume: u8, row: usize) -> Self {
        let mut d = DemoFighter::new(kind, port, costume);
        d.set_status(row);
        d
    }

    /// `scSubsysFighterSetStatus(fighter_gobj, nFTDemoStatusNull + row)`:
    /// `ftMainSetStatus(.., FTSTATUS_PRESERVE_NONE, 1.0F, 0.0F)`. The parts
    /// go back to the base detail, a modified model or texture part set
    /// resets (RE-426), and the row's script starts with its time-zero
    /// commands.
    pub fn set_status(&mut self, row: usize) {
        self.set_status_preserve(row, preserve::NONE);
    }

    /// `ftMainSetStatus(fighter_gobj, nFTDemoStatusNull + row, 0.0F, 1.0F,
    /// flags)`: [`Self::set_status`] keeping what `flags` names.
    pub fn set_status_preserve(&mut self, row: usize, flags: u32) {
        let f = &mut *self.fighter;
        on_set_status(f, flags);
        crate::motion::start_demo(f, row);
    }

    /// `ftMainSetStatus(fighter_gobj, status, 0.0F, 1.0F, flags)` for a
    /// battle status (`FTSTATUS_CHARACTERS_DEMO(status)`): its main motion
    /// script from time zero. A demo fighter skips the attack and stat
    /// IDs and the status procs (ftmain.c:4581, 4801), so only the resets
    /// and the figatree's hidden parts precede the script.
    pub fn set_main_status(&mut self, status: crate::status::AnyStatus, flags: u32) {
        let f = &mut *self.fighter;
        on_set_status(f, flags);
        if let Some(d) = crate::motion::motion_desc(f.kind, status).filter(|d| d.anim_length != 0) {
            f.model_parts.apply_hidden_parts(f.kind, d.anim_flags);
        }
        f.status.status = status;
        f.status.timing.anim_speed = 1.0;
        f.status.anim_frame = 0.0;
        crate::motion::start(f, 0.0);
    }

    /// One `scSubsysFighterProcUpdate` frame after the host played the
    /// clip, `anim_frame` being the clip's frame now (`gobj->anim_frame`):
    /// `ftMainPlayAnim`'s TopN transform reset, the status script's events
    /// (`ftMainUpdateMotionEventsAll`), then the colour animation
    /// (`ftMainRunUpdateColAnim`).
    pub fn tick(&mut self, anim_frame: f32) {
        let f = &mut *self.fighter;
        // `ftParamsUpdateFighterPartsTransform(TopN)` (RE-509).
        f.clear_joint_queries();
        f.status.anim_frame = anim_frame;
        crate::motion::advance(f);
        crate::colanim::run_update(f);
    }

    /// The model and texture parts the display draws.
    pub fn parts(&self) -> &crate::modelpart::ModelParts {
        &self.fighter.model_parts
    }

    /// Makes the effects the scripts queued (`ftParamMakeEffect`), in
    /// order; the joints they name must be sampled first.
    pub fn flush_effects(&mut self, rt: &mut crate::effect::EffectRuntime<'_>) {
        crate::fteffect::flush(&mut self.fighter, rt);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fteffect::{kind, EffectRequest, FighterEffect};
    use ssb_engine::math::Vec3;

    /// One frame of a clip that neither loops nor ends.
    fn step(d: &mut DemoFighter) {
        let frame = d.fighter.status.anim_frame + 1.0;
        d.tick(frame);
    }

    /// The effect requests the fighter queued, emptying the queue.
    fn take(d: &mut DemoFighter) -> Vec<EffectRequest> {
        let q = core::mem::take(&mut d.fighter.effects);
        q.iter()
            .filter_map(|e| match *e {
                FighterEffect::Param(r) => Some(r),
                _ => None,
            })
            .collect()
    }

    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn mario_blink_runs_through_the_battle_runner() {
        // scsubsysdatamario.c: sprite 2 for two frames, 3 for three,
        // 2 for two, then 0. The wait loop calls it again at tick 98.
        let mut d = DemoFighter::start(FighterKind::Mario, 0, 0, 0);
        let mut ids = Vec::new();
        for _ in 0..9 {
            ids.push(d.parts().draw_textures()[0]);
            step(&mut d);
        }
        assert_eq!(ids, [2, 2, 3, 3, 3, 2, 2, 0, 0]);
        for _ in 9..98 {
            step(&mut d);
        }
        assert_eq!(d.parts().draw_textures()[0], 2);
    }

    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn kirby_win_mouth_changes_at_the_source_frame() {
        let mut d = DemoFighter::start(FighterKind::Kirby, 0, 0, 1);
        for _ in 0..157 {
            step(&mut d);
        }
        assert_eq!(d.parts().draw_textures(), [0, 0]);
        step(&mut d);
        assert_eq!(d.parts().draw_textures(), [5, 0]);
        assert_eq!(
            DemoFighter::start(FighterKind::Kirby, 0, 0, 5)
                .parts()
                .draw_textures(),
            [5, 0]
        );
    }

    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn a_status_change_keeps_only_what_its_flags_preserve() {
        // Kirby's Win1 mouth is a texture part; a battle status without
        // `FTSTATUS_PRESERVE_TEXTUREPART` resets it, one with it keeps it.
        // Fall's script sets no texture part at time zero (Wait's does).
        let status = crate::status::AnyStatus::Common(crate::status::Status::Fall);
        let mut d = DemoFighter::start(FighterKind::Kirby, 0, 0, 5);
        assert_eq!(d.parts().draw_textures(), [5, 0]);
        d.set_main_status(status, preserve::TEXTUREPART);
        assert_eq!(d.parts().draw_textures(), [5, 0]);
        d.set_main_status(status, preserve::NONE);
        assert_eq!(d.parts().draw_textures(), [0, 0]);
    }

    /// `(frame, kind, joint)` of every effect the row queues in its first
    /// 240 frames.
    fn effects(kind: FighterKind, row: usize) -> Vec<(u32, u16, i8)> {
        let mut d = DemoFighter::start(kind, 0, 0, row);
        let mut out = Vec::new();
        for frame in 0..240 {
            for r in take(&mut d) {
                assert_eq!(r.lr, 1);
                out.push((frame, r.kind, r.joint));
            }
            step(&mut d);
        }
        out
    }

    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn link_selected_row_sparkles_his_sword_at_the_scripted_waits() {
        // Row 1, `llFTLinkAnimSelected`'s `D_ovl1_803918D0`: three
        // `Effect(SparkleWhiteScale, joint 11)` after async waits to 5, 25
        // and 60.
        let s = kind::SPARKLE_WHITE_SCALE;
        assert_eq!(
            effects(FighterKind::Link, 1),
            [(5, s, 11), (25, s, 11), (60, s, 11)]
        );
    }

    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn samus_wins_sparkle_joint_16_and_ness_win1_dusts_topn() {
        let (s, w) = (kind::SPARKLE_WHITE_SCALE, kind::SPARKLE_WHITE);
        // Row 1 (Win3 clip, `D_ovl1_803914F0`): two at 19 and 46, then a
        // three-pass loop of `SparkleWhite` 16 frames apart from 79.
        assert_eq!(
            effects(FighterKind::Samus, 1),
            [
                (19, s, 16),
                (46, s, 16),
                (79, w, 16),
                (95, w, 16),
                (111, w, 16)
            ]
        );
        // Row 2 (Win1 clip, `D_ovl1_8039154C`).
        assert_eq!(effects(FighterKind::Samus, 2), [(75, s, 16)]);
        // Row 1 (`D_ovl1_803926C8`): `DustLight` on TopN at each landing.
        let d = kind::DUST_LIGHT;
        assert_eq!(effects(FighterKind::Ness, 1), [(27, d, 0), (57, d, 0)]);
    }

    #[test]
    #[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
    fn link_opening_jump_queues_its_dust_at_time_zero() {
        // `D_ovl1_8039198C`: `PlayFGM(0x62)`, then
        // `Effect(nEFKindDustHeavyDouble, TopN)`, both before any wait.
        let mut d = DemoFighter::start(FighterKind::Link, 0, 0, 7);
        let r = take(&mut d);
        assert_eq!(r.len(), 1);
        assert_eq!((r[0].kind, r[0].joint), (kind::DUST_HEAVY_DOUBLE, 0));
        assert_eq!(
            (r[0].offset, r[0].scatter),
            (Some(Vec3::ZERO), Some(Vec3::ZERO))
        );
        step(&mut d);
        assert!(take(&mut d).is_empty());
        // Mario's run row has no script.
        assert!(take(&mut DemoFighter::start(FighterKind::Mario, 0, 0, 6)).is_empty());
    }

    #[test]
    fn fighter_size_fits_the_menu_hosts() {
        // Four results fighters on the heap.
        let size = core::mem::size_of::<Fighter>();
        assert!(size < 32 * 1024, "{size}");
    }
}
