//! Portable campaign frontend. Battle and bonus overlays remain explicit
//! host requests; they cannot be replaced with VS matches or skipped. The
//! ending, staff roll, congratulations, challenger and message scenes run
//! here; the staff roll needs the host's [`StaffrollAssets`].
use super::{
    challenger, congra, continue_scene, ending, intro, manager::Scene, message, select,
    session::Session, staffroll, stage_clear, Backup,
};
use alloc::boxed::Box;
use ssb_engine::input::{ControllerState, N64Buttons};

pub enum Screen {
    Select(select::Select),
    Intro(intro::Intro),
    Continue(continue_scene::Continue),
    StageClear(stage_clear::StageClear),
    Ending(ending::Ending),
    Staffroll(Box<staffroll::Staffroll>),
    Congra(congra::Congra),
    Challenger(challenger::Challenger),
    Message(message::Message),
    /// Run this overlay and return its real result via Session.
    Host(Scene),
}

/// What the staff roll reads from the ROM: the overlay's credits tables
/// and file 195's name path and tilt.
pub struct StaffrollAssets {
    pub credits: staffroll::Credits,
    pub motion: Box<dyn staffroll::NameMotion>,
}

pub struct Frontend {
    pub screen: Screen,
    pub session: Option<Box<Session>>,
    /// The remembered select fields, including a null/unplaced fighter.
    pub selection: select::Selection,
    /// The staff roll's ROM data; without it the staff roll stays a host
    /// request.
    pub staffroll_assets: Option<StaffrollAssets>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Host(Scene),
    Continue(continue_scene::Event),
}

impl Frontend {
    pub fn new(selection: select::Selection, backup: &Backup) -> Self {
        Self {
            screen: Screen::Select(select::Select::new(selection, backup.fighter_mask)),
            session: None,
            selection,
            staffroll_assets: None,
        }
    }

    /// A campaign begun by a host-drawn select (`players_1p`), which has
    /// already applied its scene and backup data. Starts at the manager's
    /// first requested scene, as a ready Proceed does.
    pub fn campaign(data: super::SceneData, backup: &Backup) -> Self {
        let selection = select::Selection {
            player: data.player,
            kind: Some(data.fkind),
            costume: data.costume,
            time_limit: data.time_limit,
            difficulty: backup.spgame_difficulty,
            stocks: backup.spgame_stock_count,
        };
        let mut frontend = Self {
            screen: Screen::Host(Scene::Startup),
            session: Some(Box::new(Session::new(data, backup))),
            selection,
            staffroll_assets: None,
        };
        frontend.sync(backup);
        frontend
    }

    /// A host whose own select already saved `selection` and proceeded
    /// (`nSCKind1PGame`): the campaign begins at its first scene. `None`
    /// for an unplaced fighter, which cannot proceed.
    pub fn start(selection: select::Selection, backup: &Backup) -> Option<Self> {
        let session = Session::new(selection.campaign()?, backup);
        let mut frontend = Self {
            screen: Screen::Host(Scene::Startup),
            session: Some(Box::new(session)),
            selection,
            staffroll_assets: None,
        };
        frontend.sync(backup);
        Some(frontend)
    }

    /// Called after the host consumes a Battle/BonusStage/other scene.
    /// This creates the new presentation once, on that scene boundary.
    pub fn sync(&mut self, backup: &Backup) -> Scene {
        let session = self.session.as_mut().expect("campaign session");
        let scene = session.manager.scene;
        self.screen = match scene {
            Scene::Ending => {
                // `mvEndingInitVars`: the 1P player's battle slot.
                let p = session.state.players[usize::from(session.data.player)];
                Screen::Ending(ending::Ending::new(p.fkind, p.costume, p.shade))
            }
            Scene::Staffroll => match self.staffroll_assets.as_ref() {
                Some(assets) => Screen::Staffroll(Box::new(staffroll::Staffroll::new(
                    assets.credits.clone(),
                    backup.unlock_mask,
                ))),
                None => Screen::Host(scene),
            },
            Scene::Congratulations => {
                Screen::Congra(congra::Congra::new(session.data.fkind, session.data.score))
            }
            Scene::Challenger => {
                Screen::Challenger(challenger::Challenger::new(session.data.challenger_fkind))
            }
            // `mnMessageInitVars` takes the queue's entry.
            Scene::Message => match session.data.unlock_message.take() {
                Some(unlock) => Screen::Message(message::Message::new(unlock)),
                None => Screen::Host(scene),
            },
            Scene::Intro => Screen::Intro(intro::Intro::new(
                session.data.stage().unwrap(),
                session.data.fkind,
            )),
            Scene::Continue => Screen::Continue(continue_scene::Continue::new(session.data.score)),
            Scene::StageClear => Screen::StageClear(stage_clear::StageClear::new(
                &session.data,
                &session.state,
                backup.spgame_difficulty,
            )),
            _ => Screen::Host(scene),
        };
        scene
    }

