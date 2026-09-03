#!/usr/bin/env python3
"""Enforce repository-wide GitHub Actions supply-chain policy."""

from __future__ import annotations

import re
import sys
from pathlib import Path


PINNED_ACTION = re.compile(r"^\s*-?\s*uses:\s*([^\s#]+)@([0-9a-f]{40})(?:\s+#\s+.+)?$")
USES = re.compile(r"^\s*-?\s*uses:\s*([^\s#]+)@([^\s#]+)")


def main() -> int:
    repository = Path(__file__).resolve().parent.parent
    workflow_directory = repository / ".github" / "workflows"
    failures: list[str] = []
    for workflow in sorted(workflow_directory.glob("*.y*ml")):
        lines = workflow.read_text(encoding="utf-8").splitlines()
        if not any(line.startswith("permissions:") for line in lines):
            failures.append(f"{workflow.name}: missing explicit top-level permissions")
        for line_number, line in enumerate(lines, start=1):
            if "write-all" in line or "read-all" in line:
                failures.append(
                    f"{workflow.name}:{line_number}: broad permission shortcut is forbidden"
                )
            use = USES.match(line)
            if use is None or use.group(1).startswith("./"):
                continue
            if PINNED_ACTION.match(line) is None:
                failures.append(
                    f"{workflow.name}:{line_number}: external action must use a full commit SHA"
                )
    if failures:
        print("\n".join(failures), file=sys.stderr)
        return 1
    print("GitHub Actions are explicitly permissioned and commit-pinned")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
