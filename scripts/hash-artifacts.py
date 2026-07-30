#!/usr/bin/env python3
"""Write portable SHA-256 sums for release bundle files."""

from __future__ import annotations

import hashlib
import os
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TARGET_ROOT = Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target"))
if not TARGET_ROOT.is_absolute():
    TARGET_ROOT = ROOT / TARGET_ROOT
BUNDLE_ROOT = TARGET_ROOT / "release/bundle"
OUTPUT = BUNDLE_ROOT / "SHA256SUMS"

if not BUNDLE_ROOT.is_dir():
    print(f"bundle directory does not exist: {BUNDLE_ROOT}", file=sys.stderr)
    raise SystemExit(1)

artifacts = sorted(
    path
    for path in BUNDLE_ROOT.rglob("*")
    if path.is_file()
    and path != OUTPUT
    and ".dSYM" not in path.parts
    and not path.name.endswith(".sig")
)
if not artifacts:
    print("release bundle contains no files", file=sys.stderr)
    raise SystemExit(1)

lines = []
for artifact in artifacts:
    digest = hashlib.sha256()
    with artifact.open("rb") as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(block)
    relative = artifact.relative_to(BUNDLE_ROOT).as_posix()
    lines.append(f"{digest.hexdigest()}  {relative}")

with OUTPUT.open("w", encoding="utf-8", newline="\n") as handle:
    handle.write("\n".join(lines) + "\n")
print(f"wrote {OUTPUT} for {len(artifacts)} artifact(s)")
