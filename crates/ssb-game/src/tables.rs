//! The game's [`ssb_tables::Hot`] values: tables a hot path (per tick, per
//! fighter) reads, decoded once from the pack's `TABLES` section (RE-589).

/// Decodes every hot value from the installed tables.
///
/// # Safety
///
/// Call once, right after `ssb_tables::install` and before any game code
/// runs: no reference to one of these values may be live or made during
/// the call, on any thread ([`ssb_tables::Hot::fill`]).
pub unsafe fn fill() {
    // SAFETY: the caller's contract.
    unsafe {
        crate::hurtbox::DAMAGE_COLLS.fill();
        crate::modelpart::HIDDEN_PARTS.fill();
        crate::physics::MARIO.fill();
        crate::colanim::EVENTS.fill();
        crate::colanim::DESCS.fill();
        crate::computer::scripts::INPUT_SCRIPTS.fill();
        crate::computer::scripts::ATTACKS.fill();
        crate::motion::fill();
    }
}
