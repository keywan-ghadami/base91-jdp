#!/bin/sh
# Materialises bench/corpus/ from the central benchmark repository.
#
#     bench/fetch.sh              # core and short, what CI measures
#     bench/fetch.sh all          # adds silesia: 202 MiB
#     bench/fetch.sh core,short,synthetic
#
# The corpus used to live here, in bench/corpus.py and bench/wire_samples.py.
# It moved to binary2textbench, which measures this codec against Base64,
# classic basE91, Ascii85, Base85N and Base94Max on the same bytes -- and two
# copies of a corpus generator that are supposed to be identical are a bug
# waiting to happen. The examples in rust/ read bench/corpus/ exactly as before;
# this script is what fills it.
#
# Override the checkout location with B2TB_DIR, or point it at a checkout you
# already have to avoid the clone.
set -eu

groups=${1:-core,short}
[ "$groups" = "all" ] && groups=core,short,synthetic,silesia

here=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
repo=${B2TB_DIR:-$here/.b2tb}

if [ ! -d "$repo/.git" ] && [ ! -d "$repo/corpus" ]; then
    echo "cloning binary2textbench into $repo" >&2
    git clone --depth 1 https://github.com/keywan-ghadami/binary2textbench "$repo"
elif [ -d "$repo/.git" ]; then
    git -C "$repo" pull --ff-only --quiet || \
        echo "note: could not update $repo, using what is there" >&2
fi

python3 "$repo/corpus/manifest.py" --groups="$groups"

# The examples take a directory of files; give them one where they expect it.
mkdir -p "$here/corpus"
for entry in "$repo"/corpus/data/*; do
    name=$(basename "$entry")
    # The download cache and the manifest are bookkeeping, not samples; linking
    # them in would put them in the measurement.
    case "$name" in _archives|manifest.json) continue ;; esac
    ln -sfn "$entry" "$here/corpus/$name"
done

echo "bench/corpus/ ready ($groups)" >&2
