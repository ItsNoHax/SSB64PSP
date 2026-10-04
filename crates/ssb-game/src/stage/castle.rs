//! Peach's Castle: the Bumper riding the animated ground (`grcastle.c`).

use super::{
    mapobj, objects_of, StageAnim, StageInit, StageItem, StageItems, StageObj, StageObjects,
};
use ssb_engine::math::Vec3;

/// `GRCommonGroundVarsCastle`.
#[derive(Debug, Clone, PartialEq)]
pub struct Castle {
    pub bumper: Option<u32>,
    /// `mpMapObjKindBumper`'s position, the Bumper's offset from the ground.
    pub bumper_pos: Vec3,
    /// This frame's Bumper X (`grCastleBumperProcUpdate`), for the item.
    pub bumper_x: f32,
}

impl Castle {
    /// `grCastleInitAll` @ 0x8010B378.
    pub fn new(
        init: &StageInit<'_>,
        objects: &mut dyn StageObjects,
        items: &mut dyn StageItems,
    ) -> Self {
        objects.play(StageAnim::CastleGround);
        let bumper_pos = objects_of(init.map_objects, mapobj::BUMPER)
            .next()
            .unwrap_or(Vec3::ZERO);
        let bumper = items.make_item(
            StageItem::Bumper {
                castle: true,
                joint: None,
            },
            bumper_pos,
        );
        Castle {
            bumper,
            bumper_pos,
            bumper_x: bumper_pos.x,
        }
    }

    /// `grCastleBumperProcUpdate` @ 0x8010B340 (priority 4, before the
    /// ground's own animation process at 5 of the next frame, and before
    /// the Bumper's own `itProcessProcItemMain` at 3 of this one).
    pub fn tick(&mut self, objects: &dyn StageObjects, items: &mut dyn StageItems) {
        if let Some(bumper) = self.bumper {
            self.bumper_x = objects.translate(StageObj::CastleGround).x + self.bumper_pos.x;
            items.set_item_x(bumper, self.bumper_x);
        }
    }
}
