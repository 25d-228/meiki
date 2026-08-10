#!/usr/bin/env python3
"""Verify the internal dependency direction documented in ADR 0002."""

import json
import pathlib
import re
import subprocess
import sys


ROOT = pathlib.Path(__file__).resolve().parent.parent
EXPECTED_INTERNAL_DEPENDENCIES = {
    "meiki-domain": set(),
    "meiki-text": {"meiki-domain"},
    "meiki-scheduler": {"meiki-domain"},
    "meiki-storage": {"meiki-domain"},
    "meiki-media": set(),
    "meiki-portable": {"meiki-domain"},
    "meiki-application": {
        "meiki-domain",
        "meiki-media",
        "meiki-portable",
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
FORBIDDEN_NETWORK_DEPENDENCIES = {
    "axios",
    "hyper",
    "oauth2",
    "reqwest",
    "surf",
    "tokio-tungstenite",
    "tungstenite",
    "ureq",
    "ws",
}
FORBIDDEN_PRODUCT_MODULES = {
    "account",
    "accounts",
    "auth",
    "cloud",
    "marketplace",
    "marketplaces",
    "plugin",
    "plugins",
    "sync",
}
FORBIDDEN_FRONTEND_NETWORK_APIS = {
    "EventSource(",
    "WebSocket(",
    "XMLHttpRequest(",
    "fetch(",
    "navigator.onLine",
}
REMOTE_MEDIA_SCHEME_LITERAL = re.compile(
    r"""["'`]https?:(?://)?""",
    flags=re.IGNORECASE,
)
REMOVED_DESKTOP_COMMANDS = {
    "add_archive_deck",
    "apply_library_bulk_action",
    "export_archive",
    "export_library_selection",
    "export_scheduler_diagnostics",
    "get_library",
    "import_archive",
    "list_backups",
    "preview_archive",
    "restore_backup",
    "rollback_scheduler",
}
REMOVED_GENERATED_BINDINGS = {
    "ArchiveAddDeckRequest.ts",
    "ArchiveAddDeckResultDto.ts",
    "ArchiveExportRequest.ts",
    "ArchiveImportRequest.ts",
    "ArchiveImportModeDto.ts",
    "ArchiveImportResultDto.ts",
    "ArchiveScopeDto.ts",
    "BackupDto.ts",
    "LibraryBulkActionDto.ts",
    "LibraryBulkRequest.ts",
    "LibraryBulkResultDto.ts",
    "LibraryCardDto.ts",
    "LibraryDeckDto.ts",
    "LibraryDueFilterDto.ts",
    "LibraryExportRequest.ts",
    "LibraryExportResultDto.ts",
    "LibraryMediaFilterDto.ts",
    "LibraryNoteDto.ts",
    "LibraryOverviewDto.ts",
    "LibraryRequest.ts",
    "LibrarySuspendedFilterDto.ts",
    "LibraryTagDto.ts",
    "LibraryTrashFilterDto.ts",
    "PortableArchivePreviewDto.ts",
    "SchedulerDiagnosticsExportDto.ts",
}
REMOVED_SETTINGS_TEXT = {
    "Archives and recovery",
    "Export full collection",
    "Preview an import",
    "Replace collection",
    "Rolling backups",
    "Restore backup",
}
REMOVED_RUNTIME_ROUTES = {
    "archive",
    "backup",
    "library",
    "restore",
}
REMOVED_DESKTOP_COMPONENTS = {
    "Button.svelte",
    "Dialog.svelte",
    "Feedback.svelte",
    "Field.svelte",
    "Menu.svelte",
    "SurfaceCard.svelte",
    "TextInput.svelte",
    "Toolbar.svelte",
}
REQUIRED_SHADCN_COMPONENTS = {
    "alert-dialog",
    "badge",
    "button",
    "card",
    "collapsible",
    "dialog",
    "input",
    "label",
    "select",
    "separator",
    "sheet",
    "switch",
    "textarea",
    "tooltip",
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
        network_dependencies = {
            dependency["name"]
            for dependency in package["dependencies"]
            if dependency["name"] in FORBIDDEN_NETWORK_DEPENDENCIES
        }
        if network_dependencies:
            failures.append(
                f"{name}: network dependencies are outside Meiki's product scope, "
                f"found {sorted(network_dependencies)}"
            )
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

    for removed_component in REMOVED_DESKTOP_COMPONENTS:
        removed_path = component_root / removed_component
        if removed_path.exists():
            failures.append(
                f"{removed_path.relative_to(ROOT)}: removed custom UI component exists"
            )

    shadcn_root = component_root / "ui"
    missing_shadcn_components = sorted(
        component
        for component in REQUIRED_SHADCN_COMPONENTS
        if not (shadcn_root / component / "index.ts").is_file()
    )
    if missing_shadcn_components:
        failures.append(
            "apps/desktop/src/lib/components/ui: required shadcn-svelte "
            f"components are missing: {missing_shadcn_components}"
        )

    desktop_source_root = ROOT / "apps" / "desktop" / "src"
    removed_styles = (
        desktop_source_root / "styles" / "themes.css",
        desktop_source_root / "styles" / "tokens.css",
    )
    for removed_style in removed_styles:
        if removed_style.exists():
            failures.append(
                f"{removed_style.relative_to(ROOT)}: removed custom design token "
                "stylesheet exists"
            )

    legacy_import_markers = tuple(
        f"components/{component}" for component in REMOVED_DESKTOP_COMPONENTS
    )
    for frontend_file in desktop_source_root.rglob("*"):
        if not frontend_file.is_file() or frontend_file.suffix not in {
            ".css",
            ".svelte",
            ".ts",
        }:
            continue
        text = frontend_file.read_text(encoding="utf-8")
        for legacy_import in legacy_import_markers:
            if legacy_import in text:
                failures.append(
                    f"{frontend_file.relative_to(ROOT)}: imports removed custom UI "
                    f"component {legacy_import!r}"
                )
        if "window.confirm(" in text:
            failures.append(
                f"{frontend_file.relative_to(ROOT)}: browser confirmation must use "
                "the in-app AlertDialog"
            )

    components_config_path = ROOT / "apps" / "desktop" / "components.json"
    if not components_config_path.is_file():
        failures.append("apps/desktop/components.json: shadcn-svelte config is missing")
    else:
        components_config = json.loads(
            components_config_path.read_text(encoding="utf-8")
        )
        expected_aliases = {
            "components": "$lib/components",
            "utils": "$lib/utils",
            "ui": "$lib/components/ui",
            "hooks": "$lib/hooks",
            "lib": "$lib",
        }
        if components_config.get("aliases") != expected_aliases:
            failures.append(
                "apps/desktop/components.json: expected standard shadcn-svelte "
                f"aliases {expected_aliases}"
            )

    package_json = json.loads(
        (ROOT / "apps" / "desktop" / "package.json").read_text(encoding="utf-8")
    )
    frontend_dependencies = set(package_json.get("dependencies", {}))
    forbidden_frontend_dependencies = (
        frontend_dependencies & FORBIDDEN_NETWORK_DEPENDENCIES
    )
    if forbidden_frontend_dependencies:
        failures.append(
            "apps/desktop/package.json: network dependencies are outside Meiki's "
            f"product scope, found {sorted(forbidden_frontend_dependencies)}"
        )

    capability_path = (
        ROOT / "apps" / "desktop" / "src-tauri" / "capabilities" / "default.json"
    )
    capability = json.loads(capability_path.read_text(encoding="utf-8"))
    network_permissions = [
        permission
        for permission in capability.get("permissions", [])
        if any(
            marker in json.dumps(permission).lower()
            for marker in ("http", "network", "websocket")
        )
    ]
    if network_permissions:
        failures.append(
            f"{capability_path.relative_to(ROOT)}: network permissions are "
            f"outside Meiki's product scope, found {network_permissions}"
        )

    tauri_config_path = (
        ROOT / "apps" / "desktop" / "src-tauri" / "tauri.conf.json"
    )
    tauri_config = json.loads(tauri_config_path.read_text(encoding="utf-8"))
    security = tauri_config.get("app", {}).get("security", {})
    asset_protocol = security.get("assetProtocol", {})
    if asset_protocol != {"enable": True, "scope": ["$APPDATA/**"]}:
        failures.append(
            f"{tauri_config_path.relative_to(ROOT)}: asset protocol must be "
            "limited to application-managed data"
        )

    csp = security.get("csp")
    directives = {}
    if isinstance(csp, str):
        for declaration in csp.split(";"):
            parts = declaration.strip().split()
            if parts:
                directives[parts[0]] = set(parts[1:])
    managed_asset_sources = {"'self'", "asset:", "http://asset.localhost"}
    if directives.get("img-src") != managed_asset_sources:
        failures.append(
            f"{tauri_config_path.relative_to(ROOT)}: img-src must "
            "allow only packaged content and the managed asset protocol"
        )
    managed_audio_sources = managed_asset_sources | {
        "meiki-media:",
        "http://meiki-media.localhost",
    }
    if directives.get("media-src") != managed_audio_sources:
        failures.append(
            f"{tauri_config_path.relative_to(ROOT)}: media-src must "
            "allow only packaged content and registered managed-media protocols"
        )

    production_roots = [ROOT / "apps" / "desktop" / "src", ROOT / "crates"]
    for production_root in production_roots:
        for source_file in production_root.rglob("*"):
            if not source_file.is_file() or source_file.suffix not in {
                ".rs",
                ".svelte",
                ".ts",
            }:
                continue
            relative_path = source_file.relative_to(ROOT)
            module_parts = {
                part.removesuffix(source_file.suffix).lower()
                for part in relative_path.parts
            }
            forbidden_modules = module_parts & FORBIDDEN_PRODUCT_MODULES
            if forbidden_modules:
                failures.append(
                    f"{relative_path}: product module is outside Meiki's scope, "
                    f"found {sorted(forbidden_modules)}"
                )
            if source_file.suffix in {".svelte", ".ts"}:
                text = source_file.read_text(encoding="utf-8")
                for network_api in FORBIDDEN_FRONTEND_NETWORK_APIS:
                    if network_api in text:
                        failures.append(
                            f"{relative_path}: production network API "
                            f"{network_api!r} is outside Meiki's scope"
                        )
                if REMOTE_MEDIA_SCHEME_LITERAL.search(text):
                    failures.append(
                        f"{relative_path}: remote HTTP(S) media schemes are "
                        "outside Meiki's product scope"
                    )

    command_surfaces = [
        ROOT / "apps" / "desktop" / "src-tauri" / "src" / "lib.rs",
        ROOT / "apps" / "desktop" / "src" / "lib" / "api.ts",
    ]
    for command_surface in command_surfaces:
        text = command_surface.read_text(encoding="utf-8")
        for command in REMOVED_DESKTOP_COMMANDS:
            if command in text:
                failures.append(
                    f"{command_surface.relative_to(ROOT)}: removed desktop "
                    f"command {command!r} is public again"
                )

    generated_root = ROOT / "apps" / "desktop" / "src" / "lib" / "generated"
    for binding in REMOVED_GENERATED_BINDINGS:
        if (generated_root / binding).exists():
            failures.append(
                f"{(generated_root / binding).relative_to(ROOT)}: removed "
                "frontend binding exists"
            )

    settings_path = desktop_source_root / "screens" / "SettingsScreen.svelte"
    settings_text = settings_path.read_text(encoding="utf-8")
    for marker in REMOVED_SETTINGS_TEXT:
        if marker in settings_text:
            failures.append(
                f"{settings_path.relative_to(ROOT)}: removed Settings text "
                f"{marker!r} is visible again"
            )

    routes_path = desktop_source_root / "lib" / "ui.ts"
    routes = routes_path.read_text(encoding="utf-8")
    for route in REMOVED_RUNTIME_ROUTES:
        if re.search(rf'["\']{re.escape(route)}["\']', routes):
            failures.append(
                f"{routes_path.relative_to(ROOT)}: obsolete runtime route "
                f"{route!r} exists"
            )

    if failures:
        print("\n".join(failures), file=sys.stderr)
        return 1

    print("Module dependency boundaries are valid.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
