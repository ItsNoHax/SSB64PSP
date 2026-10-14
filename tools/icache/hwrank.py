#!/usr/bin/env python3
# Run plans and summaries for tools/hw-link-order.sh (RE-500).
# usage: hwrank.py plan SEED ROUNDS NAME...
#          one "ROUND NAME" line per run: every name once a round, each
#          round shuffled from SEED, never one name twice in a row
#        hwrank.py summ LOG
#          "REPORTS FPS FRAME_US MAX_US BUSY_US BUSY_MAX_US BFPS" from a
#          `prof n=` log: means over reports 3..18 (1-2 hold the loading),
#          the largest frame and busy time among them, how many reports
#          the log has, and BFPS = 1e6 / BUSY_US: the frame rate the
#          frame's work alone would allow (frame_timer's busy, RE-500; 0
#          without it)
#        hwrank.py rank RESULTS.tsv [REF]
#          per scene and name: runs, mean, min, max and span of BFPS (FPS
#          when a log has no busy time; span over 0.25 flagged: RE-499's
#          same-binary repeat), and the mean difference from REF in the
#          same round (the drift reference)
import random, re, sys
from collections import defaultdict

SPAN = 0.25

def plan(seed, rounds, names):
    rng = random.Random(seed)
    last = None
    for r in range(1, rounds + 1):
        order = list(names)
        while True:
            rng.shuffle(order)
            if order[0] != last or len(order) == 1:
                break
        last = order[-1]
        for n in order:
            print(r, n)

def summ(path):
    reports = []
    for l in open(path, errors='replace'):
        m = re.match(r'prof n=(\d+) (.*)', l)
        if not m:
            continue
        kv = dict(f.split('=', 1) for f in m.group(2).split() if '=' in f)
        reports.append((int(m.group(1)), float(kv['fps']), kv['frame'].split('/'), kv.get('busy', '0/0').split('/')))
    use = [r for r in reports if 3 <= r[0] <= 18]
    if not use:
        print(len(reports), 0, 0, 0, 0, 0, 0)
        return
    fps = sum(r[1] for r in use) / len(use)
    frame = sum(int(r[2][0]) for r in use) / len(use)
    mx = max(int(r[2][1]) for r in use)
    busy = sum(int(r[3][0]) for r in use) / len(use)
    bmx = max(int(r[3][1]) for r in use)
    bfps = 1e6 / busy if busy else 0
    print(len(reports), f'{fps:.3f}', f'{frame:.0f}', mx, f'{busy:.1f}', bmx, f'{bfps:.3f}')

def rank(path, ref):
    rows = [l.rstrip('\n').split('\t') for l in open(path) if l.strip() and not l.startswith('round')]
    by = defaultdict(list)
    refs = {}
    for rnd, scene, name, reports, fps, frame, mx, busy, bmx, bfps, *_ in rows:
        if int(reports) < 19:
            continue
        v = float(bfps) or float(fps)
        by[(scene, name)].append((rnd, v, float(busy) or float(frame), int(bmx) or int(mx)))
        if name == ref:
            refs[(scene, rnd)] = v
    for scene in sorted({s for s, _ in by}):
        refmean = None
        if (scene, ref) in by:
            v = [f for _, f, _, _ in by[(scene, ref)]]
            refmean = sum(v) / len(v)
        print(f'scene {scene}' + (f'  ref {ref} {refmean:.3f}' if refmean is not None else ''))
        print(f'{"name":<12} {"runs":>4} {"mean":>8} {"min":>7} {"max":>7} {"span":>5} {"vs ref":>7} {"vs ref/round":>12} {"busy_us":>8} {"max_us":>6}')
        out = []
        for (s, name), v in by.items():
            if s != scene:
                continue
            f = [x[1] for x in v]
            mean = sum(f) / len(f)
            span = max(f) - min(f)
            d = [x[1] - refs[(scene, x[0])] for x in v if (scene, x[0]) in refs]
            dr = sum(d) / len(d) if d else float('nan')
            dm = mean - refmean if refmean is not None else float('nan')
            out.append((mean, name, len(f), min(f), max(f), span, dm, dr, sum(x[2] for x in v) / len(v), max(x[3] for x in v)))
        for mean, name, n, lo, hi, span, dm, dr, fr, mx in sorted(out, reverse=True):
            flag = '  SPAN' if span > SPAN else ''
            print(f'{name:<12} {n:>4} {mean:>8.3f} {lo:>7.2f} {hi:>7.2f} {span:>5.2f} {dm:>+7.3f} {dr:>+12.3f} {fr:>8.0f} {mx:>6}{flag}')

cmd = sys.argv[1]
if cmd == 'plan':
    plan(int(sys.argv[2]), int(sys.argv[3]), sys.argv[4:])
elif cmd == 'summ':
    summ(sys.argv[2])
elif cmd == 'rank':
    rank(sys.argv[2], sys.argv[3] if len(sys.argv) > 3 else None)
else:
    sys.exit(f'unknown command {cmd}')
