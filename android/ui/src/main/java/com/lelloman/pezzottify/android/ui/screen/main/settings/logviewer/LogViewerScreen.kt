package com.lelloman.pezzottify.android.ui.screen.main.settings.logviewer

import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Clear
import androidx.compose.material.icons.filled.KeyboardArrowDown
import androidx.compose.material.icons.filled.Search
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.navigation.NavController
import androidx.navigation.compose.rememberNavController
import com.lelloman.pezzottify.android.ui.theme.PezzottifyTheme
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch

private val InfoColor = Color(0xFF4CAF50)  // Green
private val WarnColor = Color(0xFFFF9800)  // Orange
private val ErrorColor = Color(0xFFF44336) // Red

private data class ParsedLogLine(
    val raw: String,
    val level: LogLevel?,
)

private fun parseLogLine(line: String): ParsedLogLine {
    val level = when {
        line.contains("[INFO]") -> LogLevel.INFO
        line.contains("[WARN]") -> LogLevel.WARN
        line.contains("[ERROR]") -> LogLevel.ERROR
        else -> null
    }
    return ParsedLogLine(raw = line, level = level)
}

private fun LogLevel.toColor(): Color = when (this) {
    LogLevel.INFO -> InfoColor
    LogLevel.WARN -> WarnColor
    LogLevel.ERROR -> ErrorColor
}

@Composable
fun LogViewerScreen(navController: NavController) {
    val viewModel = hiltViewModel<LogViewerScreenViewModel>()
    LogViewerScreenInternal(
        state = viewModel.state,
        navController = navController,
        onSearchQueryChanged = viewModel::onSearchQueryChanged,
        onRefresh = viewModel::onRefresh,
        onToggleLevel = viewModel::toggleLevel,
    )
}

