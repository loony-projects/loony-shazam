package com.loonyshazam.app.audio

import android.Manifest
import android.content.Context
import android.content.pm.PackageManager
import android.media.AudioFormat
import android.media.AudioRecord
import android.media.MediaRecorder
import androidx.core.content.ContextCompat
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.flowOn
import kotlinx.coroutines.isActive
import kotlinx.coroutines.Dispatchers

/** Recording constants, kept in one place so behavior is easy to reason about. */
object AudioConfig {
    /** Hard cap on a single recognition attempt, per product spec. */
    const val MAX_RECORDING_SECONDS = 12

    /**
     * Preferred sample rate first, then a standard fallback ladder for
     * devices/emulators whose mic HAL doesn't support 44.1kHz capture.
     */
    val CANDIDATE_SAMPLE_RATES = intArrayOf(44_100, 22_050, 16_000, 11_025, 8_000)

    const val CHANNEL_CONFIG = AudioFormat.CHANNEL_IN_MONO
    const val AUDIO_ENCODING = AudioFormat.ENCODING_PCM_16BIT
}

/** One event emitted while recording. */
sealed class AudioChunk {
    /** Emitted exactly once, before any samples, once a sample rate is chosen. */
    data class Format(val sampleRate: Int) : AudioChunk()

    /** A batch of freshly-captured mono PCM16 samples. */
    data class Samples(val pcm: ShortArray) : AudioChunk()
}

/** Thrown when no candidate sample rate is usable on this device. */
class UnsupportedAudioConfigurationException(message: String) : Exception(message)

/**
 * Abstraction over "a thing that can capture mono PCM16 audio", so
 * consumers (namely [com.loonyshazam.app.viewmodel.RecognitionViewModel])
 * can be unit-tested with a fake implementation instead of a real
 * [android.media.AudioRecord], which cannot run in a plain JVM unit test.
 */
interface AudioSource {
    fun record(): Flow<AudioChunk>
}

/**
 * Thin coroutine wrapper around [AudioRecord]. Records raw mono PCM16 audio
 * from the microphone (not `MediaRecorder` - the product needs raw samples
 * for WAV encoding, not a compressed container) and emits it as a cold
 * [Flow], capped at [AudioConfig.MAX_RECORDING_SECONDS].
 *
 * The flow is fully cooperative: cancelling the collecting coroutine (e.g.
 * because the ViewModel scope is cleared, or the caller explicitly cancels
 * on `onStop`) unwinds the `try/finally` below and releases the
 * [AudioRecord] immediately - no leaked native audio resources.
 */
class AudioRecorder(private val context: Context) : AudioSource {

    fun hasRecordAudioPermission(): Boolean =
        ContextCompat.checkSelfPermission(
            context,
            Manifest.permission.RECORD_AUDIO
        ) == PackageManager.PERMISSION_GRANTED

    /**
     * Records up to [AudioConfig.MAX_RECORDING_SECONDS] seconds of mono
     * PCM16 audio. Throws [SecurityException] if the permission isn't
     * granted, or [UnsupportedAudioConfigurationException] if this device
     * exposes no usable capture configuration.
     */
    override fun record(): Flow<AudioChunk> = flow {
        if (!hasRecordAudioPermission()) {
            throw SecurityException("RECORD_AUDIO permission not granted")
        }

        val (sampleRate, minBufferBytes) = selectSupportedConfig()
        emit(AudioChunk.Format(sampleRate))

        // A few multiples of the minimum buffer keeps AudioRecord from
        // dropping frames under normal scheduling jitter, without wasting
        // much memory.
        val bufferSizeBytes = minBufferBytes * 4
        val audioRecord = AudioRecord(
            MediaRecorder.AudioSource.MIC,
            sampleRate,
            AudioConfig.CHANNEL_CONFIG,
            AudioConfig.AUDIO_ENCODING,
            bufferSizeBytes
        )

        if (audioRecord.state != AudioRecord.STATE_INITIALIZED) {
            audioRecord.release()
            throw UnsupportedAudioConfigurationException(
                "AudioRecord failed to initialize at ${sampleRate}Hz"
            )
        }

        try {
            audioRecord.startRecording()

            val maxSamples = sampleRate.toLong() * AudioConfig.MAX_RECORDING_SECONDS
            var samplesRead = 0L
            val readBuffer = ShortArray(bufferSizeBytes / 2)

            while (samplesRead < maxSamples && currentCoroutineContext().isActive) {
                val n = audioRecord.read(readBuffer, 0, readBuffer.size)
                if (n > 0) {
                    emit(AudioChunk.Samples(readBuffer.copyOf(n)))
                    samplesRead += n
                } else if (n < 0) {
                    // Negative return values are AudioRecord error codes
                    // (ERROR_INVALID_OPERATION, ERROR_BAD_VALUE, ERROR_DEAD_OBJECT, ...).
                    throw IllegalStateException("AudioRecord.read failed with error code $n")
                }
            }
        } finally {
            // Always runs, including on coroutine cancellation - this is
            // what guarantees no leaked native recording resource if the
            // app is backgrounded mid-recording.
            runCatching { audioRecord.stop() }
            audioRecord.release()
        }
    }.flowOn(Dispatchers.IO)

    private fun selectSupportedConfig(): Pair<Int, Int> {
        for (rate in AudioConfig.CANDIDATE_SAMPLE_RATES) {
            val minBuf = AudioRecord.getMinBufferSize(
                rate,
                AudioConfig.CHANNEL_CONFIG,
                AudioConfig.AUDIO_ENCODING
            )
            if (minBuf > 0) {
                return rate to minBuf
            }
        }
        throw UnsupportedAudioConfigurationException(
            "No supported PCM16 mono sample rate among ${AudioConfig.CANDIDATE_SAMPLE_RATES.toList()}"
        )
    }
}
