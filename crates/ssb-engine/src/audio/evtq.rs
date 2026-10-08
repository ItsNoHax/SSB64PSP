//! libultra's `ALEventQueue` as the compressed sequence player uses it
//! (`n_alEvtqNew`, `n_alEvtqNextEvent`, `n_alEvtqPostEvent`,
//! `n_alEvtqFlushType`, `alLink`/`alUnlink`; `n_env.c:3268-3427`), and the
//! `N_ALEvent` it carries (`include/n_audio/n_libaudio.h:145-156`).
//!
//! The queue is a fixed pool of list items linked by index, reproducing the
//! C's link order exactly: posts insert into a delta-encoded list sorted by
//! time (after every item with an equal time), the free list is LIFO
//! (`alLink(item, &freeList)` inserts at the head), and a post that finds
//! the free list empty is silently dropped ([`EventQueue::dropped`]
//! counts them).

use alloc::vec::Vec;

/// `enum ALMsg` (`include/PR/libaudio.h:450-476`).
pub const AL_SEQ_REF_EVT: i16 = 0;
pub const AL_SEQ_MIDI_EVT: i16 = 1;
pub const AL_SEQP_MIDI_EVT: i16 = 2;
pub const AL_TEMPO_EVT: i16 = 3;
pub const AL_SEQ_END_EVT: i16 = 4;
pub const AL_NOTE_END_EVT: i16 = 5;
pub const AL_SEQP_ENV_EVT: i16 = 6;
pub const AL_SEQP_META_EVT: i16 = 7;
pub const AL_SEQP_PROG_EVT: i16 = 8;
pub const AL_SEQP_API_EVT: i16 = 9;
pub const AL_SEQP_VOL_EVT: i16 = 10;
pub const AL_SEQP_LOOP_EVT: i16 = 11;
pub const AL_SEQP_PRIORITY_EVT: i16 = 12;
pub const AL_SEQP_SEQ_EVT: i16 = 13;
pub const AL_SEQP_BANK_EVT: i16 = 14;
pub const AL_SEQP_PLAY_EVT: i16 = 15;
pub const AL_SEQP_STOP_EVT: i16 = 16;
pub const AL_SEQP_STOPPING_EVT: i16 = 17;
pub const AL_TRACK_END: i16 = 18;
pub const AL_CSP_LOOPSTART: i16 = 19;
pub const AL_CSP_LOOPEND: i16 = 20;
pub const AL_CSP_NOTEOFF_EVT: i16 = 21;
pub const AL_TREM_OSC_EVT: i16 = 22;
pub const AL_VIB_OSC_EVT: i16 = 23;

/// `AL_EVTQ_END`: post "after everything".
pub const AL_EVTQ_END: i32 = 0x7FFF_FFFF;

/// `ALMIDIEvent`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MidiEvent {
    pub ticks: i32,
    pub status: u8,
    pub byte1: u8,
    pub byte2: u8,
    /// NoteOn only (a varlen after the velocity); 0 otherwise (the C leaves
    /// it as whatever the stack held; only NoteOn reads it).
    pub duration: u32,
}

/// `ALTempoEvent`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TempoEvent {
    pub ticks: i32,
    pub status: u8,
    pub kind: u8,
    pub byte1: u8,
    pub byte2: u8,
    pub byte3: u8,
}

