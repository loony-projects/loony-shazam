package com.loonyshazam.app

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.Surface
import androidx.compose.ui.Modifier
import androidx.lifecycle.viewmodel.compose.viewModel
import com.loonyshazam.app.ui.navigation.LoonyShazamNavHost
import com.loonyshazam.app.ui.theme.LoonyShazamTheme
import com.loonyshazam.app.viewmodel.HistoryViewModel
import com.loonyshazam.app.viewmodel.RecognitionViewModel

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()

        val container = (application as LoonyShazamApp).container

        setContent {
            LoonyShazamTheme {
                Surface(modifier = Modifier.fillMaxSize()) {
                    val recognitionViewModel: RecognitionViewModel = viewModel(
                        factory = RecognitionViewModel.Factory(container.audioRecorder, container.repository)
                    )
                    val historyViewModel: HistoryViewModel = viewModel(
                        factory = HistoryViewModel.Factory(container.repository)
                    )
                    LoonyShazamNavHost(
                        recognitionViewModel = recognitionViewModel,
                        historyViewModel = historyViewModel,
                        musicProvider = container.musicProvider
                    )
                }
            }
        }
    }
}
