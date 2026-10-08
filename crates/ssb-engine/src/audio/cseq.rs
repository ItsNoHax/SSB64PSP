//! The compressed MIDI sequence reader (`n_csq.c` in `n_env.c:3429-3744`):
//! `n_alCSeqNew`, `n_alCSeqNextEvent`, `__alCSeqNextDelta`,
//! `__n_alCSeqGetTrackEvent`, `__getTrackByte` (0xFE backup blocks) and
//! `__readVarLen`.
//!
//! [`CSeq`] is the `ALCSeq` struct; pointers into the sequence are byte
//! offsets into the sequence buffer the caller passes in. The buffer must be
//! mutable: a loop end rewrites its loop counter in place, exactly as the
//! N64 does in the per-player copy it DMAs the sequence into
//! (`sSYAudioBGMSequenceDatas`).

use super::evtq::{Event, MidiEvent, TempoEvent};

/// `AL_MIDI_Meta`.
const AL_MIDI_META: u8 = 0xFF;
/// `AL_MIDI_META_TEMPO`.
const AL_MIDI_META_TEMPO: u8 = 0x51;
/// `AL_MIDI_META_EOT`.
const AL_MIDI_META_EOT: u8 = 0x2F;
/// `AL_CMIDI_BLOCK_CODE`.
const AL_CMIDI_BLOCK_CODE: u8 = 0xFE;
/// `AL_CMIDI_LOOPSTART_CODE`.
const AL_CMIDI_LOOPSTART_CODE: u8 = 0x2E;
/// `AL_CMIDI_LOOPEND_CODE`.
const AL_CMIDI_LOOPEND_CODE: u8 = 0x2D;
/// `AL_MIDI_ProgramChange` / `AL_MIDI_ChannelPressure` / `AL_MIDI_NoteOn`.
const PROGRAM_CHANGE: u8 = 0xC0;
const CHANNEL_PRESSURE: u8 = 0xD0;
const NOTE_ON: u8 = 0x90;

/// Reads `buf[at]`; out of range reads 0 (on the N64 a malformed sequence
/// reads whatever RDRAM follows; the shipped sequences never do).
#[inline]
fn rd(buf: &[u8], at: usize) -> u8 {
    buf.get(at).copied().unwrap_or(0)
}

/// `ALCSeq`. Locations are byte offsets into the sequence buffer; a
/// `cur_loc` of 0 is the C's NULL (offset 0 is the header, never a track).
#[derive(Debug, Clone)]
pub struct CSeq {
    pub valid_tracks: u32,
    /// `qnpt`: quarter notes per tick, `1 / division`.
    pub qnpt: f32,
    pub last_ticks: u32,
    pub last_delta_ticks: u32,
    pub delta_flag: u32,
    pub cur_loc: [usize; 16],
    pub cur_bu_ptr: [usize; 16],
    pub cur_bu_len: [u8; 16],
    pub last_status: [u8; 16],
    pub evt_delta_ticks: [u32; 16],
    /// Diagnostic: 0xFE backup blocks entered so far (not in the C).
    pub backup_blocks: u32,
}

impl Default for CSeq {
    fn default() -> Self {
        Self {
            valid_tracks: 0,
            qnpt: 0.0,
            last_ticks: 0,
            last_delta_ticks: 0,
            delta_flag: 0,
            cur_loc: [0; 16],
            cur_bu_ptr: [0; 16],
            cur_bu_len: [0; 16],
            last_status: [0; 16],
            evt_delta_ticks: [0; 16],
            backup_blocks: 0,
        }
    }
}

impl CSeq {
    /// `n_alCSeqNew`: the `ALCMidiHdr` is 16 big-endian track offsets and
    /// the division; each valid track's first delta is read at once.
    pub fn new(buf: &[u8]) -> Self {
        let be32 = |at: usize| {
            u32::from_be_bytes([rd(buf, at), rd(buf, at + 1), rd(buf, at + 2), rd(buf, at + 3)])
        };
        let mut s = Self {
            delta_flag: 1,
            ..Self::default()
        };
        for i in 0..16 {
            let off = be32(i * 4);
            if off != 0 {
                s.valid_tracks |= 1 << i;
                s.cur_loc[i] = off as usize;
                s.evt_delta_ticks[i] = s.read_var_len(buf, i);
            } else {
                s.cur_loc[i] = 0;
            }
        }
        let division = be32(64);
        // A zero division would be a float divide by zero, which the PSP
        // traps; the shipped sequences all have one (and the C would give
        // an infinite qnpt).
        s.qnpt = if division == 0 {
            f32::INFINITY
        } else {
            1.0 / division as f32
        };
        s
    }

