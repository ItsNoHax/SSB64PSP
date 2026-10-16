//! Board the Platforms for the host replays: a course's map groups played
//! the way `psp-runtime`'s `StageMap` plays them, its surfaces as
//! `MapSegments` gives them, and the course's fighter at the bonus spawn,
//! ticked in `training_step`'s order (`bonus2_ride`).

#![allow(dead_code)]

use ssb_engine::input::{ControllerState, N64Buttons};
use ssb_game::fighter::{Fighter, FighterKind};
use ssb_game::map::{GroupStatus, MapGroup};
use ssb_game::weapon::{MapSurface, MapSurfaceKind, SurfaceTopology};
use ssb_rom::pack::{AnimDesc, Pack, StageDesc};
use ssb_rom::skeleton::StageAnimator;

/// `nMPMapObjKind1PGamePlayer`, the bonus stages' spawn.
const PLAYER_SPAWN: u16 = 0x21;

/// One Board the Platforms course: its map groups and fighter.
pub struct Course<'p> {
    pub pack: &'p Pack<'p>,
    pub stage: StageDesc,
    pub animator: StageAnimator,
    pub groups: Vec<MapGroup>,
    anim: Option<AnimDesc>,
    first_node: u32,
    previous_flags: Vec<u16>,
    surfaces: Vec<(MapSurface, u16)>,
    pub fighter: Fighter,
    jump_was_held: bool,
}

impl<'p> Course<'p> {
    /// `kind`'s course with its static platforms switched on
    /// (`sc1PBonusStageInitPlatforms`) and its fighter standing or falling
    /// at the spawn, as `enter_bonus` leaves them.
    pub fn new(pack: &'p Pack<'p>, kind: FighterKind) -> Self {
        let index = pack
            .stage_of_file(283 + kind as u32)
            .expect("Bonus 2 course");
        let stage = pack.stage(index).expect("course stage");
        // `StageMap::new`.
        let anim = pack.stage_anim(index);
        let mut animator = StageAnimator::new();
        if let Some(anim) = anim {
            animator.start(pack, &anim);
        }
        let first_node = pack
            .object(stage.layers[1])
            .map_or(u32::MAX, |o| o.first_node);
        let count = pack
            .stage_lines(&stage)
            .map(|l| l.yakumono as usize + 1)
            .max()
            .unwrap_or(0);
        let mut groups = vec![MapGroup::default(); count];
        for (i, group) in groups.iter_mut().enumerate() {
            if let Some(node) = pack.node(first_node + i as u32) {
                group.translate = ssb_engine::math::Vec3::new(
                    node.rest_translate[0],
                    node.rest_translate[1],
                    node.rest_translate[2],
                );
            }
            group.animated = (0..animator.joint_count()).any(|j| {
                animator
                    .joint(j)
                    .is_some_and(|(n, _)| n == first_node + i as u32)
            });
        }
        if let Some(created) = anim.and_then(|a| StageAnimator::creation_pose(pack, &a).ok()) {
            for (i, group) in groups.iter_mut().enumerate() {
                if let Some(pose) = created.node_pose(first_node + i as u32) {
                    group.translate = ssb_engine::math::Vec3::new(
                        pose.translate[0],
                        pose.translate[1],
                        pose.translate[2],
                    );
                }
            }
        }
        // `enter_bonus`: the platforms with no animation are switched on.
        for floor in ssb_rom::bonus2::platforms(pack, &stage) {
            let g = &mut groups[floor.group as usize];
            if !g.animated {
                g.status = GroupStatus::On;
            }
        }
        let surfaces = decode(pack, &stage);
        let floors: Vec<_> = pack_floors(pack, &stage);
        let fighter = bonus_fighter(pack, &stage, kind, &floors);
        Course {
            pack,
            stage,
            animator,
            groups,
            anim,
            first_node,
            previous_flags: vec![0; count],
            surfaces,
            fighter,
            jump_was_held: false,
        }
    }

