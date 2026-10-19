//! Domain operations exposed to the Android app.
//!
//! Every method returns either a `serde_json::Value` payload or a human
//! readable error message. The JNI layer wraps them into a JSON envelope.

use std::path::PathBuf;

use moviebox_tui::providers::ReleaseProvider;
use moviebox_tui::providers::models::{PlaybackSource, ProviderKind, Release, ResolutionIntent};
use moviebox_tui::service::MovieBoxService;
use serde::Serialize;
use serde_json::{Value, json};

/// Wraps a successful payload into the envelope understood by the Kotlin side.
pub fn ok_envelope<T: Serialize>(data: &T) -> String {
    match serde_json::to_string(&json!({ "ok": true, "data": data })) {
        Ok(text) => text,
        Err(err) => error_envelope(&format!("failed to serialize response: {err}")),
    }
}

/// Wraps an error message into the envelope understood by the Kotlin side.
pub fn error_envelope(message: &str) -> String {
    serde_json::to_string(&json!({ "ok": false, "error": message }))
        .unwrap_or_else(|_| "{\"ok\":false,\"error\":\"unknown error\"}".to_string())
}

/// Installs the Android application directories into the core configuration.
pub fn init_directories(config_dir: &str, data_dir: &str, cache_dir: &str) -> Result<(), String> {
    let overrides = moviebox_tui::config::DirOverrides {
        config: non_empty(config_dir).map(PathBuf::from),
        data: non_empty(data_dir).map(PathBuf::from),
        cache: non_empty(cache_dir).map(PathBuf::from),
    };
    moviebox_tui::config::set_dir_overrides(overrides);

    for dir in [
        moviebox_tui::config::config_dir(),
        moviebox_tui::config::data_dir(),
    ]
    .into_iter()
    .flatten()
    {
        std::fs::create_dir_all(&dir).map_err(|err| format!("cannot create {dir:?}: {err}"))?;
    }
    let cache = moviebox_tui::config::cache_dir();
    std::fs::create_dir_all(&cache).map_err(|err| format!("cannot create {cache:?}: {err}"))?;

    Ok(())
}

/// Headers required to fetch a poster image, so the Kotlin image loader can
/// send the same `User-Agent`/`Referer` the core engine uses.
pub fn poster_headers(url: &str, user_agent: &str) -> String {
    let mut headers = vec![
        ("User-Agent".to_string(), user_agent.to_string()),
        (
            "Referer".to_string(),
            moviebox_tui::providers::moviebox::STREAM_REFERER.to_string(),
        ),
    ];
    if let Some(host) = moviebox_tui::util::extract_host_authority(url) {
        headers.push(("X-Forwarded-Host".to_string(), host));
    }
    ok_envelope(&json!({ "headers": headers }))
}

fn non_empty(value: &str) -> Option<&str> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

fn parse_provider(value: &str) -> Result<ProviderKind, String> {
    ProviderKind::parse(value).ok_or_else(|| format!("unknown provider: {value}"))
}

/// The long-lived bridge state: a tokio runtime plus the provider facade.
pub struct Bridge {
    runtime: tokio::runtime::Runtime,
    service: MovieBoxService,
    /// Maps a provider resource id to the subject it was listed under.
    ///
    /// MovieBox stream links are signed and time limited, so the URL handed
    /// to the player has to be refreshed right before playback. The refresh
    /// needs the subject id, which the Kotlin side does not send back, so the
    /// bridge remembers it from the listing call.
    subjects: std::sync::Mutex<std::collections::HashMap<String, String>>,
}

impl Bridge {
    pub fn new() -> Self {
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(4)
            .thread_name("moviebox-rt")
            .enable_all()
            .build()
            .expect("failed to build tokio runtime");

        // Writes rotating logs into the app-private data directory.
        moviebox_tui::logging::init();

        let service = MovieBoxService::new();
        Self {
            runtime,
            service,
            subjects: std::sync::Mutex::new(std::collections::HashMap::new()),
        }
    }

    /// Remembers which subject every listed resource id belongs to.
    fn remember_subject(&self, subject_id: &str, releases: &[Release]) {
        let mut guard = match self.subjects.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        for release in releases {
            if let Some(resource_id) = release.resource_id.as_deref() {
                guard.insert(resource_id.to_string(), subject_id.to_string());
            }
        }
    }

    /// Returns the subject id a resource id was listed under, if known.
    fn subject_for(&self, resource_id: &str) -> Option<String> {
        let guard = match self.subjects.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard.get(resource_id).cloned()
    }

