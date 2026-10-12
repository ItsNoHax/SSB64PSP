//! The PSP's hardware performance counters (RE-481), for the `profile_hw`
//! feature: cycles, stalls, cache misses and bus accesses, read per span
//! by [`crate::profile`].
//!
//! The counters sit at `0xBC400000` (PSPSDK's `pspDebugProfilerRegs`), in
//! kernel space, which a user-mode game cannot load from. ARK's
//! `KUBridge` library (exported by its SystemControl) offers
//! `kuKernelPeekw` and `kuKernelPokew`, a bare load and store in a system
//! call; [`read`] reads the [`READ`] registers through them, one call
//! each. `kuKernelCall` would copy all twenty in one call, but it costs
//! ~42,500 cycles on a PSP-2000 (it runs ~1,070 COP0 instructions, cache
//! maintenance), and a few thousand a frame took PSPLink and USB down
//! (RE-481). The counters are global: they count every thread and
//! interrupt handler, and the kernel's own work in the reads.

/// The registers in PSPSDK's order. `0` is the enable register; the rest
/// are free-running 32-bit counts.
pub const NAMES: [&str; REGS] = [
    "enable", "systemck", "cpuck", "internal", "memory", "copz", "vfpu", "sleep", "bus", "ucld", "ucst", "cld", "cst", "imiss",
    "dmiss", "dwb", "cop0", "fpu", "vfpuins", "local",
];

/// Registers read.
pub const REGS: usize = 20;

const BASE: usize = 0xBC40_0000;

/// The registers [`read`] fills; the others stay 0.
pub const READ: [usize; 10] = [2, 3, 4, 5, 8, 9, 10, 13, 14, 15];

/// Turns the profiler on if it is off and reads the [`READ`] registers into
/// `dst`.
pub fn read(dst: &mut [u32; REGS]) {
    let base = BASE as u32;
    // SAFETY: `kuKernelPeekw`/`kuKernelPokew` load and store one word in
    // kernel mode; every address is a profiler register.
    unsafe {
        if ku_kernel_peekw(base) == 0 {
            ku_kernel_pokew(base, 1);
        }
        for &i in READ.iter() {
            dst[i] = ku_kernel_peekw(base + 4 * i as u32);
        }
    }
}

// The imports of `KUBridge`'s `kuKernelPeekw` (NID 0x7A50075E) and
// `kuKernelPokew` (NID 0x0E73A39D), checked against ARK's SystemControl
// export table on the PSP-2000: `lw v0, 0(a0)` and `sw a1, 0(a0)`. The
// stub tables mirror rust-psp's `psp_extern!`, which is private to that
// crate.

#[repr(C)]
struct Stub {
    lib: &'static psp::sys::SceStubLibraryEntry,
    nid: &'static u32,
}

#[link_section = ".rodata.sceResident.KUBridge"]
static KUBRIDGE_NAME: [u8; 9] = *b"KUBridge\0";

#[link_section = ".rodata.sceNid.KUBridge"]
static KUBRIDGE_NID_START: () = ();

#[link_section = ".sceStub.text.KUBridge"]
static KUBRIDGE_STUB_START: () = ();

#[link_section = ".lib.stub.entry.KUBridge"]
static KUBRIDGE_STUB: psp::sys::SceStubLibraryEntry = psp::sys::SceStubLibraryEntry {
    name: &KUBRIDGE_NAME[0],
    version: [0, 0],
    flags: 0x4001,
    len: 5,
    v_stub_count: 0,
    // Patched by cargo-psp, as for rust-psp's own imports.
    stub_count: 0,
    nid_table: &KUBRIDGE_NID_START as *const () as *const _,
    stub_table: &KUBRIDGE_STUB_START as *const () as *const _,
};

#[link_section = ".rodata.sceNid.KUBridge.kuKernelPeekw"]
static KU_KERNEL_PEEKW_NID: u32 = 0x7A50_075E;

#[link_section = ".rodata.sceNid.KUBridge.kuKernelPokew"]
static KU_KERNEL_POKEW_NID: u32 = 0x0E73_A39D;

// SAFETY: read-only addresses, replaced by the loader with `jr ra; syscall`.
unsafe impl Sync for Stub {}

#[link_section = ".sceStub.text.KUBridge.kuKernelPeekw"]
#[no_mangle]
static __ssb_kuKernelPeekw_stub: Stub = Stub { lib: &KUBRIDGE_STUB, nid: &KU_KERNEL_PEEKW_NID };

#[link_section = ".sceStub.text.KUBridge.kuKernelPokew"]
#[no_mangle]
static __ssb_kuKernelPokew_stub: Stub = Stub { lib: &KUBRIDGE_STUB, nid: &KU_KERNEL_POKEW_NID };

unsafe fn ku_kernel_peekw(addr: u32) -> u32 {
    extern "C" {
        fn __ssb_kuKernelPeekw_stub(addr: u32) -> u32;
    }
    __ssb_kuKernelPeekw_stub(addr)
}

unsafe fn ku_kernel_pokew(addr: u32, value: u32) {
    extern "C" {
        fn __ssb_kuKernelPokew_stub(addr: u32, value: u32);
    }
    __ssb_kuKernelPokew_stub(addr, value)
}
