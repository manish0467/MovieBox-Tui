use std::time::Duration;

use reqwest::Client;
use serde::Deserialize;

use crate::providers::models::{
    CatalogItem, Episode, MediaDetails, MediaType, ProviderError, ProviderKind, Release, Season,
    SourceMirror,
};

impl std::fmt::Display for ToonPlayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Network(message) => write!(f, "network error: {message}"),
            Self::Parsing(message) => write!(f, "parse error: {message}"),
            Self::NotFound => write!(f, "not found"),
            Self::Unsupported(message) => write!(f, "{message}"),
        }
    }
}

/// ToonPlay: anime and cartoon catalogue backed by the animesalt API, with
/// playback served through the ravok file host.
///
/// Movies resolve end to end: the details payload carries `watchServers`
/// links (`ravok.buzz/video/<hash>`), and a POST against the player returns
/// a signed HLS manifest. Series metadata is complete, but series playback
/// routes through a third-party embed whose loader is tamper-protected, so
/// `episode_streams` reports it as unavailable until that chain is solved.
const CATALOGUE_BASE: &str = "https://animesalt.streamindia.co.in";
const PLAYER_BASE: &str = "https://ravok.buzz";
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0 Safari/537.36";

#[derive(Debug, Clone)]
pub struct ToonPlayClient {
    client: Client,
}

impl Default for ToonPlayClient {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
pub enum ToonPlayError {
    Network(String),
    Parsing(String),
    NotFound,
    Unsupported(String),
}

impl From<ToonPlayError> for ProviderError {
    fn from(err: ToonPlayError) -> Self {
        match err {
            ToonPlayError::Network(message) => ProviderError::Network(message),
            ToonPlayError::Parsing(message) => ProviderError::Parsing(message),
            ToonPlayError::NotFound => ProviderError::NotFound,
            ToonPlayError::Unsupported(message) => ProviderError::Unavailable(message),
        }
    }
}

impl From<reqwest::Error> for ToonPlayError {
    fn from(err: reqwest::Error) -> Self {
        Self::Network(err.to_string())
    }
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    #[serde(default)]
    data: Vec<SearchItem>,
}

#[derive(Debug, Deserialize)]
struct SearchItem {
    #[serde(default)]
    title: String,
    #[serde(default)]
    image: Option<String>,
    #[serde(rename = "type", default)]
    media_type: String,
    #[serde(default)]
    id: String,
}

#[derive(Debug, Deserialize)]
struct InfoResponse {
    #[serde(default)]
    anime: Option<InfoAnime>,
}

#[derive(Debug, Deserialize)]
struct InfoAnime {
    #[serde(default)]
    id: String,
    #[serde(default)]
    title: String,
    #[serde(rename = "type", default)]
    media_type: String,
    #[serde(default)]
    image: Option<String>,
    #[serde(default)]
    overview: Option<String>,
    #[serde(default)]
    year: Option<String>,
    #[serde(default)]
    genres: Vec<String>,
    #[serde(default)]
    duration: Option<String>,
    #[serde(rename = "watchServers", default)]
    watch_servers: Vec<WatchServer>,
    #[serde(rename = "seasonsList", default)]
    seasons_list: Vec<SeasonListEntry>,
}

#[derive(Debug, Deserialize)]
struct WatchServer {
    #[serde(default)]
    url: String,
}

#[derive(Debug, Deserialize)]
struct SeasonListEntry {
    #[serde(default)]
    season: String,
    #[serde(default)]
    episodes: Vec<SeasonEpisode>,
}

#[derive(Debug, Deserialize)]
struct SeasonEpisode {
    #[serde(default)]
    number: usize,
    #[serde(default)]
    title: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PlaybackResponse {
    #[serde(default)]
    hls: bool,
    #[serde(rename = "videoSource", default)]
    video_source: String,
}

impl ToonPlayClient {
    pub fn new() -> Self {
        let client = crate::net::http_client_builder()
            .user_agent(USER_AGENT)
            .timeout(Duration::from_secs(15))
            .connect_timeout(Duration::from_secs(8))
            .build()
            .unwrap_or_default();
        Self { client }
    }

    pub async fn search(&self, query: &str) -> Result<Vec<CatalogItem>, ToonPlayError> {
        if query.trim().is_empty() {
            return Ok(Vec::new());
        }
        let response: SearchResponse = self
            .client
            .get(format!("{CATALOGUE_BASE}/api/search"))
            .query(&[("q", query.trim())])
            .send()
            .await?
            .json()
            .await
            .map_err(|err| ToonPlayError::Parsing(format!("search response: {err}")))?;

        Ok(response
            .data
            .into_iter()
            .filter(|item| !item.id.is_empty())
            .map(|item| CatalogItem {
                id: crate::providers::models::ProviderMediaId {
                    provider: ProviderKind::ToonPlay,
                    value: item.id,
                },
                title: item.title,
                media_type: parse_media_type(&item.media_type),
                year: None,
                poster_url: item.image.filter(|url| !url.trim().is_empty()),
                season_count: None,
            })
            .collect())
    }

