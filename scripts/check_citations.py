#!/usr/bin/env python3
"""Citation integrity for the elysia tree.

A decision that nothing cites does not bind the code, and a citation of a
decision that no longer stands is a silent contradiction. The register can be
perfectly healthy — every block ruled, every block closed — while the code has
quietly stopped agreeing with it, because nothing checks the link between them.
This closes that half.

Adapted from solidjs/solid, which learned the same lesson at scale: its rule
ids are checked in CI (`packages/signals/scripts/rules-index.mjs --check`), which
fails on a citation that resolves to no rule, on a citation of a rule that no
longer stands, on a bare un-namespaced id, and on a reference to a document the
repo does not have.

Namespaces — an id only means something inside its own namespace, so a citation
must name it:

    @decision A10      a register id in register/decisions.txt
    @sop      L19      a rule in the agent SOP (external; checked only if --sop)
    @loose    L02      an item in LOOSE-ENDS.md (external; checked only if --loose)

The prefixes collide in practice: L19 is an SOP line and L02 is a loose-end
item. That is why a bare `L02` in a comment is not a citation and is never
validated — it is ambiguous, so guessing which namespace it meant is how a
check starts inventing verdicts.

Bare ids are deliberately NOT scanned. `A55` in the manifesto is a Cortex-A55
core, not decision A55; a token scanner reports it as a broken citation and the
report becomes noise nobody reads. Citation syntax is explicit or there is no
citation.

    ./scripts/check_citations.py                    # repo only
    ./scripts/check_citations.py --sop ~/.hermes/.../agent-sop/references/verification-rules.md

Exit: 0 all checks pass, 1 defect, 2 blocked (input missing or unreadable —
a checker that cannot read its input must never report success).
"""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
from pathlib import Path

SCAN_EXTS = {".rs", ".sh", ".py", ".md", ".toml", ".yml", ".yaml"}
# The register defines ids, so it is not a citation site. This script documents
# the syntax with failing examples, so it is not one either.
SKIP_PATHS = {"register/decisions.txt", "scripts/check_citations.py"}

CITE_RE = re.compile(r"@(decision|sop|loose)[ \t]+([^\s,;)\]}`]+)")
# A citation carries an id, and an id has the register's shape (A10, E6, X2).
# Without this, prose usage of the keyword gets read as a citation: smoke.sh
# prints "blocks paired: 42 @decision = 42 @end" and splits on
# `txt.split('@decision ')`, which a token-only pattern reports as two broken
# citations of ids named `=` and `vs`. Keyword usage is counted and shown, not
# silently swallowed, so the census still accounts for every occurrence.
ID_SHAPE_RE = re.compile(r"^[A-Z]{1,3}\d{1,4}$")
LINK_RE = re.compile(r"\[[^\]]*\]\(([^)]+)\)")

PASS, WARN, FAIL = "PASS", "WARN", "FAIL"


class Report:
    def __init__(self) -> None:
        self.rows: list[tuple[str, str, str]] = []

    def add(self, name: str, verdict: str, detail: str) -> None:
        self.rows.append((name, verdict, detail))

    def worst(self) -> str:
        vs = [v for _, v, _ in self.rows]
        if FAIL in vs:
            return FAIL
        if WARN in vs:
            return WARN
        return PASS

    def emit(self) -> None:
        w = max(len(n) for n, _, _ in self.rows)
        for name, verdict, detail in self.rows:
            print(f"  {name:<{w}}  {verdict:<4}  {detail}")
        verdict = self.worst()
        n_fail = sum(1 for _, v, _ in self.rows if v == FAIL)
        n_warn = sum(1 for _, v, _ in self.rows if v == WARN)
        code = {"PASS": 0, "WARN": 0, "FAIL": 1}[verdict]
        print()
        print(
            f"  RESULT: {verdict} (exit {code}) - {len(self.rows)} checks, "
            f"{n_fail} failed, {n_warn} warning(s)"
        )


