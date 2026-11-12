# The canonical entry points. `make test` is the whole local gate: build + determinism
# replay, then the end-to-end suite including its negative control. Each target is a thin
# wrapper — the logic lives in scripts/, so there is exactly one copy of every rule.
#
# smoke reads notes-side evidence (plans, ledgers) from ELYSIA_RUN_DIR. Unset, those
# checks FAIL and name the variable; they do not skip. So on a maintainer machine:
#
#     ELYSIA_RUN_DIR=<run dir> make test

.PHONY: test gate smoke okr install rollback

test: gate smoke

gate:
	bash scripts/gate.sh

smoke:
	bash scripts/smoke.sh

# Movement, not soundness: reports the key results in OKR.md. Deliberately not part of
# `test` — an unmet key result is information, not a broken tree.
okr:
	python3 scripts/okr.py

install:
	bash scripts/install.sh

rollback:
	bash scripts/install.sh --rollback
