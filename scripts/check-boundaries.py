#!/usr/bin/env python3
"""Verify the internal dependency direction documented in ADR 0002."""

import json
import pathlib
import subprocess
import sys


ROOT = pathlib.Path(__file__).resolve().parent.parent
EXPECTED_INTERNAL_DEPENDENCIES = {
    "meiki-domain": set(),
    "meiki-text": {"meiki-domain"},
    "meiki-scheduler": {"meiki-domain"},
    "meiki-storage": {"meiki-domain"},
    "meiki-media": set(),
    "meiki-portable": set(),
    "meiki-application": {
        "meiki-domain",
        "meiki-text",
        "meiki-scheduler",
        "meiki-storage",
    },
    "meiki-desktop": {"meiki-application"},
}
TEXT_ENGINE_DEPENDENCIES = {
    "unicode-general-category",
    "unicode-normalization",
    "unicode-segmentation",
}


def cargo_metadata() -> dict:
    result = subprocess.run(
        ["cargo", "metadata", "--format-version", "1"],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    return json.loads(result.stdout)


def main() -> int:
    metadata = cargo_metadata()
    internal_names = set(EXPECTED_INTERNAL_DEPENDENCIES)
    packages = {
        package["name"]: package
        for package in metadata["packages"]
        if package["name"] in internal_names
    }

    failures = []
    for name, expected in EXPECTED_INTERNAL_DEPENDENCIES.items():
        package = packages.get(name)
        if package is None:
            failures.append(f"{name}: workspace package is missing")
            continue
        actual = {
            dependency["name"]
            for dependency in package["dependencies"]
            if dependency["name"] in internal_names
        }
        if actual != expected:
            failures.append(
                f"{name}: expected internal dependencies {sorted(expected)}, "
                f"found {sorted(actual)}"
            )

    for name, package in packages.items():
        if name == "meiki-text":
            continue
        owned_dependencies = {
            dependency["name"]
            for dependency in package["dependencies"]
            if dependency["name"] in TEXT_ENGINE_DEPENDENCIES
        }
        if owned_dependencies:
            failures.append(
                f"{name}: text-engine dependencies belong in meiki-text, "
                f"found {sorted(owned_dependencies)}"
            )

    storage_root = ROOT / "crates" / "meiki-storage"
    for rust_file in ROOT.rglob("*.rs"):
        if storage_root in rust_file.parents or ".venv" in rust_file.parts:
            continue
        text = rust_file.read_text(encoding="utf-8")
        if "rusqlite" in text:
            failures.append(f"{rust_file.relative_to(ROOT)}: imports rusqlite outside storage")

    component_root = ROOT / "apps" / "desktop" / "src" / "lib" / "components"
    forbidden_component_imports = (
        "@tauri-apps",
        "../api",
        "/generated/",
        "meiki-scheduler",
        "meiki-storage",
    )
    for component_file in component_root.glob("*.svelte"):
        text = component_file.read_text(encoding="utf-8")
        for forbidden in forbidden_component_imports:
            if forbidden in text:
                failures.append(
                    f"{component_file.relative_to(ROOT)}: visual component "
                    f"contains forbidden dependency {forbidden!r}"
                )

    if failures:
        print("\n".join(failures), file=sys.stderr)
        return 1

    print("Module dependency boundaries are valid.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
