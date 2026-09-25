//! Lowering of animated stage colour registers onto GE state (RE-322).
//!
//! `gcPlayMObjMatAnim` writes `PrimColor`, `Light1Color` and `Light2Color`
//! into `mobj->sub`, and `gcDrawMObjForDObj` emits them as
//! `gDPSetPrimColor` and `gSPLightColor(LIGHT_1/LIGHT_2, ...)`. A packed
//! primitive reads a live track only while its register still holds that
//! `MObj`'s value, which the packer records as [`flags::PRIM_ANIM`],
//! [`flags::LIGHT1_ANIM`] and [`flags::LIGHT2_ANIM`]. Everything here is
//! pure so the host tests can exercise what the PSP draw path decides.

use crate::pack::{flags, PrimDesc};
use crate::skeleton::EffectColors;

/// Keeps only the stage tracks whose register is still the animated one.
///
/// `EnvColor` and `BlendColor` pass through unchanged: no stage attachment
/// animates them (`romtool stages`), so they keep the pre-RE-322 behaviour.
pub fn stage_colors(colors: EffectColors, prim_flags: u32) -> EffectColors {
    EffectColors {
        prim: colors.prim.filter(|_| prim_flags & flags::PRIM_ANIM != 0),
        light1: colors
            .light1
            .filter(|_| prim_flags & flags::LIGHT1_ANIM != 0),
        light2: colors
            .light2
            .filter(|_| prim_flags & flags::LIGHT2_ANIM != 0),
        ..colors
    }
}

/// The `LIGHT_1`/`LIGHT_2` register values a lit primitive draws with, as
/// packed ABGR: the live track where one is present, otherwise the packed
/// `G_MW_LIGHTCOL`/`MObj` value, otherwise `None` (no authored write).
pub fn light_registers(colors: Option<EffectColors>, p: &PrimDesc) -> (Option<u32>, Option<u32>) {
    (
        colors
            .and_then(|c| c.packed_light1())
            .or_else(|| (p.flags & flags::LIGHT1_COLOR != 0).then_some(p.light1_color)),
        colors
            .and_then(|c| c.packed_light2())
            .or_else(|| (p.flags & flags::LIGHT2_COLOR != 0).then_some(p.light2_color)),
    )
}

/// Which GE lighting a primitive draws under.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LightScope {
    /// Lighting off: vertex colour, including the baked stage shade.
    Off,
    /// The fighter display's light (`ftDisplayLightsDrawReflect`).
    Fighter,
    /// A stage primitive whose light colour is animated: the RSP evaluates
    /// it with the scene's directional light (`sc1PGameFuncLights`,
    /// `scVSBattleFuncLights`), which the baked shade cannot follow.
    StageAnimated,
}

/// Decides [`LightScope`]. `stage_light` is whether the caller installed the
/// scene's stage light direction for this frame.
pub fn light_scope(prim_flags: u32, fighter_light: bool, stage_light: bool) -> LightScope {
    if prim_flags & flags::LIT == 0 {
        LightScope::Off
    } else if fighter_light {
        LightScope::Fighter
    } else if stage_light && prim_flags & (flags::LIGHT1_ANIM | flags::LIGHT2_ANIM) != 0 {
        LightScope::StageAnimated
    } else {
        LightScope::Off
    }
}

/// A GE register the draw path has written, or `None` while unknown.
///
/// `None` never means "known default": a frame start or an invalidation
/// forces the next write (RE-300's lesson for the texture function).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Latch<T>(Option<T>);

impl<T: Copy + PartialEq> Latch<T> {
    /// Records `value`; `true` when the GE needs the write.
    pub fn set(&mut self, value: T) -> bool {
        if self.0 == Some(value) {
            return false;
        }
        self.0 = Some(value);
        true
    }

    /// Forgets the register, so the next [`Self::set`] writes.
    pub fn invalidate(&mut self) {
        self.0 = None;
    }

