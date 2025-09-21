#!/usr/bin/env python3
"""Portalis template migration assistant.

Automates renaming of the Flutter project directory, documentation, CI
configuration, bundle identifiers, and remaining references so the template can
be rebranded safely. Designed to be idempotent—you can rerun it while iterating
on names.
"""
from __future__ import annotations

import argparse
import re
import subprocess
from dataclasses import dataclass
from pathlib import Path
from typing import Callable, Iterable, List, Tuple

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_DIRNAME = "portalis"
WORD_PORTALIS = re.compile(r"\bPortalis\b")
SKIP_DIRS = {".git", ".dart_tool", "build", "DerivedData", "node_modules", ".gradle", "target"}
TEXT_EXTS = {
    ".dart", ".rs", ".yaml", ".yml", ".json", ".md", ".txt", ".xml", ".gradle",
    ".kt", ".kts", ".swift", ".m", ".mm", ".h", ".hpp", ".c", ".cc", ".cpp",
    ".cmake", ".plist", ".xcconfig", ".cfg", ".conf", ".ini", ".ps1", ".sh",
    ".bat", ".toml", ".lock", ".properties", ".pbxproj", ".rc", ".xcscheme"
}
SPECIAL_FILENAMES = {"project.pbxproj"}
SCRIPT_PATH = (ROOT / "scripts" / "project_migration.py").resolve()


def sanitize_bundle_suffix(name: str) -> str:
    suffix = re.sub(r"[^a-z0-9]", "", name.lower())
    return suffix or "app"


@dataclass
class Context:
    slug: str
    app_title: str
    pascal_name: str
    flutter_dir: Path
    current_dir_name: str
    previous_names: set[str]
    bundle_suffix: str
    previous_bundle_suffixes: set[str]


def derive_app_title(slug: str) -> str:
    parts = re.split(r"[-_]+", slug)
    return " ".join(part.capitalize() for part in parts if part) or slug.capitalize()


def derive_pascal(slug: str) -> str:
    parts = re.split(r"[-_]+", slug)
    return "".join(part.capitalize() for part in parts if part) or slug.capitalize()


def replace_portalis_word(text: str, replacement: str) -> str:
    return WORD_PORTALIS.sub(replacement, text)


def record_change(changed: List[str], path: Path | str) -> None:
    rel = path
    if isinstance(path, Path):
        rel = path.relative_to(ROOT).as_posix()
    if rel not in changed:
        changed.append(rel)


def update_file(path: Path, transform: Callable[[str], str], changed: List[str]) -> None:
    if not path.exists():
        return
    original = path.read_text(encoding="utf-8")
    updated = transform(original)
    if original != updated:
        path.write_text(updated, encoding="utf-8")
        record_change(changed, path)


def locate_flutter_dir(slug: str) -> tuple[Path, str]:
    slug_dir = ROOT / slug
    if (slug_dir / "pubspec.yaml").exists():
        return slug_dir, slug

    default_dir = ROOT / DEFAULT_DIRNAME
    if (default_dir / "pubspec.yaml").exists():
        return default_dir, DEFAULT_DIRNAME

    candidates = [
        path for path in sorted(ROOT.iterdir())
        if path.is_dir() and (path / "pubspec.yaml").exists()
    ]
    if candidates:
        candidate = candidates[0]
        return candidate, candidate.name

    raise SystemExit("Could not locate the Flutter project directory (expected 'portalis' or a directory containing pubspec.yaml).")


def update_readmes(ctx: Context, changed: List[str]) -> None:
    def transform_root(text: str) -> str:
        text = text.replace('title="Portalis"', f'title="{ctx.app_title}"')
        text = text.replace("# Portalis", f"# {ctx.app_title}", 1)
        text = replace_portalis_word(text, ctx.app_title)
        return text

    update_file(ROOT / "README.md", transform_root, changed)

    flutter_readme = ctx.flutter_dir / "README.md"

    def transform_flutter(text: str) -> str:
        text = text.replace("# Portalis", f"# {ctx.app_title}", 1)
        text = replace_portalis_word(text, ctx.app_title)
        return text

    update_file(flutter_readme, transform_flutter, changed)


