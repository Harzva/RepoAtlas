# RepoAtlas v0.6.5

Faster local scans and less confusing progress feedback.

## Highlights

- Pruned nested scan roots so paths like `D:\study\code`, `D:\study\code\0ai`, and deeper children are not scanned repeatedly.
- Split large scan roots into recursive parallel tasks, preserving root-level markers while still using multiple CPU cores.
- Made `Refresh` lightweight by reading local Git metadata from `.git/config` and `.git/HEAD`; detailed fetch, dirty, ahead, and behind checks now belong to `Fetch remotes`.
- Replaced the fake 96% stall with an explicit `Working / Waiting` state and animated progress while the backend finishes.
- Changed the inactive `Fetch` lane from `0%` to `Not running / --` so users can tell it has not started.

## Why

Local discovery and remote synchronization are separate jobs. RepoAtlas now scans local folders quickly first, then lets users run the heavier network fetch only when they need drift status.

## Regression guard

- Added scan-root pruning and split-task tests.
- CI continues to run formatting, `cargo check --locked`, `cargo test --locked`, and `node --check public/app.js`.
