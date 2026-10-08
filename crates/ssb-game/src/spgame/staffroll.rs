//! `sc/sccommon/scstaffroll.c`: the staff roll.
//!
//! Job titles and staff names fly along a path towards the camera as rows
//! of textured letters. A crosshair drops in, then follows the stick; the
//! camera's look-at point follows the crosshair. A or B over a name
//! highlights it, frames it and shows its role and company in the text box;
//! B also pauses the roll until A, B, Z or START. START switches between
//! the slow and fast roll speeds. The last name's arrival blacks the screen
//! out and the scene ends 60 frames later.
//!
//! The scene's objects run as the source's processes do: every `func_run`
//! (`scStaffrollFuncRun`) first, then one queue of priority-1 processes in
//! creation order ([`Proc`]). Threads become small state machines that stop
//! where the source sleeps. `scStaffrollFuncDraw`'s scene-end check runs at
//! the end of [`Staffroll::tick`]; its blackout shows from the next frame.
//!
//! The credits text lives in the overlay's `.data` ([`Credits`], parsed
//! from the ROM bytes the pack carries); the names' path
//! (`llSCStaffrollInterpolation`) and tilt (`llSCStaffrollAnimJoint`) are
//! file 195's, sampled through [`NameMotion`]. The screen is 640 x 480
//! (`dSCStaffrollVideoSetup`); every 2D position here is in that space.
use alloc::vec::Vec;
use ssb_engine::input::{ControllerState, N64Buttons};

/// `GMSTAFFROLL_ASCII_LETTER_TO_FONT_INDEX`.
pub const fn letter(c: u8) -> i32 {
    if c > b'Z' {
        c as i32 - 0x47
    } else {
        c as i32 - 0x41
    }
}
/// `GMSTAFFROLL_ASCII_NUMBER_TO_PARA_FONT_INDEX`.
pub const fn number(c: u8) -> i32 {
    0x35 + (b'9' as i32 - c as i32)
}
pub const SPACE: i32 = letter(b' ');
pub const NEWLINE: i32 = letter(b'\n');
pub const COLON: i32 = 0x34;
pub const PERIOD: i32 = 0x3F;
pub const DASH: i32 = 0x40;
pub const COMMA: i32 = 0x41;
pub const AMPERSAND: i32 = 0x42;
pub const DOUBLE_QUOTES: i32 = 0x43;
pub const SLASH: i32 = 0x44;
pub const APOSTROPHE: i32 = 0x45;
pub const QUESTION_MARK: i32 = 0x46;
pub const OPEN_PARENTHESIS: i32 = 0x47;
pub const CLOSE_PARENTHESIS: i32 = 0x48;
pub const E_ACCENT: i32 = 0x49;

/// `dSCStaffrollNameAndJobSpriteInfo` (US): each letter quad's half width
/// and half height, by name/job font index.
pub const NAME_GLYPHS: [[u8; 2]; 56] = [
    [20, 22],
    [15, 22],
    [15, 22],
    [18, 22],
    [13, 22],
    [13, 22],
    [19, 22],
    [18, 22],
    [7, 22],
    [11, 22],
    [18, 22],
    [13, 22],
    [23, 22],
    [19, 22],
    [22, 22],
    [15, 22],
    [23, 22],
    [16, 22],
    [15, 22],
    [15, 22],
    [16, 22],
    [18, 22],
    [25, 22],
    [19, 22],
    [16, 22],
    [19, 22],
    [14, 18],
    [16, 22],
    [14, 18],
    [16, 22],
    [16, 18],
    [11, 22],
    [15, 22],
    [15, 22],
    [6, 22],
    [8, 26],
    [16, 22],
    [6, 22],
    [20, 18],
    [15, 18],
    [18, 18],
    [16, 22],
    [16, 22],
    [11, 18],
    [13, 18],
    [11, 22],
    [15, 18],
    [14, 18],
    [21, 18],
    [15, 18],
    [16, 22],
    [14, 17],
    [7, 7],
    [8, 11],
    [8, 11],
    [16, 22],
];

/// `dSCStaffrollTextBoxSpriteInfo` (US): each text box glyph's advance and
/// height, by text font index.
pub const TEXT_GLYPHS: [[u8; 2]; 74] = [
    [12, 14],
    [12, 14],
    [12, 14],
    [12, 14],
    [12, 14],
    [12, 14],
    [12, 14],
    [12, 14],
    [5, 14],
    [12, 14],
    [12, 14],
    [12, 14],
    [14, 14],
    [12, 14],
    [12, 14],
    [12, 14],
    [13, 14],
    [12, 14],
    [12, 14],
    [13, 14],
    [12, 14],
    [14, 14],
    [14, 14],
    [12, 14],
    [13, 14],
    [14, 14],
    [10, 11],
    [10, 13],
    [10, 11],
    [10, 13],
    [10, 11],
    [9, 13],
    [10, 12],
    [10, 13],
    [4, 13],
    [6, 14],
    [10, 13],
    [4, 13],
    [12, 11],
    [10, 11],
    [10, 11],
    [10, 12],
    [10, 12],
    [9, 11],
    [10, 11],
    [9, 13],
    [10, 11],
    [10, 11],
    [12, 11],
    [12, 11],
    [10, 12],
    [10, 11],
    [5, 11],
    [13, 14],
    [13, 14],
    [13, 14],
    [13, 14],
    [13, 14],
    [13, 14],
    [13, 14],
    [13, 14],
    [9, 14],
    [13, 14],
    [5, 5],
    [9, 4],
    [5, 5],
    [16, 14],
    [5, 5],
    [6, 12],
    [5, 5],
    [12, 14],
    [7, 14],
    [7, 14],
    [10, 13],
];

