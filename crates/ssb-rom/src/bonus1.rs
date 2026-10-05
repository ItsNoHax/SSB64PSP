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
pub const COURSES: [Course; 12] = [
    Course {
        start: 0x1EB0,
        placements: 0x2150,
        scripts: 0x2360,
    },
    Course {
        start: 0x2068,
        placements: 0x24B0,
        scripts: 0x26C0,
    },
    Course {
        start: 0x1F20,
        placements: 0x2250,
        scripts: 0x2460,
    },
    Course {
        start: 0x1868,
        placements: 0x1B30,
        scripts: 0x1D40,
    },
    Course {
        start: 0x1BA0,
        placements: 0x2020,
        scripts: 0x2230,
    },
    Course {
        start: 0x2378,
        placements: 0x2770,
        scripts: 0x2980,
    },
    Course {
        start: 0x2D68,
        placements: 0x3290,
        scripts: 0x34A0,
    },
    Course {
        start: 0x1888,
        placements: 0x1B70,
        scripts: 0x1D80,
    },
    Course {
        start: 0x2150,
        placements: 0x2510,
        scripts: 0x2720,
    },
    Course {
        start: 0x2658,
        placements: 0x2A70,
        scripts: 0x2C80,
    },
    Course {
        start: 0x1FF8,
        placements: 0x23A0,
        scripts: 0x25B0,
    },
    Course {
        start: 0x2940,
        placements: 0x2E60,
        scripts: 0x3070,
    },
];
pub fn kind(file: u32) -> Option<u8> {
    (271..283).contains(&file).then(|| (file - 271) as u8)
}
pub const fn anim(kind: u8, instance: u8) -> u32 {
    FIRST_ANIM + kind as u32 * 10 + instance as u32
}
