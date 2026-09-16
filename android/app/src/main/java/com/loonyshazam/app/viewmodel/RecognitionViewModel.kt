package com.loonyshazam.app.viewmodel

import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import com.loonyshazam.app.audio.AudioChunk
import com.loonyshazam.app.audio.AudioSource
import com.loonyshazam.app.audio.PcmAccumulator
import com.loonyshazam.app.audio.WavEncoder
import com.loonyshazam.app.data.model.MatchInfo
import com.loonyshazam.app.data.model.RecognitionOutcome
import com.loonyshazam.app.data.model.Song
import com.loonyshazam.app.data.repository.RecognitionRepository
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

/** The recognition state machine: Idle -> Listening -> Processing -> Result | NotFound | Error. */
sealed class RecognitionUiState {
    object Idle : RecognitionUiState()
    data class Listening(val elapsedSeconds: Int) : RecognitionUiState()
    object Processing : RecognitionUiState()
    data class Result(val song: Song, val match: MatchInfo, val recognizedAt: Long) : RecognitionUiState()
    data class NotFound(val reason: String) : RecognitionUiState()
    data class Error(val message: String) : RecognitionUiState()
    object PermissionDenied : RecognitionUiState()
}

/**
 * Drives the mic -> upload -> result flow. Recording and network I/O both
 * run in [viewModelScope], so both are cancelled automatically when the
 * ViewModel is cleared, and can be cancelled explicitly via
 * [cancelListening] (e.g. when the host Activity goes to the background).
 */
class RecognitionViewModel(
    private val audioRecorder: AudioSource,
    private val repository: RecognitionRepository,
    private val clock: () -> Long = { System.currentTimeMillis() }
) : ViewModel() {

    private val _uiState = MutableStateFlow<RecognitionUiState>(RecognitionUiState.Idle)
    val uiState: StateFlow<RecognitionUiState> = _uiState.asStateFlow()

    private var recordingJob: Job? = null

    /** Call when the mic button is tapped and RECORD_AUDIO is already granted. */
    fun startListening() {
        if (_uiState.value is RecognitionUiState.Listening || _uiState.value is RecognitionUiState.Processing) {
            return
        }
        recordingJob?.cancel()
        recordingJob = viewModelScope.launch {
            _uiState.value = RecognitionUiState.Listening(0)
            try {
                val (pcm, sampleRate) = captureAudio()
                _uiState.value = RecognitionUiState.Processing
                val wavBytes = WavEncoder.encode(pcm, sampleRate)
                val result = repository.recognize(wavBytes)
                result.fold(
                    onSuccess = { outcome -> handleOutcome(outcome) },
                    onFailure = { error ->
                        _uiState.value = RecognitionUiState.Error(
                            error.message ?: "Something went wrong. Please try again."
                        )
                    }
                )
            } catch (e: CancellationException) {
                throw e
            } catch (e: SecurityException) {
                _uiState.value = RecognitionUiState.PermissionDenied
            } catch (e: Exception) {
                _uiState.value = RecognitionUiState.Error(e.message ?: "Recording failed")
            }
        }
    }

    /** Call when the RECORD_AUDIO permission request was denied. */
    fun onPermissionDenied() {
        _uiState.value = RecognitionUiState.PermissionDenied
    }

    /** Cancels an in-flight recording (e.g. app backgrounded), returning to Idle. */
    fun cancelListening() {
        if (_uiState.value is RecognitionUiState.Listening) {
            recordingJob?.cancel()
            _uiState.value = RecognitionUiState.Idle
        }
    }

    /** Returns to Idle from a terminal state (Result/NotFound/Error/PermissionDenied). */
    fun reset() {
        recordingJob?.cancel()
        _uiState.value = RecognitionUiState.Idle
    }

    private suspend fun captureAudio(): Pair<ShortArray, Int> {
        val accumulator = PcmAccumulator()
        var sampleRate = com.loonyshazam.app.audio.AudioConfig.CANDIDATE_SAMPLE_RATES.first()
        var lastReportedSecond = -1

        audioRecorder.record().collect { chunk ->
            when (chunk) {
                is AudioChunk.Format -> sampleRate = chunk.sampleRate
                is AudioChunk.Samples -> {
                    accumulator.append(chunk.pcm)
                    val elapsedSeconds = accumulator.size / sampleRate
                    if (elapsedSeconds != lastReportedSecond) {
                        lastReportedSecond = elapsedSeconds
                        _uiState.value = RecognitionUiState.Listening(elapsedSeconds)
                    }
                }
            }
        }
        return accumulator.toShortArray() to sampleRate
    }

    private suspend fun handleOutcome(outcome: RecognitionOutcome) {
        when (outcome) {
            is RecognitionOutcome.Recognized -> {
                val recognizedAt = clock()
                repository.saveToHistory(outcome.song, recognizedAt)
                _uiState.value = RecognitionUiState.Result(outcome.song, outcome.match, recognizedAt)
            }
            is RecognitionOutcome.NotRecognized -> {
                _uiState.value = RecognitionUiState.NotFound(outcome.reason)
            }
        }
    }

    override fun onCleared() {
        super.onCleared()
        recordingJob?.cancel()
    }

    class Factory(
        private val audioRecorder: AudioSource,
        private val repository: RecognitionRepository
    ) : ViewModelProvider.Factory {
        override fun <T : ViewModel> create(modelClass: Class<T>): T {
            @Suppress("UNCHECKED_CAST")
            return RecognitionViewModel(audioRecorder, repository) as T
        }
    }
}
