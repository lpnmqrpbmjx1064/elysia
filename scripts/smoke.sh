#!/usr/bin/env bash
# Elysia workflow smoke test.
#
# Exercises every stage of the loop in the chart — ruling, plan, write, gate,
# verify, record, commit, release — against the live tree, and reports one
# PASS/FAIL per stage. It is deliberately NOT written with `set -e`: a smoke
# test that stops at the first failure tells you about one stage, and the point
# is to see the state of the whole chain in one run.
#
#   ./scripts/smoke.sh            stages 0-7, fast, read-only
#   ./scripts/smoke.sh --mutant   also prove the gate can actually fail
#
# Exit: 0 all stages pass, 1 any stage fails.
#
# Anything this script reports must be re-derivable: every check is a command,
# not a judgement.

set -uo pipefail
cd "$(dirname "$0")/.." || exit 2
REPO="$PWD"
export PATH="$HOME/.cargo/bin:$PATH"

RUN="$HOME/hermes-notes/runs/sim-engine-20261111"
REGISTER="$RUN/elysia-console/decisions.txt"
# Loopback by default so the repo carries no internal network address, and so the suite
# runs anywhere the service is reachable. Point these elsewhere when testing across the
# LAN: ELYSIA_CONSOLE=http://<host>:8124 ELYSIA_APP=http://<host>:8125 bash scripts/smoke.sh
CONSOLE="${ELYSIA_CONSOLE:-http://127.0.0.1:8124}"
APP="${ELYSIA_APP:-http://127.0.0.1:8125}"
MUTANT=0
[ "${1:-}" = "--mutant" ] && MUTANT=1

PASS=0; FAILN=0; FAILED=()
ok()   { PASS=$((PASS+1));  printf '  PASS  %s\n' "$1"; }
no()   { FAILN=$((FAILN+1)); FAILED+=("$1"); printf '  FAIL  %s\n' "$1"; }
info() { printf '  info  %s\n' "$1"; }
stage(){ printf '\n== stage %s — %s ==\n' "$1" "$2"; }

# ── stage 0: the owner's ruling is intact and reachable ──────────────────────
stage 0 "ruling / register"
if [ -s "$REGISTER" ]; then ok "register exists and is non-empty"
else no "register missing or empty ($REGISTER)"; fi

B=$(grep -c '^@decision' "$REGISTER" 2>/dev/null || echo 0)
E=$(grep -cx '@end' "$REGISTER" 2>/dev/null || echo 0)
if [ "$B" -gt 0 ] && [ "$B" = "$E" ]; then ok "blocks paired: $B @decision = $E @end"
else no "block mismatch: $B @decision vs $E @end"; fi

DUP=$(grep '^@decision' "$REGISTER" 2>/dev/null | sort | uniq -d | wc -l)
if [ "$DUP" -eq 0 ]; then ok "no duplicate decision ids"
else no "$DUP duplicate decision ids"; fi

VAL=$(python3 - "$REGISTER" <<'PY'
import re,sys
try: txt=open(sys.argv[1]).read()
except OSError: print("ERR"); raise SystemExit
bad=0
for b in txt.split('@decision ')[1:]:
    opts=set(re.findall(r'^@option[ \t]+(\S+)',b,re.M))
    m=re.search(r'^@ruling[ \t]*(.*)$',b,re.M)
    r=(m.group(1).strip() if m else '')
    if r and r not in opts: bad+=1
print(bad)
PY
)
if [ "$VAL" = "0" ]; then ok "every written ruling is a declared option key"
else no "$VAL ruling(s) are not declared option keys"; fi

CH=$(curl -s --max-time 4 "$CONSOLE/health" 2>/dev/null)
# /health is PLAIN TEXT, not JSON: "ok <total> decided <ruled> of <total>".
# (An earlier version of this check looked for "ok":true and reported the
# service down while it was answering perfectly — assert the real contract.)
if printf '%s' "$CH" | grep -qE '^ok [0-9]+ decided'; then
  ok "console /health: $CH"
else
  no "console /health unexpected: ${CH:0:80}"
fi

