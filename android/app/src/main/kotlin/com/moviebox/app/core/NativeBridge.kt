package com.moviebox.app.core

/**
 * Thin JNI binding to the Rust core engine (`libmoviebox_android.so`).
 *
 * Every function returns a JSON envelope:
 * `{"ok":true,"data":<payload>}` or `{"ok":false,"error":"<message>"}`.
 */
object NativeBridge {

    init {
        System.loadLibrary("moviebox_android")
    }

    @JvmStatic
    external fun nativeInit(configDir: String, dataDir: String, cacheDir: String): String

    @JvmStatic
    external fun nativeProviders(): String

    @JvmStatic
    external fun nativeSearch(provider: String, query: String, page: Int): String

    @JvmStatic
    external fun nativeSuggest(query: String): String

    @JvmStatic
    external fun nativeHomepage(tabId: String, page: Int): String

    @JvmStatic
    external fun nativeDetails(provider: String, subjectId: String): String

    @JvmStatic
    external fun nativeEpisodeStreams(
        provider: String,
        subjectId: String,
        season: Int,
        episode: Int,
    ): String

    @JvmStatic
    external fun nativeResolvePlayback(provider: String, releaseJson: String): String

    @JvmStatic
    external fun nativeSubtitles(
        subjectId: String,
        resourceId: String,
        siblingIdsJson: String,
        season: Int,
        episode: Int,
    ): String

    @JvmStatic
    external fun nativeFavorites(): String

    @JvmStatic
    external fun nativeToggleFavorite(itemJson: String): String

    @JvmStatic
    external fun nativeHistory(): String

    @JvmStatic
    external fun nativeRecordWatchProgress(itemJson: String): String

    @JvmStatic
    external fun nativePosterUrlHeaders(url: String): String
}