def update_docs(ctx: Context, changed: List[str]) -> None:
    doc_paths = [ROOT / "doc" / name for name in ("overview.md", "setup_guide.md", "build.md")]
    previous_names = ctx.previous_names.copy()

    def transform(text: str) -> str:
        text = replace_portalis_word(text, ctx.app_title)
        for name in previous_names:
            text = text.replace(f"`{name}/`", f"`{ctx.slug}/`")
            text = text.replace(f"{name}.exe", f"{ctx.slug}.exe")
        return text

    for doc in doc_paths:
        update_file(doc, transform, changed)


def update_tests_scripts(ctx: Context, changed: List[str]) -> None:
    tests_dir = ROOT / "tests"
    if not tests_dir.exists():
        return

    names = ctx.previous_names.copy()

    for script in tests_dir.glob("*.sh"):
        def transform(text: str) -> str:
            for name in names:
                text = text.replace(f'"$PROJECT_ROOT/{name}"', f'"$PROJECT_ROOT/{ctx.slug}"')
                text = text.replace(f'"$ROOT_DIR/{name}"', f'"$ROOT_DIR/{ctx.slug}"')
                text = text.replace(f'"$PROJECT_ROOT/../{name}"', f'"$PROJECT_ROOT/../{ctx.slug}"')
            return replace_portalis_word(text, ctx.app_title)

        update_file(script, transform, changed)


def update_ci(ctx: Context, changed: List[str]) -> None:
    workflow = ROOT / ".github" / "workflows" / "pipeline.yml"
    names = ctx.previous_names.copy()

    def transform_workflow(text: str) -> str:
        for name in names:
            text = text.replace(f"WORKING_DIR: {name}", f"WORKING_DIR: {ctx.slug}")
        return replace_portalis_word(text, ctx.app_title)

    update_file(workflow, transform_workflow, changed)

    actions_dir = ROOT / ".github" / "actions"
    if not actions_dir.exists():
        return

    for action in actions_dir.glob("*/action.yml"):
        def transform_action(text: str) -> str:
            for name in names:
                text = text.replace(f"default: {name}", f"default: {ctx.slug}")
            text = text.replace("Portalis-", f"{ctx.pascal_name}-")
            return replace_portalis_word(text, ctx.app_title)

        update_file(action, transform_action, changed)


def update_flutter_package(ctx: Context, changed: List[str]) -> None:
    names = ctx.previous_names.copy()
    pubspec = ctx.flutter_dir / "pubspec.yaml"

    def transform_pubspec(text: str) -> str:
        for name in names:
            text = re.sub(rf"^name:\s*{name}\b", f"name: {ctx.slug}", text, flags=re.MULTILINE)
        return replace_portalis_word(text, ctx.app_title)

    update_file(pubspec, transform_pubspec, changed)

    main_dart = ctx.flutter_dir / "lib" / "main.dart"

    def transform_main(text: str) -> str:
        for name in names:
            text = text.replace(f"package:{name}/", f"package:{ctx.slug}/")
        return replace_portalis_word(text, ctx.app_title)

    update_file(main_dart, transform_main, changed)

    widget_test = ctx.flutter_dir / "test" / "widget_test.dart"

    def transform_test(text: str) -> str:
        for name in names:
            text = text.replace(f"package:{name}/", f"package:{ctx.slug}/")
        return replace_portalis_word(text, ctx.app_title)

    update_file(widget_test, transform_test, changed)

    info_plist = ctx.flutter_dir / "ios" / "Runner" / "Info.plist"
    update_file(info_plist, lambda t: t.replace("<string>Portalis</string>", f"<string>{ctx.app_title}</string>"), changed)

    web_index = ctx.flutter_dir / "web" / "index.html"

    def transform_web(text: str) -> str:
        text = text.replace('content="portalis"', f'content="{ctx.app_title}"')
        text = text.replace('content="Portalis"', f'content="{ctx.app_title}"')
        text = text.replace('<title>portalis</title>', f'<title>{ctx.app_title}</title>')
        text = text.replace('<title>Portalis</title>', f'<title>{ctx.app_title}</title>')
        return text

    update_file(web_index, transform_web, changed)

    web_manifest = ctx.flutter_dir / "web" / "manifest.json"
    update_file(
        web_manifest,
        lambda t: t.replace('"name": "portalis"', f'"name": "{ctx.app_title}"').replace('"short_name": "portalis"', f'"short_name": "{ctx.app_title}"'),
        changed,
    )


