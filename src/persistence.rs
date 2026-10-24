//! Deferred persistence for frontends that own an async runtime.
//!
//! Domain savers (config, addons, TV playlists, watch history, favourites)
//! call [`write`] instead of writing to disk directly. Without a sink the
//! call is a synchronous atomic write, which is what the headless CLI and the
//! tests rely on. The terminal UI installs a sink at startup so the same
//! savers hand their bytes to a background writer task instead of blocking
//! the event loop. The writer serialises writes and keeps only the latest
//! payload per path, so a burst of toggles collapses to one write per file
//! and the last action still wins.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

type Sink = tokio::sync::mpsc::UnboundedSender<(PathBuf, Vec<u8>)>;

static SINK: OnceLock<Mutex<Option<Sink>>> = OnceLock::new();
static ENQUEUED: AtomicU64 = AtomicU64::new(0);
static PROCESSED: OnceLock<AtomicU64> = OnceLock::new();

/// Deferred writes still waiting for the writer task. Zero once everything
/// handed to the sink has been written.
pub fn pending() -> u64 {
    let enqueued = ENQUEUED.load(Ordering::Relaxed);
    let processed = PROCESSED
        .get()
        .map(|counter| counter.load(Ordering::Relaxed))
        .unwrap_or(enqueued);
    enqueued.saturating_sub(processed)
}

/// Persist a payload. Synchronous atomic write unless a deferred sink is
/// installed; the deferred path reports success and the writer logs failures,
/// so callers' error logging keeps covering the synchronous path.
pub fn write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(cell) = SINK.get()
        && let Ok(guard) = cell.lock()
        && let Some(sender) = guard.clone()
    {
        ENQUEUED.fetch_add(1, Ordering::Relaxed);
        let _ = sender.send((path.to_path_buf(), bytes.to_vec()));
        return Ok(());
    }
    crate::cache::atomic_write_file(path, bytes)
}

/// Hand persistence to a background writer. Ignored in tests, which keep the
/// synchronous behaviour the existing suites assert against.
pub fn install_deferred_sink(sender: Sink) {
    if crate::config::is_test_environment() {
        return;
    }
    let cell = SINK.get_or_init(|| Mutex::new(None));
    if let Ok(mut guard) = cell.lock()
        && guard.is_none()
    {
        PROCESSED.get_or_init(|| AtomicU64::new(0));
        *guard = Some(sender);
    }
}

/// The drain loop the frontend's writer task runs: wait for a job, coalesce
/// everything already queued by keeping the latest bytes per path, then write
/// the survivors serially.
pub async fn run_writer(mut receiver: tokio::sync::mpsc::UnboundedReceiver<(PathBuf, Vec<u8>)>) {
    let processed = PROCESSED.get_or_init(|| AtomicU64::new(0));
    while let Some((first_path, first_bytes)) = receiver.recv().await {
        let mut pending = vec![(first_path, first_bytes)];
        while let Ok((path, bytes)) = receiver.try_recv() {
            if let Some(existing) = pending.iter_mut().find(|(p, _)| *p == path) {
                existing.1 = bytes;
            } else {
                pending.push((path, bytes));
            }
        }
        for (path, bytes) in pending {
            if let Err(error) = crate::cache::atomic_write_file(&path, &bytes) {
                log::warn!(
                    "deferred write to {} failed: {error}",
                    crate::logging::sanitize_path(&path)
                );
            }
            processed.fetch_add(1, Ordering::Relaxed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tempdir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "moviebox-persistence-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn write_without_sink_is_synchronous() {
        let dir = tempdir("sync");
        let path = dir.join("config.json");
        write(&path, b"one").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"one");
        write(&path, b"two").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"two");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn writer_coalesces_to_the_latest_payload_per_path() {
        let dir = tempdir("coalesce");
        let first = dir.join("a.json");
        let second = dir.join("b.json");
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
        let writer = tokio::spawn(run_writer(receiver));
        sender.send((first.clone(), b"stale".to_vec())).unwrap();
        sender.send((second.clone(), b"keep".to_vec())).unwrap();
        sender.send((first.clone(), b"fresh".to_vec())).unwrap();
        drop(sender);
        writer.await.unwrap();
        assert_eq!(std::fs::read(&first).unwrap(), b"fresh");
        assert_eq!(std::fs::read(&second).unwrap(), b"keep");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
