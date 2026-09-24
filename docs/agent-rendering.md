# Rendering Investigation Protocol

```text
symptom → asset/scene/display list → decompilation → ROM data
→ display-list/GBI state → reference ports → hypothesis → smallest change
→ targeted test → original comparison → regression → evidence and docs
```

- Identify N64 behavior before changing anything. Never tune parameters or
  guess materials, palettes, formats, filtering, LOD, transforms, animation
  timing, lighting, combiner, alpha or depth.
- If the PSP cannot reproduce a behavior, record the original behavior, the
  limitation, the approximation, its measured effect and its regression
  coverage.
- BattleShip, `sf64-psp` and `oot-PSP` are references for GBI/RSP/RDP, TMEM,
  combiner translation, framebuffer paths and `sceGu` technique. Adopt a
  technique only after confirming SSB64 needs it.
