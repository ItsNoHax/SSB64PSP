//! The first-launch installer: `INSTALL.PBP` beside the game's EBOOT.
//!
//! - The game LoadExecs it when its boot finds no valid `ssb64.pak`
//!   (`ssb_psp_runtime::launch`).
//! - It builds the pack from the user's ROM (`ssb_psp_runtime::installer`,
//!   `ssb_pack`), then quits to the XMB; the next launch of the game boots.
//! - Its own EBOOT: the game's code is not resident during the build, and
//!   `ssb-pack` is not in the game's pinned `.text` (RE-585, RE-588).
//! - Orchestration only: no renderer, no tables, no audio.

#![no_std]
#![no_main]

/// The build's deepest stack is about 90 KB (`install.log` `stack_free`).
const STACK_BYTES: usize = 256 * 1024;

psp::module!("ssb64_psp_installer", 1, 0, STACK_BYTES);

fn psp_main() {
    // A test build takes away the memory a smaller launch lacks (RE-475):
    // `ballast.txt` is the free bytes at this program's start.
    #[cfg(feature = "memory_ballast")]
    if let Some(target) = ssb_psp_runtime::memory::ballast_target() {
        ssb_psp_runtime::memory::hold_ballast(target);
    }
    ssb_psp_runtime::installer::run(ssb_psp_runtime::assets::pack_status());
}
