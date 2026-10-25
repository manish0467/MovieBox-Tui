# Roadmap

## Shipped

**Terminal client** (this repository)

- MovieBox, 4KHDHub, Dramachi, ToonPlay, BDIX (CircleFTP, DhakaFlix) and
  Stremio addons
- MovieBox stream resolution from the subject-details detector links, with a
  fresh signed URL resolved right before playback
- Batch downloads with HTTP range resume, pause and segmented transfers
- Subtitle picker, resolution picker, watch history, favourites, continue
  watching, 9 themes, poster art over Kitty/Sixel
- A headless CLI (`moviebox-cli`) over the same engine for scripts

**Android app** — see [MovieBox-Android](https://github.com/manish0467/MovieBox-Android)

Search, details, sources per resolution, playback with the CDN headers,
subtitles, favourites, watch history, continue watching, provider switching and
downloads.

## Planned

The app's roadmap lives with the app:
[MovieBox-Android/ROADMAP.md](https://github.com/manish0467/MovieBox-Android/blob/main/ROADMAP.md)
— the remaining device work (a real-phone test, the player top-bar inset
fix, store metadata, a native review of the translations) and the later
items (the Stremio addon bridge, casting, custom playlists, per-provider
endpoint overrides).

For the terminal client:

- **Skip intro** (#127) — a player level affordance, matching what the app
  does.
- **Live TV in the terminal client** already exists; it only needs to stay in
  step with the app's playlist handling.

## Blocked or not planned

- **MovieBox series episodes.** Upstream exposes no direct link for series
  episodes — the detector points at index sites that are dead or unreachable.
  The app reports this and points at 4KHDHub or Dramachi. A source-page
  resolver would be unverifiable, so it is not planned.
- **ToonPlay series playback.** The catalog and episode lists come from a
  clean API, but per-episode playback routes through a third-party embed
  whose loader is tamper-protected JavaScript. Movies play fully; series
  playback stays unavailable until that chain changes.
- **flixer.gd.** Investigated (October 2026): the site streams well, but its
  playback URLs are derived by deliberately obfuscated, wasm-backed signing
  against scraping. Deriving them programmatically would be the exact
  brittle-maintenance treadmill this roadmap avoids, so the provider is not
  planned.
