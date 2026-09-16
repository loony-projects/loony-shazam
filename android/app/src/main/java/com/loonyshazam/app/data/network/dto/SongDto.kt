package com.loonyshazam.app.data.network.dto

import com.google.gson.annotations.SerializedName

/**
 * Wire shape of `song` inside a recognition response.
 *
 * Note: [id] is a **string** on the wire (a stringified i64 from the Rust
 * backend), not a number - do not change this to Long.
 */
data class SongDto(
    @SerializedName("id") val id: String,
    @SerializedName("title") val title: String,
    @SerializedName("artist") val artist: String,
    @SerializedName("album") val album: String?,
    @SerializedName("duration_ms") val durationMs: Int,
    @SerializedName("artwork_url") val artworkUrl: String?
)
