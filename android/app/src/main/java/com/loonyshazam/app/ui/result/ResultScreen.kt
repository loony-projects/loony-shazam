package com.loonyshazam.app.ui.result

import android.content.Intent
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.MusicNote
import androidx.compose.material.icons.filled.Refresh
import androidx.compose.material.icons.filled.Share
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import coil.compose.AsyncImage
import com.loonyshazam.app.data.MusicProvider
import com.loonyshazam.app.data.model.MatchInfo
import com.loonyshazam.app.data.model.Song
import com.loonyshazam.app.viewmodel.RecognitionUiState
import java.time.Instant
import java.time.ZoneId
import java.time.format.DateTimeFormatter

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun ResultScreen(
    state: RecognitionUiState,
    musicProvider: MusicProvider,
    onSearchAgain: () -> Unit,
    onBack: () -> Unit
) {
    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Result") },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back")
                    }
                }
            )
        }
    ) { padding ->
        Column(modifier = Modifier.padding(padding)) {
            when (state) {
                is RecognitionUiState.Result -> RecognizedContent(
                    song = state.song,
                    match = state.match,
                    recognizedAt = state.recognizedAt,
                    musicProvider = musicProvider,
                    onSearchAgain = onSearchAgain
                )
                is RecognitionUiState.NotFound -> NotFoundContent(onSearchAgain = onSearchAgain)
                else -> UnavailableContent(onSearchAgain = onSearchAgain)
            }
        }
    }
}

@Composable
private fun RecognizedContent(
    song: Song,
    match: MatchInfo,
    recognizedAt: Long,
    musicProvider: MusicProvider,
    onSearchAgain: () -> Unit
) {
    val context = LocalContext.current
    var showDetails by remember { mutableStateOf(false) }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .verticalScroll(rememberScrollState())
            .padding(24.dp),
        horizontalAlignment = Alignment.CenterHorizontally
    ) {
        Artwork(artworkUrl = song.artworkUrl)
        Spacer(modifier = Modifier.height(24.dp))
        Text(song.title, style = MaterialTheme.typography.headlineMedium, textAlign = androidx.compose.ui.text.style.TextAlign.Center)
        Spacer(modifier = Modifier.height(4.dp))
        Text(song.artist, style = MaterialTheme.typography.titleLarge, color = MaterialTheme.colorScheme.onSurfaceVariant)
        song.album?.let {
            Spacer(modifier = Modifier.height(4.dp))
            Text(it, style = MaterialTheme.typography.bodyMedium)
        }
        Spacer(modifier = Modifier.height(8.dp))
        Text(
            "Recognized ${formatTimestamp(recognizedAt)}",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant
        )
        Spacer(modifier = Modifier.height(24.dp))

        Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            Button(onClick = onSearchAgain) {
                Icon(Icons.Filled.Refresh, contentDescription = null, modifier = Modifier.size(18.dp))
                Spacer(modifier = Modifier.width(6.dp))
                Text("Search again")
            }
            OutlinedButton(onClick = { shareResult(context, song) }) {
                Icon(Icons.Filled.Share, contentDescription = null, modifier = Modifier.size(18.dp))
                Spacer(modifier = Modifier.width(6.dp))
                Text("Share")
            }
        }
        Spacer(modifier = Modifier.height(12.dp))

        val externalIntent = remember(song) { musicProvider.openInExternalApp(song) }
        OutlinedButton(
            onClick = { externalIntent?.let { context.startActivity(it) } },
            enabled = externalIntent != null
        ) {
            Text(if (externalIntent != null) "Open in music app" else "Open in music app (not configured)")
        }

        Spacer(modifier = Modifier.height(12.dp))
        OutlinedButton(onClick = { showDetails = !showDetails }) {
            Text(if (showDetails) "Hide details" else "View details")
        }
        if (showDetails) {
            Spacer(modifier = Modifier.height(12.dp))
            MatchDetailsCard(match)
        }
    }
}

