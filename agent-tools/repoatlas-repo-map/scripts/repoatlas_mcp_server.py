#!/usr/bin/env python3
"""MCP server for querying RepoAtlas inventory files."""

from __future__ import annotations

import argparse
import json
import os
import sys
from pathlib import Path
from typing import Any


def normalize_repo_key(value: str) -> str:
    text = value.strip()
    text = text.removeprefix("https://github.com/")
    text = text.removeprefix("http://github.com/")
    text = text.removeprefix("git@github.com:")
    text = text.removesuffix(".git")
    return text.strip("/").lower()


def first_text(*values: Any) -> str:
    for value in values:
        if isinstance(value, str) and value:
            return value
    return ""


def int_sum(items: list[dict[str, Any]], key: str) -> int:
    total = 0
    for item in items:
        try:
            total += int(item.get(key) or 0)
        except (TypeError, ValueError):
            continue
    return total


def default_inventory_path() -> Path:
    for env_name in ("REPO_ATLAS_MCP_INVENTORY", "REPO_ATLAS_DATA"):
        value = os.environ.get(env_name)
        if value:
            return Path(value)
    return Path("repo-atlas.json")


class RepoAtlasInventory:
    def __init__(self, inventory_path: Path) -> None:
        self.inventory_path = inventory_path

    def load(self) -> dict[str, Any]:
        if not self.inventory_path.exists():
            raise FileNotFoundError(f"RepoAtlas inventory not found: {self.inventory_path}")
        return json.loads(self.inventory_path.read_text(encoding="utf-8"))

    def repositories(self) -> list[dict[str, Any]]:
        data = self.load()
        if isinstance(data.get("repositories"), list):
            return [self._from_compact_repo(repo) for repo in data["repositories"] if isinstance(repo, dict)]
        return [self._from_row(row) for row in data.get("rows", []) if isinstance(row, dict)]

    def _from_compact_repo(self, repo: dict[str, Any]) -> dict[str, Any]:
        return {
            "repoKey": first_text(repo.get("repoKey"), normalize_repo_key(first_text(repo.get("nameWithOwner"), repo.get("url")))),
            "nameWithOwner": first_text(repo.get("nameWithOwner"), repo.get("repoKey")),
            "url": first_text(repo.get("url")),
            "accountAlias": first_text(repo.get("accountAlias")),
            "accountLogin": first_text(repo.get("accountLogin")),
            "defaultBranch": first_text(repo.get("defaultBranch")),
            "localExists": bool(repo.get("localExists") or repo.get("localPaths")),
            "localPaths": [path for path in repo.get("localPaths", []) if isinstance(path, str)],
            "localStatus": first_text(repo.get("localStatus"), "no-local-copy"),
            "localStatusList": [first_text(repo.get("localStatus"), "no-local-copy")],
            "dirty": bool(repo.get("dirty")),
            "ahead": int(repo.get("ahead") or 0),
            "behind": int(repo.get("behind") or 0),
            "category": first_text(repo.get("category")),
            "sourceShape": "compact",
        }

    def _from_row(self, row: dict[str, Any]) -> dict[str, Any]:
        if isinstance(row.get("remote"), dict):
            remote = row.get("remote") or {}
            matches = [match for match in row.get("localMatches", []) if isinstance(match, dict)]
            local_paths = [match.get("path") for match in matches if isinstance(match.get("path"), str)]
            statuses = sorted({match.get("status") for match in matches if isinstance(match.get("status"), str)})
            status = ", ".join(statuses) if statuses else first_text(row.get("localStatus"), "no-local-copy")
            return {
                "repoKey": first_text(remote.get("repoKey"), normalize_repo_key(first_text(remote.get("nameWithOwner"), remote.get("url")))),
                "nameWithOwner": first_text(remote.get("nameWithOwner"), remote.get("repoKey")),
                "url": first_text(remote.get("url")),
                "accountAlias": first_text(remote.get("accountAlias")),
                "accountLogin": first_text(remote.get("accountLogin")),
                "defaultBranch": first_text((row.get("defaultBranchRef") or {}).get("name") if isinstance(row.get("defaultBranchRef"), dict) else None, row.get("defaultBranch")),
                "localExists": bool(local_paths),
                "localPaths": local_paths,
                "localStatus": status,
                "localStatusList": statuses or [status],
                "dirty": any(bool(match.get("dirty")) for match in matches),
                "ahead": int_sum(matches, "ahead"),
                "behind": int_sum(matches, "behind"),
                "category": "",
                "sourceShape": "raw",
            }

        local_paths = [path for path in row.get("localPaths", []) if isinstance(path, str)]
        statuses = [status for status in row.get("localStatusList", []) if isinstance(status, str)]
        status = first_text(row.get("localStatus"), ", ".join(statuses), "no-local-copy")
        matches = [match for match in row.get("localMatches", []) if isinstance(match, dict)]
        return {
            "repoKey": first_text(row.get("repoKey"), normalize_repo_key(first_text(row.get("name"), row.get("url")))),
            "nameWithOwner": first_text(row.get("name"), row.get("nameWithOwner"), row.get("repoKey")),
            "url": first_text(row.get("url")),
            "accountAlias": first_text(row.get("accountAlias")),
            "accountLogin": first_text(row.get("accountLogin"), row.get("owner")),
            "defaultBranch": first_text(row.get("defaultBranch")),
            "localExists": bool(local_paths),
            "localPaths": local_paths,
            "localStatus": status,
            "localStatusList": statuses or [status],
            "dirty": any(bool(match.get("dirty")) for match in matches),
            "ahead": int_sum(matches, "ahead"),
            "behind": int_sum(matches, "behind"),
            "category": first_text(row.get("category"), row.get("categoryLabel")),
            "sourceShape": "flattened",
        }

    def summary(self) -> dict[str, Any]:
        data = self.load()
        repos = self.repositories()
        matched = sum(1 for repo in repos if repo["localExists"])
        missing = len(repos) - matched
        attention = [repo for repo in repos if needs_attention(repo)]
        return {
            "inventoryPath": str(self.inventory_path),
            "generatedAt": data.get("generatedAt"),
            "remoteCount": data.get("remoteCount", len(repos)),
            "localRepoCount": data.get("localRepoCount"),
            "matchedRemoteCount": data.get("matchedRemoteCount", matched),
            "missingLocalCount": missing,
            "attentionCount": len(attention),
        }


