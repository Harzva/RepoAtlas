---
name: repoatlas-repo-map
description: Generate and query GitHub remote-to-local repository maps without requiring the RepoAtlas desktop app. Bundles gh-repo-cartographer for repository inventory, local checkout matching, compact maps, address-cache refreshes, and optional MCP lookup.
---

# RepoAtlas Repo Map

## Purpose

Use this skill to answer: "Does this GitHub repo exist locally, and where is it?"

This skill is now app-independent:

- It does not require the RepoAtlas desktop executable.
- It bundles `scripts/gh_repo_cartographer.py` as the default scan engine.
- It can read a saved repository URL cache with `--repo-address-file`.
- It can refresh that cache with matched local folders via `--repo-address-output`.
- It can still consume RepoAtlas exports or compact maps through the MCP server.

## Quick Start

Generate a repository local-address mapping file from live GitHub accounts:

```powershell
python scripts\run_repoatlas_repo_map.py --scan-root "D:\study\code\0ai\产品" --account Harzva --account saihao --no-fetch --output-dir ".\repoatlas-map"
```

Generate from a saved address cache instead of live repo enumeration:

```powershell
python scripts\run_repoatlas_repo_map.py --repo-address-file "C:\Users\harzva\.codex\skills\gh-repo-cartographer\data\github-repo-addresses-harzva-just-agent.txt" --scan-root "D:\study\code" --no-fetch --output-dir ".\repoatlas-map"
```

Refresh an address cache with local paths:

```powershell
python scripts\run_repoatlas_repo_map.py --repo-address-file ".\data\github-repo-addresses-harzva-just-agent.txt" --scan-root "D:\study\code" --no-fetch --repo-address-output ".\data\github-repo-addresses-harzva-just-agent.txt"
```

The wrapper writes:

- `repoatlas-repo-map.md`: human-readable report.
- `repoatlas-repo-map.raw.json`: full cartographer inventory.
- `repoatlas-local-address-map.json`: compact remote-to-local mapping for downstream automation.

## MCP Server

Use MCP for repeated lookup after a map is generated:

```powershell
python scripts\repoatlas_mcp_server.py --map ".\repoatlas-map\repoatlas-local-address-map.json"
```

Tools exposed:

- `repoatlas_summary`: counts and map metadata.
- `repoatlas_find_repo`: find a remote repository and return local paths.
- `repoatlas_missing_local_copies`: list remote repos without local checkouts.
- `repoatlas_needs_attention`: list dirty, ahead, behind, diverged, or no-upstream repos.

## Workflow

1. Use `run_repoatlas_repo_map.py` to generate or refresh a compact map.
2. Prefer `--repo-address-file` when a saved remote URL cache already exists.
3. Use `--no-fetch` for fast local-only checks; omit it only when fresh ahead/behind status is required.
4. Query the compact map with `repoatlas_mcp_server.py` or inspect the JSON with a structured parser.
5. Report focused results instead of pasting the full inventory.

## Output Contract

For a repository lookup, report:

- `nameWithOwner`
- `localExists`
- `localPaths`
- `localStatus`
- `dirty`, `ahead`, and `behind` when present

For an inventory summary, report:

- inventory path
- remote count
- local match count
- missing local count
- attention count
