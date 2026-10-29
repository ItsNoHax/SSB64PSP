//! `gSCManagerBackupData` across boots: `scManagerRunLoop`'s
//! `lbBackupIsSramValid` at startup and every `lbBackupWrite` after it,
//! through `ssb_psp_runtime::savedata` (D-045).
//!
//! The portable game counts its writes in `Backup::writes`; once a frame
//! the host saves when that count has moved. Golden captures boot with the
//! defaults and never touch the memory stick, so a save left by a play
//! session cannot change a capture, except the two save scenes, which
//! load and write like a normal boot.

use ssb_capture::GameScene;
use ssb_game::backup::{Backup, Loaded};
use ssb_psp_runtime::savedata;

/// Where the backup is saved and how far the saves have caught up.
pub(crate) struct Save {
    /// `None`: this run neither loads nor saves.
    path: Option<&'static str>,
    /// `Backup::writes` at the last save.
    saved: u32,
    /// A save taken before [`FIRST_WRITE_TICK`], written then.
    held: Option<alloc::boxed::Box<Backup>>,
}

/// The first tick that writes the memory stick. The boot's own save (its
/// boot count) is taken in tick 1 but written here, after the first
/// picture: the write costs about 35 ms on a PSP-2000, 0.3 s when it
/// creates the file. The same bytes, one tick later.
pub(crate) const FIRST_WRITE_TICK: u64 = 2;

/// Whether a run with this capture scene uses the memory stick.
fn persists(capture: Option<GameScene>) -> bool {
    matches!(capture, None | Some(GameScene::SaveUnlock | GameScene::SavePlayers))
}

/// The boot's backup: the save beside the pack (or the defaults), then
/// `mnTitleStartScene`'s boot count, since the run starts on the title.
pub(crate) fn boot(pack_path: Option<&'static str>, capture: Option<GameScene>) -> (Backup, Save) {
    let path = pack_path.filter(|_| persists(capture)).map(savedata::path_for);
    let (mut backup, loaded) = match path {
        Some(p) => savedata::load(p),
        None => (Backup::default(), Loaded::First),
    };
    if let Some(p) = path {
        log(&alloc::format!(
            "save: {} {} boot={} unlock={:#04x} fighters={:#06x}\n",
            p.trim_end_matches('\0'),
            match loaded {
                Loaded::First => "loaded",
                Loaded::Second => "restored from its second copy",
                Loaded::Defaults => "missing or invalid, defaults",
            },
            backup.boot,
            backup.unlock_mask,
            backup.fighter_mask,
        ));
    }
    backup.count_boot();
    (
        backup,
        Save {
            path,
            saved: 0,
            held: None,
        },
    )
}

/// `lbBackupWrite`'s memory-stick half: saves `backup` if the game wrote it
/// since the last save. Before [`FIRST_WRITE_TICK`] the save is held, and
/// written first thing at that tick.
pub(crate) fn persist(save: &mut Save, backup: &Backup, tick: u64) {
    let Some(path) = save.path else { return };
    if tick >= FIRST_WRITE_TICK {
        if let Some(held) = save.held.take() {
            write(path, &held);
        }
    }
    if backup.writes == save.saved {
        return;
    }
    save.saved = backup.writes;
    if tick < FIRST_WRITE_TICK {
        save.held = Some(alloc::boxed::Box::new(backup.clone()));
        return;
    }
    write(path, backup);
}

fn write(path: &'static str, backup: &Backup) {
    let line = match savedata::save(path, backup) {
        Ok(()) => alloc::format!(
            "save: wrote {} boot={} unlock={:#04x} fighters={:#06x}\n",
            path.trim_end_matches('\0'),
            backup.boot,
            backup.unlock_mask,
            backup.fighter_mask,
        ),
        Err(e) => alloc::format!("save: write failed {:#x}\n", e.0),
    };
    log(&line);
}

fn log(line: &str) {
    unsafe {
        psp::sys::sceIoWrite(
            psp::sys::sceKernelStdout(),
            line.as_ptr() as *const core::ffi::c_void,
            line.len(),
        );
    }
}
