//! The Rumble Pak: `src/gm/gmrumble.c`'s per-player script queues and its
//! actor, plus the motor calls the scenes make directly
//! (`syController{Init,Start,Stop}Rumble`).
//!
//! - [`Rumble`] is `sGMRumbleScripts`, `sGMRumbleLinks` and
//!   `sGMRumblePlayers`: made per scene where the original calls
//!   `gmRumbleMakeActor` ([`Rumble::new`]), ticked where its actor's
//!   process runs ([`Rumble::tick`]).
//! - Fighters do not reach it: they queue [`Cmd`]s ([`Cmds`], on
//!   `Fighter::rumble`) that the match applies in order
//!   ([`crate::matchtick`]), so a KO can rumble the other players' ports.
//! - The motor is the platform's ([`Motor`], [`install`]), a global like
//!   [`crate::sound`]'s audio system. None installed: every motor call does
//!   nothing. The PSP has no motor: its sink is a documented no-op
//!   (`psp-runtime/src/input.rs`); the queues still run, observable in host
//!   tests ([`testing::Recorder`]).
//!
//! The screen quakes (`efManagerQuakeMakeEffect`, the stages' and items'
//! `rumble_wait`/`rumble_frame`) are not the Rumble Pak.

/// `GMRumbleEvent` with its 13-bit param (`GMRumbleEventDefault`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ev {
    /// `nGMRumbleEventEnd`: back to the script's start.
    End,
    /// `nGMRumbleEventStartRumble`: motor on for `param` ticks.
    Start(u16),
    /// `nGMRumbleEventStopRumble`: motor off for `param` ticks.
    Stop(u16),
    /// `nGMRumbleEventLoopBegin`: `param` passes.
    LoopBegin(u16),
    /// `nGMRumbleEventLoopEnd`.
    LoopEnd,
}

use Ev::{End, LoopBegin, LoopEnd, Start, Stop};

/// `dGMRumbleEventList` (`gmrumble.c:10-150`, the `REGION_US` arms).
pub const SCRIPTS: [&[Ev]; 11] = [
    &[Start(8000), End],
    &[LoopBegin(8000), Start(2), Stop(2), LoopEnd, End],
    &[LoopBegin(8000), Start(2), Stop(4), LoopEnd, End],
    &[LoopBegin(8000), Start(2), Stop(8), LoopEnd, End],
    &[Start(12), End],
    &[LoopBegin(8000), Start(2), Stop(1), LoopEnd, End],
    &[Start(3), End],
    &[LoopBegin(8000), Start(2), Stop(3), LoopEnd, End],
    &[LoopBegin(8000), Start(1), Stop(3), LoopEnd, End],
    &[Start(8), End],
    &[Start(16), End],
];

/// `dGMRumblePriorities`.
pub const PRIORITIES: [u8; 11] = [10, 9, 5, 5, 6, 5, 3, 3, 3, 3, 3];

/// `GMCOMMON_PLAYERS_MAX`.
pub const PLAYERS: usize = 4;
/// `GMRUMBLE_ARRAY_COLS`: scripts per player.
pub const COLS: usize = 3;

// ---------------------------------------------------------------- motor

/// The controller motor: `syControllerInitRumble`, `syControllerStartRumble`
/// and `syControllerStopRumble` (`src/sys/controller.c`), by port.
pub trait Motor {
    fn init(&self, port: u8);
    fn start(&self, port: u8);
    fn stop(&self, port: u8);
}

#[cfg(not(any(test, feature = "std")))]
static mut SINK: Option<&'static dyn Motor> = None;

#[cfg(any(test, feature = "std"))]
std::thread_local! {
    static SINK: core::cell::Cell<Option<&'static dyn Motor>> = const { core::cell::Cell::new(None) };
}

/// Installs the platform's motor. Called once, before the first scene.
pub fn install(motor: &'static dyn Motor) {
    #[cfg(not(any(test, feature = "std")))]
    // SAFETY: set once at boot on the game thread, before any reader.
    unsafe {
        SINK = Some(motor)
    };
    #[cfg(any(test, feature = "std"))]
    SINK.with(|s| s.set(Some(motor)));
}

/// Removes the installed motor (tests).
pub fn uninstall() {
    #[cfg(not(any(test, feature = "std")))]
    // SAFETY: as in `install`.
    unsafe {
        SINK = None
    };
    #[cfg(any(test, feature = "std"))]
    SINK.with(|s| s.set(None));
}

