# RepoAtlas v0.3.2

Beginner onboarding release.

## Highlights

- Added a first-run guide modal that explains GitHub CLI login, account modes, local scan roots, and the refresh flow.
- Added a sidebar `Guide` button so users can reopen the tutorial any time.
- Added copy buttons for the key setup commands: `gh auth login --web` and `gh auth status`.
- Documented the supported login paths in README: current `gh` login, GitHub CLI web login, account-router aliases, and token/headless GitHub CLI setup.

## Login model

RepoAtlas does not store GitHub passwords or tokens. It delegates authentication to GitHub CLI:

- Recommended desktop setup: `gh auth login --web`
- Status check: `gh auth status`
- Headless setup: `gh auth login --with-token` or `GH_TOKEN`
- Multi-account setup: enter one GitHub account or configured router alias per line in RepoAtlas

## Included from v0.3.1

- Empty first-run inventories automatically refresh instead of staying at zero.
- Refresh errors remain visible in the page.
- Regression tests cover account parsing and category inference.
