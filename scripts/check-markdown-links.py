#!/usr/bin/env python3
"""Check file targets of relative Markdown links in public documentation."""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path
from urllib.parse import unquote


LINK = re.compile(r"!?\[[^\]]*\]\(([^)\n]+)\)")
SCHEME = re.compile(r"^[A-Za-z][A-Za-z0-9+.-]*:")


def tracked_markdown(repository: Path) -> list[Path]:
    result = subprocess.run(
        [
            "git",
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
            "--",
            "*.md",
        ],
        cwd=repository,
        check=True,
        capture_output=True,
    )
    return [repository / Path(item.decode()) for item in result.stdout.split(b"\0") if item]


def link_path(raw_target: str) -> str | None:
    target = raw_target.strip()
    if target.startswith("<") and target.endswith(">"):
        target = target[1:-1]
    elif " " in target:
        target = target.split(" ", 1)[0]
    if not target or target.startswith("#") or SCHEME.match(target):
        return None
    return unquote(target.split("#", 1)[0].split("?", 1)[0])


def main() -> int:
    repository = Path(
        subprocess.run(
            ["git", "rev-parse", "--show-toplevel"],
            check=True,
            capture_output=True,
            text=True,
        ).stdout.strip()
    ).resolve()
    failures: list[str] = []
    for document in tracked_markdown(repository):
        text = document.read_text(encoding="utf-8")
        for line_number, line in enumerate(text.splitlines(), start=1):
            for match in LINK.finditer(line):
                relative = link_path(match.group(1))
                if relative is None:
                    continue
                candidate = (document.parent / relative).resolve()
                try:
                    candidate.relative_to(repository)
                except ValueError:
                    failures.append(
                        f"{document.relative_to(repository)}:{line_number}: "
                        f"link escapes repository: {relative}"
                    )
                    continue
                if not candidate.exists():
                    failures.append(
                        f"{document.relative_to(repository)}:{line_number}: "
                        f"missing link target: {relative}"
                    )
    if failures:
        print("\n".join(failures), file=sys.stderr)
        return 1
    print("Tracked Markdown link targets are valid")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
