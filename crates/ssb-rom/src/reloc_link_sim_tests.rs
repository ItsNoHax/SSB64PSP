//! L2 of the runtime-loader batch: the kept-file policy for one scene
//! block per load, decided by simulating the game's scene transitions
//! over the built pack (`assets/generated/ssb64.pak`). Sizes only; nothing
//! is written anywhere (D-048).
//!
//! The model is `psp-runtime/src/scene_files.rs` (`enter_with`, `prefetch`,
//! `safe_point`, `load_missing`, `queue_missing`) driven by the root lists
//! `psp-game/src/scene_load.rs` builds, in the order `psp-game` calls them
//! (`opening_screen::load`, `load_scene`, `read_ahead_menus`,
//! `Session::enter`, `open_stage_select`, the results and `campaign`).
//! Background reads complete before the next event, and the
//! `PREFETCH_MARGIN` check is not applied (the run reports the lowest free
//! memory it implies).

extern crate std;

use alloc::string::String;
use alloc::vec::Vec;
use std::println;

use super::{place_into, FileSource, Order, Placement};
use crate::pack::Pack;
use crate::scene_roots as r;

const MAX_RUN: u32 = 4 << 20;
const MAX_BACKGROUND_RUN: u32 = 1 << 20;
const NO_BLOCK: usize = usize::MAX;

/// The bytes a 2 ms copy moves at 100 MB/s (plan rule 2.2).
const COPY_LIMIT: u64 = 200_000;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Keep {
    No,
    Scene,
    Leaving,
    Next,
}

/// How a load's bytes are held.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Policy {
    /// Today: one block per read run.
    Runs,
    /// P: one block per load; a kept file pins its block.
    Pin,
    /// C: one block per load; every kept file held elsewhere is copied in.
    Copy,
    /// C': as C, but only out of blocks the load would leave part dead.
    CopyPartial,
}

struct Block {
    size: u64,
    live: u32,
}

/// A scene-loading event.
pub(crate) enum Event {
    /// `scene_load::scene`.
    Scene(&'static str, Vec<u32>),
    /// `scene_load::battle`.
    Battle(Vec<u32>),
    /// `scene_load::set_base`.
    SetBase(Vec<u32>),
    /// `scene_load::prefetch`, `read_ahead`.
    Prefetch(Vec<u32>),
    /// `scene_load::maps`: a scene whose own files are the first, entered
    /// with the second (a stage select's first preview).
    Maps(Vec<u32>, Vec<u32>),
    /// `scene_load::maps_preview`: the scene's own files and a stage
    /// select's preview, entered within the scene.
    Preview(Vec<u32>),
    /// `scene_files::safe_point` after a frame.
    Frame,
}

struct Sim<'a> {
    pack: &'a Pack<'a>,
    policy: Policy,
    block: Vec<usize>,
    keep: Vec<Keep>,
    blocks: Vec<Option<Block>>,
    held: u64,
    peak: u64,
    /// Per load: name, the highest bytes held while it was current.
    scenes: Vec<(String, u64)>,
    reads: u32,
    copied: Vec<u64>,
    base: Vec<u32>,
    base_in_battle: bool,
    p: Placement,
    want: Vec<bool>,
    ids: Vec<u32>,
}

impl<'a> Sim<'a> {
    fn new(pack: &'a Pack<'a>, policy: Policy) -> Self {
        let n = pack.count() as usize;
        Self {
            pack,
            policy,
            block: alloc::vec![NO_BLOCK; n],
            keep: alloc::vec![Keep::No; n],
            blocks: Vec::new(),
            held: 0,
            peak: 0,
            scenes: Vec::new(),
            reads: 0,
            copied: Vec::new(),
            base: Vec::new(),
            base_in_battle: false,
            p: Placement::default(),
            want: Vec::new(),
            ids: Vec::new(),
        }
    }

