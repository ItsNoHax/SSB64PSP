//! A fighter's skeleton and its clip tick: moved verbatim from the PSP
//! runtime's `scene.rs`, which re-exports them.

extern crate alloc;

use ssb_game::fighter::FighterKind;
use ssb_game::status::AnyStatus;
use ssb_rom::pack::Pack;

/// A fighter's skeleton as `ftManagerMakeFighter` leaves it: TopN scaled
/// by `attr->size` and the `animlock` parts frozen (`ftParamSetAnimLocks`)
/// until a flagged motion's figatree clears them (RE-509).
pub fn fighter_skeleton(kind: FighterKind, size: f32) -> ssb_rom::skeleton::Skeleton {
    let mut skeleton = ssb_rom::skeleton::Skeleton::new();
    skeleton.locks = fighter_locks(kind, size);
    skeleton
}

/// [`fighter_skeleton`] written straight into `dst`: by value it is a
/// 26 KB stack temporary copied into place (RE-469).
///
/// # Safety
///
/// `dst` must be valid for writes and aligned.
pub unsafe fn write_fighter_skeleton(
    dst: *mut ssb_rom::skeleton::Skeleton,
    kind: FighterKind,
    size: f32,
) {
    // SAFETY: the caller's contract; `write_new` initialises every field.
    unsafe {
        ssb_rom::skeleton::Skeleton::write_new(dst);
        (*dst).locks = fighter_locks(kind, size);
    }
}

/// [`fighter_skeleton`] written over `skeleton`, with no stack temporary.
pub fn reset_fighter_skeleton(
    skeleton: &mut ssb_rom::skeleton::Skeleton,
    kind: FighterKind,
    size: f32,
) {
    skeleton.reset();
    skeleton.locks = fighter_locks(kind, size);
}

/// [`fighter_skeleton`] in its own heap box, built there.
pub fn boxed_fighter_skeleton(
    kind: FighterKind,
    size: f32,
) -> alloc::boxed::Box<ssb_rom::skeleton::Skeleton> {
    let mut skeleton = ssb_rom::skeleton::Skeleton::new_boxed();
    skeleton.locks = fighter_locks(kind, size);
    skeleton
}

/// [`fighter_skeleton`]'s locks: the `animlock` parts and the root scale.
pub fn fighter_locks(kind: FighterKind, size: f32) -> ssb_rom::skeleton::AnimLocks {
    ssb_rom::skeleton::AnimLocks {
        mask: ssb_game::modelpart::animlock_mask(kind),
        flagged: false,
        root_scale: [size; 3],
        // Copied from the fighter's queried joints before each display
        // (RE-509).
        queried: 0,
        queried_lead: false,
    }
}

