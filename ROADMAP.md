# Roadmap

What is shipped, what comes next, and what is deliberately not planned. The
Android app is tracked here; the terminal client is tracked in
`CHANGELOG.md`.

## Shipped

**Terminal client**

- MovieBox, 4KHDHub, Dramachi, BDIX (CircleFTP, DhakaFlix) and Stremio addons
- MovieBox stream resolution from the subject-details detector links, with a
  fresh signed URL resolved right before playback
- Batch downloads with HTTP range resume, pause and segmented transfers
- Subtitle picker, resolution picker, watch history, favourites, continue
  watching, 9 themes, poster art over Kitty/Sixel

**Android app** (`android/`, the JNI bridge in `crates/moviebox-android/`)

- Browse the home deck, search, open a title, pick a source per resolution
- Playback with the CDN headers replayed, subtitles with an in-player track
  switcher
- Favourites, watch history, continue watching, provider switching
- Downloads: start, live progress, pause, resume (the signed URL is re-resolved
  when it expires), remove, open, and the finished file is indexed into the
  system media store
- A headless CLI (`moviebox-cli`) over the same engine for scripts

## Next — Live TV

M3U playlist support in the Android app. The terminal client already has TV
mode, so most of the engine exists; the work is bridge plumbing and the app
side.

**Design (settled)**

*The core* — `providers::tv` already provides `M3UParser::fetch_playlist`
(remote **and** local file, 24 h disk cache, 15 MB cap), `parse_m3u` and the
`Channel { id, name, logo, group, stream_url }` model, which is already
`Serialize`.

*One refactor first.* The playlist list currently lives in the terminal UI
(`save_tv_playlists` / `load_tv_playlists_from_config` in `src/tui/app/mod.rs`)
as a JSON array of strings in `<config_dir>/tv_config.json`. Lift it into the
core as `config::load_tv_playlists()` / `config::save_tv_playlists(&[String])`
and let the terminal client call those. The app and the terminal client then
share one playlist file.

*The bridge* — five entry points:

| Function | Purpose |
| :--- | :--- |
| `nativeTvPlaylists()` | The configured playlist URLs and local paths. |
| `nativeTvAddPlaylist(url)` / `nativeTvRemovePlaylist(url)` | Maintain `tv_config.json`. |
| `nativeTvChannels(query, limit)` | Fetch every playlist in parallel, merge, drop empty and duplicate stream URLs, then filter by name and group **in Rust**. |
| `nativeTvResolve(streamUrl)` | The playable URL plus the headers the playlist fetch used. |

Filtering in Rust is deliberate: a 15 MB M3U is tens of thousands of
channels, and shipping all of them across JNI on every keystroke would jank
the UI. The app renders a list, exactly like its other screens.

*The app* — a fifth bottom-navigation destination with the channel list
grouped by `group`, a search field that calls `nativeTvChannels(query)` as you
type, a playlist manager mirroring the terminal client's `/config` popup, and
tap-to-play through the existing player.

*Notes*

- Plain `http://` playlists already work; the Android network security
  configuration permits cleartext.
- Scoped storage means a user cannot point at `/sdcard/…`. Offer *import
  file* through the Storage Access Framework, copy it into the app's directory
  and use that path — the core already handles local files.
- The README mentions "EPG support" for Live TV. There is no EPG code today:
  the parser reads `tvg-id`, `tvg-logo` and `group-title` only. Either
  implement XMLTV parsing or correct the claim; it is deliberately not part of
  this work.

*Effort* — about a day: the core refactor and the bridge are roughly half,
the app side the rest.

## Later

**Player polish.** The player is a bare `PlayerView` today. In priority
order: playback speed, chapter markers with seek-to-chapter, gesture controls
(brightness, volume, seek), subtitle styling (size, colour, background),
picture-in-picture actually driven, a MediaSession so playback continues with
the screen off, and casting.

**Library.** Custom playlists on top of favourites, and per-source quality
preferences so the app remembers the resolution you picked last time.

**Settings screen.** The core owns `config.json` (theme, provider toggles,
download directory, …) but the app has no surface for it. A settings screen
plus a theme set (light, OLED black, and a few accents) would follow the
patterns in the Flow app.

**Distribution.** The published APK is debug-signed, so it cannot replace a
release build. A release keystore, a signed build in CI, and store metadata
are needed for anything beyond sideloading.

**Stremio addons in the app.** The terminal client supports community addons;
the app would need the manifest, catalog and stream providers bridged, plus a
source picker.

## Blocked or not planned

- **MovieBox series episodes.** Upstream exposes no direct link for series
  episodes — the detector points at index sites that are dead or unreachable.
  The app reports this and points at 4KHDHub or Dramachi. A source-page
  resolver would be unverifiable, so it is not planned.
- **A self-updater in the app.** Sideloaded builds update by reinstalling;
  a store build updates through the store.
- **Music, subscriptions and local recommendations.** Those belong to the
  Flow app and have no counterpart in a film catalogue.
