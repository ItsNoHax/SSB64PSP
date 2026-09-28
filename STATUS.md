# Status

Current snapshot. History lives in git and `docs/evidence/`.

## Current

- **Milestone:** gameplay source port (`P1`–`P4` combined; `P0` closed).
  All 12 fighters have host moveset ports. Shared machinery and PSP game
  integration remain before `P2` is complete.
- **Completed batch:** shield tilt pose and clip start frame (RE-367).
  Clips restart on every status entry at its `frame_begin`. The stick
  tilts `Guard`/`GuardSetOff` through each fighter's eight `ShieldPose`
  sector tables, and the shield sphere sits on the posed `YRotN`. Pack v49.
- **Next gameplay batch:** Luigi's `translate_scales` (TODO, RE-367):
  `ftParamUpdateAnimKeys` scales his joint translations in clips and the
  shield pose. Stage selection and a scripted PSP shield scene are in
  `TODO.md`.
- **Parallel track:** rendering fidelity (`P5`). Not a gameplay gate.

## Last completed

| Batch | Result | Evidence |
|---|---|---|
| Shield tilt pose and clip start frame | 208 sector tables for 26 fighters, each checked against its `dobj_lookup` length; goldens unchanged; boot frame identical to v48 control | RE-367 |
| Remaining shared status clips | 66 slots, 2,407 ROM lengths agree with the decomp; goldens unchanged; boot frame identical to v47 control | RE-366 |

## Verification baseline

- Workspace tests with absolute `SSB64_ROM`, one test thread:
  **1,211 pass**. Pinned 1.98.0 Clippy with warnings denied, rustfmt and
  the `thumbv7em-none-eabi` `no_std` builds pass.
- Both PSP release builds pass. Existing viewer warnings remain.
- PPSSPPHeadless: **73 of 73 goldens match twice**, unchanged by RE-367.
  No golden raises a shield, passes through a platform or respawns.
- `psp-game` boots to the Intro at 60 FPS in PPSSPP. On the PSP-2000 a
  live scripted Training run held 16,682 µs per frame for 3,600 frames
  (RE-360).
- Pack v49: 27,085,312 bytes, SHA-256
  `fc9d79ef083b9b909706c5158b05c8c1f1ee5100a57101736a60ed72cd44ada0`.
- Physical PSP last checked in RE-361 (PSP-2000 Slim, 6.61 ARK, pack v43).

## Blockers and remaining scope

- Stage selection remains (TODO); Training loads Dream Land only.
- The acid packs three 384×384 tile-1 sprites it never draws (523 KB).
- Sector Z Arwing, bonus stages and stage items (Bumper, POW Block,
  Piranha Plant) remain; Twister/TaruCann have no clip.
- Training `grab` whiffs in PPSSPP while the host route catches (RE-351).
- Luigi's clips and shield pose ignore `translate_scales`. The shield is
  host-tested only, and its bubble is not drawn (RE-367).
- Samus through Ness remain host-only in `psp-game`; item/weapon/effect
  drawing, other item kinds and team checks remain.
- Four-player VS exceeds the ~700 KiB texture pool. The stage draw costs
  4.5 ms per frame on hardware (RE-360). Rendering performance remains `P5`.
