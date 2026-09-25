//! `sceCtrl` backend implementing [`ssb_engine::input::Input`].

use psp::sys::{self, SceCtrlData};

use ssb_engine::input::{
    map_psp_to_n64, ButtonMapping, ControllerState, Input, PspButtons, DEFAULT_MAPPING,
};

/// The PSP has one controller; ports 1-3 exist so multiplayer code compiles
/// and CPU players can occupy them.
pub const MAX_PORTS: usize = 4;

pub struct PspInput {
    current: [ControllerState; MAX_PORTS],
    previous: [ControllerState; MAX_PORTS],
    mapping: &'static [ButtonMapping],
}

impl PspInput {
    /// # Safety
    ///
    /// Must be called once before polling.
    pub unsafe fn init() -> PspInput {
        Self::init_with_mapping(DEFAULT_MAPPING)
    }

    /// Initializes the controller backend with an application's raw N64
    /// button layout.  The backend applies this table before callers observe
    /// controller state, so it never needs PSP-specific gameplay behaviour.
    ///
    /// # Safety
    ///
    /// Must be called once before polling.
    pub unsafe fn init_with_mapping(mapping: &'static [ButtonMapping]) -> PspInput {
        sys::sceCtrlSetSamplingCycle(0);
        // Analog mode: without this the nub always reads dead centre, which is
        // an easy thing to lose an afternoon to.
        sys::sceCtrlSetSamplingMode(sys::CtrlMode::Analog);

        PspInput {
            current: [ControllerState::default(); MAX_PORTS],
            previous: [ControllerState::default(); MAX_PORTS],
            mapping,
        }
    }
}

impl Input for PspInput {
    fn poll(&mut self) {
        self.previous = self.current;

        let mut pad = SceCtrlData::default();
        let samples = unsafe {
            // ReadBuffer waits for a fresh controller sample. When a slow
            // display frame requires several fixed simulation ticks, one
            // wait per tick turns catch-up into a feedback loop. Peek takes
            // the latest sample without stalling the simulation.
            sys::sceCtrlPeekBufferPositive(&mut pad, 1)
        };

        if samples > 0 {
            self.current[0] =
                map_psp_to_n64(PspButtons(pad.buttons.bits()), pad.lx, pad.ly, self.mapping);
        }

        // Remaining ports stay disconnected until CPU players fill them.
        for p in 1..MAX_PORTS {
            self.current[p] = ControllerState::default();
        }
    }

    fn state(&self, port: usize) -> ControllerState {
        self.current.get(port).copied().unwrap_or_default()
    }

    fn previous(&self, port: usize) -> ControllerState {
        self.previous.get(port).copied().unwrap_or_default()
    }

    fn set_rumble(&mut self, _port: usize, _on: bool) {
        // The PSP has no rumble motor. Deliberately a no-op rather than an
        // error: the game calls this from ordinary hit reactions.
    }
}
