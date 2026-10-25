# CONVENTIONS.md

Shared agent guide for this repository. **Read `AGENTS.md` at the repo root before
making changes** — it contains the architecture, the verified build/test commands and
the invariants that must not be broken.

Key points:

- Rust 1.90+, edition 2024. Format with `cargo fmt --all`; lint with
  `cargo clippy --all-targets --all-features --locked -- -D warnings`; test with
  `cargo test --workspace`.
- The terminal UI is gated behind the `tui` Cargo feature (default on). The Android app
  links the crate with `default-features = false`, so core code must not import from
  `src/tui/` — put shared helpers in `src/util.rs`.
- The UI is message-driven (`Action` variants handled in `src/tui/app/`). Never block
  the event loop; mutate `AppState` only inside `handle_action`.
- Streams require the `User-Agent`, `Referer` and CloudFront cookie headers to be
  replayed on every request.
- No panics on network or user-input paths; zero telemetry.
- Conventional Commits; update `CHANGELOG.md` (`## [Unreleased]`) and
  `docs/SUMMARY.md` when adding documentation.
