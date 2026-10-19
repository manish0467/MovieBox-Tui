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
)

/** Mirrors `moviebox_tui::history::WatchHistoryItem` (only the fields we use). */
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
    @SerialName("last_watched") val lastWatched: Long = 0,
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
data class ToggleFavoritePayload(val favorite: Boolean = false)

@Serializable
data class HeadersPayload(val headers: List<List<String>> = emptyList())
