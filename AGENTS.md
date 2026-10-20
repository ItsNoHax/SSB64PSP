# Agent Instructions

- Project: native Rust source port of SSB64 (N64, US) to PSP; not an emulator.
- Method: translate `ssb-decomp-re` one whole subsystem at a time.
- Scope of this file: all working rules. Skills and `.claude/agents/` link here.

## Reference order

1. `ssb-decomp-re`: gameplay types, state tables, callbacks, constants, timing.
2. ROM and data: ground truth where the decomp is ambiguous.
3. BattleShip: SSB-specific native-port adaptations.
4. `sf64-psp`, `oot-PSP`, `n64psp`: PSP technique only (GU, memory, input,
   timing, audio, cache, asset loading).
5. SSB64PSP code, then engineering assumptions.

- Sources disagree → investigate; never guess around it.
- Never copy Nintendo assets or data from reference projects
  ([D-037](docs/decisions/D-037.md)).

## Crate ownership

| Crate | Owns | Must not contain |
|---|---|---|
| `crates/ssb-game` | Portable gameplay | PSP code |
| `crates/ssb-engine` | Engine systems: math, animation, collision, traits | PSP code |
| `crates/ssb-rom` | ROM, archive, formats, asset pack | PSP code |
| `crates/ssb-capture` | Golden-capture scene specs shared by both PSP binaries and host tests | PSP code, scene behaviour |
| `crates/ssb-pose` | Portable fighter tick, joint sampling, spawn placement: shared by `psp-runtime` and host tests ([D-052](docs/decisions/D-052.md)) | PSP code, gameplay rules |
| `crates/ssb-tablegen` | Build-time generation of the tables read from the ROM: their ROM layout and decoders | Data values from the ROM |
| `psp-runtime` | All shared PSP code: GE, input, timing, audio, memory, asset loading | Gameplay logic |
| `psp-game` | Thin game application: orchestration only | Gameplay or PSP backend code |
| `psp-asset-viewer` | Debug and render-validation tool | Game logic, match state, player features |

- `ssb-game`, `ssb-engine`, `ssb-rom`, `ssb-pose` → never depend on `psp-runtime`.

## Documentation

| State | Owner |
|---|---|
| Roadmap, milestones, the only task list | `PLAN.md` |
| Current work, next, blockers, verification baseline, current figures | `STATUS.md` |
| Permanent decisions | `DECISIONS.md` + `docs/decisions/D-NNN.md` |
| Architecture (current and target), original-game reference | `docs/architecture.md` |
| Renderer model | `docs/rendering.md` + `docs/rendering/*.md` |
| Memory layout and budgets | `docs/memory.md` |
| Golden captures | `docs/visual-regression/README.md` + `tests/golden/scenes.tsv` |
| XMB icons, background, sound | `docs/xmb-assets.md` |
| Durable technical findings | `docs/evidence/INDEX.md` + `docs/evidence/re/RE-NNN.md` |
| Working rules | `AGENTS.md` |
| Overview, legal, build | `README.md` |
| History | git |

Rules:

- One fact, one owner; elsewhere link to it.
- Volatile figures (pack version, golden count, FPS, ms): only `STATUS.md`
  (current) and RE records (measured). `validate_docs.py` checks pack version
  and golden count.
- Reference docs: current system only. No history, changelogs, measurement
  chains (git and RE records hold them).
- No prose: headings, tables, fragment bullets, numbered steps, code blocks.
  Readers: agents first, owner second.
- `PLAN.md` task: 1–2 lines (what remains, acceptance, RE/D IDs).
  - Closed → delete. No `[x]` under Remaining Work.
  - Completed: milestone-level capabilities only.
- `STATUS.md`: replaced, never appended; under 4 KiB.
- No other plan, status, TODO, handoff or report files in the repo.
  - Handoff notes → scratchpad. Generated reports → `assets/generated/`.
- Code and docs disagree → verify against source, fix the wrong one.

## Batch mode

- Scope: one coherent subsystem per batch (e.g. "shield system"); dozens of
  files is normal.
- Next work decided by `STATUS.md` → don't ask. No confirmation between
  functions.
- Stop only for: genuine ambiguity, missing evidence or access, destructive
  decisions, real blockers.
- Preserve original ordering, constants, state transitions, callback semantics,
  timing. Document every PSP-forced deviation.
- Two independent readings must agree ([D-034](docs/decisions/D-034.md)).
- Done = functionally validated, not compiled ([D-035](docs/decisions/D-035.md)).

Workflow:

1. Find the decomp modules.
2. Map types and dependencies onto the Rust architecture.
3. BattleShip or PSP ports: only for integration or platform questions.
4. Translate the whole subsystem. Mid-batch: compile checks and cheapest
   targeted tests only.
5. Batch end: workspace tests (with `SSB64_ROM`; with `SSB64_STUB_TABLES=1`
   and no ROM), clippy, both PSP builds, affected goldens, one PPSSPP smoke.
6. Update `STATUS.md`, `PLAN.md`, affected docs once; commit.

| Phase (fresh agent) | Steps |
|---|---|
| `batch-research` | 1–3 |
| `batch-implement` | 4 |
| `batch-validate` | 5–6 |