/// Advances a fighter's skeleton pose for `status`. Shared by every fighter
/// scene that owns a skeleton: restarted on a status *entry* rather than
/// every tick, since an animation carries its own clock and re-seeding it
/// each frame would freeze every fighter on frame zero. `ftMainSetStatus`
/// restarts the figatree on every entry, even into the same status, and
/// starts it at the entry's `frame_begin`. A looping animation
/// is left to loop; a finite one that has run out holds its last pose, which
/// is what the original does when a status outlives its animation.
#[allow(clippy::too_many_arguments)]
pub fn tick_skeleton_animation(
    pack: &Pack<'_>,
    kind: u32,
    first_node: u32,
    status: &ssb_game::status::StatusState,
    speed: f32,
    skeleton: &mut ssb_rom::skeleton::Skeleton,
    started: &mut Option<(AnyStatus, u32)>,
    restarts_seen: &mut u8,
) -> Option<ssb_rom::figatree::JointPose> {
    let entry = status.entry;
    // A figatree restarted outside `ftMainSetStatus` (Kirby's inhale
    // wiggle, `lbCommonAddFighterPartsFigatree`): the play that consumed
    // `AOBJ_ANIM_CHANGED` posed frame 0 without advancing.
    let clip_restarted = *restarts_seen != status.clip_restarts;
    *restarts_seen = status.clip_restarts;
    // The clip starts on the status's current frame: `ftMainSetStatus`
    // parses `frame_begin`, and a setter's `ftMainPlayAnimEventsAll` or the
    // frame's own advance has moved it on before this first parse, which
    // poses without advancing (RE-466).
    // The status clock, not the figatree's `anim_frame`, which a short
    // figatree has already looped or ended (RE-474).
    let frame_begin = status.clock;
    // A setter's `ftMainPlayAnimEventsAll` after `ftMainSetStatus`'s own
    // first parse: the clip stands one play past `anim_frame_begin`.
    let setter_played = status.clock > status.anim_frame_begin;
    // `motion_id` -2: `ftParamUpdateAnimKeys` replays the installed clip's
    // held values instead of advancing it (RE-510). The frame such a status
    // starts on, its setter ran after that frame's `ftMainPlayAnim`, which
    // still advanced the clip under the old motion.
    let mut held = status.clip_paused;
    let status = status.status;
    // The pack row is the fighter's; the slot is the status's. Common
    // statuses use the shared slots, which resolve per fighter through the
    // status -> motion pairing (`ssb_rom::anim::SLOT_APPEAL`).
    let slot = status.anim_slot() as u32;
    let mut restarted = false;
    if *started != Some((status, entry)) {
        *started = Some((status, entry));
        held = false;
        restarted = !status.keeps_motion();
        // A status with no motion of its own (`CatchWait`, `CaptureWait`)
        // keeps playing the previous one; its slot is that status's slot.
        if status.keeps_motion() {
            // Yoshi Bomb's fall keeps the start clip at speed zero.
            skeleton.speed = speed;
        } else {
            if let Some(anim) = pack.fighter_anim(kind, slot) {
                skeleton.start(
                    pack,
                    &anim,
                    if setter_played {
                        frame_begin - speed
                    } else {
                        frame_begin
                    },
                    speed,
                );
                let fighter_kind = u8::try_from(kind).ok().and_then(FighterKind::from_ordinal);
                skeleton.lead_xrotn =
                    fighter_kind.is_some_and(|k| ssb_game::motion::leads_with_xrotn(k, status));
                skeleton.lead_transn =
                    fighter_kind.is_some_and(|k| ssb_game::motion::uses_transn(k, status));
                // `fp->is_use_animlocks`: only a figatree's install moves it
                // (RE-509).
                skeleton.locks.flagged =
                    fighter_kind.is_some_and(|k| ssb_game::motion::uses_animlocks(k, status));
            }
        }
    }
    // After the entry: a status set and a wiggle in one frame (the
    // captive acts first) leave the wiggle's frame 0 to this parse.
    let mut wiggle_root = None;
    if clip_restarted && !status.keeps_motion() {
        if let Some(anim) = pack.fighter_anim(kind, slot) {
            // The restart keeps the current pose for the root's step; only
            // `ftMainSetStatus` zeroes TransN.
            wiggle_root = Some(skeleton.pose(0).map(|p| ssb_rom::figatree::JointPose {
                translate: if restarted { [0.0; 3] } else { p.translate },
                ..*p
            }));
            skeleton.start(pack, &anim, 0.0, speed);
            held = false;
        }
    }
    // A rate the status changes mid-clip (`gcSetAnimSpeed`) applies from
    // the next play.
    skeleton.speed = speed;
    // The slot is read back rather than remembered, so a status whose
    // animation the pack lacks -- Kirby has no aerial jump -- simply keeps
    // the pose it had.
    // `ftMainSetStatus` zeroes TransN before the new clip's first parse, so
    // its first step is the clip's own first translation.
    let root_before = if let Some(root) = wiggle_root {
        root
    } else if restarted && setter_played {
        // `ftMainSetStatus` parsed `frame_begin` (TransN zeroed first, then
        // posed); the setter's play reads that pose as `anim_vel` and
        // advances, so the first step is from the clip's own first pose
        // (RE-468: Samus's roll starts 23.27 along, not 22.53).
        if let Some(anim) = pack.fighter_anim(kind, slot) {
            if let Some(script) = pack.anim_script(&anim) {
                let _ =
                    skeleton.tick_scaled(script, pack.fighter_translate_scales(kind), first_node);
            }
        }
        skeleton.pose(0).copied()
    } else if restarted {
        skeleton.pose(0).map(|p| ssb_rom::figatree::JointPose {
            translate: [0.0; 3],
            ..*p
        })
    } else {
        skeleton.pose(0).copied()
    };
    if let Some(anim) = pack.fighter_anim(kind, slot) {
        if let Some(script) = pack.anim_script(&anim) {
            if held {
                skeleton.hold_scaled(script, pack.fighter_translate_scales(kind), first_node);
            } else {
                let _ =
                    skeleton.tick_scaled(script, pack.fighter_translate_scales(kind), first_node);
            }
        }
    }
    root_before
}