    fn alloc(&mut self, size: u64, files: &[usize]) {
        if files.is_empty() {
            return;
        }
        let b = self.blocks.len();
        self.blocks.push(Some(Block {
            size,
            live: files.len() as u32,
        }));
        for &i in files {
            self.block[i] = b;
        }
        self.held += size;
        self.note();
    }

    fn note(&mut self) {
        self.peak = self.peak.max(self.held);
        if let Some(s) = self.scenes.last_mut() {
            s.1 = s.1.max(self.held);
        }
    }

    fn release(&mut self, i: usize) {
        let b = core::mem::replace(&mut self.block[i], NO_BLOCK);
        if b != NO_BLOCK {
            self.blocks[b].as_mut().unwrap().live -= 1;
        }
    }

    /// Reads the closure's missing files: one block per run, or one for
    /// the whole load with `extra` bytes appended for copies.
    fn read(&mut self, roots: &[u32], max_run: u32, copies: &[usize]) {
        let block = &self.block;
        place_into(
            self.pack,
            roots,
            |i| block[i as usize] != NO_BLOCK,
            Order::Ascending { max_run },
            &mut self.p,
        )
        .unwrap();
        self.reads += self.p.runs.len() as u32;
        let runs = self.p.runs.clone();
        if self.policy == Policy::Runs {
            for r in runs {
                let files: Vec<usize> = (r.first..=r.last)
                    .filter(|&i| self.p.offset[i as usize] != Placement::NONE)
                    .map(|i| i as usize)
                    .collect();
                self.alloc(u64::from(r.len.next_multiple_of(64)), &files);
            }
            return;
        }
        let mut files: Vec<usize> = runs
            .iter()
            .flat_map(|r| r.first..=r.last)
            .filter(|&i| self.p.offset[i as usize] != Placement::NONE)
            .map(|i| i as usize)
            .collect();
        let mut size = u64::from(self.p.size);
        let mut moved = 0u64;
        for &i in copies {
            let len = u64::from(self.pack.len(i as u32));
            size = size.next_multiple_of(16) + len;
            moved += len;
            self.release(i);
            files.push(i);
        }
        if !copies.is_empty() {
            self.copied.push(moved);
        }
        self.alloc(size.next_multiple_of(64), &files);
    }

    fn enter(&mut self, name: &str, roots: &[u32], within: bool) {
        self.scenes.push((String::from(name), self.held));
        super::closure(self.pack, roots, &mut self.want, &mut self.ids).unwrap();
        for i in 0..self.want.len() {
            let k = &mut self.keep[i];
            if self.want[i] {
                *k = Keep::Scene;
            } else if *k == Keep::Scene || (*k == Keep::Next && !within) {
                *k = Keep::Leaving;
            }
        }
        let copies: Vec<usize> = match self.policy {
            Policy::Runs | Policy::Pin => Vec::new(),
            Policy::Copy => (0..self.want.len())
                .filter(|&i| self.want[i] && self.block[i] != NO_BLOCK)
                .collect(),
            Policy::CopyPartial => (0..self.want.len())
                .filter(|&i| {
                    let b = self.block[i];
                    self.want[i]
                        && b != NO_BLOCK
                        && (0..self.want.len()).any(|j| self.block[j] == b && !self.want[j])
                })
                .collect(),
        };
        self.read(roots, MAX_RUN, &copies);
    }

    fn prefetch(&mut self, roots: &[u32]) {
        super::closure(self.pack, roots, &mut self.want, &mut self.ids).unwrap();
        for i in 0..self.want.len() {
            if self.want[i] && matches!(self.keep[i], Keep::No | Keep::Leaving) {
                self.keep[i] = Keep::Next;
            }
        }
        self.read(roots, MAX_BACKGROUND_RUN, &[]);
    }

