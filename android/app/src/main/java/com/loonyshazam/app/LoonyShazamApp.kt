package com.loonyshazam.app

import android.app.Application
import androidx.room.Room
import com.google.gson.Gson
import com.loonyshazam.app.audio.AudioRecorder
import com.loonyshazam.app.data.MusicProvider
import com.loonyshazam.app.data.NoOpMusicProvider
import com.loonyshazam.app.data.db.HistoryDatabase
import com.loonyshazam.app.data.network.RecognitionApi
import com.loonyshazam.app.data.repository.DefaultRecognitionRepository
import com.loonyshazam.app.data.repository.RecognitionRepository
import okhttp3.OkHttpClient
import okhttp3.logging.HttpLoggingInterceptor
import retrofit2.Retrofit
import retrofit2.converter.gson.GsonConverterFactory
import java.util.concurrent.TimeUnit

/**
 * Hand-rolled dependency container (no DI framework per the project's tech
 * stack). Everything is built lazily and lives for the process lifetime,
 * which is appropriate for a single-Activity app with no per-screen scoping
 * needs.
 */
class AppContainer(app: Application) {

    private val appContext = app.applicationContext

    val database: HistoryDatabase by lazy {
        Room.databaseBuilder(appContext, HistoryDatabase::class.java, "loony_shazam_history.db")
            .build()
    }

    private val okHttpClient: OkHttpClient by lazy {
        val logging = HttpLoggingInterceptor().apply {
            level = if (BuildConfig.DEBUG) {
                HttpLoggingInterceptor.Level.BASIC
            } else {
                HttpLoggingInterceptor.Level.NONE
            }
        }
        OkHttpClient.Builder()
            .connectTimeout(10, TimeUnit.SECONDS)
            .readTimeout(20, TimeUnit.SECONDS)
            .writeTimeout(20, TimeUnit.SECONDS)
            .addInterceptor(logging)
            .build()
    }

    private val gson: Gson by lazy { Gson() }

    private val retrofit: Retrofit by lazy {
        Retrofit.Builder()
            .baseUrl(BuildConfig.BASE_URL)
            .client(okHttpClient)
            .addConverterFactory(GsonConverterFactory.create(gson))
            .build()
    }

    private val api: RecognitionApi by lazy { retrofit.create(RecognitionApi::class.java) }

    val repository: RecognitionRepository by lazy {
        DefaultRecognitionRepository(api, database.historyDao(), gson)
    }

    val audioRecorder: AudioRecorder by lazy { AudioRecorder(appContext) }

    val musicProvider: MusicProvider by lazy { NoOpMusicProvider() }
}

class LoonyShazamApp : Application() {
    lateinit var container: AppContainer
        private set

    override fun onCreate() {
        super.onCreate()
        container = AppContainer(this)
    }
}
