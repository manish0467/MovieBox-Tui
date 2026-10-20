package com.moviebox.app.core

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/** Mirrors `moviebox_tui::providers::models::ProviderMediaId`. */
@Serializable
data class ProviderMediaId(
    val provider: String,
    val value: String,
)

/** Mirrors `moviebox_tui::providers::models::CatalogItem`. */
@Serializable
data class CatalogItem(
    val id: ProviderMediaId,
    val title: String,
    @SerialName("media_type") val mediaType: String,
    val year: String? = null,
    @SerialName("poster_url") val posterUrl: String? = null,
    @SerialName("season_count") val seasonCount: Int? = null,
) {
    val isSeries: Boolean get() = mediaType == "series"
}

/** Mirrors `moviebox_tui::providers::models::Episode`. */
@Serializable
data class Episode(
    val season: Int,
    val number: Int,
    val title: String? = null,
    val overview: String? = null,
)

/** Mirrors `moviebox_tui::providers::models::Season`. */
@Serializable
data class Season(
    val number: Int,
    val episodes: List<Episode> = emptyList(),
)

/** Mirrors `moviebox_tui::providers::models::AudioTrackOption`. */
@Serializable
data class AudioTrackOption(
    @SerialName("subject_id") val subjectId: String,
    val language: String,
    val label: String,
)

/** Mirrors `moviebox_tui::providers::models::MediaDetails`. */
@Serializable
data class MediaDetails(
    val id: ProviderMediaId,
    val title: String,
    @SerialName("media_type") val mediaType: String,
    val year: String? = null,
    val description: String? = null,
    val tagline: String? = null,
    @SerialName("imdb_rating") val imdbRating: String? = null,
    val director: String? = null,
    val stars: String? = null,
    val prints: String? = null,
    val audios: String? = null,
    @SerialName("poster_url") val posterUrl: String? = null,
    val duration: String? = null,
    val genres: List<String> = emptyList(),
    val seasons: List<Season> = emptyList(),
    val dubs: List<AudioTrackOption> = emptyList(),
) {
    val isSeries: Boolean get() = mediaType == "series" || seasons.isNotEmpty()
}

/** Mirrors `moviebox_tui::providers::models::SourceMirror`. */
@Serializable
data class SourceMirror(
    val label: String,
    @SerialName("resolver_url") val resolverUrl: String,
    val headers: List<List<String>> = emptyList(),
    @SerialName("direct_file") val directFile: Boolean = false,
)

/** Mirrors `moviebox_tui::providers::models::Release`. */
@Serializable
data class Release(
    val provider: String,
    val filename: String,
    val quality: String? = null,
    val codec: String? = null,
    val language: String? = null,
    @SerialName("size_bytes") val sizeBytes: Long? = null,
    val season: Int? = null,
    val episode: Int? = null,
    val mirrors: List<SourceMirror> = emptyList(),
    @SerialName("resource_id") val resourceId: String? = null,
) {
    val sourceLabel: String get() = mirrors.firstOrNull()?.label ?: provider
}

/** Mirrors `moviebox_tui::providers::models::SubtitleOption`. */
@Serializable
data class SubtitleOption(
    val name: String,
    val url: String,
)

/** Mirrors `moviebox_tui::favorites::FavoriteItem`. */
@Serializable
data class FavoriteItem(
    val provider: String,
    @SerialName("subject_id") val subjectId: String,
    val title: String,
    @SerialName("cover_url") val coverUrl: String? = null,
    val stype: Int = 1,
    @SerialName("release_year") val releaseYear: String = "",
    @SerialName("added_at") val addedAt: Long = 0,
) {
    val isSeries: Boolean get() = stype == 2
}

/** Mirrors `moviebox_tui::history::WatchHistoryItem`. */
@Serializable
data class WatchHistoryItem(
    val provider: String = "",
    @SerialName("subject_id") val subjectId: String = "",
    val title: String = "",
    @SerialName("cover_url") val coverUrl: String? = null,
    val season: Int = 0,
    val episode: Int = 0,
    @SerialName("progress_seconds") val progressSeconds: Long = 0,
    @SerialName("duration_seconds") val durationSeconds: Long? = null,
    val timestamp: Long = 0,
    val completed: Boolean = false,
    val stype: Long = 1,
    @SerialName("release_year") val releaseYear: String = "",
    @SerialName("stream_filename") val streamFilename: String? = null,
) {
    val progressFraction: Float
        get() {
            val dur = durationSeconds ?: return 0f
            return if (dur > 0) (progressSeconds.toFloat() / dur.toFloat()).coerceIn(0f, 1f) else 0f
        }

    val isSeries: Boolean
        get() = stype == 2L || season > 0 || episode > 0

    val isInProgress: Boolean
        get() = !completed && progressSeconds >= 10 && (durationSeconds == null || progressSeconds < (durationSeconds * 0.90).toLong())
}

/** Mirrors `moviebox_tui::history::PendingPlaybackState` for position reporting. */
@Serializable
data class PendingPlaybackState(
    val provider: String,
    @SerialName("subject_id") val subjectId: String,
    val season: Int,
    val episode: Int,
    @SerialName("progress_seconds") val progressSeconds: Long,
    @SerialName("duration_seconds") val durationSeconds: Long? = null,
    val completed: Boolean = false,
    val timestamp: Long = 0,
    val title: String? = null,
    @SerialName("cover_url") val coverUrl: String? = null,
    val stype: Long? = null,
    @SerialName("release_year") val releaseYear: String? = null,
    @SerialName("stream_filename") val streamFilename: String? = null,
)

