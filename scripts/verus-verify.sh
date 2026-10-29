#!/usr/bin/env bash
# Containerized Verus verification of `verus-test/` (strkind's `verus`
# feature). Used by CI and by `check.bash` (when Docker is available).
#
# Why a container: the Verus release binaries and z3 build need glibc >= 2.39,
# and the driver must run on exactly the rustc the release is built against.
# `rust:$RUST_VERSION-trixie` provides both (glibc 2.41).
#
# Pins: this script is the single source of truth for the Verus, z3, and Rust
# versions. They must match the vstd pin in `verus-test/Cargo.toml` (and the
# vstd entries of both lockfiles): a Verus binary can only compile the vstd
# it shipped with. `check.bash` checks that the dates agree.
#   - Verus release: see VERUS_VERSION   (vstd =0.0.0-<same date>-<time>)
#   - Rust:          see RUST_VERSION    (the release's pinned toolchain)
#   - z3:            see Z3_VERSION      (this Verus release does not bundle
#                                         z3; version from its tools/get-z3.sh)
#
# The first run downloads Verus and z3 (~100 MB) into $VERUS_CACHE_DIR (default
# ~/.cache/strkind-verus) and fills Docker volumes with crate dependencies
# (network required); later runs are incremental.
#
# Usage (from anywhere in the repo):
#   ./scripts/verus-verify.sh                     # verify verus-test/
#   ./scripts/verus-verify.sh -- --no-lifetime    # extra args go to the driver
set -euo pipefail
cd "$(dirname "$0")/.."

VERUS_VERSION=0.2026.09.06.8dea4a2
RUST_VERSION=1.98.0
Z3_VERSION=4.16.0
IMAGE=rust:$RUST_VERSION-trixie
CACHE=${VERUS_CACHE_DIR:-$HOME/.cache/strkind-verus}

# `--print-versions`: machine-readable pins, for check.bash's sync check.
if [ "${1:-}" = '--print-versions' ]; then
    echo "VERUS_VERSION=$VERUS_VERSION"
    echo "RUST_VERSION=$RUST_VERSION"
    echo "Z3_VERSION=$Z3_VERSION"
    exit 0
fi

# Cached under a VERSIONED directory, so bumping a pin above invalidates the
# cache automatically (no stale binary served after a bump).
VERUS_DIR=$CACHE/verus-$VERUS_VERSION-z3-$Z3_VERSION/verus-x86-linux
if [ ! -x "$VERUS_DIR/cargo-verus" ] || [ ! -x "$VERUS_DIR/z3" ]; then
    DL=$(dirname "$VERUS_DIR")
    rm -rf "$DL"
    mkdir -p "$DL"
    echo "Downloading Verus $VERUS_VERSION and z3 $Z3_VERSION into $DL ..." >&2
    curl -sSfL -o "$DL/verus.zip" \
        "https://github.com/verus-lang/verus/releases/download/release%2F$VERUS_VERSION/verus-$VERUS_VERSION-x86-linux.zip"
    unzip -oq "$DL/verus.zip" -d "$DL"
    rm "$DL/verus.zip"
    curl -sSfL -o "$DL/z3.zip" \
        "https://github.com/Z3Prover/z3/releases/download/z3-$Z3_VERSION/z3-$Z3_VERSION-x64-glibc-2.39.zip"
    unzip -oq "$DL/z3.zip" -d "$DL"
    cp "$DL/z3-$Z3_VERSION-x64-glibc-2.39/bin/z3" "$VERUS_DIR/z3"
    chmod +x "$VERUS_DIR/z3"
    rm -rf "$DL/z3.zip" "$DL/z3-$Z3_VERSION-x64-glibc-2.39"
fi

# The repo is mounted read-only: verification never writes into the working
# copy. It is verified from a writable tmpfs COPY (/ws) of just what the build
# reads (strkind's manifest, sources, and README - which `src/lib.rs`
# includes as crate docs - plus `verus-test/`), since cargo may need to write
# lockfiles/metadata. CARGO_HOME and CARGO_TARGET_DIR live in named volumes so
# dependency fetches and incremental builds persist across runs without
# touching host caches.
[ "${1:-}" = '--' ] && shift
INNER='tar -C /work --exclude=./verus-test/target \
          -cf - ./Cargo.toml ./Cargo.lock ./README.md ./LICENSE ./src ./verus-test \
      | tar -C /ws -xf - \
      && cd /ws/verus-test \
      && exec cargo-verus verify "$@"'
exec docker run --rm \
    -v "$PWD":/work:ro \
    -v "$VERUS_DIR":/verus:ro \
    --tmpfs /ws:exec \
    -v strkind-verus-cargo-home:/cargo-home \
    -v strkind-verus-target:/verus-target \
    -e CARGO_HOME=/cargo-home \
    -e CARGO_TARGET_DIR=/verus-target \
    -e VERUS_Z3_PATH=/verus/z3 \
    -e PATH=/verus:/usr/local/cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin \
    -w /work "$IMAGE" \
    bash -c "$INNER" -- "$@"
