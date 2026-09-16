package com.loonyshazam.app.audio

/**
 * Growable buffer of PCM16 samples. A plain `ArrayList<Short>` would box
 * every sample (4x+ memory and GC churn for ~0.5-1M samples per
 * recognition attempt); this keeps everything in a primitive `ShortArray`
 * and doubles capacity like `ArrayList` does internally.
 */
class PcmAccumulator(initialCapacity: Int = 44_100 * 2) {
    private var buffer = ShortArray(initialCapacity.coerceAtLeast(16))

    var size: Int = 0
        private set

    fun append(chunk: ShortArray, length: Int = chunk.size) {
        ensureCapacity(size + length)
        System.arraycopy(chunk, 0, buffer, size, length)
        size += length
    }

    fun toShortArray(): ShortArray = buffer.copyOf(size)

    private fun ensureCapacity(minCapacity: Int) {
        if (minCapacity <= buffer.size) return
        var newCapacity = buffer.size * 2
        while (newCapacity < minCapacity) newCapacity *= 2
        buffer = buffer.copyOf(newCapacity)
    }
}
