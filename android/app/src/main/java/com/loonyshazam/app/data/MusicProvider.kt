package com.loonyshazam.app.data

import android.content.Intent
import com.loonyshazam.app.data.model.Song

/**
 * Extensibility point for opening a recognized song in an external music
 * app (e.g. a future Spotify/Apple Music deep link). Intentionally not
 * wired up to any real provider or API credentials in this build - see
 * docs/android.md.
 */
interface MusicProvider {
    /** Returns an [Intent] to open [song] externally, or null if unsupported. */
    fun openInExternalApp(song: Song): Intent?
}

/** Default implementation: no external provider is configured yet. */
class NoOpMusicProvider : MusicProvider {
    override fun openInExternalApp(song: Song): Intent? = null
}
