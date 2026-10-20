# XMB assets

Original work. One set per crate in `xmb/`, referenced from its `Psp.toml`.

| Slot | File | Format |
|---|---|---|
| `ICON0` | `xmb/ICON0.PNG` | 144 × 80 RGBA PNG, transparent rounded corners |
| `ICON1` | `xmb/ICON1.PMF` | 144 × 80 H.264 PMF, 29.97 fps, seamless loop |
| `PIC0` | `xmb/PIC0.PNG` | 310 × 180 RGBA PNG, transparent background, drawn over `PIC1` |
| `PIC1` | `xmb/PIC1.PNG` | 480 × 272 RGB PNG |
| `SND0` | `psp-game/xmb/SND0.AT3` | ATRAC3 66 kbps (LP4), 44.1 kHz stereo, RIFF; shared by both crates |

- ICON1 loop length: 7.2 s (game), 4.8 s (viewer)
- PNGs committed as delivered; `ICON0`, `PIC0` keep alpha (art uses it)
- Commands below need [pspbuild](https://github.com/ItsNoHax/pspbuild): `cargo install --locked pspbuild`

## ICON1

- Source: `xmb/ICON1_source.mp4`
- `xmb/ICON1.PMF`: generated, committed; builds need no encoder
- Needs `ffmpeg` with `libx264`

```bash
pspbuild video icon1 psp-game/xmb/ICON1_source.mp4 -o psp-game/xmb/ICON1.PMF
pspbuild video icon1 psp-asset-viewer/xmb/ICON1_source.mp4 -o psp-asset-viewer/xmb/ICON1.PMF
```

- ffmpeg cannot write PMF
  - pspbuild: ffmpeg encodes the H.264 stream, pspbuild writes the container
  - Rules: pspbuild `docs/VIDEO.md`
- Layout copies a Sony-authored 144 × 80 PMF (`pspautotests/tests/video/pmf/test.pmf`, PPSSPP repository)
- Stream profile:
  - H.264 Main, level 2.1, CABAC
  - I and P pictures only, one reference frame, 59-frame GOPs
  - Access-unit delimiters, buffering-period and pic-timing SEI
- Container:
  - 2048-byte MPEG-2 program-stream packs
  - Each GOP starts a new pack
  - Pack holds system header and private-stream-2 index of the GOP's access units

Check a result:

```bash
ffprobe -v error -count_frames -show_entries stream=profile,level,nb_read_frames psp-game/xmb/ICON1.PMF
```

- ffmpeg decoding proves nothing: console decoder is stricter
- Only a physical PSP shows the animated icon
- Verified: both icons animate on PSP Slim, firmware 6.61 with ARK (2026-09-28, `e1acd5d`)

## Checking a built EBOOT

```bash
pspbuild inspect psp-game/target/mipsel-sony-psp/release/EBOOT.PBP
```

- Lists each slot's offset, size, kind
- PNG of wrong size or with transparency → `WARNING:` line
- Game's RGBA `ICON0.PNG` gets one
  - why: some firmware and packers reportedly reject alpha
  - shows on PSP Slim (6.61)

## SND0

- Original 25.6 s loop: 150 BPM, E minor, 16 bars (drums, bass, chord stabs, lead)
- `tools/make-snd0-loop.py` synthesizes it from nothing; no samples or music from the original game ([D-037](decisions/D-037.md))
- Note and echo tails past the end wrap to the start: seamless loop
- Viewer's `Psp.toml` points at the game's file

```bash
python3 tools/make-snd0-loop.py /tmp/snd0.wav
pspbuild audio snd0 --exact /tmp/snd0.wav -o psp-game/xmb/SND0.AT3
pspbuild audio inspect --strict psp-game/xmb/SND0.AT3
```

- ffmpeg has no ATRAC3 encoder
- pspbuild native encoder:
  - low-passes below the top QMF band
  - writes LP4 (66144 bps, joint stereo) in RIFF, three QMF bands per frame
- XMB plays nothing if a frame codes a fourth band (ffmpeg still decodes it)
- `fact` and `smpl` chunks make the XMB repeat the loop
- `audio inspect --strict` checks all of this; good file ends with:

```
Loop:                samples 1024 to 1129983 (25.60 s), forever
Verdict:             playable; matches the profile pspbuild writes
```

- XMB limit for SND0: about 500 KB, 55 s
