package com.moviebox.app.player

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.viewinterop.AndroidView
import androidx.media3.common.MediaItem
import androidx.media3.common.MimeTypes
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.DefaultHttpDataSource
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import androidx.media3.ui.PlayerView
import com.moviebox.app.ui.theme.MovieBoxTheme

/**
 * Fullscreen player built on Media3 / ExoPlayer.
 *
 * The stream headers (User-Agent, Referer, CloudFront cookie) are mandatory
 * for most providers, so they are applied to every HTTP request.
 */
@UnstableApi
class PlayerActivity : ComponentActivity() {

    private var player: ExoPlayer? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val url = intent.getStringExtra(EXTRA_URL).orEmpty()
        val headers = intent.getStringArrayListExtra(EXTRA_HEADERS)
            ?.mapNotNull { entry ->
                val parts = entry.split('=', limit = 2)
                if (parts.size == 2 && parts[0].isNotBlank()) parts[0] to parts[1] else null
            }
            ?.toMap()
            .orEmpty()
        val title = intent.getStringExtra(EXTRA_TITLE).orEmpty()

        if (url.isEmpty()) {
            finish()
            return
        }

        val httpFactory = DefaultHttpDataSource.Factory()
            .setAllowCrossProtocolRedirects(true)
            .setDefaultRequestProperties(headers)

        val exoPlayer = ExoPlayer.Builder(this)
            .setMediaSourceFactory(DefaultMediaSourceFactory(httpFactory))
            .build()
        player = exoPlayer

        val mediaItem = MediaItem.Builder()
            .setUri(url)
            .setMediaMetadata(
                androidx.media3.common.MediaMetadata.Builder()
                    .setTitle(title)
                    .build(),
            )
            .apply {
                mimeTypeFor(url)?.let { setMimeType(it) }
            }
            .build()

        exoPlayer.setMediaItem(mediaItem)
        exoPlayer.prepare()
        exoPlayer.playWhenReady = true

        setContent {
            MovieBoxTheme {
                Box(
                    modifier = Modifier
                        .fillMaxSize()
                        .background(Color.Black),
                ) {
                    AndroidView(
                        modifier = Modifier.fillMaxSize(),
                        factory = { context ->
                            PlayerView(context).apply {
                                this.player = exoPlayer
                                useController = true
                            }
                        },
                    )
                }
            }
        }
    }

    override fun onDestroy() {
        player?.release()
        player = null
        super.onDestroy()
    }

    /** Returns the explicit MIME type for manifest based streams, if any. */
    private fun mimeTypeFor(url: String): String? = when {
        url.contains(".mpd") -> MimeTypes.APPLICATION_MPD
        url.contains(".m3u8") -> MimeTypes.APPLICATION_M3U8
        else -> null
    }

    companion object {
        const val EXTRA_URL = "com.moviebox.app.extra.URL"
        const val EXTRA_HEADERS = "com.moviebox.app.extra.HEADERS"
        const val EXTRA_TITLE = "com.moviebox.app.extra.TITLE"
    }
}
