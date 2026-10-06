//! `mn/mncommon/mnmessage.c`: the unlock message ("A new character has
//! joined!", "New stage!"), and the unlock it applies to the backup.
//!
//! The source runs one task per queued `unlock_messages` entry; the 1P Game
//! queues one at a time ([`super::SceneData::unlock_message`]), so one
//! [`Message`] is one task. The scene's input waits two seconds, then any of
//! A, B or START writes the unlock and ends the scene
//! (`mnMessageFuncRun`). The US build queues no Sound Test message here
//! (`mnMessageStartScene`'s `REGION_JP` branch).
use super::{Backup, Unlock};
use crate::fighter::FighterKind;
use ssb_engine::input::N64Buttons;

/// Ticks before A, B or START may close the message.
pub const INPUT_WAIT: u32 = 120;

/// `mnMessageMakeMessage`'s US `message_pos`, by [`Unlock`].
pub const MESSAGE_POSITIONS: [[f32; 2]; 7] = [
    [85.0, 114.0],
    [35.0, 123.0],
    [58.0, 114.0],
    [44.0, 114.0],
    [48.0, 123.0],
    [56.0, 114.0],
    [54.0, 114.0],
];

/// `mnMessageMakeWallpaper`'s collage position.
pub const WALLPAPER_POSITION: [f32; 2] = [10.0, 10.0];
/// `mnMessageMakeExclaim`'s position.
pub const EXCLAIM_POSITION: [f32; 2] = [140.0, 62.0];
/// `mnMessageTintProcDisplay`: a blue `G_RM_AA_XLU_SURF` rectangle over the
/// collage.
pub const TINT: [u8; 4] = [0x00, 0x00, 0xFF, 0x3F];
/// `mnMessageTintProcDisplay`'s rectangle.
pub const TINT_RECT: [f32; 4] = [10.0, 10.0, 310.0, 230.0];

/// One message task.
#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    /// `sMNMessageUnlockID`.
    pub unlock: Unlock,
    /// `sMNMessageTotalTimeTics`.
    pub total_tics: u32,
    finished: bool,
}

impl Message {
    /// `mnMessageInitVars`, with the queue entry already taken.
    pub fn new(unlock: Unlock) -> Self {
        Self {
            unlock,
            total_tics: 0,
            finished: false,
        }
    }

    /// `mnMessageFuncRun`. Returns `true` on the tick the unlock is written
    /// and the scene ends. The stick checks only reset an unused counter.
    pub fn tick(&mut self, taps: N64Buttons, backup: &mut Backup) -> bool {
        if self.finished {
            return false;
        }
        self.total_tics += 1;
        if self.total_tics >= INPUT_WAIT
            && taps.contains(N64Buttons::A | N64Buttons::B | N64Buttons::START)
        {
            apply_unlock(backup, self.unlock);
            self.finished = true;
        }
        self.finished
    }

    /// `message_pos[message]`.
    pub fn position(&self) -> [f32; 2] {
        MESSAGE_POSITIONS[self.unlock as usize]
    }
}

/// `mnMessageApplyUnlock`: the unlock bit, a newcomer's fighter bit and the
/// Characters menu's selection, then `lbBackupWrite`.
pub fn apply_unlock(backup: &mut Backup, unlock: Unlock) {
    backup.unlock_mask |= unlock.mask();
    let fighter = match unlock {
        Unlock::Luigi => Some(FighterKind::Luigi),
        Unlock::Ness => Some(FighterKind::Ness),
        Unlock::Captain => Some(FighterKind::Captain),
        Unlock::Purin => Some(FighterKind::Purin),
        _ => None,
    };
    if let Some(kind) = fighter {
        backup.fighter_mask |= 1 << kind as u16;
        backup.characters_fkind = kind;
    }
    backup.write();
}
