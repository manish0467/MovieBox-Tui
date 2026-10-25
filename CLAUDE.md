# CLAUDE.md

This project uses a shared agent guide. **Read `AGENTS.md` at the repo root before doing
anything** — it contains the architecture, the build/test commands and the invariants
that must not be broken.

Critical rules (full detail in `AGENTS.md`):

- Rust 1.90+, edition 2024. `cargo fmt --all`, `cargo clippy --all-targets
  --all-features --locked -- -D warnings`, `cargo test --workspace`.
- The terminal UI lives behind the `tui` Cargo feature (default on). The Android app
  links the crate with `default-features = false`, so **core code must never depend on
  `src/tui/`** — shared helpers belong in `src/util.rs`.
- The UI is message-driven: add an `Action` variant and a `handle_*` method in
  `src/tui/app/`; never block the event loop. All `AppState` mutations stay on the
  event-loop task.
- Stream playback requires `User-Agent`, `Referer` and the CloudFront cookie to be
  replayed on every request.
- Never panic on network or user-input paths. Zero telemetry.
- Conventional Commits. Update `CHANGELOG.md` (`## [Unreleased]`) and
  `docs/SUMMARY.md` when adding docs.
