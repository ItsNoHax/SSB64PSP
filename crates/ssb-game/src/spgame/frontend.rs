//! Portable campaign frontend. Battle/bonus/ending overlays remain explicit
//! host requests; they cannot be replaced with VS matches or skipped.
use super::{continue_scene, intro, manager::Scene, select, session::Session, stage_clear, Backup};
use ssb_engine::input::{ControllerState, N64Buttons};

pub enum Screen {
    Select(select::Select),
    Intro(intro::Intro),
    Continue(continue_scene::Continue),
    StageClear(stage_clear::StageClear),
    /// Run this overlay and return its real result via Session.
    Host(Scene),
}

pub struct Frontend {
    pub screen: Screen,
    pub session: Option<alloc::boxed::Box<Session>>,
    /// The remembered select fields, including a null/unplaced fighter.
    pub selection: select::Selection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Host(Scene),
    Announce(intro::Announce),
    Continue(continue_scene::Event),
}

impl Frontend {
    pub fn new(selection: select::Selection, backup: &Backup) -> Self {
        Self {
            screen: Screen::Select(select::Select::new(selection, backup.fighter_mask)),
            session: None,
            selection,
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
            session: Some(alloc::boxed::Box::new(Session::new(data, backup))),
            selection,
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
            session: Some(alloc::boxed::Box::new(session)),
            selection,
        };
        frontend.sync(backup);
        Some(frontend)
    }

    /// Called after the host consumes a Battle/BonusStage/other scene.
    /// This creates the new presentation once, on that scene boundary.
    pub fn sync(&mut self, backup: &Backup) -> Scene {
        let session = self.session.as_ref().expect("campaign session");
        let scene = session.manager.scene;
        self.screen = match scene {
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
                        self.session = Some(alloc::boxed::Box::new(Session::new(
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
                for announce in frame.announce.into_iter().flatten() {
                    emit(Event::Announce(announce));
                }
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
            Screen::Host(_) => false,
        };
        if advance {
            let scene = self.sync(backup);
            if matches!(self.screen, Screen::Host(_)) {
                emit(Event::Host(scene));
            }
        }
    }
}
