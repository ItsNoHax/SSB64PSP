#!/usr/bin/env python3
"""Locate the decomp's function-local arrays in the ROM (D-048).

IDO compiles an initialised aggregate declared inside a function into its
file's `.data`, as an anonymous template the function copies from; the
symbol table has no name for it (it hides inside a `d<Mod>Unused0x...`
symbol). This tool finds each one's run-time address so that
`crates/ssb-tablegen`'s `d_arrays` lists can read it with `DArray::local`.

How: the C file is preprocessed for the US matching build (`gcc -E`, the
decomp's include paths and defines), its typedefs, structs and enums are
parsed, and every initialised local aggregate is laid out in source order
with its type's size and alignment. Consecutive locals with no file-scope
initialised data between them form a *run*; the run's byte pattern (values
known from the source, `&ll*` labels and string pointers left as wildcards)
must match the file's `.data` in the ROM at exactly one place. Then each
`char *` element is checked to point at its string literal. A run that
matches nowhere or more than once is reported, never guessed.

  tools/local_arrays.py FILE.c [--fn REGEX] [--rust] [--all]

FILE.c is relative to refs/ssb-decomp-re/src (`mn/mnvsmode/mnvsresults.c`).
Output: one line per local (function, variable, source line, type, vram,
status); `--rust` prints `DArray::local` entries to paste into a list file.
The ROM is `SSB64_ROM`, else the one `.z64` in `rom/`. Nothing read from the
ROM is printed except addresses and pass/fail.
"""
import argparse
import glob
import os
import re
import struct
import subprocess
import sys
import tempfile

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DECOMP = os.path.join(REPO, "refs", "ssb-decomp-re")
SOURCE_RS = os.path.join(REPO, "crates", "ssb-tablegen", "src", "source.rs")

# The matching US build (Makefile `DEFINES`, without NON_MATCHING/AVOID_UB).
CPP = [
    "gcc", "-E", "-nostdinc", "-Iinclude", "-Isrc", "-Ibuild/src",
    "-DF3DEX_GBI_2", "-D_MIPS_SZLONG=32", "-DNDEBUG", "-DN_MICRO",
    "-D_FINALROM", "-DREGION_US", "-D_LANGUAGE_C", "-D__sgi", "-U__linux__", "-Ulinux",
    "-Uunix", "-U__unix__", "-U__x86_64__",
]


class LocateError(Exception):
    pass


# ---------------------------------------------------------------- types

class Scalar:
    """kind: 'u' unsigned, 's' signed, 'f' float, 'p' pointer."""

    def __init__(self, kind, size, name):
        self.kind, self.size, self.align, self.name = kind, size, size, name

    def __repr__(self):
        return self.name


class Array:
    def __init__(self, elem, n):
        self.elem, self.n = elem, n
        self.align = elem.align
        self.size = None if n is None else elem.size * n

    def __repr__(self):
        return f"{self.elem!r}[{self.n}]"


