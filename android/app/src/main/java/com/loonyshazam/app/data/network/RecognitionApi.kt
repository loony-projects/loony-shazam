package com.loonyshazam.app.data.network

import com.loonyshazam.app.data.network.dto.RecognitionResponseDto
import okhttp3.MultipartBody
import retrofit2.Response
import retrofit2.http.Multipart
import retrofit2.http.POST
import retrofit2.http.Part

/** Retrofit interface for the Rust backend's public recognition endpoint. */
interface RecognitionApi {

    /**
     * `POST /api/v1/recognitions/audio` - multipart upload, single part named
     * `audio` with content-type `audio/wav`. Returns HTTP 200 for both a
     * recognized match and a no-match ([RecognitionResponseDto.recognized]
     * distinguishes them); non-2xx means a real client/server error.
     */
    @Multipart
    @POST("api/v1/recognitions/audio")
    suspend fun recognizeAudio(
        @Part audio: MultipartBody.Part
    ): Response<RecognitionResponseDto>
}
