package com.moviebox.app.core

import android.content.Context
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import kotlinx.serialization.DeserializationStrategy
import kotlinx.serialization.Serializable
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.builtins.serializer
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.contentOrNull
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive

/** Error raised when the Rust core reports a failure. */
class MovieBoxException(message: String) : Exception(message)

/** Generic `{"items": [...]}` payload wrapper. */
@Serializable
data class ItemsPayload<T>(
    val items: List<T> = emptyList(),
)

private val json = Json {
    ignoreUnknownKeys = true
    isLenient = true
    coerceInputValues = true
}

private fun <T> decodeData(raw: String, deserializer: DeserializationStrategy<T>): T {
    val root = runCatching { json.parseToJsonElement(raw).jsonObject }.getOrNull()
        ?: throw MovieBoxException("Malformed response from the core engine")
    val ok = root["ok"]?.jsonPrimitive?.booleanOrNull ?: false
    if (!ok) {
        throw MovieBoxException(
            root["error"]?.jsonPrimitive?.contentOrNull ?: "Unknown error from the core engine",
        )
    }
    val data = root["data"] ?: throw MovieBoxException("Empty response payload")
    return json.decodeFromJsonElement(deserializer, data)
}

/**
 * Kotlin facade over the Rust core engine.
 *
 * All calls are blocking JNI calls, so they are dispatched onto
 * [Dispatchers.IO].
 */
class MovieBoxRepository {

    fun initialize(context: Context) {
        val files = context.filesDir
        val cacheDir = context.cacheDir.resolve("moviebox")
        NativeBridge.nativeInit(
            files.absolutePath,
            files.absolutePath,
            cacheDir.absolutePath,
        )
    }

    suspend fun providers(): List<ProviderInfo> = withContext(Dispatchers.IO) {
        decodeData(NativeBridge.nativeProviders(), ProvidersPayload.serializer()).providers
    }

    suspend fun search(provider: String, query: String, page: Int = 1): List<CatalogItem> =
        withContext(Dispatchers.IO) {
            decodeData(
                NativeBridge.nativeSearch(provider, query, page),
                ItemsPayload.serializer(CatalogItem.serializer()),
            ).items
        }

    suspend fun suggest(query: String): List<String> = withContext(Dispatchers.IO) {
        decodeData(
            NativeBridge.nativeSuggest(query),
            ItemsPayload.serializer(String.serializer()),
        ).items
    }

    suspend fun homepage(tabId: String = "2", page: Int = 1): List<CatalogItem> =
        withContext(Dispatchers.IO) {
            decodeData(
                NativeBridge.nativeHomepage(tabId, page),
                ItemsPayload.serializer(CatalogItem.serializer()),
            ).items
        }

    suspend fun details(provider: String, subjectId: String): MediaDetails =
        withContext(Dispatchers.IO) {
            decodeData(
                NativeBridge.nativeDetails(provider, subjectId),
                DetailsPayload.serializer(),
            ).details
        }

    suspend fun episodeStreams(
        provider: String,
        subjectId: String,
        season: Int,
        episode: Int,
    ): List<Release> = withContext(Dispatchers.IO) {
        decodeData(
            NativeBridge.nativeEpisodeStreams(provider, subjectId, season, episode),
            ReleasesPayload.serializer(),
        ).releases
    }

    suspend fun resolvePlayback(provider: String, release: Release): PlaybackSource =
        withContext(Dispatchers.IO) {
            val payload = json.encodeToString(Release.serializer(), release)
            decodeData(
                NativeBridge.nativeResolvePlayback(provider, payload),
                PlaybackSource.serializer(),
            )
        }

    suspend fun subtitles(
        subjectId: String,
        resourceId: String,
        siblingIds: List<String>,
        season: Int,
        episode: Int,
    ): List<SubtitleOption> = withContext(Dispatchers.IO) {
        val siblings = json.encodeToString(
            ListSerializer(String.serializer()),
            siblingIds,
        )
        decodeData(
            NativeBridge.nativeSubtitles(subjectId, resourceId, siblings, season, episode),
            SubtitlesPayload.serializer(),
        ).captions
    }

    suspend fun favorites(): List<FavoriteItem> = withContext(Dispatchers.IO) {
        decodeData(
            NativeBridge.nativeFavorites(),
            ItemsPayload.serializer(FavoriteItem.serializer()),
        ).items
    }

    suspend fun toggleFavorite(item: FavoriteItem): Boolean = withContext(Dispatchers.IO) {
        val payload = json.encodeToString(FavoriteItem.serializer(), item)
        decodeData(
            NativeBridge.nativeToggleFavorite(payload),
            ToggleFavoritePayload.serializer(),
        ).favorite
    }

