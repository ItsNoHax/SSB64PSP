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
/// `dSC1PBonusStageTargetDescs`' `[start, dobjdesc, anim_joint]` labels
/// (`TableId::Bonus1Courses`).
static COURSE_LABELS: ssb_tables::Table<[[u32; 3]; 12]> =
    ssb_tables::Table::new(ssb_tables::TableId::Bonus1Courses);
/// `dSC1PBonusStageTargetDescs`' labels by fighter kind, read from the
/// pack's `TABLES` section.
pub static COURSES: ssb_tables::Decoded<[Course; 12]> = ssb_tables::Decoded::new(|| {
    core::array::from_fn(|k| {
        let [start, placements, scripts] = COURSE_LABELS[k];
        Course {
            start,
            placements,
            scripts,
        }
    })
});
pub fn kind(file: u32) -> Option<u8> {
    (271..283).contains(&file).then(|| (file - 271) as u8)
}
pub const fn anim(kind: u8, instance: u8) -> u32 {
    FIRST_ANIM + kind as u32 * 10 + instance as u32
}
