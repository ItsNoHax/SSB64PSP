//! A stage's moving collision groups (yakumono) and their animation: one
//! copy for the PSP's `StageMap` and the host replays (D-052).

extern crate alloc;

use alloc::vec::Vec;
use ssb_engine::math::Vec3;
use ssb_game::map::{GroupStatus, MapGroup};
use ssb_rom::pack::{AnimDesc, Pack, StageDesc};
use ssb_rom::skeleton::StageAnimator;

/// The stage's groups where `gcPlayAnimAll` left them.
pub struct StageGroups {
    pub animator: StageAnimator,
    pub groups: Vec<MapGroup>,
    anim: Option<AnimDesc>,
    /// The geometry layer's first node, group 0's.
    pub first_node: u32,
    previous_flags: Vec<u16>,
}

impl StageGroups {
    /// Pack stage `index`'s groups at their creation pose.
    pub fn new(pack: &Pack<'_>, index: u32, stage: &StageDesc) -> Self {
        let anim = pack.stage_anim(index);
        let mut animator = StageAnimator::new();
        if let Some(anim) = anim {
            animator.start(pack, &anim);
        }
        let first_node = pack
            .object(stage.layers[1])
            .map_or(u32::MAX, |o| o.first_node);
        let count = pack
            .stage_lines(stage)
            .map(|l| l.yakumono as usize + 1)
            .max()
            .unwrap_or(0);
        let mut groups = alloc::vec![MapGroup::default(); count];
        for (i, group) in groups.iter_mut().enumerate() {
            if let Some(node) = first_node.checked_add(i as u32).and_then(|n| pack.node(n)) {
                group.translate = Vec3::new(
                    node.rest_translate[0],
                    node.rest_translate[1],
                    node.rest_translate[2],
                );
            }
            group.animated = (0..animator.joint_count()).any(|j| {
                animator
                    .joint(j)
                    .is_some_and(|(n, _)| n == first_node.saturating_add(i as u32))
            });
        }
        // `grDisplayMakeGeometryLayer`'s `gcPlayAnimAll` has posed the
        // groups before their first tick, so the first speed
        // `mpCollisionPlayYakumonoAnim` measures starts there (RE-524).
        if let Some(created) = anim.and_then(|a| StageAnimator::creation_pose(pack, &a).ok()) {
            for (i, group) in groups.iter_mut().enumerate() {
                if let Some(pose) = created.node_pose(first_node.saturating_add(i as u32)) {
                    group.translate =
                        Vec3::new(pose.translate[0], pose.translate[1], pose.translate[2]);
                }
            }
        }
        Self {
            animator,
            groups,
            anim,
            first_node,
            previous_flags: alloc::vec![0; count],
        }
    }

    /// One `gcPlayAnimAll` on the groups (`mpCollisionPlayYakumonoAnim`).
    pub fn tick(&mut self, pack: &Pack<'_>) -> Result<(), ssb_rom::objanim::AnimError> {
        let Some(anim) = self.anim else {
            return Ok(());
        };
        let Some(script) = pack.anim_script(&anim) else {
            return Ok(());
        };
        let first = self.first_node;
        for (i, flag) in self.previous_flags.iter_mut().enumerate() {
            *flag = self.animator.flags(first.saturating_add(i as u32));
        }
        let groups = &self.groups;
        self.animator.tick_nodes(script, |node| {
            node.checked_sub(first)
                .and_then(|i| groups.get(i as usize))
                .is_none_or(|g| !matches!(g.status, GroupStatus::On | GroupStatus::Off))
        })?;
        for i in 0..self.animator.joint_count() {
            let Some((node, pose)) = self.animator.joint(i) else {
                continue;
            };
            let Some(group) = node
                .checked_sub(first)
                .and_then(|i| self.groups.get_mut(i as usize))
            else {
                continue;
            };
            if !matches!(group.status, GroupStatus::On | GroupStatus::Off) {
                let flags = self.animator.flags(node);
                let old_flags = self.previous_flags[(node - first) as usize];
                if old_flags == 0 && flags != 0 {
                    group.status = GroupStatus::Hidden;
                } else if old_flags != 0 && flags == 0 {
                    group.status = GroupStatus::Show;
                }
                group.set_position(Vec3::new(
                    pose.translate[0],
                    pose.translate[1],
                    pose.translate[2],
                ));
            }
        }
        Ok(())
    }
}