/// `N_ALEvent`. Pointers become indices: `voice`/`vs` name the player's
/// voice state (and so its synth vvoice), `osc` an oscillator state of
/// [`super::osc::OscPool`], `bank` a [`super::data::WaveRef::bank`]-style
/// bank id (0 music, 1 sfx).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Event {
    /// `type == -1`: what `n_alEvtqNextEvent` returns from an empty queue.
    Empty,
    SeqRef,
    /// A MIDI message read from the sequence.
    SeqMidi(MidiEvent),
    /// A MIDI message posted through the API (`n_alCSPSetChlFXMix`).
    SeqpMidi(MidiEvent),
    Tempo(TempoEvent),
    SeqEnd,
    NoteEnd {
        voice: u8,
    },
    Env {
        voice: u8,
        delta: i32,
        vol: u8,
    },
    SeqpApi,
    SeqpVol(i16),
    SeqpPriority {
        chan: u8,
        priority: u8,
    },
    /// `AL_SEQP_SEQ_EVT`: the player owns its one `ALCSeq`
    /// (`sSYAudioCSeqs[i]`), so the pointer carries no information.
    SeqpSeq,
    SeqpBank(u8),
    SeqpPlay,
    SeqpStop,
    SeqpStopping,
    TrackEnd,
    LoopStart,
    LoopEnd,
    CspNoteOff(MidiEvent),
    TremOsc {
        vs: u8,
        osc: u8,
    },
    VibOsc {
        vs: u8,
        osc: u8,
        chan: u8,
    },
    /// Types `AL_SEQ_END_EVT + 20` / `+ 21` (`n_alCSPSetBank_Alt` posts
    /// them as 0x18/0x19): set alternate bank slot 1 or 2. Unused by SSB.
    AltBank {
        slot: u8,
        bank: u8,
    },
    /// A meta event `__n_alCSeqGetTrackEvent` does not know: the C leaves
    /// `type` uninitialised (stack garbage); here it matches no case.
    Unknown,
}

impl Event {
    /// The C `type` field.
    pub fn ty(&self) -> i16 {
        match self {
            Event::Empty => -1,
            Event::SeqRef => AL_SEQ_REF_EVT,
            Event::SeqMidi(_) => AL_SEQ_MIDI_EVT,
            Event::SeqpMidi(_) => AL_SEQP_MIDI_EVT,
            Event::Tempo(_) => AL_TEMPO_EVT,
            Event::SeqEnd => AL_SEQ_END_EVT,
            Event::NoteEnd { .. } => AL_NOTE_END_EVT,
            Event::Env { .. } => AL_SEQP_ENV_EVT,
            Event::SeqpApi => AL_SEQP_API_EVT,
            Event::SeqpVol(_) => AL_SEQP_VOL_EVT,
            Event::SeqpPriority { .. } => AL_SEQP_PRIORITY_EVT,
            Event::SeqpSeq => AL_SEQP_SEQ_EVT,
            Event::SeqpBank(_) => AL_SEQP_BANK_EVT,
            Event::SeqpPlay => AL_SEQP_PLAY_EVT,
            Event::SeqpStop => AL_SEQP_STOP_EVT,
            Event::SeqpStopping => AL_SEQP_STOPPING_EVT,
            Event::TrackEnd => AL_TRACK_END,
            Event::LoopStart => AL_CSP_LOOPSTART,
            Event::LoopEnd => AL_CSP_LOOPEND,
            Event::CspNoteOff(_) => AL_CSP_NOTEOFF_EVT,
            Event::TremOsc { .. } => AL_TREM_OSC_EVT,
            Event::VibOsc { .. } => AL_VIB_OSC_EVT,
            Event::AltBank { slot, .. } => AL_SEQ_END_EVT + 19 + *slot as i16,
            Event::Unknown => i16::MIN,
        }
    }
}

/// A null link.
pub const NIL: u16 = u16::MAX;

/// `N_ALEventListItem` (or one of the two list heads).
#[derive(Debug, Clone, Copy)]
pub struct Item {
    pub next: u16,
    pub prev: u16,
    pub delta: i32,
    pub evt: Event,
}

/// `ALEventQueue`: `items[..count]` are the list items, `items[count]` is
/// `allocList`, `items[count + 1]` is `freeList`.
pub struct EventQueue {
    items: Vec<Item>,
    alloc: u16,
    free: u16,
    /// Posts dropped because every item was in use (the C drops them
    /// silently, `n_env.c:3324-3331`).
    pub dropped: u32,
}

