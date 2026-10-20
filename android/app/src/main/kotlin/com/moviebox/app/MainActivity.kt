package com.moviebox.app

import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.widget.Toast
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.viewModels
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.lifecycleScope
import androidx.navigation.NavType
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import androidx.navigation.navArgument
import com.moviebox.app.core.DownloadJob
import com.moviebox.app.core.DownloadRequest
import com.moviebox.app.core.MovieBoxRepository
import com.moviebox.app.core.PlaybackSource
import com.moviebox.app.core.SubtitleOption
import com.moviebox.app.player.PlayerActivity
import com.moviebox.app.ui.DetailsViewModel
import com.moviebox.app.ui.MovieBoxViewModel
import com.moviebox.app.ui.screens.DetailsScreen
import com.moviebox.app.ui.screens.HomeScreen
import com.moviebox.app.ui.theme.MovieBoxTheme
import kotlinx.coroutines.launch
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.json.Json

private val appJson = Json {
    ignoreUnknownKeys = true
    isLenient = true
    coerceInputValues = true
}

class MainActivity : ComponentActivity() {

    private val homeViewModel: MovieBoxViewModel by viewModels()
    private val detailsViewModel: DetailsViewModel by viewModels()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        handleIntent(intent)
        setContent {
            MovieBoxTheme {
                MovieBoxApp(
                    homeViewModel = homeViewModel,
                    detailsViewModel = detailsViewModel,
                    onPlay = ::launchPlayer,
                    onOpenDownload = ::openDownloadedMedia,
                    onDownloadRelease = ::downloadRelease,
                )
            }
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        handleIntent(intent)
    }

    private fun handleIntent(intent: Intent?) {
        val testUrl = intent?.getStringExtra("test_download_url")
        if (!testUrl.isNullOrBlank()) {
            val testTitle = intent.getStringExtra("test_download_title") ?: "Test Media"
            val dest = getExternalFilesDir(null)?.absolutePath
            val request = DownloadRequest(
                url = testUrl,
                headers = emptyList(),
                title = testTitle,
                provider = "test",
                destination = dest,
            )
            lifecycleScope.launch {
                MovieBoxRepository().startDownload(request)
                homeViewModel.loadDownloads()
            }
        }
    }

    override fun onResume() {
        super.onResume()
        homeViewModel.loadLibrary()
    }

    private fun openDownloadedMedia(job: DownloadJob) {
        val file = java.io.File(job.destination)
        if (!file.exists()) {
            Toast.makeText(this, "File not found: ${job.destination}", Toast.LENGTH_LONG).show()
            return
        }
        android.media.MediaScannerConnection.scanFile(
            this,
            arrayOf(file.absolutePath),
            arrayOf("video/mp4"),
            null,
        )
        launchPlayer(
            source = PlaybackSource(
                url = Uri.fromFile(file).toString(),
                headers = emptyList(),
                sourceLabel = "Local Download",
                provider = job.provider,
            ),
            title = job.title,
            provider = job.provider,
            subjectId = job.subjectId,
            coverUrl = null,
            season = job.season,
            episode = job.episode,
            year = null,
            startPositionSec = 0L,
        )
    }

    private fun downloadRelease(release: com.moviebox.app.core.Release) {
        detailsViewModel.download(
            context = this,
            release = release,
            onStarted = { jobId ->
                Toast.makeText(this, "Download started ($jobId)", Toast.LENGTH_SHORT).show()
            },
        )
    }

    private fun launchPlayer(
        source: PlaybackSource,
        title: String,
        provider: String = "",
        subjectId: String = "",
        coverUrl: String? = null,
        season: Int = 0,
        episode: Int = 0,
        year: String? = null,
        startPositionSec: Long = 0L,
        subtitleUrl: String? = null,
        subtitleName: String? = null,
        subtitles: List<SubtitleOption> = emptyList(),
    ) {
        val headers = source.headerMap.map { (key, value) -> "$key=$value" }
        val subtitlesJson = if (subtitles.isNotEmpty()) {
            appJson.encodeToString(
                ListSerializer(SubtitleOption.serializer()),
                subtitles,
            )
        } else {
            ""
        }
        val intent = Intent(this, PlayerActivity::class.java).apply {
            putExtra(PlayerActivity.EXTRA_URL, source.url)
            putExtra(PlayerActivity.EXTRA_HEADERS, ArrayList(headers))
            putExtra(PlayerActivity.EXTRA_TITLE, title)
            putExtra(PlayerActivity.EXTRA_PROVIDER, provider)
            putExtra(PlayerActivity.EXTRA_SUBJECT_ID, subjectId)
            putExtra(PlayerActivity.EXTRA_COVER_URL, coverUrl)
            putExtra(PlayerActivity.EXTRA_SEASON, season)
            putExtra(PlayerActivity.EXTRA_EPISODE, episode)
            putExtra(PlayerActivity.EXTRA_YEAR, year)
            putExtra(PlayerActivity.EXTRA_START_POSITION, startPositionSec)
            putExtra(PlayerActivity.EXTRA_SUBTITLE_URL, subtitleUrl)
            putExtra(PlayerActivity.EXTRA_SUBTITLE_NAME, subtitleName)
            putExtra(PlayerActivity.EXTRA_SUBTITLES_JSON, subtitlesJson)
        }
        startActivity(intent)
    }
}

