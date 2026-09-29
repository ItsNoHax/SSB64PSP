//! The team-attack rule of the hit and catch searches.
//!
//! Every search that can hurt or catch a teammate opens with the same test
//! of two `gSCManagerBattleState` fields and two teams:
//!
//! ```c
//! if ((gSCManagerBattleState->is_team_battle == TRUE) &&
//!     (gSCManagerBattleState->is_team_attack == FALSE) &&
//!     (this->team == other->team)) goto next;
//! ```
//!
//! (`ftMainSearchHitFighter`, `ftMainSearchHitWeapon`, `ftMainSearchHitItem`,
//! `ftMainSearchFighterCatch`, `itProcessSearchHitFighter`,
//! `itProcessSearchHitItem`, `itProcessSearchHitWeapon`). [`TeamRules`]
//! carries the two fields; the battle hands it to the searches, and the
//! fighter, weapon and item each carry their own `team`.
//!
//! A fighter's `team` is its `FTDesc::team`: `scVSBattleStartBattle` passes
//! `players[i].player`, which the VS select leaves as the port in a
//! free-for-all and as the team in a team battle; Training passes its own
//! `players[i].team` (0 for the player, 1 for the dummy). A weapon or item
//! takes its owner's team when it is made or picked up
//! (`wpManagerMakeWeapon`, `itMainSetFighterHold`) and its reflector's on a
//! reflect; an ownerless one has [`TEAM_DEFAULT`].
//!
//! The CPU's fighter walks (`ftComputerCheckFindTarget` and the rest) skip
//! a teammate by `team` alone, with no rule test: in a free-for-all every
//! fighter's team is its own port. See
//! [`crate::computer::behave::opponents`].

/// `WEAPON_TEAM_DEFAULT` and `ITEM_TEAM_DEFAULT`: the team of a weapon or
/// item nobody owns, which matches no fighter.
pub const TEAM_DEFAULT: u8 = 4;

/// `gSCManagerBattleState->is_team_battle` and `->is_team_attack`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TeamRules {
    pub is_team_battle: bool,
    /// VS Options' Team Attack. `dSCManagerDefaultBattleState` has it off,
    /// and VS Options, which would turn it on, is not ported.
    pub is_team_attack: bool,
}

impl TeamRules {
    /// A free-for-all or Training: nobody is spared.
    pub const FREE_FOR_ALL: TeamRules = TeamRules {
        is_team_battle: false,
        is_team_attack: false,
    };

    /// A team battle with `dSCManagerDefaultBattleState`'s Team Attack off.
    pub const TEAMS: TeamRules = TeamRules {
        is_team_battle: true,
        is_team_attack: false,
    };

    /// `is_team_battle == TRUE && is_team_attack == FALSE && a == b`: the
    /// searches skip the pair.
    pub fn spares(self, a: u8, b: u8) -> bool {
        self.is_team_battle && !self.is_team_attack && a == b
    }
}

#[cfg(test)]
#[path = "team_tests.rs"]
mod tests;
