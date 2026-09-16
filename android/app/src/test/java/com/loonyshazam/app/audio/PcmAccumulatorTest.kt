package com.loonyshazam.app.audio

import com.google.common.truth.Truth.assertThat
import org.junit.Test

class PcmAccumulatorTest {

    @Test
    fun `appends multiple chunks in order and grows past initial capacity`() {
        val accumulator = PcmAccumulator(initialCapacity = 4)

        accumulator.append(shortArrayOf(1, 2, 3))
        accumulator.append(shortArrayOf(4, 5, 6, 7, 8))

        assertThat(accumulator.size).isEqualTo(8)
        assertThat(accumulator.toShortArray()).isEqualTo(shortArrayOf(1, 2, 3, 4, 5, 6, 7, 8))
    }

    @Test
    fun `partial length append only copies the requested prefix`() {
        val accumulator = PcmAccumulator()
        accumulator.append(shortArrayOf(9, 9, 9, 1, 2), length = 3)

        assertThat(accumulator.size).isEqualTo(3)
        assertThat(accumulator.toShortArray()).isEqualTo(shortArrayOf(9, 9, 9))
    }
}
