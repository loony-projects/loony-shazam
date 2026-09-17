package com.loonyshazam.app.data

import android.content.Intent
import android.net.Uri
import com.loonyshazam.app.data.model.Song

/**
 * Extensibility point for opening a recognized song in an external music
 * app or service.
 */
interface MusicProvider {
    /** Returns an [Intent] to open [song] externally, or null if unsupported. */
    fun openInExternalApp(song: Song): Intent?
}

/**
 * Opens a web search for the song on YouTube Music. Deliberately not a
 * Spotify/Apple Music *API* integration — those require app-specific
 * credentials this build doesn't have (see docs/android.md) — but a plain
 * `https://` search URL needs no API key or auth of any kind, and Android
 * resolves it to the YouTube Music app automatically if it's installed
 * (verified app links) or a browser otherwise, so it always does
 * *something* useful rather than being disabled.
 */
class WebSearchMusicProvider : MusicProvider {
    override fun openInExternalApp(song: Song): Intent {
        val query = listOfNotNull(song.title, song.artist).joinToString(" ")
        val uri = Uri.parse("https://music.youtube.com/search").buildUpon()
            .appendQueryParameter("q", query)
            .build()
        return Intent(Intent.ACTION_VIEW, uri)
    }
}