/// `sSCStaffrollRollSpeed`'s two values.
pub const ROLL_SPEED_SLOW: f32 = 0.003_750_000_1;
pub const ROLL_SPEED_FAST: f32 = 0.049_999_997;
/// `sSCStaffrollRollBeginWait`'s limit and `sSCStaffrollRollEndWait`.
pub const ROLL_BEGIN_WAIT: i32 = 120;
pub const ROLL_END_WAIT: i32 = 60;
/// `scStaffrollMakeCamera`: the 3D camera's eye, field of view and
/// (default `dGCPerspDefault`) projection.
pub const CAMERA_EYE: [f32; 3] = [0.0, 0.0, 580.0];
pub const CAMERA_FOVY: f32 = 50.0;
pub const CAMERA_ASPECT: f32 = 4.0 / 3.0;
pub const CAMERA_NEAR: f32 = 100.0;
pub const CAMERA_FAR: f32 = 12800.0;
/// Both cameras' viewport.
pub const VIEWPORT: [f32; 4] = [20.0, 20.0, 620.0, 460.0];
/// `scStaffrollNameProcDisplay` and `scStaffrollJobProcDisplay`'s PRIM.
pub const NAME_COLOR: [u8; 3] = [0x88, 0x93, 0xFF];
pub const JOB_COLOR: [u8; 3] = [0x7F, 0x7F, 0x89];
/// The crosshair sprite (`scStaffrollMakeCrosshairGObj`).
pub const CROSSHAIR_COLOR: [u8; 3] = [0xFF, 0x00, 0x00];
pub const CROSSHAIR_SCALE: f32 = 2.0;
/// `scStaffrollMakeTextBoxBracketSObjs`.
pub const BRACKET_COLOR: [u8; 3] = [0x78, 0x6E, 0x40];
pub const BRACKET_SCALE: [f32; 2] = [2.0, 2.4];
pub const BRACKET_LEFT: [f32; 2] = [328.0, 30.0];
pub const BRACKET_RIGHT: [f32; 2] = [588.0, 30.0];
/// `dSCStaffrollTextBoxDisplayList`'s fill colour and rectangles
/// (`G_CYC_FILL`, both corners inclusive).
pub const TEXT_BOX_COLOR: [u8; 3] = [0x42, 0x3A, 0x31];
pub const TEXT_BOX_RECTS: [[i32; 4]; 4] = [
    [346, 35, 348, 164],
    [346, 35, 584, 37],
    [582, 35, 584, 164],
    [346, 162, 584, 164],
];
/// `scStaffrollHighlightProcDisplay`'s fill colour.
pub const HIGHLIGHT_COLOR: [u8; 3] = [0x80, 0x00, 0x00];
/// The role and company texts' colours.
pub const ROLE_COLOR: [u8; 3] = [0xB7, 0xBC, 0xEC];
pub const COMPANY_COLOR: [u8; 3] = [0x80, 0x40, 0x80];

/// `nSCStaffrollCompanyNull`.
pub const COMPANY_NULL: i32 = -1;

/// One `SCStaffrollText`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextInfo {
    pub start: i32,
    pub count: i32,
}

/// One `SCStaffrollJob`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Job {
    /// `prefix_id`: a job text before the title ("Chief"), or -1.
    pub prefix: i32,
    pub job: i32,
    /// The name count after which the next job rolls; -1 for none.
    pub staff_count: i32,
}

/// The overlay's credits tables, read from its `.data` (US).
#[derive(Debug, Clone, PartialEq)]
pub struct Credits {
    pub name_chars: Vec<i32>,
    pub names: Vec<TextInfo>,
    pub jobs: Vec<Job>,
    pub job_chars: Vec<i32>,
    pub job_texts: Vec<TextInfo>,
    pub role_chars: Vec<i32>,
    pub roles: Vec<TextInfo>,
    pub company_chars: Vec<i32>,
    pub companies: Vec<TextInfo>,
    pub company_ids: Vec<i32>,
}

/// `dSCStaffrollNameCharacters`'s address: where [`Credits::parse`]'s
/// bytes start. `ovl59` loads ROM 0x17F200 at 0x80131B00, so these bytes
/// are ROM 0x182960, the overlay's `.data`.
pub const DATA_VRAM: u32 = 0x8013_5260;
/// The table ends, relative to [`DATA_VRAM`] (US addresses from
/// `scstaffroll.c`'s comments).
pub const DATA_LEN: usize = 0x8013_A184 - 0x8013_5260;
const NAME_CHARS: (usize, usize) = (0x8013_5260, 0x8013_64F4);
const NAMES: (usize, usize) = (0x8013_64F4, 0x8013_6794);
const JOBS: (usize, usize) = (0x8013_679C, 0x8013_685C);
const JOB_CHARS: (usize, usize) = (0x8013_685C, 0x8013_6B10);
const JOB_TEXTS: (usize, usize) = (0x8013_6B10, 0x8013_6BA0);
const ROLE_CHARS: (usize, usize) = (0x8013_6BA0, 0x8013_9B68);
const ROLES: (usize, usize) = (0x8013_9B68, 0x8013_9E08);
const COMPANY_CHARS: (usize, usize) = (0x8013_9E08, 0x8013_9FD4);
const COMPANIES: (usize, usize) = (0x8013_9FD4, 0x8013_A02C);
const COMPANY_IDS: (usize, usize) = (0x8013_A034, 0x8013_A184);

fn words(data: &[u8], (from, to): (usize, usize)) -> Option<Vec<i32>> {
    let base = DATA_VRAM as usize;
    let bytes = data.get(from - base..to - base)?;
    Some(
        bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|w| i32::from_be_bytes(*w))
            .collect(),
    )
}

