# RepoAtlas v0.6.2

Parallel scan and lazy GitHub details release.

## Highlights

- Added bounded parallel scanning for GitHub accounts, local scan roots, and local Git repository inspection.
- Reworked local scanning to index Git roots and Agent Context markers in one filesystem pass per scan root.
- Changed GitHub live details to load only when the user clicks `Load live details`, avoiding automatic Issues, Pull Requests, Releases, Pages, Deployments, and Packages calls while browsing the repo list.
- Added cached remote fallback: if GitHub live loading fails because of a timeout or TLS error, RepoAtlas can reuse the last saved remote repository list and still refresh local matching.

## Tuning

- `REPO_ATLAS_REMOTE_WORKERS` controls parallel GitHub account scans.
- `REPO_ATLAS_LOCAL_SCAN_WORKERS` controls parallel local directory walks.
- `REPO_ATLAS_LOCAL_GIT_WORKERS` controls parallel local Git inspections.

## Regression guard

- Added tests for cached remote repository fallback.
- CI continues to run formatting, `cargo check --locked`, `cargo test --locked`, and `node --check public/app.js`.
