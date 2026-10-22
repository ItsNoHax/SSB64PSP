# Lighting

Part of [rendering.md](../rendering.md).

## Fighters and packed meshes

- Fighters lit on CPU from the active stage's directional light.
  - Set up as the original does before `ftDisplayMainProcDisplay` (RE-103, RE-105, RE-164–167).
- `ssb-engine::rsp_light` models SSB's F3DEX2 lighting exactly.
  - Normalised light direction.
  - Per-channel shade clamp before the PRIM multiply.
- `meshdraw` lights each vertex with it.
- GE lighting never enabled for packed meshes (RE-514; `sceGuLight`, RE-264, unused).

## Lit vs literal vertex colour

- Decided per vertex from `G_VTX` load-time state; primitive colour has a single owner (RE-240–243).
- `G_LIGHTING` bit alone decides.
  - Light colour writes (`G_MW_LIGHTCOL`, an `MObj`'s `gSPLightColor`) leave it alone.
  - Nothing guessed from a colour's shape.
- List that never states it runs under its object's seed:
  - Fighters: lit (`ftDisplayMainProcDisplay`).
  - Every other graph: `InitialMaterial::SCENE`; each scene's lights function sets it after the frame's reset list.
  - Stage lists: clear it themselves (RE-423).
- N64 RDRAM traces match the packer on every traced vertex load (RE-424).
  - Covered: fighters, specials, entries, items, VS stages, opening room, Training select.

## Battle camera texgen

- Battle camera = LookAtReflect: texgen basis = `camera_basis(eye, at, Y)` (RE-458).
- Movie camera (kinds 3 and 8) keeps the prior basis: open.
- Fighter ENV: fog grey, or `colanim.color2` RGBA (RE-457). NOFOG joint parts skip fog (RE-453).
- D-051: lighting/material state; reviewed rebaseline against N64 A/B.
- Open: NOFOG under demo fade, 1P intro Metal Mario darkness (RE-458).

## Animated lights

- Costume `LIGHT1COLOR`/`LIGHT2COLOR` tracks preserved (RE-261; [D-024](../decisions/D-024.md)).
- Stage geometry keeps the baked key-light shade.
- Exception: `LIT` primitive with animated `LIGHT_1`/`LIGHT_2` (Race to the Finish).
  - Lit on CPU under the stage light (`DrawState::set_stage_light`, direction `sc1PGameFuncLights` loads).
  - Uses live colours (RE-322).
