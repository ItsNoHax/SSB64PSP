//! Original loading-break screen transition assets.
//!
//! `lb/lbtransition.c::dLBTransitionDescs` names these eleven entries in this
//! exact order. The file-local offsets come from the corresponding decompiled
//! `relocData` declarations and are checked against the user's ROM by the
//! `transition_inventory` example.
//!
//! File ids are **not** the contiguous `40..=50` run their declaration order
//! might suggest. Every entry but `camera` is `llLBTransition<Name>FileID`
//! in `reloc_data_symbols.us.txt` in that contiguous range, but the eighth
//! (`camera`) entry's own `llLBTransitionCameraFileID` is `0x33` (51), not
//! 47 -- file 47 (`llLBTransitionPaperAirplaneFileID`) is a twelfth,
//! unregistered file sharing the same segment-`0x1` photo-texture signature
//! (RE-099) but never named by `dLBTransitionDescs` itself, confirmed by
//! `grep` finding no other reference to it anywhere in
//! `refs/ssb-decomp-re/src`. `camera`'s `graph`/`anim_joints` match
//! `llLBTransitionCameraDObjDesc` (`0x3f90`) and
//! `llLBTransitionCameraAnimJoint` (`0x4148`) from the same symbol table,
//! which are file 51's own offsets, not file 47's (RE-247).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransitionAsset {
    pub name: &'static str,
    pub file: u32,
    pub graph: u32,
    pub anim_joints: u32,
    pub frames: u32,
}

pub const ASSETS: [TransitionAsset; 11] = [
    TransitionAsset {
        name: "aeroplane",
        file: 40,
        graph: 0xB3F8,
        anim_joints: 0xB710,
        frames: 64,
    },
    TransitionAsset {
        name: "check",
        file: 41,
        graph: 0x3E80,
        anim_joints: 0x4038,
        frames: 64,
    },
    TransitionAsset {
        name: "gakubuthi",
        file: 42,
        graph: 0x0F98,
        anim_joints: 0x101C,
        frames: 64,
    },
    TransitionAsset {
        name: "kannon",
        file: 43,
        graph: 0x1F00,
        anim_joints: 0x1FB0,
        frames: 64,
    },
    TransitionAsset {
        name: "star",
        file: 44,
        graph: 0x2450,
        anim_joints: 0x24D4,
        frames: 64,
    },
    TransitionAsset {
        name: "sudare1",
        file: 45,
        graph: 0x74A8,
        anim_joints: 0x7660,
        frames: 64,
    },
    TransitionAsset {
        name: "sudare2",
        file: 46,
        graph: 0x3EA0,
        anim_joints: 0x3F50,
        frames: 64,
    },
    TransitionAsset {
        name: "camera",
        file: 51,
        graph: 0x3F90,
        anim_joints: 0x4148,
        frames: 64,
    },
    TransitionAsset {
        name: "block",
        file: 48,
        graph: 0x4E18,
        anim_joints: 0x536C,
        frames: 64,
    },
    TransitionAsset {
        name: "rot_scale",
        file: 49,
        graph: 0x0F98,
        anim_joints: 0x101C,
        frames: 64,
    },
    TransitionAsset {
        name: "curtain",
        file: 50,
        graph: 0x7AE0,
        anim_joints: 0x7C98,
        frames: 72,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    /// File ids per `reloc_data_symbols.us.txt`'s `llLBTransition<Name>
    /// FileID` constants, in `dLBTransitionDescs` order. Only `camera`
    /// (index 7) breaks the otherwise-contiguous `40..=50` run: its real
    /// file is 51, not 47 (RE-247).
    const EXPECTED_FILES: [u32; 11] = [40, 41, 42, 43, 44, 45, 46, 51, 48, 49, 50];

    #[test]
    fn descriptor_order_is_the_original_file_run() {
        assert_eq!(ASSETS.len(), 11);
        for (index, asset) in ASSETS.iter().enumerate() {
            assert_eq!(asset.file, EXPECTED_FILES[index]);
            assert_ne!(asset.graph, 0);
            assert_ne!(asset.anim_joints, 0);
            assert_eq!(asset.frames, if index == 10 { 72 } else { 64 });
        }
    }
}
