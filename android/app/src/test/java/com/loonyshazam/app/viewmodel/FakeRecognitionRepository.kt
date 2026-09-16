package com.loonyshazam.app.viewmodel

import com.loonyshazam.app.data.db.HistoryEntity
import com.loonyshazam.app.data.model.RecognitionOutcome
import com.loonyshazam.app.data.model.Song
import com.loonyshazam.app.data.repository.RecognitionRepository
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.yield

/**
 * Scripted [RecognitionRepository] fake for [RecognitionViewModel] tests -
 * no real networking or Room involved. [recognize] yields before returning
 * so tests observe a real suspension boundary between the `Processing`
 * state and the terminal state, matching how a real network round trip
 * would behave.
 */
class FakeRecognitionRepository(
    private val recognizeResult: Result<RecognitionOutcome>
) : RecognitionRepository {

    val savedHistory = MutableStateFlow<List<Pair<Song, Long>>>(emptyList())

    override suspend fun recognize(wavBytes: ByteArray): Result<RecognitionOutcome> {
        yield()
        return recognizeResult
    }

    override suspend fun saveToHistory(song: Song, recognizedAt: Long) {
        savedHistory.value = savedHistory.value + (song to recognizedAt)
    }

    override fun observeHistory() = MutableStateFlow<List<HistoryEntity>>(emptyList())

    override suspend fun getHistoryEntry(id: Long): HistoryEntity? = null
}
