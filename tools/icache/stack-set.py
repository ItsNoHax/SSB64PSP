#!/usr/bin/env python3
"""Copies a `psp-game` PRX with `run`'s data-cache offset set (RE-583).

`psp-game`'s `STACK_SET` is the word after the marker `SPIN`; the game
enters `run` with its frame ending at that offset of the 8 KB data-cache
way (`ssb_psp_runtime::stack_pin`). A screen of offsets on the PSP runs
copies of one build that differ in that word only.

    tools/icache/stack-set.py IN.prx OUT.prx OFFSET
"""
import struct
import sys


def main():
    src, dst, offset = sys.argv[1], sys.argv[2], int(sys.argv[3], 0)
    if offset % 8 or not 0 <= offset < 0x2000:
        sys.exit('stack-set: offset %#x is not a multiple of 8 below 0x2000' % offset)
    data = bytearray(open(src, 'rb').read())
    hits = [i for i in range(0, len(data) - 8, 4) if data[i:i + 4] == b'SPIN']
    if len(hits) != 1:
        sys.exit('stack-set: %d markers in %s, not 1' % (len(hits), src))
    old, = struct.unpack_from('<I', data, hits[0] + 4)
    struct.pack_into('<I', data, hits[0] + 4, offset)
    with open(dst, 'wb') as f:
        f.write(data)
    print('%s: STACK_SET %#x -> %#x' % (dst, old, offset))


if __name__ == '__main__':
    main()
