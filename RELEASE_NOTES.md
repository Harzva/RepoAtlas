# RepoAtlas v0.3.0

Multi-account context release.

## Highlights

- Added multi-account scanning with `accounts[]`, `REPO_ATLAS_ACCOUNTS`, and newline/comma/semicolon parsing in the UI.
- Added automatic repository categories: Skills, MCP, Memory, Software, Docs, Infra, Data, Research, Games, and Other.
- Redesigned the dashboard with a custom RepoAtlas logo, account chips, category filters, and four themes: Atlas, Midnight, Paper, and Aurora.
- Added a custom window icon so the desktop app no longer uses the default titlebar mark.
- Added macOS release packaging in GitHub Actions alongside the Windows portable exe.
- Updated CSV and Markdown exports with account and category columns.
- Refreshed README and GitHub Pages positioning for a generic, reusable RepoAtlas project.

## Requirements for live rescans

- Git
- GitHub CLI `gh`
- `gh auth login` completed for each account you want to scan, or a local account-router alias configured for that environment