#[inline]
fn sink() -> Option<&'static dyn Motor> {
    #[cfg(not(any(test, feature = "std")))]
    // SAFETY: written only by `install`/`uninstall` on the game thread.
    return unsafe { *core::ptr::addr_of!(SINK) };
    #[cfg(any(test, feature = "std"))]
    return SINK.with(|s| s.get());
}

/// `syControllerInitRumble`.
pub fn motor_init(port: u8) {
    if let Some(m) = sink() {
        m.init(port);
    }
}

/// `syControllerStartRumble`.
pub fn motor_start(port: u8) {
    if let Some(m) = sink() {
        m.start(port);
    }
}

/// `syControllerStopRumble`.
pub fn motor_stop(port: u8) {
    if let Some(m) = sink() {
        m.stop(port);
    }
}

/// `gmRumbleInitPlayers` (and `func_ovl31_8013797C`, the scheduler's soft
/// reset): Init then Stop on every port. Only the motor: the queues and
/// each player's `is_active` stay as they were, so a script still on at a
/// pause keeps `is_active` set and the actor turns the motor back on only
/// when the head script's state next differs from it (the original's
/// quirk, kept).
pub fn init_players() {
    for port in 0..PLAYERS as u8 {
        motor_init(port);
        motor_stop(port);
    }
}

/// `mnTitleFuncStart`: Stop on every port (no Init).
pub fn stop_all() {
    for port in 0..PLAYERS as u8 {
        motor_stop(port);
    }
}

// ---------------------------------------------------------------- queues

/// `GMRumbleScript`. `goto` and `event` index the script's
/// [`SCRIPTS`] entry (`p_goto`, `p_event`); `event == None` is a free
/// slot.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Script {
    pub rumble_id: u8,
    pub is_rumble_active: bool,
    /// Ticks left on the current command.
    pub rumble_status: u16,
    pub loop_count: u16,
    /// Ticks left; -1 runs until the script reaches `End`.
    pub rumble_timer: i32,
    pub goto: Option<u8>,
    pub event: Option<u8>,
}

/// `GMRumbleLink`: link `i` always holds script `i`. `prev` of the head is
/// the tail; `next` of the tail is `None`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Link {
    next: Option<u8>,
    prev: u8,
}

/// `GMRumblePlayer` with its three links and scripts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Player {
    /// The motor as the actor last drove it.
    pub is_active: bool,
    /// `rlink`: the head link.
    head: u8,
    links: [Link; COLS],
    pub scripts: [Script; COLS],
}

impl Player {
    /// `gmRumbleMakeActor`'s per-player setup: links 0, 1, 2 in order, all
    /// free.
    const fn new() -> Player {
        Player {
            is_active: false,
            head: 0,
            links: [
                Link {
                    next: Some(1),
                    prev: 2,
                },
                Link {
                    next: Some(2),
                    prev: 0,
                },
                Link {
                    next: None,
                    prev: 1,
                },
            ],
            scripts: [Script {
                rumble_id: 0,
                is_rumble_active: false,
                rumble_status: 0,
                loop_count: 0,
                rumble_timer: 0,
                goto: None,
                event: None,
            }; COLS],
        }
    }

    /// The scripts from head to tail.
    pub fn order(&self) -> [u8; COLS] {
        let mut out = [0; COLS];
        let mut cur = Some(self.head);
        for o in out.iter_mut() {
            let l = cur.expect("three links");
            *o = l;
            cur = self.links[usize::from(l)].next;
        }
        out
    }

    /// The head script.
    pub fn head(&self) -> &Script {
        &self.scripts[usize::from(self.head)]
    }

    /// `gmRumbleAddLinkAfter`: frees `l` and moves it to the tail.
    fn add_link_after(&mut self, l: u8) {
        let rprev = self.links[usize::from(self.head)].prev;
        let rnext = self.links[usize::from(l)].next;
        self.scripts[usize::from(l)].event = None;
        if let Some(rnext) = rnext {
            let lprev = self.links[usize::from(l)].prev;
            if self.links[usize::from(lprev)].next.is_none() {
                self.head = rnext;
            } else {
                self.links[usize::from(lprev)].next = Some(rnext);
                self.links[usize::from(rnext)].prev = lprev;
            }
            self.links[usize::from(l)].next = self.links[usize::from(rprev)].next;
            self.links[usize::from(rprev)].next = Some(l);
            self.links[usize::from(l)].prev = rprev;
            let head = usize::from(self.head);
            self.links[head].prev = l;
        }
    }

