#!/usr/bin/env python3
"""Synthesize the XMB background-music loop: an original, upbeat fighting-game cue.

usage: make-snd0-loop.py OUTPUT.wav

150 BPM in E minor, 16 bars (25.6 s), 44.1 kHz 16-bit stereo. Every sound is
generated here: additive (alias-free) saw and square voices, noise drums, no
samples. The piece is original; nothing is taken from the Smash Bros.
soundtrack (D-037).

The loop is seamless by construction. Notes and effect tails that run past the
end wrap around to the start, so the last bar flows into the first with no
gap and no click. The output is deterministic (fixed noise seed).

Encode the result with tools/encode-snd0.sh. See docs/xmb-assets.md.
"""
import sys
import wave

import numpy as np

RATE = 44100
BPM = 150
STEP = RATE * 60 // BPM // 4  # samples per 16th note (4410)
BARS = 16
LENGTH = BARS * 16 * STEP

rng = np.random.default_rng(64)


def midi_hz(n):
    return 440.0 * 2 ** ((n - 69) / 12)


def add(buf, start, sig, pan=0.0, gain=1.0):
    """Mix a mono signal into the stereo loop at `start`, wrapping past the end."""
    left = np.cos((pan + 1) * np.pi / 4) * gain
    right = np.sin((pan + 1) * np.pi / 4) * gain
    idx = (start + np.arange(len(sig))) % LENGTH
    np.add.at(buf[0], idx, sig * left)
    np.add.at(buf[1], idx, sig * right)


def envelope(n, attack, decay, sustain, release, hold):
    """Linear attack, exponential decay to sustain, held for `hold` samples, then release."""
    t = np.arange(n) / RATE
    env = np.where(t < attack, t / max(attack, 1e-6),
                   sustain + (1 - sustain) * np.exp(-(t - attack) / decay))
    held = hold / RATE
    rel = np.clip(1 - (t - held) / release, 0, 1)
    return env * np.where(t < held, 1, rel)


def additive(freq, n, kind, harmonics=40, bright=1.0, vibrato=0.0, detune=0.0):
    t = np.arange(n) / RATE
    phase = 2 * np.pi * freq * (1 + detune) * t
    if vibrato:
        # Delayed vibrato: none for the first 150 ms, then 5.5 Hz.
        depth = vibrato * np.clip((t - 0.15) / 0.2, 0, 1)
        # Frequency swings by +/- depth: the phase is the integral of that swing.
        phase -= depth * freq * np.cos(2 * np.pi * 5.5 * t) / 5.5
    out = np.zeros(n)
    top = min(harmonics, int(15000 / freq))
    for k in range(1, top + 1):
        if kind == "square" and k % 2 == 0:
            continue
        amp = (1 / k) * np.exp(-(k - 1) * (1 - bright) * 0.35)
        out += amp * np.sin(k * phase)
    return out


