# Roadmap

## Shipped

**Terminal client** (this repository)

- MovieBox, 4KHDHub, Dramachi, BDIX (CircleFTP, DhakaFlix) and Stremio addons
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
— Live TV (the design is written out there), the player polish work, the
settings screen, release signing and the Stremio addon bridge.

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
