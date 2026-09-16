package com.loonyshazam.app.ui.home

import android.Manifest
import android.content.pm.PackageManager
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.History
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalLifecycleOwner
import androidx.compose.ui.unit.dp
import androidx.core.content.ContextCompat
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import com.loonyshazam.app.viewmodel.RecognitionUiState
import com.loonyshazam.app.viewmodel.RecognitionViewModel

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun HomeScreen(
    viewModel: RecognitionViewModel,
    onNavigateToHistory: () -> Unit,
    onRecognitionSettled: () -> Unit
) {
    val uiState by viewModel.uiState.collectAsState()
    val context = LocalContext.current

    val permissionLauncher = rememberLauncherForActivityResult(
        ActivityResultContracts.RequestPermission()
    ) { granted ->
        if (granted) viewModel.startListening() else viewModel.onPermissionDenied()
    }

    fun requestListen() {
        val granted = ContextCompat.checkSelfPermission(
            context,
            Manifest.permission.RECORD_AUDIO
        ) == PackageManager.PERMISSION_GRANTED
        if (granted) {
            viewModel.startListening()
        } else {
            permissionLauncher.launch(Manifest.permission.RECORD_AUDIO)
        }
    }

    // Stop an in-flight recording if the app is backgrounded, so we never
    // leak a live AudioRecord or upload audio the user didn't mean to send.
    val lifecycleOwner = LocalLifecycleOwner.current
    DisposableEffect(lifecycleOwner) {
        val observer = LifecycleEventObserver { _, event ->
            if (event == Lifecycle.Event.ON_STOP) {
                viewModel.cancelListening()
            }
        }
        lifecycleOwner.lifecycle.addObserver(observer)
        onDispose { lifecycleOwner.lifecycle.removeObserver(observer) }
    }

    LaunchedEffect(uiState) {
        if (uiState is RecognitionUiState.Result || uiState is RecognitionUiState.NotFound) {
            onRecognitionSettled()
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Loony Shazam") },
                actions = {
                    IconButton(onClick = onNavigateToHistory) {
                        Icon(Icons.Filled.History, contentDescription = "History")
                    }
                }
            )
        }
    ) { padding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(padding)
                .padding(24.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.Center
        ) {
            StatusText(uiState)
            Spacer(modifier = Modifier.height(32.dp))
            Box(contentAlignment = Alignment.Center) {
                MicButton(
                    state = uiState,
                    enabled = uiState is RecognitionUiState.Idle ||
                        uiState is RecognitionUiState.Error ||
                        uiState is RecognitionUiState.PermissionDenied,
                    onClick = { requestListen() }
                )
            }
            if (uiState is RecognitionUiState.PermissionDenied) {
                Spacer(modifier = Modifier.height(24.dp))
                TextButton(onClick = { requestListen() }) {
                    Text("Grant microphone permission")
                }
            }
        }
    }
}

@Composable
private fun StatusText(state: RecognitionUiState) {
    val text = when (state) {
        is RecognitionUiState.Idle -> "Tap to identify a song"
        is RecognitionUiState.Listening -> "Listening... ${state.elapsedSeconds}s"
        is RecognitionUiState.Processing -> "Analyzing audio..."
        is RecognitionUiState.Result -> "Got it!"
        is RecognitionUiState.NotFound -> "No match found"
        is RecognitionUiState.Error -> state.message
        is RecognitionUiState.PermissionDenied ->
            "Microphone permission is required to identify songs."
    }
    Text(
        text = text,
        style = MaterialTheme.typography.titleLarge,
        color = if (state is RecognitionUiState.Error || state is RecognitionUiState.PermissionDenied) {
            MaterialTheme.colorScheme.error
        } else {
            MaterialTheme.colorScheme.onBackground
        }
    )
}
