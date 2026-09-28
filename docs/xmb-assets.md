# XMB assets

What the PSP's XMB shows for the game and for the asset viewer. Each crate has
its own set in `xmb/`, and `Psp.toml` points `cargo psp` at it:

| Slot | File | Format |
|---|---|---|
| `ICON0` | `xmb/ICON0.PNG` | 144 × 80 RGBA PNG, transparent rounded corners |
| `ICON1` | `xmb/ICON1.PMF` | 144 × 80 H.264 PMF, 29.97 fps, seamless loop |
| `PIC0` | `xmb/PIC0.PNG` | 310 × 180 RGBA PNG, transparent background, drawn over `PIC1` |
| `PIC1` | `xmb/PIC1.PNG` | 480 × 272 RGB PNG |
| `SND0` | `psp-game/xmb/SND0.AT3` | ATRAC3 66 kbps (LP4), 44.1 kHz stereo, RIFF; shared by both crates |

The loops are 7.2 s for the game and 4.8 s for the viewer. The PNGs are
committed exactly as delivered. `ICON0` and `PIC0` keep their alpha channel
because the art depends on it. AngleZero strips alpha from its `ICON0`, but
its art is fully opaque, so that is not evidence against alpha.

## ICON1

`xmb/ICON1_source.mp4` is the source. `xmb/ICON1.PMF` is generated from it and
committed, so a build needs no encoder:

```bash
python3 tools/make-pmf.py psp-game/xmb/ICON1_source.mp4 psp-game/xmb/ICON1.PMF
python3 tools/make-pmf.py psp-asset-viewer/xmb/ICON1_source.mp4 psp-asset-viewer/xmb/ICON1.PMF
```

The script needs `ffmpeg` with `libx264`. ffmpeg can read PMF but cannot write
it, so the script re-encodes the video and writes the container itself. Its
docstring lists the format rules. The layout copies a Sony-authored 144 × 80
PMF (`pspautotests/tests/video/pmf/test.pmf` in the PPSSPP repository):

- H.264 Main profile, level 2.1, CABAC, I and P pictures only, one reference
  frame, 59-frame GOPs.
- Access-unit delimiters, plus buffering-period and pic-timing SEI.
- 2048-byte MPEG-2 program-stream packs. Each GOP starts a new pack, which
  holds the system header and a private-stream-2 index of the GOP's access
  units.

To check a result, decode it and compare its layout with the reference:

```bash
ffprobe -v error -count_frames -show_entries stream=profile,level,nb_read_frames psp-game/xmb/ICON1.PMF
```

ffmpeg decoding the file does not prove the XMB will play it. The console's
own decoder is stricter than ffmpeg. Only a physical PSP shows the animated
icon. Both generated icons animate in the XMB of a PSP Slim with firmware
6.61 and ARK, tested on 2026-09-28 with commit `e1acd5d`.

## Checking a built EBOOT

List the size of each slot in a built `EBOOT.PBP`:

```bash
python3 - psp-game/target/mipsel-sony-psp/release/EBOOT.PBP <<'PY'
import struct, sys
d = open(sys.argv[1], 'rb').read()
o = struct.unpack('<8I', d[8:40])
for i, n in enumerate(['PARAM.SFO','ICON0.PNG','ICON1.PMF','PIC0.PNG','PIC1.PNG','SND0.AT3','DATA.PSP','DATA.PSAR']):
    size = (o[i+1] if i+1 < 8 else len(d)) - o[i]
    print(f'{n:<12}{size:>9} bytes')
PY
```

## SND0

The music is an original 25.6 s loop: 150 BPM in E minor, 16 bars, with
drums, bass, chord stabs and a lead melody. `tools/make-snd0-loop.py`
synthesizes it from nothing; it uses no samples and no music from the
original game ([D-037](decisions/D-037.md)). The loop is seamless because
note and echo tails that run past the end wrap around to the start. The
viewer's `Psp.toml` points at the game's file, so both EBOOTs play the same
track.

```bash
python3 tools/make-snd0-loop.py /tmp/snd0.wav
tools/encode-snd0.sh /tmp/snd0.wav psp-game/xmb/SND0.AT3
```

ffmpeg has no ATRAC3 encoder. `tools/encode-snd0.sh` is AngleZero's
`scripts/encode_music.sh`, ported. It builds libsndfile and atracdenc in a
scratch directory and applies `tools/patches/atracdenc-psp-bands.patch`. It
then low-passes the source at 15.5 kHz, encodes LP4 in a RIFF container and
drops the `fact` chunk. Last, it rejects the file unless every frame codes
exactly three QMF bands. The XMB plays nothing if a frame codes a fourth
band, although ffmpeg decodes such a file without complaint. The script
passes `CMAKE_POLICY_VERSION_MINIMUM=3.5`, because CMake 4 refuses
libsndfile's and atracdenc's old minimum versions.

A good encode ends like this:

```
>> 1103 frames, all with bands_coded=2
>> psp-game/xmb/SND0.AT3 — atrac3, 66144bps, 25.613923s, 211836 bytes
```

The XMB limits SND0 to about 500 KB and 55 s.
