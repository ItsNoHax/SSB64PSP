# Lighting

Part of [rendering.md](../rendering.md).

Status: complete; PPSSPP-validated.

- Fighters use runtime GE lighting from the active stage's directional light,
  set up as the original does before `ftDisplayMainProcDisplay`
  (RE-103, RE-105, RE-164–167).
- Lit versus literal vertex colour is decided per vertex from `G_VTX`
  load-time state, and primitive colour has a single owner (RE-240–243).
  The `G_LIGHTING` bit alone decides: light colour writes (`G_MW_LIGHTCOL`,
  an `MObj`'s `gSPLightColor`) leave it alone, and nothing is guessed from a
  colour's shape. A list that never states it runs under its object's seed:
  lit for fighters (`ftDisplayMainProcDisplay`) and for every other graph
  (`InitialMaterial::SCENE`: each scene's lights function sets it after the
  frame's reset list). Stage lists clear it themselves (RE-423). N64 RDRAM
  traces of all twelve fighters, their electric skeletons, specials,
  entries, items, eight VS stages, the opening room and the Training select
  match the packer on every one of 76,191 traced vertex loads (RE-424).
- Costume `LIGHT1COLOR`/`LIGHT2COLOR` tracks are preserved (RE-261;
  [D-024](../decisions/D-024.md)).
- `sceGuLight` does not enable its channel; `GU_LIGHT0` is enabled explicitly
  (RE-264).
- Stage geometry keeps the baked key-light shade, except a `LIT` primitive
  whose `LIGHT_1`/`LIGHT_2` is animated (Race to the Finish): it lights on
  the GE under the stage light (`DrawState::set_stage_light`, the direction
  `sc1PGameFuncLights` loads) with the live colours (RE-322).

Remaining: physical-PSP recheck of the corrected light channel; costume
selection in `psp-game`.
