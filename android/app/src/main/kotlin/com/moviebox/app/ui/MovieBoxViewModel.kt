package com.moviebox.app.ui

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import com.moviebox.app.core.CatalogItem
import com.moviebox.app.core.FavoriteItem
import com.moviebox.app.core.MovieBoxRepository
import com.moviebox.app.core.ProviderInfo
import com.moviebox.app.core.WatchHistoryItem
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

enum class MainTab(val title: String) {
    HOME("Home"),
    FAVORITES("Favorites"),
    HISTORY("History"),
}

data class HomeDeckTab(
    val id: String,
    val title: String,
)

val DEFAULT_HOME_DECK_TABS = listOf(
    HomeDeckTab("0", "Featured"),
    HomeDeckTab("1", "Movies"),
    HomeDeckTab("2", "Series"),
)

data class HomeUiState(
    val selectedTab: MainTab = MainTab.HOME,
    val providers: List<ProviderInfo> = emptyList(),
    val selectedProvider: String = "moviebox",
    val query: String = "",
    val results: List<CatalogItem> = emptyList(),
    val homepageItems: List<CatalogItem> = emptyList(),
    val selectedHomeDeckTab: String = "0",
    val continueWatching: List<WatchHistoryItem> = emptyList(),
    val favorites: List<FavoriteItem> = emptyList(),
    val history: List<WatchHistoryItem> = emptyList(),
    val loading: Boolean = false,
    val deckLoading: Boolean = false,
    val favoritesLoading: Boolean = false,
    val historyLoading: Boolean = false,
    val error: String? = null,
) {
    val activeProviderInfo: ProviderInfo?
        get() = providers.firstOrNull { it.id == selectedProvider }

    val supportsHomepage: Boolean
        get() = activeProviderInfo?.supportsHomepage == true
}

class MovieBoxViewModel(app: Application) : AndroidViewModel(app) {

    private val repository = MovieBoxRepository().also { it.initialize(app) }

    private val _state = MutableStateFlow(HomeUiState())
    val state: StateFlow<HomeUiState> = _state.asStateFlow()

    private var searchJob: Job? = null
    private var deckJob: Job? = null

    init {
        loadProviders()
        loadHomeDeck("0")
        loadLibrary()
    }

    private fun loadProviders() {
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

    fun selectMainTab(tab: MainTab) {
        _state.update { it.copy(selectedTab = tab) }
        when (tab) {
            MainTab.HOME -> {
                loadLibrary()
                if (_state.value.query.isBlank() && _state.value.homepageItems.isEmpty()) {
                    loadHomeDeck(_state.value.selectedHomeDeckTab)
                }
            }
            MainTab.FAVORITES -> loadFavorites()
            MainTab.HISTORY -> loadHistory()
        }
    }

    fun selectHomeDeckTab(tabId: String) {
        if (_state.value.selectedHomeDeckTab == tabId && _state.value.homepageItems.isNotEmpty()) return
        _state.update { it.copy(selectedHomeDeckTab = tabId) }
        loadHomeDeck(tabId)
    }

    fun loadHomeDeck(tabId: String = _state.value.selectedHomeDeckTab) {
        deckJob?.cancel()
        deckJob = viewModelScope.launch {
            _state.update { it.copy(deckLoading = true) }
            runCatching { repository.homepage(tabId, 1) }
                .onSuccess { items ->
                    _state.update { it.copy(homepageItems = items, deckLoading = false) }
                }
                .onFailure { error ->
                    _state.update { it.copy(deckLoading = false) }
                }
        }
    }

    fun loadLibrary() {
        loadContinueWatching()
        loadFavorites()
    }

    fun loadContinueWatching() {
        viewModelScope.launch {
            runCatching { repository.history() }
                .onSuccess { items ->
                    val inProgress = items.filter { it.isInProgress }
                    _state.update { it.copy(continueWatching = inProgress, history = items) }
                }
        }
    }

    fun loadFavorites() {
        viewModelScope.launch {
            _state.update { it.copy(favoritesLoading = true) }
            runCatching { repository.favorites() }
                .onSuccess { items ->
                    _state.update { it.copy(favorites = items, favoritesLoading = false) }
                }
                .onFailure {
                    _state.update { it.copy(favoritesLoading = false) }
                }
        }
    }

    fun loadHistory() {
        viewModelScope.launch {
            _state.update { it.copy(historyLoading = true) }
            runCatching { repository.history() }
                .onSuccess { items ->
                    _state.update {
                        it.copy(
                            history = items,
                            continueWatching = items.filter { item -> item.isInProgress },
                            historyLoading = false,
                        )
                    }
                }
                .onFailure {
                    _state.update { it.copy(historyLoading = false) }
                }
        }
    }

    fun toggleFavorite(item: FavoriteItem) {
        viewModelScope.launch {
            runCatching { repository.toggleFavorite(item) }
                .onSuccess {
                    loadFavorites()
                }
        }
    }

    fun onQueryChange(query: String) {
        _state.update { it.copy(query = query) }
        if (query.isBlank()) {
            _state.update { it.copy(results = emptyList(), error = null) }
            if (_state.value.homepageItems.isEmpty() && _state.value.supportsHomepage) {
                loadHomeDeck(_state.value.selectedHomeDeckTab)
            }
        } else {
            search()
        }
    }

    fun selectProvider(providerId: String) {
        if (_state.value.selectedProvider == providerId) return
        _state.update { it.copy(selectedProvider = providerId) }
        val current = _state.value
        if (current.query.isNotBlank()) {
            search()
        } else if (current.supportsHomepage) {
            loadHomeDeck(current.selectedHomeDeckTab)
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
