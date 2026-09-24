# shellcheck shell=bash
# Exact pixel comparison helpers, sourced by compare-screenshot.sh and
# golden.sh.
#
# ImageMagick 7's `compare -metric AE` reports accumulated channel error on
# this host, despite the metric's historical "absolute-error pixel count"
# name. RE-142 caught it reporting 9,835,820 for an image with only 836 changed
# pixels. These helpers build a binary per-pixel difference image instead; its
# mean times its area is the actual number of pixels whose RGB value differs.

# pixel_diff_count A B: prints the number of differing pixels. Returns 2
# (after a message on stderr) when the images differ in size.
pixel_diff_count() {
  local a="$1" b="$2" size_a size_b count
  size_a=$(magick identify -format '%wx%h' "$a") || return 2
  size_b=$(magick identify -format '%wx%h' "$b") || return 2
  if [ "$size_a" != "$size_b" ]; then
    echo "image sizes differ: $a=$size_a $b=$size_b" >&2
    return 2
  fi
  count=$(magick "$a" "$b" -compose difference -composite \
    -threshold 0 -format '%[fx:mean*w*h]' info:) || return 2
  # Round defensively for ImageMagick builds which print a floating-point
  # value.
  awk -v n="$count" 'BEGIN { printf "%.0f\n", n }'
}

# pixel_diff_mask A B OUT: writes a mask that is white where any channel
# differs and black elsewhere.
pixel_diff_mask() {
  magick "$1" "$2" -compose difference -composite -threshold 0 "$3"
}