    /// `gmRumbleUpdateEventLinks`: frees the scripts whose time is up.
    fn update_event_links(&mut self) {
        let mut cur = Some(self.head);
        while let Some(l) = cur {
            let s = &self.scripts[usize::from(l)];
            if s.event.is_none() {
                break;
            }
            if s.rumble_timer == 0 || (s.rumble_timer < 0 && check_end(s)) {
                self.add_link_after(l);
                cur = Some(self.head);
            } else {
                cur = self.links[usize::from(l)].next;
            }
        }
    }

    /// `gmRumbleGetMotorUpdateStatus`: an empty queue silences the motor
    /// (Init, Stop) and returns `true`; otherwise the motor follows the
    /// head script.
    fn motor_update_status(&mut self, port: u8) -> bool {
        let s = self.scripts[usize::from(self.head)];
        if s.event.is_none() {
            motor_init(port);
            motor_stop(port);
            self.is_active = false;
            return true;
        }
        if s.is_rumble_active != self.is_active {
            if self.is_active {
                motor_stop(port);
                self.is_active = false;
            } else {
                motor_start(port);
                self.is_active = true;
            }
        }
        false
    }

    /// `gmRumbleCheckSetEventID`: a running script of the same ID is
    /// extended (a longer finite time) or restarted (`timer == 0`).
    fn check_set_event_id(&mut self, rumble_id: u8, rumble_timer: i32) -> bool {
        let mut cur = Some(self.head);
        while let Some(l) = cur {
            let s = &mut self.scripts[usize::from(l)];
            if s.event.is_some() && s.rumble_id == rumble_id {
                if rumble_timer != 0 {
                    if s.rumble_timer > 0 && s.rumble_timer < rumble_timer {
                        s.rumble_timer = rumble_timer;
                        return true;
                    }
                } else {
                    s.rumble_status = 0;
                    s.loop_count = 0;
                    s.event = Some(0);
                    return true;
                }
            }
            cur = self.links[usize::from(l)].next;
        }
        false
    }

    /// `gmRumbleGetEventPriorityRelink`: the slot for a new script of
    /// `rumble_id`: the first free one, or the tail moved in front of the
    /// first running script it outranks (`>=`), which drops whatever the
    /// tail held. `None`: three scripts that all outrank it.
    fn priority_relink(&mut self, rumble_id: u8) -> Option<u8> {
        let main = self.head;
        let rprev = self.links[usize::from(main)].prev;
        let mut cur = Some(main);
        while let Some(c) = cur {
            let s = &self.scripts[usize::from(c)];
            if s.event.is_none() {
                return Some(c);
            }
            if PRIORITIES[usize::from(rumble_id)] >= PRIORITIES[usize::from(s.rumble_id)] {
                if c == main {
                    let pp = self.links[usize::from(rprev)].prev;
                    self.links[usize::from(pp)].next = self.links[usize::from(rprev)].next;
                    self.head = rprev;
                    self.links[usize::from(rprev)].next = Some(c);
                    self.links[usize::from(c)].prev = rprev;
                } else if c != rprev {
                    let pp = self.links[usize::from(rprev)].prev;
                    self.links[usize::from(pp)].next = self.links[usize::from(rprev)].next;
                    let head = usize::from(self.head);
                    self.links[head].prev = pp;
                    let cprev = self.links[usize::from(c)].prev;
                    self.links[usize::from(rprev)].prev = cprev;
                    self.links[usize::from(cprev)].next = Some(rprev);
                    self.links[usize::from(rprev)].next = Some(c);
                    self.links[usize::from(c)].prev = rprev;
                }
                return Some(rprev);
            }
            cur = self.links[usize::from(c)].next;
        }
        None
    }
}

/// `gmRumbleUpdateEventCheckEnd`: whether a script with no time limit has
/// reached `End`.
fn check_end(s: &Script) -> bool {
    let list = SCRIPTS[usize::from(s.rumble_id)];
    let Some(mut e) = s.event else {
        return false;
    };
    let mut is_terminate = s.rumble_status != 0;
    while !is_terminate {
        match list[usize::from(e)] {
            End => return true,
            Start(_) | Stop(_) => is_terminate = true,
            LoopBegin(_) => e += 1,
            LoopEnd => {
                if s.loop_count == 1 {
                    e += 1;
                } else {
                    e = s.goto.expect("a loop's start");
                }
            }
        }
    }
    false
}

