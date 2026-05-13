# RepoAtlas v0.5.0

Login+ and GitHub live details release.

## Highlights

- Added a `Login+` account panel for adding scan accounts or router aliases.
- Added browser login from `Login+`, with a force option for adding another GitHub CLI account.
- Added one-time token access login through GitHub CLI. RepoAtlas passes the token to `gh auth login --with-token` and does not store it.
- Added known GitHub CLI account display from `gh auth status --json hosts`.
- Added lazy-loaded repository details for Issues, Pull Requests, Releases, GitHub Pages, Deployments, and Packages.
- Added GitHub links for opening Issues, PRs, Releases, Pages settings, Deployments, Packages, and new release creation in the browser.
- Added a GitHub Pages roadmap section for a future Codex plugin UI and a static waitlist form.

## Compatibility

RepoAtlas still delegates authentication and credential storage to GitHub CLI. GitHub management actions continue to open in the browser; this release only reads and links GitHub metadata.

## Regression guard

- CI runs formatting, `cargo check --locked`, `cargo test --locked`, and `node --check public/app.js`.
