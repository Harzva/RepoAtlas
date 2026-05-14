# RepoAtlas v0.5.1

Local context scan and Hook category release.

## Highlights

- Fixed a local scanner regression that could skip normal project folders and report `Local Git 0`.
- Added the `Hook` tag for hook, githook, pre-commit, pre-push, and webhook repositories.
- Added local context project discovery for Skill, MCP, Hook, and Agent-style folders under scan roots.
- Added local context project cards showing whether each folder is an initialized Git repository.
- Kept existing remote-to-local version drift checks for projects linked to GitHub remotes.
- Added activity-time sorting options: newest activity, oldest activity, last pushed, and oldest pushed.

## Compatibility

RepoAtlas still delegates GitHub access to GitHub CLI. Local context project cards are read-only and use the same safe folder-opening allowlist as matched repositories.

## Regression guard

- CI runs formatting, `cargo check --locked`, `cargo test --locked`, and `node --check public/app.js`.
- New tests cover normal directory walking, non-Git hook project detection, and Hook tag classification.
