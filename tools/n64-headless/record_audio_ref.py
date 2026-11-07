#!/usr/bin/env python3
"""Record an N64 audio reference clip with n64_audio_dump.so and annotate it.

Runs a --route (or a --preset) like n64_driver.py with --audio-dump, and
writes OUT.steps.txt beside the dump: the advance and callback frame of every
route step, every scene change (gSCManagerSceneData.scene_curr), the sound
quality, and in How to Play every damage change of either fighter. Run
through the launcher (it builds the plugins and the seed shim):

    tools/run-n64-headless.sh --script record_audio_ref.py \\
        --out ~/ppsspp-test/x/clip.raw --route "1450:" --force-stereo

LLE reference of record (RE-479; crates/ssb-engine/src/audio/n64ref.rs):
RMG flatpak core, rsp-parallel, ParaLLEl-RDP, the preset's srand seed:

    tools/run-n64-headless.sh --rmg --script record_audio_ref.py \\
        --preset b_menu --out ~/ppsspp-test/x/menu_par.raw
    python3 tools/n64-headless/capture_frames.py ~/ppsspp-test/x/menu_par.raw

The core seeds rand() from time(NULL) and draws PI/SI interrupt delays from
it (RandomizeInterrupt): LLE captures change run to run unless --seed pins it
(n64_srand.c). A preset carries the seed of its n64ref.rs capture; with it
the re-recording is byte-identical. RMG profile cartridge save: --sram
(default fresh; the presets do not depend on it).

--force-stereo: the M64Py core always loads the shared SRAM save
(~/.var/app/net.sourceforge.m64py.M64Py/data/mupen64plus/save, whatever
SaveSRAMPath says), and that save is set to Mono. The flag writes the
fresh-cartridge default (Stereo) into dSYAudioSoundQuality and the loaded
backup's option byte after the first frame, before the game's first sound.

Addresses are US-ROM (refs/ssb-decomp-re build map); RDRAM words are
host-endian, so single bytes live at address ^ 3.
"""
import argparse
import hashlib
import os
import shutil
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import n64_driver as d  # noqa: E402

SCENE_CURR = 0x800A4AD0        # gSCManagerSceneData.scene_curr (u8)
SOUND_QUALITY = 0x8003CB24     # dSYAudioSoundQuality (s32): 0 mono, 1 stereo
BACKUP_SOUND = 0x800A44E0 + 0x451  # gSCManagerBackupData.sound_mono_or_stereo
EXPLAIN_BATTLE = 0x8018E7F0    # How to Play battle state (RE-466)
SCENE_EXPLAIN = 60

# Presets: the RE-479 LLE references replayed by crates/ssb-engine/src/audio/
# n64ref.rs (B_MENU, C_ATTRACT): route and the srand seed the capture ran
# with (its Unix start time). Advances, not audio frames: the dump trails the
# advance count by three frames. B_MENU = every value of the clip; C_ATTRACT =
# its first 3115 audio frames (capture_frames.py --first-frames 3115).
PRESETS = {
    # Start in the opening at 200, Start on the title at 355, D-pad down at
    # 480 and 530, up at 580, A on VS Mode at 630, B at 725; 860 advances.
    "b_menu": ("200:;5:start;150:;5:start;120:;5:dpad_d;45:;5:dpad_d;45:;5:dpad_u;45:;5:a;90:;5:b;130:",
               1793126694),
    # No input: the attract loop's opening fights; 3200 advances.
    "c_attract": ("3200:", 1793122166),
}
WORK_MARK = ".record_audio_ref"


def save_name(rom):
    """The RMG core's save basename: header name (no ROM database) + MD5 prefix."""
    with open(rom, "rb") as f:
        data = f.read()
    name = data[0x20:0x34].decode("ascii", "replace").strip(" \0")
    return f"{name}-{hashlib.md5(data).hexdigest()[:8].upper()}.sra"


def rmg_work_dir(args):
    """OUT.n64/{cfg,data,save}: per-recording config and cartridge save (RMG profile)."""
    work = args.out + ".n64"
    if os.path.exists(work):
        if not os.path.exists(os.path.join(work, WORK_MARK)):
            raise SystemExit(f"{work} exists and was not made by this script")
        shutil.rmtree(work)
    os.makedirs(os.path.join(work, "save"))
    open(os.path.join(work, WORK_MARK), "w").close()
    if args.sram not in ("fresh", "shared"):
        shutil.copyfile(args.sram, os.path.join(work, "save", save_name(args.rom)))
    return work


