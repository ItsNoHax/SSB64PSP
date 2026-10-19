#!/usr/bin/env python3
"""Tests for tools/local_arrays.py on synthetic C and synthetic `.data`
(no ROM, no decomp): `python3 tools/test_local_arrays.py`."""
import os
import struct
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import local_arrays as L  # noqa: E402

TYPES = """
typedef unsigned char u8;
typedef long s32;
typedef float f32;
typedef signed int intptr_t;
typedef struct Pair Pair;
typedef struct RGB { u8 r, g, b; } RGB;
struct Pair { RGB prim; RGB env; };
typedef struct Vec2f { f32 x, y; } Vec2f;
enum Kinds { nKindA, nKindB = 5, nKindC };
"""


def prog(body, file="src/t.c"):
    text = f'# 1 "include/t.h"\n{TYPES}\n# 1 "{file}"\n{body}'
    return L.Program(L.tokenize(text), file)


def f32(*vs):
    return b"".join(struct.pack(">f", v) for v in vs)


def s32(*vs):
    return b"".join(struct.pack(">i", v) for v in vs)


class Parse(unittest.TestCase):
    def test_types_and_forward_typedef(self):
        p = prog("")
        pair = p.typedefs["Pair"]
        pair = p.structs[pair] if isinstance(pair, str) else pair
        self.assertEqual((pair.size, pair.align), (6, 1))
        self.assertEqual(p.typedefs["Vec2f"].size, 8)
        self.assertEqual((p.enums["nKindB"], p.enums["nKindC"]), (5, 6))

    def test_locals_in_source_order_with_values(self):
        p = prog("""
s32 dTop[] = { 7, 8 };
void f(void)
{
    s32 i;
    f32 widths[] = { 1.0F, -2.5F };
    Pair colors[] = { { { 1, 2, 3 }, { 4, 5, 6 } } };
    s32 kinds[] = { nKindA, nKindC, 2 * 3 };
    intptr_t offsets[] = { &llFoo, 0x10 };
    char text[] = "ABC";
    char *names[] = { "MARIO", "FOX" };
    for (i = 0; i < 2; i++) { }
}
""")
        items = [(it[0], it[1].name if it[0] == "local" else it[1]) for it in p.items]
        self.assertEqual([n for _, n in items],
                         ["dTop", "widths", "colors", "kinds", "offsets", "names"])
        locs = {loc.name: loc for loc in p.locals()}
        self.assertTrue(locs["dTop"].is_global)
        self.assertEqual(bytes(locs["widths"].bytes), f32(1.0, -2.5))
        self.assertEqual(bytes(locs["colors"].bytes), bytes([1, 2, 3, 4, 5, 6]))
        self.assertEqual(bytes(locs["kinds"].bytes), s32(0, 6, 6))
        self.assertEqual(locs["offsets"].bytes[:4], [None] * 4)
        self.assertEqual(bytes(locs["offsets"].bytes[4:]), s32(0x10))
        self.assertEqual(locs["names"].strings, {0: b"MARIO", 4: b"FOX"})
        self.assertEqual(L.elem_of(locs["colors"]), ("U8", [1, 2, 3]))
        self.assertEqual(L.elem_of(locs["offsets"]), ("X32", [2]))
        self.assertEqual(L.elem_of(locs["names"]), ("CStr", [2]))

    def test_brace_elision_and_zero_fill(self):
        p = prog("""
void f(void)
{
    f32 grid[][2] = { 1.0F, 2.0F, 3.0F };
    Vec2f pos[3] = { { 1.0F, 2.0F } };
}
""")
        grid, pos = p.locals()
        self.assertEqual(grid.ty.size, 16)
        self.assertEqual(bytes(grid.bytes), f32(1.0, 2.0, 3.0, 0.0))
        self.assertEqual(bytes(pos.bytes), f32(1.0, 2.0, 0, 0, 0, 0))
        self.assertEqual(L.elem_of(pos), ("F32", [3, 2]))

    def test_function_pointer_array(self):
        p = prog("""
void f(void)
{
    void (*procs[3])(void) = { g, h, k };
}
""")
        (procs,) = p.locals()
        self.assertEqual(procs.ty.size, 12)

    def test_names(self):
        self.assertEqual(L.const_name("mnVSResultsMakeString", "widths"),
                         "MN_VS_RESULTS_MAKE_STRING_WIDTHS")
        self.assertEqual(L.const_name("mnPlayers1PGameMakeString", "pos_xy_3p"),
                         "MN_PLAYERS_1P_GAME_MAKE_STRING_POS_XY_3P")


class Fit(unittest.TestCase):
    RUN = """
void f(void)
{
    s32 a[] = { 1, 2 };
    Pair c[] = { { { 9, 9, 9 }, { 8, 8, 8 } } };
    intptr_t o[] = { &llX };
    f32 w[] = { 0.5F };
}
"""

    def test_word_aligned_layout(self):
        run = prog(self.RUN).locals()
        offs, size = L.layout(run)
        self.assertEqual(offs, [0, 8, 16, 20])
        self.assertEqual(size, 24)

    def test_unique_fit_and_window(self):
        run = prog(self.RUN).locals()
        body = s32(1, 2) + bytes([9, 9, 9, 8, 8, 8, 0, 0]) + s32(0x1234) + f32(0.5)
        data = b"\0" * 12 + body + b"\0" * 8 + body
        with self.assertRaises(L.LocateError):
            L.fit(run, data)  # two copies
        base, _ = L.fit(run, data, window=(0, 12 + len(body)))
        self.assertEqual(base, 12)
        parts = L.split_fit(run, data, (12 + len(body), len(data)))
        self.assertEqual([(len(p), b, n) for p, b, _, n in parts],
                         [(4, 12 + len(body) + 8, "ok")])

    def test_split_reports_a_gap(self):
        run = prog(self.RUN).locals()
        data = s32(1, 2) + bytes([9, 9, 9, 8, 8, 8, 0, 0]) + b"\xEE" * 8 + \
            s32(0x1234) + f32(0.5)
        parts = L.split_fit(run, data, (0, len(data)))
        self.assertEqual(parts[0][1], 0)
        self.assertIn("ok (split run)", parts[0][3])
        self.assertIn("+0x8", parts[-1][3])

    def test_strings_are_checked(self):
        p = prog('void f(void) { char *n[] = { "AB" }; f32 w[] = { 2.0F }; }')
        loc = p.locals()[0]
        mem = {0x100: s32(0x200), 0x200: b"AB\0"}

        def d(vram, n):
            for base, blob in mem.items():
                if base <= vram < base + len(blob):
                    return blob[vram - base:vram - base + n]
            return b""

        self.assertIsNone(L.check_strings(loc, 0x100, d))
        mem[0x200] = b"AC\0"
        self.assertIsNotNone(L.check_strings(loc, 0x100, d))


if __name__ == "__main__":
    unittest.main()
