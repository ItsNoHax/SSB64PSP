//! The battle's music: `gMPCollisionBGMDefault`/`gMPCollisionBGMCurrent`
//! (`src/mp/mpcollision.c`) and the Star's and Hammer's item music
//! (`ftParam*ItemMusic`, `src/ft/ftparam.c`).
//!
//! `ftParamTryUpdateItemMusic` walks every fighter. Its callers here hold
//! one fighter (or none), so they [`request_update`] and the match runs
//! [`flush_update`] over its fighters after the fighter processes of the
//! same frame. The music it picks is the same: it depends only on the
//! fighters' held Hammer and Star time, which every caller has already
//! changed.

use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use crate::fighter::Fighter;
use crate::sound::{self, id::*};

static BGM_DEFAULT: AtomicU32 = AtomicU32::new(0);
static BGM_CURRENT: AtomicU32 = AtomicU32::new(0);
static UPDATE_PENDING: AtomicBool = AtomicBool::new(false);

/// `ITSTAR_BGM_DURATION`.
pub const ITSTAR_BGM_DURATION: i32 = 10;
/// `ITHAMMER_BGM_DURATION`.
pub const ITHAMMER_BGM_DURATION: i32 = 20;
/// `ITSTAR_WARN_BEGIN_FRAME` (`ITSTAR_INVINCIBLE_TIME - 480`): the Star's
/// music ends here, 8 seconds before its invincibility.
pub const ITSTAR_WARN_BEGIN_FRAME: u16 = crate::item::utility::STAR_INVINCIBLE_TIME - 480;

/// `gMPCollisionBGMDefault`: the stage's music, or what a scene set.
pub fn bgm_default() -> u32 {
    BGM_DEFAULT.load(Ordering::Relaxed)
}

/// `gMPCollisionBGMCurrent`: what sequence player 0 plays.
pub fn bgm_current() -> u32 {
    BGM_CURRENT.load(Ordering::Relaxed)
}

/// Sets `gMPCollisionBGMDefault` (Inishie's hurry music, Final
/// Destination's, Training's).
pub fn set_bgm_default(bgm_id: u32) {
    BGM_DEFAULT.store(bgm_id, Ordering::Relaxed);
}

/// `mpCollisionSetPlayBGM`: plays the stage's `MPGroundData::bgm_id`.
pub fn set_play_bgm(ground_bgm_id: u32) {
    set_bgm_default(ground_bgm_id);
    sound::play_bgm(0, ground_bgm_id);
    BGM_CURRENT.store(ground_bgm_id, Ordering::Relaxed);
}

/// `mpCollisionSetBGM`: records the stage's music without playing it.
pub fn set_bgm(ground_bgm_id: u32) {
    set_bgm_default(ground_bgm_id);
    BGM_CURRENT.store(ground_bgm_id, Ordering::Relaxed);
}

/// `ftParamGetItemMusicLength`. The source swaps the two lengths: the Star
/// returns `ITHAMMER_BGM_DURATION` and the Hammer `ITSTAR_BGM_DURATION`,
/// so the Star's music wins over the Hammer's. Kept.
pub fn item_music_length(bgm_id: u32) -> i32 {
    if bgm_id == nSYAudioBGMStar {
        ITHAMMER_BGM_DURATION
    } else if bgm_id == nSYAudioBGMHammer {
        ITSTAR_BGM_DURATION
    } else {
        0
    }
}

/// `ftParamTryPlayItemMusic`.
pub fn try_play_item_music(bgm_id: u32) {
    if item_music_length(bgm_id) >= item_music_length(bgm_current()) {
        sound::play_bgm(0, bgm_id);
        BGM_CURRENT.store(bgm_id, Ordering::Relaxed);
    }
}

/// A `ftParamTryUpdateItemMusic` call from a site that does not hold every
/// fighter; [`flush_update`] runs it.
pub fn request_update() {
    UPDATE_PENDING.store(true, Ordering::Relaxed);
}

/// Runs a [`request_update`]ed `ftParamTryUpdateItemMusic`, if any.
pub fn flush_update<'a>(fighters: impl IntoIterator<Item = &'a Fighter>) {
    if UPDATE_PENDING.swap(false, Ordering::Relaxed) {
        try_update_item_music(fighters);
    }
}

/// `ftParamTryUpdateItemMusic`: the longest item music any fighter earns
/// (a held Hammer, a Star before its warning frame), else the default.
pub fn try_update_item_music<'a>(fighters: impl IntoIterator<Item = &'a Fighter>) {
    let mut bgm_play = bgm_default();
    let mut length = item_music_length(bgm_play);
    for f in fighters {
        let mut bgm_id = bgm_default();
        if f.items.held.is_some_and(|h| {
            h.kind == crate::item::ItemKind::Equipment(crate::item::equipment::Kind::Hammer)
        }) {
            bgm_id = nSYAudioBGMHammer;
        }
        if f.star_invincible_frames > ITSTAR_WARN_BEGIN_FRAME {
            bgm_id = nSYAudioBGMStar;
        }
        let length_new = item_music_length(bgm_id);
        if length < length_new {
            length = length_new;
            bgm_play = bgm_id;
        }
    }
    if bgm_play != bgm_current() {
        sound::play_bgm(0, bgm_play);
        BGM_CURRENT.store(bgm_play, Ordering::Relaxed);
    }
}

/// Clears the state between tests.
#[cfg(test)]
pub fn reset() {
    BGM_DEFAULT.store(0, Ordering::Relaxed);
    BGM_CURRENT.store(0, Ordering::Relaxed);
    UPDATE_PENDING.store(false, Ordering::Relaxed);
}
