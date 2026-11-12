#!/usr/bin/env python3
"""Measure the key results declared in OKR.md. Reports movement; never gates.

Grammar (line-oriented, same idiom as register/decisions.txt — @ starts a field,
a line that does not is continuation text appended to the previous field):

    @meta <key> = <value>
    @objective O1 | <one-line text>
    @why ... / @endobjective
    @kr N1
    @objective O1
    @title ...
    @baseline <number>
    @target <op><number>        op in: >= <= == > <
    @measure <one-line shell command that prints one number>
    @end

Fails closed: exit 2 when OKR.md is missing, unreadable, declares no key results, or when
any @measure produces no number. A report over empty or unparsable input is not a clean
report, and "could not measure" must never render as "on target".
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "OKR.md"
NUM = re.compile(r"-?\d+(?:\.\d+)?")
TARGET = re.compile(r"^\s*(>=|<=|==|>|<)\s*(-?\d+(?:\.\d+)?)\s*$")

OP = {
    ">=": lambda a, b: a >= b,
    "<=": lambda a, b: a <= b,
    "==": lambda a, b: a == b,
    ">": lambda a, b: a > b,
    "<": lambda a, b: a < b,
}


def parse(text: str) -> tuple[dict[str, str], dict[str, str], list[dict[str, str]]]:
    """Return (meta, objectives, krs). Unknown fields are preserved, never fatal."""
    meta: dict[str, str] = {}
    objectives: dict[str, str] = {}
    krs: list[dict[str, str]] = []
    cur: dict[str, str] | None = None
    field = ""

    for raw in text.splitlines():
        line = raw.rstrip()
        if not line.strip() or line.lstrip().startswith("#"):
            continue
        if line.startswith("@"):
            tag, _, rest = line.partition(" ")
            tag, rest = tag[1:], rest.strip()
            if tag == "meta":
                key, _, val = rest.partition("=")
                meta[key.strip()] = val.strip()
                continue
            if tag == "objective" and "|" in rest and cur is None:
                oid, _, name = rest.partition("|")
                objectives[oid.strip()] = name.strip()
                continue
            if tag == "kr":
                if cur:
                    krs.append(cur)
                cur, field = {"id": rest}, ""
                continue
            if tag == "end":
                if cur:
                    krs.append(cur)
                cur, field = None, ""
                continue
            if cur is not None and tag in ("objective", "title", "baseline", "target", "measure", "why"):
                cur[tag] = rest
                field = tag
            continue
        if cur is not None and field:
            cur[field] = (cur.get(field, "") + " " + line.strip()).strip()
    if cur:
        krs.append(cur)
    return meta, objectives, krs


def measure(cmd: str) -> float | None:
    """Run one @measure and return the first number it prints, or None if it prints none."""
    try:
        out = subprocess.run(
            cmd, shell=True, cwd=ROOT, capture_output=True, text=True, timeout=600
        )
    except subprocess.TimeoutExpired:
        return None
    hit = NUM.search(out.stdout or "")
    return float(hit.group()) if hit else None


def main() -> int:
    if not SOURCE.is_file():
        print(f"BLOCKED: {SOURCE} is missing", file=sys.stderr)
        return 2
    meta, objectives, krs = parse(SOURCE.read_text())
    if not krs:
        print(f"BLOCKED: no @kr blocks in {SOURCE} — nothing to report is not a pass", file=sys.stderr)
        return 2

    print(f"cycle: {meta.get('cycle', '?')} (opened {meta.get('opened', '?')})\n")
    met = unmeasured = 0
    current = None
    for kr in krs:
        obj = kr.get("objective", "?")
        if obj != current:
            current = obj
            print(f"{obj} — {objectives.get(obj, '(no @objective line)')}" if obj not in objectives
                  else f"{obj} — {objectives[obj]}")
        target = TARGET.match(kr.get("target", ""))
        cmd = kr.get("measure", "")
        if not target or not cmd:
            print(f"  {kr['id']:<4} MALFORMED (needs @target <op><num> and @measure): {kr.get('title','')}")
            unmeasured += 1
            continue
        op, want = target.group(1), float(target.group(2))
        got = measure(cmd)
        if got is None:
            print(f"  {kr['id']:<4} UNMEASURED — command printed no number: {cmd[:60]}")
            unmeasured += 1
            continue
        hit = OP[op](got, want)
        met += hit
        verdict = "MET" if hit else "miss"
        print(f"  {kr['id']:<4} {kr.get('title',''):<38} {kr.get('baseline','?'):>8} -> {op}{want:<9g} now {got:<9g} {verdict}")

    total = len(krs)
    print(f"\nscore: {met}/{total} met" + (f", {unmeasured} unmeasured" if unmeasured else ""))
    if unmeasured:
        print("BLOCKED: a key result that cannot be measured invalidates the report", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
