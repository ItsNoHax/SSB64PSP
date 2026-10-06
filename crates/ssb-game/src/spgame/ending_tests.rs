//! The ending, staff roll, congratulations, challenger and message scenes.
use super::{
    challenger::Challenger,
    congra::{Congra, Fade, Voice},
    ending::Ending,
    frontend,
    manager::Scene,
    message::{apply_unlock, Message},
    staffroll::{self, Credits, Job, NameMotion, Staffroll, TextInfo},
    *,
};
use alloc::{boxed::Box, vec::Vec};
use ssb_engine::input::{ControllerState, N64Buttons};

fn tap(b: u16) -> N64Buttons {
    N64Buttons(b)
}

#[test]
fn the_ending_fades_the_room_in_brightens_then_blacks_out_on_its_clock() {
    let mut e = Ending::new(FighterKind::Fox, 2, 0);
    let mut alphas = Vec::new();
    for _ in 0..EndingTics::END - 1 {
        assert!(!e.tick());
        e.draw();
        alphas.push((e.total_tics, e.fade_in_alpha, e.light_alpha));
    }
    // Opaque until tick 70, then 7 a frame to clear.
    assert_eq!(alphas[68].1, 0xFF);
    assert_eq!(alphas[69].1, 0xFF - 7);
    assert_eq!(alphas[69 + 36].1, 0);
    // The light from tick 340, 1.1 a frame up to 220.
    assert_eq!(alphas[338].2, None);
    assert!((alphas[339].2.unwrap() - 1.1).abs() < 1e-5);
    assert_eq!(alphas[538].2, Some(220.0));
    // Tick 540 removes the props, figure and light and blacks the room.
    assert!(!e.props_shown && !e.fighter_shown);
    assert_eq!(alphas[539].1, 0xFF);
    assert_eq!(alphas[539].2, None);
    assert!(e.tick(), "tick 660 requests the staff roll");
    assert!(!e.tick());
}

struct EndingTics;
impl EndingTics {
    const END: u32 = super::ending::END_TIC;
}

#[test]
fn congratulations_waits_eight_ticks_then_fades_ninety_and_ends_five_frames_after_blackout() {
    let mut c = Congra::new(FighterKind::Mario, 999_999);
    assert_eq!(c.voice, Voice::Congratulations);
    assert_eq!(
        Congra::new(FighterKind::Mario, 1_000_000).voice,
        Voice::Incredible
    );
    for _ in 0..7 {
        assert!(!c.tick(tap(N64Buttons::A)));
        assert!(c.fade.is_none());
    }
    assert!(!c.tick(tap(N64Buttons::B)));
    assert_eq!(c.fade.unwrap().current, 1);
    let mut t = 8;
    while !c.tick(tap(0)) {
        t += 1;
        assert!(t < 200);
    }
    // Proceed at the fade's 92nd update (tick 99), then four more draws.
    assert_eq!(t + 1, 8 + 91 + 4);
    assert!(c.blackout);
    let mut f = Fade::new(90);
    f.update();
    assert_eq!(f.alpha(), 2);
}

#[test]
fn the_challenger_turns_two_degrees_a_tick_and_waits_two_seconds() {
    let mut c = Challenger::new(FighterKind::Ness);
    for _ in 0..119 {
        assert!(!c.tick(tap(N64Buttons::START)));
    }
    assert!((c.rotate_y - 119.0 * super::challenger::TURN).abs() < 1e-4);
    assert!(c.tick(tap(N64Buttons::A)));
    assert!(!c.tick(tap(N64Buttons::A)), "the scene ends once");
    let mut c = Challenger::new(FighterKind::Ness);
    for _ in 0..181 {
        c.tick(tap(0));
    }
    assert!(c.rotate_y <= super::challenger::FULL_TURN && c.rotate_y >= 0.0);
}

#[test]
fn the_message_writes_its_unlock_when_closed() {
    let mut backup = Backup::default();
    let mut m = Message::new(Unlock::Ness);
    for _ in 0..119 {
        assert!(!m.tick(tap(N64Buttons::A), &mut backup));
    }
    assert_eq!(backup.unlock_mask, 0);
    assert!(m.tick(tap(N64Buttons::START), &mut backup));
    assert_eq!(backup.unlock_mask, Unlock::Ness.mask());
    assert_ne!(backup.fighter_mask & (1 << FighterKind::Ness as u16), 0);
    assert_eq!(backup.characters_fkind, FighterKind::Ness);
    assert_eq!(backup.writes, 1);
    let mut backup = Backup::default();
    apply_unlock(&mut backup, Unlock::Inishie);
    assert_eq!(backup.fighter_mask, CHARACTER_MASK_STARTER);
    assert_eq!(backup.characters_fkind, FighterKind::Mario);
    assert_eq!(backup.writes, 1);
}

