//! Builds the game's kernel module (`psp-kcall/`) into `OUT_DIR/kcall.prx`
//! for `me.rs` to embed (RE-479).
//!
//! - The same toolchain as this build: `CARGO` and `RUSTC` from the
//!   invoking `cargo psp`, `-Z build-std=core`, target `mipsel-sony-psp`.
//! - Its own target directory under `OUT_DIR`: never the outer build's
//!   (whose lock this build's cargo holds).
//! - `prxgen` (installed with `cargo-psp`, which runs it on every PSP
//!   build) turns the ELF into the PRX.

use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let kcall = manifest.join("../psp-kcall");
    for f in ["Cargo.toml", "Cargo.lock", "src/main.rs"] {
        println!("cargo:rerun-if-changed={}", kcall.join(f).display());
    }
    println!("cargo:rerun-if-changed=build.rs");
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("psp") {
        // Host tooling (`cargo check`, IDEs): an empty module; `me.rs`
        // compiles for the PSP only.
        std::fs::write(out.join("kcall.prx"), []).unwrap();
        return;
    }
    let target_dir = out.join("kcall-target");
    let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut cmd = Command::new(cargo);
    cmd.current_dir(&kcall)
        .args(["build", "--release", "--locked", "-Z", "build-std=core"])
        .args(["--target", "mipsel-sony-psp", "--target-dir"])
        .arg(&target_dir)
        // `-Z build-std` on the pinned stable toolchain, as `cargo psp`.
        .env("RUSTC_BOOTSTRAP", "1");
    // The outer build's settings are not the module's.
    for var in [
        "CARGO_TARGET_DIR",
        "CARGO_BUILD_TARGET",
        "CARGO_ENCODED_RUSTFLAGS",
        "RUSTFLAGS",
        "CARGO_BUILD_RUSTFLAGS",
        "RUSTC_WORKSPACE_WRAPPER",
        "CARGO_PROFILE_RELEASE_DEBUG",
        "CARGO_PROFILE_RELEASE_LTO",
        "CARGO_PROFILE_RELEASE_OPT_LEVEL",
    ] {
        cmd.env_remove(var);
    }
    let run = cmd.output().expect("cargo for psp-kcall");
    if !run.status.success() {
        panic!(
            "psp-kcall build failed:\n{}",
            String::from_utf8_lossy(&run.stderr)
        );
    }
    let elf = target_dir.join("mipsel-sony-psp/release/ssb64-psp-kcall");
    check_layout(&std::fs::read(&elf).unwrap());
    let prx = out.join("kcall.prx");
    let gen = Command::new("prxgen")
        .arg(&elf)
        .arg(&prx)
        .output()
        .expect("prxgen (installed with cargo-psp)");
    if !gen.status.success() {
        panic!("prxgen failed:\n{}", String::from_utf8_lossy(&gen.stderr));
    }
}

/// Every loaded segment of the ELF at the same file-offset-to-address
/// distance: `prxgen` makes one segment from the first one's offset, and
/// the loader reads `module_info` (and maps the image) by that distance.
/// A section aligned past the first segment's file offset breaks it, and
/// the system then reads `module_info` from the wrong bytes.
fn check_layout(elf: &[u8]) {
    let half = |at: usize| u16::from_le_bytes(elf[at..at + 2].try_into().unwrap()) as usize;
    let word = |at: usize| u32::from_le_bytes(elf[at..at + 4].try_into().unwrap());
    let (phoff, phentsize, phnum) = (word(0x1c) as usize, half(0x2a), half(0x2c));
    let mut delta = None;
    for i in 0..phnum {
        let ph = phoff + i * phentsize;
        if word(ph) != 1 || word(ph + 16) == 0 {
            continue; // not PT_LOAD, or no file bytes (.bss)
        }
        let d = word(ph + 4).wrapping_sub(word(ph + 8)); // p_offset - p_vaddr
        assert!(
            *delta.get_or_insert(d) == d,
            "psp-kcall: segment {i} at file distance {d:#x}, not {:#x}",
            delta.unwrap()
        );
    }
}
