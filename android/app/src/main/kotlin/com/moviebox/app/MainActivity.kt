package com.moviebox.app

import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.viewModels
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.navigation.NavType
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import androidx.navigation.navArgument
import com.moviebox.app.core.PlaybackSource
import com.moviebox.app.player.PlayerActivity
import com.moviebox.app.ui.DetailsViewModel
import com.moviebox.app.ui.MovieBoxViewModel
import com.moviebox.app.ui.screens.DetailsScreen
import com.moviebox.app.ui.screens.HomeScreen
import com.moviebox.app.ui.theme.MovieBoxTheme

class MainActivity : ComponentActivity() {

    private val homeViewModel: MovieBoxViewModel by viewModels()
    private val detailsViewModel: DetailsViewModel by viewModels()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            MovieBoxTheme {
                MovieBoxApp(
                    homeViewModel = homeViewModel,
                    detailsViewModel = detailsViewModel,
                    onPlay = ::launchPlayer,
                )
            }
        }
    }

    private fun launchPlayer(source: PlaybackSource, title: String) {
        val headers = source.headerMap.map { (key, value) -> "$key=$value" }
        val intent = Intent(this, PlayerActivity::class.java).apply {
            putExtra(PlayerActivity.EXTRA_URL, source.url)
            putExtra(PlayerActivity.EXTRA_HEADERS, ArrayList(headers))
            putExtra(PlayerActivity.EXTRA_TITLE, title)
        }
        startActivity(intent)
    }
}

@Composable
private fun MovieBoxApp(
    homeViewModel: MovieBoxViewModel,
    detailsViewModel: DetailsViewModel,
    onPlay: (PlaybackSource, String) -> Unit,
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
                onOpenItem = { item ->
                    detailsViewModel.load(item.id.provider, item.id.value)
                    navController.navigate("details/${item.id.provider}/${item.id.value}")
                },
            )
        }

        composable(
            route = "details/{provider}/{subjectId}",
            arguments = listOf(
                navArgument("provider") { type = NavType.StringType },
                navArgument("subjectId") { type = NavType.StringType },
            ),
        ) { entry ->
            val provider = entry.arguments?.getString("provider").orEmpty()
            val subjectId = entry.arguments?.getString("subjectId").orEmpty()
            DetailsScreen(
                state = detailsState,
                onBack = { navController.popBackStack() },
                onSeasonSelected = detailsViewModel::selectSeason,
                onEpisodeSelected = detailsViewModel::selectEpisode,
                onPlay = { release ->
                    detailsViewModel.resolve(release) { source ->
                        onPlay(source, detailsState.details?.title.orEmpty())
                    }
                },
            )
        }
    }
}
