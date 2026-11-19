# Changelog

All notable changes to this project are recorded here.
Format: [Keep a Changelog](https://keepachangelog.com/en/1.1.0/); versioning: [SemVer](https://semver.org/).

The version lives in `Cargo.toml` under `[workspace.package].version` — not in this file.
A release is a `v<version>` tag, cut on the same commit that bumps that field and dates the
section below, so the tag, the declared version and these notes cannot drift apart.

## [Unreleased]

### Added

- `make okr` reads `OKR.md`, runs each declared `@measure` command and prints the numbers.
  `scripts/okr.py` computes the score, so a key result is a number a command produces and
  never a number written down by hand.
- `make test` is the canonical entry point — `gate.sh` then `smoke.sh` — so the command the
  tooling recognises is the command that runs everything.
- `AGENTS.md` lists `make okr` beside `make test`, as a command and nothing more: the rules
  stay in `OKR.md`, which owns them.

### Changed

- `AGENTS.md` is a map for agents, not a second copy of the decisions.
- The register's `@meta owner` names the GitHub handle rather than a personal name.
- The repository declares that its code is written by an AI agent (A12).

### Fixed

- Smoke no longer assumes a host layout: the repo carries no host layout, and says so when it
  lacks evidence instead of failing quietly.
- Smoke: no internal network address in the published tree.
- Smoke: the open-count check failed only at zero open decisions.
- The determinism harness now tests the world that ships.
- A ruling claim is now one command from proof (`register_state.py --assert <id>=<key>`).
- Dependency hygiene: two declared-but-unused `elysia-content` edges dropped; `.gitattributes`
  added so the tree checks out the same way everywhere.
- Python bytecode kept out of the tree.

## [0.1.0] — 2026-11-12

### Added

- First demo: a 100x100 chunked world with ground and one man (I2).
- Tooling-first scaffold (A11).
- The decision register enters git, and the release path survives a bad day.
- The release is identifiable and the fingerprint honest.

[Unreleased]: https://github.com/lpnmqrpbmjx1064/elysia/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/lpnmqrpbmjx1064/elysia/releases/tag/v0.1.0