fn chars(s: &[u8]) -> Vec<i32> {
    s.iter().map(|&c| staffroll::letter(c)).collect()
}

/// A tiny credits table: two jobs, three names, their roles and two
/// companies.
fn credits() -> Credits {
    let mut name_chars = chars(b"Ta So");
    name_chars.extend(chars(b"Ky"));
    name_chars.extend(chars(b"Wo"));
    let mut job_chars = chars(b"Director");
    job_chars.extend(chars(b"Presents"));
    let mut role_chars = chars(b"Luigi and Ness");
    role_chars.push(staffroll::NEWLINE);
    role_chars.extend(chars(b"xLuigi"));
    role_chars.extend(chars(b"Luigi"));
    Credits {
        name_chars,
        names: alloc::vec![
            TextInfo { start: 0, count: 5 },
            TextInfo { start: 5, count: 2 },
            TextInfo { start: 7, count: 2 },
        ],
        jobs: alloc::vec![
            Job {
                prefix: -1,
                job: 0,
                staff_count: 2,
            },
            Job {
                prefix: -1,
                job: 1,
                staff_count: -1,
            },
        ],
        job_chars,
        job_texts: alloc::vec![
            TextInfo { start: 0, count: 8 },
            TextInfo { start: 8, count: 8 },
        ],
        role_chars,
        roles: alloc::vec![
            TextInfo {
                start: 0,
                count: 15
            },
            TextInfo {
                start: 15,
                count: 6
            },
            TextInfo {
                start: 21,
                count: 5
            },
        ],
        company_chars: chars(b"HALNINTENDO"),
        companies: alloc::vec![
            TextInfo { start: 0, count: 3 },
            TextInfo { start: 3, count: 8 },
        ],
        company_ids: alloc::vec![1, staffroll::COMPANY_NULL, 0],
    }
}

/// Every name sits at the origin, untilted.
struct Still;
impl NameMotion for Still {
    fn path(&self, _: f32) -> [f32; 3] {
        [0.0; 3]
    }
    fn rotate_z(&self, _: f32) -> f32 {
        0.0
    }
}

fn roll(s: &mut Staffroll, taps: u16) -> bool {
    s.tick(ControllerState::default(), tap(taps), &Still)
}

#[test]
fn unlocks_hide_their_names_in_the_role_texts_but_not_the_last() {
    let mut c = credits();
    c.hide_unlocks(Unlock::Ness.mask());
    let q = staffroll::QUESTION_MARK;
    assert_eq!(&c.role_chars[..5], &[q; 5]);
    // Ness is unlocked; the second text's "xLuigi" matches after the x.
    assert_eq!(&c.role_chars[10..14], &chars(b"Ness")[..]);
    assert_eq!(&c.role_chars[16..21], &[q; 5]);
    // The last text is never searched.
    assert_eq!(&c.role_chars[21..], &chars(b"Luigi")[..]);
}

#[test]
fn the_crosshair_drops_in_then_the_roll_waits_two_seconds() {
    let mut s = Staffroll::new(credits(), !0);
    for t in 1..=20 {
        roll(&mut s, 0);
        assert_eq!(s.crosshair, [291.0, 10.5 * t as f32]);
        assert_eq!(s.status, 2);
    }
    roll(&mut s, 0);
    assert_eq!(s.status, 1);
    // The first job exists from the first tick, hidden until the roll.
    assert_eq!(s.texts.len(), 1);
    assert!(s.texts[0].hidden && !s.texts[0].is_name);
    for _ in 0..120 {
        roll(&mut s, 0);
        assert_eq!(s.status, 1);
    }
    roll(&mut s, 0);
    assert_eq!(s.status, 0);
    assert!(s.text_box);
    assert!(!s.texts[0].hidden);
    assert!((s.texts[0].interpolation - staffroll::ROLL_SPEED_SLOW).abs() < 1e-7);
}

#[test]
fn letters_kern_and_drop_their_descenders() {
    let mut s = Staffroll::new(credits(), !0);
    // Run until the first name ("Ta So") exists.
    while !s.texts.iter().any(|t| t.is_name) {
        roll(&mut s, 0);
    }
    let name = s.texts.iter().find(|t| t.is_name).unwrap();
    let [tw, th] = staffroll::NAME_GLYPHS[staffroll::letter(b'T') as usize].map(f32::from);
    let [aw, _] = staffroll::NAME_GLYPHS[staffroll::letter(b'a') as usize].map(f32::from);
    assert_eq!(name.letters[0].x, tw);
    assert_eq!(name.letters[0].y, th - 22.0);
    // "Ta": the pair moves 6 closer.
    assert_eq!(name.letters[1].x, 2.0 * tw - 6.0 + aw);
    let last = name.letters.last().unwrap().x;
    assert_eq!(name.offset_x, -last * 0.5);
    while !s.texts.iter().any(|t| t.is_name && t.name_id == 1) {
        roll(&mut s, 0);
    }
    let ky = &s
        .texts
        .iter()
        .find(|t| t.is_name && t.name_id == 1)
        .unwrap()
        .letters;
    let [kw, _] = staffroll::NAME_GLYPHS[staffroll::letter(b'K') as usize].map(f32::from);
    let [yw, _] = staffroll::NAME_GLYPHS[staffroll::letter(b'y') as usize].map(f32::from);
    assert_eq!(ky[1].x, 2.0 * kw - 6.0 + yw);
    assert_eq!(ky[1].y, -8.0, "y drops below the line");
}

