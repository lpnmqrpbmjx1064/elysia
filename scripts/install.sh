#!/usr/bin/env bash
# E2 `manual` release path.
#
# Build, install a versioned file, flip the symlink, restart the unit, then
# REQUIRE the live service to report the version that was just installed.
# "Deployed" without that check is just "a process is running".
#
#   ./scripts/install.sh                 build for this host and install
#   ./scripts/install.sh --target aarch64-unknown-linux-musl
#   ./scripts/install.sh --dry-run       print the plan, change nothing
#   ./scripts/install.sh --rollback      flip back to the previous version
#
# Rollback is the same flip reversed: the unit always starts ~/bin/elysia, so
# undoing a bad push never requires editing the unit or rebuilding anything.
#
# Exit: 0 released and verified · 1 build/install failed · 2 live service does
# not report the expected version (rollback instruction printed) · 3 nothing to
# roll back to.
set -euo pipefail
cd "$(dirname "$0")/.." || exit 1
export PATH="$HOME/.cargo/bin:$PATH"

BINDIR="$HOME/bin"
LINK="$BINDIR/elysia"
PREV="$BINDIR/.elysia-prev"
UNIT="$HOME/.config/systemd/user/elysia.service"
PKG=elysia-app
TARGET=""
MODE=release
while [ $# -gt 0 ]; do
  case "$1" in
    --target) TARGET="${2:?--target needs a value}"; shift 2 ;;
    --dry-run) MODE=dry; shift ;;
    --rollback) MODE=rollback; shift ;;
    *) echo "unknown argument: $1" >&2; exit 1 ;;
  esac
done

[ -f "$UNIT" ] || { echo "no user unit at $UNIT — install it first (A1)" >&2; exit 1; }
# The unit lives in the repo (deploy/elysia.service) and is copied into place,
# so a fresh clone can deploy without anyone remembering where the file went.
REPO_UNIT=deploy/elysia.service
if [ "$MODE" != dry ] && [ -f "$REPO_UNIT" ] && ! cmp -s "$REPO_UNIT" "$UNIT"; then
  install -m 0644 "$REPO_UNIT" "$UNIT"
  SAY "unit refreshed from $REPO_UNIT"
fi
# The unit file may be new or edited; restart without this would run the old one.
systemctl --user daemon-reload
ADDR=$(grep -m1 '^ExecStart=' "$UNIT" | sed -E 's/^ExecStart=\S+ ([^ ]+).*/\1/' || true)
PORT="${ADDR##*:}"
PROBE="http://127.0.0.1:${PORT}/health"

