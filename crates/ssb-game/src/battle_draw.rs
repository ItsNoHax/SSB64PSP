//! The battle camera's draw plan: `gmCameraDefaultProcDisplay`
//! (`src/gm/gmcamera.c`) as an ordered list of [`Step`]s (RE-422).
//!
//! The camera draws six passes of display links. Each pass captures its
//! links' `GObj`s in link order (`gcCaptureCameraGObj`), and each display
//! callback writes to task head 0 or head 1; `syTaskmanUpdateDLBuffers`
//! chains head 0 into head 1, so a pass runs every head-0 list of all its
//! links, then every head-1 list of all its links. The plan splits each
//! object group into a [`Head::Zero`] step and a later [`Head::One`] step
//! of the same pass; a renderer skips the head-1 step of a group whose
//! head-0 step drew no head-1 content.
//!
//! Which head a group's lists go to:
//! - Items: `itDisplayOPAProcDisplay` draws the tree on head 0,
//!   `itDisplayXLUProcDisplay` (`ITAttributes::is_display_xlu`) both heads
//!   by `list_id`.
//! - Weapons: `wpDisplayDLHead1` and `lbCommonDObjScaleXProcDisplay` the
//!   whole tree on head 1 (every weapon whose `WPDesc` flags lack
//!   `WEAPON_FLAG_DOBJLINKS`), the `DLLinks` callbacks by `list_id`.
//! - Effects: `gcDrawDObjTreeForGObj` and `gcDrawDObjDLHead0` head 0,
//!   `lbCommonDObjScaleXProcDisplay` and `gcDrawDObjDLHead1` head 1, the
//!   `DLLinks` callbacks by `list_id`.
//! - Fighter bodies, shadows and particles: head 0. Sword trails
//!   (`ftDisplayMainDrawAfterImage`): head 1.

/// The task heads a step submits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Head {
    /// Head-0 lists only.
    Zero,
    /// Head-1 lists only.
    One,
    /// Both, in node order: only for a stage range with no head-1 lists on
    /// any stage (layer 2, links 13-15).
    Both,
}

/// What a step draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Group {
    /// The stage's render layers and objects on links `link..=last`.
    Stage,
    /// Fighters on link 1: Captain Falcon's leftward entry
    /// (`ftParamMoveDLLink`).
    EntryFighters,
    /// The entry effects on the step's link (2, 10 or 20, RE-425).
    EntryEffects,
    /// The fighters' shadows (link 7).
    Shadows,
    /// The fighters (link 9).
    Fighters,
    /// The title demos' fade (link 10), made before any effect.
    DemoFade,
    /// The trapping egg (link 10).
    EggEffects,
    /// The rebirth halo (link 10) or the KO explosion (link 18).
    KoEffects,
    /// The display effects of the step's link (10 or 18).
    DisplayEffects,
    /// The particle lists of the step's link (10, 15 or 18).
    Particles,
    /// The items (link 11).
    Items,
    /// The container smash pieces (link 11), made after the items.
    ContainerPieces,
    /// The fighters' sword trails (link 9, head 1).
    SwordTrails,
    /// The weapons (link 14).
    Weapons,
    /// Link Spin Attack's weapon graphic (link 14), then link 15's
    /// priority-2 effects ([`crate::effect_order`]).
    Link15Effects,
    /// The Poké Ball and stage monsters drawn on link 18.
    Monsters,
}

/// [`Group`]s, for a table indexed by `group as usize`.
pub const GROUPS: usize = Group::Monsters as usize + 1;

impl Group {
    /// Whether the group's draws are filtered by head: mesh draws whose
    /// lists can go to either head. The others draw whole in their fixed
    /// head's step.
    pub fn split(self) -> bool {
        !matches!(
            self,
            Group::EntryFighters
                | Group::Shadows
                | Group::Fighters
                | Group::DemoFade
                | Group::Particles
                | Group::SwordTrails
        )
    }
}

/// One draw of the plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Step {
    /// The camera pass, 1-6.
    pub pass: u8,
    /// The display link; a [`Group::Stage`] range's first link with stage
    /// content.
    pub link: u8,
    /// A [`Group::Stage`] range's last link; `link` otherwise.
    pub last: u8,
    pub group: Group,
    pub head: Head,
}

