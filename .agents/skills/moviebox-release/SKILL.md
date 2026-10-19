---
name: moviebox-release
description: "Cuts a MovieBox-Tui release: runs the static gates, verifies the changelog and packaging surfaces, and walks docs/release-checklist.md before tagging. Use when the user asks to release, tag, publish or cut a version of MovieBox-Tui."
---

# Cutting a MovieBox-Tui release

The authoritative checklist is `docs/release-checklist.md` — read it and work through it
section by section. This skill adds the decision flow and the traps.

## Step 1 — Static gates

```bash
cargo fmt --check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo check --all-targets --all-features --locked
cargo test --all-features --locked
cargo audit
cargo package --locked
```

All must pass before anything else. `cargo package --locked` must succeed for **both**
workspace crates (`moviebox-tui` and `moviebox-android`).

## Step 2 — Changelog and version

- `CHANGELOG.md` holds an `## [Unreleased]` section. Every user-visible change belongs
  there under `### Added` / `### Changed` / `### Fixed`.
- The release workflow owns version bumps and the GitHub release body. Do not hand-edit
  version numbers in `Cargo.toml` unless the maintainer asks.

## Step 3 — Packaging surfaces

If the CLI surface changed, confirm the installers match: `install.sh`, `install.ps1`,
`Formula/moviebox-tui.rb`, `bucket/moviebox-tui.json`.

## Step 4 — Runtime checks

Static gates are not enough for this project: player launch, terminal rendering and the
Termux chooser depend on the real runtime environment. Follow sections 2–5 of
`docs/release-checklist.md` (desktop playback, terminal rendering, Termux/Android,
downloads).

## Traps

- **Cross-compiled targets are not executed by CI.** The release workflow builds them and
  runs a `--version` smoke test, but Windows ARM64 is never executed (x64 runners cannot
  run it). Smoke-test it manually before certifying.
- **Four `player::tests::*` / playback tests fail on machines with `mpv`/`vlc`**
  installed because they assume an Android/Termux environment. They fail on an
  unmodified checkout too — do not treat them as a regression, and do not weaken them.
- **`docs/changelog.md` and `docs/contributing.md` are generated copies.** Never edit
  them; they are gitignored and rebuilt before `mdbook build`.
- **New top-level directories must be added to the `exclude` list in `Cargo.toml`** or
  `cargo package` will include them.
- The pre-commit hook (`git config core.hooksPath .githooks`) runs `cargo fmt --check`,
  `cargo clippy --all-targets --locked -- -D warnings` and `mdbook build`. Fix the
  failures; do not bypass the hook unless the user explicitly says so.
