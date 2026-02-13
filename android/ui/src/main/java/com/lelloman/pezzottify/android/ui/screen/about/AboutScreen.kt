package com.lelloman.pezzottify.android.ui.screen.about

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.outlined.Album
import androidx.compose.material.icons.outlined.MusicNote
import androidx.compose.material.icons.outlined.Person
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.dp
import java.util.Locale
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.navigation.NavController
import com.lelloman.pezzottify.android.ui.R

@Composable
fun AboutScreen(navController: NavController) {
    val viewModel = hiltViewModel<AboutScreenViewModel>()
    val state by viewModel.state.collectAsState()
    AboutScreenInternal(
        state = state,
        onNavigateBack = { navController.popBackStack() },
    )
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun AboutScreenInternal(
    state: AboutScreenState,
    onNavigateBack: () -> Unit = {},
) {
    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.about)) },
                navigationIcon = {
                    IconButton(onClick = onNavigateBack) {
                        Icon(
                            imageVector = Icons.AutoMirrored.Filled.ArrowBack,
                            contentDescription = stringResource(R.string.back)
                        )
                    }
                }
            )
        }
    ) { innerPadding ->
        Column(
            modifier = Modifier
                .fillMaxSize()
                .padding(innerPadding)
                .padding(horizontal = 16.dp)
                .verticalScroll(rememberScrollState())
        ) {
            // App name
            Text(
                text = state.appName,
                style = MaterialTheme.typography.headlineMedium,
                color = MaterialTheme.colorScheme.onSurface
            )

            Spacer(modifier = Modifier.height(24.dp))

            // Version
            AboutLabel(text = stringResource(R.string.version_label))
            Text(
                text = state.versionName,
                style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.onSurface
            )

            Spacer(modifier = Modifier.height(16.dp))

            // Git commit
            AboutLabel(text = stringResource(R.string.git_commit))
            Text(
                text = state.gitCommit,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )

            Spacer(modifier = Modifier.height(16.dp))

            // Server URL
            AboutLabel(text = stringResource(R.string.server_url))
            Text(
                text = state.serverUrl,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )

            Spacer(modifier = Modifier.height(16.dp))

            // Server version
            AboutLabel(text = stringResource(R.string.server_version))
            Text(
                text = state.serverVersion,
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )

            Spacer(modifier = Modifier.height(24.dp))

            // Catalog section
            Text(
                text = stringResource(R.string.catalog_section_title),
                style = MaterialTheme.typography.titleMedium,
                color = MaterialTheme.colorScheme.onSurface
            )

            Spacer(modifier = Modifier.height(12.dp))

            when {
                state.catalogStatsLoading -> {
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.Center,
                    ) {
                        CircularProgressIndicator(modifier = Modifier.size(24.dp))
                    }
                }

                state.catalogStats != null -> {
                    val stats = state.catalogStats
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.spacedBy(12.dp)
                    ) {
                        CatalogStatCard(
                            icon = Icons.Outlined.Person,
                            available = stats.artists.available,
                            unavailable = stats.artists.unavailable,
                            label = stringResource(R.string.filter_artists),
                            modifier = Modifier.weight(1f)
                        )
                        CatalogStatCard(
                            icon = Icons.Outlined.Album,
                            available = stats.albums.available,
                            unavailable = stats.albums.unavailable,
                            label = stringResource(R.string.filter_albums),
                            modifier = Modifier.weight(1f)
                        )
                        CatalogStatCard(
                            icon = Icons.Outlined.MusicNote,
                            available = stats.tracks.available,
                            unavailable = stats.tracks.unavailable,
                            label = stringResource(R.string.filter_tracks),
                            modifier = Modifier.weight(1f)
                        )
                    }
                }

                else -> {
                    Text(
                        text = stringResource(R.string.catalog_stats_unavailable),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }

            Spacer(modifier = Modifier.height(24.dp))
        }
    }
}

@Composable
private fun CatalogStatCard(
    icon: ImageVector,
    available: Int,
    unavailable: Int,
    label: String,
    modifier: Modifier = Modifier
) {
    Card(
        modifier = modifier,
        colors = CardDefaults.cardColors(
            containerColor = MaterialTheme.colorScheme.surfaceVariant
        )
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(12.dp),
            horizontalAlignment = Alignment.CenterHorizontally
        ) {
            Icon(
                imageVector = icon,
                contentDescription = null,
                modifier = Modifier.size(24.dp),
                tint = MaterialTheme.colorScheme.primary
            )
            Spacer(modifier = Modifier.height(8.dp))
            Text(
                text = formatCount(available),
                style = MaterialTheme.typography.titleLarge,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
            Text(
                text = label,
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant
            )
            if (unavailable > 0) {
                Text(
                    text = stringResource(R.string.catalog_stat_na, formatCount(unavailable)),
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.6f)
                )
            }
        }
    }
}

private fun formatCount(count: Int): String {
    return when {
        count >= 1_000_000 -> String.format(Locale.US, "%.1fM", count / 1_000_000.0)
        count >= 1_000 -> String.format(Locale.US, "%.1fK", count / 1_000.0)
        else -> count.toString()
    }
}

@Composable
private fun AboutLabel(text: String) {
    Text(
        text = text,
        style = MaterialTheme.typography.labelMedium,
        color = MaterialTheme.colorScheme.primary
    )
}

@Composable
@Preview(showBackground = true)
fun AboutScreenPreview() {
    AboutScreenInternal(
        AboutScreenState(
            appName = "Pezzottify",
            versionName = "1.0.0",
            gitCommit = "abc1234",
            serverUrl = "https://music.example.com",
            serverVersion = "0.5.0 (def5678)",
            catalogStats = CatalogStats(
                artists = CatalogStatItem(available = 1200, unavailable = 50),
                albums = CatalogStatItem(available = 3500, unavailable = 120),
                tracks = CatalogStatItem(available = 25000, unavailable = 800),
            ),
        )
    )
}
