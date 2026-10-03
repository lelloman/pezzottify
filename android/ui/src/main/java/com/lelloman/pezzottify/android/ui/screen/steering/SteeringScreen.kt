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
import androidx.compose.material.icons.outlined.Lightbulb
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalIconButton
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
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.rememberVectorPainter
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
        item {
            Text(
                text = stringResource(R.string.steering_intro),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
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
        item { AlongTheWayCard(state, actions) }
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
private fun SectionCard(title: String, help: String? = null, content: @Composable () -> Unit) {
    Card(modifier = Modifier.fillMaxWidth()) {
        Column(modifier = Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(text = title, style = MaterialTheme.typography.titleMedium)
            help?.let { HelpText(it) }
            content()
        }
    }
}

@Composable
private fun HelpText(text: String) {
    Text(
        text = text,
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
}

@Composable
private fun SourceCard(source: SteeringSource, actions: SteeringScreenActions) {
    SectionCard(
        title = stringResource(R.string.steering_source),
        help = stringResource(R.string.steering_source_help),
    ) {
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
    SectionCard(
        title = stringResource(R.string.steering_destination),
        help = stringResource(R.string.steering_destination_help),
    ) {
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
        if (destination.components.size > 1) {
            Text(
                text = stringResource(R.string.steering_destination_mix),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        destination.components.forEach { component ->
            DestinationComponentRow(
                component = component,
                showWeight = destination.components.size > 1,
                actions = actions,
            )
        }
        LinearProgressIndicator(
            progress = { destination.progress },
            modifier = Modifier.fillMaxWidth(),
        )
        HelpText(stringResource(R.string.steering_progress, state.stepsDone, state.stepsTotal))
        destination.queryToDestination?.let { similarity ->
            Text(
                text = stringResource(R.string.steering_match, percent(similarity)),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        Row(verticalAlignment = Alignment.CenterVertically) {
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = stringResource(R.string.steering_steps_remaining, state.stepsRemaining),
                    style = MaterialTheme.typography.bodyMedium,
                )
                HelpText(stringResource(R.string.steering_steps_help))
            }
            FilledTonalIconButton(
                onClick = { actions.setStepsRemaining(state.stepsRemaining - 1) },
                enabled = state.stepsRemaining > 1,
            ) {
                Icon(Icons.Default.Remove, contentDescription = stringResource(R.string.steering_fewer_tracks))
            }
            Spacer(Modifier.width(8.dp))
            FilledTonalIconButton(onClick = { actions.setStepsRemaining(state.stepsRemaining + 1) }) {
                Icon(Icons.Default.Add, contentDescription = stringResource(R.string.steering_more_tracks))
            }
        }
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedButton(
                onClick = { actions.openSearch(SteeringSearchTarget.Destination) },
                enabled = !destination.isFull,
            ) {
                Text(stringResource(R.string.steering_add_to_mix))
            }
            TextButton(onClick = actions::clearDestination) {
                Text(stringResource(R.string.steering_clear_destination))
            }
        }
    }
}

@Composable
private fun DestinationComponentRow(
    component: SteeringDestinationComponent,
    showWeight: Boolean,
    actions: SteeringScreenActions,
) {
    val reference = component.reference
    var dragWeight by remember(component.weight) { mutableFloatStateOf(component.weight) }
    Column {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = reference.label,
                    style = MaterialTheme.typography.bodyLarge,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                )
                val detail = buildString {
                    append(referenceKindLabel(reference))
                    component.similarity?.let { append(" · ").append(stringResourceMatch(it)) }
                }
                Text(
                    text = detail,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            IconButton(onClick = { actions.removeDestinationComponent(reference) }) {
                Icon(Icons.Default.Close, contentDescription = stringResource(R.string.steering_remove))
            }
        }
        if (showWeight) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    text = stringResource(R.string.steering_weight),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                Slider(
                    value = dragWeight,
                    onValueChange = { dragWeight = it },
                    onValueChangeFinished = { actions.setDestinationComponentWeight(reference, dragWeight) },
                    valueRange = MIN_COMPONENT_WEIGHT..MAX_COMPONENT_WEIGHT,
                    modifier = Modifier.weight(1f).padding(horizontal = 8.dp),
                )
                Text(
                    text = "%.1f".format(dragWeight),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
    }
}

@Composable
private fun stringResourceMatch(similarity: Float): String =
    stringResource(R.string.steering_component_match, percent(similarity))

private fun percent(similarity: Float): Int = (similarity.coerceIn(0f, 1f) * 100).roundToInt()

// The slider is linear; weights are relative within the mix, so 0.1..2 covers "a hint" to "double".
private const val MIN_COMPONENT_WEIGHT = 0.1f
private const val MAX_COMPONENT_WEIGHT = 2f

@Composable
private fun AlongTheWayCard(state: SteeringScreenState, actions: SteeringScreenActions) {
    val knobs = state.knobs
    var showMore by rememberSaveable { mutableStateOf(false) }
    SectionCard(
        title = stringResource(R.string.steering_knobs),
        help = stringResource(R.string.steering_knobs_help),
    ) {
        Text(text = stringResource(R.string.steering_away), style = MaterialTheme.typography.bodyMedium)
        HelpText(stringResource(R.string.steering_away_help))
        ReferenceChips(knobs.away, onRemove = actions::removeAway)
        OutlinedButton(onClick = { actions.openSearch(SteeringSearchTarget.Away) }) {
            Text(stringResource(R.string.steering_add_away))
        }

        HorizontalDivider()
        SpectrumSlider(
            title = stringResource(R.string.steering_follow),
            help = stringResource(R.string.steering_follow_help),
            startLabel = stringResource(R.string.steering_follow_start),
            endLabel = stringResource(R.string.steering_follow_end),
            value = knobs.recencyWeight,
            onCommit = actions::setRecencyWeight,
        )
        SpectrumSlider(
            title = stringResource(R.string.steering_variety),
            help = stringResource(R.string.steering_variety_help),
            startLabel = stringResource(R.string.steering_variety_start),
            endLabel = stringResource(R.string.steering_variety_end),
            value = knobs.variety,
            onCommit = actions::setVariety,
        )

        if (knobs.criteria.size > 1) {
            TextButton(onClick = { showMore = !showMore }) {
                Text(
                    stringResource(
                        if (showMore) R.string.steering_fewer_options else R.string.steering_more_options
                    )
                )
            }
            if (showMore) {
                Text(text = stringResource(R.string.steering_listen_for), style = MaterialTheme.typography.bodyMedium)
                HelpText(stringResource(R.string.steering_listen_for_help))
                knobs.criteria.forEach { criterion ->
                    CriterionSlider(
                        label = criterionLabel(criterion),
                        value = criterion.weight,
                        onCommit = { actions.setCriterionWeight(criterion.namespace, it) },
                    )
                }
            }
        }

        if (!knobs.isDefault) {
            TextButton(onClick = actions::resetKnobs) {
                Text(stringResource(R.string.steering_reset_knobs))
            }
        }
    }
}

@Composable
private fun criterionLabel(criterion: SteeringCriterion): String = when (criterion.kind) {
    SteeringCriterionKind.OverallSound -> stringResource(R.string.steering_listen_sound)
    SteeringCriterionKind.AudioScene -> stringResource(R.string.steering_listen_scene)
    SteeringCriterionKind.Instruments -> stringResource(R.string.steering_listen_instruments)
    SteeringCriterionKind.Other -> criterion.label
}

/** A 0..1 slider described by what each end means; shows changes while dragging, commits on release. */
@Composable
private fun SpectrumSlider(
    title: String,
    help: String,
    startLabel: String,
    endLabel: String,
    value: Float,
    onCommit: (Float) -> Unit,
) {
    var dragValue by remember(value) { mutableFloatStateOf(value) }
    Column {
        Text(text = title, style = MaterialTheme.typography.bodyMedium)
        HelpText(help)
        Slider(
            value = dragValue,
            onValueChange = { dragValue = it },
            onValueChangeFinished = { onCommit(dragValue) },
            valueRange = 0f..1f,
        )
        Row {
            Text(
                text = startLabel,
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.weight(1f),
            )
            Text(
                text = endLabel,
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                textAlign = TextAlign.End,
                modifier = Modifier.weight(1f),
            )
        }
    }
}

/** How much one way of comparing music counts, from "not at all" to "fully". */
@Composable
private fun CriterionSlider(label: String, value: Float, onCommit: (Float) -> Unit) {
    var dragValue by remember(value) { mutableFloatStateOf(value) }
    Row(verticalAlignment = Alignment.CenterVertically) {
        Text(
            text = label,
            style = MaterialTheme.typography.bodySmall,
            modifier = Modifier.width(112.dp),
        )
        Slider(
            value = dragValue,
            onValueChange = { dragValue = it },
            onValueChangeFinished = { onCommit(dragValue) },
            valueRange = 0f..1f,
            modifier = Modifier.weight(1f),
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
                        text = "${referenceKindLabel(reference)} · ${reference.label}",
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
                search.query.isNotBlank() && search.results.isEmpty() && search.concepts.isEmpty() &&
                    !search.isLoadingConcepts ->
                    SheetMessage(stringResource(R.string.steering_search_empty))

                else -> LazyColumn(modifier = Modifier.height(420.dp)) {
                    items(search.results, key = { "${it.entityType}:${it.entityId}" }) { reference ->
                        SearchResultRow(reference, actions)
                    }
                    if (search.isLoadingConcepts) {
                        item(key = "concepts-loading") { SheetMessage(stringResource(R.string.steering_concepts_loading)) }
                    }
                    search.concepts.forEach { group ->
                        item(key = "family-${group.family}") {
                            Text(
                                text = conceptFamilyLabel(group.family),
                                style = MaterialTheme.typography.titleSmall,
                                color = MaterialTheme.colorScheme.primary,
                                modifier = Modifier.padding(top = 16.dp, bottom = 4.dp),
                            )
                        }
                        items(group.concepts, key = { "concept:${it.entityId}" }) { reference ->
                            SearchResultRow(reference, actions)
                        }
                    }
                }
            }
            Spacer(Modifier.height(24.dp))
        }
    }
}

@Composable
private fun SearchResultRow(reference: SteeringReference, actions: SteeringScreenActions) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clickable { actions.pickSearchResult(reference) }
            .padding(vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (reference.entityType == CONCEPT) {
            Icon(
                painter = rememberVectorPainter(Icons.Outlined.Lightbulb),
                contentDescription = null,
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        } else {
            Icon(
                painter = painterResource(entityTypeIcon(reference.entityType)),
                contentDescription = null,
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        Spacer(Modifier.width(16.dp))
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = reference.label,
                style = MaterialTheme.typography.bodyLarge,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            Text(
                text = referenceKindLabel(reference),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
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

private const val CONCEPT = "concept"

/** "Artist", "Album", "Track", or the concept family ("Instrument", "Recorded in", ...). */
@Composable
private fun referenceKindLabel(reference: SteeringReference): String = when (reference.entityType) {
    "artist" -> stringResource(R.string.steering_entity_artist)
    "album" -> stringResource(R.string.steering_entity_album)
    CONCEPT -> reference.family?.let { conceptFamilyLabel(it) } ?: stringResource(R.string.steering_entity_concept)
    else -> stringResource(R.string.steering_entity_track)
}

@Composable
private fun conceptFamilyLabel(family: String): String = when (family) {
    "sound_genre" -> stringResource(R.string.steering_family_sound_genre)
    "instrument" -> stringResource(R.string.steering_family_instrument)
    "vocals" -> stringResource(R.string.steering_family_vocals)
    "mood" -> stringResource(R.string.steering_family_mood)
    "genre_tag" -> stringResource(R.string.steering_family_genre_tag)
    "recorded" -> stringResource(R.string.steering_family_recorded)
    "composed" -> stringResource(R.string.steering_family_composed)
    else -> stringResource(R.string.steering_entity_concept)
}

private fun entityTypeIcon(entityType: String): Int = when (entityType) {
    "artist" -> R.drawable.baseline_face_24
    "album" -> R.drawable.baseline_album_24
    else -> R.drawable.baseline_music_note_24
}