- Phases linked by a scratchpad handoff note; definitions in `.claude/agents/`.
- Physical-PSP validation, full golden matrices: milestone boundaries or
  PSP-specific investigations only.

## Evidence

- `RE-NNN` for durable findings only:
  - ambiguous original behaviour, RE discoveries;
  - PSP or PPSSPP behaviour, PSP deviations;
  - rendering discrepancies, measured performance.
- Not RE records: routine ports, progress, golden rebaselines → commit message.
- Hardware records: PSP model, firmware, build, pack version, observations.
- After adding or editing records: `python3 tools/docs/gen_evidence_index.py`,
  then `python3 tools/docs/validate_docs.py`.
- Record with no durable fact → delete; list in `docs/evidence/retired.tsv`
  (ID, title, where its fact lives, reason).
  - Code comments citing a retired ID stay; resolved through that table.
- Retired plan labels (`P0`–`P5`, `M0`–`M4`, `R0`–`R3`, `F1`, `G0`–`G5`) →
  cite the record instead.

## Constraints

- Rendering performance: `PLAN.md` milestone `MS8`, profiled on real
  workloads; never blocks gameplay ([D-032](docs/decisions/D-032.md),
  [D-036](docs/decisions/D-036.md)).
- No unsupported rendering heuristics. Unavoidable PSP deviations → measure,
  document.
- PPSSPP ≠ physical-PSP proof. Renderer-path change moves PPSSPP golden hashes
  only with hardware proof ([D-051](docs/decisions/D-051.md)).
- Asset-pipeline code changed → rebuild `assets/generated/ssb64.pak`.
- Never commit anything read or rendered from the ROM
  ([D-048](docs/decisions/D-048.md)):
  - no assets, packs, saves, generated tables, screenshots;
  - ROM tables: generated at build time by `crates/ssb-tablegen` into
    `OUT_DIR`; commit only layout (offsets, sizes, counts, names);
  - goldens: pixel hashes in `tests/golden/hashes.tsv`;
  - enforced by `tools/docs/validate_docs.py`.
- Test needs ROM-generated tables →
  `#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]`.
  CI: `SSB64_STUB_TABLES=1`, no ROM.
- Preserve user changes. Resolve exact targets before destructive commands.
- Never weaken acceptance criteria or delete failing tests to show progress.

## Context budget

- Start: `STATUS.md` → `PLAN.md` → `DECISIONS.md`.
- Evidence: `rg` over `docs/evidence/INDEX.md`; open only needed records.
  Never bulk-read `docs/evidence/re/`.
- Cost = context size × turns. Past ~250K tokens → scratchpad handoff note
  (done, next, open questions, `file:line` pointers), fresh agent.
- Decomp C: `tools/decomp.py fn|type|sym|refs NAME` before grep or sed.
- Rust: Serena, not `sed`/Read over whole files.
  - Load once: ToolSearch
    `select:mcp__serena__get_symbols_overview,mcp__serena__find_symbol,mcp__serena__find_referencing_symbols`
    (+ `replace_symbol_body` for edits). Skip `initial_instructions`.
  - `get_symbols_overview`: map a file. `find_symbol` + `include_body: true` +
    `relative_path`: read one item.
- Broad searches: read-only `Explore` subagent → `file:line`. No semantic or
  vector search.
- Reads: ≤ ~150 lines (`sed -n`, `rg -n -m 20`); never `cat` large files.
- Verbose output → file, then grep.
- Independent lookups → one call or parallel calls.
- No `sleep`/`until` polling → `run_in_background` or `Monitor`.
- Edit files with Edit. Write: new files or files under 4 KiB only
  (`tools/hooks/pretool-guard.py` blocks larger tracked files).
- Images (re-read every turn):
  - never downscale or recompress; fidelity needs native pixels;
  - exactness: pixel hash or per-pixel diff;
  - view only the region in question, native crop or nearest-neighbour upscale;
  - large comparison sheets → short-lived agent returning a verdict.
- Commands over 5 minutes expire the prompt cache.
  - Iterate: `cargo test -p <crate>`, `tools/golden.sh verify --filter REGEX`.
  - Full workspace tests, golden matrices: once, fresh validation agent.

## Skill routing

| Need | Skill |
|---|---|
| Resume work | `continue-plan` |
| Original-game behavior from decomp or ROM | `reverse-engineering` |
| Observe the running original game | `n64-emulator` |
| Renderer bug or fidelity question | `rendering` |
| Goldens and PPSSPPHeadless | `visual-regression` |
| Physical PSP, PSPLink | `psp-hardware` |
| Asset pipeline, pack format, `romtool` | `asset-pipeline` |
| Updating docs | `documentation` |

## Git

- Inspect status and diff before committing.
- One commit per batch: code, docs, pack bump, golden rebaseline together.
  No follow-up "docs: STATUS after …" commits.
- Subject: `type(scope): summary`, ≤ 72 characters.
  - Types: `feat`, `fix`, `perf`, `render`, `test`, `docs`, `tools`,
    `refactor`, `chore`.
- Body: RE/D IDs, measurements, rebaselined rows with native pixel counts
  ([D-051](docs/decisions/D-051.md)).
- Never rewrite others' history.
