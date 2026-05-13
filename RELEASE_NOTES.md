# RepoAtlas v0.4.0

Authentication and progress experience release.

## Highlights

- Added an in-app GitHub Login panel that checks GitHub CLI status and starts `gh auth login --web` from RepoAtlas.
- Added optional custom `gh.exe` path support for users who already have a portable or manually installed GitHub CLI.
- Hid Windows console windows for background `gh`, `git`, and helper commands during scans and login.
- Replaced scan completion toast with a visible operation progress bar and step chips for Auth, Remote, Local, Compare, and Render.
- Added login progress feedback for CLI detection, browser login, callback wait, and verification.
- Improved layout spacing to reduce panel overlap, widened the sidebar, and contained wide repository tables with horizontal scrolling.
- Polished the sidebar authentication panel with an achievement-style badge inspired by the local icon reference set.

## Login model

RepoAtlas still does not store GitHub passwords or tokens itself. GitHub CLI stores credentials locally after login.

- Recommended: click `Login` in RepoAtlas.
- Equivalent terminal command: `gh auth login --web --git-protocol https --hostname github.com`.
- Existing CLI users can click `Check` and scan immediately.
- Portable CLI users can set a custom `gh.exe` path in the sidebar.

## Regression guard

- CI continues to run formatting, `cargo check --locked`, `cargo test --locked`, and `node --check public/app.js`.
