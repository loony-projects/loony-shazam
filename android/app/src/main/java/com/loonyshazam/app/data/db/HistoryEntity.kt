package com.loonyshazam.app.data.db

import androidx.room.Entity
import androidx.room.PrimaryKey

/**
 * A single locally-persisted past recognition. Only metadata is stored -
 * the raw query audio is never written to disk or the database, matching
 * the backend's own "never retain query audio" policy
 * (see docs/architecture.md).
 */
@Entity(tableName = "history")
data class HistoryEntity(
    @PrimaryKey(autoGenerate = true) val id: Long = 0,
    val songId: String,
    val title: String,
    val artist: String,
    val album: String?,
    val artworkUrl: String?,
    val recognizedAt: Long
)
