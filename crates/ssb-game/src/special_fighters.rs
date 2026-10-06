//! Metal Mario, the Fighting Polygon Team and Giant Donkey Kong (RE-458).
//!
//! They are not separate characters in the source: each `nFTKind` runs its
//! base fighter's special statuses (`dFTMainSpecialStatusDescs`), indexes
//! its own `dFT<Name>MotionDescs` with the base's motion enum, and reads its
//! own `FTAttributes` (`206_MMarioMain.c`, `2xx_N*Main.c`,
//! `215_GDonkeyMain.c`). [`FighterKind::character`] is the grouping, the
//! generated scripts and pack slots give each its own table, and the few
//! places the source treats one apart (Metal Mario's and Giant Donkey
//! Kong's knockback resistance, their entries, the CPU's branches, Yoshi's
//! guard that leaves Polygon Yoshi out) say so where they are. These tests
//! cover the bindings.

#[cfg(test)]
mod tests {
    use crate::fighter::{Fighter, FighterKind, Situation};
    use crate::status::{self, AnyStatus, MarioStatus, Status};
    use ssb_engine::input::N64Buttons;

    fn grounded(kind: FighterKind) -> Fighter {
        let mut f = Fighter::new(kind, 0, 3);
        f.situation = Situation::Ground;
        f.status.status = Status::Wait.into();
        f
    }

    fn press_b(f: &mut Fighter, y: i8) {
        f.input.stick_y = y;
        f.stick.step(0, y, false, false);
        f.prev_input.buttons = N64Buttons::default();
        f.input.buttons = N64Buttons(N64Buttons::B);
    }

    #[test]
    fn every_variant_runs_its_base_fighters_specials() {
        let pairs = [
            (FighterKind::MetalMario, FighterKind::Mario),
            (FighterKind::GiantDonkey, FighterKind::Donkey),
            (FighterKind::PolyMario, FighterKind::Mario),
            (FighterKind::PolyLuigi, FighterKind::Luigi),
            (FighterKind::PolyNess, FighterKind::Ness),
        ];
        for (variant, base) in pairs {
            assert_eq!(variant.character(), base);
        }
        for kind in FighterKind::PLAYABLE {
            assert_eq!(kind.character(), *kind);
            assert!(kind.has_specials());
        }
        assert!(FighterKind::MetalMario.has_specials());
        assert!(FighterKind::GiantDonkey.has_specials());
        assert!(!FighterKind::PolyFox.has_specials());
    }

    #[test]
    fn metal_mario_throws_mario_fireballs_and_the_polygons_none() {
        let mut f = grounded(FighterKind::MetalMario);
        press_b(&mut f, 0);
        assert!(status::check_special_n(&mut f));
        assert_eq!(f.status.status, AnyStatus::Mario(MarioStatus::SpecialN));
        // `is_have_specialn` is 0 for every Polygon.
        let mut p = grounded(FighterKind::PolyMario);
        press_b(&mut p, 0);
        assert!(!status::check_special_n(&mut p));
        press_b(&mut p, 80);
        assert!(!status::check_special_hi(&mut p));
    }