    /// `n_alCSeqNextEvent`: the event of the track whose next event is
    /// earliest (the lower track on ties). Only valid while
    /// [`Self::next_delta`] returns `Some` (the C reads an uninitialised
    /// track number otherwise; here track 0).
    pub fn next_event(&mut self, buf: &mut [u8]) -> Event {
        let mut first_time = u32::MAX;
        let mut first_track = 0;
        let last = self.last_delta_ticks;
        for i in 0..16 {
            if (self.valid_tracks >> i) & 1 != 0 {
                if self.delta_flag != 0 {
                    self.evt_delta_ticks[i] = self.evt_delta_ticks[i].wrapping_sub(last);
                }
                if self.evt_delta_ticks[i] < first_time {
                    first_time = self.evt_delta_ticks[i];
                    first_track = i;
                }
            }
        }
        let mut evt = self.get_track_event(buf, first_track);
        // `evt->msg.midi.ticks = firstTime` (the union's first field).
        match &mut evt {
            Event::SeqMidi(m) => m.ticks = first_time as i32,
            Event::Tempo(t) => t.ticks = first_time as i32,
            _ => {}
        }
        self.last_ticks = self.last_ticks.wrapping_add(first_time);
        self.last_delta_ticks = first_time;
        // Also after AL_SEQ_END_EVT: the C reads a varlen past the end of
        // the last track's data.
        if !matches!(evt, Event::TrackEnd) {
            let v = self.read_var_len(buf, first_track);
            self.evt_delta_ticks[first_track] = self.evt_delta_ticks[first_track].wrapping_add(v);
        }
        self.delta_flag = 1;
        evt
    }

    /// `__alCSeqNextDelta`: ticks until the next event, `None` once every
    /// track has ended.
    pub fn next_delta(&mut self) -> Option<i32> {
        if self.valid_tracks == 0 {
            return None;
        }
        let mut first_time = u32::MAX;
        let last = self.last_delta_ticks;
        for i in 0..16 {
            if (self.valid_tracks >> i) & 1 != 0 {
                if self.delta_flag != 0 {
                    self.evt_delta_ticks[i] = self.evt_delta_ticks[i].wrapping_sub(last);
                }
                if self.evt_delta_ticks[i] < first_time {
                    first_time = self.evt_delta_ticks[i];
                }
            }
        }
        self.delta_flag = 0;
        Some(first_time as i32)
    }

    /// `__n_alCSeqGetTrackEvent`.
    fn get_track_event(&mut self, buf: &mut [u8], track: usize) -> Event {
        let status = self.get_track_byte(buf, track);
        if status == AL_MIDI_META {
            let kind = self.get_track_byte(buf, track);
            if kind == AL_MIDI_META_TEMPO {
                let byte1 = self.get_track_byte(buf, track);
                let byte2 = self.get_track_byte(buf, track);
                let byte3 = self.get_track_byte(buf, track);
                self.last_status[track] = 0;
                Event::Tempo(TempoEvent {
                    ticks: 0,
                    status,
                    kind,
                    byte1,
                    byte2,
                    byte3,
                })
            } else if kind == AL_MIDI_META_EOT {
                self.valid_tracks ^= 1 << track;
                if self.valid_tracks != 0 {
                    Event::TrackEnd
                } else {
                    Event::SeqEnd
                }
            } else if kind == AL_CMIDI_LOOPSTART_CODE {
                // Two bytes, ignored.
                self.get_track_byte(buf, track);
                self.get_track_byte(buf, track);
                self.last_status[track] = 0;
                Event::LoopStart
            } else if kind == AL_CMIDI_LOOPEND_CODE {
                // Read straight from curLoc, not through __getTrackByte:
                // loop count, current count, then a 32-bit backward offset
                // from the end of the event.
                let p = self.cur_loc[track];
                let loop_ct = rd(buf, p);
                let cur_lp_ct = rd(buf, p + 1);
                if cur_lp_ct == 0 {
                    // Done looping: reset the counter for next time.
                    if let Some(b) = buf.get_mut(p + 1) {
                        *b = loop_ct;
                    }
                    self.cur_loc[track] = p + 6;
                } else {
                    if cur_lp_ct != 0xFF {
                        if let Some(b) = buf.get_mut(p + 1) {
                            *b = cur_lp_ct - 1;
                        }
                    }
                    let offset = u32::from_be_bytes([
                        rd(buf, p + 2),
                        rd(buf, p + 3),
                        rd(buf, p + 4),
                        rd(buf, p + 5),
                    ]);
                    self.cur_loc[track] = (p + 6).wrapping_sub(offset as usize);
                }
                self.last_status[track] = 0;
                Event::LoopEnd
            } else {
                Event::Unknown
            }
        } else {
            let mut m = MidiEvent::default();
            if status & 0x80 != 0 {
                m.status = status;
                m.byte1 = self.get_track_byte(buf, track);
                self.last_status[track] = status;
            } else {
                // Running status.
                m.status = self.last_status[track];
                m.byte1 = status;
            }
            let hi = m.status & 0xF0;
            if hi != PROGRAM_CHANGE && hi != CHANNEL_PRESSURE {
                m.byte2 = self.get_track_byte(buf, track);
                if hi == NOTE_ON {
                    m.duration = self.read_var_len(buf, track);
                }
            } else {
                m.byte2 = 0;
            }
            Event::SeqMidi(m)
        }
    }