    /// Lists the providers together with their capabilities.
    pub fn providers(&self) -> String {
        let providers: Vec<Value> = ProviderKind::ENABLED
            .iter()
            .map(|kind| {
                let caps = self.service.capabilities(*kind);
                json!({
                    "id": kind.cache_key(),
                    "label": kind.label(),
                    "supportsSearch": caps.supports_search,
                    "supportsPagination": caps.supports_pagination,
                    "supportsSeries": caps.supports_series,
                    "supportsSubtitles": caps.supports_subtitles,
                    "supportsHomepage": caps.supports_homepage,
                })
            })
            .collect();
        ok_envelope(&json!({ "providers": providers }))
    }

    pub fn search(&self, provider: &str, query: &str, page: usize) -> Result<Value, String> {
        let provider = parse_provider(provider)?;
        let query = query.trim();
        if query.is_empty() {
            return Ok(json!({ "items": [], "page": page }));
        }
        let items = self
            .runtime
            .block_on(self.service.search_typed(provider, query, page))
            .map_err(|err| err.to_string())?;
        Ok(json!({ "items": items, "page": page }))
    }

    pub fn suggest(&self, query: &str) -> Result<Value, String> {
        let query = query.trim();
        if query.is_empty() {
            return Ok(json!({ "items": [] }));
        }
        let items = self.runtime.block_on(self.service.suggest(query))?;
        Ok(json!({ "items": items }))
    }

    pub fn homepage(&self, tab_id: &str, page: usize) -> Result<Value, String> {
        let (items, metrics) = self.runtime.block_on(self.service.homepage(tab_id, page))?;
        Ok(json!({ "items": items, "metrics": metrics, "page": page }))
    }

    pub fn details(&self, provider: &str, subject_id: &str) -> Result<Value, String> {
        let provider = parse_provider(provider)?;
        let details = self
            .runtime
            .block_on(self.service.details_typed(provider, subject_id))
            .map_err(|err| err.to_string())?;
        Ok(json!({ "details": details }))
    }

    pub fn episode_streams(
        &self,
        provider: &str,
        subject_id: &str,
        season: usize,
        episode: usize,
    ) -> Result<Value, String> {
        let provider = parse_provider(provider)?;
        let releases = self
            .runtime
            .block_on(self.fetch_releases(provider, subject_id, season, episode))
            .map_err(|err| err.to_string())?;
        self.remember_subject(subject_id, &releases);
        Ok(json!({ "releases": releases }))
    }

    async fn fetch_releases(
        &self,
        provider: ProviderKind,
        subject_id: &str,
        season: usize,
        episode: usize,
    ) -> Result<Vec<Release>, moviebox_tui::providers::models::ProviderError> {
        match provider {
            ProviderKind::MovieBox => {
                ReleaseProvider::episode_streams(&self.service.client, subject_id, season, episode)
                    .await
            }
            ProviderKind::FourKHdHub => {
                let client = self.service.fourk_client.as_ref().ok_or_else(|| {
                    moviebox_tui::providers::models::ProviderError::Unavailable(
                        "4KHDHub is unavailable".to_string(),
                    )
                })?;
                ReleaseProvider::episode_streams(client, subject_id, season, episode).await
            }
            ProviderKind::BdixCircleFtp => {
                ReleaseProvider::episode_streams(
                    &self.service.circleftp_client,
                    subject_id,
                    season,
                    episode,
                )
                .await
            }
            ProviderKind::BdixDhakaFlix => {
                ReleaseProvider::episode_streams(
                    &self.service.dhakaflix_client,
                    subject_id,
                    season,
                    episode,
                )
                .await
            }
            ProviderKind::Dramachi => {
                ReleaseProvider::episode_streams(
                    &self.service.dramachi_client,
                    subject_id,
                    season,
                    episode,
                )
                .await
            }
            ProviderKind::Addons => {
                Err(moviebox_tui::providers::models::ProviderError::Unavailable(
                    "Addons do not expose episode streams".to_string(),
                ))
            }
        }
    }

