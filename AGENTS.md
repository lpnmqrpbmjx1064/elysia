# AGENTS.md

Operational instructions for an agent working in this repository.

This file is a map, not a constitution. Every decision lives in
`register/decisions.txt`; every rule is enforced by `scripts/gate.sh` and
`scripts/smoke.sh`. If this file and the register disagree, **the register wins** —
so this file restates no decision, and there is exactly one copy of each.

## What this is

`elysia` — a deterministic, mostly-text simulation engine served to a browser over a
LAN, budgeted for a 1 GB RK3566 (E1, Xen). Rust workspace, path dependencies only: no
external crates, no runtime, no framework.

## Commands

```sh
cargo build --workspace --locked   # offline; path deps only
bash scripts/gate.sh               # build + determinism replay; exit 1 on any failure
bash scripts/smoke.sh              # end-to-end, includes a negative control
bash scripts/install.sh            # build, install, flip symlink, restart, verify version
bash scripts/install.sh --rollback # flip back to the previous version
```

Do not grade a gate through a pipe. `bash gate.sh | tail` reports `tail`'s exit code,
not the gate's — capture to a file, then read `$?`.

## Before you change anything

1. **A decision is ruled only when `@ruling` is non-empty.** `@agent` holds a
   recommendation and is never a ruling. Check mechanically; do not read intent into
   the file:
   ```sh
   python3 scripts/register_state.py --assert E5=agentsmd   # exit 1 if the register disagrees
   python3 scripts/register_state.py --open                 # what is still unruled
   ```
2. **Never write `@ruling` or `@rationale`** — those belong to the owner. An agent may
   write `@context`, `@option` and `@agent`. Recording a ruling the owner has stated in
   conversation is transcription, and must carry a `@ruledby` line naming the source.
3. **Prove the gate can fail.** A passing gate proves nothing by itself. Perturb the
   input, confirm exit 1, restore, confirm exit 0 *and* a byte-identical file.

## Rules the toolchain cannot check for you

- **Determinism (P3, Det) is mandatory, not aspirational.** No wall clock, no
  `SystemTime`/`Instant`, no thread races, no iteration over a `HashMap`'s order, no
  float in engine state. Integer arithmetic only. If something needs to know "when",
  the time is an input.
- **State must be reconstructible from `(seed, inputs)`.** `state_hash()` covers
  everything that can change; `hash()` covers only the terrain. Keep them distinct, and
  keep the replay harness pointed at the world that ships — not at a scaffold.
- **One step at a time.** Land small verified increments. A single all-at-once patch
  makes the failing step ambiguous and is not accepted here.

## Layout and the boundary (A10)

```
crates/elysia-core     Rng + hash primitives. No domain knowledge.
crates/elysia-world    terrain, entities, state_hash. Depends on core only.
crates/elysia-net      hand-rolled HTTP: parse request, write response. It is
                       transport and nothing else — it must NOT depend on world
                       or content.
crates/elysia-content  procedural content (C6). A workspace member, wired when there
                       is content to wire. Do not add it as a dependency before then.
crates/elysia-app      the only crate that wires the others together.
```

A declared dependency the code never imports is a defect, not a placeholder: it lets a
wrong-way import compile silently. Delete it until it is genuinely needed.

## Declared gaps — do not mistake them for coverage

- The autonomous tick loop is deferred (E4 `nextver`). Steps are player-triggered, so
  the replay harness replays an **input sequence**, not elapsed time.
- `elysia-net` and `elysia-app` compile test binaries containing **zero tests**. The
  request parser consumes untrusted bytes and is uncovered (L02/L03).
- The aarch64 build compiles and is proven static; **executing** it is unproven.

## Releasing

`scripts/install.sh` reads the version from the workspace manifest
(`version.workspace = true`; never hand-edit it in a crate), installs to
`~/bin/elysia-<version>`, flips the `elysia` symlink, restarts the user unit, and then
**requires the live service to report the version it just installed**. On failure it
prints the rollback instruction and exits non-zero; `--rollback` flips the symlink back,
and it exits 3 when there is nothing to roll back to.
