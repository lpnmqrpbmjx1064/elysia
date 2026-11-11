#!/usr/bin/env python3
"""Report decision state from the register — the authoritative field, mechanically.

The register is line-oriented: a line starting with `@` opens a field, and any
non-`@` line after it continues that field. That continuation rule is why this
tool exists: `@ruling` followed by plain text IS a ruling, while `@ruling`
followed by `@rationale` is an empty ruling. Reading that by eye is how a
recommendation in `@agent` once got reported as the owner's decision.

    ./scripts/register_state.py             summary + every open decision
    ./scripts/register_state.py E5          one decision: RULED <value> | OPEN
    ./scripts/register_state.py --open      open ids only, one per line
    ./scripts/register_state.py --assert E5=agentsmd    exit 0 only if E5 is
                                            ruled AND equals agentsmd
    ./scripts/register_state.py --json      machine-readable summary

Exit: 0 answered · 1 assertion failed · 2 register missing/empty/unparseable.
Fail-closed on purpose: a consumer that cannot read its input must not report
"clean" — an empty decision list is a defect, never zero problems.
"""
from __future__ import annotations

import json
import sys
from pathlib import Path

REGISTER = Path(__file__).resolve().parent.parent / "register" / "decisions.txt"


def parse(path: Path) -> list[dict[str, str]]:
    text = path.read_text(encoding="utf-8")
    decisions: list[dict[str, str]] = []
    cur: dict[str, str] | None = None
    field: str | None = None

    for raw in text.splitlines():
        line = raw.rstrip("\n")
        if line.startswith("#"):
            continue
        if line.startswith("@"):
            head, _, rest = line[1:].partition(" ")
            head = head.strip()
            rest = rest.strip()
            if head == "decision":
                cur = {"id": rest}
                decisions.append(cur)
                field = "id"
                continue
            if head == "end":
                cur = None
                field = None
                continue
            if cur is not None:
                # A repeated field keeps the first value; later ones are noise
                # by construction, and silent overwrite hides format drift.
                cur.setdefault(head, rest)
                field = head
            continue
        if cur is not None and field is not None and line.strip():
            prev = cur.get(field, "")
            cur[field] = (prev + " " + line.strip()).strip()

    return decisions


def main() -> int:
    if not REGISTER.exists():
        print(f"UNVERIFIED: no register at {REGISTER}", file=sys.stderr)
        return 2
    decisions = parse(REGISTER)
    if not decisions:
        print("UNVERIFIED: register parsed to 0 decisions", file=sys.stderr)
        return 2

    ruled = [d for d in decisions if (d.get("ruling") or "").strip()]
    open_ = [d for d in decisions if not (d.get("ruling") or "").strip()]
    args = sys.argv[1:]

    if not args:
        print(f"register : {REGISTER}")
        print(f"decisions: {len(decisions)}  ruled: {len(ruled)}  open: {len(open_)}")
        for d in open_:
            print(f"  OPEN  {d['id']:<4} {(d.get('title') or '')[:66]}")
        return 0

    if args[0] == "--open":
        for d in open_:
            print(d["id"])
        return 0

    if args[0] == "--json":
        print(json.dumps({
            "register": str(REGISTER),
            "total": len(decisions),
            "ruled": len(ruled),
            "open": [d["id"] for d in open_],
            "decisions": decisions,
        }, indent=2))
        return 0

    if args[0] == "--assert":
        spec = args[1] if len(args) > 1 else ""
        want_id, _, want_val = spec.partition("=")
        if not want_id or not want_val:
            print("usage: --assert ID=value", file=sys.stderr)
            return 1
        hit = next((d for d in decisions if d["id"] == want_id), None)
        if hit is None:
            print(f"FAIL  no decision {want_id}")
            return 1
        got = (hit.get("ruling") or "").strip()
        if not got:
            print(f"FAIL  {want_id} is OPEN (no ruling)")
            return 1
        if got != want_val:
            print(f"FAIL  {want_id} is ruled '{got}', not '{want_val}'")
            return 1
        print(f"OK    {want_id} = {got}")
        return 0

    for want in args:
        hit = next((d for d in decisions if d["id"] == want), None)
        if hit is None:
            print(f"UNKNOWN  {want}")
            continue
        got = (hit.get("ruling") or "").strip()
        print(f"RULED    {want} = {got}" if got else f"OPEN     {want}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
