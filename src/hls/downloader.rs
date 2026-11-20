//! Fetches an HLS source and writes it to one file, with resume.

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use futures::StreamExt;
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use tokio::fs::File;
use tokio::io::{AsyncSeekExt, AsyncWriteExt};
use tokio::sync::mpsc;
use url::Url;

use super::HlsError;
use super::playlist::{self, MediaPlaylist, Playlist};
use super::select;
use super::ts_mux::{self, InterleaveState, Interleaver};
use crate::download::{DownloadOutcome, DownloadProgress};

const MAX_ATTEMPTS: usize = 4;
const VIDEO_WORKERS: usize = 4;
const AUDIO_WORKERS: usize = 2;
/// Segments between two checkpoints (about 20 s of a 2 s-segment film).
const CHECKPOINT_EVERY: usize = 10;
const MAX_PLAYLIST_BYTES: usize = 8 * 1024 * 1024;
const CHECKPOINT_VERSION: u32 = 1;

/// What to download and where its resume state lives.
#[derive(Debug, Clone, Copy)]
pub struct HlsRequest<'a> {
    /// The master (or media) playlist.
    pub url: &'a str,
    /// The headers the source needs, sent with every request.
    pub headers: &'a [(String, String)],
    /// The height to aim for; see [`select::pick_variant`].
    pub max_height: Option<u32>,
    /// The audio language (ISO 639-1/2 or a name); see [`select::pick_audio`].
    pub audio_language: Option<&'a str>,
    /// The sidecar file that records progress for a resume.
    pub checkpoint: &'a Path,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Checkpoint {
    version: u32,
    /// The variant, the rendition and the segment counts: a resume of a
    /// different selection starts over.
    key: String,
    bytes: u64,
    next_video: usize,
    done_seconds: f64,
    interleave: InterleaveState,
}

/// The streams to fetch.
struct Plan {
    video: MediaPlaylist,
    /// The separate audio, when the variant has none of its own.
    audio: Option<MediaPlaylist>,
    key: String,
}

/// Downloads `request` into `file`, resuming from the checkpoint when it
/// matches. The file is written strictly in order, so it can be a
/// descriptor that cannot be reopened (a MediaStore pending file).
pub async fn download<F>(
    client: &Client,
    request: HlsRequest<'_>,
    mut file: File,
    cancel: Arc<AtomicBool>,
    mut report: F,
) -> Result<DownloadOutcome, HlsError>
where
    F: FnMut(DownloadProgress),
{
    let plan = plan(client, &request).await?;
    let total_seconds = plan.video.duration();
    let video_count = plan.video.segments.len();
    let audio_count = plan.audio.as_ref().map_or(0, |a| a.segments.len());

    let file_len = file.metadata().await?.len();
    let mut state = match read_checkpoint(request.checkpoint).await {
        Some(saved) if saved.key == plan.key && saved.bytes <= file_len => saved,
        _ => Checkpoint {
            version: CHECKPOINT_VERSION,
            key: plan.key.clone(),
            bytes: 0,
            next_video: 0,
            done_seconds: 0.0,
            interleave: InterleaveState::default(),
        },
    };
    // Whatever came after the last checkpoint is written again.
    file.set_len(state.bytes).await?;
    file.seek(std::io::SeekFrom::Start(state.bytes)).await?;

    let video_urls = urls(&plan.video);
    let mut video = spawn_fetcher(
        client,
        request.headers,
        video_urls,
        state.next_video,
        VIDEO_WORKERS,
        &cancel,
    );
    let mut audio = plan.audio.as_ref().map(|audio| {
        spawn_fetcher(
            client,
            request.headers,
            urls(audio),
            state.interleave.audio_front,
            AUDIO_WORKERS,
            &cancel,
        )
    });
    let mut interleaver = Interleaver::new(state.interleave);

    let started = Instant::now();
    let session_start = state.bytes;
    let mut last_report = Instant::now() - Duration::from_secs(1);
    let mut buffer = Vec::new();

    for index in state.next_video..video_count {
        if cancel.load(Ordering::Relaxed) {
            return pause(&mut file, request.checkpoint, &state).await;
        }
        let bytes = receive(&mut video, index).await?;
        buffer.clear();
        match audio.as_mut() {
            Some(audio) => {
                let segment = ts_mux::demux(&bytes)?;
                let until = Interleaver::video_until(&segment);
                while interleaver.needs_audio(until) && interleaver.audio_next() < audio_count {
                    let next = interleaver.audio_next();
                    let audio_bytes = receive(audio, next).await?;
                    interleaver.push_audio(next, ts_mux::demux(&audio_bytes)?);
                }
                interleaver.write_video(segment, &mut buffer);
            }
            // The variant carries its own sound: its segments join as they are.
            None => buffer.extend_from_slice(&bytes),
        }
        if index + 1 == video_count
            && let Some(audio) = audio.as_mut()
        {
            // The tail of the audio, after the last picture.
            while interleaver.audio_next() < audio_count {
                let next = interleaver.audio_next();
                let audio_bytes = receive(audio, next).await?;
                interleaver.push_audio(next, ts_mux::demux(&audio_bytes)?);
            }
            interleaver.finish(&mut buffer);
        }
        file.write_all(&buffer).await?;
        state.bytes += buffer.len() as u64;
        state.next_video = index + 1;
        state.done_seconds += plan.video.segments[index].duration;
        state.interleave = interleaver.state();

        if state.next_video % CHECKPOINT_EVERY == 0 {
            file.flush().await?;
            file.sync_data().await?;
            write_checkpoint(request.checkpoint, &state).await?;
        }
        if last_report.elapsed() >= Duration::from_millis(500) {
            last_report = Instant::now();
            report(progress(&state, total_seconds, session_start, started));
        }
    }

    file.flush().await?;
    file.sync_all().await?;
    let _ = tokio::fs::remove_file(request.checkpoint).await;
    report(DownloadProgress {
        downloaded: state.bytes,
        total: Some(state.bytes),
        bytes_per_second: 0.0,
        attempt: 1,
        workers: VIDEO_WORKERS,
    });
    Ok(DownloadOutcome::Completed { bytes: state.bytes })
}

