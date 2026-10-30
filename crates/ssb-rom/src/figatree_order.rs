//! The order a fighter figatree's joint scripts bind in.
//!
//! Shared by `ssb-rom` (`fighter`, re-exported there) and `ssb-tablegen`,
//! which compiles this file in (`#[path]`) to find the joint whose script
//! decides an animation's end (`END_BLOCKS`): `ssb-rom`'s build script runs
//! `ssb-tablegen`, so neither can depend on the other.

use alloc::vec::Vec;

/// One `FTHiddenPart`: a joint `ftMainUpdateHiddenPartID` inserts while a
/// motion's `anim_desc` sets the matching bit (bit 31 - index).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HiddenPart {
    /// `FTStruct::joints` index the inserted `DObj` fills.
    pub root_joint_id: u32,
    pub parent_joint_id: u32,
    /// 0 appends it as the parent's last child; 3 interposes it.
    pub joint_kind: u32,
}

/// One `DObj` a fighter figatree script binds to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeJoint {
    /// `TransN`, `XRotN` or `YRotN`: no packed node.
    Runtime(u32),
    /// A `DObjDesc` index of the fighter's model.
    Desc(u32),
}

impl TreeJoint {
    /// Its `FTStruct::joints` index: `nFTPartsJointCommonStart` (4) on for
    /// a descriptor.
    pub fn joint_id(self) -> u32 {
        match self {
            TreeJoint::Runtime(id) => id,
            TreeJoint::Desc(desc) => desc + 4,
        }
    }
}

/// The `DObj`s `lbCommonAddFighterPartsFigatree` visits, in script order.
///
/// `lbCommonSetupFighterPartsDObjs` creates each mask-enabled descriptor as
/// the last child of the latest node one level up. `ftMainSetStatus` then
/// inserts each hidden part `anim_desc` enables (bit `31 - i`), in index
/// order, and the figatree walks `TopN`'s child pre-order (RE-432).
/// `depths` holds every descriptor's `id & 0xFFF`; `hidden` holds the
/// fighter's parts. Returns `None` for a part whose parent does not exist.
pub fn figatree_order(
    depths: &[u32],
    mask: u64,
    hidden: &[HiddenPart],
    anim_desc: u32,
) -> Option<Vec<TreeJoint>> {
    // Node 0 is `TopN`. Children lists in sibling order.
    let mut kinds = alloc::vec![TreeJoint::Runtime(0)];
    let mut children: Vec<Vec<usize>> = alloc::vec![Vec::new()];
    let mut joints: [Option<usize>; 64] = [None; 64];
    joints[0] = Some(0);
    let mut latest = [0usize; 19];
    for (i, &depth) in depths.iter().enumerate().take(64) {
        if mask >> i & 1 == 0 {
            continue;
        }
        let depth = depth as usize;
        let parent = if depth == 0 {
            0
        } else {
            *latest.get(depth - 1)?
        };
        let node = kinds.len();
        kinds.push(TreeJoint::Desc(i as u32));
        children.push(Vec::new());
        children[parent].push(node);
        *latest.get_mut(depth)? = node;
        joints[i + 4] = Some(node);
    }
    for i in 0..27 {
        if anim_desc >> (31 - i) & 1 == 0 {
            continue;
        }
        let part = hidden.get(i)?;
        let parent = (*joints.get(part.parent_joint_id as usize)?)?;
        let node = kinds.len();
        kinds.push(match part.root_joint_id {
            id @ 0..=3 => TreeJoint::Runtime(id),
            id => TreeJoint::Desc(id - 4),
        });
        children.push(Vec::new());
        match part.joint_kind {
            0 => children[parent].push(node),
            1 => children[parent].insert(0, node),
            2 => {
                let at = children[parent].len().min(1);
                children[parent].insert(at, node);
            }
            _ => children[node] = core::mem::take(&mut children[parent]),
        }
        if part.joint_kind == 3 {
            children[parent].push(node);
        }
        *joints.get_mut(part.root_joint_id as usize)? = Some(node);
    }
    // `lbCommonGetTreeDObjNextFromRoot` from `TopN->child`: its subtree only.
    let mut order = Vec::new();
    let mut stack = alloc::vec![*children[0].first()?];
    while let Some(node) = stack.pop() {
        order.push(kinds[node]);
        stack.extend(children[node].iter().rev());
    }
    Some(order)
}
