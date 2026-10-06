#!/usr/bin/env python3
"""Compact lookups in refs/ssb-decomp-re, so one call replaces grep + sed.

  tools/decomp.py fn NAME [NAME...]     print each function definition
  tools/decomp.py type NAME [NAME...]   print a struct/union/enum/typedef body
  tools/decomp.py sym REGEX             #defines, enum members, globals, fields
  tools/decomp.py refs NAME             word references, one line each

Output is file:line prefixed and capped (--max, default 200 lines per item).
"""
import argparse
import os
import subprocess
import sys

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DECOMP = os.path.join(REPO, "refs", "ssb-decomp-re")
ROOTS = ["src", "include"]


def grep(pattern, extra=()):
    """Return (path, lineno, text) for an extended-regex match in C sources."""
    cmd = ["grep", "-rnE", "--include=*.c", "--include=*.h", *extra, pattern, *ROOTS]
    out = subprocess.run(cmd, cwd=DECOMP, capture_output=True, text=True).stdout
    hits = []
    for line in out.splitlines():
        path, lineno, text = line.split(":", 2)
        hits.append((path, int(lineno), text))
    return hits


def read_lines(path, cache={}):
    if path not in cache:
        with open(os.path.join(DECOMP, path), errors="replace") as f:
            cache[path] = f.read().splitlines()
    return cache[path]


def block(path, start, cap):
    """Print from line `start` (1-based) until the first brace group closes."""
    lines = read_lines(path)
    depth, opened, out = 0, False, []
    for i in range(start - 1, len(lines)):
        text = lines[i]
        out.append(f"{path}:{i + 1}: {text}")
        depth += text.count("{") - text.count("}")
        opened = opened or "{" in text
        if not opened and text.rstrip().endswith(";"):
            break  # declaration, not a body
        if opened and depth <= 0:
            break
    if len(out) > cap:
        out = out[:cap] + [f"... truncated at {cap} lines (use --max)"]
    print("\n".join(out))


def cmd_fn(names, cap):
    for name in names:
        hits = [
            h for h in grep(rf"^[A-Za-z_].*\b{name}\s*\(")
            if not h[2].rstrip().endswith(";") and not h[2].lstrip().startswith("#")
        ]
        if not hits:
            print(f"# {name}: no definition found")
        for path, lineno, _ in hits:
            block(path, lineno, cap)
        print()


def cmd_type(names, cap):
    for name in names:
        printed = set()
        # struct/union/enum NAME { ...
        for path, lineno, _ in grep(rf"^\s*(typedef\s+)?(struct|union|enum)\s+{name}\b[^;]*$"):
            block(path, lineno, cap)
            printed.add((path, lineno))
        # typedef struct ... { ... } NAME;  -> walk back to the typedef line
        for path, lineno, _ in grep(rf"^\s*\}}\s*{name}\s*;"):
            lines = read_lines(path)
            start, depth = lineno, 0
            for i in range(lineno - 1, -1, -1):
                depth += lines[i].count("}") - lines[i].count("{")
                if depth <= 0:
                    start = i + 1
                    break
            if (path, start) not in printed:
                block(path, start, cap)
                printed.add((path, start))
        found = bool(printed)
        if not found:
            for path, lineno, text in grep(rf"^\s*typedef\b.*\b{name}\s*;"):
                print(f"{path}:{lineno}: {text}")
                found = True
        if not found:
            print(f"# {name}: no type found")
        print()


def cmd_sym(regex, cap):
    pat = (
        rf"^\s*#\s*define\s+\w*({regex})"  # macros
        rf"|^\s*\w*({regex})\w*\s*(=[^=].*)?,\s*(/[/*].*)?$"  # enum members
        rf"|^\s*[A-Za-z_][A-Za-z0-9_ \t*]*\b\w*({regex})\w*(\[.*\])?\s*(=.*)?;"  # globals, fields
    )
    hits = grep(pat)
    for path, lineno, text in hits[:cap]:
        print(f"{path}:{lineno}: {text.strip()}")
    if len(hits) > cap:
        print(f"... {len(hits) - cap} more (narrow REGEX or use --max)")


def cmd_refs(name, cap):
    hits = grep(rf"\b{name}\b")
    for path, lineno, text in hits[:cap]:
        print(f"{path}:{lineno}: {text.strip()}")
    if len(hits) > cap:
        print(f"... {len(hits) - cap} more (use --max)")


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("kind", choices=["fn", "type", "sym", "refs"])
    ap.add_argument("names", nargs="+")
    ap.add_argument("--max", type=int, default=200)
    args = ap.parse_args()
    if not os.path.isdir(DECOMP):
        sys.exit(f"missing {DECOMP}")
    if args.kind == "fn":
        cmd_fn(args.names, args.max)
    elif args.kind == "type":
        cmd_type(args.names, args.max)
    elif args.kind == "sym":
        cmd_sym("|".join(args.names), args.max)
    else:
        for name in args.names:
            cmd_refs(name, args.max)


if __name__ == "__main__":
    main()
