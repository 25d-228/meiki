#!/usr/bin/env python3
"""Validate versioned release metadata without third-party dependencies."""

from __future__ import annotations

import json
import os
import re
import sqlite3
import sys
from pathlib import Path

DEFAULT_ROOT = Path(__file__).resolve().parent.parent
ROOT = Path(os.environ.get("MEIKI_RELEASE_ROOT", DEFAULT_ROOT)).resolve()


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
if re.fullmatch(r"\d+\.\d+\.\d+", version) is None:
    fail(f"release version is not stable semantic versioning: {version!r}")

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

package_lock = read_json(ROOT / "package-lock.json")
locked_packages = package_lock.get("packages")
if not isinstance(locked_packages, dict):
    fail("package-lock.json does not contain workspace packages")
package_lock_versions = {
    "npm lock root": package_lock.get("version"),
    "npm lock root package": locked_packages.get("", {}).get("version"),
    "npm lock desktop package": locked_packages.get("apps/desktop", {}).get(
        "version"
    ),
}
if set(package_lock_versions.values()) != {version}:
    fail(f"npm lockfile versions differ: {package_lock_versions}")

desktop_dependencies = {
    **desktop_package.get("dependencies", {}),
    **desktop_package.get("devDependencies", {}),
}
removed_ui_dependencies = {
    "@fontsource-variable/inter",
    "@lucide/svelte",
}
present_removed_ui_dependencies = removed_ui_dependencies & desktop_dependencies.keys()
if present_removed_ui_dependencies:
    fail(
        "desktop package contains removed UI dependencies: "
        f"{sorted(present_removed_ui_dependencies)}"
    )
required_ui_dependencies = {
    "@fontsource-variable/merriweather",
    "remixicon-svelte",
}
missing_ui_dependencies = required_ui_dependencies - desktop_dependencies.keys()
if missing_ui_dependencies:
    fail(
        "desktop package is missing fixed UI preset dependencies: "
        f"{sorted(missing_ui_dependencies)}"
    )

components_config = read_json(ROOT / "apps/desktop/components.json")
expected_preset_config = {
    "style": "vega",
    "iconLibrary": "remixicon",
    "menuColor": "default",
    "menuAccent": "subtle",
}
actual_preset_config = {
    key: components_config.get(key) for key in expected_preset_config
}
if actual_preset_config != expected_preset_config:
    fail(
        "desktop shadcn preset differs from the fixed preset: "
        f"{actual_preset_config}"
    )
tailwind_config = components_config.get("tailwind")
if not isinstance(tailwind_config, dict) or tailwind_config.get("baseColor") != "neutral":
    fail("desktop shadcn preset must use the neutral base color")

desktop_stylesheet = (ROOT / "apps/desktop/src/app.css").read_text(encoding="utf-8")
radius_match = re.search(
    r"^\s*--radius:\s*([^;]+);", desktop_stylesheet, flags=re.MULTILINE
)
if radius_match is None or radius_match.group(1).strip() not in {"0", "0px", "0rem"}:
    fail("desktop shared radius must be zero")
required_stylesheet_markers = (
    '@import "@fontsource-variable/merriweather";',
    '--font-sans: "Merriweather Variable", serif;',
    '--font-heading: "Merriweather Variable", serif;',
)
for marker in required_stylesheet_markers:
    if marker not in desktop_stylesheet:
        fail(f"desktop stylesheet is missing fixed preset marker {marker!r}")

removed_ui_markers = (
    "@fontsource-variable/inter",
    "@lucide/svelte",
    "Inter Variable",
)
desktop_source_root = ROOT / "apps/desktop/src"
for source_file in desktop_source_root.rglob("*"):
    if not source_file.is_file() or source_file.suffix not in {".css", ".svelte", ".ts"}:
        continue
    source = source_file.read_text(encoding="utf-8")
    for marker in removed_ui_markers:
        if marker in source:
            fail(
                f"{source_file.relative_to(ROOT)} contains removed UI marker {marker!r}"
            )

