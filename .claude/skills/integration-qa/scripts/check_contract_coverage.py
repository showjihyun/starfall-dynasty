#!/usr/bin/env python3
"""Check that every contract type in contracts/registry/types.json has a schema,
fixtures, and a matching occurrence in the code of each producer/consumer.

Only the Python standard library is used so QA can run it before any toolchain
(Rust, Unity) is installed.

Exit codes: 0 = no errors, 1 = errors found, 2 = registry missing or unreadable.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

TYPE_KEYS = ("event_type", "command_type", "message_type", "type")
ORPHAN_DIRS = ("commands", "events", "messages")
EXCLUDED_DIRS = {
    ".git", "target", "Library", "Temp", "Obj", "obj", "Build", "Builds",
    "Logs", "UserSettings", "PackageCache", "node_modules",
}

# tag -> (root relative to repo, file suffix, required path fragment, excluded path fragment)
TAG_SOURCES = {
    "server": ("server", ".rs", None, "history"),
    "history": ("server", ".rs", "history", None),
    "client": ("client/Assets", ".cs", None, None),
    "bots": ("tools/bots", ".rs", None, None),
}


def pascal_case(name: str) -> str:
    return "".join(part.capitalize() for part in name.lower().split("_") if part)


def load_json(path: Path):
    with path.open(encoding="utf-8") as f:
        return json.load(f)


def find_type_constant(schema) -> str | None:
    """Return the const value of a top-level type discriminator property, if any."""
    if not isinstance(schema, dict):
        return None
    props = schema.get("properties")
    if not isinstance(props, dict):
        return None
    for key in TYPE_KEYS:
        prop = props.get(key)
        if isinstance(prop, dict) and isinstance(prop.get("const"), str):
            return prop["const"]
    return None


def fixture_type_value(fixture) -> str | None:
    if isinstance(fixture, dict):
        for key in TYPE_KEYS:
            if isinstance(fixture.get(key), str):
                return fixture[key]
    return None


class SourceIndex:
    """Lazily loads source file contents per tag."""

    def __init__(self, root: Path):
        self.root = root
        self._cache: dict[str, list[tuple[Path, str]] | None] = {}

    def files_for(self, tag: str):
        if tag in self._cache:
            return self._cache[tag]
        rel_root, suffix, required, excluded = TAG_SOURCES[tag]
        base = self.root / rel_root
        if not base.is_dir():
            self._cache[tag] = None
            return None
        files = []
        for path in sorted(base.rglob(f"*{suffix}")):
            parts = set(path.relative_to(base).parts)
            if parts & EXCLUDED_DIRS:
                continue
            rel = path.relative_to(self.root).as_posix()
            if required and required not in rel:
                continue
            if excluded and excluded in rel:
                continue
            try:
                files.append((path, path.read_text(encoding="utf-8", errors="ignore")))
            except OSError:
                continue
        self._cache[tag] = files
        return files

    def find(self, tag: str, name: str):
        """Return (root_exists, first matching file or None)."""
        files = self.files_for(tag)
        if files is None:
            return False, None
        literal = re.compile(r"\b" + re.escape(name) + r"\b")
        # Allow conventional suffixes such as MineResourceCommand or MineralMinedEvent.
        pascal = re.compile(r"\b" + re.escape(pascal_case(name)) + r"(?:[A-Z]\w*)?\b")
        for path, text in files:
            if literal.search(text) or pascal.search(text):
                return True, path
        return True, None


def check(root: Path):
    contracts = root / "contracts"
    registry_path = contracts / "registry" / "types.json"
    if not registry_path.is_file():
        return None, f"registry not found: {registry_path}"
    try:
        registry = load_json(registry_path)
    except (OSError, json.JSONDecodeError) as exc:
        return None, f"registry unreadable: {exc}"

    types = registry.get("types")
    if not isinstance(types, list):
        return None, "registry has no 'types' array"

    errors: list[str] = []
    warnings: list[str] = []
    rows: list[dict] = []
    index = SourceIndex(root)
    seen: set[str] = set()
    referenced_schemas: set[Path] = set()

    for entry in types:
        name = entry.get("name") if isinstance(entry, dict) else None
        if not isinstance(name, str) or not name:
            errors.append(f"registry entry without name: {entry!r}")
            continue
        row = {"name": name, "schema": "-", "fixtures": 0, "code": {}}
        rows.append(row)

        if name in seen:
            errors.append(f"{name}: duplicate registry entry")
        seen.add(name)

        missing = [k for k in ("kind", "schema", "producers", "consumers") if k not in entry]
        if missing:
            errors.append(f"{name}: missing registry fields {missing}")
        status = entry.get("status", "active")
        kind = entry.get("kind")

        # 1. schema
        schema_rel = entry.get("schema")
        if isinstance(schema_rel, str):
            schema_path = (contracts / schema_rel).resolve()
            referenced_schemas.add(schema_path)
            if not schema_path.is_file():
                errors.append(f"{name}: schema file missing ({schema_rel})")
                row["schema"] = "missing"
            else:
                try:
                    schema = load_json(schema_path)
                    row["schema"] = "ok"
                    const = find_type_constant(schema)
                    if const is not None and const != name:
                        errors.append(f"{name}: schema type constant is '{const}'")
                        row["schema"] = "mismatch"
                except json.JSONDecodeError as exc:
                    errors.append(f"{name}: schema is not valid JSON ({exc})")
                    row["schema"] = "invalid"

        # 2. fixtures
        fixture_dir = contracts / "fixtures" / name
        fixtures = sorted(fixture_dir.glob("*.json")) if fixture_dir.is_dir() else []
        row["fixtures"] = len(fixtures)
        if not fixtures and status == "active":
            errors.append(f"{name}: no fixtures in contracts/fixtures/{name}/")
        for fx in fixtures:
            try:
                value = fixture_type_value(load_json(fx))
            except json.JSONDecodeError as exc:
                errors.append(f"{name}: fixture {fx.name} is not valid JSON ({exc})")
                continue
            if kind not in ("rest", "data") and value is not None and value != name:
                errors.append(f"{name}: fixture {fx.name} declares type '{value}'")

        # 3. code occurrences per tag
        producers = entry.get("producers") or []
        consumers = entry.get("consumers") or []
        tags = [(t, "consumer") for t in consumers]
        if status != "deprecated":
            tags = [(t, "producer") for t in producers] + tags
        for tag, role in tags:
            key = f"{role}:{tag}"
            if tag not in TAG_SOURCES:
                warnings.append(f"{name}: unknown tag '{tag}' (known: {sorted(TAG_SOURCES)})")
                row["code"][key] = "unknown-tag"
                continue
            root_exists, hit = index.find(tag, name)
            if not root_exists:
                warnings.append(f"{name}: {role} '{tag}' source root missing (not implemented yet?)")
                row["code"][key] = "no-root"
            elif hit is None:
                errors.append(
                    f"{name}: {role} '{tag}' has no reference to '{name}' or '{pascal_case(name)}'"
                )
                row["code"][key] = "missing"
            else:
                row["code"][key] = hit.relative_to(root).as_posix()

    # 4. orphan schemas
    for sub in ORPHAN_DIRS:
        base = contracts / sub
        if not base.is_dir():
            continue
        for path in base.rglob("*.schema.json"):
            if path.resolve() not in referenced_schemas:
                warnings.append(f"orphan schema not in registry: {path.relative_to(root).as_posix()}")

    return {"errors": errors, "warnings": warnings, "types": rows}, None


def render_text(result, strict: bool) -> str:
    lines = ["# Contract coverage", ""]
    lines.append(f"- types: {len(result['types'])}")
    lines.append(f"- errors: {len(result['errors'])}")
    lines.append(f"- warnings: {len(result['warnings'])}{' (strict: counted as errors)' if strict else ''}")
    lines.append("")
    if result["types"]:
        lines.append("| type | schema | fixtures | code |")
        lines.append("|------|--------|----------|------|")
        for row in result["types"]:
            code = ", ".join(f"{k}={v}" for k, v in row["code"].items()) or "-"
            lines.append(f"| {row['name']} | {row['schema']} | {row['fixtures']} | {code} |")
        lines.append("")
    for title, items in (("Errors", result["errors"]), ("Warnings", result["warnings"])):
        if items:
            lines.append(f"## {title}")
            lines.extend(f"- {item}" for item in items)
            lines.append("")
    return "\n".join(lines)


def main() -> int:
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8")
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", default=".", help="repository root (contains contracts/)")
    parser.add_argument("--strict", action="store_true", help="treat warnings as errors")
    parser.add_argument("--json", action="store_true", help="print JSON instead of text")
    args = parser.parse_args()

    result, fatal = check(Path(args.root).resolve())
    if fatal:
        if args.json:
            print(json.dumps({"fatal": fatal}, ensure_ascii=False))
        else:
            print(f"FATAL: {fatal}")
        return 2

    failed = bool(result["errors"]) or (args.strict and bool(result["warnings"]))
    result["passed"] = not failed
    if args.json:
        print(json.dumps(result, ensure_ascii=False, indent=2))
    else:
        print(render_text(result, args.strict))
        print("RESULT:", "PASS" if not failed else "FAIL")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