version_of() { grep -m1 '^version' Cargo.toml | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' || true; }
VER=$(version_of)
SAY() { printf '  %s\n' "$*"; }

# The version the live service reports, or empty if it will not answer.
# `|| true` on the curl: under `set -euo pipefail` a connection refused inside a
# pipeline fails the whole assignment and kills the script — which is how a
# healthy release once reported "did not take".
live_version() {
  { curl -s --max-time 2 "$PROBE" 2>/dev/null || true; } \
    | sed -nE 's/.*"version":"([^"]+)".*/\1/p'
}

# Poll until the live service reports $1, or fail. systemd restart is not
# synchronous from our point of view, so this has to wait rather than assume.
wait_for_version() {
  local want="$1" i got
  for i in $(seq 1 40); do
    got=$(live_version)
    [ "$got" = "$want" ] && { SAY "live service reports version $got"; return 0; }
    sleep 0.5
  done
  SAY "live service reports '${got:-no answer}', expected '$want'"
  return 1
}

flip_to() {
  local ver="$1"
  [ -x "$BINDIR/elysia-$ver" ] || { echo "missing artifact $BINDIR/elysia-$ver" >&2; exit 1; }
  ln -sfn "$BINDIR/elysia-$ver" "$LINK"
  SAY "symlink: $LINK -> elysia-$ver"
  systemctl --user restart elysia
}

if [ "$MODE" = rollback ]; then
  [ -s "$PREV" ] || { echo "no previous version recorded at $PREV" >&2; exit 3; }
  TARGET_VER=$(cat "$PREV")
  CUR_VER=$(live_version)
  SAY "rollback: ${CUR_VER:-unknown} -> $TARGET_VER"
  flip_to "$TARGET_VER"
  wait_for_version "$TARGET_VER" || { echo "ROLLBACK DID NOT TAKE" >&2; exit 2; }
  printf '%s\n' "$CUR_VER" >"$PREV"
  SAY "previous version is now '$CUR_VER'"
  exit 0
fi

# ---- build ------------------------------------------------------------------
if [ -n "$TARGET" ]; then
  LLD=$(ls "$HOME"/.rustup/toolchains/*/lib/rustlib/x86_64-unknown-linux-gnu/bin/rust-lld 2>/dev/null | head -1 || true)
  [ -n "$LLD" ] || { echo "rust-lld not found — needed for the musl cross build" >&2; exit 1; }
  BIN="target/$TARGET/release/$PKG"
  BUILD=(cargo build --release --locked -p "$PKG" --target "$TARGET"
         --config "target.$TARGET.linker=\"$LLD\"")
else
  BIN="target/release/$PKG"
  BUILD=(cargo build --release --locked -p "$PKG")
fi
SAY "version to release: $VER${TARGET:+ (target $TARGET)}"
SAY "build: ${BUILD[*]}"
SAY "probe: $PROBE"
[ "$MODE" = dry ] && { SAY "dry run — nothing changed"; exit 0; }

"${BUILD[@]}" || { echo "build failed" >&2; exit 1; }
[ -f "$BIN" ] || { echo "expected artifact missing: $BIN" >&2; exit 1; }
SAY "artifact: $BIN ($(stat -c%s "$BIN") B)"

# A cross-build cannot be installed on this host — it is for the board. Build
# it, report it, and stop before touching the running service.
if [ -n "$TARGET" ]; then
  mkdir -p "$BINDIR"
  install -m 0755 "$BIN" "$BINDIR/elysia-$VER-$TARGET"
  SAY "cross artifact staged: $BINDIR/elysia-$VER-$TARGET (not activated: wrong architecture)"
  exit 0
fi

# ---- install ----------------------------------------------------------------
mkdir -p "$BINDIR"
ARTIFACT="$BINDIR/elysia-$VER"
install -m 0755 "$BIN" "$ARTIFACT"
SAY "installed: $ARTIFACT ($(stat -c%s "$ARTIFACT") B)"

CUR=$(live_version)
if [ -n "$CUR" ] && [ "$CUR" != "$VER" ]; then
  printf '%s\n' "$CUR" >"$PREV"
  SAY "currently live: $CUR (recorded as the rollback target)"
elif [ -n "$CUR" ]; then
  SAY "currently live: $CUR (same version — no rollback target recorded)"
fi

# A hand-started instance holds the port and would make the unit fail to bind.
# The trailing `|| true` matters: `ss` does not always expose the owning pid (it
# omits users:(...) on this host), and under `pipefail` an empty match aborts the
# whole release before the flip — a false failure that looks like a bad build.
HOLDER=$( { ss -ltnp 2>/dev/null || true; } | grep ":${PORT}\b" \
          | grep -oE 'pid=[0-9]+' | cut -d= -f2 | head -1 || true)
if [ -n "$HOLDER" ] && ! systemctl --user is-active --quiet elysia; then
  EXE=$(readlink -f "/proc/$HOLDER/exe" 2>/dev/null || true)
  case "$EXE" in
    "$PWD"/*|"$BINDIR"/*)
      SAY "taking over port $PORT from hand-started pid $HOLDER ($EXE)"
      kill "$HOLDER" 2>/dev/null || true; sleep 1 ;;
    *) echo "port $PORT is held by pid $HOLDER ($EXE) which is not ours — refusing" >&2; exit 1 ;;
  esac
fi

flip_to "$VER"
systemctl --user enable elysia >/dev/null 2>&1 || true

# ---- verify -----------------------------------------------------------------
if ! wait_for_version "$VER"; then
  echo "RELEASE DID NOT TAKE. Propagate with:" >&2
  echo "  ./scripts/install.sh --rollback" >&2
  exit 2
fi
H=$(curl -s --max-time 2 "$PROBE" || true)
SAY "health: $H"
printf '%s' "$H" | grep -q '"ok":true' || { echo "health is not ok" >&2; exit 2; }
SAY "released $VER · rollback: ./scripts/install.sh --rollback"
