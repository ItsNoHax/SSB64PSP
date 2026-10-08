# Agent Instructions

SSB64PSP is a native Rust source port of Super Smash Bros. 64 to PSP, not an
emulator. Gameplay is translated from `ssb-decomp-re` in large coherent
subsystems, not rebuilt mechanic by mechanic.

## Reference order

1. `ssb-decomp-re` — gameplay behavior: types, state tables, callbacks,
   constants, timing.
2. Original ROM and data — ground truth when the decomp is ambiguous.
3. BattleShip — SSB-specific native-port adaptations.
4. `sf64-psp`, `oot-PSP`, `n64psp` — PSP platform technique only (GU,
   memory, input, timing, audio, cache, asset loading).
5. Existing SSB64PSP code, then engineering assumptions.

Investigate disagreements; do not guess around them. Never copy Nintendo
assets or copyrighted data from reference projects
([D-037](docs/decisions/D-037.md)).

## Crate ownership

| Crate | Owns | Must not contain |
|---|---|---|
| `crates/ssb-game` | Portable gameplay | PSP code |
| `crates/ssb-engine` | Engine systems: math, animation, collision, traits | PSP code |
| `crates/ssb-rom` | ROM, archive, formats, asset pack | PSP code |
| `crates/ssb-capture` | Golden-capture scene specs shared by both PSP binaries and host tests | PSP code, scene behaviour |
| `crates/ssb-tablegen` | Build-time generation of the tables read from the ROM: their ROM layout and decoders | Data values from the ROM |
| `psp-runtime` | All shared PSP code: GE, input, timing, audio, memory, asset loading | Gameplay logic |
| `psp-game` | Thin game application: orchestration only | Gameplay or PSP backend code |
| `psp-asset-viewer` | Debug and render-validation tool | Game logic, match state, player features |

`ssb-game`, `ssb-engine` and `ssb-rom` never depend on `psp-runtime`.

## Documentation map

One owner per kind of state. Update the owner; link to it elsewhere.

| State | Owner |
|---|---|
| Roadmap, subsystem state, completed and remaining work (the only task list) | `PLAN.md` |
| Current work, blockers, verification baseline | `STATUS.md` (replace, never append; under 4 KiB) |
| Permanent decisions | `DECISIONS.md` + `docs/decisions/D-NNN.md` |
| Architecture, current and target; original-game reference | `docs/architecture.md` |
| Renderer model | `docs/rendering.md` + `docs/rendering/*.md` |
| Memory layout and constraints | `docs/memory.md` |
| Golden captures | `docs/visual-regression/README.md` + `tests/golden/scenes.tsv` |
| Durable technical findings | `docs/evidence/INDEX.md` + `docs/evidence/re/RE-NNN.md` |
| Project overview and legal | `README.md` (no progress detail) |
| History | git |

Do not create other plan, status, TODO, handoff or report documents in the
repository; scratchpad handoff notes stay outside it. When code and docs
disagree, verify against source and fix the wrong record.

## Batch mode

- Work one coherent subsystem per batch (e.g. "shield system"). Translate a
  group of related decomp modules before stopping; dozens of files per batch
  is normal.
- Do not ask for confirmation between functions or ask what to work on when
  `STATUS.md` determines it. Stop only for genuine ambiguity, missing
  evidence or access, destructive decisions, or a real blocker.
- During a batch run only compile checks and the cheapest relevant tests.
- At the end of a batch: workspace tests, both PSP builds, one PPSSPP smoke
  test, then update docs once and commit the batch.
- Physical-PSP validation and full golden matrices belong at milestone
  boundaries or PSP-specific investigations.
- Preserve original ordering, constants, state transitions, callback
  semantics and timing. Document any deviation the PSP forces.

### Batch workflow

1. Find the relevant decomp modules.
2. Map types and dependencies onto the Rust architecture.
3. Check BattleShip or PSP ports only where integration or platform questions
   need them.
4. Translate the whole subsystem and fix compile errors.
5. Targeted tests → workspace tests → PSP builds → one integration smoke.
6. Update `STATUS.md`, `PLAN.md` (tick or remove closed items, add new
   ones) and affected docs once, then commit.

