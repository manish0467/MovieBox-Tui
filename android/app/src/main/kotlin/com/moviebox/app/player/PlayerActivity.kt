package com.moviebox.app.player

import android.net.Uri
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.HighQuality
import androidx.compose.material.icons.filled.Subtitles
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.lifecycle.lifecycleScope
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.MimeTypes
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.DefaultDataSource
import androidx.media3.datasource.DefaultHttpDataSource
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import androidx.media3.ui.PlayerView
import com.moviebox.app.core.MovieBoxRepository
import com.moviebox.app.core.SubtitleOption
import com.moviebox.app.ui.theme.MovieBoxTheme
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.json.Json
import java.io.File

/**
 * Fullscreen player built on Media3 / ExoPlayer.
 *
 * Invariants strictly maintained:
 * 1. Stream headers (User-Agent, Referer, CloudFront cookie) are mandatory and
 *    applied to DefaultHttpDataSource.
 * 2. External subtitles are downloaded locally and attached to Media3 as SubtitleConfiguration.
 * 3. Video resolution picker caps max video height.
 * 4. Watch position is periodically flushed to the pending playback state store so
 *    the core bridge can reconcile history.
 */
private val playerJson = Json {
    ignoreUnknownKeys = true
    isLenient = true
    coerceInputValues = true
}

@UnstableApi
class PlayerActivity : ComponentActivity() {

    private var player: ExoPlayer? = null
    private val repository = MovieBoxRepository()

    private var provider: String = ""
    private var subjectId: String = ""
    private var title: String = ""
    private var coverUrl: String? = null
    private var season: Int = 0
    private var episode: Int = 0
    private var year: String? = null
    private var headersMap: Map<String, String> = emptyMap()
    private var currentMediaUrl: String = ""

    private val availableSubtitles = mutableStateOf<List<SubtitleOption>>(emptyList())
    private val selectedSubtitleName = mutableStateOf<String?>("None")
    private val selectedResolutionLabel = mutableStateOf("Auto")

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        repository.initialize(this)

        val url = intent.getStringExtra(EXTRA_URL).orEmpty()
        currentMediaUrl = url
        headersMap = intent.getStringArrayListExtra(EXTRA_HEADERS)
            ?.mapNotNull { entry ->
                val parts = entry.split('=', limit = 2)
                if (parts.size == 2 && parts[0].isNotBlank()) parts[0] to parts[1] else null
            }
            ?.toMap()
            .orEmpty()

        title = intent.getStringExtra(EXTRA_TITLE).orEmpty()
        provider = intent.getStringExtra(EXTRA_PROVIDER).orEmpty()
        subjectId = intent.getStringExtra(EXTRA_SUBJECT_ID).orEmpty()
        coverUrl = intent.getStringExtra(EXTRA_COVER_URL)
        season = intent.getIntExtra(EXTRA_SEASON, 0)
        episode = intent.getIntExtra(EXTRA_EPISODE, 0)
        year = intent.getStringExtra(EXTRA_YEAR)
        val startPositionSec = intent.getLongExtra(EXTRA_START_POSITION, 0L)
        val initialSubUrl = intent.getStringExtra(EXTRA_SUBTITLE_URL)
        val initialSubName = intent.getStringExtra(EXTRA_SUBTITLE_NAME)
        val subtitlesJson = intent.getStringExtra(EXTRA_SUBTITLES_JSON).orEmpty()

        if (url.isEmpty()) {
            finish()
            return
        }

        if (subtitlesJson.isNotBlank()) {
            runCatching {
                val list = playerJson.decodeFromString(
                    ListSerializer(SubtitleOption.serializer()),
                    subtitlesJson,
                )
                availableSubtitles.value = list
            }
        }

        // Apply mandatory HTTP headers (User-Agent, Referer, CloudFront cookies).
        // The factory is wrapped in a DefaultDataSource so that local files
        // (the downloaded subtitle track) are served by the file data source
        // instead of the HTTP one.
        val httpFactory = DefaultHttpDataSource.Factory()
            .setAllowCrossProtocolRedirects(true)
            .setDefaultRequestProperties(headersMap)
        val dataSourceFactory = DefaultDataSource.Factory(this, httpFactory)

