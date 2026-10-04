//! `ftParamSetThrowParams` and `ftCommonThrownProcStatus`: ownership of a
//! thrown fighter's attacks, independent of its catch/capture link.

use crate::fighter::{Fighter, FighterKind};
use crate::status::AnyStatus;

/// The throwing fighter's identity, copied while the capture link exists.
/// The port identifies both `player` and `player_num` by the controller port.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThrowOwner {
    pub port: u8,
    pub kind: FighterKind,
    pub team: u8,
}

impl ThrowOwner {
    pub fn of(f: &Fighter) -> Self {
        Self {
            port: f.port,
            kind: f.kind,
            team: f.team,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ThrownState {
    /// `throw_gobj`, `throw_fkind`, `throw_player`, `throw_team`.
    pub owner: Option<ThrowOwner>,
    /// `status_vars.common.damage.script_id`: forward/cargo 0, back 1.
    pub script_id: u8,
    /// One-shot `proc_status`, consumed after the status clears its old
    /// throw pointer and before it starts the new motion events.
    pub(crate) pending: Option<(ThrowOwner, u8)>,
}

pub(crate) fn on_status(f: &mut Fighter) {
    if let Some((owner, script_id)) = f.thrown.pending.take() {
        f.thrown.owner = Some(owner);
        f.thrown.script_id = script_id;
    }
}

/// Tail of `ftCommonDamageCommonProcPhysics`, before damage velocity's
/// decay in `ftMainProcPhysicsMap`. It clears attacks below 70, but keeps
/// the throw pointer until a status change clears it.
pub(crate) fn damage_physics(f: &mut Fighter) {
    if f.thrown.owner.is_some()
        && matches!(f.status.status, AnyStatus::Common(s)
            if crate::reaction::is_damage_common(s)
                || crate::reaction::is_damage_air(s))
        && f.physics.vel_knockback.length() < 70.0
    {
        crate::combat::clear_attack_colls(f);
    }
}

#[cfg(test)]
#[path = "thrown_tests.rs"]
mod tests;
