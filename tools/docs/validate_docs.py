#!/usr/bin/env python3
"""Deterministic documentation consistency checks for the progressive-
disclosure doc layout (docs/evidence, docs/decisions, plans/, STATUS.md).

Checks:
  1. Duplicate RE record IDs (docs/evidence/re/RE-*.md).
  2. RE-XXX IDs referenced anywhere in tracked docs/code but missing a record.
  3. Duplicate D record IDs (docs/decisions/D-*.md).
  4. D-XXX IDs referenced anywhere in tracked docs/code but missing a record.
  5. Broken local Markdown links ([text](relative/path)) under docs/, plans/,
     and the top-level *.md files.
  6. PLAN.md task-table links pointing at a missing plans/**/*.md file.
  7. STATUS.md referencing an RE-XXX/D-XXX ID or task-spec path that doesn't
     exist.

Exits 1 and prints one line per problem if anything is wrong; exits 0 and
prints a summary otherwise. Intended to be run after any documentation
change (see the `documentation` Skill).

Usage: python3 tools/docs/validate_docs.py
"""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

RE_ID_RE = re.compile(r"\bRE-(\d{3})\b")
D_ID_RE = re.compile(r"\bD-(\d{3})\b")
MD_LINK_RE = re.compile(r"\[[^\]]*\]\(([^)]+)\)")

SKIP_DIRS = {".git", "target", "refs", "rom", "assets", ".serena", "node_modules"}


def iter_text_files(exts):
    for p in ROOT.rglob("*"):
        if not p.is_file():
            continue
        if any(part in SKIP_DIRS for part in p.parts):
            continue
        if p.suffix in exts:
            yield p


def main():
    problems = []

    # --- 1/2: RE IDs ---
    re_dir = ROOT / "docs" / "evidence" / "re"
    re_files = sorted(re_dir.glob("RE-*.md"))
    re_ids_defined = {}
    for f in re_files:
        m = re.match(r"RE-(\d{3})\.md$", f.name)
        if not m:
            problems.append(f"evidence: unexpected filename {f.relative_to(ROOT)}")
            continue
        num = m.group(1)
        if num in re_ids_defined:
            problems.append(f"duplicate RE ID: RE-{num} ({re_ids_defined[num]} and {f})")
        re_ids_defined[num] = f

    re_ids_referenced = set()
    for f in iter_text_files({".md", ".rs"}):
        if f.parent == re_dir:
            continue  # a record referencing itself/siblings is not a "missing" signal
        text = f.read_text(encoding="utf-8", errors="ignore")
        for m in RE_ID_RE.finditer(text):
            re_ids_referenced.add(m.group(1))

    for num in sorted(re_ids_referenced - set(re_ids_defined)):
        problems.append(f"RE-{num} referenced but docs/evidence/re/RE-{num}.md does not exist")

    # --- 3/4: D IDs ---
    d_dir = ROOT / "docs" / "decisions"
    d_files = sorted(d_dir.glob("D-*.md"))
    d_ids_defined = {}
    for f in d_files:
        m = re.match(r"D-(\d{3})\.md$", f.name)
        if not m:
            problems.append(f"decisions: unexpected filename {f.relative_to(ROOT)}")
            continue
        num = m.group(1)
        if num in d_ids_defined:
            problems.append(f"duplicate D ID: D-{num} ({d_ids_defined[num]} and {f})")
        d_ids_defined[num] = f

    d_ids_referenced = set()
    for f in iter_text_files({".md", ".rs"}):
        if f.parent == d_dir:
            continue
        text = f.read_text(encoding="utf-8", errors="ignore")
        for m in D_ID_RE.finditer(text):
            d_ids_referenced.add(m.group(1))

    for num in sorted(d_ids_referenced - set(d_ids_defined)):
        problems.append(f"D-{num} referenced but docs/decisions/D-{num}.md does not exist")

    # --- 5: broken local markdown links ---
    link_check_roots = [ROOT / "docs", ROOT / "plans"] + [
        ROOT / n for n in ("AGENTS.md", "STATUS.md", "PLAN.md", "TODO.md", "DECISIONS.md", "README.md")
    ]
    md_files = set()
    for r in link_check_roots:
        if r.is_dir():
            md_files.update(r.rglob("*.md"))
        elif r.is_file():
            md_files.add(r)

    for f in sorted(md_files):
        text = f.read_text(encoding="utf-8", errors="ignore")
        for m in MD_LINK_RE.finditer(text):
            target = m.group(1)
            if target.startswith(("http://", "https://", "#", "mailto:")):
                continue
            target_path = target.split("#", 1)[0]
            if not target_path:
                continue
            resolved = (f.parent / target_path).resolve()
            if not resolved.exists():
                problems.append(f"broken link in {f.relative_to(ROOT)}: {target}")

    # --- 6: PLAN.md task links ---
    plan_text = (ROOT / "PLAN.md").read_text(encoding="utf-8")
    for m in re.finditer(r"\((plans/[^)]+\.md)\)", plan_text):
        rel = m.group(1)
        if not (ROOT / rel).exists():
            problems.append(f"PLAN.md links to missing task file: {rel}")

    # --- 7: STATUS.md references ---
    status_text = (ROOT / "STATUS.md").read_text(encoding="utf-8")
    for m in RE_ID_RE.finditer(status_text):
        if m.group(1) not in re_ids_defined:
            problems.append(f"STATUS.md references RE-{m.group(1)}, no such evidence record")
    for m in D_ID_RE.finditer(status_text):
        if m.group(1) not in d_ids_defined:
            problems.append(f"STATUS.md references D-{m.group(1)}, no such decision record")
    for m in re.finditer(r"\((plans/[^)]+\.md)\)", status_text):
        rel = m.group(1)
        if not (ROOT / rel).exists():
            problems.append(f"STATUS.md links to missing task file: {rel}")

    if problems:
        print(f"{len(problems)} problem(s) found:\n")
        for p in problems:
            print(" -", p)
        return 1

    print(f"OK: {len(re_ids_defined)} RE records, {len(d_ids_defined)} D records, "
          f"{len(md_files)} markdown files checked, no broken links or missing IDs.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
