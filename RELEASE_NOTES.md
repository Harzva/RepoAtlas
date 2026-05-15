# RepoAtlas v0.6.1

Local Git matching reliability patch.

## Fixes

- Fixed a regression where the desktop app could show every remote repository as `Missing local` when the Windows GUI environment could not resolve `git.exe` from `PATH`.
- Added Git executable discovery for common Windows Git installation paths and the optional `REPO_ATLAS_GIT` override.
- Kept directories with a `.git` marker as local repository candidates even if `git rev-parse --show-toplevel` fails.
- Added a fallback remote parser that reads `.git/config`, so local-to-GitHub matching can still work when `git remote -v` is unavailable.

## Regression guard

- Added tests for `.git` fallback scanning and `.git/config` remote parsing.
- CI continues to run formatting, `cargo check --locked`, `cargo test --locked`, and `node --check public/app.js`.