fn infos(data: &[u8], range: (usize, usize)) -> Option<Vec<TextInfo>> {
    Some(
        words(data, range)?
            .as_chunks::<2>()
            .0
            .iter()
            .map(|w| TextInfo {
                start: w[0],
                count: w[1],
            })
            .collect(),
    )
}

impl Credits {
    /// Reads the tables from the overlay's big-endian `.data`, starting
    /// at [`DATA_VRAM`].
    pub fn parse(data: &[u8]) -> Option<Credits> {
        let credits = Credits {
            name_chars: words(data, NAME_CHARS)?,
            names: infos(data, NAMES)?,
            jobs: words(data, JOBS)?
                .as_chunks::<3>()
                .0
                .iter()
                .map(|w| Job {
                    prefix: w[0],
                    job: w[1],
                    staff_count: w[2],
                })
                .collect(),
            job_chars: words(data, JOB_CHARS)?,
            job_texts: infos(data, JOB_TEXTS)?,
            role_chars: words(data, ROLE_CHARS)?,
            roles: infos(data, ROLES)?,
            company_chars: words(data, COMPANY_CHARS)?,
            companies: infos(data, COMPANIES)?,
            company_ids: words(data, COMPANY_IDS)?,
        };
        credits.is_consistent().then_some(credits)
    }

    /// Every text range lies in its character array, every glyph has a
    /// sprite, and every name has a role and a company entry.
    fn is_consistent(&self) -> bool {
        let ranges = |infos: &[TextInfo], chars: &[i32]| {
            infos.iter().all(|t| {
                t.start >= 0 && t.count >= 0 && (t.start + t.count) as usize <= chars.len()
            })
        };
        let glyphs = |chars: &[i32], n: usize, extra: &[i32]| {
            chars
                .iter()
                .all(|&c| (0..n as i32).contains(&c) || extra.contains(&c))
        };
        self.names.len() == self.roles.len()
            && self.company_ids.len() == self.names.len()
            && ranges(&self.names, &self.name_chars)
            && ranges(&self.job_texts, &self.job_chars)
            && ranges(&self.roles, &self.role_chars)
            && ranges(&self.companies, &self.company_chars)
            && glyphs(&self.name_chars, NAME_GLYPHS.len(), &[SPACE])
            && glyphs(&self.job_chars, NAME_GLYPHS.len(), &[SPACE])
            && glyphs(&self.role_chars, TEXT_GLYPHS.len(), &[SPACE, NEWLINE])
            && glyphs(&self.company_chars, TEXT_GLYPHS.len(), &[SPACE])
            && self
                .company_ids
                .iter()
                .all(|&c| c == COMPANY_NULL || (0..self.companies.len() as i32).contains(&c))
            && self.jobs.iter().all(|j| {
                (j.prefix == -1 || (0..self.job_texts.len() as i32).contains(&j.prefix))
                    && (0..self.job_texts.len() as i32).contains(&j.job)
            })
    }

    /// `scStaffrollSetTextQuetions`: every run of `pattern` in the role
    /// texts becomes question marks. The walk is the source's: one cursor
    /// through the texts in order, all but the last, restarting a partial
    /// match on a mismatch without rechecking that character.
    pub fn hide(&mut self, pattern: &[i32]) {
        let mut cbase = 0usize;
        for info in &self.roles[..self.roles.len().saturating_sub(1)] {
            let mut cadd = cbase;
            let mut k = 0usize;
            for _ in 0..info.count {
                if self.role_chars.get(cbase) == pattern.get(k) {
                    if k == 0 {
                        cadd = cbase;
                    }
                    k += 1;
                } else {
                    k = 0;
                }
                if k == pattern.len() {
                    while k != 0 {
                        if let Some(c) = self.role_chars.get_mut(cadd) {
                            *c = QUESTION_MARK;
                        }
                        cadd += 1;
                        k -= 1;
                    }
                    cadd = cbase;
                    k = 0;
                }
                cbase += 1;
            }
        }
    }

    /// `scStaffrollTryHideUnlocks` (US).
    pub fn hide_unlocks(&mut self, unlock_mask: u8) {
        use super::Unlock;
        let word = |s: &[u8]| -> Vec<i32> { s.iter().map(|&c| letter(c)).collect() };
        if unlock_mask & Unlock::Luigi.mask() == 0 {
            self.hide(&word(b"Luigi"));
        }
        if unlock_mask & Unlock::Purin.mask() == 0 {
            self.hide(&word(b"Jigglypuff"));
        }
        if unlock_mask & Unlock::Captain.mask() == 0 {
            let mut captain = word(b"C.Falcon");
            captain[1] = PERIOD;
            self.hide(&captain);
            let mut fzero = word(b"F-ZERO X");
            fzero[1] = DASH;
            self.hide(&fzero);
        }
        if unlock_mask & Unlock::Ness.mask() == 0 {
            self.hide(&word(b"Ness"));
            self.hide(&word(b"EarthBound"));
        }
        if unlock_mask & Unlock::Inishie.mask() == 0 {
            self.hide(&word(b"ClassicMario"));
        }
    }
}

/// The names' flight, from file 195.
pub trait NameMotion {
    /// `syInterpCubic(llSCStaffrollInterpolation, t)`.
    fn path(&self, t: f32) -> [f32; 3];
    /// The root's Z rotation once `gcAddDObjAnimJoint(dobj,
    /// llSCStaffrollAnimJoint, frame)` and `gcPlayAnimAll` have run.
    fn rotate_z(&self, frame: f32) -> f32;
}

/// One letter of a name or job: a child `DObj` drawing glyph `glyph`'s quad
/// at `(x, y)` in its row.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Letter {
    pub glyph: u8,
    pub x: f32,
    pub y: f32,
}

/// One 2D text box glyph (`SObj`), at its top-left corner.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Glyph {
    pub glyph: u8,
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TextState {
    Fresh,
    WaitStatus,
    Moving,
}

