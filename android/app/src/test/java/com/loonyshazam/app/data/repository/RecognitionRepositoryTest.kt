package com.loonyshazam.app.data.repository

import com.google.common.truth.Truth.assertThat
import com.loonyshazam.app.data.model.RecognitionOutcome
import com.loonyshazam.app.data.network.RecognitionApi
import kotlinx.coroutines.test.runTest
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import okhttp3.mockwebserver.SocketPolicy
import org.junit.After
import org.junit.Before
import org.junit.Test
import retrofit2.Retrofit
import retrofit2.converter.gson.GsonConverterFactory

/**
 * Verifies the multipart request shape and JSON parsing against the exact
 * backend contract documented in docs/architecture.md / docs/android.md,
 * using a real HTTP server (MockWebServer) rather than mocking Retrofit.
 */
class RecognitionRepositoryTest {

    private lateinit var server: MockWebServer
    private lateinit var api: RecognitionApi
    private lateinit var dao: FakeHistoryDao
    private lateinit var repository: DefaultRecognitionRepository

    @Before
    fun setUp() {
        server = MockWebServer()
        server.start()
        api = Retrofit.Builder()
            .baseUrl(server.url("/"))
            .addConverterFactory(GsonConverterFactory.create())
            .build()
            .create(RecognitionApi::class.java)
        dao = FakeHistoryDao()
        repository = DefaultRecognitionRepository(api, dao)
    }

    @After
    fun tearDown() {
        server.shutdown()
    }

    @Test
    fun `recognized response is parsed into RecognitionOutcome Recognized`() = runTest {
        server.enqueue(
            MockResponse().setResponseCode(200).setBody(
                """
                {
                  "recognized": true,
                  "song": {
                    "id": "3",
                    "title": "Song Alpha",
                    "artist": "Test Artist",
                    "album": null,
                    "duration_ms": 20000,
                    "artwork_url": null
                  },
                  "match": {
                    "score": 119.46,
                    "confidence": 0.49,
                    "matched_fingerprints": 145,
                    "query_fingerprints": 294,
                    "offset_ms": 9000,
                    "algorithm_version": 1,
                    "latency_ms": 666
                  }
                }
                """.trimIndent()
            )
        )

        val result = repository.recognize(fakeWavBytes())

        assertThat(result.isSuccess).isTrue()
        val outcome = result.getOrThrow()
        check(outcome is RecognitionOutcome.Recognized)
        assertThat(outcome.song.id).isEqualTo("3")
        assertThat(outcome.song.title).isEqualTo("Song Alpha")
        assertThat(outcome.song.artist).isEqualTo("Test Artist")
        assertThat(outcome.song.album).isNull()
        assertThat(outcome.song.artworkUrl).isNull()
        assertThat(outcome.match.matchedFingerprints).isEqualTo(145)
        assertThat(outcome.match.offsetMs).isEqualTo(9000L)
    }

    @Test
    fun `not-recognized response is parsed into RecognitionOutcome NotRecognized`() = runTest {
        server.enqueue(
            MockResponse().setResponseCode(200).setBody(
                """
                {
                  "recognized": false,
                  "reason": "NO_MATCH",
                  "match": {
                    "score": 0.0,
                    "confidence": 0.0,
                    "matched_fingerprints": 0,
                    "query_fingerprints": 0,
                    "offset_ms": null,
                    "algorithm_version": 1,
                    "latency_ms": 649
                  }
                }
                """.trimIndent()
            )
        )

        val result = repository.recognize(fakeWavBytes())

        assertThat(result.isSuccess).isTrue()
        val outcome = result.getOrThrow()
        check(outcome is RecognitionOutcome.NotRecognized)
        assertThat(outcome.reason).isEqualTo("NO_MATCH")
        assertThat(outcome.match.offsetMs).isNull()
    }

    @Test
    fun `4xx error response surfaces the server message and is not retried`() = runTest {
        server.enqueue(
            MockResponse().setResponseCode(400).setBody(
                """{"error": "INVALID_AUDIO", "message": "audio too short"}"""
            )
        )

        val result = repository.recognize(fakeWavBytes())

        assertThat(result.isFailure).isTrue()
        assertThat(result.exceptionOrNull()?.message).isEqualTo("audio too short")
        assertThat(server.requestCount).isEqualTo(1)
    }

    @Test
    fun `network failure is retried exactly once before failing`() = runTest {
        server.enqueue(MockResponse().setSocketPolicy(SocketPolicy.DISCONNECT_AT_START))
        server.enqueue(MockResponse().setSocketPolicy(SocketPolicy.DISCONNECT_AT_START))

        val result = repository.recognize(fakeWavBytes())

        assertThat(result.isFailure).isTrue()
        assertThat(server.requestCount).isEqualTo(2)
    }

    @Test
    fun `multipart request uses field name audio with audio wav content type`() = runTest {
        server.enqueue(
            MockResponse().setResponseCode(200).setBody(
                """{"recognized": false, "reason": "NO_MATCH", "match": {"score":0.0,"confidence":0.0,"matched_fingerprints":0,"query_fingerprints":0,"offset_ms":null,"algorithm_version":1,"latency_ms":1}}"""
            )
        )

        repository.recognize(fakeWavBytes())

        val recorded = server.takeRequest()
        assertThat(recorded.path).isEqualTo("/api/v1/recognitions/audio")
        assertThat(recorded.method).isEqualTo("POST")
        val body = recorded.body.readUtf8()
        assertThat(body).contains("name=\"audio\"")
        assertThat(body).contains("audio/wav")
    }

    private fun fakeWavBytes(): ByteArray = com.loonyshazam.app.audio.WavEncoder.encode(
        shortArrayOf(1, 2, 3, 4, 5),
        sampleRate = 44_100
    )
}
