//! The draw order of display link 15's effects.
//!
//! `efManagerMakeEffect` adds each effect's display with
//! `gcAddGObjDisplay(gobj, proc_display, dl_link, 2, -1)`, and
//! `gcDLLinkGObjTail` (`src/sys/objman.c`) inserts a `GObj` after the last
//! one of the same or a higher priority: link 15's priority-2 effects draw
//! in the order they were made, whichever fighter owns them and whether the
//! port draws them as a fighter effect ([`Kind`]) or as one of
//! [`crate::effect::Effects`]'s display effects. Both take a key from the
//! effects' `GObj` counter (RE-542): a display effect its make's `seq`, a
//! fighter effect the counter's value when its make is first seen
//! ([`FighterStamps::stamp`]) plus a sub-stamp. The renderer draws the live
//! ones sorted by key ([`collect`]).
//!
//! The fighter effects are stamped where each fighter process ends:
//! [`crate::effect::EffectRuntime`]'s `fighter` stamps a fighter's
//! [`fighter_lives`] before it makes the effects that process queued
//! ([`crate::fteffect`]). `gcRunAll` runs the processes by priority and
//! link 3's fighters in port order within one, so a fighter effect made in
//! `ftMainProcUpdateInterrupt` (5) sorts before one another fighter made
//! in `ftMainProcPhysicsMap`'s `proc_accessory` (4) whatever their ports,
//! after the display effects made before its process ran and before the
//! ones made after. One process's fighter effect sorts before the display
//! effects that process queued (stamp before flush): the original's order
//! inside one process is not modelled.

/// A fighter effect drawn in link 15's priority-2 pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Kind {
    /// `efManagerShieldMakeEffect`.
    Shield,
    /// `efManagerYoshiShieldMakeEffect`.
    YoshiShield,
    /// `efManagerYoshiEggEscapeMakeEffect`.
    YoshiEggEscape,
    /// `efManagerNessPsychicMagnetMakeEffect`.
    Magnet,
    /// `efManagerPurinSingMakeEffect`.
    Sing,
    /// `efManagerLinkSpinAttackMakeEffect`.
    Spin,
    /// `efManagerCaptainFalconPunchMakeEffect` (Captain Falcon's or a Kirby
    /// copy's).
    Punch,
    /// `efManagerCaptainFalconKickMakeEffect`.
    Kick,
    /// `efManagerFoxReflectorMakeEffect`.
    Reflector,
    /// `efManagerSamusGrappleBeamGlowMakeEffect`.
    GrappleGlow,
}

impl Kind {
    pub const COUNT: usize = 10;
    pub const ALL: [Kind; Kind::COUNT] = [
        Kind::Shield,
        Kind::YoshiShield,
        Kind::YoshiEggEscape,
        Kind::Magnet,
        Kind::Sing,
        Kind::Spin,
        Kind::Punch,
        Kind::Kick,
        Kind::Reflector,
        Kind::GrappleGlow,
    ];
}

/// One effect's liveness and creation key.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stamp {
    live: bool,
    key: u64,
    /// The effect's played frames at the last stamp, for the kinds that
    /// count them ([`fighter_lives`]).
    ticks: Option<u16>,
}

impl Stamp {
    /// Whether the effect lived at the last [`FighterStamps::stamp`].
    pub fn live(self) -> bool {
        self.live
    }

    /// The creation key of the effect while it lives.
    pub fn key(self) -> Option<u64> {
        self.live.then_some(self.key)
    }
}

/// The key of a display effect whose make took `seq` from the effects'
/// `GObj` counter.
pub fn display_key(seq: u32) -> u64 {
    u64::from(seq) << 32
}

/// One effect's state as a stamp reads it: whether it lives and, for the
/// kinds with a play count, how many frames it has played.
pub type Life = (bool, Option<u16>);

/// Each port's fighter-effect [`Stamp`]s, indexed by [`Kind`], and the
/// sub-stamp counter they share. A new match builds new ones with its new
/// [`crate::effect::Effects`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FighterStamps {
    rows: [[Stamp; Kind::COUNT]; 4],
    /// The last sub-stamp, never zero once used.
    next: u32,
}

impl FighterStamps {
    /// Records `port`'s effects' liveness, `seq` being the effects' `GObj`
    /// counter now (the `seq` of the last effect made). An effect takes the
    /// next key when it starts to live, or when its play count went back:
    /// the game ejected and made it again since the last stamp (the
    /// renderer's players restart on the same test).
    pub fn stamp(&mut self, port: u8, lives: &[Life; Kind::COUNT], seq: u32) {
        let row = &mut self.rows[usize::from(port & 3)];
        for (stamp, &(live, ticks)) in row.iter_mut().zip(lives) {
            let remade = matches!((stamp.ticks, ticks), (Some(was), Some(now)) if now < was);
            if live && (!stamp.live || remade) {
                self.next = self.next.wrapping_add(1).max(1);
                stamp.key = display_key(seq) | u64::from(self.next);
            }
            stamp.live = live;
            stamp.ticks = if live { ticks } else { None };
        }
    }

