#!/usr/bin/env python3
"""Token-usage report over Claude Code session transcripts for this repo.

  tools/token-report.py [--last N] [SESSION_ID_PREFIX ...]

Reads ~/.claude/projects/<repo>/*.jsonl plus each session's subagent
transcripts. Reports cost shares, per-agent context size, prompt-cache
misses, which tool calls grow the context, and adoption of the
context-budget tools (tools/decomp.py, Serena). Cost weights are relative
to one uncached input token: cache read 0.1, cache write 1.25, output 5.
"""
import argparse
import glob
import json
import os
import re
from collections import Counter
from datetime import datetime

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
PROJECTS = os.path.expanduser("~/.claude/projects/" + REPO.replace("/", "-"))
WEIGHTS = {"cache_read": 0.1, "cache_write": 1.25, "input": 1.0, "output": 5.0}
MISS_TOKENS = 40_000  # a cache write this large means the context was re-cached


def classify(name, args):
    if name == "Read":
        path = args.get("file_path", "")
        return "Read:image" if re.search(r"\.(png|jpe?g)$", path) else "Read:text"
    if name != "Bash":
        return "Serena" if name.startswith("mcp__serena__") else name
    cmd = args.get("command", "")
    for key in ("decomp.py", "golden.sh", "run-ppsspp", "romtool", "cargo test",
                "cargo build", "cargo +", "git diff", "git log", "git show",
                "grep", "sed -n", "cat ", "python3"):
        if key in cmd:
            return "Bash:" + key.strip()
    return "Bash:other"


def transcripts(main):
    sid = os.path.basename(main)[:-6]
    subs = glob.glob(os.path.join(PROJECTS, sid, "subagents", "**", "*.jsonl"), recursive=True)
    return [("main", main)] + [("sub", s) for s in subs]


def scan(path, totals, growth, growth_n, agents, misses, adoption):
    seen, tools, pending = set(), {}, []
    prev_ctx = prev_time = last_tool = None
    ctxs, out = [], 0
    for line in open(path):
        try:
            d = json.loads(line)
        except ValueError:
            continue
        msg = d.get("message") or {}
        if d.get("type") == "assistant":
            for c in msg.get("content") or []:
                if isinstance(c, dict) and c.get("type") == "tool_use":
                    tools[c["id"]] = (c["name"], c.get("input") or {})
                    last_tool = classify(c["name"], c.get("input") or {})
                    adoption[last_tool] += 1
            if msg.get("id") in seen:
                continue
            seen.add(msg.get("id"))
            u = msg.get("usage") or {}
            write = u.get("cache_creation_input_tokens", 0) or 0
            read = u.get("cache_read_input_tokens", 0) or 0
            inp = u.get("input_tokens", 0) or 0
            totals["cache_read"] += read
            totals["cache_write"] += write
            totals["input"] += inp
            totals["output"] += u.get("output_tokens", 0) or 0
            totals["turns"] += 1
            out += u.get("output_tokens", 0) or 0
            ctx = read + write + inp
            ctxs.append(ctx)
            now = datetime.fromisoformat(d["timestamp"].replace("Z", "+00:00"))
            if write > MISS_TOKENS and prev_time is not None:
                gap = (now - prev_time).total_seconds()
                misses.append((gap, write, last_tool))
            if prev_ctx is not None and pending and ctx > prev_ctx:
                for kind in pending:
                    growth[kind] += (ctx - prev_ctx) / len(pending)
                    growth_n[kind] += 1
            prev_ctx, prev_time, pending = ctx, now, []
        elif d.get("type") == "user" and isinstance(msg.get("content"), list):
            for c in msg["content"]:
                if isinstance(c, dict) and c.get("type") == "tool_result":
                    name, args = tools.get(c.get("tool_use_id"), ("?", {}))
                    pending.append(classify(name, args))
    if ctxs:
        agents.append((sum(ctxs), os.path.basename(path)[:24], len(ctxs),
                       max(ctxs), sum(ctxs) // len(ctxs), out))


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--last", type=int, default=10, help="most recent N sessions (default 10)")
    ap.add_argument("sessions", nargs="*", help="session id prefixes; overrides --last")
    args = ap.parse_args()

    mains = sorted(glob.glob(os.path.join(PROJECTS, "*.jsonl")), key=os.path.getmtime)
    if args.sessions:
        mains = [m for m in mains if any(os.path.basename(m).startswith(s) for s in args.sessions)]
    else:
        mains = mains[-args.last:]

    totals, growth, growth_n, adoption = Counter(), Counter(), Counter(), Counter()
    agents, misses = [], []
    for main_path in mains:
        for _, path in transcripts(main_path):
            scan(path, totals, growth, growth_n, agents, misses, adoption)

    cost = {k: totals[k] * w for k, w in WEIGHTS.items()}
    total_cost = sum(cost.values()) or 1
    print(f"{len(mains)} sessions, {len(agents)} transcripts, {totals['turns']:,} turns")
    print("\nCost share (input-token equivalents)")
    for k in WEIGHTS:
        print(f"  {k:12s} {totals[k]:>15,} tokens  {cost[k]:>15,.0f}  {100 * cost[k] / total_cost:5.1f}%")

    agents.sort(reverse=True)
    print("\nLargest transcripts: turns, peak context, average context, output")
    for _, name, turns, peak, avg, out in agents[:15]:
        print(f"  {name:24s} {turns:>6} {peak:>10,} {avg:>10,} {out:>10,}")
    over = sum(1 for a in agents if a[3] > 250_000)
    print(f"  transcripts peaking above 250K: {over} of {len(agents)}")

    miss_tokens = sum(m[1] for m in misses)
    slow = [m for m in misses if m[0] > 300]
    print(f"\nCache misses (> {MISS_TOKENS:,} written): {len(misses)}, {miss_tokens:,} tokens; "
          f"{len(slow)} after a gap over 300 s")
    for kind, n in Counter(m[2] for m in slow).most_common(8):
        print(f"  after {kind}: {n}")

    total_growth = sum(growth.values()) or 1
    print("\nContext growth by preceding tool call")
    for kind, v in growth.most_common(15):
        print(f"  {kind:16s} {int(v):>12,} {100 * v / total_growth:5.1f}%  n={growth_n[kind]:<6} avg={int(v / growth_n[kind]):,}")

    print("\nContext-budget tool adoption (calls)")
    for kind in ("Bash:decomp.py", "Serena", "Bash:grep", "Bash:sed -n", "Bash:cat", "Agent"):
        print(f"  {kind:16s} {adoption[kind]:>6}")


if __name__ == "__main__":
    main()