def rename_flutter_directory(ctx: Context, changed: List[str]) -> None:
    if ctx.current_dir_name == ctx.slug:
        return

    old_dir = ctx.flutter_dir
    new_dir = ROOT / ctx.slug
    if new_dir.exists():
        raise SystemExit(f"Target directory '{ctx.slug}' already exists. Remove or rename it before running the migration.")

    old_name = ctx.current_dir_name
    old_dir.rename(new_dir)

    for idx, entry in enumerate(changed):
        for name in {DEFAULT_DIRNAME, old_name}:
            if entry.startswith(f"{name}/"):
                changed[idx] = entry.replace(f"{name}/", f"{ctx.slug}/", 1)
    record_change(changed, new_dir)

    ctx.previous_names.add(old_name)
    ctx.previous_bundle_suffixes.add(sanitize_bundle_suffix(old_name))
    ctx.flutter_dir = new_dir
    ctx.current_dir_name = ctx.slug


def update_platform_identifiers(ctx: Context, changed: List[str]) -> None:
    new_bundle = f"com.example.{ctx.bundle_suffix}"
    old_bundles = [f"com.example.{suffix}" for suffix in ctx.previous_bundle_suffixes if suffix != ctx.bundle_suffix]

    replacements: List[Tuple[str, str]] = []
    for old in old_bundles:
        replacements.append((old, new_bundle))
        replacements.append((old + ".RunnerTests", new_bundle + ".RunnerTests"))
        replacements.append((old + ".backend", new_bundle + ".backend"))
    replacements.append(("com.portalis.backend", f"com.{ctx.bundle_suffix}.backend"))

    for base in [ctx.flutter_dir / part for part in ("android", "ios", "macos", "linux", "windows")]:
        if base.exists():
            apply_replacements_in_tree(base, replacements, changed)

    android_dir = ctx.flutter_dir / "android"
    manifest = android_dir / "app/src/main/AndroidManifest.xml"
    update_file(manifest, lambda t: t.replace('android:label="portalis"', f'android:label="{ctx.app_title}"'), changed)

    gradle_file = android_dir / "app/build.gradle"

    def transform_gradle(text: str) -> str:
        for old in old_bundles:
            text = text.replace(f'"{old}"', f'"{new_bundle}"')
        return text

    update_file(gradle_file, transform_gradle, changed)

    def update_package_dirs(root: Path) -> None:
        base = root / "app/src/main"
        for lang in ("kotlin", "java"):
            pkg_base = base / lang / "com" / "example"
            if not pkg_base.exists():
                continue
            new_dir = pkg_base / ctx.bundle_suffix
            if new_dir.exists():
                continue
            for suffix in ctx.previous_bundle_suffixes:
                if suffix == ctx.bundle_suffix:
                    continue
                candidate = pkg_base / suffix
                if candidate.exists():
                    candidate.rename(new_dir)
                    record_change(changed, new_dir)
                    break

    update_package_dirs(android_dir)


def should_process(path: Path) -> bool:
    if any(part in SKIP_DIRS for part in path.relative_to(ROOT).parts):
        return False
    if path.name in SPECIAL_FILENAMES:
        return True
    return path.suffix in TEXT_EXTS


def apply_replacements_in_tree(base: Path, replacements: List[Tuple[str, str]], changed: List[str]) -> None:
    if not replacements:
        return

    for path in base.rglob("*"):
        if path.is_dir():
            continue
        if path.resolve() == SCRIPT_PATH:
            continue
        if not should_process(path):
            continue

        def transform(text: str) -> str:
            for old, new in replacements:
                text = text.replace(old, new)
            return text

        update_file(path, transform, changed)