/** Provider descriptor returned by `nativeProviders`. */
@Serializable
data class ProviderInfo(
    val id: String,
    val label: String,
    @SerialName("supportsSearch") val supportsSearch: Boolean = true,
    @SerialName("supportsPagination") val supportsPagination: Boolean = false,
    @SerialName("supportsSeries") val supportsSeries: Boolean = true,
    @SerialName("supportsSubtitles") val supportsSubtitles: Boolean = true,
    @SerialName("supportsHomepage") val supportsHomepage: Boolean = false,
)

/** Playback descriptor returned by `nativeResolvePlayback`. */
@Serializable
data class PlaybackSource(
    val url: String,
    val headers: List<List<String>> = emptyList(),
    val subtitle: String? = null,
    @SerialName("sourceLabel") val sourceLabel: String = "",
    @SerialName("maxHeight") val maxHeight: Long? = null,
    val provider: String = "",
) {
    val headerMap: Map<String, String>
        get() = headers.mapNotNull { pair ->
            if (pair.size >= 2) pair[0] to pair[1] else null
        }.toMap()
}

/* ------------------------------------------------------------------ *
 * Envelope payloads
 * ------------------------------------------------------------------ */

@Serializable
data class ProvidersPayload(val providers: List<ProviderInfo> = emptyList())

@Serializable
data class DetailsPayload(val details: MediaDetails)

@Serializable
data class ReleasesPayload(val releases: List<Release> = emptyList())

@Serializable
data class SubtitlesPayload(val captions: List<SubtitleOption> = emptyList())

@Serializable
data class ToggleFavoritePayload(val favorite: Boolean = false)

@Serializable
data class HeadersPayload(val headers: List<List<String>> = emptyList())

/* ------------------------------------------------------------------ *
 * Download DTOs and payloads
 * ------------------------------------------------------------------ */

/** Mirrors `moviebox_android::downloads::DownloadRequest`. */
@Serializable
data class DownloadRequest(
    val url: String,
    val headers: List<List<String>> = emptyList(),
    val title: String,
    val provider: String = "",
    @SerialName("subjectId") val subjectId: String = "",
    val season: Int = 0,
    val episode: Int = 0,
    val destination: String? = null,
)

/** Mirrors `moviebox_android::downloads::JobProgress`. */
@Serializable
data class DownloadProgress(
    val downloaded: Long = 0L,
    val total: Long? = null,
    val bytesPerSecond: Double = 0.0,
    val attempt: Int = 1,
    val workers: Int = 1,
) {
    val progressFraction: Float
        get() {
            val tot = total ?: return 0f
            return if (tot > 0) (downloaded.toFloat() / tot.toFloat()).coerceIn(0f, 1f) else 0f
        }

    val speedFormatted: String
        get() {
            val bps = bytesPerSecond
            return when {
                bps >= 1024 * 1024 -> String.format(java.util.Locale.US, "%.1f MB/s", bps / (1024.0 * 1024.0))
                bps >= 1024 -> String.format(java.util.Locale.US, "%.1f KB/s", bps / 1024.0)
                bps > 0 -> String.format(java.util.Locale.US, "%.0f B/s", bps)
                else -> "0 B/s"
            }
        }

    val downloadedFormatted: String
        get() = formatBytes(downloaded)

    val totalFormatted: String
        get() = total?.let { formatBytes(it) } ?: "Unknown"

    private fun formatBytes(bytes: Long): String {
        val b = bytes.toDouble()
        return when {
            b >= 1024 * 1024 * 1024 -> String.format(java.util.Locale.US, "%.2f GB", b / (1024.0 * 1024.0 * 1024.0))
            b >= 1024 * 1024 -> String.format(java.util.Locale.US, "%.1f MB", b / (1024.0 * 1024.0))
            b >= 1024 -> String.format(java.util.Locale.US, "%.1f KB", b / 1024.0)
            else -> "$bytes B"
        }
    }
}

/** Mirrors `moviebox_android::downloads::DownloadJob`. */
@Serializable
data class DownloadJob(
    val id: String,
    val title: String,
    val provider: String = "",
    @SerialName("subjectId") val subjectId: String = "",
    val season: Int = 0,
    val episode: Int = 0,
    val destination: String,
    val state: String = "queued",
    val error: String? = null,
    val progress: DownloadProgress = DownloadProgress(),
) {
    val isRunning: Boolean get() = state.equals("running", ignoreCase = true)
    val isPaused: Boolean get() = state.equals("paused", ignoreCase = true)
    val isCompleted: Boolean get() = state.equals("completed", ignoreCase = true)
    val isFailed: Boolean get() = state.equals("failed", ignoreCase = true)
    val isQueued: Boolean get() = state.equals("queued", ignoreCase = true)
    val isSeries: Boolean get() = season > 0 || episode > 0
}

@Serializable
data class DownloadStartPayload(val id: String)

@Serializable
data class DownloadListPayload(val jobs: List<DownloadJob> = emptyList())

@Serializable
data class DownloadCancelPayload(val cancelled: Boolean = false)

@Serializable
data class DownloadRemovePayload(val removed: Boolean = false)

@Serializable
data class DownloadDirectoryPayload(val directory: String)

