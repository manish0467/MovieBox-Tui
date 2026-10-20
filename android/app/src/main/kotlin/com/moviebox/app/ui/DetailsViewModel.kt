package com.moviebox.app.ui

import android.app.Application
import android.content.Context
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.moviebox.app.core.DownloadRequest
import com.moviebox.app.core.MediaDetails
import com.moviebox.app.core.MovieBoxRepository
import com.moviebox.app.core.PlaybackSource
import com.moviebox.app.core.Release
import com.moviebox.app.core.SubtitleOption
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

data class DetailsUiState(
    val details: MediaDetails? = null,
    val releases: List<Release> = emptyList(),
    val subtitles: List<SubtitleOption> = emptyList(),
    val selectedSeason: Int = 0,
    val selectedEpisode: Int = 0,
    val selectedResolution: String = "All",
    val availableResolutions: List<String> = emptyList(),
    val selectedSubtitle: SubtitleOption? = null,
    val isFavorite: Boolean = false,
    val loading: Boolean = true,
    val resolving: Boolean = false,
    val error: String? = null,
) {
    val filteredReleases: List<Release>
        get() = if (selectedResolution == "All") {
            releases
        } else {
            releases.filter {
                it.quality?.equals(selectedResolution, ignoreCase = true) == true ||
                it.sourceLabel.contains(selectedResolution, ignoreCase = true) ||
                it.filename.contains(selectedResolution, ignoreCase = true)
            }
        }
}

class DetailsViewModel(app: Application) : AndroidViewModel(app) {

    private val repository = MovieBoxRepository().also { it.initialize(app) }

    private val _state = MutableStateFlow(DetailsUiState())
    val state: StateFlow<DetailsUiState> = _state.asStateFlow()

    fun load(provider: String, subjectId: String) {
        viewModelScope.launch {
            _state.update { it.copy(loading = true, error = null) }
            runCatching { repository.details(provider, subjectId) }
                .onSuccess { details ->
                    _state.update { it.copy(details = details, loading = false) }
                    checkFavorite(provider, subjectId)
                    loadReleases(provider, subjectId, 0, 0)
                }
                .onFailure { error ->
                    _state.update {
                        it.copy(loading = false, error = error.message ?: "Failed to load details")
                    }
                }
        }
    }

    private fun checkFavorite(provider: String, subjectId: String) {
        viewModelScope.launch {
            runCatching { repository.favorites() }
                .onSuccess { favs ->
                    val isFav = favs.any { it.provider == provider && it.subjectId == subjectId }
                    _state.update { it.copy(isFavorite = isFav) }
                }
        }
    }

    fun toggleFavorite() {
        val details = _state.value.details ?: return
        viewModelScope.launch {
            val item = com.moviebox.app.core.FavoriteItem(
                provider = details.id.provider,
                subjectId = details.id.value,
                title = details.title,
                coverUrl = details.posterUrl,
                stype = if (details.isSeries) 2 else 1,
                releaseYear = details.year.orEmpty(),
            )
            runCatching { repository.toggleFavorite(item) }
                .onSuccess { fav ->
                    _state.update { it.copy(isFavorite = fav) }
                }
        }
    }

    fun selectResolution(resolution: String) {
        _state.update { it.copy(selectedResolution = resolution) }
    }

    fun selectSubtitle(subtitle: SubtitleOption?) {
        _state.update { it.copy(selectedSubtitle = subtitle) }
    }

    fun selectSeason(season: Int) {
        val details = _state.value.details ?: return
        val provider = details.id.provider
        val subjectId = details.id.value
        _state.update { it.copy(selectedSeason = season, selectedEpisode = 0) }
        loadReleases(provider, subjectId, season, 0)
    }

    fun selectEpisode(episode: Int) {
        val details = _state.value.details ?: return
        val provider = details.id.provider
        val subjectId = details.id.value
        val season = _state.value.selectedSeason
        _state.update { it.copy(selectedEpisode = episode) }
        loadReleases(provider, subjectId, season, episode)
    }

    private fun loadReleases(provider: String, subjectId: String, season: Int, episode: Int) {
        viewModelScope.launch {
            _state.update { it.copy(loading = true) }
            runCatching { repository.episodeStreams(provider, subjectId, season, episode) }
                .onSuccess { releases ->
                    val distinctResolutions = releases.mapNotNull { it.quality }
                        .filter { it.isNotBlank() }
                        .distinct()
                    val available = if (distinctResolutions.isNotEmpty()) {
                        listOf("All") + distinctResolutions
                    } else {
                        emptyList()
                    }
                    _state.update {
                        it.copy(
                            releases = releases,
                            availableResolutions = available,
                            selectedResolution = "All",
                            loading = false,
                        )
                    }
                    loadSubtitles(provider, subjectId, releases, season, episode)
                }
                .onFailure { error ->
                    _state.update {
                        it.copy(loading = false, error = error.message ?: "No streams available")
                    }
                }
        }
    }

    private fun loadSubtitles(
        provider: String,
        subjectId: String,
        releases: List<Release>,
        season: Int,
        episode: Int,
    ) {
        val resourceId = releases.firstOrNull { !it.resourceId.isNullOrEmpty() }?.resourceId
        if (resourceId.isNullOrEmpty()) return
        viewModelScope.launch {
            runCatching {
                repository.subtitles(subjectId, resourceId, listOf(subjectId), season, episode)
            }.onSuccess { captions ->
                val preferred = captions.firstOrNull { it.name.contains("English", ignoreCase = true) }
                    ?: captions.firstOrNull()
                _state.update { it.copy(subtitles = captions, selectedSubtitle = preferred) }
            }
        }
    }

    /** Resolves a release into a playable source. */
    fun resolve(release: Release, onReady: (PlaybackSource) -> Unit) {
        val details = _state.value.details ?: return
        viewModelScope.launch {
            _state.update { it.copy(resolving = true, error = null) }
            runCatching {
                repository.resolvePlayback(details.id.provider, release)
            }.onSuccess { source ->
                _state.update { it.copy(resolving = false) }
                onReady(source)
            }.onFailure { error ->
                _state.update {
                    it.copy(resolving = false, error = error.message ?: "Playback failed")
                }
            }
        }
    }

    /**
     * Resolves a release into a playable source and registers a background
     * download job via the bridge.
     */
    fun download(
        context: Context,
        release: Release,
        onStarted: (String) -> Unit = {},
    ) {
        val details = _state.value.details ?: return
        viewModelScope.launch {
            _state.update { it.copy(resolving = true, error = null) }
            runCatching {
                repository.resolvePlayback(details.id.provider, release)
            }.onSuccess { source ->
                val destinationBase = context.getExternalFilesDir(null)?.absolutePath
                val title = if (release.filename.isNotBlank()) release.filename else details.title
                val request = DownloadRequest(
                    url = source.url,
                    headers = source.headers,
                    title = title,
                    provider = details.id.provider,
                    subjectId = details.id.value,
                    season = _state.value.selectedSeason,
                    episode = _state.value.selectedEpisode,
                    destination = destinationBase,
                )
                val jobId = repository.startDownload(request)
                _state.update { it.copy(resolving = false) }
                onStarted(jobId)
            }.onFailure { error ->
                _state.update {
                    it.copy(resolving = false, error = error.message ?: "Failed to start download")
                }
            }
        }
    }
}
