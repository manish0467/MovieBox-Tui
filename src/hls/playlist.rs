//! HLS playlist parsing: just what a VOD download needs.
//!
//! A master playlist gives the video variants and the separate audio
//! renditions; a media playlist gives the segments. Encrypted, fMP4,
//! byte-range and live playlists are refused with a clear reason, since the
//! downloader can only join plain MPEG-TS segments.

use url::Url;

use super::HlsError;

#[derive(Debug, Clone, PartialEq)]
pub struct Variant {
    pub uri: String,
    pub bandwidth: u64,
    pub height: Option<u32>,
    pub name: Option<String>,
    /// The `AUDIO` group the variant takes its sound from, when separate.
    pub audio_group: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Rendition {
    pub group_id: String,
    pub language: Option<String>,
    pub name: String,
    pub default: bool,
    pub uri: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MasterPlaylist {
    pub variants: Vec<Variant>,
    pub audio: Vec<Rendition>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    pub uri: String,
    pub duration: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MediaPlaylist {
    pub segments: Vec<Segment>,
}

impl MediaPlaylist {
    pub fn duration(&self) -> f64 {
        self.segments.iter().map(|segment| segment.duration).sum()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Playlist {
    Master(MasterPlaylist),
    Media(MediaPlaylist),
}

/// True when `body` is an HLS playlist (it may start with a byte order mark).
pub fn is_playlist(body: &[u8]) -> bool {
    let body = body.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(body);
    let start = body
        .iter()
        .position(|byte| !byte.is_ascii_whitespace())
        .unwrap_or(body.len());
    body[start..].starts_with(b"#EXTM3U")
}

/// Parses `text`, resolving every link against `base` (the playlist's own URL).
pub fn parse(text: &str, base: &Url) -> Result<Playlist, HlsError> {
    if !is_playlist(text.as_bytes()) {
        return Err(HlsError::Playlist("not an HLS playlist".to_string()));
    }
    let lines: Vec<&str> = text
        .lines()
        .map(|line| line.trim().trim_start_matches('\u{feff}'))
        .filter(|line| !line.is_empty())
        .collect();
    if lines
        .iter()
        .any(|line| line.starts_with("#EXT-X-STREAM-INF"))
    {
        parse_master(&lines, base).map(Playlist::Master)
    } else {
        parse_media(&lines, base).map(Playlist::Media)
    }
}

fn parse_master(lines: &[&str], base: &Url) -> Result<MasterPlaylist, HlsError> {
    let mut variants = Vec::new();
    let mut audio = Vec::new();
    let mut pending: Option<Vec<(String, String)>> = None;
    for line in lines {
        if let Some(rest) = line.strip_prefix("#EXT-X-MEDIA:") {
            let attrs = attributes(rest);
            if attr(&attrs, "TYPE").is_some_and(|kind| kind.eq_ignore_ascii_case("AUDIO")) {
                audio.push(Rendition {
                    group_id: attr(&attrs, "GROUP-ID").unwrap_or_default().to_string(),
                    language: attr(&attrs, "LANGUAGE").map(str::to_string),
                    name: attr(&attrs, "NAME").unwrap_or_default().to_string(),
                    default: attr(&attrs, "DEFAULT").is_some_and(|v| v.eq_ignore_ascii_case("YES")),
                    uri: attr(&attrs, "URI")
                        .map(|uri| resolve(base, uri))
                        .transpose()?,
                });
            }
        } else if let Some(rest) = line.strip_prefix("#EXT-X-STREAM-INF:") {
            pending = Some(attributes(rest));
        } else if !line.starts_with('#') {
            if let Some(attrs) = pending.take() {
                let height = attr(&attrs, "RESOLUTION")
                    .and_then(|res| res.split_once(['x', 'X']))
                    .and_then(|(_, h)| h.parse().ok());
                variants.push(Variant {
                    uri: resolve(base, line)?,
                    bandwidth: attr(&attrs, "BANDWIDTH")
                        .and_then(|b| b.parse().ok())
                        .unwrap_or(0),
                    height,
                    name: attr(&attrs, "NAME").map(str::to_string),
                    audio_group: attr(&attrs, "AUDIO").map(str::to_string),
                });
            }
        }
    }
    if variants.is_empty() {
        return Err(HlsError::Playlist(
            "the playlist lists no video".to_string(),
        ));
    }
    Ok(MasterPlaylist { variants, audio })
}

fn parse_media(lines: &[&str], base: &Url) -> Result<MediaPlaylist, HlsError> {
    let mut segments = Vec::new();
    let mut duration: Option<f64> = None;
    let mut ended = false;
    for line in lines {
        if let Some(rest) = line.strip_prefix("#EXTINF:") {
            let value = rest.split(',').next().unwrap_or_default().trim();
            duration = Some(value.parse().unwrap_or(0.0));
        } else if let Some(rest) = line.strip_prefix("#EXT-X-KEY:") {
            let attrs = attributes(rest);
            if attr(&attrs, "METHOD").is_some_and(|m| !m.eq_ignore_ascii_case("NONE")) {
                return Err(HlsError::Unsupported("the stream is encrypted".to_string()));
            }
        } else if line.starts_with("#EXT-X-MAP") {
            return Err(HlsError::Unsupported("fMP4 segments".to_string()));
        } else if line.starts_with("#EXT-X-BYTERANGE") {
            return Err(HlsError::Unsupported("byte-range segments".to_string()));
        } else if line.starts_with("#EXT-X-ENDLIST") {
            ended = true;
        } else if !line.starts_with('#') {
            segments.push(Segment {
                uri: resolve(base, line)?,
                duration: duration.take().unwrap_or(0.0),
            });
        }
    }
    if !ended {
        return Err(HlsError::Unsupported("a live stream".to_string()));
    }
    if segments.is_empty() {
        return Err(HlsError::Playlist(
            "the playlist has no segments".to_string(),
        ));
    }
    Ok(MediaPlaylist { segments })
}

fn resolve(base: &Url, link: &str) -> Result<String, HlsError> {
    base.join(link)
        .map(String::from)
        .map_err(|err| HlsError::Playlist(format!("bad link {link}: {err}")))
}

fn attr<'a>(attrs: &'a [(String, String)], key: &str) -> Option<&'a str> {
    attrs
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(key))
        .map(|(_, value)| value.as_str())
}

/// Splits `KEY=VALUE,KEY="quoted, value"` into pairs, quotes removed.
fn attributes(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut rest = text;
    while !rest.is_empty() {
        let Some((key, after)) = rest.split_once('=') else {
            break;
        };
        let (value, next) = if let Some(quoted) = after.strip_prefix('"') {
            match quoted.split_once('"') {
                Some((value, tail)) => (value, tail.strip_prefix(',').unwrap_or(tail)),
                None => (quoted, ""),
            }
        } else {
            match after.split_once(',') {
                Some((value, tail)) => (value, tail),
                None => (after, ""),
            }
        };
        out.push((key.trim().to_string(), value.to_string()));
        rest = next;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Url {
        Url::parse("https://host.test/cdn/hls/abc/master.m3u8?md5=x&expires=1").unwrap()
    }

    // The shape ToonPlay serves (Spirited Away), links shortened.
    const MASTER: &str = "#EXTM3U\n#EXT-X-VERSION:3\n#\n#EXTM3U\n\
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"audio\",LANGUAGE=\"eng\",NAME=\"English\",DEFAULT=NO,AUTOSELECT=YES,URI=\"/hls/en\"\n\
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"audio\",LANGUAGE=\"jpn\",NAME=\"Japanese\",DEFAULT=NO,AUTOSELECT=YES,URI=\"/hls/ja\"\n\
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"audio\",LANGUAGE=\"hin\",NAME=\"Hindi\",DEFAULT=NO,AUTOSELECT=YES,URI=\"/hls/hi\"\n\
#EXT-X-STREAM-INF:PROGRAM-ID=1,BANDWIDTH=150000,RESOLUTION=426x240,NAME=\"240p\",CODECS=\"avc1.4D400D,mp4a.40.2\",AUDIO=\"audio\"\n/hls/v240\n\
#EXT-X-STREAM-INF:PROGRAM-ID=1,BANDWIDTH=2048000,RESOLUTION=1280x720,NAME=\"720p\",CODECS=\"avc1.4D401F,mp4a.40.2\",AUDIO=\"audio\"\n/hls/v720\n\
#EXT-X-STREAM-INF:PROGRAM-ID=1,BANDWIDTH=4096000,RESOLUTION=1920x1080,NAME=\"1080p\",CODECS=\"avc1.640028,mp4a.40.2\",AUDIO=\"audio\"\n/hls/v1080%3D\n";

    #[test]
    fn parses_variants_and_audio_renditions() {
        let Playlist::Master(master) = parse(MASTER, &base()).unwrap() else {
            panic!("expected a master playlist");
        };
        assert_eq!(master.variants.len(), 3);
        assert_eq!(master.variants[0].height, Some(240));
        assert_eq!(master.variants[1].bandwidth, 2_048_000);
        assert_eq!(master.variants[2].uri, "https://host.test/hls/v1080%3D");
        assert_eq!(master.variants[2].audio_group.as_deref(), Some("audio"));
        assert_eq!(master.audio.len(), 3);
        assert_eq!(master.audio[1].language.as_deref(), Some("jpn"));
        assert_eq!(
            master.audio[0].uri.as_deref(),
            Some("https://host.test/hls/en")
        );
        assert!(!master.audio[0].default);
    }

    #[test]
    fn parses_a_media_playlist() {
        let text = "#EXTM3U\n#EXT-X-TARGETDURATION:2\n#EXT-X-MEDIA-SEQUENCE:0\n\
#EXTINF:2.002133,\nhttps://cdn1.test/p/a.js\n#EXTINF:1.5,\nseg2.css\n#EXT-X-ENDLIST\n";
        let Playlist::Media(media) = parse(text, &base()).unwrap() else {
            panic!("expected a media playlist");
        };
        assert_eq!(media.segments.len(), 2);
        assert_eq!(media.segments[0].uri, "https://cdn1.test/p/a.js");
        assert_eq!(
            media.segments[1].uri,
            "https://host.test/cdn/hls/abc/seg2.css"
        );
        assert!((media.duration() - 3.502133).abs() < 1e-9);
    }

    #[test]
    fn refuses_what_cannot_be_joined() {
        let encrypted =
            "#EXTM3U\n#EXT-X-KEY:METHOD=AES-128,URI=\"k\"\n#EXTINF:2,\na.ts\n#EXT-X-ENDLIST\n";
        assert!(matches!(
            parse(encrypted, &base()),
            Err(HlsError::Unsupported(_))
        ));
        let fmp4 = "#EXTM3U\n#EXT-X-MAP:URI=\"init.mp4\"\n#EXTINF:2,\na.m4s\n#EXT-X-ENDLIST\n";
        assert!(matches!(
            parse(fmp4, &base()),
            Err(HlsError::Unsupported(_))
        ));
        let live = "#EXTM3U\n#EXTINF:2,\na.ts\n";
        assert!(matches!(
            parse(live, &base()),
            Err(HlsError::Unsupported(_))
        ));
        let clear = "#EXTM3U\n#EXT-X-KEY:METHOD=NONE\n#EXTINF:2,\na.ts\n#EXT-X-ENDLIST\n";
        assert!(parse(clear, &base()).is_ok());
    }

    #[test]
    fn recognises_a_playlist_body() {
        assert!(is_playlist(b"#EXTM3U\n"));
        assert!(is_playlist(b"\xEF\xBB\xBF\n  #EXTM3U"));
        assert!(!is_playlist(b"\x47\x40\x11\x10"));
        assert!(!is_playlist(b""));
    }

    #[test]
    fn splits_quoted_attributes() {
        let attrs = attributes("A=1,CODECS=\"avc1,mp4a\",NAME=\"x\"");
        assert_eq!(attr(&attrs, "codecs"), Some("avc1,mp4a"));
        assert_eq!(attr(&attrs, "NAME"), Some("x"));
        assert_eq!(attr(&attrs, "A"), Some("1"));
    }
}
