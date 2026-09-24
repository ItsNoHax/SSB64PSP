# Lighting

Part of [rendering.md](../rendering.md).

Status: complete; PPSSPP-validated.

- Fighters use runtime GE lighting from the active stage's directional light,
  set up as the original does before `ftDisplayMainProcDisplay`
  (RE-103, RE-105, RE-164–167).
- Lit versus literal vertex colour is decided per vertex from `G_VTX`
  load-time state, and primitive colour has a single owner (RE-240–243).
- Costume `LIGHT1COLOR`/`LIGHT2COLOR` tracks are preserved (RE-261;
  [D-024](../decisions/D-024.md)).
- `sceGuLight` does not enable its channel; `GU_LIGHT0` is enabled explicitly
  (RE-264).

Remaining: physical-PSP recheck of the corrected light channel; costume
selection in `psp-game`.
