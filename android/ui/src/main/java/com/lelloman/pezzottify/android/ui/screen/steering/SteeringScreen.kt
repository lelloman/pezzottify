package com.lelloman.pezzottify.android.ui.screen.steering

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowLeft
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.Remove
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.material3.FilterChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.InputChip
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Slider
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.navigation.NavController
import com.lelloman.pezzottify.android.ui.R
import kotlin.math.roundToInt

@Composable
fun SteeringScreen(navController: NavController) {
    val viewModel = hiltViewModel<SteeringScreenViewModel>()
    val state by viewModel.state.collectAsState()
    SteeringScreenContent(state = state, actions = viewModel, onBack = { navController.popBackStack() })
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SteeringScreenContent(
    state: SteeringScreenState,
    actions: SteeringScreenActions,
    onBack: () -> Unit,
) {
    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(stringResource(R.string.steering_title)) },
                navigationIcon = {
                    IconButton(onClick = onBack) {
                        Icon(
                            imageVector = Icons.AutoMirrored.Filled.KeyboardArrowLeft,
                            contentDescription = stringResource(R.string.back),
                        )
                    }
                },
            )
        },
    ) { innerPadding ->
        Box(modifier = Modifier.fillMaxSize().padding(innerPadding)) {
            when (state.availability) {
                SteeringAvailability.Loading -> CircularProgressIndicator(Modifier.align(Alignment.Center))
                SteeringAvailability.NothingPlaying -> EmptyMessage(stringResource(R.string.steering_nothing_playing))
                SteeringAvailability.Radio -> EmptyMessage(stringResource(R.string.steering_radio))
                SteeringAvailability.Ready -> SteeringControls(state, actions)
            }
        }
    }

    state.search?.let { search ->
        ReferenceSearchSheet(search = search, actions = actions)
    }
}

@Composable
private fun EmptyMessage(text: String) {
    Box(modifier = Modifier.fillMaxSize().padding(32.dp), contentAlignment = Alignment.Center) {
        Text(
            text = text,
            style = MaterialTheme.typography.bodyLarge,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = TextAlign.Center,
        )
    }
}

