# AGENTS.md

Working notes for coding agents on **MovieBox-Tui**. This file is the single source of
truth; the agent-specific files (`CLAUDE.md`, `GEMINI.md`, `CONVENTIONS.md`,
`.cursorrules`, `.cursor/rules/*.mdc`, `.github/copilot-instructions.md`, `.windsurfrules`,
`QWEN.md`) only point back here.

## How each agent finds this file

| Agent | Reads |
| :--- | :--- |
| **Antigravity** (`agy` CLI / IDE / App) | `AGENTS.md` natively, plus `.agents/rules/*.md` (scoped, frontmatter-activated) and `.agents/skills/*/SKILL.md` |
| **Codex** | `AGENTS.md` natively |
| **opencode / Cline / Roo / Auggie / Factory** | `AGENTS.md` natively |
| **Claude Code** | `CLAUDE.md` → points here |
| **Gemini CLI** (legacy) | `GEMINI.md` → points here (kept short: Antigravity loads both files) |
| **Cursor** | `.cursorrules` + `.cursor/rules/moviebox.mdc` → point here |
| **GitHub Copilot** | `.github/copilot-instructions.md` → points here |
| **Aider** | `CONVENTIONS.md` → points here |
| **Windsurf** | `.windsurfrules` → points here |
| **Qwen Code** | `QWEN.md` → points here |

### Antigravity specifics

- `AGENTS.md` and `GEMINI.md` are loaded **without frontmatter** and are always active
  for their directory scope. Everything in `AGENTS.md` is therefore in the prompt on
  every turn — keep it lean.
- Scoped rules live in `.agents/rules/*.md` and **require** YAML frontmatter with a
  valid `trigger` (`always_on`, `model_decision`, `glob` or `manual`); a file without
  frontmatter is silently discarded.
  - `.agents/rules/android.md` — `model_decision`: the JNI bridge, Gradle/Compose build.
  - `.agents/rules/terminal-ui.md` — `glob` on `src/tui/**, src/player.rs, src/proxy.rs, src/updater/**`.
  - `.agents/rules/docs-and-release.md` — `model_decision`: changelog, mdBook, packaging.
- Multi-step workflows are skills: `.agents/skills/moviebox-release/SKILL.md`.
- Limits: 24 KB per rule file, 20 000 tokens for all always-on rules combined. If
  `AGENTS.md` grows, move detail into `.agents/rules/`.

## What this repository is

MovieBox-Tui is a Rust terminal client (and now a native Android app) for finding,
downloading and streaming movies, TV series, anime and live TV from several providers
(MovieBox, 4KHDHub, Dramachi, BDIX mirrors, community Stremio addons, M3U/IPTV playlists).

- Rust **1.90+**, edition **2024**. License **MIT OR Apache-2.0**. Zero telemetry.
- Two frontends over one engine: a terminal UI and a Jetpack Compose Android app.

## Layout

```text
src/                      Core engine (always compiled) + terminal frontend (tui feature)
  cache.rs config.rs download.rs favorites.rs history.rs logging.rs
  models.rs net.rs providers/ service.rs util.rs        <-- core: no UI dependency
  player.rs proxy.rs tui/ updater.rs main.rs             <-- `tui` feature only
crates/moviebox-android/  JNI bridge (cdylib) used by the Android app
android/                  Gradle + Compose + Media3 Android app
docs/                     mdBook source (book.toml at the repo root)
tests/                    Integration tests
install.sh install.ps1 Formula/ bucket/   Distribution packaging
```

## Orientation (read before changing anything)

| Document | Covers |
| :--- | :--- |
| `docs/architecture.md` | Event loop, async model, data flow, crate layers |
| `docs/modules.md` | Module-by-module responsibility map |
| `docs/providers.md` | Provider behaviour and capabilities |
| `docs/players.md` | Player detection and header handling |
| `docs/downloads.md` | Download engine, ranges, resume |
| `docs/tv-mode.md` | Live TV / M3U handling |
| `docs/addons-mode.md` | Stremio addon mode |
| `docs/android.md` | Android app, JNI surface, build steps |

## Build, test and lint commands (verified on this repo)

```bash
cargo build                                  # terminal app (default features)
cargo build --no-default-features             # headless core only (what Android links)
cargo test --workspace                        # unit tests
cargo nextest run --workspace --all-features  # CI parity (install cargo-nextest)
cargo fmt --all                              # format (rustfmt defaults)
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo audit                                  # dependency audit
cargo package --locked                        # publishes the root crate only
mdbook build                                 # docs (copy CHANGELOG/CONTRIBUTING into docs/ first)
```

The workspace has two members: the published crate and `crates/moviebox-android`
(the JNI bridge). The bridge depends on the published crate **by path**, so it
must never be packaged against the registry release — that is why
`default-members` is only the root package. Use `--workspace` for check, clippy
and test so the bridge is covered.

Pre-commit hook (enable once with `git config core.hooksPath .githooks`): runs
`cargo fmt --check`, `cargo clippy --all-targets --locked -- -D warnings` and
`mdbook build`. The hook fails the commit — fix, don't bypass it (unless the user says so).

## Cargo features

- `tui` (**default**) — the terminal frontend: `tui`, `player`, `proxy`, `updater` modules
  and the `ratatui`/`crossterm`/`image`/`lru`/`unicode-*`/`tar`/`zip`/`sha2`/`semver`
  dependencies.
