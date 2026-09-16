package com.loonyshazam.app.data.model

/** Domain representation of a recognized song, decoupled from wire DTOs. */
data class Song(
    val id: String,
    val title: String,
    val artist: String,
    val album: String?,
    val durationMs: Int,
    val artworkUrl: String?
)

/** Domain representation of the match-quality metadata for one attempt. */
data class MatchInfo(
    val score: Double,
    val confidence: Double,
    val matchedFingerprints: Int,
    val queryFingerprints: Int,
    val offsetMs: Long?,
    val algorithmVersion: Int,
    val latencyMs: Long
)

/** Outcome of a single recognition request against the backend. */
sealed class RecognitionOutcome {
    data class Recognized(val song: Song, val match: MatchInfo) : RecognitionOutcome()
    data class NotRecognized(val reason: String, val match: MatchInfo) : RecognitionOutcome()
}
