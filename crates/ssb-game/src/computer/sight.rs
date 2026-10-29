//! What a CPU reads of the match besides the fighters
//! (`ftComputerProcessAll`'s reads of `gITManagerItemList`,
//! `gWPManagerWeaponList`, the Twister and the Zebes acid).

use ssb_engine::array_vec::ArrayVec;
use ssb_engine::math::{Vec2, Vec3};

use super::behave::{ItemSight, WeaponThreat};
use crate::item::{ItemPool, ITEM_ALLOC_MAX};
use crate::stage::{Controller, Stage};
use crate::team::TeamRules;
use crate::weapon::{WeaponPool, MAX_WEAPONS};

/// What a CPU reads of the match besides the fighters, as it stands when
/// its interrupt runs: the items and weapons in link order, the Twister,
/// the Zebes acid and its own PK Thunder trail.
pub struct CpuSight {
    pub items: ArrayVec<ItemSight, { ITEM_ALLOC_MAX }>,
    /// Up to two threats a weapon (`WeaponPool::cpu_threats`).
    pub weapons: ArrayVec<WeaponThreat, { 2 * MAX_WEAPONS }>,
    pub team_rules: TeamRules,
    pub twister: Option<Vec2>,
    pub acid: Option<(f32, f32)>,
    pub pk_thunder_trail: Option<Vec2>,
}

impl Default for CpuSight {
    fn default() -> Self {
        CpuSight {
            items: ArrayVec::new(),
            weapons: ArrayVec::new(),
            team_rules: TeamRules::FREE_FOR_ALL,
            twister: None,
            acid: None,
            pk_thunder_trail: None,
        }
    }
}

impl CpuSight {
    /// The view of the CPU on `port`.
    pub fn observe(port: u8, items: &ItemPool, weapons: &WeaponPool, stage: &Stage) -> Self {
        let mut sight = CpuSight::default();
        sight.observe_into(port, items, weapons, stage);
        sight
    }

    /// [`Self::observe`] into `self`, every field rewritten: returning the
    /// view moved its whole ArrayVec capacity, about 2.5 KB per CPU and
    /// frame (RE-580).
    pub fn observe_into(
        &mut self,
        port: u8,
        items: &ItemPool,
        weapons: &WeaponPool,
        stage: &Stage,
    ) {
        let flat = |v: Vec3| Vec2::new(v.x, v.y);
        self.team_rules = items.team_rules;
        self.twister = match &stage.controller {
            Controller::Hyrule(h) => h.visible_position().map(flat),
            _ => None,
        };
        self.acid = match &stage.controller {
            Controller::Zebes(z) => Some(z.level_info()),
            _ => None,
        };
        self.pk_thunder_trail = weapons.own_pk_trail(port).map(flat);
        self.items.clear();
        self.items.extend(items.cpu_sights());
        self.weapons.clear();
        self.weapons.extend(weapons.cpu_threats());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An empty match shows a CPU nothing, as [`CpuSight::default`] does,
    /// but the battle's team rules.
    #[test]
    fn an_empty_match_shows_only_the_team_rules() {
        let weapons: alloc::boxed::Box<WeaponPool> = alloc::boxed::Box::default();
        let mut items: alloc::boxed::Box<ItemPool> = alloc::boxed::Box::default();
        items.team_rules = TeamRules::TEAMS;
        let sight = CpuSight::observe(0, &items, &weapons, &Stage::none());
        let default = CpuSight::default();
        assert!(sight.items.is_empty() && sight.weapons.is_empty());
        assert_eq!(
            (sight.twister, sight.acid, sight.pk_thunder_trail),
            (default.twister, default.acid, default.pk_thunder_trail)
        );
        assert_eq!(sight.team_rules, TeamRules::TEAMS);
        assert_eq!(default.team_rules, TeamRules::FREE_FOR_ALL);
    }
}
