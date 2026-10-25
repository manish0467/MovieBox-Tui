pub mod client;

pub use client::{ToonPlayClient, ToonPlayError};

use super::models::{CatalogItem, MediaDetails, ProviderError, ProviderKind, Release};
use super::{Provider, ProviderCapabilities, ReleaseProvider};

impl Provider for ToonPlayClient {
    fn id(&self) -> ProviderKind {
        ProviderKind::ToonPlay
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            supports_search: true,
            supports_pagination: false,
            supports_series: true,
            supports_subtitles: false,
            supports_homepage: false,
        }
    }

    async fn search(&self, query: &str, _page: usize) -> Result<Vec<CatalogItem>, ProviderError> {
        self.search(query).await.map_err(ProviderError::from)
    }

    async fn details(&self, id: &str) -> Result<MediaDetails, ProviderError> {
        self.details(id).await.map_err(ProviderError::from)
    }
}

impl ReleaseProvider for ToonPlayClient {
    async fn episode_streams(
        &self,
        id: &str,
        season: usize,
        episode: usize,
    ) -> Result<Vec<Release>, ProviderError> {
        self.episode_streams(id, season, episode)
            .await
            .map_err(ProviderError::from)
    }
}
