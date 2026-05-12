# Harzva Repo Atlas v0.1.1

Product polish and workflow hardening release.

## Highlights

- Rebuilt the embedded dashboard UI with clean text, fixed encoding issues, and steadier responsive layout behavior.
- Added scan controls for custom local roots, fetch on/off, and max scan depth directly in the app.
- Added a Windows CI workflow for formatting, `cargo check --locked`, and dashboard JavaScript syntax checks.
- Added Rust build caching to the release workflow.

## Requirements for live rescans

- Git
- GitHub CLI `gh`
- `gh auth login` completed for the target GitHub account
