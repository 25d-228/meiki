#!/usr/bin/env python3
"""Regression tests for the release metadata validator."""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CHECKER = ROOT / "scripts/check-release.py"


class ReleaseCheckTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.checkout = Path(self.temporary_directory.name) / "meiki"
        tracked = subprocess.run(
            [
                "git",
                "ls-files",
                "--cached",
                "--others",
                "--exclude-standard",
            ],
            cwd=ROOT,
            check=True,
            capture_output=True,
            text=True,
        ).stdout.splitlines()
        for relative in tracked:
            source = ROOT / relative
            if not source.is_file():
                continue
            destination = self.checkout / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source, destination)

    def tearDown(self) -> None:
        self.temporary_directory.cleanup()

    def run_check(self) -> subprocess.CompletedProcess[str]:
        environment = os.environ.copy()
        environment["MEIKI_RELEASE_ROOT"] = str(self.checkout)
        return subprocess.run(
            [sys.executable, str(CHECKER)],
            cwd=self.checkout,
            env=environment,
            capture_output=True,
            text=True,
            check=False,
        )

    def test_current_release_metadata_passes(self) -> None:
        result = self.run_check()
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_package_version_disagreement_fails(self) -> None:
        manifest_path = self.checkout / "apps/desktop/package.json"
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        manifest["version"] = "0.2.1"
        manifest_path.write_text(
            json.dumps(manifest, indent=2) + "\n",
            encoding="utf-8",
        )

        result = self.run_check()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("release versions differ", result.stderr)

    def test_wrong_active_documentation_version_fails(self) -> None:
        guide_path = self.checkout / "docs/release-quality.md"
        guide = guide_path.read_text(encoding="utf-8").replace(
            "release: **0.2.0**",
            "release: **0.1.0**",
            1,
        )
        guide_path.write_text(guide, encoding="utf-8")

        result = self.run_check()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("identifies the wrong active version", result.stderr)

    def test_obsolete_accessibility_gate_fails(self) -> None:
        guide_path = self.checkout / "docs/release-quality.md"
        with guide_path.open("a", encoding="utf-8") as guide:
            guide.write(
                "\nVoiceOver or NVDA is a mandatory release gate tracked by #43.\n"
            )

        result = self.run_check()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("obsolete release-gate reference", result.stderr)


if __name__ == "__main__":
    unittest.main()
