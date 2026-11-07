#!/usr/bin/env python3
"""Audio frames and FNV-1a hash of an n64_audio_dump.so capture (host Python).

    capture_frames.py OUT.raw [--first-frames N | --values N]
                      [--frames-out FILE] [--adv-out FILE]

Reads OUT.raw.txt (the dump's sidecar: one `buf` record per AI buffer piece)
and splits the stream into the game's audio frames (552 or 368 samples;
the core delivers each frame in two pieces). Prints the frame count, the
368-sample frames as n64ref.rs `short_runs` (every tenth frame from first
through last) and the FNV-1a hash n64ref.rs compares: offset 0xCBF29CE484222325,
prime 0x100000001B3, one step per interleaved s16le value (L,R) taken as u16.

Window: the whole capture by default; --first-frames N = the first N audio
frames (C_ATTRACT: 3115); --values N = the first N s16 values.
--frames-out: one frame length per line. --adv-out: `cb_frame sample_offset
frame_index` per video frame of the sidecar (frame alignment between runs).

The split is greedy (552 first where both lengths reach a piece boundary;
`ambiguous` counts those places), as for the n64ref.rs schedules.
"""
import argparse
import sys

FRAME_LONG, FRAME_SHORT = 552, 368
FNV_OFFSET, FNV_PRIME = 0xCBF29CE484222325, 0x100000001B3


def read_bufs(sidecar):
    """(cb_frame, sample_offset, samples) per `buf` record."""
    bufs = []
    with open(sidecar) as f:
        for line in f:
            p = line.split()
            if p and p[0] == "buf":
                bufs.append((int(p[3]), int(p[5]), int(p[7]) // 4))
    return bufs


def split_frames(bufs):
    """Frame lengths: walk the piece boundaries in steps of 552 or 368 samples."""
    bounds = set()
    for _, off, n in bufs:
        bounds.add(off)
        bounds.add(off + n)
    end = bufs[-1][1] + bufs[-1][2]
    pos, frames, ambiguous = 0, [], 0
    while pos < end:
        steps = [n for n in (FRAME_LONG, FRAME_SHORT) if pos + n in bounds]
        if not steps:
            raise SystemExit(f"no frame boundary after sample {pos} (frame {len(frames)})")
        ambiguous += len(steps) > 1
        frames.append(steps[0])
        pos += steps[0]
    return frames, ambiguous


def short_runs(frames):
    """368-sample frames as (first, last) runs with a stride of ten."""
    runs = []
    for i, n in enumerate(frames):
        if n != FRAME_SHORT:
            continue
        if runs and i - runs[-1][1] == 10:
            runs[-1][1] = i
        else:
            runs.append([i, i])
    return [tuple(r) for r in runs]


def fnv1a(raw, values):
    h = FNV_OFFSET
    mv = memoryview(raw)[:values * 2].cast("H")
    for v in mv:
        h = ((h ^ v) * FNV_PRIME) & 0xFFFFFFFFFFFFFFFF
    return h


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("raw")
    win = ap.add_mutually_exclusive_group()
    win.add_argument("--first-frames", type=int, default=None)
    win.add_argument("--values", type=int, default=None)
    ap.add_argument("--frames-out", default=None)
    ap.add_argument("--adv-out", default=None)
    args = ap.parse_args()
    if sys.byteorder != "little":
        raise SystemExit("hashing assumes a little-endian host (s16le values read as u16)")

    frames, ambiguous = split_frames(read_bufs(args.raw + ".txt"))
    with open(args.raw, "rb") as f:
        raw = f.read()
    total = len(raw) // 2
    if args.values is not None:
        values = args.values
    elif args.first_frames is not None:
        if args.first_frames > len(frames):
            raise SystemExit(f"--first-frames {args.first_frames} > {len(frames)} frames captured")
        values = 2 * sum(frames[:args.first_frames])
    else:
        values = total
    if values > total:
        raise SystemExit(f"window of {values} values > {total} captured")

    nwin = args.first_frames if args.first_frames is not None else len(frames)
    print(f"frames {len(frames)} ambiguous {ambiguous} values {total}")
    print(f"short_runs {short_runs(frames[:nwin])}")
    print(f"hash 0x{fnv1a(raw, values):016x} over {values} values")

    if args.frames_out:
        with open(args.frames_out, "w") as f:
            f.write("".join(f"{n}\n" for n in frames))
    if args.adv_out:
        index, pos = {}, 0
        for i, n in enumerate(frames):
            index[pos] = i
            pos += n
        first = {}
        for cb, off, _ in read_bufs(args.raw + ".txt"):
            first.setdefault(cb, off)
        with open(args.adv_out, "w") as f:
            for cb in sorted(first):
                f.write(f"{cb} {first[cb]} {index.get(first[cb], -1)}\n")


if __name__ == "__main__":
    main()
