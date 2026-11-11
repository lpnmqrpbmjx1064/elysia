#!/usr/bin/env bash
# Elysia merge gate.
#
# D3 = `builddet`: build + the determinism replay test. Nothing more.
# Lint, the cross-architecture hash and the budget gate are deliberately absent —
# A10/B5 are declared gaps and this gate must not pretend to cover them.
#
# Exits non-zero on any failure. Local half of D2 (`both`): fast enough to run on
# every change, while CI remains the authority that cannot be skipped.
set -euo pipefail
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"

echo "== build =="
cargo build --workspace --locked 2>&1 | tail -5

echo "== determinism replay =="
cargo test --workspace --locked 2>&1 | tail -20

echo
echo "GATE PASSED (builddet): build + determinism replay only"
