#!/usr/bin/env python3
"""Emit detailed WORKLIST rows from MCQ answers once all 100 are recorded."""
from __future__ import annotations

import json
import re
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parent
BANK = ROOT / "win11-community-audit-100.md"
ANSWERS = ROOT / "answers.json"
OUT = ROOT / "WORKLIST-FROM-AUDIT.md"
OUT_CRATES = ROOT.parent / "WORKLIST-COMMUNITY-AUDIT.md"


def parse_bank(text: str) -> dict[int, dict]:
    items: dict[int, dict] = {}
    blocks = re.split(r"\n(?=## \d{3} — )", text)
    for block in blocks:
        m = re.match(r"## (\d{3}) — (.+)", block)
        if not m:
            continue
        n = int(m.group(1))
        title = m.group(2).strip()
        community = re.search(r"\*\*Community:\*\* (.+)", block)
        today = re.search(r"\*\*MDE today:\*\* (.+)", block)
        suggestion = re.search(r"\*\*Suggestion:\*\* (.+)", block)
        choices = dict(re.findall(r"^- ([A-E])\) (.+)$", block, re.M))
        items[n] = {
            "title": title,
            "community": community.group(1).strip() if community else "",
            "today": today.group(1).strip() if today else "",
            "suggestion": suggestion.group(1).strip() if suggestion else "",
            "choices": choices,
        }
    return items


def choice_label(item: dict, choice: str) -> str:
    ch = choice.upper().strip()
    if ch in item["choices"]:
        return item["choices"][ch]
    parts = re.findall(r"[A-E]", ch)
    if len(parts) > 1:
        return " + ".join(item["choices"].get(p, p) for p in parts)
    return "(custom)"


def priority(choice: str, label: str, note: str) -> str:
    ch = choice.upper()
    blob = f"{label} {note}".lower()
    if "skip" in blob or ch == "D" and "defer" in blob:
        if "skip" in blob:
            return "skip"
    if ch in {"D"} and ("skip" in blob or "defer" in blob):
        return "skip" if "skip" in blob else "deferred"
    if ch.startswith("A"):
        return "P1"
    if ch in {"B", "C"} or "+" in ch:
        return "P2"
    return "P3"


def work_steps(n: int, title: str, choice: str, label: str, today: str, note: str) -> list[str]:
    steps = [
        f"Confirm current tree still matches: {today}",
        f"Implement decision **{choice.upper()}**: {label}",
    ]
    if note:
        steps.append(f"Honor user note: {note}")
    steps.extend(
        [
            "Touch the owning crate(s): typically `mde-shell`, `mde-control`, `mde-config`, and/or `mde-comp`/`mde-anvil` as required by the surface.",
            "Persist via typed `config.toml` fields and/or `extras` bag; expose Settings UI when the decision implies discoverability.",
            "Reexec/reload shell (and compositor if needed); add/adjust unit tests where logic is pure.",
            f"VT1 verify acceptance for A{n:03d} ({title}); capture evidence under `mde/fidelity/evidence/` if chrome changed.",
        ]
    )
    return steps


