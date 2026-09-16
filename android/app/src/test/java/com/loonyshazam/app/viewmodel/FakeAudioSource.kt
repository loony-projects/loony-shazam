package com.loonyshazam.app.viewmodel

import com.loonyshazam.app.audio.AudioChunk
import com.loonyshazam.app.audio.AudioSource
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.yield

/**
 * Test double for [AudioSource] with fully controllable emission/failure
 * behavior. Yields after each emission so tests driven by
 * `StandardTestDispatcher` see a real suspension boundary between chunks
 * (and thus between resulting `RecognitionUiState.Listening` updates)
 * instead of the whole recording collapsing into one synchronous burst.
 */
class FakeAudioSource(
    private val sampleRate: Int = 44_100,
    private val chunks: List<ShortArray> = listOf(shortArrayOf(1, 2, 3, 4, 5)),
    private val failure: Throwable? = null
) : AudioSource {
    override fun record(): Flow<AudioChunk> = flow {
        failure?.let { throw it }
        emit(AudioChunk.Format(sampleRate))
        yield()
        for (chunk in chunks) {
            emit(AudioChunk.Samples(chunk))
            yield()
        }
    }
}