    suspend fun history(): List<WatchHistoryItem> = withContext(Dispatchers.IO) {
        decodeData(
            NativeBridge.nativeHistory(),
            ItemsPayload.serializer(WatchHistoryItem.serializer()),
        ).items
    }

    suspend fun posterHeaders(url: String): Map<String, String> = withContext(Dispatchers.IO) {
        decodeData(
            NativeBridge.nativePosterUrlHeaders(url),
            HeadersPayload.serializer(),
        ).headers.mapNotNull { pair ->
            if (pair.size >= 2) pair[0] to pair[1] else null
        }.toMap()
    }

    /**
     * Records watch progress back to the bridge.
     *
     * The bridge owns the history file, so the progress is written through
     * `nativeRecordWatchProgress`. The pending-playback-state file remains as
     * a fallback for the case where the bridge call is unavailable.
     */
    suspend fun recordWatchProgress(
        context: Context,
        provider: String,
        subjectId: String,
        title: String,
        coverUrl: String?,
        season: Int,
        episode: Int,
        progressSeconds: Long,
        durationSeconds: Long?,
        completed: Boolean,
        releaseYear: String? = null,
        stype: Long = if (season > 0 || episode > 0) 2L else 1L,
    ) = withContext(Dispatchers.IO) {
        val state = PendingPlaybackState(
            provider = provider,
            subjectId = subjectId,
            season = season,
            episode = episode,
            progressSeconds = progressSeconds,
            durationSeconds = durationSeconds,
            completed = completed,
            timestamp = System.currentTimeMillis() / 1000,
            title = title,
            coverUrl = coverUrl,
            stype = stype,
            releaseYear = releaseYear,
        )
        val payload = json.encodeToString(PendingPlaybackState.serializer(), state)

        val recorded = runCatching {
            val raw = NativeBridge.nativeRecordWatchProgress(payload)
            val root = json.parseToJsonElement(raw).jsonObject
            root["ok"]?.jsonPrimitive?.booleanOrNull == true
        }.getOrDefault(false)

        if (recorded) {
            return@withContext
        }

        // Fallback: the Rust core reconciles pending playback states from the
        // playback directory on the next history read.
        runCatching {
            val playbackDir = context.filesDir.resolve("playback")
            if (!playbackDir.exists()) {
                playbackDir.mkdirs()
            }
            val safeProvider = provider.replace("[^a-zA-Z0-9]".toRegex(), "_")
            val safeSubject = subjectId.replace("[^a-zA-Z0-9]".toRegex(), "_")
            val file = playbackDir.resolve("${safeProvider}_${safeSubject}_${season}_${episode}.json")
            file.writeText(payload)
        }
    }

    /**
     * Downloads an external subtitle track to a local file in app cache so
     * Media3 can attach it as a local subtitle track.
     */
    suspend fun downloadSubtitleTrack(
        context: Context,
        subtitleUrl: String,
        label: String = "sub",
        headers: Map<String, String> = emptyMap(),
    ): java.io.File? = withContext(Dispatchers.IO) {
        runCatching {
            val dir = context.cacheDir.resolve("subtitles")
            if (!dir.exists()) dir.mkdirs()

            val ext = when {
                subtitleUrl.contains(".vtt", ignoreCase = true) -> "vtt"
                subtitleUrl.contains(".srt", ignoreCase = true) -> "srt"
                subtitleUrl.contains(".ass", ignoreCase = true) -> "ass"
                else -> "srt"
            }
            val safeName = label.replace("[^a-zA-Z0-9]".toRegex(), "_")
            val hash = Integer.toHexString(subtitleUrl.hashCode())
            val targetFile = dir.resolve("${safeName}_$hash.$ext")
            if (targetFile.exists() && targetFile.length() > 0) {
                return@runCatching targetFile
            }

            val connection = java.net.URI(subtitleUrl).toURL().openConnection() as java.net.HttpURLConnection
            connection.connectTimeout = 10_000
            connection.readTimeout = 10_000
            headers.forEach { (k, v) -> connection.setRequestProperty(k, v) }
            if (!headers.containsKey("User-Agent")) {
                connection.setRequestProperty("User-Agent", "MovieBox/1.0")
            }
            connection.connect()

            if (connection.responseCode in 200..299) {
                connection.inputStream.use { input ->
                    targetFile.outputStream().use { output ->
                        input.copyTo(output)
                    }
                }
                targetFile
            } else {
                null
            }
        }.getOrNull()
    }
}