def needs_attention(repo: dict[str, Any]) -> bool:
    statuses = {str(status) for status in repo.get("localStatusList", [])}
    status_text = str(repo.get("localStatus") or "")
    status_hit = any(status in {"ahead", "behind", "diverged", "no-upstream"} for status in statuses)
    return status_hit or any(token in status_text for token in ("ahead", "behind", "diverged", "no-upstream")) or bool(repo.get("dirty"))


class RepoAtlasMcp:
    def __init__(self, inventory_path: Path) -> None:
        self.inventory = RepoAtlasInventory(inventory_path)

    def handle_tool_call(self, name: str, arguments: dict[str, Any]) -> dict[str, Any]:
        if name == "repoatlas_summary":
            payload = self.inventory.summary()
        elif name == "repoatlas_find_repo":
            payload = self.find_repo(str(arguments.get("query") or ""), int(arguments.get("limit") or 10))
        elif name == "repoatlas_missing_local_copies":
            payload = self.missing_local_copies(int(arguments.get("limit") or 50))
        elif name == "repoatlas_needs_attention":
            payload = self.needs_attention(int(arguments.get("limit") or 50))
        else:
            raise ValueError(f"Unknown tool: {name}")
        return {"content": [{"type": "text", "text": json.dumps(payload, ensure_ascii=False, indent=2)}]}

    def find_repo(self, query: str, limit: int) -> dict[str, Any]:
        needle = normalize_repo_key(query)
        exact: list[dict[str, Any]] = []
        partial: list[dict[str, Any]] = []
        for repo in self.inventory.repositories():
            haystack = " ".join(
                str(repo.get(key) or "").lower()
                for key in ("repoKey", "nameWithOwner", "url", "accountAlias", "accountLogin", "category")
            )
            repo_key = str(repo.get("repoKey") or "").lower()
            if needle and needle == repo_key:
                exact.append(repo)
            elif needle and needle in haystack:
                partial.append(repo)
        matches = (exact + partial)[: max(1, min(limit, 50))]
        return {"query": query, "normalizedQuery": needle, "count": len(matches), "matches": matches}

    def missing_local_copies(self, limit: int) -> dict[str, Any]:
        items = [repo for repo in self.inventory.repositories() if not repo["localExists"]]
        return {"count": len(items), "items": items[: max(1, min(limit, 200))]}

    def needs_attention(self, limit: int) -> dict[str, Any]:
        items = [repo for repo in self.inventory.repositories() if needs_attention(repo)]
        return {"count": len(items), "items": items[: max(1, min(limit, 200))]}