impl EventQueue {
    /// `n_alEvtqNew`: every item linked onto the free list in turn, so the
    /// last item is the first one handed out.
    pub fn new(count: usize) -> Self {
        assert!(count + 2 < NIL as usize);
        let blank = Item {
            next: NIL,
            prev: NIL,
            delta: 0,
            evt: Event::Empty,
        };
        let mut q = Self {
            items: alloc::vec![blank; count + 2],
            alloc: count as u16,
            free: count as u16 + 1,
            dropped: 0,
        };
        for i in 0..count as u16 {
            q.link(i, q.free);
        }
        q
    }

    /// The first pending item, or [`NIL`].
    pub fn first(&self) -> u16 {
        self.items[self.alloc as usize].next
    }

    pub fn item(&self, i: u16) -> &Item {
        &self.items[i as usize]
    }

    pub fn item_mut(&mut self, i: u16) -> &mut Item {
        &mut self.items[i as usize]
    }

    /// Items on the alloc list.
    pub fn pending(&self) -> usize {
        let mut n = 0;
        let mut at = self.first();
        while at != NIL {
            n += 1;
            at = self.items[at as usize].next;
        }
        n
    }

    /// `alLink`: insert `ln` after `to`.
    pub fn link(&mut self, ln: u16, to: u16) {
        let to_next = self.items[to as usize].next;
        self.items[ln as usize].next = to_next;
        self.items[ln as usize].prev = to;
        if to_next != NIL {
            self.items[to_next as usize].prev = ln;
        }
        self.items[to as usize].next = ln;
    }

    /// `alUnlink`. Like the C, `ln`'s own links are left as they were.
    pub fn unlink(&mut self, ln: u16) {
        let Item { next, prev, .. } = self.items[ln as usize];
        if next != NIL {
            self.items[next as usize].prev = prev;
        }
        if prev != NIL {
            self.items[prev as usize].next = next;
        }
    }

    /// The common "remove this pending item" sequence of
    /// `n_alEvtqFlushType`, `__n_seqpReleaseVoice`, `__n_voiceNeedsNoteKill`
    /// and `__n_seqpStopOsc`: its delta moves to the next item, it is
    /// unlinked and pushed on the free list.
    pub fn remove(&mut self, i: u16) {
        let next = self.items[i as usize].next;
        if next != NIL {
            let d = self.items[i as usize].delta;
            let n = &mut self.items[next as usize].delta;
            *n = n.wrapping_add(d);
        }
        self.unlink(i);
        self.link(i, self.free);
    }

    /// `n_alEvtqNextEvent`: pops the first item (to the free list head) and
    /// returns its event and delta; [`Event::Empty`] (type -1) and 0 when
    /// nothing is pending.
    pub fn next_event(&mut self) -> (Event, i32) {
        let item = self.first();
        if item == NIL {
            return (Event::Empty, 0);
        }
        self.unlink(item);
        self.link(item, self.free);
        let it = &self.items[item as usize];
        (it.evt, it.delta)
    }

    /// `n_alEvtqPostEvent`. With `delta == AL_EVTQ_END` the item's own
    /// delta becomes 0 once it reaches the end of the list; like the C, the
    /// walk still compares the (decreasing) `AL_EVTQ_END` remainder against
    /// each item, signed.
    pub fn post_event(&mut self, evt: Event, mut delta: i32) {
        let item = self.items[self.free as usize].next;
        if item == NIL {
            self.dropped += 1;
            return;
        }
        self.unlink(item);
        self.items[item as usize].evt = evt;
        let post_at_end = delta == AL_EVTQ_END;
        let mut node = self.alloc;
        loop {
            let next = self.items[node as usize].next;
            if next == NIL {
                self.items[item as usize].delta = if post_at_end { 0 } else { delta };
                self.link(item, node);
                return;
            }
            let nd = self.items[next as usize].delta;
            if delta < nd {
                self.items[item as usize].delta = delta;
                self.items[next as usize].delta = nd.wrapping_sub(delta);
                self.link(item, node);
                return;
            }
            delta = delta.wrapping_sub(nd);
            node = next;
        }
    }

