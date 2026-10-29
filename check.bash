#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"

RED='\033[0;31m'
GREEN='\033[0;32m'
BOLD='\033[1m'
RESET='\033[0m'

pass() { echo -e "${GREEN}✓${RESET} $1"; }
fail() { echo -e "${RED}✗${RESET} $1"; exit 1; }
step() { echo -e "\n${BOLD}▸ $1${RESET}"; }

# Feature combinations, each tested with `--no-default-features`. (The
# default `std,serde` combination is additionally tested without flags,
# including doctests, which require `alloc`.)
FEATURE_COMBOS=(
    ''
    'alloc'
    'serde'
    'alloc,serde'
    'std'
    'std,serde'
    'verus'
    'alloc,verus'
    'std,serde,verus'
)

step 'Prerequisites'
command -v cargo > /dev/null 2>&1 || fail 'Cannot find `cargo`'
command -v rustup > /dev/null 2>&1 || fail 'Cannot find `rustup` (needed for the MSRV and minimal-versions checks)'
rustup run nightly cargo --version > /dev/null 2>&1 \
    || fail 'Cannot find the nightly toolchain (needed for the minimal-versions check). To install, run `rustup toolchain install nightly --profile minimal`'
command -v cargo-msrv > /dev/null 2>&1 \
    || fail 'Cannot find `cargo-msrv`. To install, run `cargo install --locked cargo-msrv`'
pass 'All prerequisite executables found'

export RUST_BACKTRACE=1

step 'Formatting'
# Suppress diff output so AI agents run `cargo fmt` instead of manually applying each diff.
cargo fmt --all --check > /dev/null 2>&1 || fail "formatting issues found (run 'cargo fmt --all' to fix)"
pass 'All code is formatted'

step 'Clippy across the feature matrix (warnings denied)'
cargo clippy --all-targets -- -D warnings || fail 'clippy warnings found (default features)'
cargo clippy --all-targets --all-features -- -D warnings || fail 'clippy warnings found (all features)'
for combo in "${FEATURE_COMBOS[@]}"; do
    cargo clippy --all-targets --no-default-features --features "$combo" -- -D warnings \
        || fail "clippy warnings found (features: '$combo')"
done
pass 'No clippy warnings in any feature combination'

step 'Tests'
cargo test || fail 'tests failed (default features)'
for combo in "${FEATURE_COMBOS[@]}"; do
    cargo test --lib --no-default-features --features "$combo" \
        || fail "tests failed (features: '$combo')"
done
pass 'All tests passed in every feature combination'

step 'Docs (warnings denied)'
RUSTDOCFLAGS='-D warnings' cargo doc --no-deps --all-features || fail 'rustdoc warnings found'
pass 'Docs build cleanly'

step 'Verus verification test crate (verus-test/)'
# verus-test is its own workspace, so the steps above do not cover it.
(cd verus-test && cargo fmt --check > /dev/null 2>&1) \
    || fail "verus-test formatting issues found (run 'cargo fmt' in verus-test/ to fix)"
(cd verus-test && cargo clippy --all-targets -- -D warnings) || fail 'clippy warnings found (verus-test)'
# A `verus`-feature consumer must compile warning-free under plain cargo
# (the ghost blocks are cfg'd away; vstd compiles as an ordinary dependency).
(cd verus-test && cargo check --all-targets) || fail 'plain-cargo check of verus-test failed'
pass 'verus-test is formatted, clippy-clean, and compiles under plain cargo'

# Version pins must agree: a Verus release can only compile the vstd it
# shipped with. scripts/verus-verify.sh holds the Verus pin; vstd versions
# carry the same date (Verus `0.YYYY.MM.DD.<hash>` <-> vstd
# `0.0.0-YYYY-MM-DD-<time>`).
lock_vstd() { grep -A1 '^name = "vstd"$' "$1" | sed -n 's/^version = "\(.*\)"$/\1/p'; }
VERUS_VERSION=$(./scripts/verus-verify.sh --print-versions | sed -n 's/^VERUS_VERSION=//p')
VERUS_DATE=$(echo "$VERUS_VERSION" | sed -n 's/^0\.\([0-9]*\)\.\([0-9]*\)\.\([0-9]*\)\..*$/\1-\2-\3/p')
VSTD_PIN=$(sed -n 's/^vstd = "=\(.*\)"$/\1/p' verus-test/Cargo.toml)
[ -n "$VERUS_DATE" ] || fail "cannot parse VERUS_VERSION ('$VERUS_VERSION') in scripts/verus-verify.sh"
[ -n "$VSTD_PIN" ] || fail 'cannot find the `vstd = "=..."` pin in verus-test/Cargo.toml'
case "$VSTD_PIN" in
    "0.0.0-$VERUS_DATE-"*) ;;
    *) fail "vstd pin $VSTD_PIN (verus-test/Cargo.toml) does not match Verus $VERUS_VERSION (scripts/verus-verify.sh)" ;;
esac
for lock in Cargo.lock verus-test/Cargo.lock; do
    [ "$(lock_vstd "$lock")" = "$VSTD_PIN" ] \
        || fail "$lock locks vstd $(lock_vstd "$lock"), expected the pin $VSTD_PIN (verus-test/Cargo.toml)"
done
pass "Verus pins agree (Verus $VERUS_VERSION, vstd $VSTD_PIN)"

# Actual verification needs the Verus toolchain: run in Docker via the script
# (CI runs it as a separate job, setting SKIP_VERUS_VERIFY), or directly when
# a matching `cargo-verus` is on PATH; skipped (loudly) otherwise.
if [ -n "${SKIP_VERUS_VERIFY:-}" ]; then
    echo "skipped: Verus verification (SKIP_VERUS_VERIFY is set)"
elif command -v docker > /dev/null 2>&1 && docker info > /dev/null 2>&1; then
    ./scripts/verus-verify.sh || fail 'Verus verification failed'
    pass 'verus-test verifies under the Verus driver (Docker)'
elif command -v cargo-verus > /dev/null 2>&1; then
    (cd verus-test && cargo verus verify) || fail 'Verus verification failed'
    pass 'verus-test verifies under the Verus driver'
else
    echo -e "${RED}skipped:${RESET} neither Docker nor \`cargo-verus\` found; Verus verification not run (CI runs it)"
fi

step 'MSRV (rust-version in Cargo.toml)'
cargo msrv verify || fail 'MSRV verification failed'
pass 'Compiles with the declared MSRV'

step 'Minimal dependency versions'
cp Cargo.lock Cargo.lock.checkbash-backup
restore_lock() { mv Cargo.lock.checkbash-backup Cargo.lock; }
trap restore_lock EXIT
cargo +nightly update -Z direct-minimal-versions \
    || fail 'failed to resolve minimal versions of direct dependencies'
# Check the library only: dev-dependency minimums do not affect downstream
# users, and ancient dev-dependency releases may not build on modern rustc.
cargo check --all-features || fail 'failed to build with minimal dependency versions'
cargo check --no-default-features --features serde \
    || fail 'failed to build with minimal dependency versions (no_std serde)'
restore_lock
trap - EXIT
pass 'Builds with minimal versions of direct dependencies'

echo -e "\n${GREEN}${BOLD}All checks passed.${RESET}"