    /// Turns a release into a playable source (URL + required headers).
    pub fn resolve_playback(&self, provider: &str, release_json: &str) -> Result<Value, String> {
        let provider = parse_provider(provider)?;
        let release: Release = serde_json::from_str(release_json)
            .map_err(|err| format!("invalid release payload: {err}"))?;

        let source: PlaybackSource = match provider {
            ProviderKind::FourKHdHub => {
                let client = self
                    .service
                    .fourk_client
                    .as_ref()
                    .ok_or_else(|| "4KHDHub is unavailable".to_string())?;
                self.runtime
                    .block_on(client.resolve_release(&release, ResolutionIntent::Playback))
                    .map_err(|err| err.user_message())?
            }
            ProviderKind::Addons => {
                moviebox_tui::providers::addons::release_to_playback_source(&release)
                    .ok_or_else(|| "no playable stream in this release".to_string())?
            }
            ProviderKind::MovieBox => {
                // The listed link is signed and time limited, so refresh it
                // right before the player picks it up.
                let refreshed = release.resource_id.as_deref().and_then(|resource_id| {
                    let subject_id = self.subject_for(resource_id)?;
                    self.runtime.block_on(
                        self.service
                            .refresh_moviebox_source(&subject_id, resource_id),
                    )
                });
                refreshed.unwrap_or(direct_source(&release)?)
            }
            _ => direct_source(&release)?,
        };

        Ok(json!({
            "url": source.url,
            "headers": source.headers,
            "subtitle": source.subtitle,
            "sourceLabel": source.source_label,
            "maxHeight": source.max_height,
            "provider": source.provider.cache_key(),
        }))
    }

    pub fn subtitles(
        &self,
        subject_id: &str,
        resource_id: &str,
        sibling_ids_json: &str,
        season: usize,
        episode: usize,
    ) -> Result<Value, String> {
        let siblings: Vec<String> = serde_json::from_str(sibling_ids_json).unwrap_or_default();
        let captions = self.runtime.block_on(self.service.get_ext_captions(
            subject_id,
            resource_id,
            &siblings,
            season,
            episode,
        ))?;
        Ok(json!({ "captions": captions }))
    }

    pub fn favorites(&self) -> Result<Value, String> {
        let manager = moviebox_tui::favorites::FavoritesManager::new();
        Ok(json!({ "items": manager.items }))
    }

    pub fn toggle_favorite(&self, item_json: &str) -> Result<Value, String> {
        let item: moviebox_tui::favorites::FavoriteItem =
            serde_json::from_str(item_json).map_err(|err| format!("invalid favorite: {err}"))?;
        let mut manager = moviebox_tui::favorites::FavoritesManager::new();
        let added = manager.toggle(item);
        manager.save();
        Ok(json!({ "favorite": added }))
    }

    pub fn history(&self) -> Result<Value, String> {
        let manager = moviebox_tui::history::HistoryManager::new();
        Ok(json!({ "items": manager.recent }))
    }

    /// The `User-Agent` the core engine uses for provider requests.
    pub fn user_agent(&self) -> &str {
        self.service.client.user_agent()
    }
}

/// Builds a playback source from the first mirror of a release.
fn direct_source(release: &Release) -> Result<PlaybackSource, String> {
    let mirror = release
        .mirrors
        .first()
        .ok_or_else(|| "no playable mirrors were found for this release".to_string())?;
    Ok(PlaybackSource {
        provider: release.provider,
        url: mirror.resolver_url.clone(),
        headers: mirror.headers.clone(),
        subtitle: None,
        source_label: mirror.label.clone(),
        max_height: Some(release.resolution_u64()).filter(|&h| h > 0),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_envelope_is_machine_readable() {
        let raw = error_envelope("boom");
        let value: Value = serde_json::from_str(&raw).expect("valid json");
        assert_eq!(value["ok"], json!(false));
        assert_eq!(value["error"], json!("boom"));
    }

    #[test]
    fn ok_envelope_wraps_the_payload() {
        let raw = ok_envelope(&json!({ "items": [1, 2] }));
        let value: Value = serde_json::from_str(&raw).expect("valid json");
        assert_eq!(value["ok"], json!(true));
        assert_eq!(value["data"]["items"], json!([1, 2]));
    }

    #[test]
    fn unknown_providers_are_rejected() {
        assert!(parse_provider("moviebox").is_ok());
        assert!(parse_provider("fourkhdhub").is_ok());
        assert!(parse_provider("nope").is_err());
    }

    #[test]
    fn poster_headers_include_the_stream_referer() {
        let raw = poster_headers("https://cdn.example.com/a.jpg", "moviebox-ua");
        let value: Value = serde_json::from_str(&raw).expect("valid json");
        let headers = value["data"]["headers"].as_array().expect("headers array");
        assert!(headers.iter().any(|entry| entry[0] == json!("User-Agent")));
        assert!(headers.iter().any(|entry| entry[0] == json!("Referer")));
    }
}
