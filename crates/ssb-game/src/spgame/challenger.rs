//! `sc/sc1pmode/sc1pchallenger.c`: "WARNING: CHALLENGER APPROACHING" before
//! a newcomer's battle.
//!
//! The scene shows its decals and the challenger as a black silhouette
//! (`nGMColAnimFighterChallenger`) turning about its vertical axis. The
//! fighter is a demo fighter (`dFTManagerDefaultFighterDesc.pkind` is
//! `nFTPlayerKindDemo`), so `ftManagerMakeFighter` starts its
//! `nFTDemoStatusNull` (`Wait`) clip and leaves TopN's yaw at zero.
//! After two seconds any of A, B or START ends the scene.
use crate::fighter::FighterKind;
use ssb_engine::input::N64Buttons;

/// Ticks before A, B or START may close the scene.
pub const INPUT_WAIT: u32 = 120;
/// `F_CST_DTOR32(2.0F)`: `sc1PChallengerFighterProcUpdate`'s turn per tick.
pub const TURN: f32 = 2.0 * (core::f32::consts::PI / 180.0);
/// `F_CST_DTOR32(360.0F)`.
pub const FULL_TURN: f32 = 360.0 * (core::f32::consts::PI / 180.0);
/// `sc1PChallengerMakeFighter`'s translation.
pub const FIGHTER_POSITION: [f32; 3] = [610.0, -550.0, 0.0];
/// `sc1PChallengerMakeFighterCamera`'s eye; it looks at the origin.
pub const CAMERA_EYE: [f32; 3] = [0.0, 0.0, 3000.0];

/// `sc1PChallengerDecalsProcDisplay`'s dark blue panel behind the fighter
/// (`G_RM_AA_XLU_SURF`, opaque).
pub const PANEL_COLOR: [u8; 4] = [0x00, 0x00, 0x3B, 0xFF];
pub const PANEL_RECT: [f32; 4] = [207.0, 92.0, 287.0, 216.0];

/// One `sc1PChallengerMakeDecals` sprite.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Decal {
    pub sprite: DecalSprite,
    pub pos: [f32; 2],
    /// `sprite.red/green/blue`; `None` keeps the sprite's own colour.
    pub color: Option<[u8; 3]>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecalSprite {
    Exclaim,
    Warning,
    Challenger,
    Approaching,
}

/// The decals in `sc1PChallengerMakeDecals`'s order.
pub const DECALS: [Decal; 4] = [
    Decal {
        sprite: DecalSprite::Exclaim,
        pos: [139.0, 22.0],
        color: None,
    },
    Decal {
        sprite: DecalSprite::Warning,
        pos: [100.0, 63.0],
        color: Some([0xD5, 0x00, 0x27]),
    },
    Decal {
        sprite: DecalSprite::Challenger,
        pos: [55.0, 127.0],
        color: Some([0xFF, 0xA8, 0x00]),
    },
    Decal {
        sprite: DecalSprite::Approaching,
        pos: [55.0, 149.0],
        color: Some([0xFF, 0xA8, 0x00]),
    },
];

#[derive(Debug, Clone, PartialEq)]
pub struct Challenger {
    /// `sSC1PChallengerFighterKind`.
    pub fkind: FighterKind,
    /// `sSC1PChallengerTotalTimeTics`.
    pub total_tics: u32,
    /// The fighter's `DObj` yaw.
    pub rotate_y: f32,
    finished: bool,
}

impl Challenger {
    /// `sc1PChallengerInitVars`.
    pub fn new(fkind: FighterKind) -> Self {
        Self {
            fkind,
            total_tics: 0,
            rotate_y: 0.0,
            finished: false,
        }
    }

    /// One tick: `sc1PChallengerFuncRun`, then the fighter's turn process.
    /// Returns `true` on the tick the scene ends.
    pub fn tick(&mut self, taps: N64Buttons) -> bool {
        if self.finished {
            return false;
        }
        self.total_tics += 1;
        if self.total_tics >= INPUT_WAIT
            && taps.contains(N64Buttons::A | N64Buttons::B | N64Buttons::START)
        {
            self.finished = true;
        }
        self.rotate_y += TURN;
        if self.rotate_y > FULL_TURN {
            self.rotate_y -= FULL_TURN;
        }
        self.finished
    }
}
