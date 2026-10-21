//! The PSP's stage for the shared battle frame ([`ssb_game::matchtick`]):
//! the pack's map, the stage's objects and its moving groups.

use crate::scene::{FighterScene, MapSegments, StageMap, StageObjectsPort};
use ssb_game::fighter::Fighter;
use ssb_game::map::MapGroup;
use ssb_game::matchtick::{Arena, Ground, HasFighter};
use ssb_game::stage::{MapQuery, Stage, StageItems, TickInput};
use ssb_game::status::BlastZone;
use ssb_rom::pack::{Pack, StageDesc};

/// The stage's map as the fighters and pools read it: the pack's lines,
/// the moving groups where the stage left them.
pub struct PackGround<'a, 'p> {
    pub pack: &'a Pack<'p>,
    pub stage: &'a StageDesc,
    pub map: Option<&'a mut StageMap>,
}

impl PackGround<'_, '_> {
    /// The moving collision groups; none without a stage map.
    pub fn groups(&self) -> &[MapGroup] {
        self.map
            .as_deref()
            .map_or(&[][..], |map| map.groups.as_slice())
    }
}

impl Ground for PackGround<'_, '_> {
    type Surfaces<'s>
        = MapSegments<'s>
    where
        Self: 's;

    fn surfaces(&self) -> MapSegments<'_> {
        MapSegments::with_groups(self.pack, self.stage, self.groups())
    }

    fn groups(&self) -> &[MapGroup] {
        PackGround::groups(self)
    }

    fn blast_zone(&self) -> BlastZone {
        let b = &self.stage.bounds;
        BlastZone {
            top: b.top as f32,
            bottom: b.bottom as f32,
            left: b.left as f32,
            right: b.right as f32,
        }
    }

    fn quake_translate(&self, magnitude: u8, ticks: u16) -> Option<(f32, f32)> {
        ssb_pose::quake_translate(self.pack, magnitude, ticks)
    }
}

/// The battle's stage: its map and its objects.
pub struct MatchArena<'a, 'p> {
    pub ground: PackGround<'a, 'p>,
    pub props: StageObjectsPort<'a, 'p>,
}

impl<'a, 'p> Arena for MatchArena<'a, 'p> {
    type Ground = PackGround<'a, 'p>;
    type Props = StageObjectsPort<'a, 'p>;

    fn ground(&self) -> &Self::Ground {
        &self.ground
    }

    fn split(&mut self) -> (&Self::Ground, &mut Self::Props) {
        (&self.ground, &mut self.props)
    }

    fn play_anim(&mut self) {
        let pack = self.ground.pack;
        let _ = self.props.objects.advance(pack);
        if let Some(map) = self.ground.map.as_deref_mut() {
            let _ = map.tick(pack);
        }
    }

    fn floor_group(&self, line: u16) -> Option<u16> {
        self.ground.map.as_ref()?;
        self.ground
            .pack
            .stage_lines(self.ground.stage)
            .find(|l| l.id == line)
            .map(|l| l.yakumono)
    }

    fn board(&mut self, index: usize) {
        if let Some(map) = self.ground.map.as_deref_mut() {
            map.platforms[index].board(self.ground.pack);
        }
    }

    fn tick_stage(
        &mut self,
        stage_ctl: &mut Stage,
        fighters: &mut [&mut Fighter],
        items: &mut dyn StageItems,
        started: bool,
    ) {
        let (pack, stage) = (self.ground.pack, self.ground.stage);
        let mut empty: [MapGroup; 0] = [];
        let groups = self
            .ground
            .map
            .as_deref_mut()
            .map_or(&mut empty[..], |map| map.groups.as_mut_slice());
        // `mpCollisionSetDObjNoID`: a floor's original line id to its
        // collision group.
        let line_group = |line: u16| {
            pack.stage_lines(stage)
                .find(|l| l.id == line)
                .map(|l| l.yakumono as u8)
        };
        stage_ctl.tick(
            fighters,
            TickInput {
                groups,
                objects: &mut self.props,
                items,
                // The groups are being written, so the controller sees the
                // static map; only the Twister queries it, on a static
                // floor.
                map: MapQuery {
                    surfaces: || MapSegments::new(pack, stage),
                    line_group: &line_group,
                },
                started,
            },
        );
    }

    fn arwing_roll(&self) -> Option<f32> {
        self.props.objects.arwing.as_ref().map(|a| a.rotate(1)[2])
    }

    fn place_item_trees(&mut self, items: &ssb_game::item::ItemPool) {
        crate::scene::place_item_trees(self.props.objects, items);
    }
}

impl HasFighter for FighterScene {
    fn fighter(&self) -> &Fighter {
        &self.pose.fighter
    }
    fn fighter_mut(&mut self) -> &mut Fighter {
        &mut self.pose.fighter
    }
    fn rig(&self) -> Option<&ssb_game::camera::CameraRig> {
        Some(&self.camera)
    }
    fn rig_mut(&mut self) -> Option<&mut ssb_game::camera::CameraRig> {
        Some(&mut self.camera)
    }
    fn framing(&self) -> ssb_game::camera::Framing {
        FighterScene::framing(self)
    }
}