async fn pause(
    file: &mut File,
    checkpoint: &Path,
    state: &Checkpoint,
) -> Result<DownloadOutcome, HlsError> {
    file.flush().await?;
    file.sync_data().await?;
    write_checkpoint(checkpoint, state).await?;
    Ok(DownloadOutcome::Paused { bytes: state.bytes })
}

/// The size so far, and the whole size estimated from the share of the
/// running time already written.
fn progress(
    state: &Checkpoint,
    total_seconds: f64,
    session_start: u64,
    started: Instant,
) -> DownloadProgress {
    let total = (state.done_seconds > 0.0 && total_seconds > 0.0)
        .then(|| (state.bytes as f64 * total_seconds / state.done_seconds) as u64);
    let elapsed = started.elapsed().as_secs_f64();
    DownloadProgress {
        downloaded: state.bytes,
        total,
        bytes_per_second: if elapsed > 0.0 {
            state.bytes.saturating_sub(session_start) as f64 / elapsed
        } else {
            0.0
        },
        attempt: 1,
        workers: VIDEO_WORKERS,
    }
}

async fn plan(client: &Client, request: &HlsRequest<'_>) -> Result<Plan, HlsError> {
    let (text, base) = fetch_playlist(client, request.url, request.headers).await?;
    match playlist::parse(&text, &base)? {
        Playlist::Media(video) => Ok(Plan {
            key: format!("media|{}", video.segments.len()),
            video,
            audio: None,
        }),
        Playlist::Master(master) => {
            let variant = select::pick_variant(&master.variants, request.max_height)
                .ok_or_else(|| HlsError::Playlist("the playlist lists no video".into()))?;
            let rendition = variant
                .audio_group
                .as_deref()
                .and_then(|group| select::pick_audio(&master.audio, group, request.audio_language));
            let video = media(client, &variant.uri, request.headers).await?;
            let audio = match rendition.and_then(|r| r.uri.as_deref()) {
                Some(uri) => Some(media(client, uri, request.headers).await?),
                None => None,
            };
            let key = format!(
                "{}x{}|{}|{}|{}|{}",
                variant.height.unwrap_or(0),
                variant.bandwidth,
                rendition.map_or("", |r| r.name.as_str()),
                rendition.and_then(|r| r.language.as_deref()).unwrap_or(""),
                video.segments.len(),
                audio.as_ref().map_or(0, |a| a.segments.len()),
            );
            Ok(Plan { video, audio, key })
        }
    }
}

async fn media(
    client: &Client,
    url: &str,
    headers: &[(String, String)],
) -> Result<MediaPlaylist, HlsError> {
    let (text, base) = fetch_playlist(client, url, headers).await?;
    match playlist::parse(&text, &base)? {
        Playlist::Media(media) => Ok(media),
        Playlist::Master(_) => Err(HlsError::Playlist(
            "a variant is itself a master playlist".into(),
        )),
    }
}

