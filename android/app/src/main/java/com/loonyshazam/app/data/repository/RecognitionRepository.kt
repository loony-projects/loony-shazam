package com.loonyshazam.app.data.repository

import com.google.gson.Gson
import com.loonyshazam.app.data.db.HistoryDao
import com.loonyshazam.app.data.db.HistoryEntity
import com.loonyshazam.app.data.model.MatchInfo
import com.loonyshazam.app.data.model.RecognitionOutcome
import com.loonyshazam.app.data.model.Song
import com.loonyshazam.app.data.network.RecognitionApi
import com.loonyshazam.app.data.network.dto.ErrorResponseDto
import com.loonyshazam.app.data.network.dto.RecognitionResponseDto
import kotlinx.coroutines.flow.Flow
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.MultipartBody
import okhttp3.RequestBody.Companion.toRequestBody
import java.io.IOException

/** Thrown when the backend returns a well-formed 4xx/5xx error response. */
class RecognitionApiException(message: String) : IOException(message)

/**
 * Single source of truth for recognizing audio against the backend and
 * persisting recognized results to local history. Declared as an interface
 * (implemented by [DefaultRecognitionRepository]) so ViewModels can be unit
 * tested against a scripted fake instead of real networking/Room.
 */
interface RecognitionRepository {
    /**
     * Uploads [wavBytes] for recognition. Implementations should apply at
     * most one retry, and only for network-level (I/O) failures.
     */
    suspend fun recognize(wavBytes: ByteArray): Result<RecognitionOutcome>

    /** Persists a recognized song into local history. Never stores audio. */
    suspend fun saveToHistory(song: Song, recognizedAt: Long)

    fun observeHistory(): Flow<List<HistoryEntity>>

    suspend fun getHistoryEntry(id: Long): HistoryEntity?
}

/**
 * Default [RecognitionRepository]: talks to the real backend via Retrofit
 * and persists to a real Room [HistoryDao]. The raw query audio bytes are
 * only ever held in memory long enough to build the multipart request body
 * - never written to the database.
 */
class DefaultRecognitionRepository(
    private val api: RecognitionApi,
    private val historyDao: HistoryDao,
    private val gson: Gson = Gson()
) : RecognitionRepository {

    override suspend fun recognize(wavBytes: ByteArray): Result<RecognitionOutcome> {
        val part = MultipartBody.Part.createFormData(
            "audio",
            "query.wav",
            wavBytes.toRequestBody("audio/wav".toMediaType())
        )

        var lastNetworkError: IOException? = null
        val maxAttempts = 2
        for (attempt in 1..maxAttempts) {
            try {
                val response = api.recognizeAudio(part)
                return if (response.isSuccessful) {
                    val body = response.body()
                        ?: return Result.failure(IOException("Empty response body"))
                    Result.success(body.toDomain())
                } else {
                    Result.failure(RecognitionApiException(extractErrorMessage(response)))
                }
            } catch (e: IOException) {
                lastNetworkError = e
                // Loop again for the bounded retry; falls through to failure below
                // once attempts are exhausted.
            }
        }
        return Result.failure(lastNetworkError ?: IOException("Unknown network error"))
    }

    private fun extractErrorMessage(response: retrofit2.Response<RecognitionResponseDto>): String {
        val raw = response.errorBody()?.string()
        val parsed = raw?.let { runCatching { gson.fromJson(it, ErrorResponseDto::class.java) }.getOrNull() }
        return parsed?.message
            ?: parsed?.error
            ?: "Request failed with HTTP ${response.code()}"
    }

    override suspend fun saveToHistory(song: Song, recognizedAt: Long) {
        historyDao.insert(
            HistoryEntity(
                songId = song.id,
                title = song.title,
                artist = song.artist,
                album = song.album,
                artworkUrl = song.artworkUrl,
                recognizedAt = recognizedAt
            )
        )
    }

    override fun observeHistory(): Flow<List<HistoryEntity>> = historyDao.observeAll()

    override suspend fun getHistoryEntry(id: Long): HistoryEntity? = historyDao.getById(id)
}

private fun RecognitionResponseDto.toDomain(): RecognitionOutcome {
    val matchInfo = MatchInfo(
        score = match.score,
        confidence = match.confidence,
        matchedFingerprints = match.matchedFingerprints,
        queryFingerprints = match.queryFingerprints,
        offsetMs = match.offsetMs,
        algorithmVersion = match.algorithmVersion,
        latencyMs = match.latencyMs
    )
    return if (recognized && song != null) {
        RecognitionOutcome.Recognized(
            song = Song(
                id = song.id,
                title = song.title,
                artist = song.artist,
                album = song.album,
                durationMs = song.durationMs,
                artworkUrl = song.artworkUrl
            ),
            match = matchInfo
        )
    } else {
        RecognitionOutcome.NotRecognized(reason = reason ?: "NO_MATCH", match = matchInfo)
    }
}
