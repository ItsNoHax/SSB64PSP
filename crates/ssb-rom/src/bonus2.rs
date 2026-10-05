//! US `sc1pbonusstage.c` platform trees and course Bumper tables.
use crate::{
    pack::{AnimDesc, ObjectDesc, Pack},
    skeleton::{EffectMaterialAnimator, StageAnimator},
};
use alloc::vec::Vec;

pub const FILE: u32 = 136;
pub const FIRST_ANIM: u32 = 184;
pub const BUMPER_ANIM: u32 = 190;
pub const FIRST_BUMPER_ASSET: u8 = 34;
/// graph, joint table, MObj table, material table; the latter two only
/// exist on the unboarded trees. All labels are offsets into file 136.
pub const TREES: [[u32; 4]; 6] = [
    [0x3da8, 0x3e60, 0x3720, 0x3f00],
    [0x45d8, 0x4690, 0x3f70, 0x4730],
    [0x4e08, 0x4ec0, 0x47a0, 0x4f70],
    [0x5520, 0x55d0, 0, 0],
    [0x5b80, 0x5c30, 0, 0],
    [0x61e0, 0x6290, 0, 0],
];
/// map_nodes graph and root-script table in files 137..148.
pub const BUMPERS: [Option<(u32, u32)>; 12] = [
    None,
    Some((0xe160, 0xe350)),
    None,
    Some((0x2910, 0x29c0)),
    None,
    None,
    None,
    None,
    Some((0x3920, 0x3a60)),
    None,
    Some((0x4fe0, 0x5120)),
    Some((0x3fe0, 0x4090)),
];
pub fn kind(file: u32) -> Option<u8> {
    (283..295).contains(&file).then(|| (file - 283) as u8)
}
pub fn object(pack: &Pack<'_>, tree: usize) -> Option<ObjectDesc> {
    (0..pack.object_count())
        .filter_map(|i| pack.object(i))
        .find(|o| (o.source_file, o.source_offset) == (FILE, TREES[tree][0]))
}
#[derive(Clone, Copy, Debug)]
pub struct Floor {
    pub line: u16,
    pub group: u16,
    pub width: f32,
}
pub fn platforms(pack: &Pack<'_>, stage: &crate::pack::StageDesc) -> Vec<Floor> {
    pack.stage_lines(stage)
        .filter(|l| l.kind == crate::pack::line_kind::FLOOR)
        .filter_map(|l| {
            let mut vertices = pack.line_vertices(&l);
            let first = vertices.next()?;
            if first.flags & 0xff != 14 {
                return None;
            }
            let last = vertices.last().unwrap_or(first);
            Some(Floor {
                line: l.id,
                group: l.yakumono,
                width: (i32::from(last.x) - i32::from(first.x)).unsigned_abs() as f32,
            })
        })
        .collect()
}

pub struct Visual {
    pub group: u16,
    pub kind: u8,
    pub boarded: bool,
    pub object: ObjectDesc,
    pub anim: StageAnimator,
    pub materials: EffectMaterialAnimator,
    clip: AnimDesc,
}
impl Visual {
    pub fn new(pack: &Pack<'_>, group: u16, kind: u8) -> Option<Self> {
        let mut this = Self {
            group,
            kind,
            boarded: false,
            object: object(pack, kind as usize)?,
            anim: StageAnimator::new(),
            materials: EffectMaterialAnimator::new(),
            clip: pack.item_anim(FIRST_ANIM + u32::from(kind))?,
        };
        this.restart(pack);
        Some(this)
    }
    fn restart(&mut self, pack: &Pack<'_>) {
        self.anim.start_changed(pack, &self.clip);
        let o = self.object;
        self.materials.start(
            pack,
            (0..o.node_count).flat_map(|i| {
                pack.node(o.first_node + i).into_iter().flat_map(move |n| {
                    pack.mesh(n.mesh).into_iter().flat_map(move |m| {
                        (0..m.prim_count)
                            .filter_map(move |i| pack.prim(m.first_prim + i))
                            .map(|p| p.mat_anim)
                            .filter(|&i| i != u32::MAX)
                    })
                })
            }),
        );
        // `lbCommonPlayTreeDObjsAnim` parses frame zero immediately.
        self.tick(pack).expect("platform initial play");
    }
    pub fn board(&mut self, pack: &Pack<'_>) {
        if self.boarded {
            return;
        }
        self.boarded = true;
        self.object = object(pack, self.kind as usize + 3).expect("boarded platform tree");
        self.clip = pack
            .item_anim(FIRST_ANIM + u32::from(self.kind) + 3)
            .expect("boarded platform clip");
        self.restart(pack);
    }
    pub fn tick(&mut self, pack: &Pack<'_>) -> Result<(), crate::objanim::AnimError> {
        if let Some(script) = pack.anim_script(&self.clip) {
            self.anim.tick(script)?;
        }
        self.materials.tick(pack);
        Ok(())
    }
}
