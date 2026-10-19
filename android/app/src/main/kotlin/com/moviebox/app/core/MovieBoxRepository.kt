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
            ItemsPayload.serializer(Release.serializer()),
        ).items
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
            ItemsPayload.serializer(SubtitleOption.serializer()),
        ).items
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
}
