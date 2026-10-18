//! `dSC1PBonusStageTargetDescs`, US labels in fighter-kind order.
//! Targets use a shared item tree, not the course's placement descriptors.
pub const MODEL: (u32, u32) = (150, 0x10F8);
pub const FIRST_ASSET: u8 = 22;
pub const FIRST_ANIM: u32 = 64;

#[derive(Clone, Copy, Debug)]
pub struct Course {
    pub start: u32,
    pub placements: u32,
    pub scripts: u32,
}
// `dSC1PBonusStageTargetDescs`' labels, generated at build time from the
// user's ROM by `crates/ssb-tablegen` (never committed).
include!(concat!(env!("OUT_DIR"), "/bonus1_courses.rs"));
pub fn kind(file: u32) -> Option<u8> {
    (271..283).contains(&file).then(|| (file - 271) as u8)
}
pub const fn anim(kind: u8, instance: u8) -> u32 {
    FIRST_ANIM + kind as u32 * 10 + instance as u32
}
