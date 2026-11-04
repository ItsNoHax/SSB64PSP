#!/usr/bin/env python3
"""Writes psp-runtime/src/reverb_vfpu.rs: the reverb section kernel on the VFPU.

usage: python3 tools/audio/gen_reverb_vfpu.py [--check]

--check: exit 1 if the tracked file differs from what this script writes.

One step body per set of nonzero coefficients (mask 1-7: ff, fb, gain),
8 samples per step as two groups of 4, list-scheduled for the VFPU's
dependent-op latency (LAT cycles; independent ops issue one per cycle).
The arithmetic is `ssb_engine::audio::reverb::vfpu_model` (the exactness
proof and host tests live there). No ROM data: the output is code only.
"""
import os
import sys

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..')
OUT = os.path.join(ROOT, 'psp-runtime', 'src', 'reverb_vfpu.rs')

LAT = 5  # dependent-op latency, cycles (PSP-2000 probe)
CHUNK = 28  # instructions per vfpu_asm! block
HALF, MAX, MIN = 'C020', 'C030', 'C100'
# raw 8-sample column and its two s16 pairs, per window
RAW = {'x': ('C110', 'C110', 'C112'), 'y': ('C120', 'C120', 'C122'), 'o': ('C130', 'C130', 'C132')}
# per-group registers
GR = {
    'A': dict(X='C200', cX='C210', Y='C220', cY='C230', O='C300', cO='C310',
              T1='C320', T2='C330', T3='C400', U1='C620', U2='C630', U3='C700'),
    'B': dict(X='C410', cX='C420', Y='C430', cY='C500', O='C510', cO='C520',
              T1='C530', T2='C600', T3='C610', U1='C710', U2='C720', U3='C730'),
}
COEF = {'ff': ('S000', 'S001'), 'fb': ('S002', 'S003'), 'gain': ('S010', 'S011')}
NAMES = {1: 'ff', 2: 'fb', 3: 'ff, fb', 4: 'gain', 5: 'ff, gain', 6: 'fb, gain', 7: 'ff, fb, gain'}