    /// `__getTrackByte`: inside a backup block, the next copied byte;
    /// otherwise the next track byte, where `FE FE` is a literal 0xFE and
    /// `FE hi lo len` starts a block of `len` bytes copied from
    /// `hi:lo + 4` bytes back.
    fn get_track_byte(&mut self, buf: &[u8], track: usize) -> u8 {
        if self.cur_bu_len[track] != 0 {
            let b = rd(buf, self.cur_bu_ptr[track]);
            self.cur_bu_ptr[track] += 1;
            self.cur_bu_len[track] -= 1;
            return b;
        }
        let mut b = rd(buf, self.cur_loc[track]);
        self.cur_loc[track] += 1;
        if b == AL_CMIDI_BLOCK_CODE {
            let next = rd(buf, self.cur_loc[track]);
            self.cur_loc[track] += 1;
            if next != AL_CMIDI_BLOCK_CODE {
                let lo = rd(buf, self.cur_loc[track]);
                self.cur_loc[track] += 1;
                let len = rd(buf, self.cur_loc[track]);
                self.cur_loc[track] += 1;
                let backup = ((next as usize) << 8) + lo as usize;
                self.cur_bu_ptr[track] = self.cur_loc[track].wrapping_sub(backup + 4);
                self.cur_bu_len[track] = len;
                self.backup_blocks += 1;
                b = rd(buf, self.cur_bu_ptr[track]);
                self.cur_bu_ptr[track] = self.cur_bu_ptr[track].wrapping_add(1);
                // A zero-length block wraps to 255 like the C's u8.
                self.cur_bu_len[track] = self.cur_bu_len[track].wrapping_sub(1);
            }
        }
        b
    }