    /// `__n_CSPRepostEvent` (`n_env.c:3179`): reinserts an item that is on
    /// no list, by its `delta`, without touching the free list.
    pub fn repost(&mut self, item: u16) {
        let mut node = self.alloc;
        loop {
            let next = self.items[node as usize].next;
            if next == NIL {
                self.link(item, node);
                return;
            }
            let nd = self.items[next as usize].delta;
            let d = self.items[item as usize].delta;
            if d < nd {
                self.items[next as usize].delta = nd.wrapping_sub(d);
                self.link(item, node);
                return;
            }
            self.items[item as usize].delta = d.wrapping_sub(nd);
            node = next;
        }
    }

    /// `n_alEvtqFlushType`.
    pub fn flush_type(&mut self, ty: i16) {
        let mut at = self.first();
        while at != NIL {
            let next = self.items[at as usize].next;
            if self.items[at as usize].evt.ty() == ty {
                self.remove(at);
            }
            at = next;
        }
    }

    /// libultra's `alEvtqFlush`: every pending item back to the free list
    /// (SSB never calls it; kept for completeness).
    pub fn flush(&mut self) {
        let mut at = self.first();
        while at != NIL {
            let next = self.items[at as usize].next;
            self.unlink(at);
            self.link(at, self.free);
            at = next;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drain(q: &mut EventQueue) -> Vec<(i16, i32)> {
        let mut v = Vec::new();
        loop {
            let (e, d) = q.next_event();
            if e == Event::Empty {
                return v;
            }
            v.push((e.ty(), d));
        }
    }

    #[test]
    fn evtq_full_drops() {
        let mut q = EventQueue::new(4);
        q.post_event(Event::SeqpPlay, 100);
        q.post_event(Event::SeqpStop, 50);
        q.post_event(Event::SeqpApi, 100); // ties go after the earlier post
        q.post_event(Event::SeqRef, 0);
        assert_eq!(q.dropped, 0);
        q.post_event(Event::SeqEnd, 10);
        q.post_event(Event::TrackEnd, 10);
        assert_eq!(q.dropped, 2);
        assert_eq!(q.pending(), 4);
        // Delta-encoded, sorted, stable on ties; the dropped posts are gone.
        assert_eq!(
            drain(&mut q),
            [
                (AL_SEQ_REF_EVT, 0),
                (AL_SEQP_STOP_EVT, 50),
                (AL_SEQP_PLAY_EVT, 50),
                (AL_SEQP_API_EVT, 0)
            ]
        );
        assert_eq!(q.next_event(), (Event::Empty, 0));
        assert_eq!(Event::Empty.ty(), -1);
        // Space again: posts succeed.
        q.post_event(Event::SeqEnd, 10);
        assert_eq!((q.dropped, q.pending()), (2, 1));
    }

    #[test]
    fn evtq_end_and_flush() {
        let mut q = EventQueue::new(8);
        q.post_event(Event::SeqRef, 30);
        q.post_event(Event::SeqpStop, AL_EVTQ_END);
        q.post_event(Event::SeqRef, 10);
        q.post_event(Event::SeqpApi, 20);
        // AL_EVTQ_END lands last with delta 0; flushing moves deltas on.
        q.flush_type(AL_SEQ_REF_EVT);
        assert_eq!(
            drain(&mut q),
            [(AL_SEQP_API_EVT, 20), (AL_SEQP_STOP_EVT, 10)]
        );
        // The free list is LIFO: the item freed last is handed out first.
        let mut q = EventQueue::new(3);
        assert_eq!(q.items[q.free as usize].next, 2);
        q.post_event(Event::SeqRef, 0);
        assert_eq!(q.first(), 2);
        q.next_event();
        q.post_event(Event::SeqRef, 0);
        assert_eq!(q.first(), 2);
        q.post_event(Event::SeqpApi, 5);
        q.flush();
        assert_eq!(q.pending(), 0);
        assert_eq!(q.items[q.free as usize].next, 1);
    }
}
