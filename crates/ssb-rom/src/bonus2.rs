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
// `dSC1PBonusStagePlatformDescs` and `...BoardedPlatformDescs`: graph, joint
// table, MObj table, material table (the latter two only on the unboarded
// trees; all labels are offsets into file 136), and `dSC1PBonusStageBumperDescs`:
// map_nodes graph and root-script table in files 137..148. Generated at build
// time from the user's ROM by `crates/ssb-tablegen` (never committed).
include!(concat!(env!("OUT_DIR"), "/bonus2_trees.rs"));
include!(concat!(env!("OUT_DIR"), "/bonus2_bumpers.rs"));
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
        let mut this = core::mem::MaybeUninit::<Self>::uninit();
        // SAFETY: `write_new` initialises the slot when it returns true.
        unsafe { Self::write_new(this.as_mut_ptr(), pack, group, kind).then(|| this.assume_init()) }
    }

    /// [`Self::new`] pushed onto `into`, built in the vector's spare
    /// capacity: by value the 27 KB tree is a stack temporary copied into
    /// the vector (RE-469). `None` (and `into` unchanged) when the pack
    /// lacks the tree or its clip.
    pub fn push_new(into: &mut Vec<Self>, pack: &Pack<'_>, group: u16, kind: u8) -> Option<()> {
        into.reserve(1);
        let slot = into.spare_capacity_mut().first_mut()?;
        // SAFETY: the slot is valid and aligned; once `write_new` has
        // initialised it, it becomes the vector's next element.
        unsafe {
            if !Self::write_new(slot.as_mut_ptr(), pack, group, kind) {
                return None;
            }
            into.set_len(into.len() + 1);
        }
        Some(())
    }

    /// [`Self::new`] written straight into `dst`. Returns false, with
    /// nothing written, when the pack lacks the tree or its clip.
    ///
    /// # Safety
    ///
    /// `dst` must be valid for writes and aligned. Its old contents are
    /// overwritten without being dropped.
    #[inline(never)]
    pub unsafe fn write_new(dst: *mut Self, pack: &Pack<'_>, group: u16, kind: u8) -> bool {
        use core::ptr::addr_of_mut;
        ssb_engine::all_fields!(Visual {
            group,
            kind,
            boarded,
            object,
            anim,
            materials,
            clip
        });
        let Some(object) = object(pack, kind as usize) else {
            return false;
        };
        let Some(clip) = pack.item_anim(FIRST_ANIM + u32::from(kind)) else {
            return false;
        };
        // SAFETY: `dst` is valid for writes (the caller's contract); each
        // field is written once before `restart` borrows the whole.
        unsafe {
            addr_of_mut!((*dst).group).write(group);
            addr_of_mut!((*dst).kind).write(kind);
            addr_of_mut!((*dst).boarded).write(false);
            addr_of_mut!((*dst).object).write(object);
            StageAnimator::write_new(addr_of_mut!((*dst).anim));
            addr_of_mut!((*dst).materials).write(EffectMaterialAnimator::new());
            addr_of_mut!((*dst).clip).write(clip);
            (*dst).restart(pack);
        }
        true
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
