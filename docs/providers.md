# Streaming Providers

MovieBox-TUI searches and streams media across multiple independent providers.

## Available Providers

| Provider | Description |
| :--- | :--- |
| **MovieBox** | Primary streaming catalog with multi-language audio, seasons, and subtitles. |
| **4KHDHub** | High-bitrate 4K UHD and 1080p releases with fast CDN mirrors. |
| **Dramachi** | Asian dramas and series catalog. |
| **ToonPlay** | Anime and cartoon catalog (movies playable; series listed but playback pending). |
| **CircleFTP** | High-speed local BDIX mirror (Bangladesh ISPs). |
| **DhakaFlix** | Local BDIX media indexer (Bangladesh ISPs). |
| **Addons** | Community Stremio HTTP addons (Cinemeta catalog and streams). |

## Switching Providers

- **Cycle Provider**: Press **`Ctrl+P`** on the home screen to switch to the next provider.
- **Provider Menu**: Click the provider badge in the search bar (or press `Enter` on it) to open the provider selection menu.
- **Enable / Disable Providers**: Open `/settings` → **Content Modes** → **Streaming Sources** to toggle providers on or off.

## BDIX Network Detection

If you are connected through a Bangladeshi ISP supporting BDIX:
- MovieBox-TUI automatically tests local BDIX mirrors on startup and enables them if reachable.
- You can manually re-test your connection anytime via `/settings` → **Maintenance** → **Re-check BDIX Network**.

## Provider Architecture (For Developers)

All providers implement a shared Rust trait in `src/providers/`:

```rust
pub trait Provider: Send + Sync {
    fn id(&self) -> ProviderKind;
    fn capabilities(&self) -> ProviderCapabilities;
    async fn search(&self, query: &str, page: usize) -> Result<Vec<CatalogItem>, ProviderError>;
    async fn details(&self, id: &str) -> Result<MediaDetails, ProviderError>;
}
```

New providers implement `Provider` (and optionally `ReleaseProvider` for multi-mirror streams), then register in `src/service.rs`. Responses are automatically cached as MessagePack envelopes.

### MovieBox CDN Manifest & Failover Resolution

MovieBox CDN stream entries parse signed `CloudFront-Policy` and `Edge-Cache-Cookie` tokens to find stream roots. The manifest resolver accepts only `/dash/` resources. Unhosted legacy `/hls/` cookie targets are dropped to prevent 404 failures during playback or downloads.

### MovieBox Stream Resolution

As of 2026-10 the MovieBox `play-info` endpoint answers with the **site trailer**
(a ~23 second MP4, byte-identical for every title) instead of the media, and the
`resource` endpoint points `resourceLink` at the same file. Those placeholder URLs are
rejected by `is_trailer_or_notice_url()` and never offered as playable mirrors.

Movies are still resolved from the **subject details** endpoint:
`subject-api/get` returns `resourceDetectors[].resolutionList[]`, where every entry
carries a signed CDN link on the `*.hakunaymatata.com` CDN:

```text
https://bcdn.hakunaymatata.com/resource/h265/<hash>.mp4?sign=<token>&t=<expiry>
```

`moviebox_detector_json_to_releases()` turns those entries into releases with the real
file size, duration, codec and per-resolution variants, and `episode_streams()` prefers
them, keeping the `play-info`/`resource` path as a fallback in case upstream restores
signed manifests.

Series detectors carry an empty `resolutionList` and only point at an index site, so
`episode_streams()` falls back to the **H5 web play endpoint**:

```text
https://h5.aoneroom.com/wefeed-h5-bff/web/subject/play?subjectId=<id>&se=<season>&ep=<episode>
```

That endpoint only returns real streams when the request carries the detail page as
`Referer: https://h5.aoneroom.com/movies/<detail-slug>`, where the slug is the last
segment of the subject's `detailUrl` (for example
`breaking-bad-ej6Bp0MCAo7`). `moviebox_detail_slug()` extracts it from the details
payload, and `moviebox_h5_play_to_releases()` turns the `streams[]` entries into
per-resolution releases. The H5 CDN (`bcdnxw.hakunaymatata.com`) answers `429` unless
the media request carries the official web player origin as `Referer`; the generated
mirror headers therefore include `Referer: https://officialmoviebox.com/` together
with the MovieBox user agent.

Two consequences worth knowing:

- **The links are time limited.** The `t=` query is an expiry timestamp, so the URL is
  refreshed right before playback (`MovieBoxService::refresh_moviebox_source`, used by
  the terminal player path and by the Android bridge). The refresh tries the detector
  first and then the H5 endpoint, so both movie and series releases get a fresh link.
  Listed links are never cached.
- **Some titles or episodes have no upstream source.** The H5 endpoint can return an
  empty `streams` array for an unavailable episode, and MovieBox then reports that it
  has no playable source for the title; use **4KHDHub** or **Dramachi** as a fallback.

### MovieBox Placeholder Recovery (resource-position)

When the detector pass yields no playable link, `episode_streams()` also queries
`play-info` and the exact-season `resource/v2` listing. If a resource item still
points at a deprecation notice, it calls the `/resource-position/v2` failover
endpoint with the subject, resource ID, failed URL, and `failCode=404`, then uses
the recovered signed MP4. The recovered mirror is marked as a direct file and is
sent with the MovieBox user agent only; the DASH `Referer` is omitted because
direct MP4s answer HTTP 429. The recovered file's real size replaces the
placeholder size, and the stream cache schema is bumped so stale notice entries
are not reused.
