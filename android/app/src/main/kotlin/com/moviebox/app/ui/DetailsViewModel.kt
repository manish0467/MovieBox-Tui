package com.moviebox.app.ui

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
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
    val loading: Boolean = true,
    val resolving: Boolean = false,
    val error: String? = null,
)

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
                    loadReleases(provider, subjectId, 0, 0)
                }
                .onFailure { error ->
                    _state.update {
                        it.copy(loading = false, error = error.message ?: "Failed to load details")
                    }
                }
        }
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
                    _state.update { it.copy(releases = releases, loading = false) }
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
                _state.update { it.copy(subtitles = captions) }
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
}