def main():
    here = os.path.dirname(os.path.abspath(__file__))
    ap = argparse.ArgumentParser()
    ap.add_argument("--rom", required=True)
    ap.add_argument("--out", required=True, help="raw PCM path; OUT.txt and OUT.steps.txt go beside it")
    ap.add_argument("--route", default=None, help="n64_driver.py route grammar")
    ap.add_argument("--preset", choices=sorted(PRESETS),
                    help="route and seed of an n64ref.rs reference clip; implies --force-stereo "
                         "(use with --rmg; --seed overrides the preset's)")
    ap.add_argument("--shots", default="", help="comma-separated advance counts to screenshot after")
    ap.add_argument("--force-stereo", action="store_true")
    ap.add_argument("--input-so", default=os.path.join(here, "build", "n64_input.so"))
    ap.add_argument("--audio-dump-so", default=os.path.join(here, "build", "n64_audio_dump.so"))
    ap.add_argument("--config-dir", default=None,
                    help="default: the M64Py flatpak's; RMG profile: OUT.n64/cfg (written by the driver)")
    ap.add_argument("--data-dir", default=None,
                    help="default: the M64Py flatpak's; RMG profile: OUT.n64/data")
    ap.add_argument("--sram", default="fresh", metavar="fresh|shared|FILE",
                    help="RMG profile cartridge save: fresh = empty OUT.n64/save (default), shared = the RMG "
                         "flatpak's own save dir, FILE = copy of a .sra into OUT.n64/save")
    d.add_core_args(ap)
    args = ap.parse_args()
    if bool(args.route) == bool(args.preset):
        ap.error("give exactly one of --route, --preset")
    if args.preset:
        args.route, preset_seed = PRESETS[args.preset]
        args.force_stereo = True
        if args.seed is None:
            args.seed = preset_seed
        if not args.core or args.core == d.CORE_PATH:
            sys.stderr.write("[rec] warning: the presets are LLE references (RMG profile, --rmg); "
                             "this M64Py/HLE run will not match n64ref.rs\n")
    d.pin_seed(args.seed)
    shots = {int(x) for x in args.shots.split(",") if x}

    sram_dir = None
    if args.core and args.core != d.CORE_PATH:
        work = rmg_work_dir(args)
        args.config_dir = args.config_dir or os.path.join(work, "cfg")
        args.data_dir = args.data_dir or os.path.join(work, "data")
        if args.sram != "shared":
            sram_dir = os.path.join(work, "save")
    else:
        args.config_dir = args.config_dir or os.path.expanduser(
            "~/.var/app/net.sourceforge.m64py.M64Py/config/mupen64plus")
        args.data_dir = args.data_dir or os.path.expanduser(
            "~/.var/app/net.sourceforge.m64py.M64Py/data/mupen64plus")

    h = d.Mupen64PlusHarness(args.rom, args.input_so, args.config_dir, args.data_dir,
                             audio_dump_path=args.out, audio_dump_so_path=args.audio_dump_so,
                             core_path=args.core, rsp=args.rsp, gfx=args.gfx, sram_dir=sram_dir)
    h.start()

    def u32(a):
        return int.from_bytes(h.read_rdram(a, 4), "little")

    def u8(a):
        return h.read_rdram(a ^ 3, 1)[0]

    def valid(p):
        return 0x80000000 <= p < 0x80800000

    def fighters():
        out = []
        for i in range(2):
            gobj = u32(EXPLAIN_BATTLE + 0x20 + i * 0x74 + 0x58)
            if not valid(gobj):
                return None
            fp = u32(gobj + 0x84)
            if not valid(fp):
                return None
            out.append((u32(fp + 0x2C), u32(fp + 0x40)))  # percent_damage, hitlag_tics
        return out

    log = open(args.out + ".steps.txt", "w")
    n = 0
    last_scene = last_q = None
    last_dmg = {}
    for hold, p1, p2 in d.parse_route(args.route):
        log.write(f"step_start advance {n} cb_frame {h.current_frame} hold {hold} p1 0x{p1:08x}\n")
        for _ in range(hold):
            h.set_input(0, p1)
            h.set_input(1, p2)
            for attempt in range(3):
                try:
                    h.advance_frame(timeout=30.0)
                    break
                except TimeoutError:
                    sys.stderr.write(f"[rec] frame stall at advance {n}, retry {attempt + 1}\n")
            else:
                raise TimeoutError(f"advance {n} stalled 3 times")
            n += 1
            if n == 1 and args.force_stereo:
                h.write_rdram(SOUND_QUALITY, (1).to_bytes(4, "little"))
                h.write_rdram(BACKUP_SOUND ^ 3, bytes([1]))
                log.write("force_stereo after advance 1\n")
            sc = u8(SCENE_CURR)
            if sc != last_scene:
                log.write(f"scene {sc} at advance {n} cb_frame {h.current_frame}\n")
                last_scene = sc
            q = u32(SOUND_QUALITY)
            if q != last_q:
                log.write(f"sound_quality {q} at advance {n}\n")
                last_q = q
            if sc == SCENE_EXPLAIN:
                fs = fighters()
                if fs is not None:
                    for i, (dmg, lag) in enumerate(fs):
                        if last_dmg.get(i) is not None and dmg != last_dmg[i]:
                            log.write(f"hit p{i} damage {last_dmg[i]}->{dmg} hitlag {lag} "
                                      f"at advance {n} cb_frame {h.current_frame}\n")
                        last_dmg[i] = dmg
            if n % 500 == 0:
                sys.stderr.write(f"[rec] advance {n}\n")
                sys.stderr.flush()
            if n in shots:
                h.screenshot()
                log.write(f"shot advance {n} cb_frame {h.current_frame} {h.latest_screenshot()}\n")
            log.flush()
    log.write(f"end advance {n} cb_frame {h.current_frame}\n")
    log.close()
    h.stop()


if __name__ == "__main__":
    main()
