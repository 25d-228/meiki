#!/usr/bin/env python3
"""Launch the built desktop executable and verify its empty collection opens."""

from __future__ import annotations

import argparse
import os
import subprocess
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
IDENTIFIER = "jp.meiki.desktop"


def fail(message: str, log_path: Path | None = None) -> None:
    print(f"package launch smoke failed: {message}", file=sys.stderr)
    if log_path is not None and log_path.is_file():
        log = log_path.read_text(encoding="utf-8", errors="replace")
        if log:
            print(log[-8_000:], file=sys.stderr)
    raise SystemExit(1)


def executable_candidates(release_root: Path) -> list[Path]:
    if sys.platform == "darwin":
        return [
            release_root / "bundle/macos/Meiki.app/Contents/MacOS/Meiki",
            release_root / "bundle/macos/Meiki.app/Contents/MacOS/meiki-desktop",
            release_root / "meiki-desktop",
        ]
    if os.name == "nt":
        return [
            release_root / "meiki-desktop.exe",
            release_root / "Meiki.exe",
        ]
    return [
        release_root / "meiki-desktop",
        release_root / "Meiki",
    ]


def app_data_roots(smoke_root: Path, environment: dict[str, str]) -> list[Path]:
    data_root = smoke_root / IDENTIFIER
    environment["MEIKI_DATA_DIR"] = str(data_root)
    return [data_root]


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--release-root",
        type=Path,
        default=ROOT / "target/release",
        help="Cargo release output containing the packaged executable",
    )
    parser.add_argument("--timeout", type=float, default=10.0)
    arguments = parser.parse_args()

    release_root = arguments.release_root.resolve()
    executable = next(
        (path for path in executable_candidates(release_root) if path.is_file()),
        None,
    )
    if executable is None:
        expected = ", ".join(
            str(path) for path in executable_candidates(release_root)
        )
        fail(f"desktop executable not found; checked {expected}")

    with tempfile.TemporaryDirectory(prefix="meiki-package-smoke-") as temporary:
        smoke_root = Path(temporary)
        environment = os.environ.copy()
        data_roots = app_data_roots(smoke_root, environment)
        log_path = smoke_root / "launch.log"
        with log_path.open("w", encoding="utf-8") as log:
            process = subprocess.Popen(
                [str(executable)],
                cwd=executable.parent,
                env=environment,
                stdout=log,
                stderr=subprocess.STDOUT,
                text=True,
            )
            deadline = time.monotonic() + arguments.timeout
            collection = None
            while time.monotonic() < deadline:
                exit_code = process.poll()
                if exit_code is not None:
                    fail(
                        f"desktop process exited early with status {exit_code}",
                        log_path,
                    )
                collection = next(
                    (
                        root / "collection.db"
                        for root in data_roots
                        if (root / "collection.db").is_file()
                    ),
                    None,
                )
                if collection is not None:
                    break
                time.sleep(0.2)

            if collection is None:
                process.terminate()
                try:
                    process.wait(timeout=3)
                except subprocess.TimeoutExpired:
                    process.kill()
                fail(
                    "main window did not initialize the empty Today collection",
                    log_path,
                )

            process.terminate()
            try:
                process.wait(timeout=3)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=3)

        print(
            "package launch smoke passed: "
            f"executable={executable} collection={collection}"
        )


if __name__ == "__main__":
    main()