@Composable
private fun SteeringControls(state: SteeringScreenState, actions: SteeringScreenActions) {
    LazyColumn(
        contentPadding = PaddingValues(16.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        if (!state.smartContinuationEnabled) {
            item { SmartContinuationOffBanner(onTurnOn = { actions.setSmartContinuationEnabled(true) }) }
        }
        if (state.isRemote) {
            item {
                Text(
                    text = stringResource(R.string.steering_remote),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
        item { SourceCard(state.source, actions) }
        item { DestinationCard(state, actions) }
        item { KnobsCard(state, actions) }
    }
}

@Composable
private fun SmartContinuationOffBanner(onTurnOn: () -> Unit) {
    Card(colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.secondaryContainer)) {
        Row(
            modifier = Modifier.fillMaxWidth().padding(16.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                text = stringResource(R.string.steering_smart_off),
                style = MaterialTheme.typography.bodyMedium,
                modifier = Modifier.weight(1f),
            )
            TextButton(onClick = onTurnOn) { Text(stringResource(R.string.steering_turn_on)) }
        }
    }
}

@Composable
private fun SectionCard(title: String, content: @Composable () -> Unit) {
    Card(modifier = Modifier.fillMaxWidth()) {
        Column(modifier = Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(text = title, style = MaterialTheme.typography.titleMedium)
            content()
        }
    }
}

@Composable
private fun SourceCard(source: SteeringSource, actions: SteeringScreenActions) {
    SectionCard(title = stringResource(R.string.steering_source)) {
        when (source) {
            is SteeringSource.Queue -> Text(
                text = stringResource(
                    R.string.steering_source_queue,
                    source.userChosenCount,
                    source.suggestedCount,
                ),
                style = MaterialTheme.typography.bodyMedium,
            )

            is SteeringSource.References -> {
                Text(
                    text = stringResource(R.string.steering_source_references),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                ReferenceChips(source.references, onRemove = actions::removeSourceReference)
            }
        }
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedButton(onClick = { actions.openSearch(SteeringSearchTarget.Source) }) {
                Text(stringResource(R.string.steering_add_anchor))
            }
            if (source is SteeringSource.References) {
                TextButton(onClick = actions::resetSourceToQueue) {
                    Text(stringResource(R.string.steering_reset_to_queue))
                }
            }
        }
    }
}

@Composable
private fun DestinationCard(state: SteeringScreenState, actions: SteeringScreenActions) {
    SectionCard(title = stringResource(R.string.steering_destination)) {
        val destination = state.destination
        if (destination == null) {
            Text(
                text = stringResource(R.string.steering_no_destination),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            OutlinedButton(onClick = { actions.openSearch(SteeringSearchTarget.Destination) }) {
                Text(stringResource(R.string.steering_set_destination))
            }
            return@SectionCard
        }
        Text(
            text = "${entityTypeLabel(destination.reference.entityType)} · ${destination.reference.label}",
            style = MaterialTheme.typography.bodyLarge,
            maxLines = 2,
            overflow = TextOverflow.Ellipsis,
        )
        LinearProgressIndicator(
            progress = { destination.progress },
            modifier = Modifier.fillMaxWidth(),
        )
        Text(
            text = stringResource(R.string.steering_progress, state.stepsDone, state.stepsTotal),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        destination.queryToDestination?.let { similarity ->
            Text(
                text = stringResource(R.string.steering_match, (similarity.coerceIn(0f, 1f) * 100).roundToInt()),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(
                text = stringResource(R.string.steering_steps_remaining),
                style = MaterialTheme.typography.bodyMedium,
                modifier = Modifier.weight(1f),
            )
            FilledTonalIconButton(
                onClick = { actions.setStepsRemaining(state.stepsRemaining - 1) },
                enabled = state.stepsRemaining > 1,
            ) {
                Icon(Icons.Default.Remove, contentDescription = null)
            }
            Text(
                text = state.stepsRemaining.toString(),
                style = MaterialTheme.typography.titleMedium,
                textAlign = TextAlign.Center,
                modifier = Modifier.width(48.dp),
            )
            FilledTonalIconButton(onClick = { actions.setStepsRemaining(state.stepsRemaining + 1) }) {
                Icon(Icons.Default.Add, contentDescription = null)
            }
        }
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedButton(onClick = { actions.openSearch(SteeringSearchTarget.Destination) }) {
                Text(stringResource(R.string.steering_change_destination))
            }
            TextButton(onClick = actions::clearDestination) {
                Text(stringResource(R.string.steering_clear_destination))
            }
        }
    }
}

@Composable
private fun KnobsCard(state: SteeringScreenState, actions: SteeringScreenActions) {
    val knobs = state.knobs
    SectionCard(title = stringResource(R.string.steering_knobs)) {
        KnobSlider(
            label = stringResource(R.string.steering_recency),
            hint = stringResource(R.string.steering_recency_hint),
            value = knobs.recencyWeight,
            onCommit = actions::setRecencyWeight,
        )
        KnobSlider(stringResource(R.string.steering_diversity), null, knobs.diversity, actions::setDiversity)
        KnobSlider(stringResource(R.string.steering_randomness), null, knobs.randomness, actions::setRandomness)

        Text(text = stringResource(R.string.steering_mode), style = MaterialTheme.typography.bodyMedium)
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            FilterChip(
                selected = knobs.mode == SteeringKnobs.MODE_SIMILAR,
                onClick = { actions.setMode(SteeringKnobs.MODE_SIMILAR) },
                label = { Text(stringResource(R.string.steering_mode_similar)) },
            )
            FilterChip(
                selected = knobs.mode == SteeringKnobs.MODE_EXPLORE,
                enabled = state.canUseExplore,
                onClick = { actions.setMode(SteeringKnobs.MODE_EXPLORE) },
                label = { Text(stringResource(R.string.steering_mode_explore)) },
            )
        }
        if (!state.canUseExplore) {
            Text(
                text = stringResource(R.string.steering_explore_disabled),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }

        if (knobs.criteria.size > 1) {
            HorizontalDivider()
            Text(text = stringResource(R.string.steering_criteria), style = MaterialTheme.typography.bodyMedium)
            knobs.criteria.forEach { criterion ->
                KnobSlider(
                    label = criterion.label,
                    hint = null,
                    value = criterion.weight,
                    onCommit = { actions.setCriterionWeight(criterion.namespace, it) },
                )
            }
        }

        HorizontalDivider()
        Text(text = stringResource(R.string.steering_away), style = MaterialTheme.typography.bodyMedium)
        ReferenceChips(knobs.away, onRemove = actions::removeAway)
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedButton(onClick = { actions.openSearch(SteeringSearchTarget.Away) }) {
                Text(stringResource(R.string.steering_add_away))
            }
            if (!knobs.isDefault) {
                TextButton(onClick = actions::resetKnobs) {
                    Text(stringResource(R.string.steering_reset_knobs))
                }
            }
        }
    }
}

/** Slider that shows changes while dragging but only commits on release. */
@Composable
private fun KnobSlider(label: String, hint: String?, value: Float, onCommit: (Float) -> Unit) {
    var dragValue by remember(value) { mutableFloatStateOf(value) }
    Column {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(text = label, style = MaterialTheme.typography.bodyMedium, modifier = Modifier.weight(1f))
            Text(
                text = "%.2f".format(dragValue),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        hint?.let {
            Text(
                text = it,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        Slider(
            value = dragValue,
            onValueChange = { dragValue = it },
            onValueChangeFinished = { onCommit(dragValue) },
            valueRange = 0f..1f,
        )
    }
}

@Composable
private fun ReferenceChips(references: List<SteeringReference>, onRemove: (SteeringReference) -> Unit) {
    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        references.forEach { reference ->
            InputChip(
                selected = false,
                onClick = { onRemove(reference) },
                label = {
                    Text(
                        text = "${entityTypeLabel(reference.entityType)} · ${reference.label}",
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                },
                trailingIcon = {
                    Icon(
                        Icons.Default.Close,
                        contentDescription = stringResource(R.string.steering_remove),
                        modifier = Modifier.size(18.dp),
                    )
                },
            )
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun ReferenceSearchSheet(search: SteeringSearch, actions: SteeringScreenActions) {
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    ModalBottomSheet(onDismissRequest = actions::closeSearch, sheetState = sheetState) {
        Column(modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp)) {
            OutlinedTextField(
                value = search.query,
                onValueChange = actions::updateSearchQuery,
                placeholder = { Text(stringResource(R.string.steering_search_hint)) },
                singleLine = true,
                modifier = Modifier.fillMaxWidth(),
            )
            Spacer(Modifier.height(8.dp))
            when {
                search.isSearching -> CircularProgressIndicator(
                    modifier = Modifier.align(Alignment.CenterHorizontally).padding(16.dp),
                )

                search.isError -> SheetMessage(stringResource(R.string.steering_search_error))
                search.query.isNotBlank() && search.results.isEmpty() ->
                    SheetMessage(stringResource(R.string.steering_search_empty))

                else -> LazyColumn(modifier = Modifier.height(360.dp)) {
                    items(search.results, key = { "${it.entityType}:${it.entityId}" }) { reference ->
                        Row(
                            modifier = Modifier
                                .fillMaxWidth()
                                .clickable { actions.pickSearchResult(reference) }
                                .padding(vertical = 12.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Icon(
                                painter = painterResource(entityTypeIcon(reference.entityType)),
                                contentDescription = null,
                                tint = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                            Spacer(Modifier.width(16.dp))
                            Column(modifier = Modifier.weight(1f)) {
                                Text(
                                    text = reference.label,
                                    style = MaterialTheme.typography.bodyLarge,
                                    maxLines = 1,
                                    overflow = TextOverflow.Ellipsis,
                                )
                                Text(
                                    text = entityTypeLabel(reference.entityType),
                                    style = MaterialTheme.typography.bodySmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                )
                            }
                        }
                    }
                }
            }
            Spacer(Modifier.height(24.dp))
        }
    }
}

@Composable
private fun SheetMessage(text: String) {
    Text(
        text = text,
        style = MaterialTheme.typography.bodyMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.padding(16.dp),
    )
}

@Composable
private fun entityTypeLabel(entityType: String): String = when (entityType) {
    "artist" -> stringResource(R.string.steering_entity_artist)
    "album" -> stringResource(R.string.steering_entity_album)
    else -> stringResource(R.string.steering_entity_track)
}

private fun entityTypeIcon(entityType: String): Int = when (entityType) {
    "artist" -> R.drawable.baseline_face_24
    "album" -> R.drawable.baseline_album_24
    else -> R.drawable.baseline_music_note_24
}