- `cargo build --no-default-features` produces the headless engine only. The Android
  bridge depends on the crate with `default-features = false`, so **the shared library
  must never link the terminal frontend**.
- Workspace: root package + `crates/moviebox-android` (only the root package is a
  default member, see the build commands above).

## Architecture invariants — do not break these

1. **Message-driven UI.** Never block the event loop. New behaviour = a new `Action`
   variant handled by a `handle_*` method in `src/tui/app/*.rs`. `handle_action` in
   `app/run.rs` is a thin router; keep it thin.
2. **Single-threaded state.** All `AppState` mutations happen inside `handle_action` on
   the event-loop task. Background tasks send `Action` messages back to the channel.
3. **Blocking work** (disk I/O, image decoding, M3U parsing) runs in
   `tokio::task::spawn_blocking`.
4. **Provider contract.** Providers implement `Provider` / `ReleaseProvider` from
   `src/providers/models.rs`. Domain models are `serde` `Serialize`/`Deserialize` — the
   wire format is shared with the Android app, so field names and optionality are API.
5. **Stream auth headers are mandatory, and the set depends on the CDN.** The
   `*.hakunaymatata.com` links behind the MovieBox detector want the MovieBox app
   user agent and answer `429` when a `Referer` is sent; the signed manifest hosts
   want the `Referer` and the CloudFront cookie. Dropping a header that a host
   expects yields 403s, 428s or 429s and an "unplayable" stream.
6. **No panics** on network or user-input paths. Return errors instead.
7. **Zero telemetry.** All state stays on local disk.
8. Core code must not depend on the terminal. Anything the core needs lives in
   `src/util.rs` (durations, language labels, header rules), not in `src/tui/text.rs`.

## Android app specifics

- **JNI contract:** Rust exports `Java_com_moviebox_app_core_NativeBridge_<fn>` in
  `crates/moviebox-android/src/lib.rs`; the Kotlin declarations live in
  `android/app/src/main/kotlin/com/moviebox/app/core/NativeBridge.kt` (`@JvmStatic
  external fun`). **Change both sides together** or the app crashes with
  `UnsatisfiedLinkError`.
- **Payload format:** every bridge call returns a JSON envelope
  `{"ok":true,"data":…}` / `{"ok":false,"error":"…"}` (see `ok_envelope` /
  `error_envelope` in `crates/moviebox-android/src/api.rs`). Kotlin decodes it in
  `core/MovieBoxRepository.kt`.
- **Directories:** Android has no XDG dirs. The app passes app-private paths to
  `nativeInit`, which installs `config::set_dir_overrides`.
- **DNS:** on Android the engine uses the system resolver (`netd`); the public-DNS
  fallback in `src/net.rs` is `cfg(not(target_os = "android"))` on purpose — do not
  re-enable it there.
- **Cleartext HTTP** is enabled through `res/xml/network_security_config.xml` because
  BDIX mirrors serve plain HTTP.
- **Build order:** build the `.so` first (`cargo ndk -t arm64-v8a -o
  android/app/src/main/jniLibs build --release`), then `./gradlew assembleDebug`.
  `-PmovieboxBuildRust=true` makes Gradle drive cargo for you.
- `android/app/src/main/jniLibs/` is generated — never commit it.

## Repository remotes and upstream

- `origin` is the fork (`Alenbosh/Box`), `upstream` is the original project
  (`mesamirh/MovieBox-Tui`).
- Fixes for the terminal client belong in a focused branch that can be opened as
  a pull request against `upstream`. That branch must **not** contain the Android
  app, the agent files or any other unrelated work — a small reviewable diff is
  the whole point.
- The Android app is distributed as an APK from a GitHub release on the fork; it
  is not published to crates.io.

## Style, commits and scope

- Follow idiomatic Rust and `rustfmt` defaults. Do not hand-format.
- Conventional Commits (`feat:`, `fix:`, `refactor:`, `docs:`, `chore:`, `perf:`, `style:`).
- Keep changes focused; one concern per PR. Do not mix refactors with behaviour changes.
- Do not add dependencies without a good reason (say why in the PR).
- Never commit `target/`, `android/app/build/`, `android/app/src/main/jniLibs/`, editor
  settings or debug dumps.
- If you add a top-level directory, add it to the `exclude` list in `Cargo.toml` so
  `cargo package` keeps working.
- `docs/` is an mdBook: new pages must be registered in `docs/SUMMARY.md`.
- `CHANGELOG.md` has an `## [Unreleased]` section — add entries under
  `### Added` / `### Changed` / `### Fixed`.

## Test hygiene

The player detection tests used to fail on any machine that has `mpv`/`vlc`
installed, because they assumed a Termux environment. They now inject fixture
openers and restore the environment afterwards, and the two timing-dependent
flakes (the HTTP probe budget, the `SIGHUP` race in the detached-process test)
are fixed as well.

When a test fails, check the baseline first (`git stash`) before changing it —
and never fix a test by weakening its assertions.

## Before you say "done"

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace
cargo build --no-default-features
```

For Android changes also confirm:

```bash
cargo build -p moviebox-android --target aarch64-linux-android --release
cd android && ./gradlew assembleDebug
```

Report which commands you ran and their results. If something could not be run
(no NDK, no device, no network), say so explicitly instead of implying it passed.
