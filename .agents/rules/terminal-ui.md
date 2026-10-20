---
trigger: glob
globs: "src/tui/**, src/player.rs, src/player/**, src/proxy.rs, src/updater/**"
description: "Rules for the terminal frontend: the message-driven Action bus, event-loop discipline and player launch behaviour."
---

# Terminal frontend rules

Full project guide: `@../AGENTS.md`. These rules apply to the ratatui/crossterm layer
(`src/tui/`, `src/player.rs`, `src/proxy.rs`, `src/updater/`).

## The event loop is the contract

`App::run` (in `src/tui/app/run.rs`) owns the only loop. It draws when `state.dirty`
and then waits on a `select!` over the `Action` channel. Therefore:

- **Never block the event loop.** Disk I/O, image decoding, M3U parsing and any other
  blocking work belongs in `tokio::task::spawn_blocking`.
- **New behaviour = a new `Action` variant** plus a `handle_*` method in the matching
  `src/tui/app/*.rs` module. `handle_action` stays a thin router.
- **All `AppState` mutations happen inside `handle_action`**, on the event-loop task.
  Background tasks send `Action` messages back through the channel; they never touch
  `AppState` directly.
- Render functions (`src/tui/screens/`, `src/tui/widgets/`) are pure: they read state
  and draw. They must not perform I/O or mutate state.

## Shared helpers

Anything the core engine also needs (duration parsing, language labels, header rules)
belongs in `src/util.rs`, not `src/tui/text.rs`. `src/tui/text.rs` re-exports those
helpers so the terminal code keeps its current import paths.

## Player launch

- `src/player.rs` detects the player and builds the command; `src/proxy.rs` serves
  authenticated CloudFront streams over localhost. The stream headers are mandatory,
  but which ones depend on the host: the signed manifest hosts want the `Referer`
  and the CloudFront cookie, while the MovieBox detector links want the app user
  agent and reject a `Referer` with 429.
- On Termux the player is dispatched through Android intents (`termux-open` / `am`).
  Android-intent playback must keep forwarding `User-Agent`, `Referer` and subtitles.

## Testing note

Four tests in this area fail on machines that have `mpv`/`vlc` installed, because they
assume an Android/Termux environment. They fail identically on an unmodified checkout —
verify the baseline before changing them, and never weaken the assertions.