/// One rolling job title (link 4) or staff name (link 3), with its
/// `SCStaffrollName`.
#[derive(Debug, Clone, PartialEq)]
pub struct Text {
    serial: u32,
    /// `job_or_name`: a staff name rather than a job title.
    pub is_name: bool,
    /// `sSCStaffrollNameID` when it was made.
    pub name_id: i32,
    pub offset_x: f32,
    pub interpolation: f32,
    /// `status == -1`: the last name, whose arrival ends the roll.
    is_last: bool,
    /// `GOBJ_FLAG_HIDDEN` until the roll begins.
    pub hidden: bool,
    paused: bool,
    state: TextState,
    pub letters: Vec<Letter>,
    /// The root `DObj`'s translation and Z rotation.
    pub translate: [f32; 3],
    pub rotate_z: f32,
}

/// The highlighted name's frame (`llSCStaffrollDObjDesc`, link 2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    target: u32,
    pub translate: [f32; 3],
    pub rotate_z: f32,
    /// The child `DObj`'s X: the frame's right side.
    pub child_x: f32,
}

/// `scStaffrollHighlightProcDisplay`'s flashing lock-on corners.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Highlight {
    pub size: i32,
    round: u8,
    pub pos: [f32; 2],
}

impl Highlight {
    /// The four filled rectangles, `[x0, y0, x1, y1]` inclusive, each
    /// clamped by `scStaffrollGetLockOnPosition{X,Y}`.
    pub fn rects(&self) -> [[i32; 4]; 4] {
        let s = self.size;
        let (x, y) = (self.pos[0], self.pos[1]);
        let cx = |v: f32| (v as i32).clamp(20, 620);
        let cy = |v: f32| (v as i32).clamp(20, 460);
        let px = |k: i32| (s * k) as f32 + x;
        let py = |k: i32| (s * k) as f32 + y;
        let px2 = |k: i32| (s * k + 2) as f32 + x;
        let py2 = |k: i32| (s * k + 2) as f32 + y;
        [
            [cx(px(-30)), cy(py(-25)), cx(px2(-30)), cy(py2(45))],
            [cx(px(-30)), cy(py(-25)), cx(px2(65)), cy(py2(-25))],
            [cx(px(-30)), cy(py(45)), cx(px2(65)), cy(py2(45))],
            [cx(px(65)), cy(py(-25)), cx(px2(65)), cy(py2(45))],
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Proc {
    Crosshair,
    Scroll,
    Camera,
    Text(u32),
    Frame,
    Highlight,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum CrosshairState {
    Fresh,
    CenterIn(i32),
    Free,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Scroll {
    started: bool,
    queued_name: bool,
    job: usize,
    current: u32,
    last_name: Option<u32>,
    paused: bool,
}

pub struct Staffroll {
    credits: Credits,
    /// `sSCStaffrollNameID`.
    pub name_id: i32,
    /// `sSCStaffrollRollSpeed`.
    pub roll_speed: f32,
    /// `sSCStaffrollStatus`: 2 while the crosshair drops in, 1 while the
    /// roll waits, 0 rolling, -1 and -2 once the last name arrives.
    pub status: i32,
    pub is_paused: bool,
    roll_begin_wait: i32,
    roll_end_wait: i32,
    /// The crosshair `SObj`'s top-left corner.
    pub crosshair: [f32; 2],
    crosshair_delta: [f32; 2],
    crosshair_state: CrosshairState,
    /// The 3D camera's look-at point.
    pub camera_at: [f32; 3],
    scroll: Option<Scroll>,
    /// Jobs and names in creation order.
    pub texts: Vec<Text>,
    pub frame: Option<Frame>,
    pub highlight: Option<Highlight>,
    /// The role (link 10) and company (link 11) texts.
    pub role_text: Option<Vec<Glyph>>,
    pub company_text: Option<Vec<Glyph>>,
    /// The text box's brackets and frame, once the roll begins.
    pub text_box: bool,
    procs: Vec<Proc>,
    next_serial: u32,
    /// `SYVIDEO_FLAG_BLACKOUT`.
    pub blackout: bool,
    blackout_pending: bool,
    finished: bool,
    /// Highlights made, each with its `nSYAudioFGMTrainingSel`.
    pub highlights: u32,
}

impl Staffroll {
    /// `scStaffrollFuncStart`: the unlock-hidden texts, the crosshair, the
    /// scroll and the camera, in that order, then the music.
    pub fn new(mut credits: Credits, unlock_mask: u8) -> Self {
        credits.hide_unlocks(unlock_mask);
        crate::sound::stop_bgm_all();
        crate::sound::play_bgm(0, crate::sound::id::nSYAudioBGMStaffroll);
        Self {
            credits,
            name_id: 0,
            roll_speed: ROLL_SPEED_SLOW,
            status: 2,
            is_paused: false,
            roll_begin_wait: 0,
            roll_end_wait: ROLL_END_WAIT,
            crosshair: [0.0; 2],
            crosshair_delta: [0.0; 2],
            crosshair_state: CrosshairState::Fresh,
            camera_at: [0.0; 3],
            scroll: Some(Scroll {
                started: false,
                queued_name: true,
                job: 0,
                current: 0,
                last_name: None,
                paused: false,
            }),
            texts: Vec::new(),
            frame: None,
            highlight: None,
            role_text: None,
            company_text: None,
            text_box: false,
            procs: alloc::vec![Proc::Crosshair, Proc::Scroll, Proc::Camera],
            next_serial: 0,
            blackout: false,
            blackout_pending: false,
            finished: false,
            highlights: 0,
        }
    }

    pub fn credits(&self) -> &Credits {
        &self.credits
    }

    /// One frame. Returns `true` on the frame the scene ends.
    pub fn tick(
        &mut self,
        input: ControllerState,
        taps: N64Buttons,
        motion: &dyn NameMotion,
    ) -> bool {
        if self.finished {
            return false;
        }
        if self.blackout_pending {
            self.blackout_pending = false;
            self.blackout = true;
        }
        self.func_run(taps);
        let mut i = 0;
        while i < self.procs.len() {
            if self.run(self.procs[i], input, motion) {
                self.procs.remove(i);
            } else {
                i += 1;
            }
        }
        // `scStaffrollFuncDraw`, after `gcDrawAll`.
        if self.roll_end_wait != 0 && (self.status == -1 || self.status == -2) {
            self.roll_end_wait -= 1;
        }
        if self.roll_end_wait == 0 {
            self.finished = true;
        }
        if self.status == -1 {
            crate::sound::stop_bgm_all();
            self.blackout_pending = true;
            self.status = -2;
        }
        self.finished
    }

    /// `scStaffrollFuncRun`.
    fn func_run(&mut self, taps: N64Buttons) {
        if self.roll_end_wait != 0 && (self.status == -1 || self.status == -2) {
            return;
        }
        if self.status == 1 {
            if self.roll_begin_wait < ROLL_BEGIN_WAIT {
                self.roll_begin_wait += 1;
            } else {
                self.text_box = true;
                self.status = 0;
            }
        }
        let mut paused = self.is_paused;
        if !self.is_paused {
            paused = self.pause_status_highlight(taps);
        }
        if self.is_paused {
            paused = self.pause_status_resume(taps);
        }
        self.is_paused = paused;
        if taps.contains(N64Buttons::START) {
            self.roll_speed = if self.roll_speed == ROLL_SPEED_SLOW {
                ROLL_SPEED_FAST
            } else {
                ROLL_SPEED_SLOW
            };
        }
    }

    fn set_paused(&mut self, paused: bool) {
        if let Some(scroll) = self.scroll.as_mut() {
            scroll.paused = paused;
        }
        for t in &mut self.texts {
            t.paused = paused;
        }
    }

    /// `scStaffrollGetPauseStatusResume`.
    fn pause_status_resume(&mut self, taps: N64Buttons) -> bool {
        if taps.contains(N64Buttons::A | N64Buttons::B | N64Buttons::Z | N64Buttons::START) {
            self.set_paused(false);
            return false;
        }
        true
    }

    /// `scStaffrollGetPauseStatusHighlight`.
    fn pause_status_highlight(&mut self, taps: N64Buttons) -> bool {
        if taps.contains(N64Buttons::A | N64Buttons::B) {
            self.try_highlight();
            if taps.contains(N64Buttons::B) {
                self.set_paused(true);
                return true;
            }
        }
        false
    }

    /// `func_ovl59_8013330C`: the first name (link 3, in order) whose
    /// projected frame holds the crosshair.
    fn try_highlight(&mut self) {
        let camera = camera_matrix(self.camera_at);
        let cursor = [
            (self.crosshair[0] + 29.0) - 320.0,
            240.0 - (self.crosshair[1] + 29.0),
        ];
        let hit = self.texts.iter().filter(|t| t.is_name).find(|t| {
            let right = t.offset_x.abs() * 2.0 + 18.0;
            let bottom = -(26.0 + 4.0);
            let corners = [
                [-22.0, 28.0, 0.0],
                [-22.0, bottom, 0.0],
                [right, 28.0, 0.0],
                [right, bottom, 0.0],
            ];
            let model = mul(&tra_rot_rpy(t.translate, [0.0, 0.0, t.rotate_z]), &camera);
            let p = corners.map(|v| project(&model, v));
            let top = edge(p[0], p[2]);
            let bottom = edge(p[1], p[3]);
            let left = edge(p[0], p[1]);
            let right = edge(p[2], p[3]);
            !inside(top, cursor)
                && inside(bottom, cursor)
                && inside(left, cursor)
                && !inside(right, cursor)
        });
        let Some(hit) = hit.map(|t| (t.serial, t.name_id)) else {
            return;
        };
        self.highlights += 1;
        crate::sound::play_fgm(crate::sound::id::nSYAudioFGMTrainingSel);
        // `func_ovl59_8013202C`: make the frame once, else retarget it.
        match self.frame.as_mut() {
            Some(frame) => frame.target = hit.0,
            None => {
                self.frame = Some(Frame {
                    target: hit.0,
                    translate: [0.0; 3],
                    rotate_z: 0.0,
                    child_x: 110.0,
                });
                self.procs.push(Proc::Frame);
            }
        }
        if self.highlight.is_none() {
            self.highlight = Some(Highlight {
                size: 0,
                round: 0,
                pos: [self.crosshair[0] + 8.0, self.crosshair[1] + 20.0],
            });
            self.procs.push(Proc::Highlight);
        }
        self.role_text = Some(self.role_glyphs(hit.1));
        self.company_text = Some(self.company_glyphs(hit.1));
    }

    /// Runs one process; `true` when it ejects itself.
    fn run(&mut self, proc: Proc, input: ControllerState, motion: &dyn NameMotion) -> bool {
        match proc {
            Proc::Crosshair => {
                self.run_crosshair(input);
                false
            }
            Proc::Scroll => self.run_scroll(),
            Proc::Camera => {
                // `scStaffrollUpdateCameraAt`.
                self.camera_at[0] += self.crosshair_delta[0] * 0.25;
                self.camera_at[1] -= self.crosshair_delta[1] * 0.25;
                false
            }
            Proc::Text(serial) => self.run_text(serial, motion),
            Proc::Frame => self.run_frame(),
            Proc::Highlight => self.run_highlight(),
        }
    }

    /// `scStaffrollCrosshairThreadUpdate`.
    fn run_crosshair(&mut self, input: ControllerState) {
        match self.crosshair_state {
            CrosshairState::Fresh => {
                self.crosshair = [291.0, 0.0];
                self.crosshair[1] += 10.5;
                self.crosshair_state = CrosshairState::CenterIn(19);
            }
            CrosshairState::CenterIn(wait) => {
                if wait != 0 {
                    self.crosshair[1] += 10.5;
                    self.crosshair_state = CrosshairState::CenterIn(wait - 1);
                } else {
                    self.status = 1;
                    self.crosshair_state = CrosshairState::Free;
                    self.move_crosshair(input);
                }
            }
            CrosshairState::Free => self.move_crosshair(input),
        }
    }

    fn move_crosshair(&mut self, input: ControllerState) {
        let (sx, sy) = (i32::from(input.stick_x), i32::from(input.stick_y));
        let base = self.crosshair;
        if sx.abs() > 16 {
            self.crosshair[0] += sx as f32 * 0.125;
        }
        if sy.abs() > 16 {
            self.crosshair[1] -= sy as f32 * 0.125;
        }
        self.crosshair[0] = self.crosshair[0].clamp(32.0, 540.0);
        self.crosshair[1] = self.crosshair[1].clamp(36.0, 400.0);
        self.crosshair_delta = [self.crosshair[0] - base[0], self.crosshair[1] - base[1]];
    }

    fn text(&self, serial: u32) -> Option<&Text> {
        self.texts.iter().find(|t| t.serial == serial)
    }

    /// `scStaffrollScrollThreadUpdate`.
    fn run_scroll(&mut self) -> bool {
        let Some(mut scroll) = self.scroll else {
            return true;
        };
        if scroll.paused {
            return false;
        }
        if !scroll.started {
            scroll.started = true;
            scroll.current = self.make_job(0);
        }
        let count = self.credits.roles.len() as i32;
        if self.name_id < count {
            let threshold = if scroll.queued_name { 0.15 } else { 0.3 };
            let interpolation = self.text(scroll.current).map_or(1.0, |t| t.interpolation);
            if interpolation > threshold {
                if scroll.queued_name {
                    let serial = self.make_name();
                    scroll.current = serial;
                    scroll.last_name = Some(serial);
                    self.name_id += 1;
                    if self.credits.jobs.get(scroll.job).map(|j| j.staff_count)
                        == Some(self.name_id)
                    {
                        scroll.queued_name = false;
                    }
                } else {
                    scroll.job += 1;
                    scroll.current = self.make_job(scroll.job);
                    scroll.queued_name = true;
                }
            }
            self.scroll = Some(scroll);
            return false;
        }
        if let Some(last) = scroll.last_name {
            if let Some(t) = self.texts.iter_mut().find(|t| t.serial == last) {
                t.is_last = true;
            }
        }
        self.scroll = None;
        true
    }

    fn push_text(&mut self, is_name: bool, letters: Vec<Letter>, last_x: f32) -> u32 {
        let serial = self.next_serial;
        self.next_serial += 1;
        // `scStaffrollJobAndNameInitStruct`: the root's X less the last
        // letter's, halved.
        self.texts.push(Text {
            serial,
            is_name,
            name_id: self.name_id,
            offset_x: (0.0 - last_x) * 0.5,
            interpolation: 0.0,
            is_last: false,
            hidden: false,
            paused: false,
            state: TextState::Fresh,
            letters,
            translate: [0.0; 3],
            rotate_z: 0.0,
        });
        self.procs.push(Proc::Text(serial));
        serial
    }

    /// `scStaffrollMakeJobGObj`.
    fn make_job(&mut self, job: usize) -> u32 {
        let Some(&job) = self.credits.jobs.get(job) else {
            return self.push_text(false, Vec::new(), 0.0);
        };
        let mut letters = Vec::new();
        let mut wbase = 0.0;
        if job.prefix != -1 {
            let spacing = self.job_letters(job.prefix, 0.0, &mut letters);
            wbase = 16.0 + spacing;
        }
        let before = letters.len();
        self.job_letters(job.job, wbase, &mut letters);
        // `job_setup.dobj`: the job text's last letter.
        let last_x = letters[before..].last().map_or(0.0, |l| l.x);
        self.push_text(false, letters, last_x)
    }

    /// `scStaffrollMakeJobDObjs`: returns the spacing after the text.
    fn job_letters(&self, text: i32, mut wbase: f32, out: &mut Vec<Letter>) -> f32 {
        let info = self.credits.job_texts[text as usize];
        let chars =
            &self.credits.job_chars[info.start as usize..(info.start + info.count) as usize];
        let mut previous: Option<i32> = None;
        for &c in chars {
            if c == SPACE {
                wbase += 16.0;
                previous = Some(c);
                continue;
            }
            let [w, h] = NAME_GLYPHS[c as usize].map(f32::from);
            if let Some(p) = previous {
                wbase -= kerning(p, c);
            }
            let x = wbase + w;
            let mut y = h - 22.0;
            wbase = x + w;
            if c == letter(b'z') {
                y += 1.0;
            }
            if c == letter(b'j') {
                y = 22.0 - h;
            }
            if c == number(b'8') {
                y += 22.0;
            }
            if is_any(b"gpqy", c) {
                y = -8.0;
            }
            out.push(Letter {
                glyph: c as u8,
                x,
                y,
            });
            previous = Some(c);
        }
        wbase
    }

    /// `scStaffrollMakeNameGObjAndDObjs`.
    fn make_name(&mut self) -> u32 {
        let info = self.credits.names[self.name_id as usize];
        let chars =
            &self.credits.name_chars[info.start as usize..(info.start + info.count) as usize];
        let mut letters = Vec::new();
        let mut wbase = 0.0;
        let mut previous: Option<i32> = None;
        for &c in chars {
            if c == SPACE {
                wbase += 16.0;
                previous = Some(c);
                continue;
            }
            let [w, h] = NAME_GLYPHS[c as usize].map(f32::from);
            if let Some(p) = previous {
                wbase -= kerning(p, c);
            }
            let x = wbase + w;
            let mut y = h - 22.0;
            wbase = x + w;
            if c == letter(b'z') {
                y += 1.0;
            }
            if c == letter(b'j') {
                y = 22.0 - h;
            }
            if is_any(b"gpqy", c) {
                y = -8.0;
            }
            if c == number(b'9') {
                y -= 4.0;
            }
            letters.push(Letter {
                glyph: c as u8,
                x,
                y,
            });
            previous = Some(c);
        }
        let last_x = letters.last().map_or(0.0, |l| l.x);
        self.push_text(true, letters, last_x)
    }

    /// `scStaffrollJobAndNameThreadUpdate`.
    fn run_text(&mut self, serial: u32, motion: &dyn NameMotion) -> bool {
        let speed = self.roll_speed;
        let status = self.status;
        let Some(index) = self.texts.iter().position(|t| t.serial == serial) else {
            return true;
        };
        let t = &mut self.texts[index];
        if t.paused {
            return false;
        }
        if t.state == TextState::Fresh {
            t.interpolation = 0.0;
            t.hidden = true;
            t.state = TextState::WaitStatus;
        }
        if t.state == TextState::WaitStatus {
            if status != 0 {
                return false;
            }
            t.hidden = false;
            t.state = TextState::Moving;
        }
        if t.interpolation != 1.0 {
            t.rotate_z = motion.rotate_z(t.interpolation * 99.0);
            let pos = motion.path(t.interpolation);
            t.translate = [pos[0] + t.offset_x, pos[1] + 12.0, pos[2]];
            t.interpolation += speed;
            if t.interpolation > 1.0 {
                t.interpolation = 1.0;
            }
            return false;
        }
        if t.is_last {
            self.status = -1;
        }
        self.texts.remove(index);
        true
    }

    fn eject_frame(&mut self) {
        self.frame = None;
        self.role_text = None;
        self.company_text = None;
    }

    /// `func_ovl59_80131F34`: the frame follows its name until the name's
    /// next step would arrive, then goes with the texts.
    fn run_frame(&mut self) -> bool {
        let Some(frame) = self.frame else {
            return true;
        };
        let Some(t) = self.text(frame.target) else {
            self.eject_frame();
            return true;
        };
        if t.interpolation + self.roll_speed >= 1.0 {
            self.eject_frame();
            return true;
        }
        let (translate, rotate_z, child_x) =
            (t.translate, t.rotate_z, t.offset_x.abs() * 2.0 + 18.0);
        self.frame = Some(Frame {
            translate,
            rotate_z,
            child_x,
            ..frame
        });
        false
    }

    /// `scStaffrollHighlightThreadUpdate`: three rounds of the corners
    /// closing in from size 6 to 0, one size per tick.
    fn run_highlight(&mut self) -> bool {
        let Some(h) = self.highlight.as_mut() else {
            return true;
        };
        if h.round == 0 && h.size == 0 {
            h.round = 1;
            h.size = 5;
            return false;
        }
        if h.size != 0 {
            h.size -= 1;
            return false;
        }
        if h.round < 3 {
            h.round += 1;
            h.size = 5;
            return false;
        }
        self.highlight = None;
        true
    }

    /// `scStaffrollMakeStaffRoleTextSObjs`.
    fn role_glyphs(&self, name_id: i32) -> Vec<Glyph> {
        let mut out = Vec::new();
        let Some(info) = self.credits.roles.get(name_id as usize) else {
            return out;
        };
        let (mut wbase, mut hbase) = (350.0, 40.0);
        for &c in &self.credits.role_chars[info.start as usize..(info.start + info.count) as usize]
        {
            if c != SPACE && c != NEWLINE {
                let mut hvar = 0.0;
                if c >= letter(b'a') {
                    hvar = 3.0;
                    let tall = is_any(b"bdfhijklt", c)
                        || c == COLON
                        || (b'0'..=b'9').any(|n| c == number(n))
                        || c == AMPERSAND
                        || c == QUESTION_MARK
                        || c == E_ACCENT
                        || c == DOUBLE_QUOTES;
                    if tall {
                        hvar = 1.0;
                    }
                }
                hvar += lowered(c);
                out.push(Glyph {
                    glyph: c as u8,
                    x: wbase,
                    y: hbase + hvar,
                });
                wbase += f32::from(TEXT_GLYPHS[c as usize][0]);
            } else if c == SPACE {
                wbase += 3.0;
            } else {
                wbase = 350.0;
                hbase += 20.0;
            }
        }
        out
    }

    /// `scStaffrollMakeCompanyTextSObjs`.
    fn company_glyphs(&self, name_id: i32) -> Vec<Glyph> {
        let mut out = Vec::new();
        let Some(&company) = self.credits.company_ids.get(name_id as usize) else {
            return out;
        };
        if company == COMPANY_NULL {
            return out;
        }
        let info = self.credits.companies[company as usize];
        let mut wbase = 350.0;
        for &c in
            &self.credits.company_chars[info.start as usize..(info.start + info.count) as usize]
        {
            if c == SPACE {
                wbase += 3.0;
                continue;
            }
            let mut hvar = 0.0;
            if c >= letter(b'a') {
                hvar = 3.0;
                let tall =
                    is_any(b"bdfhijklt", c) || c == COLON || c == AMPERSAND || c == DOUBLE_QUOTES;
                if tall {
                    hvar = 1.0;
                }
            }
            if c == OPEN_PARENTHESIS || c == CLOSE_PARENTHESIS {
                hvar = 0.0;
            }
            hvar += lowered(c);
            out.push(Glyph {
                glyph: c as u8,
                x: wbase,
                y: 140.0 + hvar,
            });
            wbase += f32::from(TEXT_GLYPHS[c as usize][0]);
        }
        out
    }
}

/// The period, dash and comma sit lower in the text box.
fn lowered(c: i32) -> f32 {
    match c {
        PERIOD => 6.0,
        DASH => 2.0,
        COMMA => 7.0,
        _ => 0.0,
    }
}

/// Whether `c` is one of `set`'s letters.
fn is_any(set: &[u8], c: i32) -> bool {
    set.iter().any(|&l| c == letter(l))
}

/// The source's pair kerning between a name or job's letters.
fn kerning(previous: i32, c: i32) -> f32 {
    if (is_any(b"KTVWY", previous) && is_any(b"acegmnopqrsuvwxyz", c))
        || (is_any(b"kry", previous) && is_any(b"aeo", c))
    {
        6.0
    } else if (previous == letter(b'o') && c == letter(b's'))
        || (previous == letter(b'S') && c == letter(b'u'))
    {
        4.0
    } else {
        0.0
    }
}

type Mtx = [[f32; 4]; 4];

/// `guMtxCatF(a, b, r)`: `r = a * b`.
fn mul(a: &Mtx, b: &Mtx) -> Mtx {
    core::array::from_fn(|i| core::array::from_fn(|j| (0..4).map(|k| a[i][k] * b[k][j]).sum()))
}

/// `syMatrixTraRotRpyRScaF` with unit scale.
fn tra_rot_rpy(t: [f32; 3], r: [f32; 3]) -> Mtx {
    use ssb_engine::math::sin_cos;
    let (sinr, cosr) = sin_cos(r[0]);
    let (sinp, cosp) = sin_cos(r[1]);
    let (siny, cosy) = sin_cos(r[2]);
    [
        [cosp * cosy, cosp * siny, -sinp, 0.0],
        [
            sinr * sinp * cosy - cosr * siny,
            sinr * sinp * siny + cosr * cosy,
            sinr * cosp,
            0.0,
        ],
        [
            cosr * sinp * cosy + sinr * siny,
            cosr * sinp * siny - sinr * cosy,
            cosr * cosp,
            0.0,
        ],
        [t[0], t[1], t[2], 1.0],
    ]
}

/// `func_ovl59_80131C88`: `syMatrixLookAtF` times `syMatrixPerspFastF`.
/// The source reads its sine and cosine from a table; these are computed.
fn camera_matrix(at: [f32; 3]) -> Mtx {
    use ssb_engine::math::{sin_cos, sqrt};
    let [ex, ey, ez] = CAMERA_EYE;
    let (ux, uy, uz) = (0.0f32, 1.0f32, 0.0f32);
    let (mut lx, mut ly, mut lz) = (at[0] - ex, at[1] - ey, at[2] - ez);
    let len = -1.0 / sqrt(lx * lx + ly * ly + lz * lz);
    lx *= len;
    ly *= len;
    lz *= len;
    let (mut rx, mut ry, mut rz) = (uy * lz - uz * ly, uz * lx - ux * lz, ux * ly - uy * lx);
    let len = 1.0 / sqrt(rx * rx + ry * ry + rz * rz);
    rx *= len;
    ry *= len;
    rz *= len;
    let (mut vx, mut vy, mut vz) = (ly * rz - lz * ry, lz * rx - lx * rz, lx * ry - ly * rx);
    let len = 1.0 / sqrt(vx * vx + vy * vy + vz * vz);
    vx *= len;
    vy *= len;
    vz *= len;
    let look = [
        [rx, vx, lx, 0.0],
        [ry, vy, ly, 0.0],
        [rz, vz, lz, 0.0],
        [
            -(ex * rx + ey * ry + ez * rz),
            -(ex * vx + ey * vy + ez * vz),
            -(ex * lx + ey * ly + ez * lz),
            1.0,
        ],
    ];
    let (sin, cos) = sin_cos(CAMERA_FOVY * 0.008_726_646);
    let cot = cos / sin;
    let (near, far, scale) = (CAMERA_NEAR, CAMERA_FAR, 1.0);
    let persp = [
        [(cot / CAMERA_ASPECT) * scale, 0.0, 0.0, 0.0],
        [0.0, cot * scale, 0.0, 0.0],
        [0.0, 0.0, ((near + far) * scale) / (near - far), -scale],
        [0.0, 0.0, (2.0 * near * far * scale) / (near - far), 0.0],
    ];
    mul(&look, &persp)
}

/// `func_ovl59_80131BB0`: a point's offset from the 640 x 480 screen's
/// centre.
fn project(m: &Mtx, v: [f32; 3]) -> [f32; 2] {
    let [x, y, z] = v;
    let w = m[0][0] * x + m[1][0] * y + m[2][0] * z + m[3][0];
    let h = m[0][1] * x + m[1][1] * y + m[2][1] * z + m[3][1];
    let i = 1.0 / (m[0][3] * x + m[1][3] * y + m[2][3] * z + m[3][3]);
    [w * i * 640.0 * 0.5, h * i * 480.0 * 0.5]
}

/// `func_ovl59_80131E70(out, a.x, a.y, b.x, b.y)`: the line through two
/// projected corners.
fn edge(a: [f32; 2], b: [f32; 2]) -> [f32; 3] {
    [a[1] - b[1], b[0] - a[0], (a[0] * b[1]) - (b[0] * a[1])]
}

/// `scStaffrollCheckCursorNameOverlap`.
fn inside(line: [f32; 3], cursor: [f32; 2]) -> bool {
    line[0] * cursor[0] + line[1] * cursor[1] + line[2] >= 0.0
}
