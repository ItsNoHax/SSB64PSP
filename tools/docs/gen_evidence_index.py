#!/usr/bin/env python3
"""Regenerate docs/evidence/INDEX.md from docs/evidence/re/RE-*.md metadata.

Deterministic: reads each record's heading + `Status:`/`Topics:`/
`Related tasks:` metadata lines (written by this repo's evidence-record
convention, see docs/evidence/re/RE-001.md for the shape) and rebuilds the
index table. Run after adding or editing a RE-*.md record so the index does
not silently drift from the corpus.

Usage: python3 tools/docs/gen_evidence_index.py [--check]

--check: exit 1 if regenerating would change docs/evidence/INDEX.md, without
writing (for CI / pre-commit use).
"""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
RE_DIR = ROOT / "docs" / "evidence" / "re"
INDEX_PATH = ROOT / "docs" / "evidence" / "INDEX.md"

HEADING_RE = re.compile(r"^# (RE-(\d+) — .*)$")


def short_title(full_title: str) -> str:
    full_title = full_title.replace(" *(OPEN)*", "")
    short = re.split(r"; ", full_title, maxsplit=1)[0]
    if len(short) > 110:
        short = short[:107].rstrip() + "..."
    return short


def parse_record(path: Path):
    lines = path.read_text(encoding="utf-8").splitlines()
    m = HEADING_RE.match(lines[0])
    if not m:
        raise ValueError(f"{path}: first line is not a '# RE-NNN — ...' heading")
    heading, num = m.group(1), m.group(2)
    status, topics, task = "COMPLETE", "", ""
    for line in lines[2:9]:
        if line.startswith("Status:"):
            status = line.split(":", 1)[1].strip()
        elif line.startswith("Topics:"):
            topics = line.split(":", 1)[1].strip()
        elif line.startswith("Related tasks:"):
            task = line.split(":", 1)[1].strip().split(",")[0].strip()
        elif line.strip() == "":
            break
    title = heading.split(" — ", 1)[1] if " — " in heading else heading
    return {
        "id": f"RE-{num}",
        "num": int(num),
        "title": short_title(title),
        "status": status,
        "topics": topics,
        "task": task,
    }


def build_index():
    records = [parse_record(p) for p in sorted(RE_DIR.glob("RE-*.md"))]
    records.sort(key=lambda r: r["num"])
    ids = [r["num"] for r in records]
    dupes = {n for n in ids if ids.count(n) > 1}
    if dupes:
        raise SystemExit(f"duplicate RE ids: {sorted(dupes)}")

    lines = [
        "# Reverse-Engineering Evidence Index",
        "",
        "One-line entry per investigation. Load `docs/evidence/re/RE-XXX.md` for the",
        "full record (question, evidence, implementation, verification, conclusion).",
        "Do not bulk-read this corpus; grep this index by ID or topic tag first.",
        "",
        "Status `OPEN` = unresolved question, still tracked in `TODO.md`.",
        "Status `COMPLETE` = investigation concluded (may still feed a task `IN_PROGRESS`).",
        "",
        "Regenerate with `python3 tools/docs/gen_evidence_index.py` after adding or",
        "editing a record.",
        "",
        "| ID | Title | Status | Topics | Task |",
        "|---|---|---|---|---|",
    ]
    for r in records:
        title = r["title"].replace("|", "\\|")
        lines.append(f"| {r['id']} | {title} | {r['status']} | {r['topics']} | {r['task']} |")
    return "\n".join(lines) + "\n"


def main():
    check = "--check" in sys.argv
    new_text = build_index()
    if check:
        old_text = INDEX_PATH.read_text(encoding="utf-8") if INDEX_PATH.exists() else ""
        if old_text != new_text:
            print("docs/evidence/INDEX.md is stale; run tools/docs/gen_evidence_index.py")
            return 1
        return 0
    INDEX_PATH.write_text(new_text, encoding="utf-8")
    print(f"wrote {INDEX_PATH} ({len(new_text)} bytes)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
