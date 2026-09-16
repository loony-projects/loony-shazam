package com.loonyshazam.app.ui.history

import android.content.Intent
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.MusicNote
import androidx.compose.material.icons.filled.Share
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
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import coil.compose.AsyncImage
import com.loonyshazam.app.data.MusicProvider
import com.loonyshazam.app.data.db.HistoryEntity
import com.loonyshazam.app.data.model.Song
import com.loonyshazam.app.viewmodel.HistoryViewModel
import java.time.Instant
import java.time.ZoneId
import java.time.format.DateTimeFormatter

/** Read-only detail view for a past recognition, reached by tapping a History row. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun HistoryDetailScreen(
    viewModel: HistoryViewModel,
    historyId: Long,
    musicProvider: MusicProvider,
    onBack: () -> Unit
) {
    LaunchedEffect(historyId) { viewModel.loadDetail(historyId) }
    val entry by viewModel.selected.collectAsState()
    val context = LocalContext.current

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Details") },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back")
                    }
                }
            )
        }
    ) { padding ->
        val current = entry
        if (current == null) {
            Column(
                modifier = Modifier.padding(padding).fillMaxSize(),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.Center
            ) {
                Text("Loading...")
            }
            return@Scaffold
        }

        Column(
            modifier = Modifier.padding(padding).fillMaxSize().padding(24.dp),
            horizontalAlignment = Alignment.CenterHorizontally
        ) {
            DetailArtwork(current.artworkUrl)
            Spacer(modifier = Modifier.height(24.dp))
            Text(current.title, style = MaterialTheme.typography.headlineMedium)
            Spacer(modifier = Modifier.height(4.dp))
            Text(
                current.artist,
                style = MaterialTheme.typography.titleLarge,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
            current.album?.let {
                Spacer(modifier = Modifier.height(4.dp))
                Text(it, style = MaterialTheme.typography.bodyMedium)
            }
            Spacer(modifier = Modifier.height(8.dp))
            Text(
                "Recognized ${formatFullTimestamp(current.recognizedAt)}",
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
            Spacer(modifier = Modifier.height(24.dp))

            Row {
                OutlinedButton(onClick = { shareHistoryEntry(context, current) }) {
                    Icon(Icons.Filled.Share, contentDescription = null, modifier = Modifier.size(18.dp))
                    Spacer(modifier = Modifier.width(6.dp))
                    Text("Share")
                }
            }
            Spacer(modifier = Modifier.height(12.dp))
            val song = Song(
                id = current.songId,
                title = current.title,
                artist = current.artist,
                album = current.album,
                durationMs = 0,
                artworkUrl = current.artworkUrl
            )
            val externalIntent = musicProvider.openInExternalApp(song)
            OutlinedButton(
                onClick = { externalIntent?.let { context.startActivity(it) } },
                enabled = externalIntent != null
            ) {
                Text(if (externalIntent != null) "Open in music app" else "Open in music app (not configured)")
            }
        }
    }
}

@Composable
private fun DetailArtwork(artworkUrl: String?) {
    val shape = RoundedCornerShape(16.dp)
    if (artworkUrl != null) {
        AsyncImage(
            model = artworkUrl,
            contentDescription = "Album artwork",
            contentScale = ContentScale.Crop,
            modifier = Modifier.size(220.dp).clip(shape)
        )
    } else {
        Card(modifier = Modifier.size(220.dp), shape = shape) {
            Column(
                modifier = Modifier.fillMaxSize(),
                horizontalAlignment = Alignment.CenterHorizontally,
                verticalArrangement = Arrangement.Center
            ) {
                Icon(
                    Icons.Filled.MusicNote,
                    contentDescription = "No artwork available",
                    modifier = Modifier.size(64.dp),
                    tint = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }
        }
    }
}

private fun shareHistoryEntry(context: android.content.Context, entry: HistoryEntity) {
    val summary = "I identified \"${entry.title}\" by ${entry.artist} with Loony Shazam!"
    val intent = Intent(Intent.ACTION_SEND).apply {
        type = "text/plain"
        putExtra(Intent.EXTRA_TEXT, summary)
    }
    context.startActivity(Intent.createChooser(intent, "Share result"))
}

private fun formatFullTimestamp(epochMillis: Long): String {
    val formatter = DateTimeFormatter.ofPattern("MMM d, yyyy 'at' h:mm a")
    return Instant.ofEpochMilli(epochMillis).atZone(ZoneId.systemDefault()).format(formatter)
}
