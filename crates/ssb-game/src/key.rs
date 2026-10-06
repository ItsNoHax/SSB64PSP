//! `ft/ftkey.c`: a fighter driven by an input script (`FTKey`), the
//! `nFTPlayerKindKey` and `nFTPlayerKindGameKey` fighters of the opening
//! movie and How to Play.
//!
//! A script is a list of big-endian halfwords (`FTKeyEvent`): an opcode in
//! the top four bits and a wait in the low twelve, then for a button event
//! one halfword of buttons and for a stick event one of `x << 8 | y`. Each
//! event sets `fp->input.cp` and waits; `ftMainProcInterrupt` then reads
//! `cp` as a computer player's (`case nFTPlayerKindGameKey`), so the fighter
//! sees the script exactly as it would a pad.

use alloc::vec::Vec;
use ssb_engine::input::{ControllerState, N64Buttons};

/// `nFTKeyEventEnd`.
const EVENT_END: u16 = 0;
/// `nFTKeyEventButton`.
const EVENT_BUTTON: u16 = 1;
/// `nFTKeyEventStick`.
const EVENT_STICK: u16 = 2;

/// Reads the script at `at` in big-endian `data` up to and including its
/// `FTKEY_EVENT_END()`, or `None` if it runs off the data.
pub fn parse(data: &[u8], at: usize) -> Option<Vec<u16>> {
    let mut out = Vec::new();
    let mut pc = at;
    loop {
        let word = u16::from_be_bytes([*data.get(pc)?, *data.get(pc + 1)?]);
        out.push(word);
        pc += 2;
        match word >> 12 {
            EVENT_END => return Some(out),
            EVENT_BUTTON | EVENT_STICK => {
                out.push(u16::from_be_bytes([*data.get(pc)?, *data.get(pc + 1)?]));
                pc += 2;
            }
            // An unknown opcode is not a script.
            _ => return None,
        }
    }
}

/// `FTKey` with the `FTComputerInput` it writes.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Key {
    script: Vec<u16>,
    /// Index of the next event; `None` once the script ended
    /// (`key->script = NULL`).
    pc: Option<usize>,
    input_wait: i32,
    /// `cp->button_inputs`.
    pub buttons: u16,
    /// `cp->stick_range`.
    pub stick: (i8, i8),
}

impl Key {
    /// `ftParamSetKey`: start `script` at once.
    pub fn new(script: Vec<u16>) -> Key {
        Key {
            script,
            pc: Some(0),
            input_wait: 0,
            buttons: 0,
            stick: (0, 0),
        }
    }

    /// `ftParamCheckHaveKey`.
    pub fn has_script(&self) -> bool {
        self.pc.is_some()
    }

    /// `ftKeyProcessKeyEvents`: count the wait down, then run every event
    /// due now.
    pub fn process(&mut self) {
        if self.pc.is_none() {
            return;
        }
        if self.input_wait != 0 {
            self.input_wait -= 1;
        }
        while let Some(pc) = self.pc {
            if self.input_wait > 0 {
                break;
            }
            let Some(&word) = self.script.get(pc) else {
                self.pc = None;
                break;
            };
            self.input_wait = i32::from(word & 0xFFF);
            match word >> 12 {
                EVENT_BUTTON => {
                    self.buttons = self.script.get(pc + 1).copied().unwrap_or(0);
                    self.pc = Some(pc + 2);
                }
                EVENT_STICK => {
                    let v = self.script.get(pc + 1).copied().unwrap_or(0);
                    self.stick = ((v >> 8) as u8 as i8, v as u8 as i8);
                    self.pc = Some(pc + 2);
                }
                _ => self.pc = None,
            }
        }
    }

    /// `cp` as `ftMainProcInterrupt` reads it.
    pub fn controller(&self) -> ControllerState {
        ControllerState {
            buttons: N64Buttons(self.buttons),
            stick_x: self.stick.0,
            stick_y: self.stick.1,
            connected: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bytes(words: &[u16]) -> Vec<u8> {
        words.iter().flat_map(|w| w.to_be_bytes()).collect()
    }

    #[test]
    fn events_wait_then_set_the_input() {
        // Button A for 2, stick (40, -20) for 3, end.
        let data = bytes(&[0x1002, 0x8000, 0x2003, 0x28EC, 0x0000, 0x0000]);
        let script = parse(&data, 0).unwrap();
        assert_eq!(script.len(), 5);
        let mut k = Key::new(script);
        k.process();
        assert_eq!((k.buttons, k.stick), (0x8000, (0, 0)));
        k.process();
        assert_eq!(k.stick, (0, 0));
        k.process();
        assert_eq!(k.stick, (40, -20));
        k.process();
        k.process();
        assert!(k.has_script());
        k.process();
        // The end keeps the last input.
        assert!(!k.has_script());
        assert_eq!((k.buttons, k.stick), (0x8000, (40, -20)));
    }

    #[test]
    fn a_zero_wait_runs_the_next_event_the_same_tick() {
        let data = bytes(&[0x1000, 0x0010, 0x2001, 0x0505, 0x0000]);
        let mut k = Key::new(parse(&data, 0).unwrap());
        k.process();
        assert_eq!((k.buttons, k.stick), (0x0010, (5, 5)));
    }
}
