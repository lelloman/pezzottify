package com.lelloman.simpleaiassistant.ui

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.unit.dp
import com.lelloman.simpleaiassistant.model.ChatMessage
import com.lelloman.simpleaiassistant.model.MessageRole

@Composable
fun ChatMessageItem(
    message: ChatMessage,
    debugMode: Boolean,
    modifier: Modifier = Modifier
) {
    val isUser = message.role == MessageRole.USER
    val isTool = message.role == MessageRole.TOOL

    Box(
        modifier = modifier.fillMaxWidth(),
        contentAlignment = if (isUser) Alignment.CenterEnd else Alignment.CenterStart
    ) {
        MessageBubble(
            isUser = isUser,
            isTool = isTool,
            modifier = Modifier.fillMaxWidth(0.85f)
        ) {
            Column {
                // Tool name indicator
                if (isTool && message.toolName != null) {
                    Text(
                        text = message.toolName,
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant
                    )
                }

                // Message content
                Text(
                    text = message.content,
                    style = MaterialTheme.typography.bodyMedium
                )

                // Tool calls indicator (for assistant messages)
                if (message.toolCalls != null && message.toolCalls.isNotEmpty()) {
                    if (debugMode) {
                        message.toolCalls.forEach { toolCall ->
                            Text(
                                text = "→ ${toolCall.name}(${toolCall.input})",
                                style = MaterialTheme.typography.labelSmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                                modifier = Modifier.padding(top = 4.dp)
                            )
                        }
                    } else {
                        Text(
                            text = "Used ${message.toolCalls.size} tool(s)",
                            style = MaterialTheme.typography.labelSmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.padding(top = 4.dp)
                        )
                    }
                }
            }
        }
    }
}

@Composable
fun MessageBubble(
    isUser: Boolean,
    modifier: Modifier = Modifier,
    isTool: Boolean = false,
    content: @Composable () -> Unit
) {
    val backgroundColor = when {
        isUser -> MaterialTheme.colorScheme.primaryContainer
        isTool -> MaterialTheme.colorScheme.surfaceVariant.copy(alpha = 0.5f)
        else -> MaterialTheme.colorScheme.surfaceVariant
    }

    val shape = RoundedCornerShape(
        topStart = 16.dp,
        topEnd = 16.dp,
        bottomStart = if (isUser) 16.dp else 4.dp,
        bottomEnd = if (isUser) 4.dp else 16.dp
    )

    Surface(
        modifier = modifier
            .clip(shape),
        color = backgroundColor,
        shape = shape
    ) {
        Box(modifier = Modifier.padding(12.dp)) {
            content()
        }
    }
}