def main() -> None:
    answers = json.loads(ANSWERS.read_text())
    responses = answers.get("responses") or {}
    bank = parse_bank(BANK.read_text())
    missing = [i for i in range(1, 101) if f"{i:03d}" not in responses]
    if missing:
        print(f"Incomplete: {len(missing)} unanswered (next={missing[0]:03d})")
        raise SystemExit(2)

    rows = []
    for i in range(1, 101):
        key = f"{i:03d}"
        resp = responses[key]
        choice = str(resp["choice"])
        note = resp.get("note") or ""
        item = bank[i]
        label = choice_label(item, choice)
        pri = priority(choice, label, note)
        rows.append(
            {
                "id": f"A{i:03d}",
                "n": i,
                "pri": pri,
                "title": item["title"],
                "choice": choice.upper(),
                "label": label,
                "note": note,
                "community": item["community"],
                "today": item["today"],
                "suggestion": item["suggestion"],
            }
        )

    counts = Counter(r["pri"] for r in rows)
    do_rows = [r for r in rows if r["pri"] not in {"skip"}]
    skip_rows = [r for r in rows if r["pri"] == "skip"]

    lines: list[str] = [
        "# Worklist from MDE × Win11 community audit",
        "",
        "Generated from 100 MCQ decisions (goal: audit MDE GUI vs community Win11 recommendations).",
        "",
        f"**Totals:** {len(rows)} items — "
        + ", ".join(f"{k}={v}" for k, v in sorted(counts.items())),
        "",
        "## Priority summary",
        "",
        "| Priority | Count | Meaning |",
        "|----------|------:|---------|",
        f"| P1 | {counts.get('P1', 0)} | Primary build / high community parity |",
        f"| P2 | {counts.get('P2', 0)} | Secondary / polish / alternate path |",
        f"| P3 | {counts.get('P3', 0)} | Lower urgency |",
        f"| deferred | {counts.get('deferred', 0)} | Explicitly deferred |",
        f"| skip | {counts.get('skip', 0)} | Won't do (recorded for audit trail) |",
        "",
        "## Index (doable items first)",
        "",
        "| ID | Pri | Decision | Item |",
        "|----|-----|----------|------|",
    ]
    for r in sorted(do_rows, key=lambda x: (x["pri"], x["n"])):
        lines.append(f"| {r['id']} | {r['pri']} | {r['choice']} | {r['title']} |")
    if skip_rows:
        lines.append("")
        lines.append("### Skipped (audit trail only)")
        lines.append("")
        lines.append("| ID | Decision | Item |")
        lines.append("|----|----------|------|")
        for r in skip_rows:
            lines.append(f"| {r['id']} | {r['choice']} | {r['title']} |")

    lines.extend(["", "---", "", "## Detailed work items", ""])

    for r in rows:
        lines.append(f"### {r['id']} — {r['title']}")
        lines.append("")
        lines.append(f"- **Priority:** `{r['pri']}`")
        lines.append(f"- **Decision:** `{r['choice']}` — {r['label']}")
        if r["note"]:
            lines.append(f"- **User note:** {r['note']}")
        lines.append(f"- **Community driver:** {r['community']}")
        lines.append(f"- **MDE today:** {r['today']}")
        lines.append(f"- **Intent:** {r['suggestion']}")
        lines.append("")
        if r["pri"] == "skip":
            lines.append("**Status:** skip — no implementation work. Kept for audit completeness.")
            lines.append("")
            continue
        lines.append("**Acceptance criteria:**")
        lines.append(
            f"1. Behavior matches decision `{r['choice']}` ({r['label']}) on a live MDE seat."
        )
        lines.append(
            "2. Setting/config is discoverable where the decision implies user control "
            "(Settings page and/or profile preset), not only a hidden TOML key."
        )
        lines.append(
            "3. No regression to adjacent chrome (taskbar/Start/QS/NC/lock) unless intentional."
        )
        lines.append("")
        lines.append("**Work steps:**")
        for i, step in enumerate(
            work_steps(r["n"], r["title"], r["choice"], r["label"], r["today"], r["note"]),
            1,
        ):
            lines.append(f"{i}. {step}")
        lines.append("")

    # Notable combined / special decisions
    lines.extend(
        [
            "---",
            "",
            "## Notable special decisions",
            "",
            "- **A037 = A+C:** QS Bluetooth connect/disconnect for known devices **and** full pair wizard in QS.",
            "- **A044 = B:** Deep-integrate Thunar as “MDE Explorer” (not a greenfield W8 Explorer epic).",
            "- **A075 = B:** Thunar already has bulk rename — rely on FM tools.",
            "- **A081 = C:** Ctrl+Shift+Esc → MMC Performance; **polish and expand MMC**.",
            "- **A006 = C:** Bottom taskbar only forever (no top/side edges).",
            "- **A029 = C / A047 = C:** Leave taskbar peek deferred; keep title/icon Task View (no live thumbs push).",
            "",
        ]
    )

    text_out = "\n".join(lines)
    OUT.write_text(text_out)
    OUT_CRATES.write_text(text_out)
    print(f"Wrote {OUT}")
    print(f"Wrote {OUT_CRATES}")
    print(f"doable={len(do_rows)} skip={len(skip_rows)}")


if __name__ == "__main__":
    main()
