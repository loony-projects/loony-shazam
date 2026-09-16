package com.loonyshazam.app.data.repository

import com.loonyshazam.app.data.db.HistoryDao
import com.loonyshazam.app.data.db.HistoryEntity
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow

/** In-memory [HistoryDao] fake so repository tests don't need a real Room/Robolectric setup. */
class FakeHistoryDao : HistoryDao {
    private var nextId = 1L
    private val _entries = MutableStateFlow<List<HistoryEntity>>(emptyList())
    val entries: StateFlow<List<HistoryEntity>> = _entries

    override suspend fun insert(entity: HistoryEntity): Long {
        val withId = entity.copy(id = nextId++)
        _entries.value = _entries.value + withId
        return withId.id
    }

    override fun observeAll() = entries

    override suspend fun getById(id: Long): HistoryEntity? = _entries.value.find { it.id == id }
}
