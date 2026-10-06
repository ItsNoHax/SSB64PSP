//! The N64 logo (`mn/mncommon/mnstartup.c`) and the opening movie
//! (`mv/mvopening/*.c`), the attract loop's last boundary (RE-467).
//!
//! The movie is nineteen scenes in the order their exits name: the room,
//! the portraits, eight fighters, the run, the cliff, Saffron, the jungle,
//! the Yoshis' nest, Sector Z, the standoff, the clash and the newcomers;
//! A, B or START in any of them loads the title. The room resets the
//! scheduler's tic count (`sySchedulerSetTicCount(0)`) as the music starts,
//! and every later scene's start busy-waits for the count to reach its
//! [`Kind::start_tic`] (`sySchedulerGetTicCount`), holding the last
//! picture meanwhile. [`Clock`] is that count, one tic per frame.
//!
//! Each scene keeps a [`movie::World`] of its cameras and display objects;
//! the fighter scenes and the jungle also run a battle on a stage, which the
//! host makes from their [`fighters::Battle`].

pub mod clash;
pub mod cliff;
pub mod fighters;
pub mod movie;
pub mod newcomers;
pub mod portraits;
pub mod room;
pub mod run;
pub mod sector;
pub mod standoff;
pub mod startup;
pub mod yamabuki;
pub mod yoster;

/// The opening's scenes (`nSCKindOpening*`), in the order they play.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    Room,
    Portraits,
    Mario,
    Donkey,
    Link,
    Samus,
    Yoshi,
    Kirby,
    Fox,
    Pikachu,
    Run,
    Cliff,
    Yamabuki,
    Jungle,
    Yoster,
    Sector,
    Standoff,
    Clash,
    Newcomers,
}

impl Kind {
    /// Every scene, in play order.
    pub const ORDER: [Kind; 19] = [
        Kind::Room,
        Kind::Portraits,
        Kind::Mario,
        Kind::Donkey,
        Kind::Link,
        Kind::Samus,
        Kind::Yoshi,
        Kind::Kirby,
        Kind::Fox,
        Kind::Pikachu,
        Kind::Run,
        Kind::Cliff,
        Kind::Yamabuki,
        Kind::Jungle,
        Kind::Yoster,
        Kind::Sector,
        Kind::Standoff,
        Kind::Clash,
        Kind::Newcomers,
    ];

    /// The scene its last tick loads; `None` after the newcomers, which
    /// load the title.
    pub fn next(self) -> Option<Kind> {
        let i = Kind::ORDER.iter().position(|&k| k == self)?;
        Kind::ORDER.get(i + 1).copied()
    }

    /// `while (sySchedulerGetTicCount() < N)` at the end of the scene's
    /// `FuncStart`; the room starts the count.
    pub fn start_tic(self) -> Option<u32> {
        Some(match self {
            Kind::Room => return None,
            Kind::Portraits => 1335,
            Kind::Mario => 1515,
            Kind::Donkey => 1605,
            Kind::Link => 1695,
            Kind::Samus => 1785,
            Kind::Yoshi => 1875,
            Kind::Kirby => 1965,
            Kind::Fox => 2055,
            Kind::Pikachu => 2145,
            Kind::Run => 2250,
            Kind::Cliff => 2500,
            Kind::Yamabuki => 2690,
            Kind::Jungle => 2880,
            Kind::Yoster => 3230,
            Kind::Sector => 3420,
            Kind::Standoff => 3610,
            Kind::Clash => 3975,
            Kind::Newcomers => 4155,
        })
    }

    /// The eight fighter scenes' fighter (`mvOpening<Name>`).
    pub fn fighter(self) -> Option<crate::fighter::FighterKind> {
        use crate::fighter::FighterKind as F;
        Some(match self {
            Kind::Mario => F::Mario,
            Kind::Donkey => F::Donkey,
            Kind::Link => F::Link,
            Kind::Samus => F::Samus,
            Kind::Yoshi => F::Yoshi,
            Kind::Kirby => F::Kirby,
            Kind::Fox => F::Fox,
            Kind::Pikachu => F::Pikachu,
            _ => return None,
        })
    }

    /// Whether the scene runs a battle (`game_type ==
    /// nSCBattleGameTypeMovie`): the eight fighters and the jungle.
    pub fn is_battle(self) -> bool {
        self.fighter().is_some() || self == Kind::Jungle
    }
}

/// `mnTitleFuncStart` after the newcomers waits for this tic.
pub const TITLE_START_TIC: u32 = 4215;

/// `sSYSchedulerTicCount` since the room reset it: one tic per frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Clock {
    pub tic: u32,
}

impl Clock {
    /// `sySchedulerSetTicCount(0)`.
    pub fn reset(&mut self) {
        self.tic = 0;
    }

    /// `sySchedulerVRetrace`.
    pub fn retrace(&mut self) {
        self.tic = self.tic.wrapping_add(1);
    }

    /// Whether `kind`'s start has stopped waiting.
    pub fn ready(&self, kind: Kind) -> bool {
        kind.start_tic().is_none_or(|t| self.tic >= t)
    }
}

/// Where a scene's `syTaskmanSetLoadScene` goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    /// `nSCKindTitle`.
    Title,
    Next(Kind),
}

impl Exit {
    /// The scene's own end: the next scene, or the title after the last.
    pub fn after(kind: Kind) -> Exit {
        kind.next().map_or(Exit::Title, Exit::Next)
    }
}

/// The scenes' shared `func_run` prologue: `TotalTimeTics++`, then from
/// tic `skip_from` A, B or START loads the title. Returns the tic count.
pub fn run_tics(tics: &mut i32, tapped: bool, skip_from: i32, exit: &mut Option<Exit>) -> i32 {
    *tics += 1;
    if *tics >= skip_from && tapped {
        *exit = Some(Exit::Title);
    }
    *tics
}