def body(mask):
    """Scheduled instructions of one step for `mask`: (ops, cycles, count)."""
    ff, fb, gn = mask & 1, mask & 2, mask & 4
    ops = []  # (text, dst list, src list)

    def op(t, d, s):
        ops.append((t, d, s))

    needx, needo = bool(ff or fb), bool(gn)
    loads = [('x', 'X', 'cX', 'U2', fb)] if needx else []
    loads += [('y', 'Y', 'cY', 'U3', ff)]
    if needo:
        loads += [('o', 'O', 'cO', 'U1', gn)]
    for name, v, c, it, needc in loads:
        raw, pa, pb = RAW[name]
        op(f"lv.q {raw}, 0({{{name}}})", [raw], ['mem_' + name])
        for g, p in (('A', pa), ('B', pb)):
            r = GR[g]
            op(f"vs2i.p {r[it]}, {p}", [r[it]], [raw])
            op(f"vi2f.q {r[v]}, {r[it]}, 16", [r[v]], [r[it]])
            if needc:
                op(f"vi2f.q {r[c]}, {r[it]}, 31", [r[c]], [r[it]])
                op(f"vsub.q {r[c]}, {HALF}, {r[c]}", [r[c]], [HALF, r[c]])

    def mix(g, dst, cdst, src, coef, T1, T2, T3):
        r = GR[g]
        sh, sl = COEF[coef]
        D, C, S, t1, t2, t3 = r[dst], r[cdst], r[src], r[T1], r[T2], r[T3]
        op(f"vscl.q {t1}, {S}, {sh}", [t1], [S, 'C000' if sh[1:3] == '00' else 'C010'])
        op(f"vscl.q {t3}, {S}, {sl}", [t3], [S, 'C000' if sl[1:3] == '00' else 'C010'])
        op(f"vf2id.q {t2}, {t1}, 0", [t2], [t1])
        op(f"vadd.q {t3}, {t3}, {C}", [t3], [t3, C])
        op(f"vi2f.q {t2}, {t2}, 0", [t2], [t2])
        op(f"vsub.q {t1}, {t1}, {t2}", [t1], [t1, t2])
        op(f"vadd.q {D}, {D}, {t2}", [D], [D, t2])
        op(f"vadd.q {t1}, {t1}, {t3}", [t1], [t1, t3])
        op(f"vf2id.q {t1}, {t1}, 0", [t1], [t1])
        op(f"vi2f.q {t1}, {t1}, 0", [t1], [t1])
        op(f"vadd.q {D}, {D}, {t1}", [D], [D, t1])
        op(f"vmin.q {D}, {D}, {MAX}", [D], [D, MAX])
        op(f"vmax.q {D}, {D}, {MIN}", [D], [D, MIN])

    for g in 'AB':
        if ff:
            mix(g, 'Y', 'cY', 'X', 'ff', 'T1', 'T2', 'T3')
    for g in 'AB':
        if fb:
            mix(g, 'X', 'cX', 'Y', 'fb', 'T1', 'T2', 'T3')
        if gn:
            mix(g, 'O', 'cO', 'Y', 'gain', 'U1', 'U2', 'U3')
    stores = []
    if ff:
        stores.append(('y', 'Y', 'cY'))
    if fb:
        stores.append(('x', 'X', 'cX'))
    if gn:
        stores.append(('o', 'O', 'cO'))
    for name, v, c in stores:
        raw, pa, pb = RAW[name]
        for g, p in (('A', pa), ('B', pb)):
            r = GR[g]
            op(f"vf2iz.q {r[c]}, {r[v]}, 16", [r[c]], [r[v]])
            op(f"vi2s.q {p}, {r[c]}", [raw], [r[c], raw])
        op(f"sv.q {raw}, 0({{{name}}})", ['mem_' + name], [raw])

    # dependencies, in program order: RAW (latency), WAR / WAW (order)
    n = len(ops)
    deps = [[] for _ in range(n)]
    last_w, readers = {}, {}
    for i, (t, d, s) in enumerate(ops):
        for x in s:
            if x in last_w:
                deps[i].append((last_w[x], LAT))
        for x in d:
            if x in last_w:
                deps[i].append((last_w[x], 1))
            for j in readers.get(x, []):
                deps[i].append((j, 1))
        for x in s:
            readers.setdefault(x, []).append(i)
        for x in d:
            last_w[x] = i
            readers[x] = []
    succ = [[] for _ in range(n)]
    for i in range(n):
        for j, lat in deps[i]:
            succ[j].append((i, lat))
    # list scheduling: longest path to the end first, then program order
    prio = [0] * n
    for i in reversed(range(n)):
        prio[i] = max([lat + prio[k] for k, lat in succ[i]], default=1)
    done_at = {}
    sched = []
    cyc = 0
    left = set(range(n))
    while left:
        ready = [i for i in left if all(j in done_at and done_at[j] + lat <= cyc for j, lat in deps[i])]
        if ready:
            i = max(ready, key=lambda i: (prio[i], -i))
            sched.append(i)
            done_at[i] = cyc
            left.remove(i)
        cyc += 1
    return [ops[i][0] for i in sched], cyc, n


HEAD = '''//! The reverb's plain sections on the VFPU ([`section`], an
//! `ssb_engine::audio::reverb::SectionKernel`), bit-identical to the
//! integer path: `reverb::vfpu_model` is this kernel step for step in
//! `f32`, with the proof that every intermediate is exact and the host
//! tests against `mixq`; the `audio_selftest` hashes check it on the PSP.
//!
//! The step bodies are list-scheduled for the VFPU's latency (generated:
//! one body per set of nonzero coefficients, as `section` skips a zero
//! coefficient's mix). Registers: matrices 0-7, the constants loaded on
//! every call (no state kept between calls). The audio thread owns its
//! VFPU context; [`with_saved`] keeps the game thread's registers (gum's
//! matrices) around the selftest.
//!
//! Written by `tools/audio/gen_reverb_vfpu.py` (code only, no ROM data):
//! change the script and rerun it; never edit this file by hand.

/// Constants: `C000` = `[ff >> 8, ff & 255, fb >> 8, fb & 255]` and `C010`
/// = `[gain >> 8, gain & 255, -, -]` as `vfpu_model::split` scales them;
/// `C020` = 1/2, `C030` = 32767, `C100` = -32768.
#[repr(C, align(16))]
struct Consts([f32; 20]);

/// `y = mixq(y, x, ff)`, `x = mixq(x, y, fb)`, `out = mixq(out, y, gain)`
/// per sample (a zero coefficient's mix skipped), 8 samples per step as
/// two groups of 4.
///
/// # Safety
///
/// As `SectionKernel`: `x`, `y`, `out` 16-byte aligned, disjoint, `n`
/// samples each, `n` a multiple of 8; the calling thread has the VFPU.
pub unsafe fn section(x: *mut i16, y: *mut i16, out: *mut i16, n: usize, coef: [i32; 3]) {
    let split = |g: i32| ((g >> 8) as f32 / 128.0, (g & 255) as f32 / 32768.0);
    let ((ffh, ffl), (fbh, fbl), (gh, gl)) = (split(coef[0]), split(coef[1]), split(coef[2]));
    let h = 0.5;
    let c = Consts([
        ffh, ffl, fbh, fbl, gh, gl, 0.0, 0.0, h, h, h, h, 32767.0, 32767.0, 32767.0, 32767.0,
        -32768.0, -32768.0, -32768.0, -32768.0,
    ]);
    // SAFETY: `c` is a live, 16-byte-aligned 80-byte block.
    unsafe {
        psp::vfpu_asm!(
            "lv.q C000,  0({c})",
            "lv.q C010, 16({c})",
            "lv.q C020, 32({c})",
            "lv.q C030, 48({c})",
            "lv.q C100, 64({c})",
            c = in(reg) (&c),
            options(nostack),
        );
    }
    let mask = (coef[0] != 0) as u8 | ((coef[1] != 0) as u8) << 1 | ((coef[2] != 0) as u8) << 2;
    // One loop per mask, the step inlined (no call per 8 samples).
    macro_rules! run {
        ($step:ident) => {{
            let mut k = 0;
            while k < n {
                // SAFETY: the caller's contract: 8 aligned samples at `k`
                // in each window.
                unsafe { $step(x.add(k), y.add(k), out.add(k)) };
                k += 8;
            }
        }};
    }
    match mask {
'''

