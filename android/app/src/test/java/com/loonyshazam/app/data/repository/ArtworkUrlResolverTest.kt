package com.loonyshazam.app.data.repository

import com.google.common.truth.Truth.assertThat
import com.loonyshazam.app.BuildConfig
import org.junit.Test

/**
 * [resolveArtworkUrl] resolves the backend's relative `/artwork/...` paths
 * against [BuildConfig.BASE_URL] (unit tests run against the debug
 * variant, so BASE_URL here is the real "http://localhost:8080/" debug
 * value — see app/build.gradle.kts).
 */
class ArtworkUrlResolverTest {

    @Test
    fun `null passes through unchanged`() {
        assertThat(resolveArtworkUrl(null)).isNull()
    }

    @Test
    fun `relative path is resolved against BASE_URL`() {
        val resolved = resolveArtworkUrl("/artwork/abc123.png")
        assertThat(resolved).isEqualTo("${BuildConfig.BASE_URL.trimEnd('/')}/artwork/abc123.png")
    }

    @Test
    fun `relative path without leading slash is also resolved`() {
        val resolved = resolveArtworkUrl("artwork/abc123.png")
        assertThat(resolved).isEqualTo("${BuildConfig.BASE_URL.trimEnd('/')}/artwork/abc123.png")
    }

    @Test
    fun `already-absolute http url passes through unchanged`() {
        val absolute = "https://cdn.example.com/covers/abc123.jpg"
        assertThat(resolveArtworkUrl(absolute)).isEqualTo(absolute)
    }
}
