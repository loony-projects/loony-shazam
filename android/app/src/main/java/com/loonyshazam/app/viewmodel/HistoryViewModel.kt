package com.loonyshazam.app.viewmodel

import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import com.loonyshazam.app.data.db.HistoryEntity
import com.loonyshazam.app.data.repository.RecognitionRepository
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharingStarted
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.stateIn
import kotlinx.coroutines.launch

/** Backs the History screen and history detail lookups. */
class HistoryViewModel(private val repository: RecognitionRepository) : ViewModel() {

    val history: StateFlow<List<HistoryEntity>> = repository.observeHistory()
        .stateIn(viewModelScope, SharingStarted.WhileSubscribed(5_000), emptyList())

    private val _selected = MutableStateFlow<HistoryEntity?>(null)
    val selected: StateFlow<HistoryEntity?> = _selected.asStateFlow()

    fun loadDetail(id: Long) {
        viewModelScope.launch {
            _selected.value = repository.getHistoryEntry(id)
        }
    }

    class Factory(private val repository: RecognitionRepository) : ViewModelProvider.Factory {
        override fun <T : ViewModel> create(modelClass: Class<T>): T {
            @Suppress("UNCHECKED_CAST")
            return HistoryViewModel(repository) as T
        }
    }
}