TOOLS = [
    {
        "name": "repoatlas_summary",
        "description": "Return counts and metadata for the configured RepoAtlas inventory.",
        "inputSchema": {"type": "object", "properties": {}, "additionalProperties": False},
    },
    {
        "name": "repoatlas_find_repo",
        "description": "Find whether a GitHub repository exists locally and return local paths and sync status.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "query": {"type": "string", "description": "Repository key, GitHub URL, or partial text."},
                "limit": {"type": "integer", "minimum": 1, "maximum": 50, "default": 10},
            },
            "required": ["query"],
            "additionalProperties": False,
        },
    },
    {
        "name": "repoatlas_missing_local_copies",
        "description": "List remote repositories with no local checkout.",
        "inputSchema": {
            "type": "object",
            "properties": {"limit": {"type": "integer", "minimum": 1, "maximum": 200, "default": 50}},
            "additionalProperties": False,
        },
    },
    {
        "name": "repoatlas_needs_attention",
        "description": "List repositories that are dirty, ahead, behind, diverged, or missing upstreams.",
        "inputSchema": {
            "type": "object",
            "properties": {"limit": {"type": "integer", "minimum": 1, "maximum": 200, "default": 50}},
            "additionalProperties": False,
        },
    },
]


def respond(message_id: Any, result: Any = None, error: Any = None) -> None:
    response: dict[str, Any] = {"jsonrpc": "2.0", "id": message_id}
    if error is None:
        response["result"] = result
    else:
        response["error"] = error
    sys.stdout.write(json.dumps(response, ensure_ascii=False) + "\n")
    sys.stdout.flush()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--inventory", default=str(default_inventory_path()), help="RepoAtlas inventory or exported JSON path.")
    args = parser.parse_args()
    server = RepoAtlasMcp(Path(args.inventory))

    for line in sys.stdin:
        if not line.strip():
            continue
        request_id = None
        try:
            request = json.loads(line)
            request_id = request.get("id")
            method = request.get("method")
            if method == "initialize":
                respond(
                    request_id,
                    {
                        "protocolVersion": "2024-11-05",
                        "capabilities": {"tools": {}},
                        "serverInfo": {"name": "repoatlas", "version": "0.1.0"},
                    },
                )
            elif method == "tools/list":
                respond(request_id, {"tools": TOOLS})
            elif method == "tools/call":
                params = request.get("params") or {}
                respond(request_id, server.handle_tool_call(str(params.get("name")), params.get("arguments") or {}))
            elif method and method.startswith("notifications/"):
                continue
            else:
                respond(request_id, error={"code": -32601, "message": f"Method not found: {method}"})
        except Exception as exc:
            respond(request_id, error={"code": -32000, "message": str(exc)})
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