class Struct:
    def __init__(self, name, fields, union=False):
        self.name, self.fields = name, []
        off = size = 0
        align = 1
        for fname, ftype in fields:
            if ftype is None or ftype.size is None:
                raise LocateError(f"struct {name}: field {fname} has no size")
            align = max(align, ftype.align)
            if union:
                self.fields.append((fname, ftype, 0))
                size = max(size, ftype.size)
            else:
                off = -(-off // ftype.align) * ftype.align
                self.fields.append((fname, ftype, off))
                off += ftype.size
                size = off
        self.align = align
        self.size = -(-size // align) * align
        self.union = union

    def __repr__(self):
        return self.name


BASIC = {
    ("char",): Scalar("s", 1, "char"),
    ("signed", "char"): Scalar("s", 1, "s8"),
    ("unsigned", "char"): Scalar("u", 1, "u8"),
    ("short",): Scalar("s", 2, "s16"),
    ("signed", "short"): Scalar("s", 2, "s16"),
    ("unsigned", "short"): Scalar("u", 2, "u16"),
    ("int",): Scalar("s", 4, "s32"),
    ("signed", "int"): Scalar("s", 4, "s32"),
    ("signed",): Scalar("s", 4, "s32"),
    ("unsigned", "int"): Scalar("u", 4, "u32"),
    ("unsigned",): Scalar("u", 4, "u32"),
    ("long",): Scalar("s", 4, "s32"),
    ("signed", "long"): Scalar("s", 4, "s32"),
    ("unsigned", "long"): Scalar("u", 4, "u32"),
    ("long", "long"): Scalar("s", 8, "s64"),
    ("signed", "long", "long"): Scalar("s", 8, "s64"),
    ("unsigned", "long", "long"): Scalar("u", 8, "u64"),
    ("float",): Scalar("f", 4, "f32"),
    ("double",): Scalar("f", 8, "f64"),
}
QUALIFIERS = {"const", "volatile", "static", "extern", "register", "auto", "inline"}
TYPE_WORDS = {"char", "short", "int", "long", "float", "double", "signed", "unsigned", "void"}
POINTER = Scalar("p", 4, "ptr")


# ---------------------------------------------------------------- tokens

TOKEN_RE = re.compile(r"""
    (?P<ws>[ \t\r\f\v]+|\n)
  | (?P<num>(?:0[xX][0-9a-fA-F]+|(?:\d+\.\d*|\.\d+|\d+)(?:[eE][+-]?\d+)?)[uUlLfF]*)
  | (?P<id>[A-Za-z_]\w*)
  | (?P<str>"(?:\\.|[^"\\])*")
  | (?P<chr>'(?:\\.|[^'\\])*')
  | (?P<op>->|\+\+|--|<<=?|>>=?|[<>!=]=|&&|\|\||[-+*/%&|^]=|\.\.\.|[{}()\[\];,.<>=+\-*/%&|^!~?:\#])
""", re.X)


class Tok:
    __slots__ = ("kind", "text", "file", "line")

    def __init__(self, kind, text, file, line):
        self.kind, self.text, self.file, self.line = kind, text, file, line

    def __repr__(self):
        return self.text


def tokenize(text, file="<text>"):
    """Tokens of preprocessed C, each tagged with its original file and line
    (from `# N "file"` markers)."""
    out = []
    line = 1
    for raw in text.split("\n"):
        m = re.match(r'#\s*(?:line\s+)?(\d+)\s+"([^"]*)"', raw)
        if m:
            line, file = int(m.group(1)), m.group(2)
            continue
        if raw.lstrip().startswith("#"):
            line += 1
            continue
        pos = 0
        while pos < len(raw):
            m = TOKEN_RE.match(raw, pos)
            if not m:
                raise LocateError(f"{file}:{line}: cannot tokenize {raw[pos:pos + 20]!r}")
            pos = m.end()
            kind = m.lastgroup
            if kind == "ws":
                continue
            out.append(Tok(kind, m.group(), file, line))
        line += 1
    return out


def unescape(s):
    body = s[1:-1]
    return body.encode("latin-1").decode("unicode_escape").encode("latin-1")


# ---------------------------------------------------------------- parser

class Program:
    """Types, enums and declarations of one preprocessed translation unit."""

    def __init__(self, toks, main_file):
        self.toks = toks
        self.main = main_file
        self.typedefs = {}
        self.structs = {}
        self.enums = {}
        # Each item: ("local", Local) or ("global", name, line).
        self.items = []
        self._top()

    # -- helpers
    def match(self, i, toks=None):
        """Index of the bracket closing the one at `toks[i]`."""
        toks = self.toks if toks is None else toks
        pairs = {"(": ")", "[": "]", "{": "}"}
        o, c = toks[i].text, pairs[toks[i].text]
        depth = 0
        for j in range(i, len(toks)):
            t = toks[j].text
            if t == o:
                depth += 1
            elif t == c:
                depth -= 1
                if depth == 0:
                    return j
        raise LocateError("unbalanced brackets")

    def eval(self, toks):
        """Value of a constant expression, or None when it holds a label,
        an unknown name or a construct this evaluator does not know."""
        parts = []
        i = 0
        while i < len(toks):
            t = toks[i]
            if t.kind == "num":
                txt = t.text.rstrip("uUlLfF") if not t.text.lower().startswith("0x") \
                    else t.text.rstrip("uUlL")
                parts.append(txt)
            elif t.kind == "id":
                if t.text in self.enums:
                    parts.append(str(self.enums[t.text]))
                elif t.text == "sizeof":
                    return None
                else:
                    return None
            elif t.kind == "chr":
                parts.append(str(unescape(t.text)[0]))
            elif t.text == "(":
                # A cast `(type)` is dropped.
                j = self.match(i, toks)
                inner = toks[i + 1:j]
                if inner and self._is_type_start(inner, 0) and all(
                        x.kind == "id" or x.text == "*" for x in inner):
                    i = j + 1
                    continue
                parts.append("(")
            elif t.text in ("+", "-", "*", "/", "%", "<<", ">>", "|", "&", "^", "~", ")"):
                parts.append({"/": "/"}.get(t.text, t.text))
            else:
                return None
            i += 1
        expr = " ".join(parts)
        try:
            v = eval(expr, {"__builtins__": {}}, {})
        except Exception:
            return None
        return v

    def _is_type_start(self, toks, i):
        t = toks[i]
        return t.kind == "id" and (t.text in TYPE_WORDS or t.text in self.typedefs
                                   or t.text in ("struct", "union", "enum")
                                   or t.text in QUALIFIERS)

    # -- types
    def base_type(self, i):
        """Parse a declaration's base type at token `i`; return
        (type or None, next index, is_typedef, is_static)."""
        words = []
        ty = None
        is_typedef = is_static = False
        while i < len(self.toks):
            t = self.toks[i]
            if t.text == "typedef":
                is_typedef = True
            elif t.text in QUALIFIERS:
                is_static |= t.text == "static"
            elif t.text in ("struct", "union"):
                ty, i = self.struct_spec(i)
                continue
            elif t.text == "enum":
                i = self.enum_spec(i)
                ty = BASIC[("int",)]
                continue
            elif t.text in TYPE_WORDS:
                words.append(t.text)
            elif t.kind == "id" and not words and ty is None and t.text in self.typedefs:
                ty = self.typedefs[t.text]
                if isinstance(ty, str):
                    ty = self.structs.get(ty) or ty
            elif t.text == "__attribute__":
                i = self.match(i + 1) + 1
                continue
            else:
                break
            i += 1
        if words:
            if words == ["void"]:
                ty = Scalar("u", 0, "void")
            else:
                ty = BASIC.get(tuple(w for w in words if w != "int") or ("int",)) \
                    or BASIC.get(tuple(words))
        return ty, i, is_typedef, is_static

    def struct_spec(self, i):
        union = self.toks[i].text == "union"
        i += 1
        tag = None
        if self.toks[i].kind == "id":
            tag = self.toks[i].text
            i += 1
        if self.toks[i].text != "{":
            # A reference; the definition may come later (`typedef struct X X;`
            # before `struct X {...}`), so resolve it by name when used.
            key = ("union " if union else "struct ") + (tag or "?")
            return self.structs.get(key) or key, i
        end = self.match(i)
        fields = []
        j = i + 1
        ok = True
        while j < end:
            base, k, _, _ = self.base_type(j)
            while True:
                decl = self.declarator(k, base)
                if decl is None:
                    ok = False
                    k = self._skip_to(k, (";",), end)
                    break
                name, fty, k = decl
                if self.toks[k].text == ":":
                    ok = False  # bit-field
                    k = self._skip_to(k, (",", ";"), end)
                fields.append((name, fty))
                if self.toks[k].text == ",":
                    k += 1
                    continue
                break
            j = k + 1
        key = ("union " if union else "struct ") + (tag or f"@{i}")
        try:
            st = Struct(tag or key, fields, union) if ok else None
        except LocateError:
            st = None
        self.structs[key] = st
        return st, end + 1

    def _skip_to(self, k, stops, end):
        depth = 0
        while k < end:
            t = self.toks[k].text
            if t in "([{":
                depth += 1
            elif t in ")]}":
                depth -= 1
            elif depth == 0 and t in stops:
                return k
            k += 1
        return k

    def enum_spec(self, i):
        i += 1
        if self.toks[i].kind == "id":
            i += 1
        if self.toks[i].text != "{":
            return i
        end = self.match(i)
        j = i + 1
        value = 0
        while j < end:
            name = self.toks[j].text
            j += 1
            if self.toks[j].text == "=":
                k = self._skip_to(j + 1, (",",), end)
                v = self.eval(self.toks[j + 1:k])
                value = v if isinstance(v, int) else value
                j = k
            self.enums[name] = value
            value += 1
            if j < end and self.toks[j].text == ",":
                j += 1
        return end + 1

    def declarator(self, i, base, allow_ref=False):
        """`*name[dims]` or `(*name)(args)`; returns (name, type, next) or None."""
        if base is None:
            return None
        ptr = 0
        while self.toks[i].text in ("*",) or self.toks[i].text in QUALIFIERS:
            ptr += self.toks[i].text == "*"
            i += 1
        if self.toks[i].text == "(":
            # Function pointer (or pointer to array): a pointer.
            # `(*name[dims])(args)`: an array of pointers.
            close = self.match(i)
            inner = self.toks[i + 1:close]
            if not inner or inner[0].text != "*":
                return None
            k = 1
            while k < len(inner) and inner[k].text in QUALIFIERS:
                k += 1
            if k >= len(inner) or inner[k].kind != "id":
                return None
            name = inner[k].text
            dims = []
            k += 1
            while k < len(inner) and inner[k].text == "[":
                c = self.match(k, inner)
                dims.append(self.eval(inner[k + 1:c]) if c > k + 1 else None)
                k = c + 1
            j = close + 1
            while j < len(self.toks) and self.toks[j].text in ("(", "["):
                j = self.match(j) + 1
            ty = POINTER
            for n in reversed(dims):
                ty = Array(ty, n)
            return name, ty, j
        if self.toks[i].kind != "id":
            return None
        name = self.toks[i].text
        i += 1
        if self.toks[i].text == "(":
            return None  # a function declaration
        if isinstance(base, str) and not ptr:
            resolved = self.structs.get(base)
            if resolved is None:
                if allow_ref and self.toks[i].text != "[":
                    return name, base, i
                return None
            base = resolved
        ty = POINTER if ptr else base
        dims = []
        while self.toks[i].text == "[":
            close = self.match(i)
            inner = self.toks[i + 1:close]
            dims.append(self.eval(inner) if inner else None)
            i = close + 1
        for n in reversed(dims):
            ty = Array(ty, n)
        return name, ty, i

    # -- top level
    def _top(self):
        toks = self.toks
        i = 0
        n = len(toks)
        while i < n:
            start = i
            base, j, is_typedef, _ = self.base_type(i)
            if j == i:
                j = i + 1
            # Walk this declaration to `;` or a function body.
            k = j
            is_function = False
            while k < n:
                t = toks[k].text
                if t in ("(", "["):
                    k = self.match(k) + 1
                    continue
                if t == "{":
                    if toks[k - 1].text == ")":
                        self._function(start, j, k)
                        k = self.match(k) + 1
                        is_function = True
                        break
                    k = self.match(k) + 1
                    continue
                if t == ";":
                    k += 1
                    break
                k += 1
            if is_typedef:
                self._typedef(base, j, k)
            elif not is_function and toks[start].file == self.main and any(
                    toks[x].text == "=" for x in range(j, k)) and toks[start].text != "extern":
                self._global(base, start, j, k)
            i = k

    def _typedef(self, base, j, end):
        while j < end:
            decl = self.declarator(j, base, allow_ref=True)
            if decl is None:
                return
            name, ty, j = decl
            self.typedefs[name] = ty
            if self.toks[j].text != ",":
                return
            j += 1

    def _function(self, start, j, body):
        name = None
        for x in range(j, body):
            if self.toks[x].text == "(":
                name = self.toks[x - 1].text
                break
        end = self.match(body)
        i = body + 1
        stmt_start = True
        while i < end:
            t = self.toks[i]
            if stmt_start and self._is_type_start(self.toks, i) and not (
                    t.kind == "id" and t.text in self.typedefs and
                    self.toks[i + 1].text in ("=", "(", ")", ";", "[", ".", "->")):
                i = self._local(name, i, end)
                stmt_start = True
                continue
            stmt_start = t.text in ("{", "}", ";")
            i += 1

    def _global(self, base, start, j, end):
        """File-scope data: laid out like a local when its type and
        initializer can be, else a separator between runs."""
        line = self.toks[start].line
        while j < end:
            decl = self.declarator(j, base)
            k = self._skip_to(j, (",", ";"), end)
            if decl is None:
                name = next((t.text for t in self.toks[j:k] if t.kind == "id"), "?")
                self.items.append(("global", name, line))
                j = k + 1
                continue
            name, ty, j = decl
            if self.toks[j].text == "=":
                init = self.toks[j + 1:k]
                if init and init[0].text != "{" and init[0].kind != "str":
                    init = [Tok("op", "{", "", line)] + init + [Tok("op", "}", "", line)]
                loc = Local(self, None, name, ty, init, line, True)
                loc.is_global = True
                # Size 0: its initializer is a header the decomp generates
                # at build time, stubbed empty here.
                if loc.bytes is None or loc.ty.size == 0:
                    self.items.append(("global", name, line))
                else:
                    self.items.append(("local", loc))
            j = k + 1

    def _local(self, func, i, end):
        base, j, _, is_static = self.base_type(i)
        while True:
            decl = self.declarator(j, base)
            if decl is None:
                k = self._skip_to(j, (";",), end)
                if any(t.text == "=" for t in self.toks[j:k]) and any(
                        t.text == "{" for t in self.toks[j:k]):
                    # An initialised aggregate of a type this parser does
                    # not lay out: it still occupies `.data`.
                    name = next((t.text for t in self.toks[j:k] if t.kind == "id"), "?")
                    loc = Local(self, func, name, None, [], self.toks[i].line, is_static)
                    loc.error = f"cannot lay out the type of `{name}`"
                    self.items.append(("local", loc))
                return k + 1
            name, ty, j = decl
            if self.toks[j].text == "=":
                k = self._skip_to(j + 1, (",", ";"), end)
                init = self.toks[j + 1:k]
                aggregate = bool(init) and init[0].text == "{"
                # IDO keeps a `char x[] = "..."` template with the string
                # literals in `.rodata`, not in `.data`.
                if init and init[0].kind == "str":
                    aggregate = False
                if aggregate:
                    self.items.append(("local", Local(self, func, name, ty, init,
                                                      self.toks[i].line, is_static)))
                j = k
            if self.toks[j].text == ",":
                j += 1
                continue
            return self._skip_to(j, (";",), end) + 1

    def locals(self):
        return [it[1] for it in self.items if it[0] == "local"]


# ---------------------------------------------------------------- values

def group(prog, toks):
    """An initializer as a tree: a list of items, each a list (a brace
    group) or a token list (an expression)."""
    if not toks or toks[0].text != "{":
        return toks
    items, cur = [], []
    depth = 0
    # Split the outer group's contents on top-level commas.
    body = toks[1:-1]
    for t in body:
        if t.text in "([{":
            depth += 1
        elif t.text in ")]}":
            depth -= 1
        if depth == 0 and t.text == ",":
            items.append(cur)
            cur = []
        else:
            cur.append(t)
    if cur:
        items.append(cur)
    return [group(prog, it) if it and it[0].text == "{" else it for it in items]


def is_group(item):
    return isinstance(item, list) and (not item or isinstance(item[0], list) or
                                       not isinstance(item[0], Tok))


class Cursor:
    def __init__(self, items):
        self.items, self.i = items, 0

    def done(self):
        return self.i >= len(self.items)

    def peek(self):
        return self.items[self.i]

    def take(self):
        self.i += 1
        return self.items[self.i - 1]


class Local:
    """One initialised local aggregate: its type, its bytes (None where the
    source does not fix them) and the string each `char *` points at."""

    def __init__(self, prog, func, name, ty, init, line, is_static):
        self.func, self.name, self.line, self.is_static = func, name, line, is_static
        self.prog = prog
        self.strings = {}  # byte offset -> bytes of a `char *` element
        self.error = None
        self.is_global = False
        tree = group(prog, init)
        self.ty = ty
        if ty is None:
            self.bytes = None
            return
        try:
            if isinstance(ty, Array) and ty.n is None:
                ty = self.ty = Array(ty.elem, self._count(ty.elem, tree))
            self.bytes = [0] * ty.size
            if isinstance(tree, list) and tree and isinstance(tree[0], Tok) and \
                    tree[0].kind == "str":
                self._string(ty, 0, tree)
            else:
                self._members(ty, 0, Cursor(tree))
        except LocateError as e:
            self.error = str(e)
            self.bytes = None

    def _count(self, elem, tree):
        if tree and isinstance(tree[0], Tok) and tree[0].kind == "str":
            return len(b"".join(unescape(t.text) for t in tree)) + 1
        n = 0
        cur = Cursor(tree)
        scratch = [0] * (elem.size * (len(tree) + 1))
        save = self.bytes if hasattr(self, "bytes") else None
        self.bytes = scratch
        while not cur.done():
            if n * elem.size >= len(self.bytes):
                self.bytes.extend([0] * elem.size * 8)
            self._elem(elem, n * elem.size, cur)
            n += 1
        self.bytes = save
        self.strings = {}
        return n

    def _members(self, ty, off, cur):
        if isinstance(ty, Array):
            for k in range(ty.n):
                if cur.done():
                    return
                self._elem(ty.elem, off + k * ty.elem.size, cur)
        elif isinstance(ty, Struct):
            for _, fty, foff in ty.fields[:1] if ty.union else ty.fields:
                if cur.done():
                    return
                self._elem(fty, off + foff, cur)
        else:
            self._elem(ty, off, cur)

    def _elem(self, ty, off, cur):
        item = cur.peek()
        if isinstance(ty, Scalar):
            cur.take()
            if is_group(item):
                item = item[0]
            self._scalar(ty, off, item)
        elif isinstance(ty, Array) and ty.elem.size == 1 and not is_group(item) \
                and item and item[0].kind == "str":
            cur.take()
            self._string(ty, off, item)
        elif is_group(item):
            cur.take()
            self._members(ty, off, Cursor(item))
        else:
            self._members(ty, off, cur)

    def _string(self, ty, off, toks):
        s = b"".join(unescape(t.text) for t in toks)
        for k, b in enumerate(s[:ty.size]):
            self.bytes[off + k] = b

    def _scalar(self, ty, off, toks):
        if toks and toks[0].kind == "str":
            if ty is not POINTER:
                raise LocateError("string for a non-pointer")
            self.strings[off] = b"".join(unescape(t.text) for t in toks)
            v = None
        elif toks and toks[0].text == "&":
            v = None
        else:
            v = self.prog.eval(toks)
        if v is None:
            for k in range(ty.size):
                self.bytes[off + k] = None
            return
        if ty.kind == "f":
            raw = struct.pack(">f" if ty.size == 4 else ">d", float(v))
        else:
            raw = (int(v) & ((1 << (8 * ty.size)) - 1)).to_bytes(ty.size, "big")
        for k, b in enumerate(raw):
            self.bytes[off + k] = b

    # -- shape for d_arrays
    def leaf(self):
        """(leaf scalar, dims) when every leaf has one type, else None."""
        def walk(t):
            if isinstance(t, Scalar):
                return t, []
            if isinstance(t, Array):
                r = walk(t.elem)
                return r and (r[0], [t.n] + r[1])
            if isinstance(t, Struct) and not t.union:
                rs = [walk(f) for _, f, _ in t.fields]
                if rs and all(r and r[0] is rs[0][0] and r[1] == rs[0][1] for r in rs):
                    return rs[0][0], [len(rs)] + rs[0][1]
            return None
        return walk(self.ty)


# ---------------------------------------------------------------- ROM

def overlay_of(cfile):
    """(segment name, rom start, vram, data start, data end) of `cfile`
    (path under src without `.c`) from the yaml."""
    yaml = open(os.path.join(DECOMP, "smashbrothers.us.yaml")).read()
    segs = re.split(r"\n  - name: ", yaml)
    for seg in segs[1:]:
        name = seg.split("\n", 1)[0].strip()
        start = re.search(r"\n    start: (0x[0-9A-Fa-f]+)", seg)
        vram = re.search(r"\n    vram: (0x[0-9A-Fa-f]+)", seg)
        subs = re.findall(r"\[(0x[0-9A-Fa-f]+),\s*([.\w]+)(?:,\s*([\w/]+))?", seg)
        for k, (addr, kind, path) in enumerate(subs):
            if kind == ".data" and path == cfile:
                if not (start and vram):
                    raise LocateError(f"{name}: no start/vram")
                end = int(subs[k + 1][0], 16) if k + 1 < len(subs) else None
                return (name, int(start.group(1), 16), int(vram.group(1), 16),
                        int(addr, 16), end)
    raise LocateError(f"no .data subsegment for {cfile} in the yaml")


def rust_overlay(rom, vram):
    text = open(SOURCE_RS).read()
    for m in re.finditer(r"pub const (\w+): Overlay = Overlay \{\s*vram: (0x[\w_]+),"
                         r"\s*rom: (0x[\w_]+),", text):
        if int(m.group(2).replace("_", ""), 16) == vram and \
                int(m.group(3).replace("_", ""), 16) == rom:
            return m.group(1)
    return None


def load_rom():
    path = os.environ.get("SSB64_ROM")
    if not path:
        roms = glob.glob(os.path.join(REPO, "rom", "*.z64"))
        if len(roms) != 1:
            raise LocateError("set SSB64_ROM (no single .z64 in rom/)")
        path = roms[0]
    return open(path, "rb").read()


def symbols_in(lo, hi):
    out = []
    for line in open(os.path.join(DECOMP, "symbols", "symbols_us.txt")):
        m = re.match(r"(d\w+) = (0x[0-9a-fA-F]+);", line)
        if m and lo <= int(m.group(2), 16) < hi:
            out.append((int(m.group(2), 16), m.group(1)))
    return sorted(out)


# ---------------------------------------------------------------- fit

def runs(prog):
    """Locals grouped into runs: no file-scope initialised data between.
    Each run comes with the names of the file-scope data before and after
    it (None at the ends of the file)."""
    out, cur, before = [], [], None
    for it in prog.items:
        if it[0] == "global":
            if cur:
                out.append((cur, before, it[1]))
            cur, before = [], it[1]
        else:
            cur.append(it[1])
    if cur:
        out.append((cur, before, None))
    return out


def layout(run, min_align=4):
    """Offsets of a run's locals from its start (source order). IDO starts
    each template on a word boundary, whatever its element type."""
    offs, off = [], 0
    for loc in run:
        a = max(loc.ty.align, min_align)
        off = -(-off // a) * a
        offs.append(off)
        off += loc.ty.size
    return offs, off


def pattern(run, offs, size):
    pat = [None] * size
    for loc, o in zip(run, offs):
        if loc.bytes is None:
            continue
        pat[o:o + len(loc.bytes)] = loc.bytes
    return pat


def find(data, pat, step=4):
    """Offsets in `data` where `pat` matches (None is a wildcard)."""
    known = [(k, b) for k, b in enumerate(pat) if b is not None]
    if not known:
        return None
    k0, b0 = known[0]
    hits = []
    start = 0
    while True:
        p = data.find(bytes([b0]), start)
        if p < 0:
            break
        base = p - k0
        start = p + 1
        if base < 0 or base % step or base + len(pat) > len(data):
            continue
        if all(data[base + k] == b for k, b in known):
            hits.append(base)
    return hits


def fit(run, data, window=None, min_align=4):
    """Base offset in `data` of a run, or raise with a diagnosis. `window`
    (lo, hi) bounds where the run may lie."""
    offs, size = layout(run, min_align)
    pat = pattern(run, offs, size)
    hits = find(data, pat)
    if hits is None:
        raise LocateError("the run has no known byte")
    if window:
        hits = [h for h in hits if window[0] <= h and h + size <= window[1]]
    if len(hits) == 1:
        return hits[0], offs
    raise LocateError(f"the run matches {len(hits)} places")


def check_strings(loc, vram, d):
    """Each `char *` points at its literal, NUL-terminated."""
    for off, s in loc.strings.items():
        ptr = int.from_bytes(d(vram + off, 4), "big")
        got = d(ptr, len(s) + 1)
        if got != s + b"\0":
            return f"`{loc.name}` element at +0x{off:X} does not point at {s!r}"
    return None


# ---------------------------------------------------------------- names

def snake(name):
    s = re.sub(r"([a-z])([A-Z])", r"\1_\2", name)
    s = re.sub(r"([A-Z]+)([A-Z][a-z])", r"\1_\2", s)
    s = re.sub(r"([a-z])([0-9])", r"\1_\2", s)
    return s.upper()


def const_name(func, var):
    """`mnVSResultsMakeString` + `widths` -> `MN_VS_RESULTS_MAKE_STRING_WIDTHS`."""
    return f"{snake(func)}_{snake(var)}".replace("__", "_")


ELEM = {"u8": "U8", "u16": "U16", "s32": "I32", "u32": "U32", "f32": "F32"}


def elem_of(loc):
    leaf = loc.leaf()
    if not leaf:
        return None, None
    sc, dims = leaf
    if sc is POINTER:
        return ("CStr" if loc.strings else "X32"), dims
    if loc.bytes is not None and sc.size == 4 and sc.kind != "f" and any(
            loc.bytes[k] is None for k in range(len(loc.bytes))):
        return "X32", dims  # `&ll*` labels: absolute offsets in a file
    return ELEM.get(sc.name), dims


# ---------------------------------------------------------------- main

def preprocess(cfile):
    """`gcc -E` output. A header the decomp generates at build time (absent
    here) is replaced by an empty file: it holds data, not types."""
    with tempfile.TemporaryDirectory() as stubs:
        for _ in range(16):
            out = subprocess.run(CPP + [f"-I{stubs}", f"src/{cfile}.c"], cwd=DECOMP,
                                 capture_output=True, text=True, errors="replace")
            m = re.search(r"fatal error: ([^:]+): No such file", out.stderr)
            if not out.returncode or not m:
                break
            path = os.path.join(stubs, m.group(1))
            os.makedirs(os.path.dirname(path), exist_ok=True)
            open(path, "w").close()
    if out.returncode:
        raise LocateError(out.stderr[:2000])
    return out.stdout


def locate(cfile, rom=None):
    """[(Local, vram or None, status)] for one decomp file."""
    prog = Program(tokenize(preprocess(cfile)), f"src/{cfile}.c")
    seg, seg_rom, seg_vram, data_rom, data_end = overlay_of(cfile)
    rom = rom if rom is not None else load_rom()
    data = rom[data_rom:data_end]
    data_vram = seg_vram + data_rom - seg_rom

    def d(vram, n):
        at = seg_rom + vram - seg_vram
        return rom[at:at + n]

    syms = dict((name, addr) for addr, name in symbols_in(data_vram, data_vram + len(data)))
    out = []
    end = 0
    for run, before, after in runs(prog):
        lo = max(end, syms.get(before, data_vram) - data_vram)
        hi = syms.get(after, data_vram + len(data)) - data_vram
        for part, base, offs, note in split_fit(run, data, (lo, hi)):
            if base is not None:
                end = base + offs[-1] + part[-1].ty.size
            if base is None:
                out.extend((loc, None, f"unplaced: {note}") for loc in part)
                continue
            for loc, o in zip(part, offs):
                vram = data_vram + base + o
                status = check_strings(loc, vram, d) or note
                if loc.is_global and loc.name in syms and syms[loc.name] != vram + 0:
                    status = f"symbol `{loc.name}` is at 0x{syms[loc.name]:08X}"
                out.append((loc, vram, status))
    return prog, (seg, seg_rom, seg_vram, data_vram, data_vram + len(data)), out


def run_size(run):
    offs, size = layout(run)
    return size


def split_fit(run, data, window):
    """[(locals, base or None, offsets, note)] for one run.

    The run is fitted whole. Failing that, its longest uniquely matching
    prefixes are fitted one after another, each break reported with the
    gap from the previous part; a part with no unique match of its own is
    placed when its neighbours leave exactly its room. A local whose type
    cannot be laid out splits the run (its size is unknown)."""
    lo, hi = window
    for k, loc in enumerate(run):
        if loc.bytes is None:
            left = split_fit(run[:k], data, window) if k else []
            ends = [b + o[-1] + p[-1].ty.size for p, b, o, _ in left if b is not None]
            right = split_fit(run[k + 1:], data, (max(ends, default=lo), hi)) \
                if k + 1 < len(run) else []
            return left + [([loc], None, None, f"cannot evaluate: {loc.error}")] + right
    try:
        base, offs = fit(run, data, window)
        return [(run, base, offs, "ok")]
    except LocateError:
        pass
    parts = []
    i = 0
    while i < len(run):
        placed_end = next((b + o[-1] + p[-1].ty.size for p, b, o, _ in reversed(parts)
                           if b is not None), lo)
        best = None
        for k in range(len(run), i, -1):
            try:
                base, offs = fit(run[i:k], data, (placed_end, hi))
            except LocateError:
                continue
            best = (k, base, offs)
            break
        if best is None:
            if parts and parts[-1][1] is None:
                parts[-1] = (parts[-1][0] + [run[i]], None, None, parts[-1][3])
            else:
                parts.append(([run[i]], None, None, "no unique match"))
            i += 1
            continue
        k, base, offs = best
        if parts and parts[-1][1] is None:
            # Sandwiched between placed parts: placed if it fills the gap.
            gap_run = parts[-1][0]
            start = -(-placed_end // 4) * 4
            g_offs, g_size = layout(gap_run)
            if len(parts) > 1 and -(-(start + g_size) // 4) * 4 == base:
                parts[-1] = (gap_run, start, g_offs, "placed by its neighbours")
        prev = next(((b + o[-1] + p[-1].ty.size) for p, b, o, _ in reversed(parts)
                     if b is not None), None)
        gap = "" if prev is None else f", {base - prev:+#x} after the previous part"
        note = f"break before `{run[i].name}`{gap}" if parts else f"ok (split run){gap}"
        parts.append((run[i:k], base, offs, note))
        i = k
    return parts


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("file", help="decomp file under src, e.g. mn/mnvsmode/mnvsresults.c")
    ap.add_argument("--fn", help="only functions matching this regex")
    ap.add_argument("--var", help="only variables matching this regex")
    ap.add_argument("--rust", action="store_true", help="print DArray::local entries")
    ap.add_argument("--globals", action="store_true",
                    help="also list the file-scope data laid out with the locals")
    a = ap.parse_args()
    cfile = a.file[:-2] if a.file.endswith(".c") else a.file
    try:
        prog, (seg, seg_rom, seg_vram, lo, hi), rows = locate(cfile)
    except LocateError as e:
        sys.exit(f"local_arrays: {e}")
    ovl = rust_overlay(seg_rom, seg_vram) or f"/* {seg} */"
    for loc, vram, status in rows:
        if loc.is_global and not a.globals:
            if status != "ok" and not status.startswith(("ok ", "break", "placed")):
                print(f"{cfile}.c:{loc.line}: file-scope `{loc.name}`: {status}",
                      file=sys.stderr)
            continue
        if a.fn and not re.search(a.fn, loc.func or ""):
            continue
        if a.var and not re.search(a.var, loc.name):
            continue
        elem, dims = elem_of(loc)
        if not a.rust:
            v = f"0x{vram:08X}" if vram is not None else "-"
            print(f"{loc.func}\t{loc.name}\t{cfile}.c:{loc.line}\t{loc.ty!r}\t"
                  f"{elem}{dims}\t{v}\t{status}")
            continue
        if vram is None or status != "ok" or elem is None:
            print(f"    // {loc.func} `{loc.name}`: {status}"
                  f"{'' if elem else ' (no single element type)'}")
            continue
        if not dims:
            print(f"    // {loc.func} `{loc.name}`: a scalar, not an array")
            continue
        dims_rs = ", ".join(str(x) for x in dims)
        print(f"    // {cfile}.c:{loc.line}.\n"
              f"    DArray::local(\n"
              f"        \"{const_name(loc.func, loc.name)}\",\n"
              f"        \"{loc.func} {loc.name}\",\n"
              f"        {ovl},\n"
              f"        0x{vram >> 16:04X}_{vram & 0xFFFF:04X},\n"
              f"        Elem::{elem},\n"
              f"        &[{dims_rs}],\n"
              f"    ),")


if __name__ == "__main__":
    main()
