# RepoAtlas v0.3.1

Regression fix for empty first-run inventories.

## Fixes

- Automatically starts a refresh when the embedded inventory is still empty, so a fresh install does not sit at `0` repositories until the user guesses the next step.
- Shows refresh and account errors as a persistent in-page banner instead of only a short toast.
- Treats `current gh login`, `current`, and similar account text as the active GitHub CLI login, preventing the helper text from becoming a broken account alias.
- Clarifies the account input placeholder: leave it empty for the current `gh` login, or enter multiple accounts such as `Harzva` and `saihao` on separate lines.
- Adds regression tests for multi-account parsing and category inference in CI.

## Included from v0.3.0

- Multi-account scanning with `accounts[]`, `REPO_ATLAS_ACCOUNTS`, and newline/comma/semicolon parsing.
- Automatic repository categories: Skills, MCP, Memory, Software, Docs, Infra, Data, Research, Games, and Other.
- Custom RepoAtlas logo, account chips, category filters, and four themes: Atlas, Midnight, Paper, and Aurora.
- Custom desktop window icon.
- Windows and macOS release packaging through GitHub Actions.