@Composable
private fun MatchDetailsCard(match: MatchInfo) {
    Card(modifier = Modifier.fillMaxWidth()) {
        Column(modifier = Modifier.padding(16.dp)) {
            DetailRow("Confidence", "${(match.confidence * 100).toInt()}%")
            DetailRow("Score", "%.2f".format(match.score))
            DetailRow("Matched fingerprints", "${match.matchedFingerprints} / ${match.queryFingerprints}")
            match.offsetMs?.let { DetailRow("Offset in track", "${it}ms") }
            DetailRow("Algorithm version", match.algorithmVersion.toString())
            DetailRow("Server latency", "${match.latencyMs}ms")
        }
    }
}

@Composable
private fun DetailRow(label: String, value: String) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(vertical = 4.dp),
        horizontalArrangement = Arrangement.SpaceBetween
    ) {
        Text(label, color = MaterialTheme.colorScheme.onSurfaceVariant)
        Text(value, style = MaterialTheme.typography.bodyLarge)
    }
}

@Composable
private fun NotFoundContent(onSearchAgain: () -> Unit) {
    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(24.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center
    ) {
        Icon(
            Icons.Filled.MusicNote,
            contentDescription = null,
            modifier = Modifier.size(72.dp),
            tint = MaterialTheme.colorScheme.onSurfaceVariant
        )
        Spacer(modifier = Modifier.height(16.dp))
        Text("We couldn't recognize that song", style = MaterialTheme.typography.titleLarge)
        Spacer(modifier = Modifier.height(8.dp))
        Text(
            "Try moving closer to the audio source and search again.",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant
        )
        Spacer(modifier = Modifier.height(24.dp))
        Button(onClick = onSearchAgain) { Text("Search again") }
    }
}

@Composable
private fun UnavailableContent(onSearchAgain: () -> Unit) {
    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(24.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.Center
    ) {
        Text("No result to show", style = MaterialTheme.typography.titleLarge)
        Spacer(modifier = Modifier.height(16.dp))
        Button(onClick = onSearchAgain) { Text("Back to listening") }
    }
}

@Composable
private fun Artwork(artworkUrl: String?, size: androidx.compose.ui.unit.Dp = 220.dp) {
    val shape = RoundedCornerShape(16.dp)
    if (artworkUrl != null) {
        AsyncImage(
            model = artworkUrl,
            contentDescription = "Album artwork",
            contentScale = ContentScale.Crop,
            modifier = Modifier
                .size(size)
                .clip(shape)
        )
    } else {
        PlaceholderArtwork(icon = Icons.Filled.MusicNote, size = size)
    }
}

@Composable
private fun PlaceholderArtwork(icon: ImageVector, size: androidx.compose.ui.unit.Dp) {
    Card(
        modifier = Modifier.size(size),
        shape = RoundedCornerShape(16.dp)
    ) {
        Column(
            modifier = Modifier.fillMaxSize(),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.Center
        ) {
            Icon(
                icon,
                contentDescription = "No artwork available",
                modifier = Modifier.size(64.dp),
                tint = MaterialTheme.colorScheme.onSurfaceVariant
            )
        }
    }
}

private fun shareResult(context: android.content.Context, song: Song) {
    val summary = "I just identified \"${song.title}\" by ${song.artist} with Loony Shazam!"
    val intent = Intent(Intent.ACTION_SEND).apply {
        type = "text/plain"
        putExtra(Intent.EXTRA_TEXT, summary)
    }
    context.startActivity(Intent.createChooser(intent, "Share result"))
}

private fun formatTimestamp(epochMillis: Long): String {
    val formatter = DateTimeFormatter.ofPattern("MMM d, yyyy 'at' h:mm a")
    return Instant.ofEpochMilli(epochMillis).atZone(ZoneId.systemDefault()).format(formatter)
}
