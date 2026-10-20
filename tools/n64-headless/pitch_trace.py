"""Per-frame fighter joint trace from a scripted pad timeline (RE-503).

Run inside the M64Py flatpak (see .claude/skills/n64-emulator/SKILL.md):
  flatpak run --env=SDL_VIDEODRIVER=offscreen --command=python3 \
      net.sourceforge.m64py.M64Py pitch_trace.py ROM OUT WORKDIR \
      "4:1:a;41:200:b+x=-45+y=70" LAST [--scene 54] [--joint 4]

Windows: ';'-separated 'start:len:btn+btn+x=N+y=N' (route tokens of
n64_driver.parse_route). Row t is frame t of scene --scene (t=0: its first
frame); the pad read in frame t is the one whose window covers t.
Per row and player: status_id, lr, ga, vel_air x/y, TopN position x/y,
joint rotate x/y/z and scale x/y/z, as f32 bits then decimals.
"""
import argparse
import os
import struct
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from n64_driver import Mupen64PlusHarness, parse_route  # noqa: E402

SCENE_CURR = 0x800A4AD0     # gSCManagerSceneData.scene_curr (u8, first byte)
BATTLE_STATE = 0x800A50E8   # gSCManagerBattleState
FT_STATUS, FT_LR, FT_VEL_AIR, FT_GA, FT_JOINTS = 0x24, 0x44, 0x48, 0x14C, 0x8E8
DOBJ_TRANSLATE = 0x1C       # DObj.translate.vec (TopN: the fighter's position)


def windows(spec):
    out = []
    for tok in spec.split(";"):
        if not tok.strip():
            continue
        start, length, btn = tok.split(":", 2)
        ((_, value, _),) = parse_route("1:" + btn)
        out.append((int(start), int(length), value))
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("rom")
    ap.add_argument("out")
    ap.add_argument("workdir", help="writable config/data dir inside the sandbox")
    ap.add_argument("script")
    ap.add_argument("last", type=int)
    ap.add_argument("--scene", type=int, default=54, help="scene id gating t=0 (54: Training)")
    ap.add_argument("--joint", type=int, default=4)
    ap.add_argument("--players", type=int, default=2)
    a = ap.parse_args()
    so = os.path.join(os.path.dirname(os.path.abspath(__file__)), "build", "n64_input.so")
    for sub in ("config", "data"):
        os.makedirs(os.path.join(a.workdir, sub), exist_ok=True)
    h = Mupen64PlusHarness(a.rom, so, os.path.join(a.workdir, "config"), os.path.join(a.workdir, "data"))
    h.start()
    pads = windows(a.script)
    u32 = h.read_u32

    def ok(p):
        return 0x80000000 <= p < 0x80800000

    def flt(bits):
        return struct.unpack("<f", struct.pack("<I", bits))[0]

    def s32(v):
        return v - (1 << 32) if v & 0x80000000 else v

    start, t, out = None, -1, open(a.out, "w")
    while True:
        pad = 0
        for s, n, v in pads:
            if s <= t + 1 < s + n:
                pad = v
        h.set_input(0, pad)
        h.advance_frame(timeout=60.0)
        if u32(SCENE_CURR) >> 24 != a.scene:
            if start is not None or h.current_frame > 4000:
                break
            continue
        if start is None:
            start = h.current_frame
            out.write("start %d\n" % start)
        t = h.current_frame - start
        if t > a.last:
            break
        row = ["t=%d in=%08x" % (t, pad)]
        bs = u32(BATTLE_STATE)
        for p in range(a.players if ok(bs) else 0):
            g = u32(bs + 0x20 + p * 0x74 + 0x58)
            fp = u32(g + 0x84) if ok(g) else 0
            if not ok(fp):
                continue
            j = u32(fp + FT_JOINTS + 4 * a.joint)
            vel = [u32(fp + FT_VEL_AIR + 4 * i) for i in range(2)]
            row.append("p%d st=%d lr=%d ga=%d v=%08x,%08x (%.4f,%.4f)" % (
                p, s32(u32(fp + FT_STATUS)), s32(u32(fp + FT_LR)), u32(fp + FT_GA),
                *vel, *map(flt, vel)))
            topn = u32(fp + FT_JOINTS)
            if ok(topn):
                pos = [u32(topn + DOBJ_TRANSLATE + 4 * i) for i in range(2)]
                row.append("pos=%.4f,%.4f" % tuple(map(flt, pos)))
            if ok(j):
                rot = [u32(j + 0x30 + 4 * i) for i in range(3)]
                sca = [u32(j + 0x40 + 4 * i) for i in range(3)]
                row.append("rot=%08x,%08x,%08x sc=%08x,%08x,%08x (%.6f,%.6f,%.6f / %.4f,%.4f,%.4f)" % (
                    *rot, *sca, *map(flt, rot), *map(flt, sca)))
        out.write(" ".join(row) + "\n")
        out.flush()
    out.close()
    h.stop()


if __name__ == "__main__":
    main()