    /// `MapSegments::with_groups`.
    pub fn segments(&self) -> impl Iterator<Item = MapSurface> + '_ {
        self.surfaces.iter().filter_map(|&(surface, yakumono)| {
            let group = self.groups.get(usize::from(yakumono));
            group.is_none_or(|g| g.exists()).then(|| MapSurface {
                motion: group.and_then(|g| g.motion()),
                ..surface
            })
        })
    }

    /// `StageMap::tick`.
    pub fn tick_map(&mut self) {
        let Some(anim) = self.anim else { return };
        let script = self.pack.anim_script(&anim).expect("course script");
        let first = self.first_node;
        for (i, flag) in self.previous_flags.iter_mut().enumerate() {
            *flag = self.animator.flags(first + i as u32);
        }
        let groups = &self.groups;
        self.animator
            .tick_nodes(script, |node| {
                node.checked_sub(first)
                    .and_then(|i| groups.get(i as usize))
                    .is_none_or(|g| !matches!(g.status, GroupStatus::On | GroupStatus::Off))
            })
            .expect("course animation");
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
                let old = self.previous_flags[(node - first) as usize];
                if old == 0 && flags != 0 {
                    group.status = GroupStatus::Hidden;
                } else if old != 0 && flags == 0 {
                    group.status = GroupStatus::Show;
                }
                group.set_position(ssb_engine::math::Vec3::new(
                    pose.translate[0],
                    pose.translate[1],
                    pose.translate[2],
                ));
            }
        }
    }

    /// One battle tick in `training_step`'s order: the ground's animation,
    /// the fighter's interrupt, then its map physics.
    pub fn tick(&mut self, pad: ControllerState) {
        self.tick_map();
        let jump_held = pad.buttons.0
            & (N64Buttons::C_UP | N64Buttons::C_DOWN | N64Buttons::C_LEFT | N64Buttons::C_RIGHT)
            != 0;
        let tapped = jump_held && !self.jump_was_held;
        let released = !jump_held && self.jump_was_held;
        self.jump_was_held = jump_held;
        let mut f = std::mem::replace(&mut self.fighter, Fighter::new(FighterKind::Mario, 0, 0));
        f.set_input(pad, tapped, released);
        let surfaces = || self.segments().collect::<Vec<_>>();
        f.tick_interrupt(&surfaces);
        f.tick_physics_map_before_slope(&surfaces);
        self.fighter = f;
    }

    /// The group of the line the fighter stands on (`floor_line_id`'s
    /// yakumono).
    pub fn floor_group(&self) -> Option<u16> {
        let line = self.fighter.floor?.line;
        self.pack
            .stage_lines(&self.stage)
            .find(|l| l.id == line)
            .map(|l| l.yakumono)
    }

    /// One frame in the N64 trace's `Y` and `T` lines (`bonus2` capture
    /// scripts), for a line-by-line diff.
    pub fn trace(&self, frame: u32) -> String {
        use std::fmt::Write;
        let mut s = format!("F {frame}\n");
        for (i, g) in self.groups.iter().enumerate() {
            if g.animated || g.status != GroupStatus::None {
                let _ = writeln!(
                    s,
                    "Y {i} {:.3} {:.3} {:.3}",
                    g.translate.x, g.translate.y, g.translate.z
                );
            }
        }
        let f = &self.fighter;
        let _ = writeln!(
            s,
            "T 0 {} {} {} {} {:.4} {:.4} {:.4} {:.4} {:.4} {} {:04x}",
            f.kind as u32,
            f.status.status.id(),
            u8::from(!f.is_grounded()),
            f.facing.sign() as i32,
            f.pos.x,
            f.pos.y,
            f.physics.vel_air.x,
            f.physics.vel_air.y,
            f.physics.vel_ground.x,
            f.floor.map_or(-1, |s| i32::from(s.line)),
            f.floor.map_or(0, |s| s.flags),
        );
        s
    }
}

/// `Decoder`: every surface in table order with its line's group.
fn decode(pack: &Pack<'_>, stage: &StageDesc) -> Vec<(MapSurface, u16)> {
    use ssb_rom::pack::line_kind;
    let mut out = Vec::new();
    for line in pack.stage_lines(stage) {
        let kind = match line.kind {
            line_kind::FLOOR => MapSurfaceKind::Floor,
            line_kind::CEILING => MapSurfaceKind::Ceiling,
            line_kind::RIGHT_WALL => MapSurfaceKind::RightWall,
            line_kind::LEFT_WALL => MapSurfaceKind::LeftWall,
            _ => continue,
        };
        let points: Vec<_> = pack.line_vertices(&line).collect();
        for (n, pair) in points.windows(2).enumerate() {
            out.push((
                MapSurface {
                    kind,
                    segment: ssb_game::collision::Segment {
                        x1: pair[0].x,
                        y1: pair[0].y,
                        x2: pair[1].x,
                        y2: pair[1].y,
                        flags: pair[0].flags,
                    },
                    topology: Some(SurfaceTopology {
                        line: line.id,
                        point: n as u16,
                        segments: line.vertex_count - 1,
                        vertex1: pair[0].vertex_id,
                        vertex2: pair[1].vertex_id,
                    }),
                    motion: None,
                },
                line.yakumono,
            ));
        }
    }
    out
}

/// `FloorSegments`: the static floors a fighter's entry projects onto.
fn pack_floors(pack: &Pack<'_>, stage: &StageDesc) -> Vec<(u16, ssb_game::collision::Segment)> {
    decode(pack, stage)
        .into_iter()
        .filter(|(s, _)| s.kind == MapSurfaceKind::Floor)
        .map(|(s, _)| (s.topology.unwrap().line, s.segment))
        .collect()
}

/// The bonus fighter at `nMPMapObjKind1PGamePlayer`, as `enter_bonus`
/// leaves it: `set_wait_or_fall` and facing the stage centre.
fn bonus_fighter(
    pack: &Pack<'_>,
    stage: &StageDesc,
    kind: FighterKind,
    floors: &[(u16, ssb_game::collision::Segment)],
) -> Fighter {
    let mut f = super::dream_land::training_fighter_at(pack, stage, kind, 0, PLAYER_SPAWN, floors);
    f.facing = if f.pos.x >= 0.0 {
        ssb_game::fighter::Facing::Left
    } else {
        ssb_game::fighter::Facing::Right
    };
    f
}

/// A pad holding `buttons` and the stick.
pub fn pad(buttons: u16, stick_x: i8, stick_y: i8) -> ControllerState {
    ControllerState {
        buttons: N64Buttons(buttons),
        stick_x,
        stick_y,
        connected: true,
    }
}
