package com.lelloman.pezzottify.android.ui.screen.main.content.work

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.navigation.NavController
import com.lelloman.pezzottify.android.domain.statics.*
import com.lelloman.pezzottify.android.ui.R
import com.lelloman.pezzottify.android.ui.component.ArtistAvatarRow
import com.lelloman.pezzottify.android.ui.component.NullablePezzottifyImage
import com.lelloman.pezzottify.android.ui.component.PezzottifyImageShape
import com.lelloman.pezzottify.android.ui.screen.main.MainScreenScaffold
import com.lelloman.pezzottify.android.ui.toAlbum
import com.lelloman.pezzottify.android.ui.toArtist
import com.lelloman.pezzottify.android.ui.toTrack
import com.lelloman.pezzottify.android.ui.toWork

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun WorkScreen(workId: String, navController: NavController) {
    val vm = hiltViewModel<WorkScreenViewModel, WorkScreenViewModel.Factory>(creationCallback = { it.create(workId) })
    val state by vm.state.collectAsState()
    val uriHandler = LocalUriHandler.current
    MainScreenScaffold(topBar = {
        TopAppBar(title = { Text(stringResource(R.string.work)) }, navigationIcon = {
            IconButton(onClick = { navController.popBackStack() }) {
                Icon(Icons.AutoMirrored.Filled.ArrowBack, stringResource(R.string.back))
            }
        })
    }) { padding ->
        LazyColumn(modifier = Modifier.fillMaxSize().padding(padding), contentPadding = PaddingValues(16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp)) {
            state.work?.let { work ->
                item(key = "identity") {
                    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                        Text(work.title, style = MaterialTheme.typography.headlineMedium)
                        work.kind?.takeIf { it.isNotBlank() }?.let { Text(it.replace('_', ' ')) }
                        if (work.creatorArtistIds.isNotEmpty()) {
                            ArtistAvatarRow(work.creatorArtistIds.take(4), vm.contentResolver, navController::toArtist)
                        } else {
                            NullablePezzottifyImage(url = null, shape = PezzottifyImageShape.SmallSquare)
                        }
                        if (work.creators.isNotEmpty()) Text(work.creators.joinToString(" · "))
                        work.compositionYear?.let { Text(stringResource(R.string.work_composed, it)) }
                    }
                }
                state.relations.groupBy { it.relationshipType to it.direction }.forEach { (key, relations) ->
                    item(key = "relation-$key") {
                        var expanded by rememberSaveable(workId, key) { mutableStateOf(false) }
                        Column {
                            TextButton(onClick = { expanded = !expanded }) {
                                Text("${relationLabel(key.first, key.second)} (${relations.size})")
                            }
                            if (expanded) relations.forEach { relation ->
                                TextButton(onClick = { navController.toWork(relation.work.id) }) {
                                    Text(relation.work.title)
                                }
                            }
                        }
                    }
                }
                item(key = "scope") {
                    var expanded by remember { mutableStateOf(false) }
                    Column {
                        Text(stringResource(R.string.work_recordings), style = MaterialTheme.typography.titleLarge)
                        Box {
                            OutlinedButton(onClick = { expanded = true }) { Text(scopeLabel(state.scope)) }
                            DropdownMenu(expanded = expanded, onDismissRequest = { expanded = false }) {
                                WorkScope.entries.forEach { scope ->
                                    DropdownMenuItem(text = { Text(scopeLabel(scope)) }, onClick = {
                                        expanded = false
                                        vm.changeScope(scope)
                                    })
                                }
                            }
                        }
                    }
                }
            }
            items(state.tracks, key = { "track-${it.track.id}" }) { recording ->
                Row(modifier = Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
                    Column(Modifier.weight(1f)) {
                        Text(recording.track.name, style = MaterialTheme.typography.titleMedium,
                            modifier = Modifier.clickable { navController.toTrack(recording.track.id) }.padding(vertical = 8.dp))
                        Text(recording.artists.joinToString(" · ") { it.artist.name })
                        TextButton(onClick = { navController.toAlbum(recording.album.id) }) { Text(recording.album.name) }
                        if (recording.relationshipScope != "direct") recording.recordingWork?.let { work ->
                            TextButton(onClick = { navController.toWork(work.id) }) {
                                Text(stringResource(if (recording.relationshipScope == "part") R.string.work_part_recording else R.string.work_related_recording, work.title))
                            }
                        }
                    }
                    IconButton(onClick = { vm.play(recording) }, enabled = recording.track.availability == TrackAvailability.Available) {
                        Icon(Icons.Default.PlayArrow, stringResource(R.string.work_play_recording, recording.track.name))
                    }
                }
            }
            if (state.loading) item { CircularProgressIndicator() }
            if (state.failed) item {
                Column {
                    Text(stringResource(if (state.notFound) R.string.work_not_found else R.string.work_load_error))
                    TextButton(onClick = vm::loadMore) { Text(stringResource(R.string.work_retry)) }
                }
            }
            if (state.hasMore && !state.loading && !state.failed) item(key = "next-page-${state.scope}-${state.nextOffset}") {
                LaunchedEffect(state.scope, state.nextOffset) { vm.loadMore() }
                TextButton(onClick = vm::loadMore) { Text(stringResource(R.string.work_load_more)) }
            }
            if (state.work != null && state.tracks.isEmpty() && !state.hasMore && !state.loading && !state.failed) item {
                Text(stringResource(R.string.work_no_recordings))
            }
            state.work?.let { work ->
                if (work.catalogNumber != null || work.musicbrainzId != null || work.wikidataId != null) item(key = "about") {
                    var expanded by rememberSaveable(workId) { mutableStateOf(false) }
                    Column {
                        TextButton(onClick = { expanded = !expanded }) { Text(stringResource(R.string.work_about)) }
                        if (expanded) {
                            work.catalogNumber?.let { Text(it) }
                            work.musicbrainzId?.let { id ->
                                TextButton(onClick = { uriHandler.openUri("https://musicbrainz.org/work/$id") }) { Text("MusicBrainz") }
                            }
                            work.wikidataId?.let { id ->
                                TextButton(onClick = { uriHandler.openUri("https://www.wikidata.org/wiki/$id") }) { Text("Wikidata") }
                            }
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun scopeLabel(scope: WorkScope): String = stringResource(when (scope) {
    WorkScope.All -> R.string.work_scope_all
    WorkScope.Parts -> R.string.work_scope_parts
    WorkScope.Related -> R.string.work_scope_related
})

@Composable
private fun relationLabel(type: String, direction: String): String {
    val outgoing = direction == "outgoing"
    val resource = when (type) {
        "parts" -> if (outgoing) R.string.work_parts else R.string.work_part_of
        "arrangement" -> if (outgoing) R.string.work_arrangements else R.string.work_arrangement_of
        "orchestration" -> if (outgoing) R.string.work_orchestrations else R.string.work_orchestration_of
        "based on" -> if (outgoing) R.string.work_based_on_this else R.string.work_based_on
        "other version" -> if (outgoing) R.string.work_versions else R.string.work_version_of
        "adaptation" -> if (outgoing) R.string.work_adaptations else R.string.work_adaptation_of
        "revision of" -> if (outgoing) R.string.work_revisions else R.string.work_revision_of
        "included works" -> if (outgoing) R.string.work_included else R.string.work_included_in
        "medley" -> if (outgoing) R.string.work_medley_of else R.string.work_in_medleys
        "musical quotation" -> if (outgoing) R.string.work_quotes_music else R.string.work_music_quoted
        "lyrical quotation" -> if (outgoing) R.string.work_quotes_lyrics else R.string.work_lyrics_quoted
        "named after work" -> if (outgoing) R.string.work_named_after else R.string.work_inspired_name
        else -> return stringResource(if (outgoing) R.string.work_relation_outgoing else R.string.work_relation_incoming, type)
    }
    return stringResource(resource)
}
