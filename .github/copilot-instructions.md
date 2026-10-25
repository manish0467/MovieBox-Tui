# Copilot instructions

Read `AGENTS.md` at the repo root before making changes — it is the source of truth for
the architecture, the verified build/test commands and the invariants that must not be
broken.

Quick rules:
- Rust 1.90+, edition 2024. `cargo fmt --all`, `cargo clippy --all-targets
  --all-features --locked -- -D warnings`, `cargo test --workspace`.
- Core code must not depend on `src/tui/`; the terminal UI sits behind the `tui` Cargo
  feature because the Android app links the crate with `default-features = false`.
- The terminal UI is message-driven (`Action` variants handled in `src/tui/app/`); never
  block the event loop.
- Replay `User-Agent`, `Referer` and the CloudFront cookie on every stream request.
- No panics on network or user-input paths. Zero telemetry. Conventional Commits.
