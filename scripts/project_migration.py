#!/usr/bin/env python3
"""Project Migration Assistant for Portalis template.

Usage:
    ./scripts/project_migration.py --slug my_app [--app-title "My App"]

This updates high-level metadata (docs, CI defaults, Flutter package name)
so the template better reflects the target application. Remaining platform-
specific identifiers are surfaced at the end for manual follow-up.
"""
from __future__ import annotations

import argparse
import re
import subprocess
from dataclasses import dataclass
from pathlib import Path
from typing import Callable, Iterable

ROOT = Path(__file__).resolve().parents[1]


@dataclass
class Context:
    slug: str
    app_title: str
    pascal: str


WordPattern = re.compile(r"\bPortalis\b")


def slug_to_title(slug: str) -> str:
    parts = re.split(r"[-_]+", slug)
    return " ".join(part.capitalize() for part in parts if part)


def slug_to_pascal(slug: str) -> str:
    parts = re.split(r"[-_]+", slug)
    return "".join(part.capitalize() for part in parts if part)


def replace_portalis_words(text: str, title: str) -> str:
    # Keep backtick version first to avoid breaking markdown code fences.
    text = text.replace("`Portalis`", f"`{title}`")
    return WordPattern.sub(title, text)


def update_file(path: Path, transform: Callable[[str], str], ctx: Context, changed: list[Path]) -> None:
    data = path.read_text(encoding="utf-8")
    new_data = transform(data)
    if new_data != data:
        path.write_text(new_data, encoding="utf-8")
        changed.append(path)


def update_readme(path: Path, ctx: Context, changed: list[Path]) -> None:
    def transform(text: str) -> str:
        text = text.replace('title="Portalis"', f'title="{ctx.app_title}"')
        text = replace_portalis_words(text, ctx.app_title)
        return text

    update_file(path, transform, ctx, changed)


def update_doc(path: Path, ctx: Context, changed: list[Path]) -> None:
    def transform(text: str) -> str:
        text = replace_portalis_words(text, ctx.app_title)
        text = text.replace("`portalis/`", f"`{ctx.slug}/`")
        text = text.replace("portalis.exe", f"{ctx.slug}.exe")
        return text

    update_file(path, transform, ctx, changed)


def update_tests_scripts(ctx: Context, changed: list[Path]) -> None:
    for path in (ROOT / "tests").iterdir():
        if path.suffix != ".sh":
            continue

        def transform(text: str) -> str:
            text = text.replace('"$PROJECT_ROOT/portalis"', f'"$PROJECT_ROOT/{ctx.slug}"')
            text = text.replace('"$ROOT_DIR/portalis"', f'"$ROOT_DIR/{ctx.slug}"')
            text = replace_portalis_words(text, ctx.app_title)
            return text

        update_file(path, transform, ctx, changed)


def update_ci_files(ctx: Context, changed: list[Path]) -> None:
    pipeline = ROOT / ".github/workflows/pipeline.yml"

    def pipeline_transform(text: str) -> str:
        text = text.replace("WORKING_DIR: portalis", f"WORKING_DIR: {ctx.slug}")
        return text

    update_file(pipeline, pipeline_transform, ctx, changed)

    action_dir = ROOT / ".github/actions"
    for action in action_dir.glob("*/action.yml"):
        def transform(text: str) -> str:
            text = text.replace("default: portalis", f"default: {ctx.slug}")
            return text

        update_file(action, transform, ctx, changed)


