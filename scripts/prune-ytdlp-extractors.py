#!/usr/bin/env python3
"""Prune yt-dlp extractors to keep only specified sites.

Rewrites yt_dlp/extractor/_extractors.py to import only the allowed
extractor modules, then deletes all other extractor .py files.

Usage:
    python3 scripts/prune-ytdlp-extractors.py /path/to/yt-dlp [--keep youtube,spotify]

The --keep flag accepts a comma-separated list of extractor module names
(filenames without .py). Defaults to "youtube" if not specified.
"""

import argparse
import ast
import os
import sys
from pathlib import Path

# Core extractor modules that must always be kept (yt-dlp internals)
CORE_MODULES = {
    "__init__",
    "_extractors",
    "extractors",
    "common",
    "commonmistakes",
    "commonprotocols",
    "generic",
    "genericembeds",
    "openload",
    "adobepass",  # imported directly by yt_dlp/__init__.py
    # Some non-target extractors are imported by core downloader logic in newer yt-dlp.
    # Keep them to avoid import-time failures during build (lazy extractor generation).
    "afreecatv",
}


def parse_extractors_file(extractors_path: Path) -> list[tuple[str, str]]:
    """Parse _extractors.py and return list of (module_name, import_statement)."""
    source = extractors_path.read_text()
    tree = ast.parse(source)

    imports = []
    lines = source.splitlines()

    for node in ast.iter_child_nodes(tree):
        if isinstance(node, ast.ImportFrom) and node.module:
            # Get the module name (e.g., ".youtube" -> "youtube")
            module = node.module.lstrip(".")
            # Extract the full import statement text from source
            start = node.lineno - 1
            end = node.end_lineno
            stmt = "\n".join(lines[start:end])
            imports.append((module, stmt))

    return imports


def main():
    parser = argparse.ArgumentParser(description="Prune yt-dlp extractors")
    parser.add_argument("ytdlp_dir", help="Path to yt-dlp source directory")
    parser.add_argument(
        "--keep",
        default="youtube",
        help="Comma-separated extractor module names to keep (default: youtube)",
    )
    args = parser.parse_args()

    ytdlp_dir = Path(args.ytdlp_dir)
    extractor_dir = ytdlp_dir / "yt_dlp" / "extractor"
    extractors_file = extractor_dir / "_extractors.py"

    if not extractors_file.exists():
        print(f"ERROR: {extractors_file} not found", file=sys.stderr)
        sys.exit(1)

    keep_modules = {m.strip() for m in args.keep.split(",")}
    allowed_modules = CORE_MODULES | keep_modules

    print(f"Keeping extractor modules: {sorted(keep_modules)}")
    print(f"Core modules: {sorted(CORE_MODULES)}")

    # Parse existing imports
    imports = parse_extractors_file(extractors_file)
    print(f"Found {len(imports)} import statements in _extractors.py")

    # Filter imports to only allowed modules
    kept_imports = []
    removed_count = 0
    for module, stmt in imports:
        if module in allowed_modules:
            kept_imports.append(stmt)
        else:
            removed_count += 1

    print(f"Keeping {len(kept_imports)} imports, removing {removed_count}")

    # Write pruned _extractors.py
    new_content = "\n".join(kept_imports) + "\n"
    extractors_file.write_text(new_content)
    print(f"Wrote pruned {extractors_file}")

    # Delete extractor .py files that aren't in the allowed set
    deleted_count = 0
    for py_file in sorted(extractor_dir.glob("*.py")):
        module_name = py_file.stem
        if module_name not in allowed_modules and module_name != "lazy_extractors":
            py_file.unlink()
            deleted_count += 1

    print(f"Deleted {deleted_count} extractor files")

    # Remove lazy_extractors.py if it exists (will be regenerated)
    lazy = extractor_dir / "lazy_extractors.py"
    if lazy.exists():
        lazy.unlink()
        print("Removed stale lazy_extractors.py")

    print("Done! Run `python devscripts/make_lazy_extractors.py` next.")


if __name__ == "__main__":
    main()
