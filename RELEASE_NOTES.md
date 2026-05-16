# RepoAtlas v0.6.4

Clearer scan and fetch progress release.

## Highlights

- Replaced the single progress bar with two progress lanes: `Scan` and `Fetch`.
- Fixed the confusing 88% stall by moving long waits into an explicit `Waiting` state instead of pretending the final stage is still progressing.
- Kept `Refresh` and `Fetch remotes` visually separate so users can tell whether local scanning or remote fetching is active.
- Added responsive stacking for the progress lanes on narrow screens.

## Why

Scan and fetch are different pipelines. A local scan can finish even when network fetch is slow, and a fetch can run later without making local discovery feel stuck.

## Regression guard

- CI continues to run formatting, `cargo check --locked`, `cargo test --locked`, and `node --check public/app.js`.
