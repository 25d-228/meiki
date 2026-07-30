#!/usr/bin/env python3
"""Validate versioned release metadata without third-party dependencies."""

from __future__ import annotations

import json
import os
import re
import sqlite3
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def fail(message: str) -> None:
    print(f"release check failed: {message}", file=sys.stderr)
    raise SystemExit(1)


def read_json(path: Path) -> dict[str, object]:
    return json.loads(path.read_text(encoding="utf-8"))


workspace_manifest = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
version_match = re.search(
    r"^\[workspace\.package\]\s.*?^version = \"([^\"]+)\"",
    workspace_manifest,
    flags=re.MULTILINE | re.DOTALL,
)
if version_match is None:
    fail("Cargo workspace version is missing")
version = version_match.group(1)

root_package = read_json(ROOT / "package.json")
desktop_package = read_json(ROOT / "apps/desktop/package.json")
tauri_config = read_json(ROOT / "apps/desktop/src-tauri/tauri.conf.json")
versions = {
    "Cargo workspace": version,
    "root npm workspace": root_package.get("version"),
    "desktop npm package": desktop_package.get("version"),
    "Tauri bundle": tauri_config.get("version"),
}
if set(versions.values()) != {version}:
    fail(f"release versions differ: {versions}")

if os.environ.get("GITHUB_REF_TYPE") == "tag":
    tag = os.environ.get("GITHUB_REF_NAME")
    if tag != f"v{version}":
        fail(f"tag {tag!r} does not match release version v{version}")

bundle = tauri_config.get("bundle")
if not isinstance(bundle, dict) or bundle.get("active") is not True:
    fail("Tauri bundle must be active")
if bundle.get("targets") != "all":
    fail("Tauri must declare all platform bundle targets")

icons = bundle.get("icon")
if not isinstance(icons, list) or not icons:
    fail("Tauri bundle icons are missing")
tauri_root = ROOT / "apps/desktop/src-tauri"
for relative in icons:
    if not isinstance(relative, str):
        fail("Tauri icon path is not text")
    icon = tauri_root / relative
    if not icon.is_file() or icon.stat().st_size < 128:
        fail(f"bundle icon is missing or invalid: {relative}")

migrations = sorted((ROOT / "crates/meiki-storage/migrations").glob("*.sql"))
migration_versions = [int(path.name.split("_", 1)[0]) for path in migrations]
expected_versions = list(range(1, len(migrations) + 1))
if migration_versions != expected_versions:
    fail(f"migration versions are not contiguous: {migration_versions}")

storage_source = (ROOT / "crates/meiki-storage/src/lib.rs").read_text(
    encoding="utf-8"
)
latest_match = re.search(r"const LATEST_SCHEMA_VERSION: u32 = (\d+);", storage_source)
if latest_match is None or int(latest_match.group(1)) != migration_versions[-1]:
    fail("LATEST_SCHEMA_VERSION does not match the newest migration")

released_schema = ROOT / "crates/meiki-storage/fixtures/released/v0.1-schema-7.db"
if not released_schema.is_file():
    fail("released v0.1 database fixture is missing")
with sqlite3.connect(f"file:{released_schema}?mode=ro", uri=True) as connection:
    released_version = connection.execute(
        "SELECT COALESCE(MAX(version), 0) FROM schema_migrations"
    ).fetchone()[0]
if released_version != 7:
    fail(f"released v0.1 database fixture has schema {released_version}, expected 7")

portable_source = (ROOT / "crates/meiki-portable/src/lib.rs").read_text(
    encoding="utf-8"
)
if "pub const ARCHIVE_VERSION: u32 = 3;" not in portable_source:
    fail("current .meiki archive version 3 is not declared")
if "const PREVIOUS_ARCHIVE_VERSION: u32 = 2;" not in portable_source:
    fail("published .meiki archive version 2 import support is not declared")
if "const LEGACY_ARCHIVE_VERSION: u32 = 1;" not in portable_source:
    fail("published .meiki archive version 1 import support is not declared")

for relative in [
    "README.md",
    "CONTRIBUTING.md",
    "docs/release-quality.md",
    "docs/user-guide.md",
]:
    if not (ROOT / relative).is_file():
        fail(f"required release documentation is missing: {relative}")

print(
    "release metadata valid: "
    f"version={version} schema={migration_versions[-1]} archive=3"
)
