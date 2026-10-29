//! Data-cache placement of the game loop's stack (RE-583).
//!
//! The PSP's data cache is 16 KB two-way with 64-byte lines: an address's
//! set is its offset in an 8 KB way. Every frame's calls run on the stack
//! below the frame of the function that never returns (`psp-game`'s
//! `run`), so where that frame ends in the way decides which sets each
//! frame's locals share with `.bss` (pinned, RE-580) and the heap. Called
//! directly, it ends wherever the caller's stack and its own size put it:
//! a change to its locals moved every scene's stack sets and measured up to
//! 0.4 ms a frame on a PSP-2000 (RE-583). [`enter`] calls it with its
//! stack pointer lowered so that its frame ends at one offset of the way in
//! every build.

/// The data cache's way: an address's set is its offset in this many bytes.
pub const DCACHE_WAY: usize = 0x2000;

/// Bytes kept between the caller's stack pointer and the entered frame: the
/// four argument words a callee may store above its stack pointer, rounded
/// up to a cache line.
const GAP: usize = 64;

/// Calls `body` with its stack pointer lowered so that, after `body`'s
/// prologue, it sits `way_offset` bytes into [`DCACHE_WAY`]. At most
/// `DCACHE_WAY + GAP` bytes of the thread's stack go unused.
///
/// `body`'s frame size is read from its prologue (`addiu $sp, $sp, -n`, or
/// `lui`/`ori` into `$at` then `subu`/`addu $sp, $sp, $at`, the forms
/// LLVM emits for MIPS); any other prologue panics. Check the result in
/// `body` with [`check`].
///
/// # Safety
///
/// `way_offset` is a multiple of 8 below [`DCACHE_WAY`], and the thread's
/// stack has `DCACHE_WAY + GAP` bytes to spare below the caller.
#[inline(never)]
pub unsafe fn enter(body: unsafe fn() -> !, way_offset: usize) -> ! {
    let frame = frame_size(body as usize).expect("stack_pin: unrecognised prologue");
    let top = stack_pointer() - GAP;
    let mut body_sp = ((top - frame) & !(DCACHE_WAY - 1)) | way_offset;
    if body_sp > top - frame {
        body_sp -= DCACHE_WAY;
    }
    // SAFETY: `body` never returns, so nothing above the new stack pointer
    // is read again but the caller's frames, which stay in place. `$25`
    // holds the target as MIPS position-independent calls expect; the
    // delay slot switches the stack.
    core::arch::asm!(
        ".set push",
        ".set noreorder",
        ".set noat",
        "jr $25",
        "move $sp, {sp}",
        ".set pop",
        sp = in(reg) body_sp + frame,
        in("$25") body as usize,
        options(noreturn),
    );
}

/// Panics unless the caller's stack pointer is `way_offset` bytes into
/// [`DCACHE_WAY`]: called first in a body [`enter`] started, it proves the
/// prologue read.
#[inline(always)]
pub fn check(way_offset: usize) {
    // `u32` formats with the code the game already has.
    let sp = stack_pointer() as u32;
    assert!(
        sp as usize % DCACHE_WAY == way_offset,
        "stack_pin: sp {sp:#x}, not {:#x} in the way",
        way_offset as u32
    );
}

/// The stack pointer of the function this is inlined into.
#[inline(always)]
pub fn stack_pointer() -> usize {
    let sp: usize;
    // SAFETY: reads a register.
    unsafe {
        core::arch::asm!(
            ".set push",
            ".set noat",
            "move {}, $sp",
            ".set pop",
            out(reg) sp,
            options(nomem, nostack, preserves_flags),
        )
    };
    sp
}

/// The bytes the prologue at `f` subtracts from `$sp`, from its first 16
/// instructions.
fn frame_size(f: usize) -> Option<usize> {
    // `$at` (`$1`) as the last `lui`/`ori`/`addiu` set it.
    let mut at: Option<u32> = None;
    for i in 0..16 {
        // SAFETY: code is readable memory; 16 words of a function.
        let w = unsafe { core::ptr::read_volatile((f as *const u32).add(i)) };
        let imm = w & 0xffff;
        match w >> 16 {
            // addiu $sp, $sp, imm
            0x27bd if imm & 0x8000 != 0 => return Some((0x1_0000 - imm) as usize),
            0x3c01 => at = Some(imm << 16),     // lui $at, imm
            0x3421 => at = at.map(|v| v | imm), // ori $at, $at, imm
            0x2421 => at = at.map(|v| v.wrapping_add(imm as i16 as i32 as u32)), // addiu $at, $at, imm
            _ => match w {
                0x03a1_e823 => return at.map(|v| v as usize), // subu $sp, $sp, $at
                0x03a1_e821 => return at.map(|v| v.wrapping_neg() as usize), // addu $sp, $sp, $at
                _ => {}
            },
        }
    }
    None
}
