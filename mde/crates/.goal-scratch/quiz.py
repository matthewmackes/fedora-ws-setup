#!/usr/bin/env python3
"""One-at-a-time MCQ quiz for the MDE × Win11 community audit.

Usage:
  python3 quiz.py              # resume from first unanswered
  python3 quiz.py --from 1     # start at N
  python3 quiz.py --status     # progress
"""
from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent
BANK = ROOT / "win11-community-audit-100.md"
ANSWERS = ROOT / "answers.json"


def load_answers() -> dict:
    if ANSWERS.exists():
        return json.loads(ANSWERS.read_text())
    return {"goal": "MDE × Win11 community audit MCQs", "answered": 0, "total": 100, "responses": {}}


def save_answers(data: dict) -> None:
    data["answered"] = len(data.get("responses") or {})
    data["total"] = 100
    ANSWERS.write_text(json.dumps(data, indent=2) + "\n")


def parse_bank(text: str) -> dict[int, dict]:
    items: dict[int, dict] = {}
    for block in re.split(r"\n(?=## \d{3} — )", text):
        m = re.match(r"## (\d{3}) — (.+)", block)
        if not m:
            continue
        n = int(m.group(1))
        choices = dict(re.findall(r"^- ([A-E])\) (.+)$", block, re.M))
        community = re.search(r"\*\*Community:\*\* (.+)", block)
        today = re.search(r"\*\*MDE today:\*\* (.+)", block)
        suggestion = re.search(r"\*\*Suggestion:\*\* (.+)", block)
        items[n] = {
            "title": m.group(2).strip(),
            "community": community.group(1).strip() if community else "",
            "today": today.group(1).strip() if today else "",
            "suggestion": suggestion.group(1).strip() if suggestion else "",
            "choices": choices,
        }
    return items


def record_md_answer(n: int, choice: str, note: str) -> None:
    text = BANK.read_text()
    key = f"## {n:03d} —"
    start = text.find(key)
    if start < 0:
        return
    next_h = text.find("\n## ", start + 1)
    block = text[start:] if next_h < 0 else text[start:next_h]
    ans_line = f"ANSWER: {choice.upper()}"
    if note:
        ans_line += f" — {note}"
    new_block, count = re.subn(r"^ANSWER:.*$", ans_line, block, count=1, flags=re.M)
    if count:
        text = text[:start] + new_block + (text[start + len(block) :] if next_h >= 0 else "")
        BANK.write_text(text)


def ask_one(n: int, item: dict, data: dict) -> bool:
    print()
    print("=" * 72)
    print(f"Suggestion {n:03d} / 100 — {item['title']}")
    print("=" * 72)
    if item["community"]:
        print(f"Community: {item['community']}")
    if item["today"]:
        print(f"MDE today: {item['today']}")
    if item["suggestion"]:
        print(f"Suggestion: {item['suggestion']}")
    print()
    for letter in "ABCDE":
        if letter in item["choices"]:
            print(f"  {letter}) {item['choices'][letter]}")
    print()
    print("Enter A–E, or: s=skip for later, q=quit & save, ?=reprint")
    while True:
        raw = input(f"[{n:03d}]> ").strip()
        if not raw:
            continue
        low = raw.lower()
        if low in {"q", "quit"}:
            return False
        if low in {"s", "skip"}:
            print(f"Skipped {n:03d}")
            return True
        if low in {"?", "h", "help"}:
            ask_one(n, item, data)
            return True
        choice = raw[0].upper()
        if choice not in "ABCDE":
            print("Need A–E, s, q, or ?")
            continue
        note = raw[1:].strip(" -—:") if len(raw) > 1 else ""
        if choice == "E" and not note:
            note = input("E — specify: ").strip()
        data.setdefault("responses", {})[f"{n:03d}"] = {
            "choice": choice,
            "note": note,
            "title": item["title"],
        }
        save_answers(data)
        record_md_answer(n, choice, note)
        print(f"Recorded {n:03d} = {choice}" + (f" ({note})" if note else ""))
        return True


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--from", dest="start", type=int, default=0)
    ap.add_argument("--status", action="store_true")
    args = ap.parse_args()

    if not BANK.exists():
        print(f"Missing bank: {BANK}", file=sys.stderr)
        return 1

    bank = parse_bank(BANK.read_text())
    if len(bank) != 100:
        print(f"Warning: parsed {len(bank)} items (expected 100)", file=sys.stderr)

    data = load_answers()
    done = set(data.get("responses") or {})

    if args.status:
        print(f"Answered {len(done)}/100")
        for i in range(1, 101):
            k = f"{i:03d}"
            if k in done:
                r = done[k]
                print(f"  {k} {r.get('choice')}  {r.get('title','')}")
            else:
                print(f"  {k} —")
                break
        nxt = next((i for i in range(1, 101) if f"{i:03d}" not in done), None)
        print(f"Next unanswered: {nxt:03d}" if nxt else "All done")
        return 0

    start = args.start or next((i for i in range(1, 101) if f"{i:03d}" not in done), 101)
    if start > 100:
        print("All 100 answered. Run: python3 generate-worklist.py")
        return 0

    print(f"MDE × Win11 audit quiz — starting at {start:03d}")
    print(f"Answers → {ANSWERS}")

    for n in range(start, 101):
        k = f"{n:03d}"
        if k in done:
            continue
        if n not in bank:
            print(f"Missing bank item {n:03d}", file=sys.stderr)
            continue
        if not ask_one(n, bank[n], data):
            print(f"Saved progress: {len(data.get('responses') or {})}/100")
            return 0

    print("All 100 answered.")
    print("Generating worklist…")
    import subprocess

    r = subprocess.run([sys.executable, str(ROOT / "generate-worklist.py")], check=False)
    return r.returncode


if __name__ == "__main__":
    raise SystemExit(main())