def lowpass(x, cutoff):
    """Windowed-sinc FIR applied circularly through the FFT (keeps the loop seamless)."""
    taps = 255
    m = np.arange(taps) - taps // 2
    h = np.sinc(2 * cutoff / RATE * m) * np.hamming(taps)
    h /= h.sum()
    kernel = np.zeros(x.shape[-1])
    kernel[:taps] = h
    kernel = np.roll(kernel, -(taps // 2))
    return np.real(np.fft.ifft(np.fft.fft(x) * np.fft.fft(kernel)))


def highpass(x, cutoff):
    return x - lowpass(x, cutoff)


# --- Harmony --------------------------------------------------------------
# A section: i-VI-VII-V twice. B section lifts: VI-VII-i-i, VI-VII-V-V.
CHORDS = ["Em", "C", "D", "B", "Em", "C", "D", "B",
          "C", "D", "Em", "Em", "C", "D", "B", "B"]
ROOT = {"Em": 40, "C": 36, "D": 38, "B": 35}
VOICING = {"Em": [52, 55, 59, 64], "C": [48, 52, 55, 60],
           "D": [50, 54, 57, 62], "B": [47, 51, 54, 59]}

# --- Lead melody: (16th offset in bar, MIDI note, length in 16ths) --------
MOTIF = [
    [(0, 76, 4), (4, 71, 2), (6, 76, 2), (8, 79, 4), (12, 78, 2), (14, 76, 2)],
    [(0, 79, 6), (6, 76, 2), (8, 72, 2), (10, 76, 2), (12, 79, 4)],
    [(0, 81, 4), (4, 78, 2), (6, 74, 2), (8, 81, 2), (10, 83, 2), (12, 81, 2), (14, 78, 2)],
]
MELODY = MOTIF + [
    [(0, 75, 6), (6, 78, 2), (8, 83, 8)],
] + MOTIF + [
    [(0, 83, 4), (4, 81, 2), (6, 78, 2), (8, 75, 4), (12, 78, 4)],
    # B section
    [(0, 84, 4), (4, 83, 2), (6, 79, 2), (8, 76, 4), (12, 79, 4)],
    [(0, 81, 4), (4, 78, 2), (6, 81, 2), (8, 86, 6), (14, 83, 2)],
    [(0, 83, 8), (8, 79, 4), (12, 76, 4)],
    [(0, 71, 2), (2, 74, 2), (4, 76, 2), (6, 79, 2), (8, 83, 2), (10, 86, 2), (12, 88, 4)],
    [(0, 88, 4), (4, 86, 2), (6, 84, 2), (8, 83, 4), (12, 79, 4)],
    [(0, 86, 4), (4, 84, 2), (6, 83, 2), (8, 81, 4), (12, 78, 4)],
    [(0, 75, 4), (4, 78, 4), (8, 83, 8)],
    [(0, 81, 2), (2, 78, 2), (4, 75, 2), (6, 71, 2), (8, 75, 2), (10, 78, 2), (12, 81, 2), (14, 83, 2)],
]
assert len(MELODY) == BARS and len(CHORDS) == BARS

STABS = [0, 3, 6, 8, 11, 14]  # syncopated chord hits per bar


def kick():
    n = int(0.35 * RATE)
    t = np.arange(n) / RATE
    freq = 50 + 110 * np.exp(-t / 0.03)
    body = np.sin(2 * np.pi * np.cumsum(freq) / RATE) * np.exp(-t / 0.12)
    click = rng.standard_normal(n) * np.exp(-t / 0.003) * 0.3
    return np.tanh(1.6 * (body + click))


def snare():
    n = int(0.25 * RATE)
    t = np.arange(n) / RATE
    tone = np.sin(2 * np.pi * 190 * t) * np.exp(-t / 0.05) * 0.6
    noise = highpass(np.pad(rng.standard_normal(n), (0, 0)), 1200) * np.exp(-t / 0.07)
    return tone + noise * 0.8


def hat(open_):
    n = int((0.22 if open_ else 0.05) * RATE)
    t = np.arange(n) / RATE
    noise = rng.standard_normal(n)
    noise = noise - lowpass(noise, 7000)
    return noise * np.exp(-t / (0.08 if open_ else 0.012))


def crash():
    n = int(1.6 * RATE)
    t = np.arange(n) / RATE
    noise = rng.standard_normal(n)
    noise = noise - lowpass(noise, 4000)
    return noise * np.exp(-t / 0.5)


def main():
    if len(sys.argv) != 2:
        sys.exit(__doc__)

    drums = np.zeros((2, LENGTH))
    bass = np.zeros((2, LENGTH))
    chords = np.zeros((2, LENGTH))
    lead = np.zeros((2, LENGTH))

    k, s, hc, ho, cr = kick(), snare(), hat(False), hat(True), crash()
    for bar in range(BARS):
        b0 = bar * 16 * STEP
        fill = bar in (7, 15)
        # Kick: driving, with a push into beat 3.
        for step in (0, 6, 8, 10) if bar % 2 else (0, 3, 8, 10):
            if fill and step > 8:
                continue
            add(drums, b0 + step * STEP, k, gain=0.95)
        for step in (4, 12):
            add(drums, b0 + step * STEP, s, gain=0.7)
        if fill:
            for i, step in enumerate((9, 11, 13, 14, 15)):
                add(drums, b0 + step * STEP, s, pan=0.3 * (i % 2 * 2 - 1), gain=0.35 + 0.08 * i)
        for step in range(16):
            if step == 14 and not fill:
                add(drums, b0 + step * STEP, ho, pan=0.35, gain=0.22)
            elif step % 2 == 0:
                add(drums, b0 + step * STEP, hc, pan=0.35, gain=0.3)
            else:
                add(drums, b0 + step * STEP, hc, pan=0.35, gain=0.12)
        if bar in (0, 8):
            add(drums, b0, cr, pan=-0.4, gain=0.35)

        # Bass: 8ths on the root, octave jump on the offbeat of beats 2 and 4.
        root = ROOT[CHORDS[bar]]
        for step in range(0, 16, 2):
            note = root + (12 if step in (6, 14) else 0)
            n = 2 * STEP
            sig = additive(midi_hz(note), n, "saw", harmonics=18, bright=0.55)
            sig *= envelope(n, 0.004, 0.09, 0.55, 0.03, n - int(0.03 * RATE))
            add(bass, b0 + step * STEP, sig, gain=0.55)

        # Chord stabs: two detuned saws per voice, spread wide.
        section_b = bar >= 8
        for step in STABS:
            n = int(1.6 * STEP)
            for note in VOICING[CHORDS[bar]]:
                for side, det in ((-0.7, -0.004), (0.7, 0.004)):
                    sig = additive(midi_hz(note), n, "saw", harmonics=24, bright=0.6, detune=det)
                    sig *= envelope(n, 0.003, 0.07, 0.3, 0.04, n - int(0.04 * RATE))
                    add(chords, b0 + step * STEP, sig, pan=side,
                        gain=(0.075 if section_b else 0.06))

        # Lead: square with delayed vibrato, one octave-up saw layer in section B.
        for off, note, length in MELODY[bar]:
            n = length * STEP + int(0.08 * RATE)
            sig = additive(midi_hz(note), n, "square", harmonics=20, bright=0.75, vibrato=0.012)
            if section_b:
                sig += 0.35 * additive(midi_hz(note + 12), n, "saw", harmonics=10, bright=0.5)
            sig *= envelope(n, 0.006, 0.25, 0.7, 0.08, length * STEP)
            add(lead, b0 + off * STEP, sig, gain=0.24)

    # Circular effects: a dotted-8th ping-pong echo on the lead, a short room on the chords.
    echo = int(3 * STEP)
    lead = lead + 0.28 * np.stack([np.roll(lead[1], echo), np.roll(lead[0], echo)])
    lead = lead + 0.12 * np.roll(lead, 2 * echo, axis=1)
    room = chords.copy()
    for delay, g in ((1301, 0.22), (1777, 0.18), (2531, 0.14), (3307, 0.1)):
        room += g * np.roll(chords[::-1], delay, axis=1)
    chords = lowpass(room, 6500)
    bass = lowpass(bass, 1800)

    mix = drums + bass + chords + lead
    mix = highpass(mix, 30)
    mix = np.tanh(1.2 * mix / np.max(np.abs(mix))) / np.tanh(1.2)
    mix *= 10 ** (-1.0 / 20)  # -1 dBFS peak
    pcm = (mix.T * 32767).astype("<i2")

    with wave.open(sys.argv[1], "wb") as w:
        w.setnchannels(2)
        w.setsampwidth(2)
        w.setframerate(RATE)
        w.writeframes(pcm.tobytes())
    print(f"{sys.argv[1]}: {LENGTH / RATE:.2f} s, {BARS} bars at {BPM} BPM")


if __name__ == "__main__":
    main()