    pub async fn details(&self, id: &str) -> Result<MediaDetails, ToonPlayError> {
        let id = id.trim();
        if id.is_empty() {
            return Err(ToonPlayError::NotFound);
        }
        let anime = self.fetch_info(id).await?;
        let media_type = parse_media_type(&anime.media_type);
        let mut seasons = Vec::new();
        if media_type == MediaType::Series {
            for entry in &anime.seasons_list {
                let number = entry.season.parse::<usize>().unwrap_or(0);
                let episodes = entry
                    .episodes
                    .iter()
                    .map(|episode| Episode {
                        season: number,
                        number: episode.number,
                        title: episode.title.clone(),
                        overview: None,
                    })
                    .collect();
                seasons.push(Season { number, episodes });
            }
        }

        Ok(MediaDetails {
            id: crate::providers::models::ProviderMediaId {
                provider: ProviderKind::ToonPlay,
                value: anime.id,
            },
            title: anime.title,
            media_type,
            year: anime.year.filter(|y| !y.trim().is_empty()),
            description: anime.overview.filter(|s| !s.trim().is_empty()),
            tagline: None,
            imdb_rating: None,
            director: None,
            stars: None,
            prints: None,
            audios: None,
            poster_url: anime.image.filter(|url| !url.trim().is_empty()),
            duration: anime.duration.filter(|d| !d.trim().is_empty()),
            genres: anime
                .genres
                .into_iter()
                .filter(|g| !g.trim().is_empty())
                .collect(),
            seasons,
            dubs: Vec::new(),
        })
    }

    pub async fn episode_streams(
        &self,
        id: &str,
        season: usize,
        episode: usize,
    ) -> Result<Vec<Release>, ToonPlayError> {
        let anime = self.fetch_info(id).await?;
        if parse_media_type(&anime.media_type) == MediaType::Series
            || !anime.seasons_list.is_empty()
        {
            return Err(ToonPlayError::Unsupported(
                "ToonPlay series playback is not supported yet".to_string(),
            ));
        }
        let _ = (season, episode);

        let mut releases = Vec::new();
        for url in anime
            .watch_servers
            .iter()
            .map(|server| server.url.clone())
            .filter(|url| !url.trim().is_empty())
        {
            match self.resolve_watch_server(&url).await {
                Ok(mirror) => {
                    releases.push(Release {
                        provider: ProviderKind::ToonPlay,
                        filename: anime.title.clone(),
                        quality: detect_quality(&anime.title),
                        codec: None,
                        language: detect_language(&anime.title),
                        size_bytes: None,
                        season: None,
                        episode: None,
                        mirrors: vec![mirror],
                        resource_id: None,
                    });
                }
                Err(error) => {
                    log::warn!("toonplay: server {url} failed: {error}");
                }
            }
            if !releases.is_empty() {
                break;
            }
        }
        if releases.is_empty() {
            return Err(ToonPlayError::Parsing(
                "no playable source on the watch servers".to_string(),
            ));
        }
        Ok(releases)
    }

    async fn fetch_info(&self, id: &str) -> Result<InfoAnime, ToonPlayError> {
        let response: InfoResponse = self
            .client
            .get(format!("{CATALOGUE_BASE}/api/info"))
            .query(&[("id", id.trim())])
            .send()
            .await?
            .json()
            .await
            .map_err(|err| ToonPlayError::Parsing(format!("info response: {err}")))?;
        response.anime.ok_or(ToonPlayError::NotFound)
    }