Large batches run as phases, one fresh agent each, linked by a scratchpad
handoff note: `batch-research` (steps 1–3, output: module map and port
plan), `batch-implement` (step 4 and targeted tests), `batch-validate`
(steps 5–6). Definitions: `.claude/agents/`.

## Evidence

Write an `RE-NNN` record only for a durable technical finding: ambiguous
original behavior, RE discoveries, PSP or PPSSPP behavior, PSP deviations,
rendering discrepancies, measured performance — not routine ports, progress
reports or golden rebaselines (those go in the commit message). Hardware
records include PSP model, firmware, build, pack version and observations.
Then run `python3 tools/docs/gen_evidence_index.py` and
`python3 tools/docs/validate_docs.py`.

Retired IDs: a record with no durable fact of its own is deleted and listed
in `docs/evidence/retired.tsv` (ID, title, where its fact lives now,
reason); the index shows it under "Retired records". Code comments that cite
a retired `RE-NNN` stay as they are and resolve through that table. Labels
of earlier plans (`P0`–`P5`, `M0`–`M4`, `R0`–`R3`, `F1`, `G0`–`G5`) are
retired too; cite the record, not the label, in new text.

## Constraints

- Rendering performance is `PLAN.md` milestone `MS8`, profiled on real
  workloads; it never blocks gameplay.
- No unsupported rendering heuristics. Measure and document unavoidable PSP
  deviations.
- PPSSPP is not physical-PSP proof.
- Rebuild `assets/generated/ssb64.pak` when asset-pipeline code changes.
- Never commit anything read or rendered from the ROM: no assets, packs,
  saves, generated tables or screenshots ([D-048](docs/decisions/D-048.md)).
  A table the game reads from the ROM is generated at build time by
  `crates/ssb-tablegen` into `OUT_DIR`; commit only its layout (offsets,
  sizes, counts, names). Goldens are pixel hashes in
  `tests/golden/hashes.tsv`; captures and local PNGs stay out of Git.
  `tools/docs/validate_docs.py` enforces this.
- A test that needs the ROM-generated tables gets
  `#[cfg_attr(ssb64_stub_tables, ignore = "needs the ROM-generated tables")]`;
  CI builds with `SSB64_STUB_TABLES=1` and has no ROM.
- Preserve user changes. Use `apply_patch` for edits. Avoid destructive
  commands; resolve exact targets first. Never weaken acceptance
  criteria or delete failing tests to show progress.

## Context budget

- Start from `STATUS.md` and the indexes (`PLAN.md`,
  `docs/evidence/INDEX.md`, `DECISIONS.md`). Open only the records the batch
  needs.
- Never bulk-read `docs/evidence/re/`.
- Every turn re-reads the whole context, so cost grows with context size
  times turns. Past ~250K tokens, write a handoff note (done, next, open
  questions, file:line pointers) to scratchpad and continue in a fresh agent.
- Decomp C: `tools/decomp.py fn|type|sym|refs NAME` prints one function,
  type, symbol list or reference list in one call. Use it before grep/sed.
- Rust: use Serena, not `sed`/Read over whole files. Load once with
  ToolSearch `select:mcp__serena__get_symbols_overview,mcp__serena__find_symbol,mcp__serena__find_referencing_symbols`
  (add `replace_symbol_body` for edits). Skip `initial_instructions`.
  `get_symbols_overview` maps a file; `find_symbol` with
  `include_body: true` and `relative_path` reads one item.
- Delegate broad searches to a read-only subagent (`Explore`) that returns
  `file:line` answers. Semantic/vector search is deliberately not used.
- Read at most ~150 lines per call (`sed -n` ranges, `rg -n -m 20`); never
  `cat` large files. Redirect verbose output to a file and grep it.
- Batch independent lookups into one Bash call or parallel tool calls.
- Never poll with `sleep`/`until` loops. Run long jobs with
  `run_in_background` or wait with `Monitor`.
- Edit existing files; never rewrite a whole file with Write.
- Commands over 5 minutes expire the prompt cache and re-bill the whole
  context. While iterating, run `cargo test -p <crate>` and
  `tools/golden.sh verify --filter REGEX` on affected scenes. Run full
  workspace tests and golden matrices once, in a fresh low-context
  validation agent.

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

Inspect status and diff before committing. Commit a completed batch as one
coherent unit. Do not rewrite others' history.
