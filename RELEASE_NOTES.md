# RepoAtlas v0.6.0

Repository-centered Agent Context Tabs release.

## Highlights

- Replaced the old generic tag filter with repository-centered Context Tabs: Agents, Memory, Skills, MCP, Workflow, Rules, Hooks, and Other.
- Added multi-label context classification so one GitHub repository can appear under multiple context tabs.
- Added context evidence for repository details, including marker hits such as `AGENTS.md`, `.codex/skills`, `.github/workflows`, and `.pre-commit-config.yaml`.
- Attached local context markers back to matched GitHub repositories instead of turning RepoAtlas into a local-folder-first manager.
- Added Unlinked Local Contexts as a secondary panel for local context folders that do not map to scanned GitHub repositories.
- Added Git scope labels for local contexts: Git root, Inside Git, and No Git.
- Updated README and GitHub Pages copy to explain the new repo-centered Agent Context workflow.

## Compatibility

RepoAtlas remains read-only for local context management. It does not read marker file contents, initialize Git repositories, create templates, or edit user context files.

## Regression guard

- CI continues to run formatting, `cargo check --locked`, `cargo test --locked`, and `node --check public/app.js`.
- New tests cover Agents, Memory, Skills, MCP, Workflow, Rules, Hooks, multi-label classification, local context matching, and Git scope states.
