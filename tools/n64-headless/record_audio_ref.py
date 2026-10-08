#!/usr/bin/env python3
"""Record an N64 audio reference clip with n64_audio_dump.so and annotate it.

Runs a --route like n64_driver.py with --audio-dump, and writes OUT.steps.txt
beside the dump: the advance and callback frame of every route step, every
scene change (gSCManagerSceneData.scene_curr), the sound quality, and in
How to Play every damage change of either fighter. Run inside the M64Py
flatpak, from a directory the sandbox can read:

    flatpak run --env=SDL_VIDEODRIVER=offscreen --command=python3 \\
        net.sourceforge.m64py.M64Py tools/n64-headless/record_audio_ref.py \\
        --rom ROM --out ~/ppsspp-test/x/clip.raw --route "1450:" --force-stereo

--force-stereo: the M64Py core always loads the shared SRAM save
(~/.var/app/net.sourceforge.m64py.M64Py/data/mupen64plus/save, whatever
SaveSRAMPath says), and that save is set to Mono. The flag writes the
fresh-cartridge default (Stereo) into dSYAudioSoundQuality and the loaded
backup's option byte after the first frame, before the game's first sound.

Addresses are US-ROM (refs/ssb-decomp-re build map); RDRAM words are
host-endian, so single bytes live at address ^ 3.
"""
import argparse
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import n64_driver as d  # noqa: E402

SCENE_CURR = 0x800A4AD0        # gSCManagerSceneData.scene_curr (u8)
SOUND_QUALITY = 0x8003CB24     # dSYAudioSoundQuality (s32): 0 mono, 1 stereo
BACKUP_SOUND = 0x800A44E0 + 0x451  # gSCManagerBackupData.sound_mono_or_stereo
EXPLAIN_BATTLE = 0x8018E7F0    # How to Play battle state (RE-466)
SCENE_EXPLAIN = 60


def main():
    here = os.path.dirname(os.path.abspath(__file__))
    ap = argparse.ArgumentParser()
    ap.add_argument("--rom", required=True)
    ap.add_argument("--out", required=True, help="raw PCM path; OUT.txt and OUT.steps.txt go beside it")
    ap.add_argument("--route", required=True, help="n64_driver.py route grammar")
    ap.add_argument("--shots", default="", help="comma-separated advance counts to screenshot after")
    ap.add_argument("--force-stereo", action="store_true")
    ap.add_argument("--input-so", default=os.path.join(here, "build", "n64_input.so"))
    ap.add_argument("--audio-dump-so", default=os.path.join(here, "build", "n64_audio_dump.so"))
    ap.add_argument("--config-dir", default=os.path.expanduser("~/.var/app/net.sourceforge.m64py.M64Py/config/mupen64plus"))
    ap.add_argument("--data-dir", default=os.path.expanduser("~/.var/app/net.sourceforge.m64py.M64Py/data/mupen64plus"))
    args = ap.parse_args()
    shots = {int(x) for x in args.shots.split(",") if x}

    h = d.Mupen64PlusHarness(args.rom, args.input_so, args.config_dir, args.data_dir,
                             audio_dump_path=args.out, audio_dump_so_path=args.audio_dump_so)
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
                    h.advance_frame(timeout=20.0)
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
            if n in shots:
                h.screenshot()
                log.write(f"shot advance {n} cb_frame {h.current_frame} {h.latest_screenshot()}\n")
            log.flush()
    log.write(f"end advance {n} cb_frame {h.current_frame}\n")
    log.close()
    h.stop()


if __name__ == "__main__":
    main()