async fn fetch_playlist(
    client: &Client,
    url: &str,
    headers: &[(String, String)],
) -> Result<(String, Url), HlsError> {
    let base = Url::parse(url).map_err(|err| HlsError::Playlist(format!("bad link: {err}")))?;
    let never = AtomicBool::new(false);
    let body = fetch(client, url, headers, &never).await?;
    if body.len() > MAX_PLAYLIST_BYTES {
        return Err(HlsError::Playlist("the playlist is too large".into()));
    }
    let text = String::from_utf8(body)
        .map_err(|_| HlsError::Playlist("the playlist is not text".into()))?;
    Ok((text, base))
}

fn urls(media: &MediaPlaylist) -> Vec<String> {
    media
        .segments
        .iter()
        .map(|segment| segment.uri.clone())
        .collect()
}

type Fetched = Result<(usize, Vec<u8>), HlsError>;

/// Fetches `urls[start..]` in the background, `workers` at a time, and hands
/// them over in order. Dropping the receiver stops it.
fn spawn_fetcher(
    client: &Client,
    headers: &[(String, String)],
    urls: Vec<String>,
    start: usize,
    workers: usize,
    cancel: &Arc<AtomicBool>,
) -> mpsc::Receiver<Fetched> {
    let (sender, receiver) = mpsc::channel(workers * 2);
    let client = client.clone();
    let headers = Arc::new(headers.to_vec());
    let cancel = Arc::clone(cancel);
    tokio::spawn(async move {
        let mut fetches = futures::stream::iter(start..urls.len())
            .map(|index| {
                let client = client.clone();
                let headers = Arc::clone(&headers);
                let cancel = Arc::clone(&cancel);
                let url = urls[index].clone();
                async move {
                    fetch(&client, &url, &headers, &cancel)
                        .await
                        .map(|bytes| (index, bytes))
                }
            })
            .buffered(workers);
        while let Some(result) = fetches.next().await {
            let failed = result.is_err();
            if sender.send(result).await.is_err() || failed {
                break;
            }
        }
    });
    receiver
}

async fn receive(
    receiver: &mut mpsc::Receiver<Fetched>,
    index: usize,
) -> Result<Vec<u8>, HlsError> {
    match receiver.recv().await {
        Some(Ok((got, bytes))) if got == index => Ok(bytes),
        Some(Ok((got, _))) => Err(HlsError::Segment(format!(
            "segment {got} arrived for {index}"
        ))),
        Some(Err(error)) => Err(error),
        None => Err(HlsError::Network("the segment download stopped".into())),
    }
}

/// One GET with retries. A refused or vanished link is not retried for long:
/// the signed source has to be resolved again.
async fn fetch(
    client: &Client,
    url: &str,
    headers: &[(String, String)],
    cancel: &AtomicBool,
) -> Result<Vec<u8>, HlsError> {
    let mut last = HlsError::Network("no attempt".into());
    for attempt in 1..=MAX_ATTEMPTS {
        if attempt > 1 {
            if cancel.load(Ordering::Relaxed) {
                return Err(last);
            }
            tokio::time::sleep(Duration::from_secs(1 << (attempt - 2))).await;
        }
        let mut builder = client.get(url);
        for (name, value) in headers {
            builder = builder.header(name.as_str(), value.as_str());
        }
        match builder.send().await {
            Ok(response) if response.status().is_success() => match response.bytes().await {
                Ok(bytes) => return Ok(bytes.to_vec()),
                Err(error) => last = HlsError::Network(error.to_string()),
            },
            Ok(response) => {
                let status = response.status();
                if is_expired(status) {
                    // One more try covers a CDN hiccup; a second refusal is final.
                    if matches!(last, HlsError::Expired(_)) {
                        return Err(HlsError::Expired(status.as_u16()));
                    }
                    last = HlsError::Expired(status.as_u16());
                } else {
                    last = HlsError::Http(status.as_u16());
                }
            }
            Err(error) => last = HlsError::Network(error.to_string()),
        }
    }
    Err(last)
}

fn is_expired(status: StatusCode) -> bool {
    matches!(status.as_u16(), 401 | 403 | 404 | 410)
}

async fn read_checkpoint(path: &Path) -> Option<Checkpoint> {
    let text = tokio::fs::read(path).await.ok()?;
    serde_json::from_slice::<Checkpoint>(&text)
        .ok()
        .filter(|checkpoint| checkpoint.version == CHECKPOINT_VERSION)
}