    /// `__readVarLen`.
    fn read_var_len(&mut self, buf: &[u8], track: usize) -> u32 {
        let mut value = self.get_track_byte(buf, track) as u32;
        if value & 0x80 != 0 {
            value &= 0x7F;
            loop {
                let c = self.get_track_byte(buf, track) as u32;
                value = (value << 7).wrapping_add(c & 0x7F);
                if c & 0x80 == 0 {
                    break;
                }
            }
        }
        value
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::super::testdata;
    use super::*;
    use alloc::vec::Vec;

    /// A sequence's bytes as `syAudioReadRom` copies them (even length).
    fn seq_bytes(data: &super::super::data::AudioData, id: usize) -> Vec<u8> {
        let e = data.seqs[id];
        let len = e.len as usize + (e.len as usize & 1);
        let mut v: Vec<u8> = data.sbk[e.offset as usize..].iter().copied().take(len).collect();
        v.resize(len, 0);
        v
    }

    #[test]
    fn cseq_backup_and_loop() {
        let Some(data) = testdata::data() else {
            return;
        };
        let (mut backups, mut loop_dec, mut loop_reset, mut loop_forever) = (0, 0, 0, 0);
        let mut notes = 0;
        for id in 0..data.seqs.len() {
            let mut buf = seq_bytes(&data, id);
            if buf.len() < 68 {
                continue;
            }
            let mut s = CSeq::new(&buf);
            assert!(s.qnpt > 0.0 && s.qnpt.is_finite(), "seq {id}");
            for _ in 0..20_000 {
                if s.next_delta().is_none() {
                    break;
                }
                let before_loc = s.cur_loc;
                // The current loop counter a loop end at curLoc would read
                // (FF 2D loopCt curLpCt ...).
                let before_ct: [u8; 16] = core::array::from_fn(|t| rd(&buf, before_loc[t] + 3));
                // After `__alCSeqNextDelta` the pending deltas are current.
                let track = (0..16)
                    .filter(|&t| (s.valid_tracks >> t) & 1 != 0)
                    .min_by_key(|&t| s.evt_delta_ticks[t]);
                let evt = s.next_event(&mut buf);
                match evt {
                    Event::SeqMidi(m) => {
                        assert!((0x80..0xF0).contains(&m.status), "seq {id}: status {:#x}", m.status);
                        assert!(m.byte1 < 0x80, "seq {id}: byte1 {:#x}", m.byte1);
                        if m.status & 0xF0 == NOTE_ON {
                            assert!((1..0x80).contains(&m.byte2), "seq {id}: vel {:#x}", m.byte2);
                            notes += 1;
                        }
                    }
                    Event::LoopEnd => {
                        // The track `n_alCSeqNextEvent` picked: the lowest
                        // valid track with the smallest pending delta.
                        let t = track.expect("loop end track");
                        assert_eq!(rd(&buf, before_loc[t]), 0xFF, "seq {id}");
                        assert_eq!(rd(&buf, before_loc[t] + 1), AL_CMIDI_LOOPEND_CODE, "seq {id}");
                        let p = before_loc[t] + 2;
                        let (loop_ct, now) = (buf[p], buf[p + 1]);
                        match before_ct[t] {
                            0 => {
                                assert_eq!(now, loop_ct, "seq {id}: counter reset");
                                assert!(s.cur_loc[t] > before_loc[t]);
                                loop_reset += 1;
                            }
                            0xFF => {
                                assert_eq!(now, 0xFF);
                                assert!(s.cur_loc[t] <= before_loc[t]);
                                loop_forever += 1;
                            }
                            c => {
                                assert_eq!(now, c - 1, "seq {id}: counter decrement");
                                // An empty loop body (only the delta before
                                // the loop end) lands back on the same byte.
                                assert!(s.cur_loc[t] <= before_loc[t], "seq {id}: loops back");
                                loop_dec += 1;
                            }
                        }
                    }
                    Event::Unknown => panic!("seq {id}: unknown meta event"),
                    Event::SeqEnd => break,
                    _ => {}
                }
            }
            backups += s.backup_blocks;
        }
        std::eprintln!(
            "notes {notes} backups {backups} loop dec {loop_dec} reset {loop_reset} forever {loop_forever}"
        );
        assert!(notes > 10_000);
        assert!(backups > 100);
        // Every shipped loop end loops forever (counter 0xFF); the
        // finite-loop rewrite is covered by `cseq_loop_counter_rewrite`.
        assert!(loop_forever > 0);
        assert_eq!(loop_dec + loop_reset, 0);
    }

    /// A finite loop (`FF 2D 02 02`): the counter is decremented in the
    /// sequence bytes on each pass, then reset to the loop count when the
    /// loop falls through, as `__n_alCSeqGetTrackEvent` does.
    #[test]
    fn cseq_loop_counter_rewrite() {
        let mut buf = alloc::vec![0u8; 68];
        buf[3] = 68; // track 0 at offset 68
        buf[67] = 96; // division
        buf.extend_from_slice(&[
            0x00, 0x90, 0x3C, 0x64, 0x01, // 68: delta 0, NoteOn dur 1
            0x0A, // 73: delta 10
            0xFF, 0x2D, 0x02, 0x02, 0x00, 0x00, 0x00, 0x0E, // 74: loop end, back 14 -> 68
            0x00, 0xFF, 0x2F, // 82: delta 0, end of track
        ]);
        let mut s = CSeq::new(&buf);
        let mut trace = Vec::new();
        while s.next_delta().is_some() {
            let e = s.next_event(&mut buf);
            let tag = match e {
                Event::SeqMidi(_) => 'n',
                Event::LoopEnd => char::from(b'0' + buf[77]),
                Event::SeqEnd => 'e',
                _ => '?',
            };
            trace.push(tag);
            if tag == 'e' {
                break;
            }
        }
        assert_eq!(trace, ['n', '1', 'n', '0', 'n', '2', 'e']);
        assert_eq!(buf[77], 2, "counter restored for the next play");
    }
}