        val exoPlayer = ExoPlayer.Builder(this)
            .setMediaSourceFactory(DefaultMediaSourceFactory(dataSourceFactory))
            .build()
        player = exoPlayer

        val initialMediaItem = buildMediaItem(url, title, null, null)
        exoPlayer.setMediaItem(initialMediaItem)

        if (startPositionSec > 0) {
            exoPlayer.seekTo(startPositionSec * 1000L)
        }

        exoPlayer.prepare()
        exoPlayer.playWhenReady = true

        // If an initial subtitle was chosen, download and attach it
        if (!initialSubUrl.isNullOrBlank()) {
            selectedSubtitleName.value = initialSubName ?: "Subtitles"
            attachSubtitleFromUrl(initialSubUrl, initialSubName ?: "Default")
        }

        // Periodic watch progress recording back to the bridge
        lifecycleScope.launch {
            while (isActive) {
                delay(4000)
                if (player?.isPlaying == true) {
                    saveProgress()
                }
            }
        }

        setContent {
            MovieBoxTheme {
                PlayerScreenContent(
                    player = exoPlayer,
                    title = title,
                    season = season,
                    episode = episode,
                    subtitles = availableSubtitles.value,
                    selectedSubtitle = selectedSubtitleName.value,
                    selectedResolution = selectedResolutionLabel.value,
                    onBack = {
                        saveProgress()
                        finish()
                    },
                    onSelectSubtitle = { sub ->
                        if (sub == null) {
                            selectedSubtitleName.value = "None"
                            disableSubtitles()
                        } else {
                            selectedSubtitleName.value = sub.name
                            attachSubtitleFromUrl(sub.url, sub.name)
                        }
                    },
                    onSelectResolution = { label, height ->
                        selectedResolutionLabel.value = label
                        setResolutionLimit(height)
                    },
                )
            }
        }
    }

    private fun buildMediaItem(
        url: String,
        mediaTitle: String,
        subtitleFile: File?,
        subtitleLabel: String?,
    ): MediaItem {
        val builder = MediaItem.Builder()
            .setUri(url)
            .setMediaMetadata(
                androidx.media3.common.MediaMetadata.Builder()
                    .setTitle(mediaTitle)
                    .build(),
            )
            .apply {
                mimeTypeFor(url)?.let { setMimeType(it) }
            }

        if (subtitleFile != null && subtitleFile.exists() && subtitleFile.length() > 0) {
            val subMime = if (subtitleFile.name.endsWith(".vtt", ignoreCase = true) ||
                subtitleFile.name.contains(".vtt", ignoreCase = true)
            ) {
                MimeTypes.TEXT_VTT
            } else {
                MimeTypes.APPLICATION_SUBRIP
            }
            val subConfig = MediaItem.SubtitleConfiguration.Builder(Uri.fromFile(subtitleFile))
                .setMimeType(subMime)
                .setLabel(subtitleLabel ?: "Default")
                .setSelectionFlags(C.SELECTION_FLAG_DEFAULT)
                .build()
            builder.setSubtitleConfigurations(listOf(subConfig))
        }

        return builder.build()
    }

    private fun attachSubtitleFromUrl(subUrl: String, label: String) {
        lifecycleScope.launch {
            val file = repository.downloadSubtitleTrack(
                context = this@PlayerActivity,
                subtitleUrl = subUrl,
                label = label,
                headers = headersMap,
            )
            if (file != null && player != null) {
                val exo = player ?: return@launch
                val currentPos = exo.currentPosition
                val isPlaying = exo.playWhenReady
                val updatedItem = buildMediaItem(currentMediaUrl, title, file, label)
                exo.setMediaItem(updatedItem, currentPos)
                exo.playWhenReady = isPlaying
                // Ensure text tracks are enabled
                exo.trackSelectionParameters = exo.trackSelectionParameters
                    .buildUpon()
                    .setTrackTypeDisabled(C.TRACK_TYPE_TEXT, false)
                    .build()
            }
        }
    }

    private fun disableSubtitles() {
        val exo = player ?: return
        exo.trackSelectionParameters = exo.trackSelectionParameters
            .buildUpon()
            .setTrackTypeDisabled(C.TRACK_TYPE_TEXT, true)
            .build()
    }

    private fun setResolutionLimit(height: Int?) {
        val exo = player ?: return
        val builder = exo.trackSelectionParameters.buildUpon()
        if (height == null || height <= 0) {
            builder.clearVideoSizeConstraints()
        } else {
            builder.setMaxVideoSize(Int.MAX_VALUE, height)
        }
        exo.trackSelectionParameters = builder.build()
    }

    private fun saveProgress() {
        val p = player ?: return
        val durationMs = p.duration
        val positionMs = p.currentPosition
        if (durationMs > 0 && positionMs > 0 && subjectId.isNotBlank()) {
            val progressSec = positionMs / 1000
            val durationSec = durationMs / 1000
            val completed = progressSec >= (durationSec * 0.90)
            lifecycleScope.launch(Dispatchers.IO) {
                repository.recordWatchProgress(
                    context = applicationContext,
                    provider = provider,
                    subjectId = subjectId,
                    title = title,
                    coverUrl = coverUrl,
                    season = season,
                    episode = episode,
                    progressSeconds = progressSec,
                    durationSeconds = durationSec,
                    completed = completed,
                    releaseYear = year,
                )
            }
        }
    }

    override fun onPause() {
        super.onPause()
        saveProgress()
    }

    override fun onStop() {
        super.onStop()
        saveProgress()
    }

    override fun onDestroy() {
        saveProgress()
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
        const val EXTRA_PROVIDER = "com.moviebox.app.extra.PROVIDER"
        const val EXTRA_SUBJECT_ID = "com.moviebox.app.extra.SUBJECT_ID"
        const val EXTRA_COVER_URL = "com.moviebox.app.extra.COVER_URL"
        const val EXTRA_SEASON = "com.moviebox.app.extra.SEASON"
        const val EXTRA_EPISODE = "com.moviebox.app.extra.EPISODE"
        const val EXTRA_YEAR = "com.moviebox.app.extra.YEAR"
        const val EXTRA_START_POSITION = "com.moviebox.app.extra.START_POSITION"
        const val EXTRA_SUBTITLE_URL = "com.moviebox.app.extra.SUBTITLE_URL"
        const val EXTRA_SUBTITLE_NAME = "com.moviebox.app.extra.SUBTITLE_NAME"
        const val EXTRA_SUBTITLES_JSON = "com.moviebox.app.extra.SUBTITLES_JSON"
    }
}