    /// The stamps, one row per port.
    pub fn rows(&self) -> &[[Stamp; Kind::COUNT]; 4] {
        &self.rows
    }
}

/// `f`'s link-15 effects' [`Life`]s by [`Kind`]: the state the renderer
/// draws them from. The shield bubble is not Yoshi's (his guard is his
/// egg); the counted kinds live while their play count does.
pub fn fighter_lives(f: &crate::fighter::Fighter) -> [Life; Kind::COUNT] {
    let shield = f.guard.is_shield && f.kind != crate::fighter::FighterKind::Yoshi;
    let egg_escape = f.yoshi.egg_escape_active;
    let counted = |t: Option<u16>| (t.is_some(), t);
    let punch =
        crate::captain::punch_effect_ticks(f).or_else(|| crate::kirby_copy::punch_effect_ticks(f));
    [
        (shield, None),
        (!egg_escape && crate::combat::is_yoshi_egg_shield(f), None),
        (egg_escape, None),
        counted(crate::ness::magnet_effect_ticks(f)),
        counted(crate::purin::sing_effect_ticks(f)),
        counted(crate::link::spin_effect_ticks(f)),
        counted(punch),
        counted(crate::captain::kick_effect_ticks(f)),
        counted(crate::status::fox_reflector_effect(f).map(|e| e.ticks)),
        counted(crate::grab::grapple_glow_ticks(f)),
    ]
}

/// What an [`Entry`] draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Item {
    /// A port's fighter effect.
    Fighter { port: u8, kind: Kind },
    /// The display effect in this pool slot of [`crate::effect::Effects`].
    Display(u8),
}

/// A live effect to draw: its key and what it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    pub key: u64,
    pub item: Item,
}

impl Entry {
    /// An empty entry to fill an output array with.
    pub const EMPTY: Entry = Entry {
        key: 0,
        item: Item::Display(0),
    };
}

/// The most effects one frame draws: every kind on four ports and every
/// display effect.
pub const MAX_ENTRIES: usize = 4 * Kind::COUNT + crate::effect::DISPLAY_MAX;

/// Sorts `entries` into draw order: by key, stably, so entries that share
/// a key keep the order they were collected in.
pub fn order(entries: &mut [Entry]) {
    // Insertion sort: stable and allocation-free; the display effects
    // arrive in pool order, which is mostly make order.
    for i in 1..entries.len() {
        let mut j = i;
        while j > 0 && entries[j - 1].key > entries[j].key {
            entries.swap(j - 1, j);
            j -= 1;
        }
    }
}

