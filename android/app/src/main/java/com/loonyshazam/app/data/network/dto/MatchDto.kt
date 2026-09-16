package com.loonyshazam.app.data.network.dto

import com.google.gson.annotations.SerializedName

/** Wire shape of the `match` object present on every recognition response. */
data class MatchDto(
    @SerializedName("score") val score: Double,
    @SerializedName("confidence") val confidence: Double,
    @SerializedName("matched_fingerprints") val matchedFingerprints: Int,
    @SerializedName("query_fingerprints") val queryFingerprints: Int,
    @SerializedName("offset_ms") val offsetMs: Long?,
    @SerializedName("algorithm_version") val algorithmVersion: Int,
    @SerializedName("latency_ms") val latencyMs: Long
)
