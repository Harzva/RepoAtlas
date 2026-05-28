#!/usr/bin/env python3
"""Generate a no-app remote-to-local repository address map."""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


SCRIPT_DIR = Path(__file__).resolve().parent
SKILL_DIR = SCRIPT_DIR.parent
DEFAULT_SCAN_ROOT = Path(os.environ.get("REPOATLAS_SCAN_ROOT", Path.cwd()))
DEFAULT_REPOATLAS_REMOTE = "https://github.com/Harzva/RepoAtlas.git"


def cartographer_candidates() -> list[Path]:
    return [
        SCRIPT_DIR / "gh_repo_cartographer.py",
        SKILL_DIR.parent / "gh-repo-cartographer" / "scripts" / "gh_repo_cartographer.py",
        Path.home() / ".codex" / "skills" / "gh-repo-cartographer" / "scripts" / "gh_repo_cartographer.py",
    ]


def first_existing(paths: list[Path]) -> Path | None:
    for path in paths:
        if path.exists():
            return path
    return None


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--account", action="append", default=[], help="Managed GitHub account alias. Repeatable.")
    parser.add_argument("--repo-address-file", action="append", default=[], help="Text cache containing GitHub repository URLs. Repeatable.")
    parser.add_argument("--repo-address-output", help="Write repository URL cache with matched local paths.")
    parser.add_argument("--scan-root", action="append", default=[], help="Root to scan for local Git repositories. Repeatable.")
    parser.add_argument("--max-depth", type=int, default=6, help="Maximum scan depth for local Git repositories.")
    parser.add_argument("--output-dir", default="repoatlas-repo-map-output", help="Directory for generated reports.")
    parser.add_argument("--no-fetch", action="store_true", help="Skip git fetch in the underlying cartographer.")
    parser.add_argument("--include-pages", action="store_true", help="Include GitHub Pages metadata.")
    parser.add_argument("--include-releases", action="store_true", help="Include latest release metadata.")
    parser.add_argument("--cartographer", help="Path to gh_repo_cartographer.py. Defaults to the bundled script.")
    parser.add_argument("--repoatlas-source", default=os.environ.get("REPOATLAS_SOURCE", ""), help="Optional RepoAtlas source checkout path for metadata only.")
    return parser


def repo_remote(path: Path) -> str | None:
    if not path or not (path / ".git").exists():
        return None
    proc = subprocess.run(
        ["git", "-C", str(path), "remote", "get-url", "origin"],
        text=True,
        capture_output=True,
        check=False,
    )
    if proc.returncode != 0:
        return None
    return proc.stdout.strip()


def compact_inventory(
    raw: dict[str, Any],
    output_dir: Path,
    raw_path: Path,
    markdown_path: Path,
    cartographer: Path,
    repoatlas_source: Path | None,
) -> dict[str, Any]:
    rows = raw.get("rows", [])
    repositories: list[dict[str, Any]] = []
    missing: list[str] = []
    attention: list[dict[str, Any]] = []

    for row in rows:
        remote = row.get("remote") or {}
        matches = row.get("localMatches") or []
        repo_key = remote.get("repoKey")
        local_paths = [match.get("path") for match in matches if match.get("path")]
        statuses = sorted({match.get("status") for match in matches if match.get("status")})
        dirty = any(bool(match.get("dirty")) for match in matches)
        local_exists = bool(local_paths)
        status = ", ".join(statuses) if statuses else row.get("localStatus") or "no-local-copy"

        item = {
            "repoKey": repo_key,
            "nameWithOwner": remote.get("nameWithOwner"),
            "url": remote.get("url"),
            "accountAlias": remote.get("accountAlias"),
            "accountLogin": remote.get("accountLogin"),
            "defaultBranch": row.get("defaultBranch"),
            "localExists": local_exists,
            "localPaths": local_paths,
            "localStatus": status,
            "dirty": dirty,
            "ahead": sum(int(match.get("ahead") or 0) for match in matches),
            "behind": sum(int(match.get("behind") or 0) for match in matches),
        }
        repositories.append(item)

        if not local_exists and repo_key:
            missing.append(repo_key)
        if status not in {"synced", "no-local-copy"} or dirty or item["ahead"] or item["behind"]:
            attention.append(item)

    source_remote = repo_remote(repoatlas_source) if repoatlas_source else None
    return {
        "generatedAt": datetime.now(timezone.utc).isoformat(),
        "tool": {
            "name": "repoatlas-repo-map",
            "mode": "no-app",
            "cartographer": str(cartographer),
        },
        "repoAtlas": {
            "sourcePath": str(repoatlas_source) if repoatlas_source else "",
            "sourceRemote": source_remote,
            "expectedRemote": DEFAULT_REPOATLAS_REMOTE,
            "requiresDesktopApp": False,
        },
        "outputs": {
            "directory": str(output_dir),
            "markdownReport": str(markdown_path),
            "rawJson": str(raw_path),
        },
        "summary": {
            "remoteCount": raw.get("remoteCount", len(repositories)),
            "localRepoCount": raw.get("localRepoCount"),
            "matchedRemoteCount": raw.get("matchedRemoteCount"),
            "missingLocalCount": len(missing),
            "attentionCount": len(attention),
        },
        "repositories": repositories,
        "missingLocalCopies": missing,
        "needsAttention": attention,
        "localOnly": raw.get("localOnly", []),
    }


