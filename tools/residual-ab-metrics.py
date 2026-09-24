#!/usr/bin/env python3
"""RE-312 A/B frame metrics (see tools/residual-ab-capture.sh) at native PSP resolution (480x272).

usage: ab_metrics.py NAME A.png B.png OUTDIR [HIDE.png]

A = current shipped, B = candidate, HIDE = candidate pack with the site's
direct texture fully transparent (cutout sites only). Writes
NAME-{a,b,diff,crop}.png and NAME.json into OUTDIR.
"""
import json
import sys

import numpy as np
from PIL import Image


def load(p):
    im = np.asarray(Image.open(p).convert("RGB")).astype(np.int16)
    assert im.shape == (272, 480, 3), (p, im.shape)
    return im


def heat(d):
    # d: 0..255 max-channel difference, amplified x8 then colour-mapped.
    v = np.clip(d.astype(np.int32) * 8, 0, 255).astype(np.uint8)
    out = np.zeros(d.shape + (3,), np.uint8)
    out[..., 0] = v
    out[..., 1] = np.where(d >= 16, v, v // 3)
    out[..., 2] = np.where(d >= 32, 255, 0)
    out[d == 0] = 0
    return out


def main():
    name, a_p, b_p, out = sys.argv[1:5]
    hide_p = sys.argv[5] if len(sys.argv) > 5 else None
    a, b = load(a_p), load(b_p)
    d = np.abs(a - b).max(axis=2)
    changed = d > 0
    m = {
        "name": name,
        "pixels": int(d.size),
        "changed": int(changed.sum()),
        "max_channel": int(d.max()),
        "mean_changed": round(float(d[changed].mean()), 3) if changed.any() else 0.0,
        "ge8": int((d >= 8).sum()),
        "ge16": int((d >= 16).sum()),
        "ge32": int((d >= 32).sum()),
    }
    if hide_p:
        h = load(hide_p)
        drawn_a = np.abs(a - h).max(axis=2) > 0
        drawn_b = np.abs(b - h).max(axis=2) > 0
        m["site_pixels_a"] = int(drawn_a.sum())
        m["site_pixels_b"] = int(drawn_b.sum())
        m["silhouette_changed"] = int((drawn_a ^ drawn_b).sum())
        m["silhouette_gained"] = int((drawn_b & ~drawn_a).sum())
        m["silhouette_lost"] = int((drawn_a & ~drawn_b).sum())
    Image.fromarray(a.astype(np.uint8)).save(f"{out}/{name}-a.png")
    Image.fromarray(b.astype(np.uint8)).save(f"{out}/{name}-b.png")
    Image.fromarray(heat(d)).save(f"{out}/{name}-diff.png")
    if changed.any():
        ys, xs = np.nonzero(changed)
        y0, y1 = max(ys.min() - 6, 0), min(ys.max() + 7, 272)
        x0, x1 = max(xs.min() - 6, 0), min(xs.max() + 7, 480)
        # Cap the crop at 120x68 around the strongest change.
        if x1 - x0 > 120 or y1 - y0 > 68:
            cy, cx = np.unravel_index(np.argmax(d), d.shape)
            x0 = int(np.clip(cx - 60, 0, 360))
            y0 = int(np.clip(cy - 34, 0, 204))
            x1, y1 = x0 + 120, y0 + 68
        m["crop"] = [int(x0), int(y0), int(x1), int(y1)]
        scale = int(max(1, min(8, 480 // int(x1 - x0))))
        tiles = [a[y0:y1, x0:x1].astype(np.uint8), b[y0:y1, x0:x1].astype(np.uint8), heat(d[y0:y1, x0:x1])]
        gap = np.full((y1 - y0, 1, 3), 255, np.uint8)
        row = np.concatenate([tiles[0], gap, tiles[1], gap, tiles[2]], axis=1)
        big = row.repeat(scale, axis=0).repeat(scale, axis=1)
        Image.fromarray(big).save(f"{out}/{name}-crop.png")
        m["crop_scale"] = scale
    json.dump(m, open(f"{out}/{name}.json", "w"), indent=1, sort_keys=True)
    print(json.dumps(m, sort_keys=True))


if __name__ == "__main__":
    main()
