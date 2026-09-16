package com.loonyshazam.app.data.network.dto

import com.google.gson.annotations.SerializedName

/**
 * Wire shape of `POST /api/v1/recognitions/audio`'s success/no-match body
 * (HTTP 200 either way - see docs/architecture.md and docs/android.md).
 */
data class RecognitionResponseDto(
    @SerializedName("recognized") val recognized: Boolean,
    @SerializedName("song") val song: SongDto?,
    @SerializedName("reason") val reason: String?,
    @SerializedName("match") val match: MatchDto
)

/** Wire shape of a 4xx/5xx error body: `{"error": "CODE", "message": "..."}`. */
data class ErrorResponseDto(
    @SerializedName("error") val error: String?,
    @SerializedName("message") val message: String?
)
