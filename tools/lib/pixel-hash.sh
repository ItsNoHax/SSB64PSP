# shellcheck shell=bash
# Golden pixel hashes, sourced by golden.sh and golden-reference.sh.
#
# A golden is recorded as the SHA-256 of its decoded pixels, not of its PNG
# file: the bytes are 8-bit RGB, row-major from the top-left corner, with no
# header. PNG encoders differ in compression and metadata; the pixels do not.
# This is the same equality pixel_diff_count tests (any RGB channel differs),
# so a hash match is exactly a 0-pixel difference.
#
# tests/golden/hashes.tsv holds one row per golden: name, WIDTHxHEIGHT and
# the hash. The PNGs themselves are not committed (they show the game's
# characters and stages); tools/golden.sh keeps local copies in the
# gitignored tests/golden/local/ for difference masks.

# pixel_hash FILE: prints "WIDTHxHEIGHT SHA256".
pixel_hash() {
  local size sum
  size=$(magick identify -format '%wx%h' "$1") || return 2
  sum=$(magick "$1" -alpha off -depth 8 rgb:- | sha256sum) || return 2
  printf '%s %s\n' "$size" "${sum%% *}"
}

# golden_hash NAME MANIFEST: prints the manifest's "WIDTHxHEIGHT SHA256" for
# NAME, or nothing.
golden_hash() {
  awk -F'\t' -v g="$1" '$1 == g { print $2 " " $3; exit }' "$2"
}
