//! The VS countdown's traffic light and the "GO!" letters —
//! `ifCommonCountdownMakeInterface`, `ifCommonCountdownThread`,
//! `ifCommonTrafficMakeSObj` and `ifCommonAnnounceGoMakeInterface` in
//! `if/ifcommon.c`.
//!
//! The light slides down for 60 ticks, lights red, yellow, then blue at
//! 120, 180 and 240, turns every lamp blue at "Go" (300), and slides back up
//! from 360. Each new lamp pops in at three times its size and shrinks 0.2
//! per tick. The sprites are file 82's (`ssb_rom::sprite::GAME_STATUS`), and
//! [`Sprite`] indexes that list.

/// `SP_TEXSHUF | SP_TRANSPARENT`.
pub const ATTR_TRANSPARENT: u16 = 0x0201;
/// `SP_CLOUD | SP_TEXSHUF`.
pub const ATTR_CLOUD: u16 = 0x1200;

/// An index into `ssb_rom::sprite::GAME_STATUS`'s offsets.
pub type Sprite = u8;
pub const LETTER_G: Sprite = 0;
pub const LETTER_O: Sprite = 1;
pub const EXCLAMATION: Sprite = 2;
pub const ROD: Sprite = 12;
pub const FRAME: Sprite = 13;
pub const ROD_SHADOW: Sprite = 14;

/// `dIFCommonTrafficSpriteOffsets` by colour id: red, yellow and blue dim,
/// contour, then light.
const TRAFFIC_SPRITES: [Sprite; 9] = [15, 16, 17, 21, 22, 23, 18, 19, 20];
/// `dIFCommonTrafficSpriteColors{R,G,B}`.
const TRAFFIC_R: [u8; 9] = [0xFE, 0xFF, 0x4B, 0xFF, 0xFF, 0x22, 0xFF, 0xFF, 0xFF];
const TRAFFIC_G: [u8; 9] = [0x0C, 0xA2, 0x64, 0x38, 0xA2, 0x66, 0xFF, 0xFF, 0xFF];
const TRAFFIC_B: [u8; 9] = [0x0C, 0x00, 0xFF, 0x38, 0x00, 0xFE, 0xFF, 0xFF, 0xFF];
/// `dIFCommonTrafficGoBacklight{R,G,B}` and `...GoShadow{R,G,B}`: the rod
/// shadow's primitive and environment colours before and at "Go".
const BACKLIGHT: [[u8; 3]; 2] = [[0, 0, 0], [0x6A, 0x6A, 0x95]];
const SHADOW: [[u8; 3]; 2] = [[0, 0, 0], [0x12, 0x12, 0x2E]];
/// `dIFCommonTrafficSpriteData`: position and colour id.
const TRAFFIC: [(f32, f32, u8); 15] = [
    (123.0, -13.0, 0),
    (140.0, -11.0, 1),
    (153.0, -11.0, 1),
    (166.0, -11.0, 1),
    (180.0, -15.0, 2),
    (107.0, -33.0, 3),
    (119.0, 21.0, 4),
    (132.0, 21.0, 4),
    (145.0, 21.0, 4),
    (162.0, 20.0, 5),
    (115.0, -26.0, 6),
    (131.0, 32.0, 7),
    (144.0, 32.0, 7),
    (157.0, 32.0, 7),
    (167.0, 23.0, 8),
];
/// `dIFCommonAnnounceGoSpriteData`.
const GO: [(f32, f32, Sprite); 3] = [
    (82.0, 93.0, LETTER_G),
    (144.0, 93.0, LETTER_O),
    (214.0, 93.0, EXCLAMATION),
];

/// `dIFCommonAnnounceSuddenDeathSpriteData`: "SUDDEN DEATH!" from file 37
/// (`ssb_rom::sprite::ANNOUNCE_COMMON`: A is 0, "!" 26), primitive white
/// and environment black (`dIFCommonAnnounceSuddenDeathSpriteColors`).
pub const SUDDEN_DEATH: [(f32, f32, u8); 12] = [
    (74.0, 67.0, 18),
    (102.0, 67.0, 20),
    (132.0, 67.0, 3),
    (163.0, 67.0, 3),
    (193.0, 67.0, 4),
    (217.0, 67.0, 13),
    (83.0, 113.0, 3),
    (113.0, 113.0, 4),
    (135.0, 113.0, 0),
    (165.0, 113.0, 19),
    (192.0, 113.0, 7),
    (227.0, 113.0, 26),
];