/// `gmRumbleUpdateEventExecute` (`head == true`: the head script drives
/// the motor) and `gmRumbleUpdateEventQueue` (the others only track their
/// state): runs commands until one sets a duration.
fn run_events(is_active: &mut bool, s: &mut Script, port: u8, head: bool) {
    let list = SCRIPTS[usize::from(s.rumble_id)];
    while s.rumble_status == 0 {
        let i = s.event.expect("a running script");
        match list[usize::from(i)] {
            End => s.event = Some(0),
            Start(param) => {
                s.rumble_status = param;
                s.event = Some(i + 1);
                if !head {
                    s.is_rumble_active = true;
                } else if !*is_active {
                    motor_start(port);
                    *is_active = true;
                    s.is_rumble_active = true;
                }
            }
            Stop(param) => {
                s.rumble_status = param;
                s.event = Some(i + 1);
                if !head {
                    s.is_rumble_active = false;
                } else if *is_active {
                    motor_stop(port);
                    *is_active = false;
                    s.is_rumble_active = false;
                }
            }
            LoopBegin(param) => {
                s.loop_count = param;
                s.event = Some(i + 1);
                s.goto = s.event;
            }
            LoopEnd => {
                s.loop_count = s.loop_count.wrapping_sub(1);
                s.event = if s.loop_count != 0 {
                    s.goto
                } else {
                    Some(i + 1)
                };
            }
        }
    }
}

/// The countdowns after a script's commands ran.
fn count_down(s: &mut Script) {
    s.rumble_status = s.rumble_status.wrapping_sub(1);
    if s.rumble_timer > 0 {
        s.rumble_timer -= 1;
    }
}

/// The rumble actor's state (see the module docs).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rumble {
    pub players: [Player; PLAYERS],
}

impl Default for Rumble {
    fn default() -> Self {
        Self::new()
    }
}

impl Rumble {
    /// `gmRumbleMakeActor`: every player's three slots free, the motor
    /// state off. Makes no motor call.
    pub const fn new() -> Rumble {
        Rumble {
            players: [Player::new(); PLAYERS],
        }
    }

    /// `gmRumbleActorProcUpdate`, once per frame its process runs: for each
    /// port with a running head script, free the finished scripts, sync
    /// the motor to the head, run the head's commands (motor), then the
    /// others' (state only).
    pub fn tick(&mut self) {
        for (port, p) in self.players.iter_mut().enumerate() {
            let port = port as u8;
            if p.head().event.is_none() {
                continue;
            }
            p.update_event_links();
            if p.motor_update_status(port) {
                continue;
            }
            let h = p.head;
            run_events(&mut p.is_active, &mut p.scripts[usize::from(h)], port, true);
            count_down(&mut p.scripts[usize::from(h)]);
            let mut cur = p.links[usize::from(h)].next;
            while let Some(l) = cur {
                if p.scripts[usize::from(l)].event.is_none() {
                    break;
                }
                run_events(
                    &mut p.is_active,
                    &mut p.scripts[usize::from(l)],
                    port,
                    false,
                );
                count_down(&mut p.scripts[usize::from(l)]);
                cur = p.links[usize::from(l)].next;
            }
        }
    }

    /// `gmRumbleSetPlayerRumbleParams`: script `rumble_id` on `port` for
    /// `rumble_timer` ticks (0: until it ends by itself, which a looped
    /// script never does). No motor call: the next [`Rumble::tick`]
    /// drives it.
    pub fn make(&mut self, port: u8, rumble_id: u8, rumble_timer: i32) {
        // The original indexes its tables with any ID; no caller or
        // motion script passes one past them.
        let Some(p) = self.players.get_mut(usize::from(port)) else {
            return;
        };
        if usize::from(rumble_id) >= SCRIPTS.len() || p.check_set_event_id(rumble_id, rumble_timer)
        {
            return;
        }
        if let Some(l) = p.priority_relink(rumble_id) {
            let s = &mut p.scripts[usize::from(l)];
            s.rumble_timer = if rumble_timer == 0 { -1 } else { rumble_timer };
            s.rumble_id = rumble_id;
            s.is_rumble_active = false;
            s.rumble_status = 0;
            s.loop_count = 0;
            s.goto = None;
            s.event = Some(0);
        }
    }

