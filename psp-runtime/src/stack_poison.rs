//! Signalling-NaN stack poison: a detector for speculated FPU traps.
//!
//! LLVM may compute a float compare or division before the branch that
//! guards it, on a slot the guarded path never wrote: the payload of a
//! `None` `Option`, or a value only one arm assigns. The result is thrown
//! away, so every host and the N64 behave, but a PSP thread traps on an
//! invalid operation (D-050), and a signalling NaN in that stack garbage
//! raises one. Whether a build traps then depends on what earlier calls
//! left on the stack: the opening's Jungle fight trapped this way on a
//! PSP-2000 in `ssb_game::map::move_ground_pushed`, through a compare on
//! `CliffEdge.facing` while it was a float.
//!
//! With the `stack_poison` feature, [`poison`] fills the stack below its
//! caller with a signalling NaN (`0x7FA0_0000`, which traps `c.olt.s` on a
//! PSP), so the code the caller runs next reads that instead of whatever
//! was there: a speculated float operation on an unwritten slot traps on
//! the first frame that reaches it. The game calls it before each
//! fighter's interrupt and physics passes and before the stage, item and
//! weapon processes. Without the feature it compiles to nothing. Diagnostic
//! only: never in a release build.

/// Words of stack below the caller that [`poison`] overwrites: 16 KB of
/// the main thread's 256 KB.
#[cfg(feature = "stack_poison")]
const WORDS: usize = 4096;

/// A signalling NaN on the PSP's FPU.
#[cfg(feature = "stack_poison")]
const SNAN: u32 = 0x7FA0_0000;

/// Fills the stack below the caller with [`SNAN`] (`stack_poison` only).
#[inline(always)]
pub fn poison() {
    #[cfg(feature = "stack_poison")]
    fill();
}

#[cfg(feature = "stack_poison")]
#[inline(never)]
fn fill() {
    let mut a = [SNAN; WORDS];
    core::hint::black_box(&mut a);
}
