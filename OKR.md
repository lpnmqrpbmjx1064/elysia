# elysia — key results

Internal project management. Three artifacts, one job each, no overlap:

| Artifact | Records | Fails when |
|---|---|---|
| `register/decisions.txt` | what was **decided** | a decision is unruled (`register_state.py`) |
| `Makefile` → `gate.sh` + `smoke.sh` | whether the tree is **sound** | code is wrong, evidence missing, tree dirty |
| this file | whether anything **moved** | nothing — it measures, it never gates |

`make okr` reads this file, runs each `@measure` command, and prints the numbers.
The script scores. Never write a score here by hand: a self-graded number is a prior,
not a measurement.

**A key result is a number a command produces.** If it cannot be measured by `make okr`
it is not a key result — it belongs in `LOOSE-ENDS.md` as a wish. Example: "measured on
the RK3566" is real and important, and it is *not* a KR here, because no command on this
machine can produce it. It stays a ledger item until the board is in reach.

**Cycle = one release.** Re-measure at each tag. Never re-baseline quietly, and never
lower a target inside a cycle — that is how this file becomes decoration.

@meta cycle = v0.1.0 -> v0.2.0
@meta opened = 2026-11-12

@objective O1 | The code that faces the network is trustworthy
@why elysia-net parses bytes off a socket. It has zero tests, and the repository is now
public, so the gap is no longer private. This is the first thing a reader would attack and
the last thing the gate covers.
@endobjective

@kr N1
@objective O1
@title elysia-net tests, including hostile input
@baseline 0
@target >=20
@measure cargo test -p elysia-net -- --list 2>/dev/null | grep -c ': test$'
@end

@objective O2 | No crate that contains code is untested
@why Five crates exist and one of them carries all 19 tests. A crate with zero tests is a
crate whose behaviour is asserted only by the fact that it compiles.
@endobjective

@kr N2
@objective O2
@title crates with zero tests
@baseline 4
@target <=1
@measure for c in crates/*/; do p=$(basename "$c"); n=$(cargo test -p "$p" -- --list 2>/dev/null | grep -c ': test$'); [ "$n" -eq 0 ] && echo "$p"; done | wc -l
@end

@objective O3 | The footprint stays a budget, not a hope
@why The whole design is justified by a 4 MB / 64 MB envelope on a 1 GB board. Binary size
creeps silently; a ceiling turns creep into a decision somebody has to make on purpose.
@endobjective

@kr D1
@objective O3
@title release binary bytes
@baseline 310656
@target <=320000
@measure stat -c%s target/release/elysia-app
@end

@objective O4 | Decisions stay ruled
@why An unruled decision is a place where the agent would have to guess, and guessing is
the thing this project spends its gates preventing. Zero is the correct number and it is
worth holding, not improving.
@endobjective

@kr R1
@objective O4
@title decisions with an empty @ruling
@baseline 0
@target ==0
@measure python3 scripts/register_state.py --json | python3 -c 'import json,sys; print(len(json.load(sys.stdin)["open"]))'
@end