    fn safe_point(&mut self) {
        for i in 0..self.keep.len() {
            if self.keep[i] == Keep::Leaving {
                self.keep[i] = Keep::No;
                self.release(i);
            }
        }
        for b in &mut self.blocks {
            if b.as_ref().is_some_and(|b| b.live == 0) {
                self.held -= b.take().unwrap().size;
            }
        }
    }

    fn run(&mut self, events: &[Event]) {
        for e in events {
            match e {
                Event::Scene(name, roots) => {
                    self.base = roots.clone();
                    self.base_in_battle = matches!(*name, "opening" | "explain" | "auto-demo");
                    self.enter(name, roots, false);
                }
                Event::Battle(roots) => {
                    let within = self.base_in_battle;
                    if !within {
                        self.base.clear();
                    }
                    let mut all = self.base.clone();
                    all.extend_from_slice(roots);
                    self.enter("battle", &all, within);
                }
                Event::SetBase(roots) => {
                    self.base = roots.clone();
                    self.base_in_battle = true;
                }
                Event::Maps(own, preview) => {
                    self.base = own.clone();
                    self.base_in_battle = false;
                    let mut all = own.clone();
                    all.extend_from_slice(preview);
                    self.enter("maps", &all, false);
                }
                Event::Preview(roots) => {
                    let mut all = self.base.clone();
                    all.extend_from_slice(roots);
                    self.enter("maps-preview", &all, true);
                }
                Event::Prefetch(roots) => self.prefetch(roots),
                Event::Frame => self.safe_point(),
            }
        }
    }
}

