package com.loonyshazam.app.audio

import java.nio.ByteBuffer
import java.nio.ByteOrder

/**
 * Encodes raw PCM16 mono samples into a minimal 44-byte-header canonical
 * WAV file, entirely in memory (no external library - this is a well-known,
 * fixed binary layout).
 */
object WavEncoder {
    private const val HEADER_SIZE = 44
    private const val PCM_FORMAT = 1
    private const val BITS_PER_SAMPLE = 16
    private const val BYTES_PER_SAMPLE = BITS_PER_SAMPLE / 8

    fun encode(pcm: ShortArray, sampleRate: Int, channels: Int = 1): ByteArray {
        val dataSize = pcm.size * BYTES_PER_SAMPLE
        val byteRate = sampleRate * channels * BYTES_PER_SAMPLE
        val blockAlign = channels * BYTES_PER_SAMPLE

        val out = ByteBuffer.allocate(HEADER_SIZE + dataSize).order(ByteOrder.LITTLE_ENDIAN)

        // RIFF chunk descriptor
        out.put("RIFF".toByteArray(Charsets.US_ASCII))
        out.putInt(36 + dataSize) // ChunkSize = 4 + (8 + SubChunk1Size) + (8 + SubChunk2Size)
        out.put("WAVE".toByteArray(Charsets.US_ASCII))

        // fmt subchunk
        out.put("fmt ".toByteArray(Charsets.US_ASCII))
        out.putInt(16) // Subchunk1Size for PCM
        out.putShort(PCM_FORMAT.toShort()) // AudioFormat = 1 (PCM, uncompressed)
        out.putShort(channels.toShort())
        out.putInt(sampleRate)
        out.putInt(byteRate)
        out.putShort(blockAlign.toShort())
        out.putShort(BITS_PER_SAMPLE.toShort())

        // data subchunk
        out.put("data".toByteArray(Charsets.US_ASCII))
        out.putInt(dataSize)
        for (sample in pcm) {
            out.putShort(sample)
        }

        return out.array()
    }
}
