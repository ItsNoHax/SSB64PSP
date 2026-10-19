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

The `pspbuild` commands below need
[pspbuild](https://github.com/ItsNoHax/pspbuild), installed with
`cargo install --locked pspbuild`.

## ICON1

`xmb/ICON1_source.mp4` is the source. `xmb/ICON1.PMF` is generated from it and
committed, so a build needs no encoder:

```bash
pspbuild video icon1 psp-game/xmb/ICON1_source.mp4 -o psp-game/xmb/ICON1.PMF
pspbuild video icon1 psp-asset-viewer/xmb/ICON1_source.mp4 -o psp-asset-viewer/xmb/ICON1.PMF
```

This needs `ffmpeg` with `libx264`. ffmpeg can read PMF but cannot write it,
so pspbuild has ffmpeg encode the H.264 stream and writes the container
itself; its `docs/VIDEO.md` lists the format rules. The layout copies a
Sony-authored 144 × 80 PMF (`pspautotests/tests/video/pmf/test.pmf` in the
PPSSPP repository):

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

List each slot in a built `EBOOT.PBP` with its offset, size and kind, and
check the PNG slots' sizes and alpha:

```bash
pspbuild inspect psp-game/target/mipsel-sony-psp/release/EBOOT.PBP
```

A PNG of the wrong size, or one with transparency, gets a `WARNING:` line.
The game's `ICON0.PNG` is RGBA and gets one. pspbuild's evidence is mixed:
some firmware and packers are reported to reject alpha there, but an RGBA
ICON0 shows on a PSP Slim (6.61).

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
pspbuild audio snd0 --exact /tmp/snd0.wav -o psp-game/xmb/SND0.AT3
pspbuild audio inspect --strict psp-game/xmb/SND0.AT3
```

ffmpeg has no ATRAC3 encoder.
[pspbuild](https://github.com/ItsNoHax/pspbuild)'s native encoder low-passes the source below
the top QMF band and writes LP4 (66144 bps, joint stereo) in a RIFF container
with every frame coding three QMF bands; the XMB plays nothing if a frame
codes a fourth band, although ffmpeg decodes such a file without complaint. It
also writes `fact` and `smpl` chunks, so the XMB repeats the loop instead of
stopping after one pass. `audio inspect --strict` checks all of this; a good
file ends with:

```
Loop:                samples 1024 to 1129983 (25.60 s), forever
Verdict:             playable; matches the profile pspbuild writes
```

The XMB limits SND0 to about 500 KB and 55 s.
