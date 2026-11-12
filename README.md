# elysia

A simulation engine, mostly text, played in a browser over a LAN.

The deployment target is a Rockchip RK3566 with 1 GB of RAM, where not all of it
is usable. That budget is the reason for the shape of everything here: a single
static binary, no framework, no runtime, no database, and the browser doing the
drawing.

**Status: 0.1.0.** One 100×100 map of generated terrain and one entity that
walks on it. Everything else is a decision recorded in the register, not a
feature that exists yet.

## Authorship

**This code is written by an AI agent.** Every commit is produced by Hermes, a
Nous Research agent instance, working under the direction of the repository
maintainer. The maintainer writes the rulings in `register/decisions.txt`; the
agent implements them; `scripts/gate.sh` and `scripts/smoke.sh` decide whether
the result is acceptable. Nothing is merged because the agent says it works.

This is stated up front because it is material to how you should read the code.
The prose here is machine-generated and reads as confident by default. Trust the
register and the gates, not the tone.

## The register

`register/decisions.txt` is the source of truth for this project — every design
decision, what was ruled, what was deferred and why. The console in
`~/hermes-notes/.../elysia-console/` serves it at `http://<host>:8124/`, reading
the file on every request so hand edits appear immediately.

Only the owner writes `@ruling` and `@rationale`. Contributions may add
`@context`, `@option` and `@agent`; they may never fill a ruling in.

## Run it

```bash
cargo run -p elysia-app -- 0.0.0.0:8125 20261111     # bind addr, seed
```

Then open `http://<host>:8125/`. Routes:

- `/` — the client (renders map and entity, sends moves)
- `/state` — JSON: seed, size, entity, state hash, 100 rows of 100 chars
- `/move?d=n|s|e|w` — one step, returns the new state
- `/health` — JSON: `ok`, `version`, seed, entity, state hash

## Check it

```bash
cargo test --workspace          # 18 tests
./scripts/gate.sh               # what a change must pass: build + determinism
./scripts/smoke.sh              # all eight stages of the loop
./scripts/smoke.sh --mutant     # ...and proves the gate can actually fail
```

The smoke test injects a wall-clock read into world seeding, requires the gate to
go red, then restores the file and verifies its hash came back. A gate that
cannot fail is decoration.

## Release it

```bash
./scripts/install.sh                       # build, install, flip, restart, verify
./scripts/install.sh --rollback            # undo: the same flip reversed
./scripts/install.sh --target aarch64-unknown-linux-musl   # stage for the board
./scripts/install.sh --dry-run             # print the plan, change nothing
```

The installer refuses to report success unless the live service reports the
version it just installed. The deployment point is the `~/bin/elysia` symlink,
so releasing never edits the unit and rolling back never rebuilds anything. The
unit itself is `deploy/elysia.service`.

## Layout

- `crates/elysia-core` — no dependencies; hashing, rng, primitives
- `crates/elysia-world` — terrain, chunks, the entity
- `crates/elysia-content` — data tables
- `crates/elysia-net` — HTTP/TCP transport, no game knowledge
- `crates/elysia-app` — the binary: routes and wiring
- `crates/<crate>/tests/` — the harness
- `register/decisions.txt` — the register
- `deploy/elysia.service` — the systemd user unit
- `scripts/gate.sh`, `scripts/smoke.sh`, `scripts/install.sh`

Dependencies point inward only. `scripts/smoke.sh` checks the dependency graph,
so a wrong-way import fails loudly instead of being caught in review.

## Measured

On the x86_64 host, 2026-11-12: artifact 310,656 B (budget 4 MB), ~520 KB
resident under systemd (budget 64 MB). Numbers measured here are not
automatically true on the board — re-measure before relying on them.

## Licence

AGPL-3.0-only. See `LICENSE`.
