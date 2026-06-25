#!/bin/sh
# Empire 5 container entrypoint.
#
# On first start (no empire.db found) the world generator runs automatically
# using the NC / SC environment variables, then the server starts.
# On subsequent starts the existing world is preserved and only the server runs.
#
# Environment variables:
#   NC   Number of continents / player slots  (default: 8)
#   SC   Continent size in sectors            (default: 30)
#   NI   Number of islands                   (default: NC)
#   IS   Average island size                  (default: SC/2)
#   SP   Spike percent (0=round, 100=snake)   (default: 10)
#   PM   Mountain percent                     (default: 0)
#   DI   Min distance between continents      (default: 2)
#   ID   Min distance islands → continents    (default: 1)

set -e

DATA=/srv/empire/data
DB=$DATA/empire.db
CFG=/srv/empire/empire.toml

# Ensure subdirectories exist inside the volume
mkdir -p "$DATA/info" "$DATA/tele"

if [ ! -f "$DB" ]; then
    NC=${NC:-8}
    SC=${SC:-30}

    # Build the positional arg list — only include optional args if set
    ARGS="$NC $SC"
    [ -n "${NI:-}" ] && ARGS="$ARGS $NI"
    [ -n "${IS:-}" ] && ARGS="$ARGS $IS"
    [ -n "${SP:-}" ] && ARGS="$ARGS $SP"
    [ -n "${PM:-}" ] && ARGS="$ARGS $PM"
    [ -n "${DI:-}" ] && ARGS="$ARGS $DI"
    [ -n "${ID:-}" ] && ARGS="$ARGS $ID"

    echo "==> Empire 5: first run — generating world (NC=$NC SC=$SC)..."
    # shellcheck disable=SC2086
    empire-world \
        --config "$CFG" \
        --script "$DATA/newcap_script" \
        $ARGS
    echo ""
    echo "==> World ready.  newcap_script is at $DATA/newcap_script"
    echo "==> Connect as deity (POGO, blank password) and run newcap_script"
    echo "==> to assign player nation slots and capitals."
    echo ""
fi

echo "==> Starting empire-server..."
exec empire-server --config "$CFG" "$@"