@Composable
private fun PlayerScreenContent(
    player: ExoPlayer,
    title: String,
    season: Int,
    episode: Int,
    subtitles: List<SubtitleOption>,
    selectedSubtitle: String?,
    selectedResolution: String,
    onBack: () -> Unit,
    onSelectSubtitle: (SubtitleOption?) -> Unit,
    onSelectResolution: (String, Int?) -> Unit,
) {
    var showControls by remember { mutableStateOf(true) }
    var showSubtitleDialog by remember { mutableStateOf(false) }
    var showResolutionDialog by remember { mutableStateOf(false) }

    val resolutions = listOf(
        "Auto" to null,
        "1080p" to 1080,
        "720p" to 720,
        "480p" to 480,
        "360p" to 360,
    )

    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(Color.Black)
            .clickable(
                indication = null,
                interactionSource = remember { MutableInteractionSource() },
            ) {
                showControls = !showControls
            },
    ) {
        AndroidView(
            modifier = Modifier.fillMaxSize(),
            factory = { context ->
                PlayerView(context).apply {
                    this.player = player
                    useController = true
                }
            },
        )

        AnimatedVisibility(
            visible = showControls,
            enter = fadeIn(),
            exit = fadeOut(),
            modifier = Modifier.align(Alignment.TopCenter),
        ) {
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .background(Color.Black.copy(alpha = 0.65f))
                    .statusBarsPadding()
                    .padding(horizontal = 8.dp, vertical = 6.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                IconButton(onClick = onBack) {
                    Icon(
                        Icons.AutoMirrored.Filled.ArrowBack,
                        contentDescription = "Back",
                        tint = Color.White,
                    )
                }

                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        text = title,
                        style = MaterialTheme.typography.titleMedium,
                        color = Color.White,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                    if (season > 0 || episode > 0) {
                        Text(
                            text = "Season $season · Episode $episode",
                            style = MaterialTheme.typography.labelSmall,
                            color = Color.LightGray,
                        )
                    }
                }

                IconButton(onClick = { showResolutionDialog = true }) {
                    Icon(
                        Icons.Filled.HighQuality,
                        contentDescription = "Resolution ($selectedResolution)",
                        tint = if (selectedResolution != "Auto") MaterialTheme.colorScheme.primary else Color.White,
                    )
                }

                IconButton(onClick = { showSubtitleDialog = true }) {
                    Icon(
                        Icons.Filled.Subtitles,
                        contentDescription = "Subtitles ($selectedSubtitle)",
                        tint = if (selectedSubtitle != "None" && selectedSubtitle != null) MaterialTheme.colorScheme.primary else Color.White,
                    )
                }
            }
        }

        if (showSubtitleDialog) {
            AlertDialog(
                onDismissRequest = { showSubtitleDialog = false },
                title = { Text("Subtitles") },
                text = {
                    LazyColumn {
                        item {
                            SubtitleDialogRow(
                                title = "None (Off)",
                                selected = selectedSubtitle == null || selectedSubtitle == "None",
                                onClick = {
                                    onSelectSubtitle(null)
                                    showSubtitleDialog = false
                                },
                            )
                        }
                        items(subtitles) { sub ->
                            SubtitleDialogRow(
                                title = sub.name,
                                selected = selectedSubtitle == sub.name,
                                onClick = {
                                    onSelectSubtitle(sub)
                                    showSubtitleDialog = false
                                },
                            )
                        }
                    }
                },
                confirmButton = {
                    TextButton(onClick = { showSubtitleDialog = false }) {
                        Text("Close")
                    }
                },
            )
        }

        if (showResolutionDialog) {
            AlertDialog(
                onDismissRequest = { showResolutionDialog = false },
                title = { Text("Quality / Resolution") },
                text = {
                    LazyColumn {
                        items(resolutions) { (label, height) ->
                            ResolutionDialogRow(
                                title = label,
                                selected = selectedResolution == label,
                                onClick = {
                                    onSelectResolution(label, height)
                                    showResolutionDialog = false
                                },
                            )
                        }
                    }
                },
                confirmButton = {
                    TextButton(onClick = { showResolutionDialog = false }) {
                        Text("Close")
                    }
                },
            )
        }
    }
}

@Composable
private fun SubtitleDialogRow(title: String, selected: Boolean, onClick: () -> Unit) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .padding(vertical = 12.dp, horizontal = 8.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            text = title,
            style = MaterialTheme.typography.bodyLarge,
            fontWeight = if (selected) FontWeight.Bold else FontWeight.Normal,
        )
        if (selected) {
            Icon(Icons.Filled.Check, contentDescription = null, tint = MaterialTheme.colorScheme.primary)
        }
    }
}

@Composable
private fun ResolutionDialogRow(title: String, selected: Boolean, onClick: () -> Unit) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable(onClick = onClick)
            .padding(vertical = 12.dp, horizontal = 8.dp),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            text = title,
            style = MaterialTheme.typography.bodyLarge,
            fontWeight = if (selected) FontWeight.Bold else FontWeight.Normal,
        )
        if (selected) {
            Icon(Icons.Filled.Check, contentDescription = null, tint = MaterialTheme.colorScheme.primary)
        }
    }
}
