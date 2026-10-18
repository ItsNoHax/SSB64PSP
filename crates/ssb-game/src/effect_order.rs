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
//! fighter effect the counter's value at the start of the frame the
//! renderer first sees it live ([`CreationOrder::begin`]) plus a sub-stamp.
//! The renderer draws the live ones sorted by key ([`collect`]).
//!
//! A fighter effect so sorts after every display made on an earlier frame
//! and before the display effects made on its own frame. `gcRunAll` runs
//! the process queue by priority: the fighters' `ftMainProcUpdateInterrupt`
//! (5) and `ftMainProcPhysicsMapDefault` (4), which make every [`Kind`],
//! come before the weapons', items' and effects' main processes (3) and
//! the hit searches (1 and 0), which make most display effects.
//!
//! Fighter effects made on the same frame take the order
//! [`CreationOrder::stamp`] is called in: the renderer stamps ports in
//! ascending order, which is the fighters' `GObj` order, and one port's
//! kinds in [`Kind`] order. The original's order there also depends on
//! which fighter process made each (every fighter's priority-5 process
//! before any priority-4 one); this does not model that, and two fighters
//! making link-15 effects on one frame from different processes is rare.

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
}

impl Stamp {
    /// Whether the effect lived at the last [`CreationOrder::stamp`].
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

/// The counter every [`Stamp`] draws from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CreationOrder {
    /// The effects' `GObj` counter at the start of the frame being stamped.
    base: u32,
    /// The counter at the last [`Self::begin`]: the next frame's `base`.
    end: u32,
    /// The sub-stamp, never zero once used.
    next: u32,
}

impl CreationOrder {
    /// Starts a frame's stamping: `last_seq` is the effects' `GObj` counter
    /// after the frame's processes ran, the `seq` of the last effect made.
    /// The frame's fighter effects take the counter as the previous frame
    /// left it. A counter that went back is a new match's.
    pub fn begin(&mut self, last_seq: u32) {
        self.base = if last_seq < self.end { 0 } else { self.end };
        self.end = last_seq;
    }

    /// Records an effect's liveness this frame. It takes the next key when
    /// it starts to live, or when `remade` says the game ejected and made it
    /// again without a frame between.
    pub fn stamp(&mut self, stamp: &mut Stamp, live: bool, remade: bool) {
        if live && (!stamp.live || remade) {
            self.next = self.next.wrapping_add(1).max(1);
            stamp.key = display_key(self.base) | u64::from(self.next);
        }
        stamp.live = live;
    }
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

    fn stamps() -> [[Stamp; Kind::COUNT]; 4] {
        [[Stamp::default(); Kind::COUNT]; 4]
    }

    fn drawn_with(
        stamps: &[[Stamp; Kind::COUNT]],
        displays: &[(u32, u8)],
    ) -> alloc::vec::Vec<Item> {
        let mut out = [Entry::EMPTY; MAX_ENTRIES];
        let n = collect(stamps, displays.iter().copied(), &mut out);
        out[..n].iter().map(|e| e.item).collect()
    }

    fn drawn(stamps: &[[Stamp; Kind::COUNT]]) -> alloc::vec::Vec<(u8, Kind)> {
        drawn_with(stamps, &[])
            .into_iter()
            .map(|i| match i {
                Item::Fighter { port, kind } => (port, kind),
                Item::Display(_) => unreachable!(),
            })
            .collect()
    }

    fn f(port: u8, kind: Kind) -> Item {
        Item::Fighter { port, kind }
    }

    #[test]
    fn a_later_port_effect_made_first_draws_first() {
        let mut order = CreationOrder::default();
        let mut s = stamps();
        // Frame 1: port 2's Sing is made.
        order.stamp(&mut s[2][Kind::Sing as usize], true, false);
        // Frame 2: port 0's Falcon Punch flame is made; the Sing lives on.
        order.stamp(&mut s[0][Kind::Punch as usize], true, false);
        order.stamp(&mut s[2][Kind::Sing as usize], true, false);
        assert_eq!(drawn(&s), [(2, Kind::Sing), (0, Kind::Punch)]);
    }

    #[test]
    fn a_shield_raised_after_a_flame_draws_over_it() {
        let mut order = CreationOrder::default();
        let mut s = stamps();
        order.stamp(&mut s[1][Kind::Punch as usize], true, false);
        order.stamp(&mut s[0][Kind::Shield as usize], false, false);
        // Next frame port 0 raises its shield.
        order.stamp(&mut s[0][Kind::Shield as usize], true, false);
        order.stamp(&mut s[1][Kind::Punch as usize], true, false);
        assert_eq!(drawn(&s), [(1, Kind::Punch), (0, Kind::Shield)]);
    }

