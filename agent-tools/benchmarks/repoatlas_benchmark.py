#!/usr/bin/env python3
"""Benchmark traditional repo cartography against RepoAtlas MCP lookup."""

from __future__ import annotations

import argparse
import importlib.util
import json
import statistics
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


DEFAULT_CARTOGRAPHER = (
    Path(__file__).resolve().parents[1]
    / "gh-repo-cartographer"
    / "scripts"
    / "gh_repo_cartographer.py"
)
DEFAULT_SCAN_ROOT = Path.cwd()
MCP_SERVER = Path(__file__).resolve().parents[1] / "mcp" / "repoatlas_mcp_server.py"


def estimate_tokens(text: str) -> int:
    # Simple cross-model estimate for planning: English/code averages roughly 4 chars/token.
    return max(1, round(len(text) / 4))


def timed(label: str, fn):
    start = time.perf_counter()
    value = fn()
    return label, time.perf_counter() - start, value


def load_cartographer(path: Path):
    spec = importlib.util.spec_from_file_location("gh_repo_cartographer_benchmark", path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"Cannot import cartographer script: {path}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def compact_status_counts(rows: list[dict[str, Any]]) -> dict[str, int]:
    counts: dict[str, int] = {}
    for row in rows:
        status = row.get("localStatus") or "unknown"
        counts[status] = counts.get(status, 0) + 1
    return dict(sorted(counts.items(), key=lambda item: (-item[1], item[0])))


def run_mcp_query(inventory_path: Path, query: str, repeat: int) -> dict[str, Any]:
    payload = (
        json.dumps(
            {
                "jsonrpc": "2.0",
                "id": 1,
                "method": "tools/call",
                "params": {
                    "name": "repoatlas_find_repo",
                    "arguments": {"query": query, "limit": 10},
                },
            },
            ensure_ascii=False,
        )
        + "\n"
    )
    timings: list[float] = []
    outputs: list[str] = []
    for _ in range(max(1, repeat)):
        start = time.perf_counter()
        proc = subprocess.run(
            [sys.executable, str(MCP_SERVER), "--inventory", str(inventory_path)],
            input=payload,
            text=True,
            capture_output=True,
            check=False,
        )
        elapsed = time.perf_counter() - start
        if proc.returncode != 0:
            raise RuntimeError(proc.stderr.strip() or proc.stdout.strip() or "MCP query failed")
        timings.append(elapsed)
        outputs.append(proc.stdout)
    output = outputs[-1]
    return {
        "repeat": max(1, repeat),
        "seconds": {
            "min": min(timings),
            "median": statistics.median(timings),
            "max": max(timings),
        },
        "responseBytes": len(output.encode("utf-8")),
        "estimatedResponseTokens": estimate_tokens(output),
        "lastResponse": json.loads(output.splitlines()[-1]),
    }


def run_traditional_scan(args: argparse.Namespace, cartographer) -> dict[str, Any]:
    phase_times: dict[str, float] = {}

    if args.account:
        accounts = list(dict.fromkeys(args.account))
    else:
        _, elapsed, accounts = timed("accountDiscovery", lambda: cartographer.unique(cartographer.parse_router_accounts()))
        phase_times["accountDiscovery"] = elapsed
    if not accounts:
        raise RuntimeError("No accounts found. Pass --account or configure gh-account-router.")

    scan_root_values = args.scan_root or [str(DEFAULT_SCAN_ROOT)]
    _, elapsed, scan_roots = timed("scanRootDiscovery", lambda: cartographer.discover_scan_roots(scan_root_values))
    phase_times["scanRootDiscovery"] = elapsed
    if not scan_roots:
        raise RuntimeError("No scan roots exist.")

    _, elapsed, remote_repos = timed("remoteScan", lambda: cartographer.list_remote_repos(accounts))
    phase_times["remoteScan"] = elapsed

    if args.include_pages or args.include_releases:
        _, elapsed, _ = timed(
            "remoteMetadataEnrichment",
            lambda: cartographer.enrich_remote_metadata(
                remote_repos,
                include_pages=args.include_pages,
                include_releases=args.include_releases,
            ),
        )
        phase_times["remoteMetadataEnrichment"] = elapsed

    _, elapsed, git_roots = timed("localGitDiscovery", lambda: cartographer.find_git_roots(scan_roots, args.max_depth))
    phase_times["localGitDiscovery"] = elapsed

    _, elapsed, local_repos = timed(
        "localRepoInspection",
        lambda: [cartographer.inspect_local_repo(path, fetch=not args.no_fetch) for path in git_roots],
    )
    phase_times["localRepoInspection"] = elapsed

    _, elapsed, inventory = timed("mergeInventory", lambda: cartographer.merge_inventory(remote_repos, local_repos))
    phase_times["mergeInventory"] = elapsed

    _, elapsed, markdown = timed(
        "renderMarkdown",
        lambda: cartographer.render_markdown(inventory, scan_roots, fetched=not args.no_fetch),
    )
    phase_times["renderMarkdown"] = elapsed

    raw_json = json.dumps(inventory, ensure_ascii=False, indent=2)
    phase_times["traditionalTotal"] = sum(phase_times.values())
    return {
        "accounts": accounts,
        "scanRoots": [str(root) for root in scan_roots],
        "gitRootCount": len(git_roots),
        "inventory": inventory,
        "markdown": markdown,
        "rawJson": raw_json,
        "phaseSeconds": phase_times,
    }


def render_report(result: dict[str, Any]) -> str:
    summary = result["summary"]
    phases = result["traditional"]["phaseSeconds"]
    token = result["tokenComparison"]
    speed = result["speedComparison"]
    lines = [
        "# RepoAtlas Agent Lookup Benchmark",
        "",
        f"- Generated at: {result['generatedAt']}",
        f"- Query: `{result['query']}`",
        f"- Accounts: {', '.join(result['traditional']['accounts'])}",
        f"- Scan roots: {', '.join(result['traditional']['scanRoots'])}",
        f"- No fetch: {'yes' if result['options']['noFetch'] else 'no'}",
        "",
        "## Inventory",
        "",
        f"- Remote repositories: {summary['remoteCount']}",
        f"- Local Git repositories discovered: {summary['localRepoCount']}",
        f"- Remote repositories matched locally: {summary['matchedRemoteCount']}",
        f"- Missing local copies: {summary['missingLocalCount']}",
        "",
        "## Time Comparison",
        "",
        "| Step | Seconds |",
        "|---|---:|",
    ]
    for key, value in phases.items():
        lines.append(f"| {key} | {value:.3f} |")
    lines.extend(
        [
            f"| MCP lookup median | {result['mcp']['seconds']['median']:.3f} |",
            "",
            f"- Traditional full scan vs MCP lookup: **{speed['traditionalScanVsMcpMedian']:.1f}x faster** for repeated focused lookup.",
            f"- Traditional full report bytes vs MCP response bytes: **{token['rawJsonVsMcpResponseBytesRatio']:.1f}x smaller** response.",
            "",
            "## Token Estimate",
            "",
            "| Artifact | Bytes | Estimated tokens |",
            "|---|---:|---:|",
            f"| Traditional Markdown report | {token['markdownBytes']} | {token['markdownEstimatedTokens']} |",
            f"| Traditional raw JSON inventory | {token['rawJsonBytes']} | {token['rawJsonEstimatedTokens']} |",
            f"| MCP focused response | {token['mcpResponseBytes']} | {token['mcpEstimatedResponseTokens']} |",
            "",
            "Token estimates use a simple 4 characters per token planning heuristic. Actual tokenizer counts vary by model and language mix.",
            "",
            "## Status Counts",
            "",
            "| Status | Count |",
            "|---|---:|",
        ]
    )
    for status, count in result["statusCounts"].items():
        lines.append(f"| {status} | {count} |")
    lines.append("")
    return "\n".join(lines)


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cartographer", default=str(DEFAULT_CARTOGRAPHER), help="Path to gh_repo_cartographer.py.")
    parser.add_argument("--account", action="append", default=[], help="Account alias/login to scan. Repeatable.")
    parser.add_argument("--scan-root", action="append", default=[], help="Local root to scan. Repeatable.")
    parser.add_argument("--max-depth", type=int, default=6, help="Maximum local Git discovery depth.")
    parser.add_argument("--no-fetch", action="store_true", help="Skip git fetch during local repo inspection.")
    parser.add_argument("--include-pages", action="store_true", help="Include GitHub Pages metadata in the traditional scan.")
    parser.add_argument("--include-releases", action="store_true", help="Include release metadata in the traditional scan.")
    parser.add_argument("--query", default="Harzva/RepoAtlas", help="Repository query used for MCP lookup.")
    parser.add_argument("--repeat-mcp", type=int, default=5, help="MCP query repetitions for min/median/max timing.")
    parser.add_argument("--output-dir", default="agent-tools/benchmarks/output", help="Directory for benchmark artifacts.")
    return parser


def main() -> int:
    args = build_parser().parse_args()
    output_dir = Path(args.output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)
    cartographer = load_cartographer(Path(args.cartographer))

    traditional = run_traditional_scan(args, cartographer)
    raw_inventory_path = output_dir / "repoatlas-benchmark-inventory.json"
    markdown_path = output_dir / "repoatlas-benchmark-traditional.md"
    raw_inventory_path.write_text(traditional["rawJson"], encoding="utf-8")
    markdown_path.write_text(traditional["markdown"], encoding="utf-8")

    mcp = run_mcp_query(raw_inventory_path, args.query, args.repeat_mcp)
    inventory = traditional["inventory"]
    rows = inventory.get("rows", [])
    remote_count = int(inventory.get("remoteCount") or len(rows))
    matched_count = int(inventory.get("matchedRemoteCount") or 0)
    summary = {
        "remoteCount": remote_count,
        "localRepoCount": int(inventory.get("localRepoCount") or traditional["gitRootCount"]),
        "matchedRemoteCount": matched_count,
        "missingLocalCount": max(0, remote_count - matched_count),
    }
    raw_json_bytes = len(traditional["rawJson"].encode("utf-8"))
    markdown_bytes = len(traditional["markdown"].encode("utf-8"))
    mcp_bytes = int(mcp["responseBytes"])
    traditional_total = traditional["phaseSeconds"]["traditionalTotal"]
    mcp_median = mcp["seconds"]["median"]
    result = {
        "generatedAt": datetime.now(timezone.utc).isoformat(),
        "query": args.query,
        "options": {
            "noFetch": args.no_fetch,
            "includePages": args.include_pages,
            "includeReleases": args.include_releases,
            "maxDepth": args.max_depth,
        },
        "summary": summary,
        "traditional": {
            "accounts": traditional["accounts"],
            "scanRoots": traditional["scanRoots"],
            "gitRootCount": traditional["gitRootCount"],
            "phaseSeconds": traditional["phaseSeconds"],
            "markdownPath": str(markdown_path),
            "rawJsonPath": str(raw_inventory_path),
        },
        "mcp": mcp,
        "speedComparison": {
            "traditionalScanVsMcpMedian": traditional_total / mcp_median if mcp_median else None,
        },
        "tokenComparison": {
            "markdownBytes": markdown_bytes,
            "markdownEstimatedTokens": estimate_tokens(traditional["markdown"]),
            "rawJsonBytes": raw_json_bytes,
            "rawJsonEstimatedTokens": estimate_tokens(traditional["rawJson"]),
            "mcpResponseBytes": mcp_bytes,
            "mcpEstimatedResponseTokens": mcp["estimatedResponseTokens"],
            "rawJsonVsMcpResponseBytesRatio": raw_json_bytes / mcp_bytes if mcp_bytes else None,
            "markdownVsMcpResponseBytesRatio": markdown_bytes / mcp_bytes if mcp_bytes else None,
        },
        "statusCounts": compact_status_counts(rows),
    }

    result_path = output_dir / "repoatlas-benchmark-result.json"
    report_path = output_dir / "repoatlas-benchmark-report.md"
    result_path.write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding="utf-8")
    report_path.write_text(render_report(result), encoding="utf-8")
    print(f"Benchmark JSON: {result_path}")
    print(f"Benchmark report: {report_path}")
    print(
        "Summary: "
        f"{summary['remoteCount']} remote, "
        f"{summary['localRepoCount']} local, "
        f"{summary['matchedRemoteCount']} matched, "
        f"{summary['missingLocalCount']} missing"
    )
    print(
        "Speed: "
        f"traditional {traditional_total:.3f}s, "
        f"MCP median {mcp_median:.3f}s, "
        f"{result['speedComparison']['traditionalScanVsMcpMedian']:.1f}x faster"
    )
    print(
        "Token estimate: "
        f"raw JSON {result['tokenComparison']['rawJsonEstimatedTokens']} vs "
        f"MCP {result['tokenComparison']['mcpEstimatedResponseTokens']}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
