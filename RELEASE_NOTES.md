# RepoAtlas v0.6.6

Local match folder path correctness release.

## Highlights

- Fixed the `Local matches` card folder button to open the same primary local checkout shown in repository details.
- Unified the main folder/path actions around `localMatches[0].path`, with `localPaths[0]` kept only as a compatibility fallback.
- Updated the card folder tooltip to show the full local path instead of only the folder basename.

## Why

The repository detail panel already used the matched checkout path directly. The top `Local matches` cards were still reading the older aggregated path field, which could display or open an unexpected path when multiple local matches or cached rows were present.

## Regression guard

- `Local matches` cards and repository detail actions now share the same primary path helper.
- CI continues to run formatting, `cargo check --locked`, `cargo test --locked`, and `node --check public/app.js`.