def main() -> int:
    args = build_parser().parse_args()
    cartographer = Path(args.cartographer) if args.cartographer else first_existing(cartographer_candidates())
    if not cartographer or not cartographer.exists():
        print("error: gh_repo_cartographer.py not found. Pass --cartographer or install the bundled script.", file=sys.stderr)
        return 2

    scan_roots = [Path(value) for value in args.scan_root] or [DEFAULT_SCAN_ROOT]
    existing_roots = [root for root in scan_roots if root.exists()]
    if not existing_roots:
        roots = ", ".join(str(root) for root in scan_roots)
        print(f"error: no scan roots exist: {roots}", file=sys.stderr)
        return 2

    output_dir = Path(args.output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)
    markdown_path = output_dir / "repoatlas-repo-map.md"
    raw_path = output_dir / "repoatlas-repo-map.raw.json"
    compact_path = output_dir / "repoatlas-local-address-map.json"

    command = [
        sys.executable,
        str(cartographer),
        "--max-depth",
        str(args.max_depth),
        "--output",
        str(markdown_path),
        "--json-output",
        str(raw_path),
    ]
    for account in args.account:
        command.extend(["--account", account])
    for address_file in args.repo_address_file:
        command.extend(["--repo-address-file", address_file])
    for root in existing_roots:
        command.extend(["--scan-root", str(root)])
    if args.repo_address_output:
        command.extend(["--repo-address-output", args.repo_address_output])
    if args.no_fetch:
        command.append("--no-fetch")
    if args.include_pages:
        command.append("--include-pages")
    if args.include_releases:
        command.append("--include-releases")

    proc = subprocess.run(command, text=True, capture_output=True, check=False)
    if proc.returncode != 0:
        if proc.stdout:
            print(proc.stdout)
        if proc.stderr:
            print(proc.stderr, file=sys.stderr)
        return proc.returncode

    raw = json.loads(raw_path.read_text(encoding="utf-8"))
    repoatlas_source = Path(args.repoatlas_source) if args.repoatlas_source else None
    compact = compact_inventory(raw, output_dir, raw_path, markdown_path, cartographer, repoatlas_source)
    compact_path.write_text(json.dumps(compact, ensure_ascii=False, indent=2), encoding="utf-8")

    summary = compact["summary"]
    print(f"Cartographer engine: {cartographer}")
    if repoatlas_source:
        print(f"RepoAtlas source: {repoatlas_source}")
        print(f"RepoAtlas remote: {repo_remote(repoatlas_source) or DEFAULT_REPOATLAS_REMOTE}")
    print(f"Markdown report: {markdown_path}")
    print(f"Raw inventory JSON: {raw_path}")
    print(f"Local address map JSON: {compact_path}")
    if args.repo_address_output:
        print(f"Repo address cache: {args.repo_address_output}")
    print(
        "Summary: "
        f"{summary['remoteCount']} remote, "
        f"{summary['matchedRemoteCount']} matched locally, "
        f"{summary['missingLocalCount']} missing local copies, "
        f"{summary['attentionCount']} needing attention"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
