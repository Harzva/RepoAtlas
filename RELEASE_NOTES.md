# RepoAtlas v0.4.1

Local matching and multi-tag classification patch.

## Highlights

- Added stronger default scan roots, including current working ancestors and the user's `D:\study\code` workspace when present, so local clones such as RepoAtlas are less likely to appear as missing.
- Merged explicit scan roots with default roots instead of replacing them completely.
- Replaced single-category inference with multi-tag inference. A repository can now appear under multiple category filters when it matches multiple contexts.
- Tightened Skills detection so generic prose such as "GBrain skills" does not classify a normal QA/RAG repository as a Skills repository.
- Updated dashboard badges, filters, detail panels, CSV export, and Markdown export to show multiple tags.
- Added regression tests for multi-tag inference, CampusAgent-QA classification, scan-root merging, and visible flattened rows.

## Compatibility

Existing inventories still work because `category` and `categoryLabel` remain as the primary tag for backward compatibility. New scans also include `categories` and `categoryLabels`.

## Regression guard

- CI runs formatting, `cargo check --locked`, `cargo test --locked`, and `node --check public/app.js`.