    /// `gmRumbleStopRumbleID`: frees every running script of `rumble_id` on
    /// `port`, then syncs the motor to the new head at once.
    pub fn stop_id(&mut self, port: u8, rumble_id: u32) {
        let Some(p) = self.players.get_mut(usize::from(port)) else {
            return;
        };
        if p.head().event.is_none() {
            return;
        }
        let mut cur = Some(p.head);
        while let Some(l) = cur {
            let s = &p.scripts[usize::from(l)];
            if s.event.is_none() {
                break;
            }
            if rumble_id == u32::from(s.rumble_id) {
                p.add_link_after(l);
                cur = Some(p.head);
            } else {
                cur = p.links[usize::from(l)].next;
            }
        }
        p.motor_update_status(port);
    }

    /// Applies one fighter's queued command. `is_man[port]` and
    /// `stock_out[port]` describe the battle's other players for a KO.
    pub fn apply(&mut self, cmd: Cmd, others: &[Other; PLAYERS]) {
        match cmd {
            Cmd::Make { port, id, len } => self.make(port, id, len),
            Cmd::Stop { port, id } => self.stop_id(port, id),
            Cmd::MakeIfMan { port, id, len } => {
                if others.get(usize::from(port)).is_some_and(|o| o.is_man) {
                    self.make(port, id, len);
                }
            }
            // `ftCommonDeadUpdateRumble`'s loop over the other players
            // (`ftcommondead.c:27-43`): each Man fighter still in a stock
            // battle (or any battle without stocks).
            Cmd::KoOthers { port } => {
                for (i, o) in others.iter().enumerate() {
                    if i != usize::from(port) && o.is_man && !o.out_of_stock {
                        self.make(i as u8, 1, 15);
                    }
                }
            }
        }
    }
}

/// What a KO's rumble reads of another player: `pkind == nFTPlayerKindMan`
/// (in the battle state and the fighter, `ftParamMakeRumble`) and
/// `SCBATTLE_GAMERULE_STOCK && stock_count == -1`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Other {
    pub is_man: bool,
    pub out_of_stock: bool,
}

// ---------------------------------------------------------------- fighters

/// A fighter's call into the rumble queues, applied by the match in the
/// order the fighter made it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cmd {
    /// `gmRumbleSetPlayerRumbleParams` (already through
    /// `ftParamMakeRumble`'s Man gate).
    Make { port: u8, id: u8, len: i32 },
    /// `gmRumbleStopRumbleID`.
    Stop { port: u8, id: u32 },
    /// An item's `ftParamMakeRumble` on `players[ip->player]`'s fighter
    /// (`itmball.c:390-400`, `itmsbomb.c:336-345`): the Man gate is read
    /// when the match applies it.
    MakeIfMan { port: u8, id: u8, len: i32 },
    /// `ftCommonDeadUpdateRumble`'s rumble for the players other than
    /// `port`, which only the match can see.
    KoOthers { port: u8 },
}

/// Commands a fighter can queue between two drains: a few status changes
/// (two or three stops each) and makes in one process.
pub const CMDS_MAX: usize = 32;

/// A fighter's queued [`Cmd`]s.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cmds {
    items: [Option<Cmd>; CMDS_MAX],
    len: u8,
    /// Commands dropped for a full queue (a fighter no match drains).
    pub dropped: u16,
}

impl Default for Cmds {
    fn default() -> Self {
        Cmds {
            items: [None; CMDS_MAX],
            len: 0,
            dropped: 0,
        }
    }
}

