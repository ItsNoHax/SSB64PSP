//! Yoshi's Island: the three clouds that sink and evaporate (`gryoster.c`).

use super::{standing_group, MapQuery, StageAnim, StageObj, StageObjects};
use crate::fighter::Fighter;
use crate::map::{GroupStatus, MapGroup};
use ssb_engine::math::Vec3;

/// `dGRYosterCloudLineIDs`: each cloud's collision group.
pub const CLOUD_GROUPS: [u8; 3] = [1, 2, 3];

/// `grYosterCloudStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloudStatus {
    Solid,
    Evaporate,
}

/// `GRYosterCloud`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cloud {
    /// The cloud GObj's translation; only Y moves.
    pub pos: Vec3,
    pub altitude: f32,
    pub pressure: f32,
    pub status: CloudStatus,
    pub anim: Option<CloudStatus>,
    pub is_line_active: bool,
    pub pressure_timer: i8,
    pub evaporate_wait: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Yoster {
    pub clouds: [Cloud; 3],
    /// The vapor this frame's evaporations made, for [`super::Stage::flush_effects`].
    pub fx: crate::wpeffect::Emit,
}

/// `grYosterUpdateCloudSolid`'s vapor offset from the cloud's root.
pub const VAPOR_OFFSET: Vec3 = Vec3::new(-750.0, -350.0, 0.0);

fn group(groups: &mut [MapGroup], id: u8) -> Option<&mut MapGroup> {
    groups.get_mut(id as usize)
}

impl Yoster {
    /// `grYosterInitAll` @ 0x801089F4. The clouds start at their groups'
    /// positions; each group is switched on.
    pub fn new(groups: &mut [MapGroup], objects: &mut dyn StageObjects) -> Self {
        let clouds = core::array::from_fn(|i| {
            let id = CLOUD_GROUPS[i];
            let pos = group(groups, id).map_or(Vec3::ZERO, |g| g.translate);
            objects.set_translate(StageObj::Cloud(i as u8), pos);
            if let Some(g) = group(groups, id) {
                g.status = GroupStatus::On;
            }
            Cloud {
                pos,
                altitude: pos.y,
                pressure: 0.0,
                status: CloudStatus::Solid,
                anim: Some(CloudStatus::Solid),
                is_line_active: false,
                pressure_timer: -1,
                evaporate_wait: 0,
            }
        });
        // `gcAddAnimJointAll(llGRYosterMap_1E0_AnimJoint)` per cloud is not
        // played: its scripts write only scale, and every cloud `DObj` has
        // only a `nGCMatrixKindTra` matrix (RE-365). The material
        // animations are added by the first tick.
        Yoster {
            clouds,
            fx: crate::wpeffect::Emit::default(),
        }
    }

    /// `grYosterCheckFighterCloudStand` @ 0x801085A8.
    fn stood_on<F>(fighters: &[&mut Fighter], map: &MapQuery<'_, F>, id: u8) -> bool {
        fighters.iter().any(|f| standing_group(f, map) == Some(id))
    }

    /// `grYosterProcUpdate` @ 0x80108960.
    pub fn tick<F>(
        &mut self,
        fighters: &[&mut Fighter],
        groups: &mut [MapGroup],
        objects: &mut dyn StageObjects,
        map: &MapQuery<'_, F>,
    ) {
        for (i, cloud) in self.clouds.iter_mut().enumerate() {
            let id = CLOUD_GROUPS[i];
            match cloud.status {
                CloudStatus::Solid => {
                    // `grYosterUpdateCloudSolid` @ 0x80108634.
                    if objects.mat_anim_idle(StageObj::Cloud(i as u8)) {
                        if !cloud.is_line_active {
                            if let Some(g) = group(groups, id) {
                                g.status = GroupStatus::On;
                            }
                            cloud.is_line_active = true;
                        }
                        if cloud.pressure_timer == 0 {
                            cloud.status = CloudStatus::Evaporate;
                            cloud.anim = Some(CloudStatus::Evaporate);
                            cloud.evaporate_wait = 180;
                            // `grYosterCloudVaporMakeEffect` below the cloud
                            // as it stands, before this frame's sink.
                            self.fx.push(crate::wpeffect::WeaponEffect::CloudVapor(
                                cloud.pos + VAPOR_OFFSET,
                            ));
                        } else {
                            if Self::stood_on(fighters, map, id) {
                                if cloud.pressure_timer == -1 {
                                    cloud.pressure_timer = 120;
                                }
                                cloud.pressure = (cloud.pressure + 5.0).min(180.0);
                            } else {
                                cloud.pressure_timer = -1;
                                cloud.pressure = (cloud.pressure - 5.0).max(0.0);
                            }
                            if cloud.pressure_timer > 0 {
                                cloud.pressure_timer -= 1;
                            }
                        }
                    }
                    cloud.pos.y = cloud.altitude - cloud.pressure;
                    objects.set_translate_y(StageObj::Cloud(i as u8), cloud.pos.y);
                    if let Some(g) = group(groups, id) {
                        g.set_position(cloud.pos);
                    }
                }
                CloudStatus::Evaporate => {
                    // `grYosterUpdateCloudEvaporate` @ 0x80108814.
                    if cloud.is_line_active {
                        if let Some(g) = group(groups, id) {
                            g.status = GroupStatus::Off;
                        }
                        cloud.is_line_active = false;
                    }
                    if cloud.evaporate_wait == 0 {
                        cloud.status = CloudStatus::Solid;
                        cloud.anim = Some(CloudStatus::Solid);
                        cloud.pressure_timer = -1;
                        cloud.pressure = 0.0;
                    } else {
                        cloud.evaporate_wait -= 1;
                    }
                }
            }
            // `grYosterUpdateCloudAnim` @ 0x80108890.
            match cloud.anim.take() {
                Some(CloudStatus::Solid) => objects.play(StageAnim::CloudSolid(i as u8)),
                Some(CloudStatus::Evaporate) => objects.play(StageAnim::CloudEvaporate(i as u8)),
                None => {}
            }
        }
    }
}