    pub fn get(&self) -> Option<T> {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matanim::MaterialJoint;
    use crate::pack::PackWriter;
    use crate::skeleton::MaterialAnimator;

    const OP_SET_ANIM: u32 = 14;
    const OP_EXT_VAL_BLOCK: u32 = 20;
    const OP_SET_VAL_AFTER_BLOCK: u32 = 10;
    const PRIM: u32 = 1 << crate::matanim::TRACK_PRIM;
    const LIGHT1: u32 = 1 << crate::matanim::TRACK_LIGHT1;
    const LIGHT2: u32 = 1 << crate::matanim::TRACK_LIGHT2;

    const fn cmd(opcode: u32, flags: u32, payload: u32) -> u32 {
        (opcode << 25) | (flags << 15) | payload
    }

    fn bytes(words: &[u32]) -> alloc::vec::Vec<u8> {
        words.iter().flat_map(|w| w.to_be_bytes()).collect()
    }

    /// `dGRBonus3File2_Layer1MatAnim_MatAnimJoint_0x67F0`, verbatim, at
    /// offset 0: alpha 102 -> 0 -> 102, a one-tick dip to 100, 0, 102.
    fn glow_fade() -> alloc::vec::Vec<u8> {
        bytes(&[
            cmd(OP_EXT_VAL_BLOCK, PRIM, 0),
            0xFFFF_A366,
            cmd(OP_EXT_VAL_BLOCK, PRIM, 60),
            0xFFFF_A300,
            cmd(OP_EXT_VAL_BLOCK, PRIM, 60),
            0xFFFF_A366,
            cmd(OP_EXT_VAL_BLOCK, PRIM, 1),
            0xFFFF_A364,
            cmd(OP_EXT_VAL_BLOCK, PRIM, 59),
            0xFFFF_A300,
            cmd(OP_EXT_VAL_BLOCK, PRIM, 60),
            0xFFFF_A366,
            cmd(OP_SET_ANIM, 0, 0),
            0,
        ])
    }

    /// `..._0x6828`, verbatim: both lights fade to black and back together.
    fn light_fade() -> alloc::vec::Vec<u8> {
        bytes(&[
            cmd(OP_EXT_VAL_BLOCK, LIGHT1 | LIGHT2, 0),
            0xB3B3_B300,
            0x8080_8000,
            cmd(OP_EXT_VAL_BLOCK, LIGHT1 | LIGHT2, 60),
            0,
            0,
            cmd(OP_EXT_VAL_BLOCK, LIGHT1 | LIGHT2, 60),
            0xB3B3_B300,
            0x8080_8000,
            cmd(OP_EXT_VAL_BLOCK, LIGHT1 | LIGHT2, 1),
            0xB0B0_B000,
            0x7D7D_7D00,
            cmd(OP_EXT_VAL_BLOCK, LIGHT1 | LIGHT2, 59),
            0,
            0,
            cmd(OP_EXT_VAL_BLOCK, LIGHT1 | LIGHT2, 60),
            0xB3B3_B300,
            0x8080_8000,
            cmd(OP_SET_ANIM, 0, 0),
            0,
        ])
    }

    /// `gcPlayMObjMatAnim`'s linear colour step (`objanim.c:1350-1386`),
    /// written from the decomp independently of `MaterialJoint`.
    fn lerp(base: u32, target: u32, length: f32, duration: f32) -> [u8; 4] {
        let interp = ((length * (1.0 / duration) * 256.0) as i32).clamp(0, 256) as u32;
        let (b, t) = (base.to_be_bytes(), target.to_be_bytes());
        core::array::from_fn(|i| (((256 - interp) * b[i] as u32 + interp * t[i] as u32) >> 8) as u8)
    }

    fn after(data: &[u8], track: usize, ticks: u32) -> [u8; 4] {
        let mut j = MaterialJoint::start(0, 0.0);
        for _ in 0..ticks {
            j.tick(data, 1.0).unwrap();
        }
        j.track_color(track).unwrap()
    }

    const P: usize = crate::matanim::TRACK_PRIM_COLOR;
    const L1: usize = crate::matanim::TRACK_LIGHT1_COLOR;
    const L2: usize = crate::matanim::TRACK_LIGHT2_COLOR;

    #[test]
    fn prim_color_interpolates_each_byte_like_the_packed_multiply() {
        let d = glow_fade();
        // The engine's tick `n` is the decomp's frame `n`: a segment's
        // length is `n - 1` after `n` ticks, as the decomp's first
        // `gcPlayMObjMatAnim` sees 0.
        assert_eq!(
            after(&d, P, 1),
            [0xFF, 0xFF, 0xA3, 0x66],
            "rest alpha first"
        );
        assert_eq!(after(&d, P, 3), lerp(0xFFFF_A366, 0xFFFF_A300, 2.0, 60.0));
        assert_eq!(after(&d, P, 32), lerp(0xFFFF_A366, 0xFFFF_A300, 31.0, 60.0));
        assert_eq!(after(&d, P, 32)[3], 49, "midpoint alpha");
        // RGB is constant across every key: only alpha animates.
        assert_eq!(after(&d, P, 32)[..3], [0xFF, 0xFF, 0xA3]);
    }

    #[test]
    fn prim_color_reaches_each_key_exactly() {
        let d = glow_fade();
        assert_eq!(
            after(&d, P, 61),
            [0xFF, 0xFF, 0xA3, 0],
            "end of the first fade"
        );
        // The second segment starts from exactly the first one's target.
        assert_eq!(after(&d, P, 62), lerp(0xFFFF_A300, 0xFFFF_A366, 1.0, 60.0));
        assert_eq!(after(&d, P, 121)[3], 102, "back to the rest alpha");
        // One-tick key: 102 -> 100 in a single step.
        assert_eq!(after(&d, P, 122)[3], 100);
    }

    #[test]
    fn prim_color_loops_through_set_anim_without_a_seam() {
        let d = glow_fade();
        // Period: 60 + 60 + 1 + 59 + 60 = 240 ticks.
        for tick in [1, 17, 58, 59, 119, 120, 178, 179, 239] {
            assert_eq!(after(&d, P, tick), after(&d, P, tick + 240), "tick {tick}");
        }
        // Across the loop the last segment's target is the first one's base.
        assert_eq!(after(&d, P, 241)[3], 102);
        assert_eq!(after(&d, P, 242), lerp(0xFFFF_A366, 0xFFFF_A300, 1.0, 60.0));
    }

    #[test]
    fn light1_and_light2_interpolate_independently() {
        let d = light_fade();
        let (l1, l2) = (after(&d, L1, 32), after(&d, L2, 32));
        assert_eq!(l1, lerp(0xB3B3_B300, 0, 31.0, 60.0));
        assert_eq!(l2, lerp(0x8080_8000, 0, 31.0, 60.0));
        assert_ne!(l1, l2);
        assert_eq!(after(&d, L1, 61), [0; 4]);
        assert_eq!(after(&d, L2, 121), [0x80, 0x80, 0x80, 0]);
        assert_eq!(after(&d, L1, 122), [0xB0, 0xB0, 0xB0, 0]);
        assert_eq!(after(&d, L2, 122), [0x7D, 0x7D, 0x7D, 0]);
    }

    fn prim(flags: u32) -> PrimDesc {
        PrimDesc {
            texture: PrimDesc::NO_TEXTURE,
            flags,
            prim_color: 0,
            env_color: 0,
            index_offset: 0,
            index_count: 0,
            texture_blend_base: 0,
            texture_blend_target: 0,
            flat_color: 0,
            light1_color: 0x00B3_B3B3,
            light2_color: 0x0080_8080,
            alpha_compare_ref: 0,
            mat_anim: 0,
            texgen_scale_s: 0,
            texgen_scale_t: 0,
            texgen_origin_s: 0,
            texgen_origin_t: 0,
            phase_s_q5: 0,
            phase_t_q5: 0,
        }
    }

    #[test]
    fn stage_tracks_apply_only_where_the_register_is_still_animated() {
        let live = EffectColors {
            prim: Some([1, 2, 3, 4]),
            light1: Some([5, 6, 7, 0]),
            light2: Some([8, 9, 10, 0]),
            ..EffectColors::default()
        };
        assert_eq!(stage_colors(live, 0), EffectColors::default());
        let only_prim = stage_colors(live, flags::PRIM_ANIM);
        assert_eq!(
            (only_prim.prim, only_prim.light1),
            (Some([1, 2, 3, 4]), None)
        );
        let only_l2 = stage_colors(live, flags::LIGHT2_ANIM);
        assert_eq!(
            (only_l2.light1, only_l2.light2),
            (None, Some([8, 9, 10, 0]))
        );
    }

    #[test]
    fn light_registers_fall_back_to_the_packed_static_value() {
        let statics = flags::LIT | flags::LIGHT1_COLOR | flags::LIGHT2_COLOR;
        let p = prim(statics);
        assert_eq!(
            light_registers(None, &p),
            (Some(0x00B3_B3B3), Some(0x0080_8080))
        );
        let animated = EffectColors {
            light1: Some([0x10, 0x10, 0x10, 0]),
            ..EffectColors::default()
        };
        assert_eq!(
            light_registers(Some(animated), &p),
            (Some(0x0010_1010), Some(0x0080_8080)),
            "LIGHT_2 keeps its own value while LIGHT_1 animates"
        );
        assert_eq!(light_registers(None, &prim(flags::LIT)), (None, None));
    }

    #[test]
    fn light_scope_is_stage_animated_only_for_lit_animated_primitives() {
        let lit = flags::LIT | flags::LIGHT1_ANIM;
        assert_eq!(light_scope(lit, false, true), LightScope::StageAnimated);
        assert_eq!(
            light_scope(flags::LIT | flags::LIGHT2_ANIM, false, true),
            LightScope::StageAnimated
        );
        assert_eq!(
            light_scope(lit, false, false),
            LightScope::Off,
            "no stage light installed"
        );
        assert_eq!(
            light_scope(flags::LIGHT1_ANIM, false, true),
            LightScope::Off,
            "unlit"
        );
        assert_eq!(
            light_scope(flags::LIT, false, true),
            LightScope::Off,
            "static stage shade"
        );
        assert_eq!(light_scope(lit, true, true), LightScope::Fighter);
    }

    #[test]
    fn latch_skips_identical_writes_and_never_a_changed_one() {
        let mut l = Latch::default();
        assert!(l.set((Some(1u32), None)), "unknown always writes");
        assert!(!l.set((Some(1), None)), "identical skipped");
        assert!(l.set((Some(2), None)), "changed LIGHT_1 written");
        assert!(l.set((Some(2), Some(0))), "changed LIGHT_2 written");
        l.invalidate();
        assert!(l.set((Some(2), Some(0))), "invalidation forces a reinstall");
    }

    #[test]
    fn static_and_animated_primitives_alternate_without_stale_registers() {
        let d = light_fade();
        let mut j = MaterialJoint::start(0, 0.0);
        for _ in 0..32 {
            j.tick(&d, 1.0).unwrap();
        }
        let live = EffectColors {
            light1: j.track_color(L1),
            light2: j.track_color(L2),
            ..EffectColors::default()
        };
        let statics = flags::LIT | flags::LIGHT1_COLOR | flags::LIGHT2_COLOR;
        let animated = prim(statics | flags::LIGHT1_ANIM | flags::LIGHT2_ANIM);
        let fixed = prim(statics);
        let registers = |p: &PrimDesc| light_registers(Some(stage_colors(live, p.flags)), p);

        let mut cache = Latch::default();
        assert!(cache.set(registers(&fixed)), "static first installs");
        assert!(
            cache.set(registers(&animated)),
            "animated after static installs"
        );
        assert_eq!(
            cache.get().unwrap().0,
            Some(crate::psp_texture::pack_abgr(lerp(
                0xB3B3_B300,
                0,
                31.0,
                60.0
            )))
        );
        assert!(
            cache.set(registers(&fixed)),
            "static after animated restores"
        );
        assert_eq!(cache.get(), Some((Some(0x00B3_B3B3), Some(0x0080_8080))));
        assert!(!cache.set(registers(&fixed)), "and is then cached");
    }

    #[test]
    fn colour_tracks_stay_in_step_with_a_palette_track_on_the_same_clock() {
        // Palette 0/1 alternating every 4 ticks and a prim alpha ramp on the
        // same script, ticked by the stage `MaterialAnimator`.
        const PALETTE: u32 = 1 << crate::matanim::TRACK_PALETTE_ID;
        let d = bytes(&[
            cmd(OP_EXT_VAL_BLOCK, PRIM, 0),
            0xFFFF_FF00,
            cmd(OP_SET_VAL_AFTER_BLOCK, PALETTE, 0),
            0,
            cmd(OP_EXT_VAL_BLOCK, PRIM, 4),
            0xFFFF_FF80,
            cmd(OP_SET_VAL_AFTER_BLOCK, PALETTE, 4),
            0x3F80_0000,
            cmd(OP_SET_ANIM, 0, 0),
            0,
        ]);
        let mut w = PackWriter::new();
        let i = w.add_mat_anim(
            149,
            &d,
            0,
            0,
            &[alloc::vec![0; 16], alloc::vec![1; 16]],
            &[],
            [0; 10],
            0,
            [0; 3],
        );
        let packed = w.finish();
        let pack = crate::pack::Pack::open(&packed).unwrap();
        let mut stage = MaterialAnimator::new();
        stage.start(&pack);
        let mut joint = MaterialJoint::start(0, 0.0);
        for tick in 1..40 {
            stage.tick(&pack);
            joint.tick(&d, 1.0).unwrap();
            let colors = stage.resolved_colors(i).unwrap();
            assert_eq!(colors.prim, joint.track_color(P), "tick {tick}");
            let palette = stage
                .resolved_palette(&pack, i)
                .map(|p| p - pack.mat_anim(i).unwrap().first_palette);
            let expected = joint
                .track_value(crate::matanim::TRACK_PALETTE_ID)
                .map(|v| (v + 0.5) as u32);
            assert_eq!(palette, expected, "tick {tick}");
        }
    }
}
