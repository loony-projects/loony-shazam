package com.loonyshazam.app.audio

import com.google.common.truth.Truth.assertThat
import org.junit.Test
import java.nio.ByteBuffer
import java.nio.ByteOrder

class WavEncoderTest {

    @Test
    fun `header has correct RIFF WAVE chunk markers`() {
        val pcm = shortArrayOf(1, 2, 3, 4)
        val wav = WavEncoder.encode(pcm, sampleRate = 44_100)

        assertThat(String(wav, 0, 4, Charsets.US_ASCII)).isEqualTo("RIFF")
        assertThat(String(wav, 8, 4, Charsets.US_ASCII)).isEqualTo("WAVE")
        assertThat(String(wav, 12, 4, Charsets.US_ASCII)).isEqualTo("fmt ")
        assertThat(String(wav, 36, 4, Charsets.US_ASCII)).isEqualTo("data")
    }

    @Test
    fun `header encodes sample rate, channel count and bit depth correctly`() {
        val pcm = shortArrayOf(10, -20, 30, -40, 50)
        val sampleRate = 11_025
        val wav = WavEncoder.encode(pcm, sampleRate = sampleRate, channels = 1)
        val buf = ByteBuffer.wrap(wav).order(ByteOrder.LITTLE_ENDIAN)

        val chunkSize = buf.getInt(4)
        val subchunk1Size = buf.getInt(16)
        val audioFormat = buf.getShort(20)
        val numChannels = buf.getShort(22)
        val sampleRateRead = buf.getInt(24)
        val byteRate = buf.getInt(28)
        val blockAlign = buf.getShort(32)
        val bitsPerSample = buf.getShort(34)
        val dataSize = buf.getInt(40)

        assertThat(subchunk1Size).isEqualTo(16)
        assertThat(audioFormat).isEqualTo(1)
        assertThat(numChannels).isEqualTo(1)
        assertThat(sampleRateRead).isEqualTo(sampleRate)
        assertThat(blockAlign).isEqualTo(2)
        assertThat(bitsPerSample).isEqualTo(16)
        assertThat(byteRate).isEqualTo(sampleRate * 1 * 2)
        assertThat(dataSize).isEqualTo(pcm.size * 2)
        assertThat(chunkSize).isEqualTo(36 + dataSize)
        assertThat(wav.size).isEqualTo(44 + dataSize)
    }

    @Test
    fun `sample bytes are written little-endian in order after the header`() {
        val pcm = shortArrayOf(0x0102, -1, 5)
        val wav = WavEncoder.encode(pcm, sampleRate = 8_000)
        val buf = ByteBuffer.wrap(wav).order(ByteOrder.LITTLE_ENDIAN)

        assertThat(buf.getShort(44)).isEqualTo(0x0102.toShort())
        assertThat(buf.getShort(46)).isEqualTo((-1).toShort())
        assertThat(buf.getShort(48)).isEqualTo(5.toShort())
    }

    @Test
    fun `empty sample array still produces a valid 44-byte header`() {
        val wav = WavEncoder.encode(ShortArray(0), sampleRate = 44_100)
        assertThat(wav.size).isEqualTo(44)
    }
}
