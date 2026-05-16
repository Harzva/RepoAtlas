# RepoAtlas v0.6.3

Separated local scanning from Git remote fetching.

## Highlights

- `Refresh` now performs discovery and matching only. It scans GitHub repository lists, local folders, Git metadata, and context markers without running `git fetch`.
- Added a dedicated `Fetch remotes` action for known local Git repositories. This runs `git fetch --all --prune` in bounded parallel and then updates ahead/behind/diverged/dirty status.
- The UI now treats scan progress and fetch progress as separate operations with separate progress steps.
- Fetching reuses the saved repository inventory, so a slow network fetch no longer blocks local repository discovery.

## Why

Local filesystem discovery and network synchronization have different cost and failure modes. Keeping them separate makes normal scans faster, keeps local matching useful offline, and lets users choose when they want version drift checks.

## Regression guard

- CI continues to run formatting, `cargo check --locked`, `cargo test --locked`, and `node --check public/app.js`.