/// The countdown thread's slide per tick.
const SLIDE: f32 = 0.883_333_3;
/// `I_SEC_TO_TICS(n)`.
const fn sec(n: u32) -> u32 {
    n * 60
}

/// One `SObj` of the countdown.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SObj {
    pub sprite: Sprite,
    pub pos: (f32, f32),
    pub scale: f32,
    /// `sprite.red`, `.green`, `.blue`.
    pub prim: [u8; 3],
    pub env: [u8; 3],
    pub attr: u16,
    /// The rod shadow, which moves to the list's end at "Go".
    rod_shadow: bool,
}

const MAX: usize = 12;

/// The countdown's `GObj`: its `SObj` list in draw order and its thread.
#[derive(Debug, Clone, PartialEq)]
pub struct Countdown {
    list: [Option<SObj>; MAX],
    len: usize,
    /// `ifGetSObj`: the lamp the next change replaces.
    current: usize,
    /// The thread's `timer`.
    pub timer: u32,
    lamp_status: i32,
    scale: f32,
    /// Ejected at the end of its slide.
    pub done: bool,
    /// Ticks the "GO!" letters have left (`ifCommonAnnounceThread`).
    go_wait: u32,
    /// "SUDDEN DEATH!" shows until its thread makes "GO!".
    pub sudden_death_letters: bool,
}

fn traffic(id: usize) -> SObj {
    let (x, y, color) = TRAFFIC[id];
    let c = usize::from(color);
    SObj {
        sprite: TRAFFIC_SPRITES[c],
        pos: (x, y),
        scale: 1.0,
        prim: [TRAFFIC_R[c], TRAFFIC_G[c], TRAFFIC_B[c]],
        env: [0; 3],
        attr: ATTR_CLOUD,
        rod_shadow: false,
    }
}

fn plain(sprite: Sprite, x: f32, y: f32) -> SObj {
    SObj {
        sprite,
        pos: (x, y),
        scale: 1.0,
        prim: [0xFF; 3],
        env: [0; 3],
        attr: ATTR_TRANSPARENT,
        rod_shadow: false,
    }
}

impl Default for Countdown {
    fn default() -> Countdown {
        Countdown::new()
    }
}

impl Countdown {
    /// `ifCommonCountdownMakeInterface`.
    pub fn new() -> Countdown {
        let mut c = Countdown {
            list: [None; MAX],
            len: 0,
            current: 0,
            timer: 0,
            lamp_status: -1,
            scale: 1.0,
            done: false,
            go_wait: 0,
            sudden_death_letters: false,
        };
        c.push(plain(ROD, 103.0, -57.0));
        c.push(plain(FRAME, 111.0, -23.0));
        for i in 0..6 {
            c.current = c.push(traffic(i));
        }
        c.push(traffic(10));
        let mut shadow = plain(ROD_SHADOW, 182.0, -11.0);
        shadow.prim = BACKLIGHT[0];
        shadow.env = SHADOW[0];
        shadow.rod_shadow = true;
        c.push(shadow);
        c
    }

    /// `ifCommonSuddenDeathMakeInterface`'s side: no traffic light, only
    /// the "GO!" letters once [`Countdown::start_go`] runs.
    pub fn sudden_death() -> Countdown {
        let mut c = Countdown::new();
        c.done = true;
        c.sudden_death_letters = true;
        c
    }

    /// `ifCommonAnnounceGoMakeInterface` from `ifCommonSuddenDeathThread`.
    pub fn start_go(&mut self) {
        self.go_wait = 60;
        // `gcEjectGObj(NULL)`: the thread's own letters go.
        self.sudden_death_letters = false;
        crate::sound::play_fgm(crate::sound::id::nSYAudioVoiceAnnounceGo);
    }

    fn push(&mut self, s: SObj) -> usize {
        self.list[self.len] = Some(s);
        self.len += 1;
        self.len - 1
    }

