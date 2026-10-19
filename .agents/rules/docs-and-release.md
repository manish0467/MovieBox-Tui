---
trigger: model_decision
description: "Use when changing documentation, the changelog, packaging (install.sh, install.ps1, Formula, bucket) or cutting a release for MovieBox-Tui."
---

# Docs, changelog and packaging playbook

Full project guide: `@../AGENTS.md`.

## Documentation

- `docs/` is an mdBook (`book.toml` at the repo root). A new page must be registered in
  `docs/SUMMARY.md`, otherwise it is invisible in the rendered book.
- `docs/changelog.md` and `docs/contributing.md` are **generated copies** of
  `CHANGELOG.md` and `CONTRIBUTING.md`. They are gitignored — do not edit them by hand;
  the CI and the pre-commit hook copy them before building the book.
- The pre-commit hook runs `mdbook build` when `mdbook` is installed. Keep the book
  building: no broken relative links, no missing SUMMARY entries.

## Changelog

`CHANGELOG.md` has an `## [Unreleased]` section with `### Added`, `### Changed` and
`### Fixed` subsections, grouped by bold area headers (for example
`**Providers & Stream Resolution**`). Add user-visible entries there; do not create new
version sections — the release workflow handles versioning.

## Packaging surfaces

Changing the CLI surface (flags, env vars, install paths) usually means updating all of:

| Surface | File |
| :--- | :--- |
| Unix installer | `install.sh` |
| Windows installer | `install.ps1` |
| Homebrew | `Formula/moviebox-tui.rb` |
| Scoop bucket | `bucket/moviebox-tui.json` |
| Cargo package | `Cargo.toml` (`exclude` list, features, `[[bin]]`) |

The `exclude` list in `Cargo.toml` keeps the published crate small. If you add a
top-level directory (for example `.agents/`), add it there too, otherwise
`cargo package` will ship it and may fail to verify.

## Release

Follow `docs/release-checklist.md` end to end — it is the authoritative checklist and
covers the static gates, the desktop playback checks, the terminal rendering checks
and the Termux/Android checks. `cargo package --locked` must succeed for every crate in
the workspace.
