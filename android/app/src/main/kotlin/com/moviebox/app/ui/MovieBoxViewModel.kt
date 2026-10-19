package com.moviebox.app.ui

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.moviebox.app.core.CatalogItem
import com.moviebox.app.core.MovieBoxRepository
import com.moviebox.app.core.ProviderInfo
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

data class HomeUiState(
    val providers: List<ProviderInfo> = emptyList(),
    val selectedProvider: String = "moviebox",
    val query: String = "",
    val results: List<CatalogItem> = emptyList(),
    val loading: Boolean = false,
    val error: String? = null,
)

class MovieBoxViewModel(app: Application) : AndroidViewModel(app) {

    private val repository = MovieBoxRepository().also { it.initialize(app) }

    private val _state = MutableStateFlow(HomeUiState())
    val state: StateFlow<HomeUiState> = _state.asStateFlow()

    private var searchJob: Job? = null

    init {
        viewModelScope.launch {
            runCatching { repository.providers() }
                .onSuccess { providers ->
                    _state.update { it.copy(providers = providers) }
                }
                .onFailure { error ->
                    _state.update { it.copy(error = error.message) }
                }
        }
    }

    fun onQueryChange(query: String) {
        _state.update { it.copy(query = query) }
    }

    fun selectProvider(providerId: String) {
        if (_state.value.selectedProvider == providerId) return
        _state.update { it.copy(selectedProvider = providerId) }
        if (_state.value.query.isNotBlank()) {
            search()
        }
    }

    fun search() {
        val current = _state.value
        val query = current.query.trim()
        if (query.isEmpty()) {
            _state.update { it.copy(results = emptyList(), error = null) }
            return
        }
        searchJob?.cancel()
        searchJob = viewModelScope.launch {
            delay(350) // debounce keystrokes
            _state.update { it.copy(loading = true, error = null) }
            runCatching { repository.search(current.selectedProvider, query, 1) }
                .onSuccess { items ->
                    _state.update { it.copy(results = items, loading = false) }
                }
                .onFailure { error ->
                    _state.update {
                        it.copy(
                            loading = false,
                            error = error.message ?: "Search failed",
                        )
                    }
                }
        }
    }

    fun clearError() {
        _state.update { it.copy(error = null) }
    }
}