    /// `gcEjectSObj`: the list closes up behind it.
    fn remove(&mut self, i: usize) {
        for k in i..self.len - 1 {
            self.list[k] = self.list[k + 1];
        }
        self.len -= 1;
        self.list[self.len] = None;
    }

    fn slide(&mut self, dy: f32) {
        for s in self.list[..self.len].iter_mut().flatten() {
            s.pos.1 += dy;
        }
    }

    /// One tick of `ifCommonCountdownThread`, and of the "GO!" letters'
    /// `ifCommonAnnounceThread`. `sizes[sprite]` is each file 82 sprite's
    /// width and height, which a lamp's pop-in centres on.
    pub fn tick(&mut self, sizes: &[(u16, u16)]) {
        if self.go_wait > 0 {
            self.go_wait -= 1;
        }
        if self.done {
            return;
        }
        let t = self.timer;
        if t < 60 {
            self.slide(SLIDE);
        } else if t < sec(6) {
            let mut changed = true;
            match t {
                x if x == sec(2) => {
                    self.lamp_status = 6;
                    crate::sound::play_fgm(crate::sound::id::nSYAudioVoiceAnnounceThree);
                }
                x if x == sec(3) => {
                    self.lamp_status = 7;
                    crate::sound::play_fgm(crate::sound::id::nSYAudioVoiceAnnounceTwo);
                }
                x if x == sec(4) => {
                    self.lamp_status = 8;
                    crate::sound::play_fgm(crate::sound::id::nSYAudioVoiceAnnounceOne);
                }
                x if x == sec(5) => {
                    // `ifCommonAnnounceGoMakeInterface`: 60 ticks on screen.
                    self.go_wait = 60;
                    self.lamp_status = 9;
                    crate::sound::play_fgm(crate::sound::id::nSYAudioVoiceAnnounceGo);
                }
                _ => changed = false,
            }
            if self.lamp_status != -1 {
                let lamp = self.lamp_status as usize;
                if changed {
                    // `gcEjectSObj(sobj->next)`, then `sobj`.
                    self.remove(self.current + 1);
                    self.remove(self.current);
                    let mut s = traffic(lamp);
                    s.scale = 3.0;
                    self.scale = 3.0;
                    self.current = self.push(s);
                    self.push(traffic(lamp + 5));
                    if lamp == 9 {
                        // The rod shadow lights and moves to the end.
                        if let Some(i) = self.list[..self.len]
                            .iter()
                            .position(|s| s.is_some_and(|s| s.rod_shadow))
                        {
                            let mut shadow = self.list[i].unwrap();
                            shadow.prim = BACKLIGHT[1];
                            shadow.env = SHADOW[1];
                            self.remove(i);
                            if i < self.current {
                                self.current -= 1;
                            }
                            self.push(shadow);
                        }
                    }
                }
                if self.scale != 1.0 {
                    self.scale -= 0.2;
                    if self.scale < 1.0 {
                        self.scale = 1.0;
                    }
                    let rscale = (self.scale - 1.0) * 0.5;
                    let (x, y, _) = TRAFFIC[lamp];
                    if let Some(s) = self.list[self.current].as_mut() {
                        let (w, h) = sizes.get(usize::from(s.sprite)).copied().unwrap_or((0, 0));
                        s.scale = self.scale;
                        s.pos = (x - rscale * f32::from(w), y - rscale * f32::from(h));
                    }
                }
            }
        } else if t < sec(7) {
            self.slide(-SLIDE);
        } else {
            self.done = true;
            return;
        }
        self.timer += 1;
    }

    /// The countdown's `SObj`s in draw order, then the "GO!" letters'.
    pub fn sobjs(&self) -> impl Iterator<Item = SObj> + '_ {
        let light = self.list[..self.len]
            .iter()
            .flatten()
            .copied()
            .filter(move |_| !self.done);
        let go = GO
            .iter()
            .filter(move |_| self.go_wait > 0)
            .map(|&(x, y, sprite)| SObj {
                sprite,
                pos: (x, y),
                scale: 1.0,
                prim: [0xFF; 3],
                env: [0; 3],
                attr: ATTR_CLOUD,
                rod_shadow: false,
            });
        light.chain(go)
    }
}

#[cfg(test)]
#[path = "countdown_tests.rs"]
mod tests;