/// `scene_load`'s root lists over `pack`.
pub(crate) struct Roots<'a> {
    pack: &'a Pack<'a>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Opening {
    Room,
    Portraits,
    Fight(u32, &'static [u32]),
    Run,
    Cliff,
    Yamabuki,
    Jungle,
    Yoster,
    Sector,
    Standoff,
    Clash,
    Newcomers,
}

/// `ssb_game::opening::Kind::ORDER`; fights as `(gkind, FTKind)`
/// (`scene_load::scene_roots`).
const OPENING: [Opening; 19] = [
    Opening::Room,
    Opening::Portraits,
    Opening::Fight(0, &[0]),
    Opening::Fight(2, &[2]),
    Opening::Fight(4, &[5]),
    Opening::Fight(3, &[3]),
    Opening::Fight(5, &[6]),
    Opening::Fight(6, &[8]),
    Opening::Fight(1, &[1]),
    Opening::Fight(7, &[9]),
    Opening::Run,
    Opening::Cliff,
    Opening::Yamabuki,
    Opening::Jungle,
    Opening::Yoster,
    Opening::Sector,
    Opening::Standoff,
    Opening::Clash,
    Opening::Newcomers,
];

/// The opening room's figures (`mvOpeningRoomInitVars` picks them at
/// random; any two).
const ROOM_FIGURES: [u32; 2] = [2, 5];

impl<'a> Roots<'a> {
    pub(crate) fn new(pack: &'a Pack<'a>) -> Self {
        Self { pack }
    }

    /// `scene_load::pinned_figatrees`: the rest stream (RE-572 option C).
    fn pinned_figatrees(&self, kind: u32) -> Vec<u32> {
        r::clip_files(self.pack, [r::anim_kind(kind)], &r::PINNED_CLIP_SLOTS)
    }

    fn fighter_battle(&self, kinds: &[u32]) -> Vec<u32> {
        let mut v: Vec<u32> = Vec::new();
        v.extend(r::GM_COMMON);
        v.extend(r::EF_COMMON);
        v.extend(r::IT_COMMON);
        v.extend(r::FT_MANAGER);
        for &k in kinds {
            v.extend(r::FIGHTER_MAIN.get(k as usize).copied());
            v.extend(self.pinned_figatrees(k));
        }
        v
    }

    fn stage_battle(gkind: u32) -> Vec<u32> {
        let mut v: Vec<u32> = Vec::new();
        v.extend(r::ground_map(gkind));
        match gkind {
            17..=28 => v.extend(r::BONUS1),
            29..=40 => v.extend(r::BONUS2),
            11 => v.extend(r::EXPLAIN),
            14 => v.extend(r::ZAKO),
            _ => {}
        }
        v
    }

    fn battle(&self, gkind: u32, kinds: &[u32]) -> Vec<u32> {
        let mut v = self.fighter_battle(kinds);
        v.extend(Self::stage_battle(gkind));
        v
    }

    fn opening(&self, o: Opening) -> Vec<u32> {
        let mut v: Vec<u32> = Vec::new();
        let room = [12, ROOM_FIGURES[0], ROOM_FIGURES[1]];
        let kinds: &[u32] = match o {
            Opening::Room => &room,
            Opening::Run | Opening::Clash => &[0, 1, 2, 3, 5, 6, 8, 9],
            Opening::Yoster => &[6],
            Opening::Cliff => &[5],
            Opening::Standoff => &[0, 8],
            Opening::Yamabuki => &[9],
            _ => &[],
        };
        if !kinds.is_empty() {
            v.extend(r::EF_COMMON);
            v.extend(r::FT_MANAGER);
            v.extend(kinds.iter().map(|&k| r::FIGHTER_MAIN[k as usize]));
        }
        let fight = match o {
            Opening::Fight(g, k) => Some((g, k)),
            Opening::Jungle => Some((2, &[2, 3][..])),
            _ => None,
        };
        if let Some((gkind, kinds)) = fight {
            v.extend(r::GM_COMMON);
            v.extend(r::EF_COMMON);
            v.extend(r::IT_COMMON);
            v.extend(r::FT_MANAGER);
            v.extend(r::ground_map(gkind));
            v.extend(kinds.iter().map(|&k| r::FIGHTER_MAIN[k as usize]));
            v.extend(self.battle(gkind, kinds));
        }
        v.extend_from_slice(match o {
            Opening::Room => &r::OPENING_ROOM[..],
            Opening::Portraits => &r::OPENING_PORTRAITS,
            Opening::Fight(..) => &r::OPENING_FIGHTER,
            Opening::Run => &r::OPENING_RUN,
            Opening::Cliff => &r::OPENING_CLIFF,
            Opening::Yamabuki => &r::OPENING_YAMABUKI,
            Opening::Jungle => &r::OPENING_JUNGLE,
            Opening::Yoster => &r::OPENING_YOSTER,
            Opening::Sector => &r::OPENING_SECTOR,
            Opening::Standoff => &r::OPENING_STANDOFF,
            Opening::Clash => &r::OPENING_CLASH,
            Opening::Newcomers => &r::OPENING_NEWCOMERS,
        });
        v
    }

    fn players(&self, files: &[u32]) -> Vec<u32> {
        let mut v = files.to_vec();
        v.extend(r::EF_COMMON);
        v.extend(r::FT_MANAGER);
        v.extend(&r::FIGHTER_MAIN[..12]);
        v.extend(r::clip_files(self.pack, 0..12, &r::SELECT_CLIP_SLOTS));
        v
    }

    fn characters() -> Vec<u32> {
        let mut v = r::CHARACTERS.to_vec();
        v.extend(r::EF_COMMON);
        v.extend(r::FT_MANAGER);
        v.extend(&r::FIGHTER_MAIN[..12]);
        v
    }

    /// `scene_load::maps_roots`: the stage select's files and the preview
    /// of `gkind`, the stage its cursor opens on.
    fn maps(training: bool, gkind: u8) -> Vec<u32> {
        let mut v = r::MAPS.to_vec();
        v.extend(Self::preview(training, gkind));
        v
    }

    /// `scene_load::maps_preview_roots`.
    pub(crate) fn preview(training: bool, gkind: u8) -> Vec<u32> {
        r::maps_preview(gkind, training).map_or_else(Vec::new, |f| f.to_vec())
    }

    /// `open_stage_select` on `from` (`scene_load::maps`), the battle's
    /// fighters read ahead, then the cursor over `path` (the random slot is
    /// `0xDE`), each landing reading its preview.
    fn stage_select(e: &mut Vec<Event>, training: bool, from: u8, path: &[u8], fighters: Vec<u32>) {
        e.push(Event::Maps(r::MAPS.to_vec(), Self::preview(training, from)));
        e.push(Event::Prefetch(fighters));
        e.push(Event::Frame);
        for &g in path {
            e.push(Event::Preview(Self::preview(training, g)));
            e.push(Event::Frame);
        }
    }

    fn results(&self) -> Vec<u32> {
        let mut v = r::VS_RESULTS.to_vec();
        v.extend(r::EF_COMMON);
        v.extend(r::FT_MANAGER);
        v.extend(&r::FIGHTER_MAIN[..12]);
        v.extend(r::TRANSITIONS);
        v.extend(r::clip_files(self.pack, 0..12, &r::RESULTS_CLIP_SLOTS));
        v
    }

    /// `scene_load::campaign`'s intro (0) and stage clear (2).
    fn campaign(&self, id: u8, fkind: u32) -> Vec<u32> {
        let mut v = Vec::new();
        if id == 0 {
            v.extend(r::ONE_P_INTRO);
            v.extend(r::EF_COMMON);
            v.extend(r::FT_MANAGER);
            v.extend(r::FIGHTER_MAIN.get(fkind as usize).copied());
        } else {
            v.extend(r::ONE_P_STAGE_CLEAR);
        }
        v
    }

    /// The N64 logo, the opening's 19 scenes, then the attract loop:
    /// title, How to Play, Characters, the title demo, the logo.
    pub(crate) fn attract(&self) -> Vec<Event> {
        use Event::*;
        let mut e = Vec::new();
        e.push(Scene("startup", r::STARTUP.to_vec()));
        e.push(Frame);
        for (k, &o) in OPENING.iter().enumerate() {
            e.push(Scene("opening", self.opening(o)));
            let next = OPENING
                .get(k + 1)
                .map_or_else(|| r::TITLE.to_vec(), |&n| self.opening(n));
            e.push(Prefetch(next));
            e.push(Frame);
            if let Opening::Fight(g, kinds) = o {
                e.push(Battle(self.battle(g, kinds)));
                e.push(Frame);
            }
            if o == Opening::Jungle {
                e.push(Battle(self.battle(2, &[2, 3])));
                e.push(Frame);
            }
        }
        let explain = self.battle(11, &[0, 4]);
        let mut explain_scene = r::EXPLAIN.to_vec();
        explain_scene.extend(&explain);
        // `auto_demo::next_gkind` (Pupupu first) and `candidate_kinds`
        // on a new save: the eight starters.
        let starters = [0, 1, 2, 3, 5, 6, 8, 9];
        let mut demo_next = r::AUTO_DEMO.to_vec();
        demo_next.extend(self.battle(6, &starters));
        e.push(Scene("title", r::TITLE.to_vec()));
        e.push(Prefetch(explain_scene.clone()));
        e.push(Frame);
        e.push(Scene("explain", explain_scene));
        e.push(Prefetch(Self::characters()));
        e.push(Frame);
        e.push(Battle(explain));
        e.push(Frame);
        e.push(Scene("characters", Self::characters()));
        e.push(Prefetch(demo_next));
        e.push(Frame);
        e.push(Scene("auto-demo", r::AUTO_DEMO.to_vec()));
        e.push(Prefetch(r::STARTUP.to_vec()));
        e.push(Frame);
        e.push(Battle(self.battle(6, &[0, 1, 2, 8])));
        e.push(Frame);
        e.push(Scene("startup", r::STARTUP.to_vec()));
        e.push(Prefetch(self.opening(Opening::Room)));
        e.push(Frame);
        e
    }

    /// Title, mode select, VS mode, two VS battles with results, then the
    /// 1P mode, Training, and five 1P stages.
    pub(crate) fn menus(&self) -> Vec<Event> {
        use Event::*;
        let mut e = Vec::new();
        let menu =
            |e: &mut Vec<Event>, name: &'static str, roots: Vec<u32>, ahead: Vec<Vec<u32>>| {
                e.push(Scene(name, roots));
                e.extend(ahead.into_iter().map(Prefetch));
                e.push(Frame);
            };
        let one_p_selects = || {
            alloc::vec![
                self.players(&r::PLAYERS_1P),
                self.players(&r::PLAYERS_TRAINING),
                self.players(&r::PLAYERS_BONUS),
            ]
        };
        menu(&mut e, "title", r::TITLE.to_vec(), alloc::vec![]);
        let mode_select_ahead = || {
            alloc::vec![
                r::ONE_P_MODE.to_vec(),
                r::VS_MODE.to_vec(),
                r::OPTION.to_vec(),
                r::DATA.to_vec()
            ]
        };
        menu(
            &mut e,
            "mode-select",
            r::MODE_SELECT.to_vec(),
            mode_select_ahead(),
        );
        menu(
            &mut e,
            "vs-mode",
            r::VS_MODE.to_vec(),
            alloc::vec![self.players(&r::PLAYERS_VS)],
        );
        // The select opens on the stage last picked (Peach's Castle at
        // first), passes the random slot, and rests on the battle's.
        for (from, path, gkind, kinds) in [
            (0u8, &[1u8, 0xDE, 6][..], 6u32, &[0u32, 1, 2, 3][..]),
            (6, &[3], 3, &[9, 8]),
        ] {
            menu(
                &mut e,
                "players-vs",
                self.players(&r::PLAYERS_VS),
                alloc::vec![Self::maps(false, from)],
            );
            // `open_stage_select`: the fighters, the previews the cursor
            // lands on, then the stage on dwell.
            Self::stage_select(&mut e, false, from, path, self.fighter_battle(kinds));
            e.push(Prefetch(Self::stage_battle(gkind)));
            e.push(Battle(self.battle(gkind, kinds)));
            e.push(Frame);
            e.push(Prefetch(self.results()));
            menu(&mut e, "results", self.results(), alloc::vec![]);
        }
        menu(
            &mut e,
            "mode-select",
            r::MODE_SELECT.to_vec(),
            mode_select_ahead(),
        );
        menu(&mut e, "1p-mode", r::ONE_P_MODE.to_vec(), one_p_selects());
        // Training.
        menu(
            &mut e,
            "players-training",
            self.players(&r::PLAYERS_TRAINING),
            alloc::vec![Self::maps(true, 0)],
        );
        let mut fighters = self.fighter_battle(&[5, 2]);
        fighters.extend(r::TRAINING);
        Self::stage_select(&mut e, true, 0, &[5, 4], fighters);
        e.push(Prefetch(Self::stage_battle(4)));
        let mut training = self.battle(4, &[5, 2]);
        training.extend(r::TRAINING);
        e.push(Battle(training));
        e.push(Frame);
        menu(
            &mut e,
            "players-training",
            self.players(&r::PLAYERS_TRAINING),
            alloc::vec![Self::maps(true, 4)],
        );
        // The 1P Game as Mario: Link, the Yoshi team, Fox, the Mario
        // Bros., Kirby's team, the Polygons (`campaign::start_stage`).
        menu(&mut e, "1p-mode", r::ONE_P_MODE.to_vec(), one_p_selects());
        menu(
            &mut e,
            "players-1p",
            self.players(&r::PLAYERS_1P),
            alloc::vec![],
        );
        let stages: [(u32, &[u32], Vec<u32>); 6] = [
            (4, &[0, 5], Vec::new()),
            (5, &[0, 6, 6, 6], Vec::new()),
            (1, &[0, 1], Vec::new()),
            (8, &[0, 0, 4], Vec::new()),
            (6, &[0, 8, 8, 8], r::KIRBY_TEAM.to_vec()),
            (14, &[0, 14, 15, 16], {
                let mut v = Vec::new();
                for k in 14..=25 {
                    v.extend(r::FIGHTER_MAIN.get(k as usize).copied());
                    v.extend(self.pinned_figatrees(k));
                }
                v
            }),
        ];
        for (gkind, kinds, base) in stages {
            menu(&mut e, "1p-intro", self.campaign(0, 0), alloc::vec![]);
            e.push(SetBase(base));
            e.push(Battle(self.battle(gkind, kinds)));
            e.push(Frame);
            menu(&mut e, "1p-stage-clear", self.campaign(2, 0), alloc::vec![]);
        }
        e
    }
}