const fn step(pass: u8, link: u8, group: Group, head: Head) -> Step {
    Step {
        pass,
        link,
        last: link,
        group,
        head,
    }
}

const fn stage(pass: u8, first: u8, last: u8, head: Head) -> Step {
    Step {
        pass,
        link: first,
        last,
        group: Group::Stage,
        head,
    }
}

use Group as G;
use Head::{Both, One, Zero};

/// The battle's six passes, through link 20. The stage ranges cover the
/// links between the passes' own (RE-422 "Links": stage content is on
/// links 4, 6, 12 (the acid), 13, 16 and 17).
pub const BATTLE: [Step; 42] = [
    // Pass 1: links 1-2.
    step(1, 1, G::EntryFighters, Zero),
    step(1, 2, G::EntryEffects, Zero),
    step(1, 2, G::EntryEffects, One),
    // Pass 2: link 4 (layer 0).
    stage(2, 0, 5, Zero),
    stage(2, 0, 5, One),
    // Pass 3: links 6-12.
    stage(3, 6, 8, Zero),
    step(3, 7, G::Shadows, Zero),
    step(3, 9, G::Fighters, Zero),
    step(3, 10, G::DemoFade, Zero),
    step(3, 10, G::EggEffects, Zero),
    step(3, 10, G::EntryEffects, Zero),
    step(3, 10, G::KoEffects, Zero),
    step(3, 10, G::DisplayEffects, Zero),
    step(3, 10, G::Particles, Zero),
    step(3, 11, G::Items, Zero),
    step(3, 11, G::ContainerPieces, Zero),
    // The stage controller's objects on links 9-11 draw here, after the
    // items, rather than interleaved with the fighters and items by link.
    stage(3, 9, 12, Zero),
    // The stage's only head-1 lists of the pass are layer 1's (link 6).
    stage(3, 6, 12, One),
    step(3, 9, G::SwordTrails, One),
    step(3, 10, G::EggEffects, One),
    step(3, 10, G::EntryEffects, One),
    step(3, 10, G::KoEffects, One),
    step(3, 10, G::DisplayEffects, One),
    step(3, 11, G::Items, One),
    step(3, 11, G::ContainerPieces, One),
    // Pass 4: links 13-15.
    stage(4, 13, 15, Both),
    step(4, 14, G::Weapons, Zero),
    step(4, 15, G::Link15Effects, Zero),
    step(4, 15, G::Particles, Zero),
    step(4, 14, G::Weapons, One),
    step(4, 15, G::Link15Effects, One),
    // Pass 5: links 16-18.
    stage(5, 16, 17, Zero),
    step(5, 18, G::Monsters, Zero),
    step(5, 18, G::KoEffects, Zero),
    step(5, 18, G::DisplayEffects, Zero),
    step(5, 18, G::Particles, Zero),
    stage(5, 16, 18, One),
    step(5, 18, G::Monsters, One),
    step(5, 18, G::KoEffects, One),
    step(5, 18, G::DisplayEffects, One),
    // Pass 6: links 19-20.
    step(6, 20, G::EntryEffects, Zero),
    step(6, 20, G::EntryEffects, One),
];

#[cfg(test)]
mod tests {
    use super::*;

    /// `gmCameraDefaultProcDisplay`'s `camera_mask` per pass
    /// (`gmcamera.c:1057-1102`).
    const MASKS: [&[u8]; 6] = [
        &[1, 2],
        &[4],
        &[6, 7, 9, 10, 11, 12],
        &[13, 14, 15],
        &[16, 17, 18],
        &[19, 20],
    ];

    fn steps() -> Vec<Step> {
        BATTLE.to_vec()
    }

    #[test]
    fn passes_ascend_and_draw_only_their_links() {
        let s = steps();
        for w in s.windows(2) {
            assert!(w[0].pass <= w[1].pass, "{w:?}");
        }
        for st in &s {
            let mask = MASKS[usize::from(st.pass) - 1];
            assert!(st.link <= st.last, "{st:?}");
            // A stage range spans the renderer's stage links around the
            // pass's own: it must cover one of them.
            let covered = if st.group == Group::Stage {
                mask.iter().any(|l| (st.link..=st.last).contains(l))
            } else {
                mask.contains(&st.link)
            };
            assert!(covered, "{st:?}");
        }
    }