impl Cmds {
    pub fn push(&mut self, cmd: Cmd) {
        match self.items.get_mut(usize::from(self.len)) {
            Some(slot) => {
                *slot = Some(cmd);
                self.len += 1;
            }
            None => self.dropped = self.dropped.saturating_add(1),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The queued commands in order, emptying the queue.
    pub fn take(&mut self) -> impl Iterator<Item = Cmd> {
        let items = self.items;
        let len = usize::from(self.len);
        self.items = [None; CMDS_MAX];
        self.len = 0;
        items.into_iter().take(len).flatten()
    }
}

/// `ftParamMakeRumble`: only a Man fighter rumbles its port.
pub fn make(f: &mut crate::fighter::Fighter, id: u8, len: i32) {
    if f.is_man {
        let port = f.port;
        f.rumble.push(Cmd::Make { port, id, len });
    }
}

/// `gmRumbleStopRumbleID(fp->player, id)`, which the callers gate on
/// `pkind != nFTPlayerKindDemo`.
pub fn stop(f: &mut crate::fighter::Fighter, id: u32) {
    if !f.is_demo {
        let port = f.port;
        f.rumble.push(Cmd::Stop { port, id });
    }
}

/// Whether `ftMainSetStatus` from `from` into `to` carries
/// `FTSTATUS_PRESERVE_RUMBLE` ([`crate::colanim::PRESERVE_RUMBLE`], and the
/// Hammer's switches within its Wait and Walk statuses,
/// `ftHammerGetStatUpdateFlags`).
pub fn preserved(
    kind: crate::fighter::FighterKind,
    from: crate::status::AnyStatus,
    to: crate::status::AnyStatus,
) -> bool {
    (crate::item_use::is_hammer(from) && crate::item_use::is_hammer(to))
        || crate::colanim::preserved_in(&crate::colanim::PRESERVE_RUMBLE, kind, from, to)
}

/// `(s32)((x * 0.75F) + 4.0F)`: a hit's rumble length for the fighter hit.
pub fn damage_len(damage: i32) -> i32 {
    (damage as f32 * 0.75 + 4.0) as i32
}

/// `(s32)((x * 0.5F) + 2.0F)`: a hit's rumble length for the attacker.
pub fn attack_len(damage: i32) -> i32 {
    (damage as f32 * 0.5 + 2.0) as i32
}

/// `ftParamMakeRumble(fp, 0, damage_len)` if positive
/// (`ftMainProcessHitCollisionStatsMain`, `ftCommonThrown2`).
pub fn on_damage(f: &mut crate::fighter::Fighter, damage: i32) {
    let len = damage_len(damage);
    if len > 0 {
        make(f, 0, len);
    }
}

/// `ftParamMakeRumble(fp, 5, attack_len)` if positive.
pub fn on_attack(f: &mut crate::fighter::Fighter, damage: i32) {
    let len = attack_len(damage);
    if len > 0 {
        make(f, 5, len);
    }
}

// ---------------------------------------------------------------- results

/// `func_ovl31_80137898`: the VS results' pulse on the winner's port
/// (link 15, a thread). It wakes every frame from the first: odd tics
/// Start, even tics Stop, and at tic 120 Init, Stop and its end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WinnerPulse {
    pub winner: u8,
    pub tic: u32,
}

impl WinnerPulse {
    pub fn new(winner: u8) -> WinnerPulse {
        WinnerPulse { winner, tic: 0 }
    }

    /// One wake. Returns `false` once the thread has ended.
    pub fn tick(&mut self) -> bool {
        self.tic += 1;
        if self.tic == 120 {
            motor_init(self.winner);
            motor_stop(self.winner);
            return false;
        }
        if !self.tic.is_multiple_of(2) {
            motor_start(self.winner);
        } else {
            motor_stop(self.winner);
        }
        true
    }
}

#[cfg(test)]
pub mod testing {
    use super::Motor;
    use std::sync::Mutex;
    use std::vec::Vec;

    /// One motor call.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Call {
        Init(u8),
        Start(u8),
        Stop(u8),
    }

    /// Records the motor calls in order.
    #[derive(Default)]
    pub struct Recorder {
        calls: Mutex<Vec<Call>>,
    }

    impl Recorder {
        /// Leaks a fresh recorder and installs it for this thread.
        pub fn install() -> &'static Recorder {
            let r: &'static Recorder = Box::leak(Box::default());
            super::install(r);
            r
        }

        /// Takes the calls recorded so far.
        pub fn take(&self) -> Vec<Call> {
            core::mem::take(&mut self.calls.lock().unwrap())
        }
    }

    impl Motor for Recorder {
        fn init(&self, port: u8) {
            self.calls.lock().unwrap().push(Call::Init(port));
        }
        fn start(&self, port: u8) {
            self.calls.lock().unwrap().push(Call::Start(port));
        }
        fn stop(&self, port: u8) {
            self.calls.lock().unwrap().push(Call::Stop(port));
        }
    }
}

#[cfg(test)]
#[path = "rumble_tests.rs"]
mod tests;
