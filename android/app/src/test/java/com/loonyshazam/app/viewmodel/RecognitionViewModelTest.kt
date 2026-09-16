package com.loonyshazam.app.viewmodel

import app.cash.turbine.test
import com.google.common.truth.Truth.assertThat
import com.loonyshazam.app.data.model.MatchInfo
import com.loonyshazam.app.data.model.RecognitionOutcome
import com.loonyshazam.app.data.model.Song
import kotlinx.coroutines.test.runTest
import org.junit.Rule
import org.junit.Test

/**
 * Verifies the Idle -> Listening -> Processing -> Result | NotFound | Error
 * state machine described in docs/architecture.md, driven entirely through
 * fakes (no real mic, no real network).
 */
class RecognitionViewModelTest {

    @get:Rule
    val mainDispatcherRule = MainDispatcherRule()

    private val song = Song(
        id = "3",
        title = "Song Alpha",
        artist = "Test Artist",
        album = null,
        durationMs = 20_000,
        artworkUrl = null
    )

    private val match = MatchInfo(
        score = 119.46,
        confidence = 0.49,
        matchedFingerprints = 145,
        queryFingerprints = 294,
        offsetMs = 9_000,
        algorithmVersion = 1,
        latencyMs = 666
    )

    @Test
    fun `successful recognition transitions Idle to Listening to Processing to Result and saves history`() = runTest(mainDispatcherRule.dispatcher) {
        val audioSource = FakeAudioSource(chunks = listOf(shortArrayOf(1, 2, 3)))
        val repository = FakeRecognitionRepository(Result.success(RecognitionOutcome.Recognized(song, match)))
        val viewModel = RecognitionViewModel(audioSource, repository, clock = { 1_000L })

        viewModel.uiState.test {
            assertThat(awaitItem()).isEqualTo(RecognitionUiState.Idle)

            viewModel.startListening()

            assertThat(awaitItem()).isInstanceOf(RecognitionUiState.Listening::class.java)
            assertThat(awaitItem()).isEqualTo(RecognitionUiState.Processing)
            val result = awaitItem()
            assertThat(result).isEqualTo(RecognitionUiState.Result(song, match, 1_000L))

            cancelAndIgnoreRemainingEvents()
        }

        assertThat(repository.savedHistory.value).containsExactly(song to 1_000L)
    }

    @Test
    fun `not-recognized outcome transitions to NotFound without saving history`() = runTest(mainDispatcherRule.dispatcher) {
        val audioSource = FakeAudioSource()
        val repository = FakeRecognitionRepository(
            Result.success(RecognitionOutcome.NotRecognized("NO_MATCH", match.copy(confidence = 0.0)))
        )
        val viewModel = RecognitionViewModel(audioSource, repository)

        viewModel.uiState.test {
            assertThat(awaitItem()).isEqualTo(RecognitionUiState.Idle)
            viewModel.startListening()
            assertThat(awaitItem()).isInstanceOf(RecognitionUiState.Listening::class.java)
            assertThat(awaitItem()).isEqualTo(RecognitionUiState.Processing)
            assertThat(awaitItem()).isEqualTo(RecognitionUiState.NotFound("NO_MATCH"))
            cancelAndIgnoreRemainingEvents()
        }

        assertThat(repository.savedHistory.value).isEmpty()
    }

    @Test
    fun `repository failure transitions to Error with the failure message`() = runTest(mainDispatcherRule.dispatcher) {
        val audioSource = FakeAudioSource()
        val repository = FakeRecognitionRepository(Result.failure(java.io.IOException("network down")))
        val viewModel = RecognitionViewModel(audioSource, repository)

        viewModel.uiState.test {
            assertThat(awaitItem()).isEqualTo(RecognitionUiState.Idle)
            viewModel.startListening()
            assertThat(awaitItem()).isInstanceOf(RecognitionUiState.Listening::class.java)
            assertThat(awaitItem()).isEqualTo(RecognitionUiState.Processing)
            assertThat(awaitItem()).isEqualTo(RecognitionUiState.Error("network down"))
            cancelAndIgnoreRemainingEvents()
        }
    }

    @Test
    fun `SecurityException from the audio source becomes PermissionDenied`() = runTest(mainDispatcherRule.dispatcher) {
        val audioSource = FakeAudioSource(failure = SecurityException("no permission"))
        val repository = FakeRecognitionRepository(Result.success(RecognitionOutcome.NotRecognized("NO_MATCH", match)))
        val viewModel = RecognitionViewModel(audioSource, repository)

        viewModel.uiState.test {
            assertThat(awaitItem()).isEqualTo(RecognitionUiState.Idle)
            viewModel.startListening()
            // Listening(0) is set before recording actually starts, so it's
            // observed briefly even though the mic never really opens.
            assertThat(awaitItem()).isInstanceOf(RecognitionUiState.Listening::class.java)
            assertThat(awaitItem()).isEqualTo(RecognitionUiState.PermissionDenied)
            cancelAndIgnoreRemainingEvents()
        }
    }

    @Test
    fun `onPermissionDenied sets PermissionDenied state directly`() = runTest(mainDispatcherRule.dispatcher) {
        val viewModel = RecognitionViewModel(
            FakeAudioSource(),
            FakeRecognitionRepository(Result.success(RecognitionOutcome.NotRecognized("NO_MATCH", match)))
        )

        viewModel.uiState.test {
            assertThat(awaitItem()).isEqualTo(RecognitionUiState.Idle)
            viewModel.onPermissionDenied()
            assertThat(awaitItem()).isEqualTo(RecognitionUiState.PermissionDenied)
            cancelAndIgnoreRemainingEvents()
        }
    }

    @Test
    fun `reset returns to Idle from a terminal state`() = runTest(mainDispatcherRule.dispatcher) {
        val viewModel = RecognitionViewModel(
            FakeAudioSource(),
            FakeRecognitionRepository(Result.success(RecognitionOutcome.Recognized(song, match)))
        )

        viewModel.uiState.test {
            assertThat(awaitItem()).isEqualTo(RecognitionUiState.Idle)
            viewModel.startListening()
            skipItems(2) // Listening, Processing
            assertThat(awaitItem()).isInstanceOf(RecognitionUiState.Result::class.java)

            viewModel.reset()
            assertThat(awaitItem()).isEqualTo(RecognitionUiState.Idle)

            cancelAndIgnoreRemainingEvents()
        }
    }
}