/// The live effects of `stamps` (one row per port, indexed by [`Kind`])
/// and the display effects `displays` (`(seq, pool slot)`) in draw order.
/// Returns the filled length of `out`.
pub fn collect(
    stamps: &[[Stamp; Kind::COUNT]],
    displays: impl IntoIterator<Item = (u32, u8)>,
    out: &mut [Entry; MAX_ENTRIES],
) -> usize {
    let mut n = 0;
    for (seq, slot) in displays {
        if n == MAX_ENTRIES {
            break;
        }
        out[n] = Entry {
            key: display_key(seq),
            item: Item::Display(slot),
        };
        n += 1;
    }
    for (port, row) in stamps.iter().enumerate().take(4) {
        for kind in Kind::ALL {
            let Some(key) = row[kind as usize].key() else {
                continue;
            };
            if n == MAX_ENTRIES {
                break;
            }
            out[n] = Entry {
                key,
                item: Item::Fighter {
                    port: port as u8,
                    kind,
                },
            };
            n += 1;
        }
    }
    order(&mut out[..n]);
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every port's lives, all dead, and the stamps they feed.
    struct World {
        lives: [[Life; Kind::COUNT]; 4],
        stamps: FighterStamps,
    }

    impl World {
        fn new() -> World {
            World {
                lives: [[(false, None); Kind::COUNT]; 4],
                stamps: FighterStamps::default(),
            }
        }

        /// Sets one effect's life (a bool kind's play count stays `None`).
        fn set(&mut self, port: u8, kind: Kind, live: bool) {
            self.lives[usize::from(port)][kind as usize] = (live, None);
        }

        /// Sets a counted effect's play count (`None`: not live).
        fn ticks(&mut self, port: u8, kind: Kind, ticks: Option<u16>) {
            self.lives[usize::from(port)][kind as usize] = (ticks.is_some(), ticks);
        }

        /// One fighter process's end: stamp `port` at counter `seq`.
        fn stamp(&mut self, port: u8, seq: u32) {
            self.stamps.stamp(port, &self.lives[usize::from(port)], seq);
        }

        fn key(&self, port: u8, kind: Kind) -> Option<u64> {
            self.stamps.rows()[usize::from(port)][kind as usize].key()
        }

        fn drawn_with(&self, displays: &[(u32, u8)]) -> alloc::vec::Vec<Item> {
            let mut out = [Entry::EMPTY; MAX_ENTRIES];
            let n = collect(self.stamps.rows(), displays.iter().copied(), &mut out);
            out[..n].iter().map(|e| e.item).collect()
        }

        fn drawn(&self) -> alloc::vec::Vec<(u8, Kind)> {
            self.drawn_with(&[])
                .into_iter()
                .map(|i| match i {
                    Item::Fighter { port, kind } => (port, kind),
                    Item::Display(_) => unreachable!(),
                })
                .collect()
        }
    }

    fn f(port: u8, kind: Kind) -> Item {
        Item::Fighter { port, kind }
    }

    #[test]
    fn a_later_port_effect_made_first_draws_first() {
        let mut w = World::new();
        // Frame 1: port 2's Sing is made.
        w.ticks(2, Kind::Sing, Some(1));
        w.stamp(0, 0);
        w.stamp(2, 0);
        // Frame 2: port 0's Falcon Punch flame is made; the Sing lives on.
        w.ticks(0, Kind::Punch, Some(1));
        w.ticks(2, Kind::Sing, Some(2));
        w.stamp(0, 0);
        w.stamp(2, 0);
        assert_eq!(w.drawn(), [(2, Kind::Sing), (0, Kind::Punch)]);
    }

    #[test]
    fn a_shield_raised_after_a_flame_draws_over_it() {
        let mut w = World::new();
        w.ticks(1, Kind::Punch, Some(1));
        w.stamp(0, 0);
        w.stamp(1, 0);
        // Next frame port 0 raises its shield.
        w.set(0, Kind::Shield, true);
        w.ticks(1, Kind::Punch, Some(2));
        w.stamp(0, 0);
        w.stamp(1, 0);
        assert_eq!(w.drawn(), [(1, Kind::Punch), (0, Kind::Shield)]);
    }

    #[test]
    fn one_stamp_keeps_kind_order() {
        let mut w = World::new();
        w.ticks(1, Kind::Kick, Some(1));
        w.set(1, Kind::Reflector, true);
        w.ticks(3, Kind::Magnet, Some(1));
        for port in 0..4 {
            w.stamp(port, 0);
        }
        assert_eq!(
            w.drawn(),
            [(1, Kind::Kick), (1, Kind::Reflector), (3, Kind::Magnet)]
        );
    }

    /// RE-542: a fighter effect made in a later process sorts after one
    /// another fighter made in an earlier one, whatever their ports. Port
    /// 1's shield comes up in `ftMainProcUpdateInterrupt` (priority 5),
    /// port 0's Falcon Kick in `ftMainProcPhysicsMap` (4).
    #[test]
    fn same_frame_makes_take_process_order() {
        let mut w = World::new();
        // Priority 5, both ports: port 1 raises its shield.
        w.set(1, Kind::Shield, true);
        w.stamp(0, 7);
        w.stamp(1, 7);
        // Priority 4, both ports: port 0's accessory makes the kick.
        w.ticks(0, Kind::Kick, Some(1));
        w.stamp(0, 7);
        w.stamp(1, 7);
        assert!(w.key(1, Kind::Shield) < w.key(0, Kind::Kick));
        assert_eq!(w.drawn(), [(1, Kind::Shield), (0, Kind::Kick)]);
    }

    /// A display effect made between two fighter processes draws between
    /// their effects.
    #[test]
    fn a_display_made_between_two_processes_draws_between() {
        let mut w = World::new();
        w.set(0, Kind::Shield, true);
        w.stamp(0, 4);
        // Port 0's process made display seq 5 (slot 3) after the stamp.
        w.set(1, Kind::Shield, true);
        w.stamp(1, 5);
        assert_eq!(
            w.drawn_with(&[(5, 3)]),
            [f(0, Kind::Shield), Item::Display(3), f(1, Kind::Shield)]
        );
    }

    #[test]
    fn an_ejected_effect_made_again_goes_to_the_back() {
        let mut w = World::new();
        w.ticks(0, Kind::Kick, Some(1));
        w.ticks(1, Kind::Spin, Some(1));
        w.stamp(0, 0);
        w.stamp(1, 0);
        // Port 0's kick ends and starts again a frame later.
        w.ticks(0, Kind::Kick, None);
        w.stamp(0, 0);
        w.ticks(0, Kind::Kick, Some(1));
        w.stamp(0, 0);
        assert_eq!(w.drawn(), [(1, Kind::Spin), (0, Kind::Kick)]);
    }

    /// A counted effect whose play count went back was ejected and made
    /// again with no stamp between: it goes to the back; one that played on
    /// or held keeps its place.
    #[test]
    fn a_play_count_that_went_back_is_a_new_make() {
        let mut w = World::new();
        w.ticks(0, Kind::Magnet, Some(5));
        w.stamp(0, 0);
        let first = w.key(0, Kind::Magnet);
        w.ticks(0, Kind::Magnet, Some(5));
        w.stamp(0, 0);
        assert_eq!(w.key(0, Kind::Magnet), first);
        w.ticks(0, Kind::Magnet, Some(6));
        w.stamp(0, 0);
        assert_eq!(w.key(0, Kind::Magnet), first);
        w.ticks(1, Kind::Sing, Some(1));
        w.stamp(1, 0);
        w.ticks(0, Kind::Magnet, Some(1));
        w.stamp(0, 0);
        let remade = w.key(0, Kind::Magnet);
        assert!(remade > first);
        assert_eq!(w.drawn(), [(1, Kind::Sing), (0, Kind::Magnet)]);
        // Dead, then made again at a higher count: new.
        w.ticks(0, Kind::Magnet, None);
        w.stamp(0, 0);
        w.ticks(0, Kind::Magnet, Some(9));
        w.stamp(0, 0);
        assert!(w.key(0, Kind::Magnet) > remade);
    }

    #[test]
    fn dead_effects_are_not_drawn() {
        let mut w = World::new();
        w.set(0, Kind::Shield, true);
        w.stamp(0, 0);
        w.set(0, Kind::Shield, false);
        w.stamp(0, 0);
        assert!(w.drawn().is_empty());
        assert_eq!(w.key(0, Kind::Shield), None);
    }

    #[test]
    fn display_effects_merge_by_make_order() {
        let mut w = World::new();
        // Frame 1 ends with the counter at 4 (display seq 4 in slot 7).
        // Frame 2: port 1 raises its shield (priority 5) and a hit makes
        // display seq 5 (slot 2) later in the frame: the shield is under
        // it, over the earlier display.
        w.set(1, Kind::Shield, true);
        w.stamp(1, 4);
        w.stamp(1, 5);
        assert_eq!(
            w.drawn_with(&[(5, 2), (4, 7)]),
            [Item::Display(7), f(1, Kind::Shield), Item::Display(2)]
        );
        // Frame 3: port 0's Falcon Punch flame, after both displays; the
        // shield keeps its place.
        w.ticks(0, Kind::Punch, Some(1));
        w.stamp(0, 5);
        w.stamp(1, 5);
        assert_eq!(
            w.drawn_with(&[(5, 2), (4, 7)]),
            [
                Item::Display(7),
                f(1, Kind::Shield),
                Item::Display(2),
                f(0, Kind::Punch)
            ]
        );
    }

    #[test]
    fn a_full_frame_is_clamped_to_the_output() {
        let mut w = World::new();
        for port in 0..4 {
            for kind in Kind::ALL {
                w.set(port, kind, true);
            }
            w.stamp(port, 0);
        }
        let displays: alloc::vec::Vec<(u32, u8)> = (0..crate::effect::DISPLAY_MAX as u8)
            .map(|i| (u32::from(i), i))
            .collect();
        let mut out = [Entry::EMPTY; MAX_ENTRIES];
        assert_eq!(collect(w.stamps.rows(), displays, &mut out), MAX_ENTRIES);
    }

    #[test]
    fn order_is_stable_for_equal_keys() {
        let mut e = [
            Entry {
                key: 5,
                item: f(0, Kind::Sing),
            },
            Entry {
                key: 2,
                item: f(3, Kind::Kick),
            },
            Entry {
                key: 5,
                item: f(1, Kind::Magnet),
            },
            Entry {
                key: 1,
                item: f(2, Kind::Shield),
            },
        ];
        order(&mut e);
        let got: alloc::vec::Vec<_> = e.iter().map(|e| (e.key, e.item)).collect();
        assert_eq!(
            got,
            [
                (1, f(2, Kind::Shield)),
                (2, f(3, Kind::Kick)),
                (5, f(0, Kind::Sing)),
                (5, f(1, Kind::Magnet))
            ]
        );
    }
}
