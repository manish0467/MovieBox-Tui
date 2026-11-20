//! Downloading an HLS (`.m3u8`) source into one playable file.
//!
//! A plain fetch of a playlist link saves a few kilobytes of text. This module
//! reads the playlist instead, picks a video variant and an audio rendition,
//! fetches their MPEG-TS segments in parallel and writes a single `.ts` file
//! in which the two streams are interleaved at packet level ([`ts_mux`]), with
//! no re-encoding. The output is written in order to one file, so the Android
//! app can stream it straight into a MediaStore descriptor, and a sidecar
//! checkpoint lets a paused download continue where it stopped.

pub mod downloader;
pub mod playlist;
pub mod select;
pub mod ts_mux;

pub use downloader::{HlsRequest, download};
pub use playlist::is_playlist;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum HlsError {
    #[error("network error: {0}")]
    Network(String),
    #[error("server returned HTTP {0}")]
    Http(u16),
    /// The signed link no longer works; resolving the source again fixes it.
    #[error("the stream link expired (HTTP {0})")]
    Expired(u16),
    #[error("bad playlist: {0}")]
    Playlist(String),
    #[error("bad segment: {0}")]
    Segment(String),
    #[error("this stream cannot be downloaded: {0}")]
    Unsupported(String),
    #[error("file error: {0}")]
    File(#[from] std::io::Error),
}

/// True when `url` names an HLS playlist by its path.
pub fn is_playlist_url(url: &str) -> bool {
    let path = url.split(['?', '#']).next().unwrap_or_default();
    path.to_ascii_lowercase().ends_with(".m3u8")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_playlist_links() {
        assert!(is_playlist_url(
            "https://ravok.buzz/cdn/hls/abc/master.m3u8?md5=x&expires=1"
        ));
        assert!(is_playlist_url("https://a.test/INDEX.M3U8"));
        assert!(!is_playlist_url("https://a.test/film.mp4?x=.m3u8"));
        assert!(!is_playlist_url("https://a.test/film.mkv"));
    }
}
