# Lighting

Part of [docs/rendering.md](../rendering.md). Describes the current model; see `docs/evidence/re/` for how it was established.

### Lighting

Status: COMPLETE; PPSSPP revalidated after RE-264

RE-103/105 and RE-164–167 establish the runtime path; R2.2/C1–C2 (RE-240–243) prove single-source PRIM ownership and `G_VTX` load-time normal/colour provenance. RE-261 preserves fighter costume `LIGHT1COLOR`/`LIGHT2COLOR` tracks. RE-264 fixes the missing independent `GU_LIGHT0` enable that had left only ambient light active, and revalidates Mario, Fox and Link against refreshed deterministic captures

**Remaining work:** Physical-PSP confirmation of the corrected light-channel state remains in R2; dynamic gameplay costume selection remains future gameplay integration