def update_flutter_package(ctx: Context, changed: list[Path]) -> None:
    pubspec = ROOT / "portalis" / "pubspec.yaml"

    def transform_pubspec(text: str) -> str:
        return re.sub(r"^name:\s*portalis\b", f"name: {ctx.slug}", text, flags=re.MULTILINE)

    update_file(pubspec, transform_pubspec, ctx, changed)

    main_dart = ROOT / "portalis" / "lib" / "main.dart"

    def transform_main(text: str) -> str:
        text = text.replace("'package:portalis/", f"'package:{ctx.slug}/")
        text = replace_portalis_words(text, ctx.app_title)
        return text

    update_file(main_dart, transform_main, ctx, changed)

    widget_test = ROOT / "portalis" / "test" / "widget_test.dart"

    def transform_test(text: str) -> str:
        text = text.replace("import 'package:portalis/main.dart';", f"import 'package:{ctx.slug}/main.dart';")
        text = replace_portalis_words(text, ctx.app_title)
        return text

    update_file(widget_test, transform_test, ctx, changed)

    info_plist = ROOT / "portalis" / "ios" / "Runner" / "Info.plist"

    def transform_plist(text: str) -> str:
        return text.replace("<string>Portalis</string>", f"<string>{ctx.app_title}</string>")

    update_file(info_plist, transform_plist, ctx, changed)

    web_index = ROOT / "portalis" / "web" / "index.html"

    def transform_web_index(text: str) -> str:
        text = text.replace('content="portalis"', f'content="{ctx.app_title}"')
        text = text.replace("<title>portalis</title>", f"<title>{ctx.app_title}</title>")
        return text

    update_file(web_index, transform_web_index, ctx, changed)

    web_manifest = ROOT / "portalis" / "web" / "manifest.json"

    def transform_manifest(text: str) -> str:
        text = text.replace('"name": "portalis"', f'"name": "{ctx.app_title}"')
        text = text.replace('"short_name": "portalis"', f'"short_name": "{ctx.app_title}"')
        return text

    update_file(web_manifest, transform_manifest, ctx, changed)


def find_remaining_markers(patterns: Iterable[str]) -> dict[str, list[str]]:
    results: dict[str, list[str]] = {}
    for pattern in patterns:
        try:
            completed = subprocess.run(
                ["rg", pattern],
                cwd=ROOT,
                stdout=subprocess.PIPE,
                stderr=subprocess.DEVNULL,
                check=False,
                text=True,
            )
        except FileNotFoundError:
            return {}
        if completed.returncode == 0 and completed.stdout:
            results[pattern] = completed.stdout.strip().splitlines()
    return results


def main() -> None:
    parser = argparse.ArgumentParser(description="Migrate the Portalis template metadata to a new project name.")
    parser.add_argument("--slug", required=True, help="Flutter package / directory slug (e.g., my_app)")
    parser.add_argument("--app-title", dest="app_title", help="Display name used in docs and UI (default: title case of slug)")
    args = parser.parse_args()

    slug = args.slug.strip()
    if not re.fullmatch(r"[a-z][a-z0-9_]*", slug):
        raise SystemExit("--slug must start with a letter and contain only lowercase letters, digits, or underscores")

    app_title = args.app_title.strip() if args.app_title else slug_to_title(slug)
    pascal = slug_to_pascal(slug)
    ctx = Context(slug=slug, app_title=app_title, pascal=pascal)

    changed: list[Path] = []

    # Update documentation and messaging.
    update_readme(ROOT / "README.md", ctx, changed)
    update_readme(ROOT / "portalis" / "README.md", ctx, changed)
    for doc_path in [
        ROOT / "doc" / "overview.md",
        ROOT / "doc" / "setup_guide.md",
        ROOT / "doc" / "build.md",
    ]:
        update_doc(doc_path, ctx, changed)

    # Update helper scripts and CI defaults.
    update_tests_scripts(ctx, changed)
    update_ci_files(ctx, changed)

    # Update Flutter package metadata.
    update_flutter_package(ctx, changed)

    # Summarize
    print("\nUpdated files:")
    if changed:
        for path in changed:
            rel = path.relative_to(ROOT)
            print(f"  - {rel}")
    else:
        print("  (no changes were necessary)")

    print("\nNext steps:")
    print("  • Review platform-specific identifiers (Android/iOS/macOS) and update bundle IDs as needed.")
    print("  • Rename the Flutter project directory from 'portalis' to the new slug if desired, and adjust remaining paths.")
    print("  • Run ./tests/all.sh to verify everything compiles under the new name.")

    reminders = find_remaining_markers(["Portalis", "portalis"])
    if reminders:
        print("\nLocations still containing 'Portalis' or 'portalis' for manual review:")
        for pattern, lines in reminders.items():
            print(f"  Pattern '{pattern}':")
            for line in lines[:10]:
                print(f"    {line}")
            if len(lines) > 10:
                print(f"    ... ({len(lines) - 10} more)")


if __name__ == "__main__":
    main()