WITH_SAVED = '''/// Runs `f` with the calling thread's 128 VFPU registers saved and
/// restored around it: for the selftest, which runs [`section`] on the
/// game thread.
pub fn with_saved<R>(f: impl FnOnce() -> R) -> R {
    #[repr(C, align(16))]
    struct Regs([[f32; 4]; 32]);
    let mut r = Regs([[0.0; 4]; 32]);
    let p = r.0.as_mut_ptr();
    // SAFETY: `r` is a live, 16-byte-aligned 512-byte block.
    unsafe { save(p) };
    let v = f();
    // SAFETY: as above.
    unsafe { restore(p) };
    v
}
'''


def all_columns(name, insn, verb):
    """`save` / `restore`: all 32 columns, two blocks of 16."""
    s = f'''
/// {verb} all 32 VFPU columns {'to' if insn == 'sv.q' else 'from'} `p` (512 bytes, 16-byte aligned).
unsafe fn {name}(p: *mut [f32; 4]) {{
    // SAFETY: the caller's contract.
    unsafe {{
'''
    for half, arg in ((0, 'p'), (1, '(p.add(16))')):
        s += "        psp::vfpu_asm!(\n"
        for i in range(16):
            col = 16 * half + i
            s += f'            "{insn} C{col // 4}{col % 4}0, {16 * i}({{p}})",\n'
        s += f"            p = in(reg) {arg},\n            options(nostack),\n        );\n"
    return s + "    }\n}\n"


def generate():
    out = HEAD
    for m in range(1, 8):
        out += f"        {m} => run!(step{m}),\n"
    out += "        _ => {}\n    }\n}\n"
    for m in range(1, 8):
        ops, _cyc, n = body(m)
        used = ''.join(ops)

        def pn(p):
            return p if '{' + p + '}' in used else '_' + p

        out += f'''
/// One step of [`section`] with {NAMES[m]} nonzero ({n} instructions).
///
/// # Safety
///
/// As [`section`], for 8 samples.
#[inline(always)]
unsafe fn step{m}({pn('x')}: *mut i16, {pn('y')}: *mut i16, {pn('o')}: *mut i16) {{
    // SAFETY: the caller's contract.
    unsafe {{
'''
        for i in range(0, len(ops), CHUNK):
            ch = ops[i:i + CHUNK]
            out += "        psp::vfpu_asm!(\n"
            for line in ch:
                out += f'            "{line}",\n'
            for p in 'xyo':
                if '{' + p + '}' in ''.join(ch):
                    out += f"            {p} = in(reg) {p},\n"
            out += "            options(nostack),\n        );\n"
        out += "    }\n}\n"
    out += '\n' + WITH_SAVED
    out += all_columns('save', 'sv.q', 'Stores')
    out += all_columns('restore', 'lv.q', 'Loads')
    return out


def main():
    text = generate()
    if sys.argv[1:] == ['--check']:
        with open(OUT) as f:
            if f.read() != text:
                print(f"{OUT}: stale; run tools/audio/gen_reverb_vfpu.py", file=sys.stderr)
                sys.exit(1)
        return
    if sys.argv[1:]:
        sys.exit(__doc__)
    with open(OUT, 'w') as f:
        f.write(text)


if __name__ == '__main__':
    main()