# /api/decisions is an OBJECT: {"meta":[],"ratified":[],"decisions":[],"counts":{}}
# so len(payload) is 4, not the decision count. Read the fields by name.
API=$(curl -s --max-time 4 "$CONSOLE/api/decisions" 2>/dev/null \
      | python3 -c 'import json,sys
try:
    d=json.load(sys.stdin)
    c=d.get("counts") or {}
    total=c.get("total") or len(d.get("decisions") or [])
    ruled=c.get("ruled")
    if ruled is None:
        ruled=sum(1 for x in (d.get("decisions") or []) if (x.get("ruling") or "").strip())
    print(f"{total} {ruled} {total-int(ruled)}")
except Exception:
    print("0 0 0")')
read -r API_TOTAL API_RULED API_OPEN <<<"$API"
# `grep -c` prints 0 AND exits 1 when there are no matches, so `|| echo -1` appended a
# second line and FILE_OPEN became "0\n-1" — which read as a disagreement the first time
# the register legitimately reached zero open decisions. Separate "cannot read the file"
# from "read it, counted zero", and swallow grep's exit status without losing its stdout.
if [ -r "$REGISTER" ]; then
  FILE_OPEN=$(grep -c '^@ruling[[:space:]]*$' "$REGISTER" || true)
else
  FILE_OPEN=-1
fi
if [ "$API_TOTAL" = "$B" ]; then ok "console serves $API_TOTAL decisions = file's $B"
else no "console serves $API_TOTAL decisions but the file has $B"; fi
if [ "$API_OPEN" = "$FILE_OPEN" ]; then ok "open count agrees: $API_OPEN ruled=$API_RULED"
else no "open count disagrees: api=$API_OPEN file=$FILE_OPEN"; fi

# ── stage 1: the plan surface a newcomer would read ─────────────────────────
stage 1 "plan / docs"
for f in MANIFESTO.md PHASE1-CHECKLIST.md PHASE1-PLAN.md LOOSE-ENDS.md; do
  if [ -s "$RUN/$f" ]; then ok "$f present ($(stat -c%s "$RUN/$f") B)"
  else no "$f missing or empty"; fi
done
for f in README.md AGENTS.md; do
  if [ -s "$REPO/$f" ]; then ok "$f present"
  else info "$f absent (known open: E5)"; fi
done

# ── stage 2: the tree builds from the locked manifest ───────────────────────
stage 2 "build"
if cargo build --workspace --locked >/tmp/smoke-build.log 2>&1; then
  ok "cargo build --workspace --locked"
else
  no "cargo build failed (see /tmp/smoke-build.log)"; tail -5 /tmp/smoke-build.log | sed 's/^/        /'
fi

# ── stage 3: the merge gate ─────────────────────────────────────────────────
stage 3 "gate"
if bash scripts/gate.sh >/tmp/smoke-gate.log 2>&1; then
  ok "scripts/gate.sh rc=0 (builddet: build + determinism replay)"
else
  no "gate failed"; tail -12 /tmp/smoke-gate.log | sed 's/^/        /'
fi

# A gate that cannot fail is decoration. Inject a wall-clock read into the
# world's seeding so the same seed must produce different worlds, require the
# gate to fail, then restore the file and prove its hash came back.
if [ "$MUTANT" = 1 ]; then
  stage "3b" "gate can actually fail (negative control)"
  F=crates/elysia-core/src/lib.rs
  BAK=$(mktemp /tmp/smoke-mut.XXXXXX)
  cp "$F" "$BAK"
  SHA0=$(sha256sum "$F" | cut -d' ' -f1)
  N=$(python3 - "$F" <<'PY'
import sys
p=sys.argv[1]; s=open(p).read()
old="let mut rng = Rng::new(seed);"
new=("let mut rng = Rng::new(seed ^ std::time::SystemTime::now()"
     ".duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos() as u64);")
if old not in s:
    print(0); raise SystemExit
open(p,"w").write(s.replace(old,new,1))
print(1)
PY
)
  if [ "$N" = "1" ]; then
    ok "mutation applied (clock mixed into the seed)"
    if bash scripts/gate.sh >/tmp/smoke-mutant.log 2>&1; then
      no "gate PASSED with a nondeterministic world — the gate is not load-bearing"
    else
      ok "gate failed as it must (rc != 0)"
    fi
  else
    no "mutation did not apply — negative control unverified, not a pass"
  fi
  cp "$BAK" "$F"; rm -f "$BAK"
  SHA1=$(sha256sum "$F" | cut -d' ' -f1)
  if [ "$SHA0" = "$SHA1" ]; then ok "source restored, hash unchanged (${SHA0:0:12})"
  else no "RESTORE FAILED — ${F} differs from ${SHA0:0:12}"; fi
fi

# ── stage 4: the invariants, read from the layers themselves ────────────────
stage 4 "verify / invariants"
deps_of() { cargo tree -p "$1" --depth 1 --prefix none --locked 2>/dev/null \
            | sed '1d' | awk '{print $1}' | grep -v '^$' | sort -u | tr '\n' ' ' | sed 's/ *$//'; }
allows() { local got="$1"; shift; local d
  for d in $got; do case " $* " in *" $d "*) ;; *) return 1 ;; esac; done; return 0; }

CORE_D=$(deps_of elysia-core)
if [ -z "$CORE_D" ]; then ok "core depends on nothing (A10 inward-only)"
else no "core has dependencies: $CORE_D"; fi
if allows "$(deps_of elysia-world)" elysia-core; then ok "world depends on core only"
else no "world deps outside the boundary: $(deps_of elysia-world)"; fi
if allows "$(deps_of elysia-net)" elysia-core; then ok "net depends on core only"
else no "net deps outside the boundary: $(deps_of elysia-net)"; fi
if allows "$(deps_of elysia-app)" elysia-core elysia-world elysia-content elysia-net; then
  ok "app wires the layers, adds no layer"