    /// `syTaskmanUpdateDLBuffers`: a pass's head-0 lists all run before its
    /// head-1 lists.
    #[test]
    fn a_pass_draws_every_head_zero_step_before_any_head_one_step() {
        for pass in 1..=6 {
            let heads: Vec<_> = steps()
                .into_iter()
                .filter(|s| s.pass == pass)
                .map(|s| s.head)
                .collect();
            let first_one = heads.iter().position(|&h| h == One).unwrap();
            assert!(heads[first_one..].iter().all(|&h| h == One), "pass {pass}");
        }
    }

    /// `gcCaptureCameraGObj` walks the links in ascending order within each
    /// head. The stage ranges are left out: pass 3's head-0 range 9-12
    /// draws after the items.
    #[test]
    fn each_head_of_a_pass_draws_its_links_in_ascending_order() {
        for pass in 1..=6 {
            for head in [Zero, One] {
                let links: Vec<_> = steps()
                    .into_iter()
                    .filter(|s| s.pass == pass && s.group != Group::Stage)
                    .filter(|s| s.head == head || (head == Zero && s.head == Both))
                    .map(|s| s.link)
                    .collect();
                assert!(
                    links.windows(2).all(|w| w[0] <= w[1]),
                    "pass {pass} {head:?} {links:?}"
                );
            }
        }
    }

    /// A split group's head-1 step follows its head-0 step in the same
    /// pass; fixed-head groups draw once.
    #[test]
    fn split_groups_draw_both_heads_in_the_same_pass() {
        let s = steps();
        for (i, st) in s.iter().enumerate() {
            let later = |head| {
                s[i + 1..].iter().any(|o| {
                    o.pass == st.pass && o.group == st.group && o.link == st.link && o.head == head
                })
            };
            let earlier = |head| {
                s[..i].iter().any(|o| {
                    o.pass == st.pass && o.group == st.group && o.link == st.link && o.head == head
                })
            };
            match (st.group.split(), st.head) {
                (true, Zero) if st.group != Group::Stage => assert!(later(One), "{st:?}"),
                (true, One) if st.group != Group::Stage => assert!(earlier(Zero), "{st:?}"),
                (false, h) => {
                    let fixed = if st.group == Group::SwordTrails {
                        One
                    } else {
                        Zero
                    };
                    assert_eq!(h, fixed, "{st:?}");
                }
                _ => {}
            }
        }
    }

    /// D1/D2 of the batch: the weapons' head-1 lists draw after link 15's
    /// head-0 effects and particle list 1; the items' head-1 lists after
    /// layer 1's.
    #[test]
    fn weapons_head_one_follows_link_fifteen_head_zero_and_items_follow_layer_one() {
        let s = steps();
        let at = |group, link, head| {
            s.iter()
                .position(|o| o.group == group && o.link == link && o.head == head)
                .unwrap()
        };
        let weapons = at(Group::Weapons, 14, One);
        assert!(at(Group::Link15Effects, 15, Zero) < weapons);
        assert!(at(Group::Particles, 15, Zero) < weapons);
        assert!(weapons < at(Group::Link15Effects, 15, One));
        assert!(at(Group::Stage, 6, One) < at(Group::Items, 11, One));
        assert!(at(Group::Stage, 9, Zero) < at(Group::Stage, 6, One));
        assert!(at(Group::SwordTrails, 9, One) < at(Group::DisplayEffects, 10, One));
    }

    /// No stage link draws twice in one head, and each head-0 stage link's
    /// head-1 lists draw in the same pass.
    #[test]
    fn stage_steps_cover_each_head_once() {
        for pass in 1..=6 {
            let cover = |head: Head| {
                let mut links = Vec::new();
                for s in BATTLE
                    .iter()
                    .filter(|s| s.pass == pass && s.group == Group::Stage)
                {
                    if s.head == head || s.head == Both {
                        links.extend(s.link..=s.last);
                    }
                }
                links
            };
            let (zero, one) = (cover(Zero), cover(One));
            for links in [&zero, &one] {
                let mut unique = links.clone();
                unique.sort_unstable();
                unique.dedup();
                assert_eq!(unique.len(), links.len(), "pass {pass}");
            }
            assert!(zero.iter().all(|l| one.contains(l)), "pass {pass}");
        }
    }

    #[test]
    fn groups_index_a_table() {
        assert!(steps().iter().all(|s| (s.group as usize) < GROUPS));
    }
}