#[test]
#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]
fn kept_file_policy_by_simulation() {
    let Some(bytes) = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/generated/ssb64.pak"),
    )
    .ok() else {
        return;
    };
    let pack = Pack::open(&bytes).unwrap();
    let roots = Roots::new(&pack);
    let mut events = roots.attract();
    events.extend(roots.attract());
    events.extend(roots.menus());
    let policies = [Policy::Runs, Policy::Pin, Policy::Copy, Policy::CopyPartial];
    let sims: Vec<Sim<'_>> = policies
        .iter()
        .map(|&policy| {
            let mut s = Sim::new(&pack, policy);
            s.run(&events);
            s
        })
        .collect();
    let base = &sims[0];
    // The whole-heap peak is `vs4@2400`'s (STATUS.md); the rest of the heap
    // is taken as constant, so a policy's scene-block peak is compared with
    // today's scene by scene.
    for s in &sims {
        let worse: Vec<(usize, &str, i64)> = s
            .scenes
            .iter()
            .zip(&base.scenes)
            .enumerate()
            .filter(|(_, (a, b))| a.1 > b.1)
            .map(|(k, (a, b))| (k, a.0.as_str(), a.1 as i64 - b.1 as i64))
            .collect();
        let max_copy = s.copied.iter().copied().max().unwrap_or(0);
        println!(
            "{:?}: peak {} B, reads {}, loads {}, copies {} B in {} loads (largest {} B), {} loads above today's peak (worst {:?})",
            s.policy,
            s.peak,
            s.reads,
            s.scenes.len(),
            s.copied.iter().sum::<u64>(),
            s.copied.len(),
            max_copy,
            worse.len(),
            worse.iter().max_by_key(|w| w.2)
        );
        for (k, name, d) in worse.iter().take(12) {
            println!(
                "    load {k} {name}: +{d} B (today {} B)",
                base.scenes[*k].1
            );
        }
    }
    if std::env::var_os("SSB64_SIM_LOADS").is_some() {
        for (k, loads) in base.scenes.iter().enumerate() {
            println!(
                "  load {k:3} {:16} runs {:9} pin {:9} copy {:9} copy-partial {:9}",
                loads.0, loads.1, sims[1].scenes[k].1, sims[2].scenes[k].1, sims[3].scenes[k].1
            );
        }
    }
    // Plan rule 2.2.
    let pin_ok = sims[1]
        .scenes
        .iter()
        .zip(&base.scenes)
        .all(|(a, b)| a.1 <= b.1);
    let copy_ok = |s: &Sim<'_>| s.copied.iter().all(|&c| c < COPY_LIMIT);
    let choice = if pin_ok {
        "P"
    } else if copy_ok(&sims[3]) {
        "C (partial blocks)"
    } else if copy_ok(&sims[2]) {
        "C"
    } else {
        "per-run blocks for kept files"
    };
    println!("policy: {choice}");
    assert_eq!(
        base.reads, sims[1].reads,
        "one block per load reads the same runs"
    );
    assert_eq!(
        choice, "per-run blocks for kept files",
        "the measured choice; revisit with the numbers"
    );
}