else no "app deps outside the boundary: $(deps_of elysia-app)"; fi

TR=$(cargo test --workspace --locked 2>&1 \
     | grep -oE 'test result: ok\. [0-9]+ passed' | grep -oE '[0-9]+' \
     | awk '{s+=$1} END{print s+0}')
if [ "$TR" -ge 15 ]; then ok "$TR tests passed across the workspace"
else no "only $TR tests passed (expected at least 15)"; fi
for c in elysia-net elysia-app; do
  n=$(cargo test -p "$c" --locked 2>&1 | grep -oE 'test result: ok\. [0-9]+ passed' \
      | grep -oE '[0-9]+' | awk '{s+=$1} END{print s+0}')
  if [ "$n" -gt 0 ]; then ok "$c has $n tests"
  else info "$c has 0 tests (known gap: L02/L03)"; fi
done

SH=$(curl -s --max-time 3 "$APP/health" 2>/dev/null)
if printf '%s' "$SH" | grep -q '"ok":true'; then ok "app /health answers"
else no "app /health: ${SH:0:60}"; fi

SS=$(curl -s --max-time 3 "$APP/state" 2>/dev/null \
     | python3 -c 'import json,sys
try:
    d=json.load(sys.stdin); r=d.get("rows") or []
    print(d.get("w"),d.get("h"),len(r),len(r[0]) if r else 0)
except Exception: print("0 0 0 0")')
read -r SW SHH SROWS SRW <<<"$SS"
if [ "$SW" = "100" ] && [ "$SHH" = "100" ] && [ "$SROWS" = "100" ] && [ "$SRW" = "100" ]; then
  ok "/state is a 100x100 map with 100 rows of 100 chars"
else no "/state shape wrong: w=$SW h=$SHH rows=$SROWS rowlen=$SRW"; fi

# the move endpoint takes d=n|s|e|w (not "left"/"right": that spelling was the
# check's assumption, not the contract, and it read as a broken endpoint).
if curl -s --max-time 3 -X POST "$APP/move?d=s" 2>/dev/null | grep -q '"man"'; then
  ok "move endpoint answers with entity state"
else no "move endpoint did not return entity state"; fi

# ── stage 5: where the evidence lands ───────────────────────────────────────
stage 5 "record"
if [ -d loop-records ] && [ -w loop-records ]; then ok "repo loop-records/ writable"
else no "repo loop-records/ missing or not writable"; fi
if [ -d "$HOME/hermes-notes/runs/loop-records" ] && [ -w "$HOME/hermes-notes/runs/loop-records" ]; then
  ok "notes loop-records/ writable"
else no "notes loop-records/ missing or not writable"; fi

# ── stage 6: commit readiness ───────────────────────────────────────────────
stage 6 "commit"
DIRTY=$(git status --porcelain 2>/dev/null | wc -l)
if [ "$DIRTY" -eq 0 ]; then ok "working tree clean"
else no "$DIRTY path(s) uncommitted"; fi
BR=$(git rev-parse --abbrev-ref HEAD 2>/dev/null)
if [ "$BR" = "main" ]; then ok "on main (A9 trunk)"
else no "branch is '$BR', expected main"; fi
if git rev-parse --verify HEAD >/dev/null 2>&1; then
  ok "HEAD exists: $(git log -1 --format='%h %s' 2>/dev/null | cut -c1-52)"
else no "no commits yet"; fi

# ── stage 7: can a version be released and identified at runtime? ───────────
stage 7 "release"
VER=$(grep -m1 '^version' Cargo.toml 2>/dev/null | grep -oE '[0-9]+\.[0-9]+\.[0-9]+')
if [ -n "$VER" ] && [ "$VER" != "0.0.0" ]; then ok "workspace version is $VER"
else no "workspace version is '${VER:-none}' — 0.0.0 cannot name a release (E1/L08)"; fi
if printf '%s' "$SH" | grep -q 'version'; then ok "the running build reports its version"
else no "app /health does not report a version — 'which build is live?' is unanswerable (L08)"; fi
if compgen -G "$HOME/bin/elysia*" >/dev/null; then
  ok "installed artifact present: $(ls -1 "$HOME/bin"/elysia* 2>/dev/null | wc -l) file(s)"
else no "nothing installed in ~/bin (E2 manual release not yet run)"; fi
UNITS=$(systemctl --user list-unit-files 'elysia*' --no-legend 2>/dev/null | wc -l)
if [ "$UNITS" -gt 0 ]; then ok "$UNITS elysia unit file(s) present (A1 user unit)"
else no "no elysia systemd --user unit — a session reset kills the service (L09)"; fi

printf '\n──────── %d passed, %d failed ────────\n' "$PASS" "$FAILN"
[ "$FAILN" -eq 0 ] || { printf 'failed: %s\n' "${FAILED[*]}"; exit 1; }
exit 0