    #[test]
    fn variants_have_their_own_motion_tables() {
        use crate::motion::{fighter_scripts, motion_desc};
        for kind in (13..=26).filter_map(FighterKind::from_ordinal) {
            assert!(fighter_scripts(kind).is_some(), "{kind:?}");
            assert!(crate::motion::combat_attrs(kind).is_some(), "{kind:?}");
            assert!(crate::hurtbox::damage_colls(kind).is_some(), "{kind:?}");
            assert!(crate::hurtbox::yoshi_egg_coll(kind).is_some(), "{kind:?}");
        }
        // Polygon Mario reads Mario's own script blob: every motion with a
        // script names the same words as Mario's (all its entries are
        // `FTANIM_FLAG_SUBMOTION_SCRIPT`).
        let mario = fighter_scripts(FighterKind::Mario).unwrap();
        let poly = fighter_scripts(FighterKind::PolyMario).unwrap();
        assert!(core::ptr::eq(mario.words, poly.words));
        // Metal Mario's blob is his own `205_MMarioMainMotion` then Mario's:
        // his Wait (`dMMarioMainMotion_Walk3 + 0x8` read from
        // `file_submotion`) is Mario's Wait script at 0x24.
        let metal = fighter_scripts(FighterKind::MetalMario).unwrap();
        let wait = motion_desc(FighterKind::MetalMario, Status::Wait.into()).unwrap();
        let mario_wait = motion_desc(FighterKind::Mario, Status::Wait.into()).unwrap();
        let offset = metal.words.len() - mario.words.len();
        assert_eq!(wait.script as usize, offset + mario_wait.script as usize);
        // Giant Donkey Kong is twice Donkey Kong's size.
        assert_eq!(
            crate::motion::combat_attrs(FighterKind::GiantDonkey)
                .unwrap()
                .size,
            2.0
        );
    }

    #[test]
    fn metal_mario_and_giant_donkey_resist_knockback() {
        assert_eq!(
            Fighter::new(FighterKind::MetalMario, 0, 3).knockback_resist_passive,
            30.0
        );
        assert_eq!(
            Fighter::new(FighterKind::GiantDonkey, 0, 3).knockback_resist_passive,
            48.0
        );
        assert_eq!(
            Fighter::new(FighterKind::Mario, 0, 3).knockback_resist_passive,
            0.0
        );
        assert_eq!(
            Fighter::new(FighterKind::PolyDonkey, 0, 3).knockback_resist_passive,
            0.0
        );
    }

    #[test]
    fn metal_mario_and_giant_donkey_enter_as_their_bases() {
        use crate::appear::{entry_effect, EntryEffect};
        assert_eq!(
            entry_effect(FighterKind::MetalMario),
            Some(EntryEffect::Pipe)
        );
        assert_eq!(
            entry_effect(FighterKind::GiantDonkey),
            Some(EntryEffect::Barrel)
        );
        assert_eq!(entry_effect(FighterKind::PolyMario), None);
        let mut f = grounded(FighterKind::MetalMario);
        crate::appear::appear_set_status(&mut f);
        assert_eq!(f.status.status, AnyStatus::Mario(MarioStatus::AppearR));
        let mut p = grounded(FighterKind::PolyFox);
        crate::appear::appear_set_status(&mut p);
        assert_eq!(p.status.status, AnyStatus::Common(Status::Wait));
    }

    #[test]
    fn the_variants_us_hurtboxes_keep_the_unrevised_sizes() {
        let mario = crate::hurtbox::damage_colls(FighterKind::Mario).unwrap();
        let metal = crate::hurtbox::damage_colls(FighterKind::MetalMario).unwrap();
        assert_eq!(mario.len(), metal.len());
        // The live boxes are the descriptors' halves (140 and 160).
        assert_eq!(mario[1].size.y, 70.0);
        assert_eq!(metal[1].size.y, 80.0);
        assert_eq!(&mario[2..], &metal[2..]);
        assert_eq!(
            crate::hurtbox::damage_colls(FighterKind::GiantDonkey),
            crate::hurtbox::damage_colls(FighterKind::Donkey)
        );
    }

    #[test]
    fn polygon_intro_frames_appear_on_their_thresholds() {
        use crate::spgame::intro::{polygon_frames, Entrance};
        for kind in (14..=25).filter_map(FighterKind::from_ordinal) {
            for frame in 0..3 {
                for tic in 0..80 {
                    let mut e = Entrance::polygon(kind, frame);
                    e.tick(tic);
                    assert_eq!(
                        e.shown,
                        polygon_frames(kind, tic)[frame],
                        "{kind:?} {frame} {tic}"
                    );
                }
            }
        }
    }
}
