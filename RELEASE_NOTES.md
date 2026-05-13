# RepoAtlas v0.2.0

General-purpose rename and privacy cleanup release.

## Highlights

- Renamed the product and release assets to RepoAtlas.
- Removed the Harzva-specific seed inventory from the public app bundle.
- Switched the default inventory to an empty, account-neutral seed file.
- Added an account field so users can scan the current `gh` login or an optional local account-router alias.
- Replaced public screenshots and docs with generic example repositories.
- Updated report routes to generic `repo-atlas.*` exports.

## Requirements for live rescans

- Git
- GitHub CLI `gh`
- `gh auth login` completed for the GitHub account you want to inventory
