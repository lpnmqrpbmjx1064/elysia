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

# Quiet when it passes, complete when it fails. The old form piped both commands
# through `tail`, so a failing build printed five truncated lines and the evidence
# behind the verdict existed nowhere (L04). A gate may be terse on success; it must
# never withhold the reason on failure.
run_step() {
  local label="$1"
  shift
  local log
  log=$(mktemp "${TMPDIR:-/tmp}/elysia-gate.XXXXXX")
  if "$@" >"$log" 2>&1; then
    printf '  pass  %s\n' "$label"
    grep -E 'test result:' "$log" | sed 's/^ *//; s/^/        /' || true
    rm -f "$log"
    return 0
  fi
  printf '  FAIL  %s\n' "$label"
  printf '  ---- full output ----\n'
  sed 's/^/  | /' "$log"
  printf '  ---- end of output ----\n'
  rm -f "$log"
  return 1
}

echo "== build =="
run_step "cargo build --workspace --locked" cargo build --workspace --locked

echo "== determinism replay =="
run_step "cargo test --workspace --locked" cargo test --workspace --locked

echo
echo "GATE PASSED (builddet): build + determinism replay only"