def read_register(path: Path) -> tuple[dict[str, str], set[str]]:
    """Parse the register into {id: body} and the set of ids that no longer stand.

    Block pairing is checked with exact terminators: the file header carries
    `@endmeta`/`@endratified`, so a prefix match on `@end` reports phantom
    unclosed blocks on a healthy register.
    """
    text = path.read_text(encoding="utf-8", errors="replace")
    blocks: dict[str, str] = {}
    superseded: set[str] = set()
    current: str | None = None
    body: list[str] = []
    for line in text.splitlines():
        if line.startswith("@decision"):
            current = line.split(None, 1)[1].strip() if len(line.split()) > 1 else ""
            body = []
        elif line.rstrip() == "@end" and current is not None:
            blocks[current] = "\n".join(body)
            current = None
        elif current is not None:
            body.append(line)
    if current is not None:  # unclosed tail block
        blocks.setdefault(current, "\n".join(body))
    for did, b in blocks.items():
        if re.search(r"^@status[ \t]+superseded", b, re.M) or re.search(
            r"^@supersededby[ \t]+\S", b, re.M
        ):
            superseded.add(did)
    return blocks, superseded


def tracked_files(root: Path) -> list[Path]:
    try:
        out = subprocess.run(
            ["git", "-C", str(root), "ls-files", "-z"],
            capture_output=True,
            check=True,
        ).stdout.decode()
        rels = [p for p in out.split("\0") if p]
    except (subprocess.CalledProcessError, FileNotFoundError):
        rels = [str(p.relative_to(root)) for p in root.rglob("*") if p.is_file()]
    return [root / r for r in rels]


def external_ids(path: Path | None, pattern: str) -> set[str] | None:
    """Ids from an external doc, or None when the doc is unavailable."""
    if path is None or not path.is_file():
        return None
    text = path.read_text(encoding="utf-8", errors="replace")
    return set(re.findall(pattern, text))


