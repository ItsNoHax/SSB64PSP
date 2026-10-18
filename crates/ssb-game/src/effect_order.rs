//! The draw order of the fighters' display-link-15 effects.
//!
//! `efManagerMakeEffect` adds each effect's display with
//! `gcAddGObjDisplay(gobj, proc_display, dl_link, 2, -1)`, and
//! `gcDLLinkGObjTail` (`src/sys/objman.c`) inserts a `GObj` after the last
//! one of the same or a higher priority: link 15's priority-2 effects draw
//! in the order they were made, whichever fighter owns them. The renderer
//! keeps a creation stamp per effect ([`Stamp`]) from one counter
//! ([`CreationOrder`]) and draws the live ones sorted by it ([`order`]).
//!
//! Effects made on the same frame take the order [`CreationOrder::stamp`]
//! is called in: the renderer stamps ports in ascending order, which is
//! the fighters' `GObj` order, and one port's kinds in [`Kind`] order. The
//! original's order there also depends on which fighter process made each
//! (`proc_interrupt` before `proc_physics` across all fighters); this does
//! not model that, and two fighters making effects on one frame is rare.

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
}

impl Kind {
    pub const COUNT: usize = 9;
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
    ];
}

/// One effect's liveness and creation stamp.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stamp {
    live: bool,
    seq: u32,
}

impl Stamp {
    /// Whether the effect lived at the last [`CreationOrder::stamp`].
    pub fn live(self) -> bool {
        self.live
    }

    /// The creation stamp of the effect while it lives.
    pub fn seq(self) -> Option<u32> {
        self.live.then_some(self.seq)
    }
}

/// The counter every [`Stamp`] draws from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CreationOrder {
    next: u32,
}

impl CreationOrder {
    /// Records an effect's liveness this frame. It takes the next stamp when
    /// it starts to live, or when `remade` says the game ejected and made it
    /// again without a frame between.
    pub fn stamp(&mut self, stamp: &mut Stamp, live: bool, remade: bool) {
        if live && (!stamp.live || remade) {
            stamp.seq = self.next;
            self.next = self.next.wrapping_add(1);
        }
        stamp.live = live;
    }
}

/// A live effect to draw: its stamp, owner port and kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    pub seq: u32,
    pub port: u8,
    pub kind: Kind,
}

/// The most effects one frame draws: every kind on four ports.
pub const MAX_ENTRIES: usize = 4 * Kind::COUNT;

/// Sorts `entries` into draw order: by creation stamp, stably, so entries
/// that share a stamp keep the order they were collected in.
pub fn order(entries: &mut [Entry]) {
    // Insertion sort: stable, allocation-free and at most 36 entries.
    for i in 1..entries.len() {
        let mut j = i;
        while j > 0 && entries[j - 1].seq > entries[j].seq {
            entries.swap(j - 1, j);
            j -= 1;
        }
    }
}

/// The live effects of `stamps` (one row per port, indexed by [`Kind`]) in
/// draw order. Returns the filled length of `out`.
pub fn collect(stamps: &[[Stamp; Kind::COUNT]], out: &mut [Entry; MAX_ENTRIES]) -> usize {
    let mut n = 0;
    for (port, row) in stamps.iter().enumerate().take(4) {
        for kind in Kind::ALL {
            if let Some(seq) = row[kind as usize].seq() {
                out[n] = Entry {
                    seq,
                    port: port as u8,
                    kind,
                };
                n += 1;
            }
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

    fn drawn(stamps: &[[Stamp; Kind::COUNT]]) -> alloc::vec::Vec<(u8, Kind)> {
        let mut out = [Entry {
            seq: 0,
            port: 0,
            kind: Kind::Shield,
        }; MAX_ENTRIES];
        let n = collect(stamps, &mut out);
        out[..n].iter().map(|e| (e.port, e.kind)).collect()
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
        assert_eq!(s[0][Kind::Shield as usize].seq(), None);
    }

    #[test]
    fn order_is_stable_for_equal_stamps() {
        let mut e = [
            Entry {
                seq: 5,
                port: 0,
                kind: Kind::Sing,
            },
            Entry {
                seq: 2,
                port: 3,
                kind: Kind::Kick,
            },
            Entry {
                seq: 5,
                port: 1,
                kind: Kind::Magnet,
            },
            Entry {
                seq: 1,
                port: 2,
                kind: Kind::Shield,
            },
        ];
        order(&mut e);
        let got: alloc::vec::Vec<_> = e.iter().map(|e| (e.seq, e.port)).collect();
        assert_eq!(got, [(1, 2), (2, 3), (5, 0), (5, 1)]);
    }
}
