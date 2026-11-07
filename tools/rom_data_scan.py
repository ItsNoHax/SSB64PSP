#!/usr/bin/env python3
"""Scan built binaries for ROM data (D-048: the EBOOT carries none).

usage: rom_data_scan.py --rom ROM [--files DIR] [--min N] [--text-min N] [BINARY...]

- Corpus: the whole ROM (code, overlays, compressed archive) and every
  decompressed archive file (`romtool extract ROM --out DIR` -> DIR/files;
  run here into a temporary directory without --files), each as stored,
  16-bit swapped and 32-bit swapped (a table converted to the PSP's
  little-endian words still matches).
- BINARY: default `DEFAULT_TARGETS` that exist (the release ELFs of the
  game, the installer and the asset viewer, and `romtool`); an ELF or PRX
  (its allocated PROGBITS sections, hits named by symbol), an EBOOT.PBP (DATA.PSP as a PRX, PARAM.SFO and the XMB files
  whole) or any other file (whole).
- Hit: a run the binary shares with the corpus, of at least `--min` bytes
  (default 32) in data, `--text-min` (default 64) in code (MIPS idioms the
  two compilers share run to 48 bytes), unless trivial (`trivial`): fewer
  than 8 distinct byte values,
  periodic with a period of 16 bytes or less (zeros, 0xFF fill, repeated
  words), or an arithmetic progression of 16- or 32-bit words.
- Method: K-byte anchors every K bytes of the binary (K = 16 for 32),
  fingerprinted and looked up at every byte offset of the corpus (numpy),
  then extended both ways byte by byte: every shared run of 2K - 1 bytes or
  more is found, unless each of its anchors has fewer than 4 distinct
  byte values (zero fill: skipped for speed).
- Known hits (`KNOWN`, third-party data that matches the ROM by
  coincidence) are printed as such and do not fail the scan.
- Exit 1 when any binary has an unknown hit. Prints nothing read from the ROM:
  offsets, lengths, symbols and archive file ids only.
- Needs the ROM, so it is not part of `tools/docs/validate_docs.py` (CI has
  none); run it on release builds (docs/evidence/re/RE-589.md).
"""

import argparse
import bisect
import os
import re
import struct
import subprocess
import sys
import tempfile

import numpy as np

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
C = [np.uint64(c) for c in (0x9E3779B97F4A7C15, 0xC2B2AE3D27D4EB4F,
                             0x165667B19E3779F9, 0x27D4EB2F165667C5,
                             0x85EBCA77C2B2AE63, 0xD6E8FEB86659FD93,
                             0xA0761D6478BD642F, 0xE7037ED1A0B428DB)]


def fingerprints(buf, offset, count, k):
    """Fingerprint of the k bytes (8 <= k <= 64, k % 8 == 0) at
    offset + 8*j for j < count."""
    n = k // 8
    w = np.frombuffer(buf, dtype="<u8", count=count + n - 1, offset=offset)
    with np.errstate(over="ignore"):
        fp = w[:count] * C[0]
        for i in range(1, n):
            fp = fp + w[i:i + count] * C[i]
    return fp


def progression(run, width):
    """`run` holds, at some byte phase, an arithmetic progression of at
    least four little-endian words of `width` bytes (0, 1, 2, ...; GE
    opcodes with no argument): no information beyond start and step."""
    for phase in range(width):
        n = (len(run) - phase) // width
        if n < 4 or len(run) - n * width > 2 * width - 1:
            continue
        w = [int.from_bytes(run[phase + i * width:phase + (i + 1) * width], "little")
             for i in range(n)]
        if len({b - a for a, b in zip(w, w[1:])}) == 1:
            return True
    return False


def trivial(run):
    """Fewer than 8 distinct byte values, periodic with a period of 16 bytes
    or less (zeros, 0xFF fill, a repeated word), or an arithmetic
    progression of 16- or 32-bit words, alone or between zero fill (an
    index ramp 0, 1, 2, ... after the zero padding before it)."""
    if len(set(run)) < 8:
        return True
    if any(run[p:] == run[:-p] for p in range(1, 17)):
        return True
    core = run.strip(b"\0")
    return any(progression(r, w) for r in (run, core) for w in (4, 2))