@Composable
private fun MovieBoxApp(
    homeViewModel: MovieBoxViewModel,
    detailsViewModel: DetailsViewModel,
    onPlay: (
        source: PlaybackSource,
        title: String,
        provider: String,
        subjectId: String,
        coverUrl: String?,
        season: Int,
        episode: Int,
        year: String?,
        startPositionSec: Long,
        subtitleUrl: String?,
        subtitleName: String?,
        subtitles: List<SubtitleOption>,
    ) -> Unit,
    onOpenDownload: (DownloadJob) -> Unit,
    onDownloadRelease: (com.moviebox.app.core.Release) -> Unit,
) {
    val navController = rememberNavController()
    val homeState by homeViewModel.state.collectAsStateWithLifecycle()
    val detailsState by detailsViewModel.state.collectAsStateWithLifecycle()

    NavHost(navController = navController, startDestination = "home") {
        composable("home") {
            HomeScreen(
                state = homeState,
                onQueryChange = homeViewModel::onQueryChange,
                onSearch = homeViewModel::search,
                onProviderSelected = homeViewModel::selectProvider,
                onSelectMainTab = homeViewModel::selectMainTab,
                onSelectHomeDeckTab = homeViewModel::selectHomeDeckTab,
                onToggleFavorite = homeViewModel::toggleFavorite,
                onOpenItem = { item ->
                    detailsViewModel.load(item.id.provider, item.id.value)
                    navController.navigate("details/${item.id.provider}/${item.id.value}")
                },
                onOpenHistoryItem = { historyItem ->
                    detailsViewModel.load(historyItem.provider, historyItem.subjectId)
                    if (historyItem.season > 0) {
                        detailsViewModel.selectSeason(historyItem.season)
                    }
                    if (historyItem.episode > 0) {
                        detailsViewModel.selectEpisode(historyItem.episode)
                    }
                    navController.navigate("details/${historyItem.provider}/${historyItem.subjectId}")
                },
                onPauseDownload = homeViewModel::pauseDownload,
                onResumeDownload = homeViewModel::resumeDownload,
                onRemoveDownload = homeViewModel::removeDownload,
                onRefreshDownloads = homeViewModel::loadDownloads,
                onOpenDownload = onOpenDownload,
            )
        }

        composable(
            route = "details/{provider}/{subjectId}",
            arguments = listOf(
                navArgument("provider") { type = NavType.StringType },
                navArgument("subjectId") { type = NavType.StringType },
            ),
        ) {
            DetailsScreen(
                state = detailsState,
                onBack = { navController.popBackStack() },
                onToggleFavorite = detailsViewModel::toggleFavorite,
                onResolutionSelected = detailsViewModel::selectResolution,
                onSubtitleSelected = detailsViewModel::selectSubtitle,
                onSeasonSelected = detailsViewModel::selectSeason,
                onEpisodeSelected = detailsViewModel::selectEpisode,
                onPlay = { release, sub ->
                    detailsViewModel.resolve(release) { source ->
                        val details = detailsState.details
                        onPlay(
                            source,
                            details?.title.orEmpty(),
                            details?.id?.provider.orEmpty(),
                            details?.id?.value.orEmpty(),
                            details?.posterUrl,
                            detailsState.selectedSeason,
                            detailsState.selectedEpisode,
                            details?.year,
                            0L,
                            sub?.url,
                            sub?.name,
                            detailsState.subtitles,
                        )
                    }
                },
                onDownload = onDownloadRelease,
            )
        }
    }
}
