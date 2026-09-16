package com.loonyshazam.app.ui.home

import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Mic
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.scale
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import com.loonyshazam.app.viewmodel.RecognitionUiState

/**
 * Large circular mic button. Its appearance mirrors the current recognition
 * state: idle (static mic), listening (pulsing ring), processing (spinner).
 */
@Composable
fun MicButton(
    state: RecognitionUiState,
    enabled: Boolean,
    onClick: () -> Unit,
    modifier: Modifier = Modifier
) {
    val isListening = state is RecognitionUiState.Listening
    val isProcessing = state is RecognitionUiState.Processing

    val infiniteTransition = rememberInfiniteTransition(label = "mic-pulse")
    val pulseScale by infiniteTransition.animateFloat(
        initialValue = 1f,
        targetValue = 1.15f,
        animationSpec = infiniteRepeatable(
            animation = tween(durationMillis = 700),
            repeatMode = RepeatMode.Reverse
        ),
        label = "mic-pulse-scale"
    )

    val buttonColor = when {
        isListening -> MaterialTheme.colorScheme.error
        else -> MaterialTheme.colorScheme.primary
    }

    Box(
        modifier = modifier
            .size(140.dp)
            .scale(if (isListening) pulseScale else 1f)
            .background(color = buttonColor, shape = CircleShape)
            .clickable(enabled = enabled && !isProcessing, onClick = onClick),
        contentAlignment = Alignment.Center
    ) {
        if (isProcessing) {
            CircularProgressIndicator(color = Color.White, strokeWidth = 4.dp)
        } else {
            Icon(
                imageVector = Icons.Filled.Mic,
                contentDescription = "Start listening",
                tint = Color.White,
                modifier = Modifier.size(56.dp)
            )
        }
    }
}
