package com.loonyshazam.app.data

import android.content.Intent
import com.google.common.truth.Truth.assertThat
import com.loonyshazam.app.data.model.Song
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * Uses Robolectric (already a test dependency, see app/build.gradle.kts)
 * rather than a plain JVM unit test because this exercises real
 * `android.net.Uri`/`Intent` behavior (the default JVM Android stubs
 * return null from `Uri.parse`, which isn't useful here).
 *
 * Pinned to `sdk = 34`: Robolectric 4.13's newest supported simulated
 * platform is API 34, one behind this project's `targetSdk = 35` — this
 * only affects which Android version Robolectric emulates *inside the
 * test JVM*, not the app's real target/compile SDK.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class WebSearchMusicProviderTest {

    private val provider = WebSearchMusicProvider()

    @Test
    fun `builds a YouTube Music search intent with title and artist`() {
        val song = Song(
            id = "1",
            title = "Arohi",
            artist = "Shankuraj Konwar",
            album = "Arohi",
            durationMs = 312610,
            artworkUrl = null
        )

        val intent = provider.openInExternalApp(song)

        assertThat(intent.action).isEqualTo(Intent.ACTION_VIEW)
        val uri = intent.data!!
        assertThat(uri.scheme).isEqualTo("https")
        assertThat(uri.host).isEqualTo("music.youtube.com")
        assertThat(uri.getQueryParameter("q")).isEqualTo("Arohi Shankuraj Konwar")
    }

    @Test
    fun `always returns a usable intent, never null`() {
        val song = Song(
            id = "2",
            title = "Unknown Title",
            artist = "Unknown Artist",
            album = null,
            durationMs = 0,
            artworkUrl = null
        )

        assertThat(provider.openInExternalApp(song)).isNotNull()
    }
}
