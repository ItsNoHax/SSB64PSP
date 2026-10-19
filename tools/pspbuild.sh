#!/usr/bin/env bash
# Run pspbuild (github.com/ItsNoHax/pspbuild), the PSP EBOOT and XMB-media
# toolkit, at the revision pinned below (docs/xmb-assets.md).
#
# The first run installs that revision into target/pspbuild/REV; later runs
# reuse it. Arguments go to pspbuild unchanged, e.g.
#
#   tools/pspbuild.sh audio snd0 loop.wav -o psp-game/xmb/SND0.AT3
#   tools/pspbuild.sh audio inspect --strict psp-game/xmb/SND0.AT3

set -euo pipefail
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
REV=4fe3c5fd597636be7f42a79ced576daae3982a51
ROOT="$REPO/target/pspbuild/$REV"

if [ ! -x "$ROOT/bin/pspbuild" ]; then
    echo ">> installing pspbuild $REV" >&2
    cargo install -q --locked --git https://github.com/ItsNoHax/pspbuild --rev "$REV" \
        --root "$ROOT" pspbuild >&2
fi
exec "$ROOT/bin/pspbuild" "$@"