#[test]
fn start_switches_the_roll_speed_and_b_pauses_until_a_tap() {
    let mut s = Staffroll::new(credits(), !0);
    for _ in 0..141 {
        roll(&mut s, 0);
    }
    roll(&mut s, N64Buttons::START);
    assert_eq!(s.roll_speed, staffroll::ROLL_SPEED_FAST);
    let before = s.texts[0].interpolation;
    roll(&mut s, N64Buttons::B);
    assert!(s.is_paused);
    for _ in 0..10 {
        roll(&mut s, 0);
    }
    assert_eq!(s.texts[0].interpolation, before);
    roll(&mut s, N64Buttons::Z);
    assert!(!s.is_paused);
    assert!(s.texts[0].interpolation > before);
    roll(&mut s, N64Buttons::START);
    assert_eq!(s.roll_speed, staffroll::ROLL_SPEED_SLOW);
}

#[test]
fn a_over_a_name_highlights_it_and_shows_its_role_and_company() {
    let mut s = Staffroll::new(credits(), !0);
    while !s.texts.iter().any(|t| t.is_name && t.interpolation > 0.0) {
        roll(&mut s, 0);
    }
    roll(&mut s, N64Buttons::A);
    assert_eq!(s.highlights, 1);
    let frame = s.frame.unwrap();
    let name = s.texts.iter().find(|t| t.is_name).unwrap();
    assert_eq!(frame.translate, name.translate);
    assert_eq!(frame.child_x, name.offset_x.abs() * 2.0 + 18.0);
    let role = s.role_text.as_ref().unwrap();
    assert_eq!(role[0].x, 350.0);
    // "Luigi and Ness": 'u' sits 3 lower, the tall 'i' 1.
    assert_eq!(role[1].y, 43.0);
    assert_eq!(role[2].y, 41.0);
    // Name 0's company is NINTENDO, from x 350 at y 140.
    let company = s.company_text.as_ref().unwrap();
    assert_eq!(company.len(), 8);
    assert_eq!((company[0].x, company[0].y), (350.0, 140.0));
    // The lock-on corners close in three times, then go.
    let mut sizes = Vec::new();
    while let Some(h) = s.highlight {
        sizes.push(h.size);
        roll(&mut s, 0);
    }
    assert_eq!(sizes, [5, 4, 3, 2, 1, 0].repeat(3));
}

#[test]
fn the_last_names_arrival_blacks_out_and_ends_sixty_frames_later() {
    let mut s = Staffroll::new(credits(), !0);
    let mut t = 0;
    while s.status >= 0 {
        roll(&mut s, if t == 150 { N64Buttons::START } else { 0 });
        t += 1;
        assert!(t < 5000);
    }
    assert_eq!(s.status, -2);
    assert!(!s.blackout, "the blackout shows from the next frame");
    // The arrival's frame and 59 more.
    let mut frames = 1;
    loop {
        frames += 1;
        let done = roll(&mut s, N64Buttons::START);
        assert!(s.blackout);
        assert!(frames < 100);
        if done {
            break;
        }
    }
    assert_eq!(frames, 60);
    // Two jobs and three names were made, in the source's order.
    assert_eq!(s.name_id, 3);
}

fn finished_boss_session(backup: &Backup) -> session::Session {
    let mut s = session::Session::new(
        SceneData {
            stage: Stage::Boss as u8,
            fkind: FighterKind::Kirby,
            ..Default::default()
        },
        backup,
    );
    s.manager.scene = Scene::StageClear;
    s
}