@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
private fun LogViewerScreenInternal(
    state: StateFlow<LogViewerScreenState>,
    navController: NavController,
    onSearchQueryChanged: (String) -> Unit,
    onRefresh: () -> Unit,
    onToggleLevel: (LogLevel) -> Unit,
) {
    val currentState by state.collectAsState()
    val listState = rememberLazyListState()
    val scope = rememberCoroutineScope()

    val filteredLines = remember(currentState.logContent, currentState.searchQuery, currentState.enabledLevels) {
        currentState.logContent
            .lines()
            .map { parseLogLine(it) }
            .filter { parsed ->
                // Filter by level (null level = continuation lines, always show if parent level matches)
                val levelMatch = parsed.level == null || parsed.level in currentState.enabledLevels
                // Filter by search query
                val searchMatch = currentState.searchQuery.isBlank() ||
                        parsed.raw.contains(currentState.searchQuery, ignoreCase = true)
                levelMatch && searchMatch
            }
    }

    // Auto-scroll to bottom when content loads
    LaunchedEffect(currentState.isLoading) {
        if (!currentState.isLoading && filteredLines.isNotEmpty()) {
            listState.scrollToItem(filteredLines.lastIndex)
        }
    }

    // Show FAB when not at bottom
    val showScrollToBottom by remember {
        derivedStateOf {
            val lastVisibleItem = listState.layoutInfo.visibleItemsInfo.lastOrNull()?.index ?: 0
            filteredLines.isNotEmpty() && lastVisibleItem < filteredLines.lastIndex - 5
        }
    }

    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text("Logs") },
                navigationIcon = {
                    IconButton(onClick = { navController.popBackStack() }) {
                        Icon(
                            imageVector = Icons.AutoMirrored.Filled.ArrowBack,
                            contentDescription = "Back"
                        )
                    }
                }
            )
        },
        floatingActionButton = {
            if (showScrollToBottom) {
                FloatingActionButton(
                    onClick = {
                        scope.launch {
                            if (filteredLines.isNotEmpty()) {
                                listState.animateScrollToItem(filteredLines.lastIndex)
                            }
                        }
                    }
                ) {
                    Icon(
                        imageVector = Icons.Default.KeyboardArrowDown,
                        contentDescription = "Scroll to bottom"
                    )
                }
            }
        }
    ) { innerPadding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(innerPadding)
        ) {
            // Search bar
            OutlinedTextField(
                value = currentState.searchQuery,
                onValueChange = onSearchQueryChanged,
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 16.dp, vertical = 8.dp),
                placeholder = { Text("Filter logs...") },
                leadingIcon = {
                    Icon(
                        imageVector = Icons.Default.Search,
                        contentDescription = null
                    )
                },
                trailingIcon = {
                    if (currentState.searchQuery.isNotEmpty()) {
                        IconButton(onClick = { onSearchQueryChanged("") }) {
                            Icon(
                                imageVector = Icons.Default.Clear,
                                contentDescription = "Clear search"
                            )
                        }
                    }
                },
                singleLine = true,
            )

            // Level filter chips
            FlowRow(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 16.dp, vertical = 4.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                LogLevel.entries.forEach { level ->
                    val isSelected = level in currentState.enabledLevels
                    FilterChip(
                        selected = isSelected,
                        onClick = { onToggleLevel(level) },
                        label = { Text(level.label) },
                        colors = FilterChipDefaults.filterChipColors(
                            selectedContainerColor = level.toColor().copy(alpha = 0.2f),
                            selectedLabelColor = level.toColor(),
                        )
                    )
                }
            }

            if (currentState.isLoading) {
                Box(
                    modifier = Modifier.fillMaxSize(),
                    contentAlignment = Alignment.Center
                ) {
                    CircularProgressIndicator()
                }
            } else {
                PullToRefreshBox(
                    isRefreshing = currentState.isRefreshing,
                    onRefresh = onRefresh,
                    modifier = Modifier.fillMaxSize()
                ) {
                    if (filteredLines.isEmpty()) {
                        Box(
                            modifier = Modifier.fillMaxSize(),
                            contentAlignment = Alignment.Center
                        ) {
                            Text(
                                text = "No logs",
                                color = MaterialTheme.colorScheme.onSurfaceVariant
                            )
                        }
                    } else {
                        val horizontalScrollState = rememberScrollState()
                        // Estimate width based on longest line (monospace ~6.5dp per char at 11sp)
                        val maxLineLength = remember(filteredLines) {
                            filteredLines.maxOfOrNull { it.raw.length } ?: 0
                        }
                        val estimatedWidth = (maxLineLength * 6.5).dp + 16.dp

                        SelectionContainer {
                            Box(
                                modifier = Modifier
                                    .fillMaxSize()
                                    .horizontalScroll(horizontalScrollState)
                            ) {
                                LazyColumn(
                                    state = listState,
                                    modifier = Modifier
                                        .widthIn(min = estimatedWidth)
                                        .padding(horizontal = 8.dp)
                                ) {
                                    itemsIndexed(
                                        items = filteredLines,
                                        key = { index, _ -> index }
                                    ) { _, parsed ->
                                        Text(
                                            text = parsed.raw,
                                            modifier = Modifier.fillMaxWidth(),
                                            fontFamily = FontFamily.Monospace,
                                            fontSize = 11.sp,
                                            lineHeight = 14.sp,
                                            color = parsed.level?.toColor()
                                                ?: MaterialTheme.colorScheme.onSurface,
                                            softWrap = false,
                                        )
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

@Preview(showBackground = true)
@Composable
private fun LogViewerScreenPreview() {
    PezzottifyTheme {
        LogViewerScreenInternal(
            state = MutableStateFlow(
                LogViewerScreenState(
                    logContent = """
                        [2024-01-15 10:30:45.123] [INFO] [App] Application started
                        [2024-01-15 10:30:45.456] [INFO] [Network] Connecting to server
                        [2024-01-15 10:30:46.789] [INFO] [Auth] User logged in successfully
                        [2024-01-15 10:30:47.012] [WARN] [Cache] Cache miss for album_123
                        [2024-01-15 10:30:48.345] [ERROR] [Player] Failed to load track
                        java.io.IOException: Connection refused
                            at com.example.Player.load(Player.kt:42)
                    """.trimIndent(),
                    isLoading = false,
                )
            ),
            navController = rememberNavController(),
            onSearchQueryChanged = {},
            onRefresh = {},
            onToggleLevel = {},
        )
    }
}

@Preview(showBackground = true)
@Composable
private fun LogViewerScreenPreviewLoading() {
    PezzottifyTheme {
        LogViewerScreenInternal(
            state = MutableStateFlow(LogViewerScreenState(isLoading = true)),
            navController = rememberNavController(),
            onSearchQueryChanged = {},
            onRefresh = {},
            onToggleLevel = {},
        )
    }
}

@Preview(showBackground = true)
@Composable
private fun LogViewerScreenPreviewDark() {
    PezzottifyTheme(darkTheme = true) {
        LogViewerScreenInternal(
            state = MutableStateFlow(
                LogViewerScreenState(
                    logContent = """
                        [2024-01-15 10:30:45.123] [INFO] [App] Application started
                        [2024-01-15 10:30:45.456] [WARN] [Network] Slow connection detected
                        [2024-01-15 10:30:46.789] [ERROR] [Auth] Authentication failed
                    """.trimIndent(),
                    isLoading = false,
                )
            ),
            navController = rememberNavController(),
            onSearchQueryChanged = {},
            onRefresh = {},
            onToggleLevel = {},
        )
    }
}