    #[test]
    fn same_frame_makes_keep_port_then_kind_order() {
        let mut order = CreationOrder::default();
        let mut s = stamps();
        for (port, row) in s.iter_mut().enumerate() {
            order.stamp(&mut row[Kind::Magnet as usize], port == 3, false);
            order.stamp(&mut row[Kind::Kick as usize], port == 1, false);
            order.stamp(&mut row[Kind::Reflector as usize], port == 1, false);
        }
        assert_eq!(
            drawn(&s),
            [(1, Kind::Kick), (1, Kind::Reflector), (3, Kind::Magnet)]
        );
    }

    #[test]
    fn an_ejected_effect_made_again_goes_to_the_back() {
        let mut order = CreationOrder::default();
        let mut s = stamps();
        order.stamp(&mut s[0][Kind::Kick as usize], true, false);
        order.stamp(&mut s[1][Kind::Spin as usize], true, false);
        // Port 0's kick ends and starts again a frame later.
        order.stamp(&mut s[0][Kind::Kick as usize], false, false);
        order.stamp(&mut s[0][Kind::Kick as usize], true, false);
        assert_eq!(drawn(&s), [(1, Kind::Spin), (0, Kind::Kick)]);
        // Remade with no frame between: also new.
        order.stamp(&mut s[1][Kind::Spin as usize], true, true);
        assert_eq!(drawn(&s), [(0, Kind::Kick), (1, Kind::Spin)]);
        // Living on keeps its place.
        order.stamp(&mut s[0][Kind::Kick as usize], true, false);
        assert_eq!(drawn(&s), [(0, Kind::Kick), (1, Kind::Spin)]);
    }

    #[test]
    fn dead_effects_are_not_drawn() {
        let mut order = CreationOrder::default();
        let mut s = stamps();
        order.stamp(&mut s[0][Kind::Shield as usize], true, false);
        order.stamp(&mut s[0][Kind::Shield as usize], false, false);
        assert!(drawn(&s).is_empty());
        assert_eq!(s[0][Kind::Shield as usize].key(), None);
    }

    #[test]
    fn display_effects_merge_by_make_order() {
        let mut order = CreationOrder::default();
        let mut s = stamps();
        // Frame 1 ends with the counter at 4 (display seq 4 in slot 7).
        order.begin(4);
        // Frame 2: port 1 raises its shield (priority 5) and a hit makes
        // display seq 5 (slot 2) later in the frame: the shield is under it,
        // over the earlier display.
        order.begin(5);
        order.stamp(&mut s[1][Kind::Shield as usize], true, false);
        assert_eq!(
            drawn_with(&s, &[(5, 2), (4, 7)]),
            [Item::Display(7), f(1, Kind::Shield), Item::Display(2)]
        );
        // Frame 3: port 0's Falcon Punch flame, after both displays; the
        // shield keeps its place.
        order.begin(5);
        order.stamp(&mut s[1][Kind::Shield as usize], true, false);
        order.stamp(&mut s[0][Kind::Punch as usize], true, false);
        assert_eq!(
            drawn_with(&s, &[(5, 2), (4, 7)]),
            [
                Item::Display(7),
                f(1, Kind::Shield),
                Item::Display(2),
                f(0, Kind::Punch)
            ]
        );
    }

    #[test]
    fn a_new_match_restarts_the_base() {
        let mut order = CreationOrder::default();
        let mut s = stamps();
        order.begin(900);
        order.begin(950);
        // A new match: the counter starts again.
        order.begin(3);
        order.stamp(&mut s[0][Kind::Shield as usize], true, false);
        assert_eq!(
            drawn_with(&s, &[(2, 0), (3, 1)]),
            [f(0, Kind::Shield), Item::Display(0), Item::Display(1)]
        );
    }

    #[test]
    fn a_full_frame_is_clamped_to_the_output() {
        let mut order = CreationOrder::default();
        let mut s = stamps();
        for row in s.iter_mut() {
            for kind in Kind::ALL {
                order.stamp(&mut row[kind as usize], true, false);
            }
        }
        let displays: alloc::vec::Vec<(u32, u8)> = (0..crate::effect::DISPLAY_MAX as u8)
            .map(|i| (u32::from(i), i))
            .collect();
        let mut out = [Entry::EMPTY; MAX_ENTRIES];
        assert_eq!(collect(&s, displays, &mut out), MAX_ENTRIES);
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
