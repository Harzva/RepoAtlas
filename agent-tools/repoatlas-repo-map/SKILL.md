---
name: repoatlas-repo-map
description: Use RepoAtlas inventories as an agent context layer. Query whether GitHub repositories exist locally, find local checkout paths, identify missing local copies, and register the bundled RepoAtlas MCP server for token-efficient repository lookup.
---

# RepoAtlas Repo Map

## Use When

Use this skill when the user asks:

- Whether `owner/repo` exists locally.
- Where a GitHub repository is checked out.
- Which repositories are missing local clones.
- Which repositories are dirty, ahead, behind, diverged, or missing upstreams.
- How to install RepoAtlas as an MCP-backed agent tool.

## Preferred Path

Prefer the bundled MCP server for repeated lookup:

```toml
[mcp_servers.repoatlas]
command = "python"
args = [
  "C:\\path\\to\\RepoAtlas\\agent-tools\\mcp\\repoatlas_mcp_server.py",
  "--inventory",
  "C:\\path\\to\\repo-atlas.json",
]
startup_timeout_sec = 30
```

Then ask targeted questions such as:

```text
Find local paths for Harzva/RepoAtlas.
List repos missing local copies.
Show repos that need attention.
```

## Workflow

1. Locate the RepoAtlas inventory JSON.
   - Use a desktop export such as `repo-atlas.json` when available.
   - Use `REPO_ATLAS_DATA` when the user points to the live app inventory.
   - Use a compact generated map only when the user already has one.
2. If MCP is configured, call `repoatlas_find_repo`, `repoatlas_missing_local_copies`, `repoatlas_needs_attention`, or `repoatlas_summary`.
3. If MCP is not configured, run `agent-tools/mcp/repoatlas_mcp_server.py` directly with a JSON-RPC smoke payload or inspect the JSON with a structured parser.
4. Report only focused results. Do not paste the full inventory unless the user explicitly asks for it.

## Reporting Contract

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

## Notes

RepoAtlas desktop creates and exports repository inventory. The MCP server consumes that inventory so agents can answer small questions without loading large Markdown or JSON reports into context.