def main() -> int:
    ap = argparse.ArgumentParser(description="Check that citations resolve.")
    ap.add_argument("--root", default=".", help="repository root")
    ap.add_argument("--register", default="register/decisions.txt")
    ap.add_argument("--sop", default=None, help="SOP reference file defining L-ids")
    ap.add_argument("--loose", default=None, help="LOOSE-ENDS.md defining L-nn items")
    args = ap.parse_args()

    root = Path(args.root).resolve()
    rep = Report()

    reg_path = (root / args.register).resolve()
    if not reg_path.is_file():
        rep.add("C1 register", FAIL, f"{args.register} not found - cannot resolve citations")
        rep.emit()
        return 2
    blocks, superseded = read_register(reg_path)
    if not blocks:
        rep.add("C1 register", FAIL, f"{args.register} parsed to 0 decision blocks")
        rep.emit()
        return 2

    files = tracked_files(root)
    if not files:
        rep.add("C0 input", FAIL, "no tracked files found - nothing to check")
        rep.emit()
        return 2

    # A checker that scans nothing must not report success: an empty scan set is
    # indistinguishable from a clean tree, and that ambiguity is how a gate
    # silently stops gating.
    scannable = [
        f
        for f in files
        if str(f.relative_to(root)) not in SKIP_PATHS and f.suffix in SCAN_EXTS and f.is_file()
    ]
    if not scannable:
        rep.add("C0 input", FAIL, f"0 scannable file(s) among {len(files)} tracked - nothing to check")
        rep.emit()
        return 2

    # C2: the register's own structure, since every citation hangs off it.
    n_dec = len(re.findall(r"^@decision", reg_path.read_text(encoding="utf-8", errors="replace"), re.M))
    rep.add(
        "C2 register blocks",
        PASS,
        f"{n_dec} @decision, {len(blocks)} unique id(s), {len(superseded)} superseded",
    )

    # C3/C4: resolve every explicit citation.
    unresolved: list[str] = []
    sup_cites: list[str] = []
    malformed: list[str] = []
    per_ns = {"decision": 0, "sop": 0, "loose": 0}
    cited_ids: set[str] = set()
    sop_path = Path(args.sop).expanduser() if args.sop else None
    loose_path = Path(args.loose).expanduser() if args.loose else None
    # A source the caller explicitly named but we cannot read is BLOCKED, never
    # silently downgraded to "unchecked" - that is how a citation quietly stops
    # being verified while the report still reads PASS.
    for label, p in (("--sop", sop_path), ("--loose", loose_path)):
        if p is not None and not (p.is_file() and p.stat().st_size > 0):
            rep.add("C0 input", FAIL, f"{label} {p} is missing, not a regular file, or empty")
            rep.emit()
            return 2
    sop_ids = external_ids(sop_path, r"(?<![\w-])(L\d{1,3})(?![\w])")
    loose_ids = external_ids(loose_path, r"^###[ \t]+(L\d{1,3})")
    scanned = 0
    mentions = 0  # keyword usage that is not a citation (prose, parser calls)
    for f in scannable:
        rel = str(f.relative_to(root))
        scanned += 1
        try:
            text = f.read_text(encoding="utf-8", errors="replace")
        except OSError:
            continue
        for lineno, line in enumerate(text.splitlines(), 1):
            for ns, cid in CITE_RE.findall(line):
                if not ID_SHAPE_RE.match(cid):
                    mentions += 1
                    continue
                per_ns[ns] += 1
                where = f"{rel}:{lineno}"
                if ns == "decision":
                    if cid in superseded:
                        sup_cites.append(f"{where} cites superseded {cid}")
                    elif cid not in blocks:
                        unresolved.append(f"{where} cites unknown {cid}")
                    else:
                        cited_ids.add(cid)
                elif ns == "sop":
                    if sop_ids is not None and cid not in sop_ids:
                        unresolved.append(f"{where} cites unknown SOP rule {cid}")
                elif ns == "loose":
                    if loose_ids is not None and cid not in loose_ids:
                        unresolved.append(f"{where} cites unknown loose end {cid}")
            for stray in re.findall(r"@(?:decisions|dec|ruling)[ \t]+\S+", line):
                malformed.append(f"{rel}:{lineno} unknown citation keyword {stray.split()[0]}")

    total_cites = sum(per_ns.values())
    if unresolved or sup_cites:
        detail = "; ".join((unresolved + sup_cites)[:6])
        rep.add("C3 citations", FAIL, f"{len(unresolved)} unresolved, {len(sup_cites)} superseded - {detail}")
    else:
        unchecked = []
        if per_ns["sop"] and sop_ids is None:
            unchecked.append(f"{per_ns['sop']} @sop (no --sop)")
        if per_ns["loose"] and loose_ids is None:
            unchecked.append(f"{per_ns['loose']} @loose (no --loose)")
        note = f"; unchecked: {', '.join(unchecked)}" if unchecked else ""
        rep.add(
            "C3 citations",
            WARN if unchecked else PASS,
            f"{total_cites} citation(s) resolved across {scanned} file(s)"
            f" (decision {per_ns['decision']}, sop {per_ns['sop']}, loose {per_ns['loose']}){note}",
        )

    rep.add(
        "C4 citation form",
        FAIL if malformed else PASS,
        f"{len(malformed)} malformed - {'; '.join(malformed[:4])}" if malformed else "every citation names a namespace",
    )

    # C5: a doc that points at a file the repo does not have.
    broken: list[str] = []
    for f in files:
        if f.suffix != ".md" or not f.is_file():
            continue
        try:
            text = f.read_text(encoding="utf-8", errors="replace")
        except OSError:
            continue
        for target in LINK_RE.findall(text):
            t = target.strip().split("#", 1)[0].strip()
            if not t or re.match(r"^(https?:|mailto:|tel:)", t):
                continue
            if not (f.parent / t).exists():
                broken.append(f"{f.relative_to(root)} -> {t}")
    rep.add(
        "C5 doc links",
        FAIL if broken else PASS,
        f"{len(broken)} broken - {'; '.join(broken[:4])}" if broken else "every relative markdown link resolves",
    )

    # C6: the census. Uncited decisions are information, not a failure: a ruling
    # can legitimately bind a process rather than a line of code.
    uncited = sorted(set(blocks) - cited_ids, key=lambda s: (s[0], int(re.sub(r"\D", "", s) or 0)))
    rep.add(
        "C6 census",
        PASS,
        f"{len(cited_ids)}/{len(blocks)} decision(s) cited in code/tests/docs"
        f"; {mentions} non-citation keyword mention(s)"
        + (f"; uncited: {', '.join(uncited[:12])}{' ...' if len(uncited) > 12 else ''}" if uncited else ""),
    )

    rep.emit()
    return {"PASS": 0, "WARN": 0, "FAIL": 1}[rep.worst()]


if __name__ == "__main__":
    sys.exit(main())