def swapped(data, width):
    n = len(data) // width * width
    dt = {2: "u2", 4: "u4"}[width]
    body = np.frombuffer(data, dtype=">" + dt, count=n // width).astype("<" + dt)
    return body.tobytes() + data[n:]


class Corpus:
    """Named blobs concatenated (each padded to 8 bytes), in three byte
    orders."""

    def __init__(self, parts):
        self.names, self.starts, chunks, pos = [], [], [], 0
        for name, data in parts:
            pad = (-len(data)) % 8
            self.names.append(name)
            self.starts.append(pos)
            chunks.append(data + b"\0" * pad)
            pos += len(data) + pad
        self.ends = self.starts[1:] + [pos]
        raw = b"".join(chunks)
        self.variants = [("as stored", raw), ("16-bit swapped", swapped(raw, 2)),
                         ("32-bit swapped", swapped(raw, 4))]

    def where(self, pos):
        i = bisect.bisect_right(self.starts, pos) - 1
        return i, self.starts[i], self.ends[i]


def elf_regions(data, data_min, text_min):
    """(section, address, bytes, minimum run) of each allocated PROGBITS
    section, and the symbols [(address, size, name)] sorted; None for a
    file that is not a 32-bit little-endian ELF."""
    if data[:4] != b"\x7fELF" or data[4] != 1 or data[5] != 1:
        return None, None
    shoff, = struct.unpack_from("<I", data, 0x20)
    shentsize, shnum, shstrndx = struct.unpack_from("<HHH", data, 0x2E)
    shdrs = [struct.unpack_from("<IIIIIIIIII", data, shoff + i * shentsize)
             for i in range(shnum)]
    strtab = shdrs[shstrndx]

    def name_at(table, off):
        start = table[4] + off
        return data[start:data.index(b"\0", start)].decode("latin-1")

    regions, symbols = [], []
    for sh in shdrs:
        name, typ, flags, addr, off, size = sh[:6]
        if typ == 1 and flags & 2 and size:
            minimum = text_min if flags & 4 else data_min
            regions.append((name_at(strtab, name), addr, data[off:off + size], minimum))
        if typ == 2:  # SYMTAB
            names = shdrs[sh[6]]
            for i in range(size // 16):
                st_name, value, st_size, info, _, shndx = struct.unpack_from(
                    "<IIIBBH", data, off + i * 16)
                if st_name and shndx and info & 0xF in (0, 1, 2):
                    symbols.append((value, st_size, name_at(names, st_name)))
    symbols.sort()
    return regions, symbols


def demangle(name):
    m = re.match(r"_ZN(.*)E$", name)
    if not m:
        return name
    parts, s = [], m.group(1)
    while s and s[0].isdigit():
        n = re.match(r"\d+", s).group(0)
        parts.append(s[len(n):len(n) + int(n)])
        s = s[len(n) + int(n):]
    if parts and re.fullmatch(r"h[0-9a-f]{16}", parts[-1]):
        parts.pop()
    return "::".join(parts).replace("$LT$", "<").replace("$GT$", ">") \
        .replace("$u20$", " ").replace("$C$", ",").replace("..", "::")


def symbol_at(symbols, addr):
    i = bisect.bisect_right(symbols, (addr, 1 << 32, "")) - 1
    while i >= 0:
        value, size, name = symbols[i]
        if value <= addr < value + max(size, 1):
            return f"{demangle(name)}+0x{addr - value:X}"
        if addr - value > 0x10000:
            break
        i -= 1
    return "?"


PBP_PARTS = ["PARAM.SFO", "ICON0.PNG", "ICON1.PMF", "PIC0.PNG", "PIC1.PNG",
             "SND0.AT3", "DATA.PSP", "DATA.PSAR"]


def regions_of(data, data_min, text_min):
    """`elf_regions` of an ELF or PRX, of an EBOOT.PBP's DATA.PSP plus its
    other parts whole, or the whole file."""
    regions, symbols = elf_regions(data, data_min, text_min)
    if regions is not None:
        return regions, symbols
    if data[:4] == b"\0PBP":
        offs = list(struct.unpack_from("<8I", data, 8)) + [len(data)]
        regions, symbols = [], []
        for name, a, b in zip(PBP_PARTS, offs, offs[1:]):
            part = data[a:b]
            sub, syms = elf_regions(part, data_min, text_min) if part else (None, None)
            if sub is not None:
                regions += [(f"{name}:{sec}", addr, d, m) for sec, addr, d, m in sub]
                symbols = syms
            elif part:
                regions.append((name, 0, part, data_min))
        return regions, symbols
    return [("(whole file)", 0, data, data_min)], []


def scan(regions, corpus):
    """Maximal shared runs: [(region index, start, end, variant, corpus pos)]."""
    # Anchors of K bytes every K bytes: a run of 2K - 1 bytes holds one.
    K = min(64, (min(r[3] for r in regions) + 1) // 2 // 8 * 8)
    anchors = {}
    for ri, (_, _, data, _) in enumerate(regions):
        for p in range(0, len(data) - K + 1, K):
            if len(set(data[p:p + K])) < 4:
                continue
            fp = int(fingerprints(data, p, 1, K)[0])
            anchors.setdefault(fp, []).append((ri, p))
    if not anchors:
        return []
    keys = np.array(sorted(anchors), dtype=np.uint64)
    hits = {}
    for vname, blob in corpus.variants:
        for o in range(8):
            count = (len(blob) - o) // 8 - (K // 8 - 1)
            if count <= 0:
                continue
            fps = fingerprints(blob, o, count, K)
            for j in np.nonzero(np.isin(fps, keys))[0]:
                cpos = o + 8 * int(j)
                for ri, p in anchors[int(fps[j])]:
                    data = regions[ri][2]
                    if blob[cpos:cpos + K] != data[p:p + K]:
                        continue
                    _, lo, hi = corpus.where(cpos)
                    s, c = p, cpos
                    while s > 0 and c > lo and data[s - 1] == blob[c - 1]:
                        s, c = s - 1, c - 1
                    e, ce = p + K, cpos + K
                    while e < len(data) and ce < hi and data[e] == blob[ce]:
                        e, ce = e + 1, ce + 1
                    if e - s < regions[ri][3] or trivial(data[s:e]):
                        continue
                    key = (ri, s, e)
                    hits.setdefault(key, (vname, c))
    # Keep maximal runs only.
    out = []
    for (ri, s, e), src in sorted(hits.items()):
        if any(r == ri and a <= s and e <= b and (a, b) != (s, e)
               for (r, a, b) in hits):
            continue
        out.append((ri, s, e) + src)
    return out


def load_corpus(rom, files_dir):
    with open(rom, "rb") as f:
        parts = [("rom", f.read())]
    for name in sorted(os.listdir(files_dir)):
        if name.endswith(".bin"):
            with open(os.path.join(files_dir, name), "rb") as f:
                parts.append((f"file {name[:-4]}", f.read()))
    return Corpus(parts)


# (corpus name, start, end, reason): a hit whose corpus span lies inside
# [start, end) of that corpus entry is known (RE-589 item 9).
KNOWN = [
    ("rom", 0x1AC410, 0x1AC410 + 0x800,
     "rust-psp psp::debug MSX_FONT: the same 8x8 MSX glyphs as the ROM's"),
    ("rom", 0x3DA4C, 0x3DA84,
     "Rust core's f32 powers of ten 1e0..1e10 (dec2flt fast path, romtool): "
     "the same eleven floats as the N64 main segment's, with their zero fill"),
]


# The release binaries scanned when none is named (each one that exists).
DEFAULT_TARGETS = [
    "psp-game/target/mipsel-sony-psp/release/ssb64-psp-game",
    "psp-installer/target/mipsel-sony-psp/release/ssb64-psp-installer",
    "psp-asset-viewer/target/mipsel-sony-psp/release/ssb64-psp-asset-viewer",
    "target/release/romtool",
]


def known(name, off, length):
    for kname, start, end, reason in KNOWN:
        if name == kname and start <= off and off + length <= end:
            return reason
    return None


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--rom", required=True)
    ap.add_argument("--files", help="romtool extract's files/ directory")
    ap.add_argument("--min", type=int, default=32)
    ap.add_argument("--text-min", type=int, default=64)
    ap.add_argument("binaries", nargs="*")
    args = ap.parse_args()
    if not args.binaries:
        args.binaries = [p for p in (os.path.join(ROOT, t) for t in DEFAULT_TARGETS)
                         if os.path.exists(p)]
        if not args.binaries:
            sys.exit("no binary named and no default target built")
    if min(args.min, args.text_min) < 31:
        sys.exit("--min and --text-min must be at least 31 (16-byte anchors)")

    with tempfile.TemporaryDirectory() as tmp:
        files = args.files
        if not files:
            subprocess.run(["cargo", "run", "--release", "-q", "-p", "romtool", "--",
                            "extract", args.rom, "--out", tmp], cwd=ROOT, check=True,
                           stdout=subprocess.DEVNULL)
            files = os.path.join(tmp, "files")
        corpus = load_corpus(args.rom, files)

    total = 0
    for path in args.binaries:
        with open(path, "rb") as f:
            data = f.read()
        regions, symbols = regions_of(data, args.min, args.text_min)
        size = sum(len(r[2]) for r in regions)
        hits = scan(regions, corpus)
        lines, unknown = [], 0
        for ri, s, e, vname, cpos in hits:
            sec, addr, _, _ = regions[ri]
            i, lo, _ = corpus.where(cpos)
            where = symbol_at(symbols, addr + s) if symbols else f"0x{s:X}"
            why = known(corpus.names[i], cpos - lo, e - s)
            unknown += why is None
            lines.append(f"  {sec} 0x{addr + s:08X} {e - s:6d} B  {where}  <- "
                         f"{corpus.names[i]} +0x{cpos - lo:X} ({vname})"
                         + (f"  known: {why}" if why else ""))
        print(f"{path}: {len(regions)} regions, {size} bytes, {len(hits)} hits, "
              f"{unknown} unknown")
        for line in lines:
            print(line)
        total += unknown
    sys.exit(1 if total else 0)


if __name__ == "__main__":
    main()
