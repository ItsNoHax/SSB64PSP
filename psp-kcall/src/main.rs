//! The game's kernel-mode PRX (PLAN "Audio cost", RE-479): runs one
//! function of the game in kernel mode, then unloads itself.
//!
//! - `module_info` attribute 0x1000: the system loads the module into the
//!   kernel partition and runs `module_start` on a kernel-mode thread.
//! - `sceKernelStartModule(id, 4, &func, ..)`: [`kcall_start`] calls
//!   `func` (an `extern "C" fn() -> i32` in the game) on that thread and
//!   returns 1 (`SCE_KERNEL_NO_RESIDENT`), so the system unloads the module
//!   again. One load and start per call; no exports, no imports, no state.
//! - Interrupts stay as the thread has them: `func` may make system calls.
//! - Built and embedded by `psp-runtime/build.rs`; loaded from memory by
//!   `psp-runtime/src/me.rs` (`bring_up`).

#![no_std]
#![no_main]
#![feature(asm_experimental_arch)]

use core::ffi::c_void;

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

/// `SceModuleInfo` (pspsdk `pspmoduleinfo.h`).
#[repr(C, align(16))]
struct ModuleInfo {
    attribute: u16,
    version: [u8; 2],
    name: [u8; 28],
    gp: *const u8,
    ent_top: *const u8,
    ent_end: *const u8,
    stub_top: *const u8,
    stub_end: *const u8,
}

// SAFETY: read-only data the loader relocates; never written at run time.
unsafe impl Sync for ModuleInfo {}

/// `SceLibraryEntryTable` (pspsdk `pspmoduleexport.h`).
#[repr(C)]
struct LibEntry {
    name: *const u8,
    version: [u8; 2],
    attribute: u16,
    entry_len: u8,
    var_count: u8,
    func_count: u16,
    table: *const c_void,
}

// SAFETY: as `ModuleInfo`.
unsafe impl Sync for LibEntry {}

/// The system library's exports: NIDs, then the function, then the variable.
#[repr(C)]
struct SysExports {
    nids: [u32; 2],
    start: unsafe extern "C" fn(usize, *const u32) -> i32,
    info: *const ModuleInfo,
}

// SAFETY: as `ModuleInfo`.
unsafe impl Sync for SysExports {}

/// `PSP_MODULE_KERNEL`.
const ATTR_KERNEL: u16 = 0x1000;
/// `SCE_LIB_IS_SYSLIB`.
const LIB_SYSLIB: u16 = 0x8000;
/// The NIDs of `module_start` and `module_info`.
const NID_MODULE_START: u32 = 0xd632_acdb;
const NID_MODULE_INFO: u32 = 0xf01d_73a7;
/// `module_start`'s "unload me again" result.
const NO_RESIDENT: i32 = 1;

extern "C" {
    static _gp: u8;
    static __lib_ent_top: u8;
    static __lib_ent_bottom: u8;
    static __lib_stub_top: u8;
    static __lib_stub_bottom: u8;
}

const fn name(s: &[u8]) -> [u8; 28] {
    let mut n = [0; 28];
    let mut i = 0;
    while i < s.len() {
        n[i] = s[i];
        i += 1;
    }
    n
}

#[no_mangle]
#[used]
#[link_section = ".rodata.sceModuleInfo"]
static module_info: ModuleInfo = ModuleInfo {
    attribute: ATTR_KERNEL,
    version: [1, 0],
    name: name(b"ssb64_kcall"),
    // SAFETY: linker-defined addresses, never read through.
    gp: unsafe { &_gp },
    ent_top: unsafe { &__lib_ent_top },
    ent_end: unsafe { &__lib_ent_bottom },
    stub_top: unsafe { &__lib_stub_top },
    stub_end: unsafe { &__lib_stub_bottom },
};

#[used]
#[link_section = ".lib.ent"]
static LIB_ENT: LibEntry = LibEntry {
    name: core::ptr::null(),
    version: [0, 0],
    attribute: LIB_SYSLIB,
    entry_len: 4,
    var_count: 1,
    func_count: 1,
    table: &SYS_EXPORTS as *const SysExports as *const c_void,
};

#[used]
#[link_section = ".rodata.sceResident"]
static SYS_EXPORTS: SysExports = SysExports {
    nids: [NID_MODULE_START, NID_MODULE_INFO],
    start: module_start,
    info: &module_info,
};

extern "C" {
    /// [`kcall_start`] behind a 16-byte aligned jump (below).
    fn module_start(args: usize, argp: *const u32) -> i32;
}

// `.text` 16-byte aligned: its file offset then keeps the same distance
// to its address as the 16-byte aligned sections after it, which
// `prxgen` (one segment from the first one's offset) and the loader
// (`module_info` from that segment's `p_paddr`) assume. Unaligned, the
// system reads `module_info` 12 bytes off.
core::arch::global_asm!(
    r#"
        .section .text.module_start, "ax", @progbits
        .p2align 4
        .set push
        .set noreorder
    .global module_start
    .type module_start, @function
    module_start:
        j kcall_start
        nop
        .set pop
    "#
);

/// Kernel mode: calls the function whose address is `argp`'s first word.
#[no_mangle]
extern "C" fn kcall_start(args: usize, argp: *const u32) -> i32 {
    if args >= 4 && !argp.is_null() {
        // SAFETY: the system copied the caller's 4 argument bytes, the
        // address of an `extern "C" fn() -> i32` in the game.
        let func: extern "C" fn() -> i32 =
            unsafe { core::mem::transmute(argp.read_unaligned() as usize) };
        func();
    }
    NO_RESIDENT
}

// The bounds of the (empty) import and export tables, as rust-psp
// defines them for user modules.
core::arch::global_asm!(
    r#"
        .section .lib.ent.top, "a", @progbits
        .align 2
        .word 0
    .global __lib_ent_top
    __lib_ent_top:
        .section .lib.ent.btm, "a", @progbits
        .align 2
    .global __lib_ent_bottom
    __lib_ent_bottom:
        .word 0
        .section .lib.stub.top, "a", @progbits
        .align 2
        .word 0
    .global __lib_stub_top
    __lib_stub_top:
        .section .lib.stub.btm, "a", @progbits
        .align 2
    .global __lib_stub_bottom
    __lib_stub_bottom:
        .word 0
    "#
);