    /// Resolves a `ravok.buzz/video/<hash>` page into a signed HLS manifest.
    async fn resolve_watch_server(&self, url: &str) -> Result<SourceMirror, ToonPlayError> {
        let hash = extract_video_hash(url)
            .ok_or_else(|| ToonPlayError::Parsing(format!("unrecognized watch server: {url}")))?;

        let page_url = format!("{PLAYER_BASE}/video/{hash}");
        let response = self
            .client
            .post(format!("{PLAYER_BASE}/player/index.php"))
            .query(&[("data", hash), ("do", "getVideo")])
            .header("X-Requested-With", "XMLHttpRequest")
            .header("Referer", &page_url)
            .form(&[("hash", hash), ("r", page_url.as_str())])
            .send()
            .await?;

        let playback: PlaybackResponse = response
            .json()
            .await
            .map_err(|err| ToonPlayError::Parsing(format!("playback response: {err}")))?;

        if !playback.hls || playback.video_source.trim().is_empty() {
            return Err(ToonPlayError::Unsupported(
                "the watch server returned a non-HLS source".to_string(),
            ));
        }

        Ok(SourceMirror {
            label: "ToonPlay".to_string(),
            resolver_url: playback.video_source,
            headers: vec![("User-Agent".to_string(), USER_AGENT.to_string())],
            direct_file: true,
        })
    }
}

fn parse_media_type(raw: &str) -> MediaType {
    if raw.eq_ignore_ascii_case("series") {
        MediaType::Series
    } else {
        MediaType::Movie
    }
}

/// The video hash is the last path segment of a `ravok.buzz/video/<hash>` link.
fn extract_video_hash(url: &str) -> Option<&str> {
    let trimmed = url.trim();
    let hash = trimmed.rsplit('/').next()?;
    if hash.len() >= 16 && hash.chars().all(|c| c.is_ascii_alphanumeric()) {
        Some(hash)
    } else {
        None
    }
}

/// ToonPlay titles usually carry the resolution ("1080p HD BluRay").
fn detect_quality(title: &str) -> Option<String> {
    let lower = title.to_ascii_lowercase();
    for candidate in ["2160p", "1080p", "720p", "480p"] {
        if lower.contains(candidate) {
            return Some(candidate.to_string());
        }
    }
    if lower.contains("4k") || lower.contains("uhd") {
        return Some("4k".to_string());
    }
    None
}

fn detect_language(title: &str) -> Option<String> {
    let lower = title.to_ascii_lowercase();
    if lower.contains("multi audio") || lower.contains("multi-audio") {
        Some("Multi Audio".to_string())
    } else if lower.contains("dual audio") || lower.contains("dual-audio") {
        Some("Dual Audio".to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn video_hash_is_the_last_segment() {
        assert_eq!(
            extract_video_hash("https://ravok.buzz/video/15709800bdacf685676ca21dad559a45"),
            Some("15709800bdacf685676ca21dad559a45")
        );
        assert_eq!(extract_video_hash("https://ravok.buzz/video/"), None);
        assert_eq!(extract_video_hash("https://example.com/watch/abc"), None);
    }

    #[test]
    fn media_type_follows_the_api_type() {
        assert_eq!(parse_media_type("movie"), MediaType::Movie);
        assert_eq!(parse_media_type("series"), MediaType::Series);
        assert_eq!(parse_media_type(""), MediaType::Movie);
    }

    #[test]
    fn quality_and_language_come_from_the_title() {
        assert_eq!(
            detect_quality(
                "Doraemon Nobita's New Dinosaur (2020) 1080p HD BluRay Multi Audio ESub"
            ),
            Some("1080p".to_string())
        );
        assert_eq!(
            detect_language(
                "Doraemon Nobita's New Dinosaur (2020) 1080p HD BluRay Multi Audio ESub"
            ),
            Some("Multi Audio".to_string())
        );
        assert_eq!(detect_quality("A plain title"), None);
    }

    #[test]
    fn search_items_map_to_catalog_items() {
        let payload = r#"{"success":true,"data":[
            {"title":"Doraemon: Nobita's New Dinosaur","image":"https://image.tmdb.org/t/p/w500/x.jpg","type":"movie","id":"movies-doraemon-nobitas-new-dinosaur","link":"","categories":[]},
            {"title":"Boruto","image":null,"type":"series","id":"series-boruto","link":"","categories":[]},
            {"title":"","image":null,"type":"movie","id":"","link":"","categories":[]}
        ]}"#;
        let response: SearchResponse = serde_json::from_str(payload).unwrap();
        assert_eq!(response.data.len(), 3);
    }

    #[test]
    fn info_payloads_deserialize_for_movies_and_series() {
        let movie = r#"{"success":true,"anime":{"id":"movies-d","title":"D","type":"movie","image":"i","overview":"o","year":"2020","genres":["a"],"duration":"1h 48m","watchServers":[{"name":"options-0","url":"https://ravok.buzz/video/abc123def4567890"}]}}"#;
        let parsed: InfoResponse = serde_json::from_str(movie).unwrap();
        let anime = parsed.anime.unwrap();
        assert_eq!(anime.watch_servers.len(), 1);

        let series = r#"{"success":true,"anime":{"id":"series-b","title":"B","type":"series","image":null,"overview":null,"year":null,"genres":[],"seasons":"1","seasonsList":[{"season":"1","link":"","episodeCount":2,"episodes":[{"number":1,"title":"B 1x1","id":"episode/b-1x1"}]}]}}"#;
        let parsed: InfoResponse = serde_json::from_str(series).unwrap();
        let anime = parsed.anime.unwrap();
        assert_eq!(anime.seasons_list.len(), 1);
        assert_eq!(anime.seasons_list[0].episodes.len(), 1);
    }

    #[test]
    fn playback_payload_deserializes() {
        let payload = r#"{"hls":true,"videoSource":"https://ravok.buzz/cdn/hls/x/master.m3u8?md5=a&expires=1","videoImage":"","securedLink":"","downloadLinks":[],"ck":"Y"}"#;
        let parsed: PlaybackResponse = serde_json::from_str(payload).unwrap();
        assert!(parsed.hls);
        assert!(parsed.video_source.contains("master.m3u8"));
    }
}
