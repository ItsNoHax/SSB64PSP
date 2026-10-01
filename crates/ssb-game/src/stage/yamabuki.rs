//! Saffron City: the Poké Center gate (`gryamabuki.c`).
//!
//! The gate is collision group 3. Its X position follows the emerging
//! Pokémon; the Pokémon themselves are stage items made through
//! [`StageItems::make_item`].

use super::{StageAnim, StageItem, StageItems, StageObjects};
use crate::fighter::Fighter;
use crate::hazard::material;
use crate::map::{GroupStatus, MapGroup};
use crate::rng;
use ssb_engine::math::Vec3;

/// The gate's collision group.
pub const GATE_GROUP: u8 = 3;
/// `nITKindGroundMonsterEnd - nITKindGroundMonsterStart + 1`.
pub const MONSTER_KINDS: i32 = 5;
pub const GATE_NEAR_X: f32 = 960.0;
pub const GATE_FAR_X: f32 = 1600.0;

/// `grYamabukiGateStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GateStatus {
    Sleep,
    Wait,
    Open,
}

/// `GRCommonGroundVarsYamabuki`.
#[derive(Debug, Clone, PartialEq)]
pub struct Yamabuki {
    pub status: GateStatus,
    pub gate_pos: Vec3,
    pub no_entry: bool,
    pub monster_wait: u16,
    pub gate_wait: u16,
    pub monster_prev: u8,
    pub monster: Option<u32>,
    /// `mpMapObjKindMonster`'s position.
    pub monster_pos: Vec3,
}

fn gate_y(groups: &[MapGroup]) -> f32 {
    groups
        .get(GATE_GROUP as usize)
        .map_or(0.0, |g| g.translate.y)
}

impl Yamabuki {
    /// `grYamabukiInitGroundVars` @ 0x8010B250.
    pub fn new(
        groups: &mut [MapGroup],
        objects: &mut dyn StageObjects,
        items: &mut dyn StageItems,
    ) -> Self {
        Self::with_monster_pos(groups, objects, items, Vec3::ZERO)
    }

    pub fn with_monster_pos(
        groups: &mut [MapGroup],
        objects: &mut dyn StageObjects,
        _items: &mut dyn StageItems,
        monster_pos: Vec3,
    ) -> Self {
        if let Some(g) = groups.get_mut(GATE_GROUP as usize) {
            g.status = GroupStatus::On;
        }
        let y = Yamabuki {
            status: GateStatus::Sleep,
            gate_pos: Vec3::new(GATE_NEAR_X, gate_y(groups), 0.0),
            no_entry: false,
            monster_wait: 0,
            gate_wait: 1,
            monster_prev: MONSTER_KINDS as u8,
            monster: None,
            monster_pos,
        };
        objects.play(StageAnim::GateClose);
        y.update_group(groups);
        y
    }

    fn update_group(&self, groups: &mut [MapGroup]) {
        if let Some(g) = groups.get_mut(GATE_GROUP as usize) {
            g.set_position(self.gate_pos);
        }
    }

    /// `grYamabukiGateCheckPlayersNear` @ 0x8010AD18.
    fn players_near(fighters: &[&mut Fighter]) -> bool {
        fighters.iter().any(|f| {
            f.is_grounded()
                && f.floor.is_some_and(|s| {
                    s.flags & crate::collision::flags::MATERIAL == material::DETECT
                })
        })
    }

    /// `grYamabukiGateMakeMonster` @ 0x8010AD70 (`dITManagerForceMonsterKind`
    /// is 0 in retail).
    fn make_monster(&mut self, items: &mut dyn StageItems) {
        self.status = GateStatus::Open;
        self.no_entry = false;
        let mut id = rng::rand_int_range(MONSTER_KINDS) as u8;
        if id == self.monster_prev {
            id = if i32::from(id) == MONSTER_KINDS - 1 {
                0
            } else {
                id + 1
            };
        }
        self.monster_prev = id;
        self.monster = items.make_item(StageItem::Monster(id), self.monster_pos);
    }

    /// `grYamabukiGateSetClosedWait` @ 0x8010B0B8.
    fn set_closed_wait(&mut self, groups: &[MapGroup], objects: &mut dyn StageObjects) {
        self.status = GateStatus::Wait;
        self.gate_wait = 1000;
        self.monster_wait = (rng::rand_int_range(1000) + 1000) as u16;
        self.gate_pos = Vec3::new(GATE_NEAR_X, gate_y(groups), self.gate_pos.z);
        objects.play(StageAnim::GateClose);
    }

    /// `grYamabukiGateProcUpdate` @ 0x8010B130.
    pub fn tick(
        &mut self,
        fighters: &[&mut Fighter],
        groups: &mut [MapGroup],
        objects: &mut dyn StageObjects,
        items: &mut dyn StageItems,
        started: bool,
    ) {
        match self.status {
            GateStatus::Sleep => {
                if started {
                    self.status = GateStatus::Wait;
                    self.monster_wait = (rng::rand_int_range(1000) + 1000) as u16;
                }
            }
            GateStatus::Wait => {
                self.update_wait(fighters, groups, objects, items);
                self.update_group(groups);
            }
            GateStatus::Open => {
                self.update_open(groups, objects, &*items);
                self.update_group(groups);
            }
        }
    }

    /// `grYamabukiGateUpdateWait` @ 0x8010AF48.
    fn update_wait(
        &mut self,
        fighters: &[&mut Fighter],
        groups: &[MapGroup],
        objects: &mut dyn StageObjects,
        items: &mut dyn StageItems,
    ) {
        if self.gate_wait == 0 {
            if Self::players_near(fighters) {
                self.make_monster(items);
                return;
            }
        } else {
            self.gate_wait -= 1;
            if self.gate_wait == 0 {
                // `grYamabukiGateAddAnimOpenEntry`.
                objects.play(StageAnim::GateOpen);
                self.gate_pos = Vec3::new(GATE_FAR_X, gate_y(groups), self.gate_pos.z);
            }
        }
        self.monster_wait = self.monster_wait.wrapping_sub(1);
        if self.monster_wait == 0 {
            if self.gate_wait != 0 {
                objects.play(StageAnim::GateOpen);
            }
            self.make_monster(items);
        }
    }

    /// `grYamabukiGateUpdateOpen` @ 0x8010AFF4.
    fn update_open(
        &mut self,
        groups: &[MapGroup],
        objects: &mut dyn StageObjects,
        items: &dyn StageItems,
    ) {
        let Some(monster) = self.monster else {
            self.set_closed_wait(groups, objects);
            return;
        };
        if self.no_entry {
            return;
        }
        let Some((pos, width)) = items.item_pos_width(monster) else {
            return;
        };
        let mut x = pos.x - width;
        if x < GATE_NEAR_X {
            x = GATE_NEAR_X;
            self.no_entry = true;
        } else if x > GATE_FAR_X {
            x = GATE_FAR_X;
        }
        self.gate_pos = Vec3::new(x, gate_y(groups), self.gate_pos.z);
    }

    /// `grYamabukiGateClearMonsterGObj` @ 0x8010B0AC, called by the monster.
    pub fn clear_monster(&mut self) {
        self.monster = None;
    }
}
