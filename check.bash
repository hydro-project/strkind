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