def apply_global_replacements(ctx: Context, changed: List[str]) -> None:
    names = ctx.previous_names.copy()
    bundles = [f"com.example.{suffix}" for suffix in ctx.previous_bundle_suffixes if suffix != ctx.bundle_suffix]
    replacements: List[Tuple[str, str]] = []

    replacements.append(("Portalis", ctx.app_title))
    replacements.append(("PORTALIS", ctx.app_title.upper()))

    for name in names:
        replacements.append((name, ctx.slug))
        replacements.append((name.upper(), ctx.slug.upper()))

    new_bundle = f"com.example.{ctx.bundle_suffix}"
    for old in bundles:
        replacements.append((old, new_bundle))
        replacements.append((old + ".RunnerTests", new_bundle + ".RunnerTests"))
        replacements.append((old + ".backend", new_bundle + ".backend"))

    replacements.append(("portalis.app", f"{ctx.slug}.app"))

    apply_replacements_in_tree(ROOT, replacements, changed)


def find_remaining_markers(patterns: Iterable[str]) -> dict[str, list[str]]:
    results: dict[str, list[str]] = {}
    for pattern in patterns:
        try:
            proc = subprocess.run(
                ["rg", pattern],
                cwd=ROOT,
                stdout=subprocess.PIPE,
                stderr=subprocess.DEVNULL,
                check=False,
                text=True,
            )
        except FileNotFoundError:
            return {}
        if proc.returncode == 0 and proc.stdout:
            lines = [line for line in proc.stdout.strip().splitlines() if "scripts/project_migration.py" not in line]
            if lines:
                results[pattern] = lines
    return results


def parse_args() -> Context:
    parser = argparse.ArgumentParser(description="Migrate the Portalis template to a new project slug.")
    parser.add_argument("--slug", required=True, help="Flutter package/directory slug (lowercase letters, digits, underscores)")
    parser.add_argument("--app-title", dest="app_title", help="Display name used in docs and UI (defaults to title-cased slug)")
    args = parser.parse_args()

    slug = args.slug.strip()
    if not re.fullmatch(r"[a-z][a-z0-9_]*", slug):
        raise SystemExit("--slug must start with a letter and contain only lowercase letters, digits, or underscores")

    app_title = args.app_title.strip() if args.app_title else derive_app_title(slug)
    flutter_dir, current_name = locate_flutter_dir(slug)
    previous_names = {DEFAULT_DIRNAME, current_name}
    bundle_suffix = sanitize_bundle_suffix(slug)
    previous_bundle_suffixes = {sanitize_bundle_suffix(DEFAULT_DIRNAME), sanitize_bundle_suffix(current_name)}

    return Context(
        slug=slug,
        app_title=app_title,
        pascal_name=derive_pascal(slug),
        flutter_dir=flutter_dir,
        current_dir_name=current_name,
        previous_names=previous_names,
        bundle_suffix=bundle_suffix,
        previous_bundle_suffixes=previous_bundle_suffixes,
    )


def main() -> None:
    ctx = parse_args()
    changed: List[str] = []

    update_readmes(ctx, changed)
    update_docs(ctx, changed)
    update_tests_scripts(ctx, changed)
    update_ci(ctx, changed)
    update_flutter_package(ctx, changed)
    rename_flutter_directory(ctx, changed)
    update_platform_identifiers(ctx, changed)
    apply_global_replacements(ctx, changed)

    leftovers = find_remaining_markers(["Portalis", "portalis"])

    print("Updated files/directories:")
    if changed:
        for entry in sorted(changed):
            print(f"  - {entry}")
    else:
        print("  (no changes applied)")

    print("\nNext steps:")
    print("  • Run ./tests/all.sh to verify builds.")
    if leftovers:
        print("  • Review remaining references below (some may be intentional).")
        print("\nRemaining references to inspect:")
        for pattern, lines in leftovers.items():
            print(f"  Pattern '{pattern}':")
            for line in lines[:10]:
                print(f"    {line}")
            if len(lines) > 10:
                print(f"    ... ({len(lines) - 10} more)")
    else:
        print("  • No residual 'Portalis' references detected.")


if __name__ == "__main__":
    main()
