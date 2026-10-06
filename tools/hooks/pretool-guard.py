#!/usr/bin/env python3
"""Claude Code PreToolUse hook: reject tool calls that waste context.

Reads the hook JSON on stdin. Exit 2 blocks the call and returns stderr to
the agent as the reason; exit 0 allows it. See AGENTS.md "Context budget".
"""
import json
import os
import re
import subprocess
import sys

WRITE_LIMIT = 4096  # tracked files above this size must be edited, not rewritten


def block(reason):
    print(reason, file=sys.stderr)
    sys.exit(2)


def check_bash(cmd):
    if re.search(r"\b(until|while)\b[^\n]*;\s*do\b[^\n]*\bsleep\b", cmd):
        block("Polling loop rejected. Start the job with run_in_background, "
              "or wait for it with Monitor.")
    for secs in re.findall(r"\bsleep\s+(\d+)", cmd):
        if int(secs) >= 30:
            block(f"`sleep {secs}` rejected. Use run_in_background or Monitor.")
    if re.search(r"\bcat\s+[^|;&]*refs/ssb-decomp-re/[^|;&\s]*\.[ch]\b", cmd):
        block("`cat` of a decomp source rejected. Use "
              "`tools/decomp.py fn|type|sym|refs NAME`, or `sed -n` with a "
              "range of at most ~150 lines.")


def check_write(path):
    if not path or not os.path.isfile(path) or os.path.getsize(path) <= WRITE_LIMIT:
        return
    tracked = subprocess.run(
        ["git", "ls-files", "--error-unmatch", path],
        cwd=os.path.dirname(path), capture_output=True,
    ).returncode == 0
    if tracked:
        block(f"Write over tracked file {path} ({os.path.getsize(path)} bytes) "
              "rejected. Use Edit for the changed parts.")


def main():
    try:
        event = json.load(sys.stdin)
    except ValueError:
        return
    tool = event.get("tool_name")
    args = event.get("tool_input") or {}
    if tool == "Bash":
        check_bash(args.get("command", ""))
    elif tool == "Write":
        check_write(args.get("file_path", ""))


if __name__ == "__main__":
    main()
