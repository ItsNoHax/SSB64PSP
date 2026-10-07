//! The module's main thread, with a stack the application chooses.
//!
//! `psp::module!` starts `psp_main` on a thread with a fixed 256 KiB stack.
//! The game's frame needs more than that, and nothing on a PSP grows a
//! stack: the deepest frames write below the stack's base into whatever the
//! allocator put there, and the thread manager kills the thread with a
//! Breakpoint exception once its stack-bottom word is overwritten. PPSSPP
//! never checks the bound, so the overflow is invisible there (RE-469).
//!
//! [`module_with_stack!`](crate::module_with_stack) is `psp::module!` with
//! the main thread's stack size as a parameter. Starting the game on a
//! second thread instead would lose the working directory: `psp::module!`'s
//! entry sets it with `sceIoChdir` for its own thread only, and the pack is
//! opened by a relative path.

use psp::sys;

/// `psp::module!` (rust-psp `a89142b`) with the main thread's stack size,
/// in bytes, as a fourth argument. Everything else is that macro's.
#[macro_export]
macro_rules! module_with_stack {
    ($name:expr, $version_major:expr, $version_minor:expr, $stack_bytes:expr) => {
        #[doc(hidden)]
        mod __psp_module {
            #[no_mangle]
            #[link_section = ".rodata.sceModuleInfo"]
            #[used]
            static MODULE_INFO: ::psp::Align16<::psp::sys::SceModuleInfo> =
                ::psp::Align16(::psp::sys::SceModuleInfo {
                    mod_attribute: 0,
                    mod_version: [$version_major, $version_minor],
                    mod_name: ::psp::sys::SceModuleInfo::name($name),
                    terminal: 0,
                    gp_value: unsafe { &_gp },
                    stub_top: unsafe { &__lib_stub_top },
                    stub_end: unsafe { &__lib_stub_bottom },
                    ent_top: unsafe { &__lib_ent_top },
                    ent_end: unsafe { &__lib_ent_bottom },
                });

            extern "C" {
                static _gp: u8;
                static __lib_ent_bottom: u8;
                static __lib_ent_top: u8;
                static __lib_stub_bottom: u8;
                static __lib_stub_top: u8;
            }

            #[no_mangle]
            #[link_section = ".lib.ent"]
            #[used]
            static LIB_ENT: ::psp::sys::SceLibraryEntry = ::psp::sys::SceLibraryEntry {
                name: core::ptr::null(),
                version: ($version_major, $version_minor),
                attribute: ::psp::sys::SceLibAttr::SCE_LIB_IS_SYSLIB,
                entry_len: 4,
                var_count: 1,
                func_count: 1,
                entry_table: &LIB_ENT_TABLE,
            };

            #[no_mangle]
            #[link_section = ".rodata.sceResident"]
            #[used]
            static LIB_ENT_TABLE: ::psp::sys::SceLibraryEntryTable =
                ::psp::sys::SceLibraryEntryTable {
                    module_start_nid: 0xd632acdb, // module_start
                    module_info_nid: 0xf01d73a7,  // SceModuleInfo
                    module_start: module_start,
                    module_info: &MODULE_INFO.0,
                };

            use core::ffi::c_void;

            #[no_mangle]
            extern "C" fn module_start(argc_bytes: usize, argv: *mut c_void) -> isize {
                extern "C" fn main_thread(argc: usize, argv: *mut c_void) -> i32 {
                    ::psp::_start!(super::psp_main, argc, argv)
                }

                unsafe {
                    let id = ::psp::sys::sceKernelCreateThread(
                        b"main_thread\0".as_ptr(),
                        main_thread,
                        // `psp::module!`'s priority.
                        32,
                        ($stack_bytes) as i32,
                        ::psp::sys::ThreadAttributes::USER | ::psp::sys::ThreadAttributes::VFPU,
                        core::ptr::null_mut(),
                    );

                    ::psp::sys::sceKernelStartThread(id, argc_bytes, argv);
                }

                0
            }
        }
    };
}

/// Bytes of the current thread's stack never written since it started:
/// the kernel fills a new stack with `0xFF` and this counts the untouched
/// bytes above its base (`sceKernelGetThreadStackFreeSize`). PPSSPP
/// implements the same scan.
pub fn stack_free_bytes() -> usize {
    let free = unsafe { sys::sceKernelGetThreadStackFreeSize(sys::SceUid(0)) };
    free.max(0) as usize
}
