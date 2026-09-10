# Rendering Investigation Protocol

Load for rendering work or discrepancies.

```text
symptom -> affected asset/scene/display list -> original decompilation
-> ROM data -> display-list/GBI state -> reference ports -> hypothesis
-> smallest change -> targeted test -> original comparison -> regression
-> evidence and status update
```

Do not tune parameters until N64 behavior is identified. Do not guess
materials, palettes, texture formats/filtering, LOD, mipmaps, transforms,
animation timing, lighting, combiner, alpha, or depth behavior. If PSP cannot
reproduce behavior directly, document original behavior, limitation,
approximation, measured effect, and regression coverage.

BattleShip, `sf64-psp`, and `oot-PSP` are useful for GBI/RSP/RDP, TMEM, display
lists, material state, combiner translation, framebuffer paths, `sceGu`,
texture handling, and debugging. Adopt techniques only after confirming SSB64
requires them.