cargo_lock = (ROOT / "Cargo.lock").read_text(encoding="utf-8")
workspace_lock_versions = {}
for block in cargo_lock.split("[[package]]")[1:]:
    name_match = re.search(r'^name = "([^"]+)"', block, flags=re.MULTILINE)
    locked_version_match = re.search(
        r'^version = "([^"]+)"', block, flags=re.MULTILINE
    )
    if (
        name_match is not None
        and locked_version_match is not None
        and name_match.group(1).startswith("meiki-")
    ):
        workspace_lock_versions[name_match.group(1)] = locked_version_match.group(1)
expected_workspace_packages = {
    "meiki-application",
    "meiki-desktop",
    "meiki-domain",
    "meiki-media",
    "meiki-portable",
    "meiki-scheduler",
    "meiki-storage",
    "meiki-text",
}
if set(workspace_lock_versions) != expected_workspace_packages:
    fail(
        "Cargo.lock workspace packages differ: "
        f"{sorted(workspace_lock_versions)}"
    )
if set(workspace_lock_versions.values()) != {version}:
    fail(f"Cargo.lock workspace versions differ: {workspace_lock_versions}")

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
latest_schema_version = migration_versions[-1]

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
archive_match = re.search(
    r"pub const ARCHIVE_VERSION: u32 = (\d+);", portable_source
)
if archive_match is None:
    fail("current .meiki archive version is not declared")
archive_version = int(archive_match.group(1))
if "const POLICY_ARCHIVE_VERSION: u32 = 3;" not in portable_source:
    fail("published .meiki archive version 3 policy compatibility is not declared")
if "const LEGACY_ARCHIVE_VERSION: u32 = 1;" not in portable_source:
    fail("published .meiki archive version 1 import support is not declared")

release_notes_relative = f"docs/releases/v{version}.md"
required_documentation = [
    "README.md",
    "CONTRIBUTING.md",
    "docs/release-quality.md",
    "docs/testing.md",
    "docs/user-guide.md",
    "docs/data-portability.md",
    release_notes_relative,
]
for relative in required_documentation:
    if not (ROOT / relative).is_file():
        fail(f"required release documentation is missing: {relative}")

current_release_pattern = re.compile(
    rf"Current\s+release:\s+\*\*{re.escape(version)}\*\*",
    flags=re.IGNORECASE,
)
release_notes_link = f"({release_notes_relative})"
for relative in ("README.md", "CONTRIBUTING.md"):
    text = (ROOT / relative).read_text(encoding="utf-8")
    if current_release_pattern.search(text) is None:
        fail(f"{relative} does not identify current release {version}")
    if release_notes_link not in text:
        fail(f"{relative} does not link current release notes")
    if "v0.1 epic" in text.lower():
        fail(f"{relative} still routes contributors through the v0.1 epic")

release_quality = (ROOT / "docs/release-quality.md").read_text(encoding="utf-8")
if current_release_pattern.search(release_quality) is None:
    fail("docs/release-quality.md identifies the wrong active version")

release_notes = (ROOT / release_notes_relative).read_text(encoding="utf-8")
metadata_documents = {
    "docs/release-quality.md": release_quality,
    release_notes_relative: release_notes,
}
schema_pattern = re.compile(
    rf"database schema:?\s+\*\*{latest_schema_version}\*\*",
    flags=re.IGNORECASE,
)
archive_pattern = re.compile(
    rf"`?\.meiki`? archive version:?\s+\*\*{archive_version}\*\*",
    flags=re.IGNORECASE,
)
for relative, text in metadata_documents.items():
    if schema_pattern.search(text) is None:
        fail(
            f"{relative} does not identify database schema "
            f"{latest_schema_version}"
        )
    if archive_pattern.search(text) is None:
        fail(
            f"{relative} does not identify .meiki archive version "
            f"{archive_version}"
        )

obsolete_gate_markers = ("#43", "voiceover", "nvda")
markdown_paths = [
    ROOT / "README.md",
    ROOT / "CONTRIBUTING.md",
    *(ROOT / "docs").rglob("*.md"),
]
for path in markdown_paths:
    text = path.read_text(encoding="utf-8").lower()
    for marker in obsolete_gate_markers:
        if marker in text:
            fail(
                f"{path.relative_to(ROOT)} contains obsolete release-gate "
                f"reference {marker!r}"
            )

print(
    "release metadata valid: "
    f"version={version} schema={latest_schema_version} archive={archive_version}"
)