    /// One simulation tick. Emits scene handoffs once and leaves the real
    /// battle's ticks, setup, replacements and result collection to Session.
    /// `scheduler_tic` is the current overlay's scheduler clock; the host
    /// resets it on entry to Intro, as `sc1PIntroFuncStart` does.
    pub fn tick(
        &mut self,
        scheduler_tic: u32,
        input: ControllerState,
        taps: N64Buttons,
        backup: &mut Backup,
        mut emit: impl FnMut(Event),
    ) {
        let advance = match &mut self.screen {
            Screen::Select(select) => {
                if let Some(outcome) = select.tick(input, taps) {
                    let (selection, scene) = match outcome {
                        select::Outcome::Proceed(s) => (s, None),
                        select::Outcome::Back(s) => (s, Some(Scene::ModeMenu)),
                        select::Outcome::Timeout(s) => (s, Some(Scene::Startup)),
                    };
                    self.selection = selection;
                    selection.save(backup);
                    if let Some(scene) = scene {
                        self.screen = Screen::Host(scene);
                        emit(Event::Host(scene));
                    } else {
                        self.session = Some(Box::new(Session::new(
                            selection.campaign().expect("ready selection"),
                            backup,
                        )));
                        self.sync(backup);
                    }
                }
                false
            }
            Screen::Intro(intro) => {
                let frame = intro.tick(scheduler_tic, taps);
                if frame.proceed {
                    let session = self.session.as_mut().expect("campaign session");
                    session
                        .manager
                        .advance(&mut session.data, &mut session.state, backup);
                }
                frame.proceed
            }
            Screen::Continue(scene) => {
                let frame = scene.tick(input, taps);
                if let Some(event) = frame.event {
                    if event == continue_scene::Event::Accepted {
                        // The source changes scene data on Yes, before its
                        // four-second retry hold, not when the overlay exits.
                        self.session.as_mut().expect("campaign session").data.score = scene.score;
                    }
                    emit(Event::Continue(event));
                }
                if let Some(accepted) = frame.result {
                    let session = self.session.as_mut().expect("campaign session");
                    session.data.is_continue = accepted;
                    session
                        .manager
                        .advance(&mut session.data, &mut session.state, backup);
                }
                frame.result.is_some()
            }
            Screen::StageClear(scene) => {
                if let Some(score) = scene.tick(taps) {
                    let session = self.session.as_mut().expect("campaign session");
                    session.data.score = score;
                    session
                        .manager
                        .advance(&mut session.data, &mut session.state, backup);
                    true
                } else {
                    false
                }
            }
            Screen::Ending(scene) => scene.tick(),
            Screen::Staffroll(scene) => match self.staffroll_assets.as_ref() {
                Some(assets) => scene.tick(input, taps, assets.motion.as_ref()),
                None => false,
            },
            Screen::Congra(scene) => scene.tick(taps),
            Screen::Challenger(scene) => scene.tick(taps),
            Screen::Message(scene) => scene.tick(taps, backup),
            Screen::Host(_) => false,
        };
        if advance
            && matches!(
                self.screen,
                Screen::Ending(_)
                    | Screen::Staffroll(_)
                    | Screen::Congra(_)
                    | Screen::Challenger(_)
                    | Screen::Message(_)
            )
        {
            let session = self.session.as_mut().expect("campaign session");
            session
                .manager
                .advance(&mut session.data, &mut session.state, backup);
        }
        if advance {
            let scene = self.sync(backup);
            if matches!(self.screen, Screen::Host(_)) {
                emit(Event::Host(scene));
            }
        }
    }
}