#[test]
fn a_cleared_campaign_runs_the_ending_staff_roll_and_congratulations_into_a_challenger() {
    let mut backup = Backup::default();
    let mut f = frontend::Frontend::new(Default::default(), &backup);
    f.session = Some(Box::new(finished_boss_session(&backup)));
    f.staffroll_assets = Some(frontend::StaffrollAssets {
        credits: credits(),
        motion: Box::new(Still),
    });
    f.sync(&backup);
    let mut events = Vec::new();
    let mut t = 0;
    let mut seen = Vec::new();
    while !matches!(f.screen, frontend::Screen::Host(_)) {
        t += 1;
        assert!(t < 20_000);
        let name = match &f.screen {
            frontend::Screen::StageClear(_) => "clear",
            frontend::Screen::Ending(_) => "ending",
            frontend::Screen::Staffroll(_) => "staffroll",
            frontend::Screen::Congra(_) => "congra",
            frontend::Screen::Challenger(_) => "challenger",
            _ => "other",
        };
        if seen.last() != Some(&name) {
            seen.push(name);
        }
        let press = if t % 150 == 0 { N64Buttons::A } else { 0 };
        f.tick(
            t,
            ControllerState::default(),
            tap(press),
            &mut backup,
            |e| events.push(e),
        );
    }
    assert_eq!(
        seen,
        ["clear", "ending", "staffroll", "congra", "challenger"]
    );
    assert_eq!(events.last(), Some(&frontend::Event::Host(Scene::Battle)));
    let s = f.session.as_ref().unwrap();
    // Normal, no continues, two stocks: Ness approaches.
    assert_eq!(s.data.stage(), Some(Stage::Ness));
    assert_eq!(s.data.challenger_fkind, FighterKind::Ness);
    assert_eq!(s.state.players[0].stock_count, 0);
    assert!(s.data.unlock_message.is_none());
    assert!(backup.spgame_records[FighterKind::Kirby as usize].is_spgame_complete);
}

#[test]
fn without_its_assets_the_staff_roll_stays_a_host_request() {
    let mut backup = Backup::default();
    let mut f = frontend::Frontend::new(Default::default(), &backup);
    let mut s = finished_boss_session(&backup);
    s.manager.scene = Scene::Staffroll;
    f.session = Some(Box::new(s));
    assert_eq!(f.sync(&backup), Scene::Staffroll);
    assert!(matches!(f.screen, frontend::Screen::Host(Scene::Staffroll)));
    f.tick(
        1,
        ControllerState::default(),
        tap(N64Buttons::A),
        &mut backup,
        |_| {},
    );
    assert!(matches!(f.screen, frontend::Screen::Host(Scene::Staffroll)));
}

#[test]
fn a_won_challenge_shows_its_newcomer_message_then_returns_to_startup() {
    let mut backup = Backup::default();
    let mut f = frontend::Frontend::new(Default::default(), &backup);
    let mut s = finished_boss_session(&backup);
    s.data.stage = Stage::Ness as u8;
    s.manager.scene = Scene::Battle;
    s.state.players[0].stock_count = 0;
    s.state.time_remain = 1;
    s.manager.advance(&mut s.data, &mut s.state, &mut backup);
    assert_eq!(s.manager.scene, Scene::Message);
    f.session = Some(Box::new(s));
    f.sync(&backup);
    assert!(f.session.as_ref().unwrap().data.unlock_message.is_none());
    let mut events = Vec::new();
    for t in 1..=120 {
        f.tick(
            t,
            ControllerState::default(),
            tap(if t == 120 { N64Buttons::A } else { 0 }),
            &mut backup,
            |e| events.push(e),
        );
    }
    assert_eq!(events, [frontend::Event::Host(Scene::Startup)]);
    assert_ne!(backup.fighter_mask & (1 << FighterKind::Ness as u16), 0);
}

#[test]
fn the_classic_mario_message_follows_a_newcomers_when_due() {
    let mut backup = Backup {
        ground_mask: GROUND_MASK_ALL,
        ..Default::default()
    };
    for (i, r) in backup.spgame_records.iter_mut().enumerate() {
        r.is_spgame_complete = CHARACTER_MASK_STARTER & (1 << i) != 0;
    }
    let mut f = frontend::Frontend::new(Default::default(), &backup);
    let mut s = finished_boss_session(&backup);
    s.data.stage = Stage::Purin as u8;
    s.manager.scene = Scene::Battle;
    s.state.time_remain = 1;
    s.manager.advance(&mut s.data, &mut s.state, &mut backup);
    f.session = Some(Box::new(s));
    f.sync(&backup);
    let mut unlocks = Vec::new();
    let mut events = Vec::new();
    for t in 1..=240 {
        if let frontend::Screen::Message(m) = &f.screen {
            if unlocks.last() != Some(&m.unlock) {
                unlocks.push(m.unlock);
            }
        }
        f.tick(
            t,
            ControllerState::default(),
            tap(if t % 120 == 0 { N64Buttons::A } else { 0 }),
            &mut backup,
            |e| events.push(e),
        );
    }
    assert_eq!(unlocks, [Unlock::Purin, Unlock::Inishie]);
    assert_eq!(events, [frontend::Event::Host(Scene::Startup)]);
    assert_eq!(
        backup.unlock_mask,
        Unlock::Purin.mask() | Unlock::Inishie.mask()
    );
}