async fn write_checkpoint(path: &Path, state: &Checkpoint) -> Result<(), HlsError> {
    let text = serde_json::to_vec(state).map_err(|err| std::io::Error::other(err.to_string()))?;
    let temporary = path.with_extension("tmp");
    tokio::fs::write(&temporary, text).await?;
    tokio::fs::rename(&temporary, path).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::ts_mux::tests::segment;
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;
    use tokio::io::AsyncReadExt;
    use tokio::net::TcpListener;

    /// A tiny HTTP server: path → body, with a count of requests per path and
    /// an optional path that fails until `allow` is set.
    struct Server {
        base: String,
        hits: Arc<Mutex<HashMap<String, usize>>>,
    }

    async fn serve(routes: HashMap<String, Vec<u8>>) -> Server {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let routes = Arc::new(routes);
        let hits = Arc::new(Mutex::new(HashMap::new()));
        let counter = Arc::clone(&hits);
        tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    break;
                };
                let routes = Arc::clone(&routes);
                let counter = Arc::clone(&counter);
                tokio::spawn(async move {
                    let mut request = vec![0u8; 4096];
                    let read = socket.read(&mut request).await.unwrap_or(0);
                    let text = String::from_utf8_lossy(&request[..read]).to_string();
                    let path = text.split_whitespace().nth(1).unwrap_or("/").to_string();
                    let agent_ok = text
                        .to_ascii_lowercase()
                        .contains("user-agent: moviebox-test");
                    *counter.lock().unwrap().entry(path.clone()).or_insert(0) += 1;
                    let response = match routes.get(&path) {
                        Some(body) if agent_ok => {
                            let mut head = format!(
                                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                body.len()
                            )
                            .into_bytes();
                            head.extend_from_slice(body);
                            head
                        }
                        Some(_) => b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec(),
                        None => b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec(),
                    };
                    let _ = socket.write_all(&response).await;
                });
            }
        });
        Server { base, hits }
    }

    fn headers() -> Vec<(String, String)> {
        vec![("User-Agent".into(), "moviebox-test".into())]
    }

    fn video_segment(start: u64) -> Vec<u8> {
        segment(
            0x1b,
            &[],
            &(0..4).map(|i| (start + i * 3750, 400)).collect::<Vec<_>>(),
        )
    }

    fn audio_segment(start: u64) -> Vec<u8> {
        segment(
            0x0f,
            &[],
            &(0..8).map(|i| (start + i * 3750, 100)).collect::<Vec<_>>(),
        )
    }

    /// A master with two variants and two languages; 25 video segments of
    /// 1/6 s and 13 audio segments covering them.
    fn routes() -> HashMap<String, Vec<u8>> {
        let mut routes = HashMap::new();
        routes.insert(
            "/master.m3u8".to_string(),
            b"#EXTM3U\n\
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"a\",LANGUAGE=\"eng\",NAME=\"English\",URI=\"/en.m3u8\"\n\
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"a\",LANGUAGE=\"jpn\",NAME=\"Japanese\",URI=\"/ja.m3u8\"\n\
#EXT-X-STREAM-INF:BANDWIDTH=100,RESOLUTION=426x240,AUDIO=\"a\"\n/v240.m3u8\n\
#EXT-X-STREAM-INF:BANDWIDTH=900,RESOLUTION=1280x720,AUDIO=\"a\"\n/v720.m3u8\n"
                .to_vec(),
        );
        let mut video = String::from("#EXTM3U\n");
        for i in 0..25u64 {
            video.push_str(&format!("#EXTINF:0.1667,\n/v/{i}.js\n"));
            routes.insert(format!("/v/{i}.js"), video_segment(i * 15_000));
        }
        video.push_str("#EXT-X-ENDLIST\n");
        routes.insert("/v720.m3u8".into(), video.into_bytes());
        let mut audio = String::from("#EXTM3U\n");
        for i in 0..13u64 {
            audio.push_str(&format!("#EXTINF:0.34,\n/a/{i}.css\n"));
            routes.insert(format!("/a/{i}.css"), audio_segment(i * 30_000));
        }
        audio.push_str("#EXT-X-ENDLIST\n");
        routes.insert("/ja.m3u8".into(), audio.into_bytes());
        routes
    }

    async fn run(
        server: &Server,
        directory: &Path,
        cancel: Arc<AtomicBool>,
    ) -> (Result<DownloadOutcome, HlsError>, Vec<u8>) {
        let output = directory.join("film.ts");
        let file = tokio::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&output)
            .await
            .unwrap();
        let checkpoint = directory.join("film.ts.hls.json");
        let url = format!("{}/master.m3u8", server.base);
        let headers = headers();
        let request = HlsRequest {
            url: &url,
            headers: &headers,
            max_height: Some(1080),
            audio_language: Some("ja"),
            checkpoint: &checkpoint,
        };
        let flag = Arc::clone(&cancel);
        let mut reports = 0;
        let result = download(&Client::new(), request, file, cancel, |_| {
            reports += 1;
            if reports == 1 {
                flag.store(true, Ordering::Relaxed);
            }
        })
        .await;
        (result, tokio::fs::read(&output).await.unwrap())
    }

    #[tokio::test]
    async fn downloads_the_chosen_variant_and_language_into_one_file() {
        let server = serve(routes()).await;
        let directory = tempfile::tempdir().unwrap();
        // The progress callback pauses each run on its first report, so the
        // film is written over many resumed runs.
        let mut pauses = 0;
        let mut last_len = 0;
        let (written, output) = loop {
            let (result, bytes) =
                run(&server, directory.path(), Arc::new(AtomicBool::new(false))).await;
            assert!(bytes.len() >= last_len, "a resume lost data");
            last_len = bytes.len();
            match result.unwrap() {
                DownloadOutcome::Paused { bytes: at } => {
                    assert_eq!(at as usize, bytes.len());
                    assert!(directory.path().join("film.ts.hls.json").exists());
                    pauses += 1;
                    assert!(pauses < 100, "no progress");
                }
                DownloadOutcome::Completed { bytes: written } => break (written, bytes),
            }
        };
        assert!(pauses >= 2);
        assert_eq!(written as usize, output.len());
        assert!(!directory.path().join("film.ts.hls.json").exists());

        // The same bytes as a download that was never paused.
        let fresh = tempfile::tempdir().unwrap();
        let output_path = fresh.path().join("film.ts");
        let file = File::create(&output_path).await.unwrap();
        let url = format!("{}/master.m3u8", server.base);
        let headers = headers();
        let checkpoint = fresh.path().join("cp.json");
        let request = HlsRequest {
            url: &url,
            headers: &headers,
            max_height: Some(1080),
            audio_language: Some("ja"),
            checkpoint: &checkpoint,
        };
        download(
            &Client::new(),
            request,
            file,
            Arc::new(AtomicBool::new(false)),
            |_| {},
        )
        .await
        .unwrap();
        assert_eq!(tokio::fs::read(&output_path).await.unwrap(), output);

        // 720p (the tallest within 1080) and Japanese; never English or 240p.
        let hits = server.hits.lock().unwrap();
        assert!(hits.contains_key("/v720.m3u8") && hits.contains_key("/ja.m3u8"));
        assert!(!hits.contains_key("/v240.m3u8") && !hits.contains_key("/en.m3u8"));
        // Both streams are in the file.
        let pids: std::collections::HashSet<u16> = output
            .chunks_exact(ts_mux::PACKET)
            .map(|p| (u16::from(p[1] & 0x1f) << 8) | u16::from(p[2]))
            .collect();
        assert!(pids.contains(&ts_mux::VIDEO_PID) && pids.contains(&ts_mux::AUDIO_PID));
    }

    #[tokio::test]
    async fn a_refused_segment_reports_an_expired_link() {
        let mut routes = routes();
        routes.remove("/v/3.js");
        let server = serve(routes).await;
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("film.ts");
        let file = File::create(&output).await.unwrap();
        let url = format!("{}/master.m3u8", server.base);
        let headers = headers();
        let checkpoint = directory.path().join("cp.json");
        let request = HlsRequest {
            url: &url,
            headers: &headers,
            max_height: None,
            audio_language: Some("ja"),
            checkpoint: &checkpoint,
        };
        let result = download(
            &Client::new(),
            request,
            file,
            Arc::new(AtomicBool::new(false)),
            |_| {},
        )
        .await;
        assert!(matches!(result, Err(HlsError::Expired(404))), "{result:?}");
        // Segments before the missing one were fetched; the playlists were fine.
        assert!(server.hits.lock().unwrap().contains_key("/v/2.js"));
    }

    #[tokio::test]
    async fn the_source_headers_are_sent() {
        let server = serve(routes()).await;
        let directory = tempfile::tempdir().unwrap();
        let file = File::create(directory.path().join("film.ts"))
            .await
            .unwrap();
        let url = format!("{}/master.m3u8", server.base);
        let checkpoint = directory.path().join("cp.json");
        let request = HlsRequest {
            url: &url,
            headers: &[],
            max_height: None,
            audio_language: None,
            checkpoint: &checkpoint,
        };
        let result = download(
            &Client::new(),
            request,
            file,
            Arc::new(AtomicBool::new(false)),
            |_| {},
        )
        .await;
        assert!(matches!(result, Err(HlsError::Expired(403))), "{result:?}");
    }
}
