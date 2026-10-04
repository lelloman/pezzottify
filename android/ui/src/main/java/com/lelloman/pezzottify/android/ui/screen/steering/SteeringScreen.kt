package com.lelloman.pezzottify.android.ui.screen.steering

import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowForward
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowLeft
import androidx.compose.material.icons.filled.Add
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.KeyboardArrowDown
import androidx.compose.material.icons.filled.Remove
import androidx.compose.material.icons.filled.Search
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Slider
import androidx.compose.material3.SliderDefaults
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TextField
import androidx.compose.material3.TextFieldDefaults
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Shape
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.tooling.preview.Preview
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.navigation.NavController
import com.lelloman.pezzottify.android.ui.R
import com.lelloman.pezzottify.android.ui.component.NullablePezzottifyImage
import com.lelloman.pezzottify.android.ui.component.PezzottifyImagePlaceholder
import com.lelloman.pezzottify.android.ui.component.PezzottifyImageShape
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.flowOf
import kotlin.math.absoluteValue
import kotlin.math.roundToInt

/** Resolves a reference's artwork URL; the screen provides the ViewModel's cached lookup. */
private val LocalSteeringArtwork = staticCompositionLocalOf<(SteeringReference) -> Flow<String?>> {
    { flowOf(null) }
}

@Composable
fun SteeringScreen(navController: NavController) {
    val viewModel = hiltViewModel<SteeringScreenViewModel>()
    val state by viewModel.state.collectAsState()
    SteeringScreenContent(
        state = state,
        actions = viewModel,
        onBack = { navController.popBackStack() },
        artwork = viewModel::artworkUrl,
    )
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SteeringScreenContent(
    state: SteeringScreenState,
    actions: SteeringScreenActions,
    onBack: () -> Unit,
    artwork: (SteeringReference) -> Flow<String?> = { flowOf(null) },
) {
    CompositionLocalProvider(LocalSteeringArtwork provides artwork) {
        val accent by animateColorAsState(heroAccent(state), label = "steeringAccent")
        Box(
            modifier = Modifier
                .fillMaxSize()
                .background(MaterialTheme.colorScheme.surface),
        ) {
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .height(360.dp)
                    .background(
                        Brush.verticalGradient(
                            listOf(accent.copy(alpha = 0.55f), MaterialTheme.colorScheme.surface),
                        ),
                    ),
            )
            Scaffold(
                containerColor = Color.Transparent,
                topBar = {
                    TopAppBar(
                        title = {},
                        navigationIcon = {
                            IconButton(onClick = onBack) {
                                Icon(
                                    imageVector = Icons.AutoMirrored.Filled.KeyboardArrowLeft,
                                    contentDescription = stringResource(R.string.back),
                                )
                            }
                        },
                        colors = TopAppBarDefaults.topAppBarColors(containerColor = Color.Transparent),
                    )
                },
            ) { innerPadding ->
                Box(modifier = Modifier.fillMaxSize().padding(innerPadding)) {
                    when (state.availability) {
                        SteeringAvailability.Loading -> CircularProgressIndicator(Modifier.align(Alignment.Center))
                        SteeringAvailability.NothingPlaying ->
                            EmptyMessage(stringResource(R.string.steering_nothing_playing))
                        SteeringAvailability.Radio -> EmptyMessage(stringResource(R.string.steering_radio))
                        SteeringAvailability.Ready -> SteeringControls(state, actions, accent)
                    }
                }
            }
        }

        state.search?.let { search ->
            ReferenceSearchSheet(search = search, actions = actions)
        }
    }
}

@Composable
private fun heroAccent(state: SteeringScreenState): Color {
    val first = state.destination?.components?.firstOrNull()?.reference
    return when {
        first == null -> MaterialTheme.colorScheme.surfaceVariant
        first.entityType == CONCEPT -> conceptColor(first.entityId)
        else -> MaterialTheme.colorScheme.primary
    }
}

@Composable
private fun EmptyMessage(text: String) {
    Box(modifier = Modifier.fillMaxSize().padding(32.dp), contentAlignment = Alignment.Center) {
        Text(
            text = text,
            style = MaterialTheme.typography.titleMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = TextAlign.Center,
        )
    }
}

@Composable
private fun SteeringControls(state: SteeringScreenState, actions: SteeringScreenActions, accent: Color) {
    LazyColumn(
        contentPadding = PaddingValues(start = 20.dp, end = 20.dp, bottom = 32.dp),
        verticalArrangement = Arrangement.spacedBy(28.dp),
    ) {
        item {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(
                    text = stringResource(R.string.steering_title),
                    style = MaterialTheme.typography.displaySmall,
                    fontWeight = FontWeight.Bold,
                )
                Text(
                    text = stringResource(R.string.steering_intro),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
        if (!state.smartContinuationEnabled) {
            item { SmartContinuationOffBanner(onTurnOn = { actions.setSmartContinuationEnabled(true) }) }
        }
        if (state.isRemote) {
            item {
                Text(
                    text = stringResource(R.string.steering_remote),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
        item { JourneyHero(state, actions, accent) }
        item { StartingPointSection(state.source, actions) }
        item { HeadingToSection(state, actions) }
        item { AlongTheWaySection(state, actions) }
    }
}

@Composable
private fun SmartContinuationOffBanner(onTurnOn: () -> Unit) {
    Surface(
        shape = RoundedCornerShape(16.dp),
        color = MaterialTheme.colorScheme.secondaryContainer,
    ) {
        Row(
            modifier = Modifier.fillMaxWidth().padding(start = 16.dp, end = 8.dp, top = 8.dp, bottom = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                text = stringResource(R.string.steering_smart_off),
                style = MaterialTheme.typography.bodyMedium,
                modifier = Modifier.weight(1f),
            )
            Button(onClick = onTurnOn, shape = CircleShape) { Text(stringResource(R.string.steering_turn_on)) }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Journey: starting point -> heading to
// ---------------------------------------------------------------------------------------------

@Composable
private fun JourneyHero(state: SteeringScreenState, actions: SteeringScreenActions, accent: Color) {
    val destination = state.destination
    Column(verticalArrangement = Arrangement.spacedBy(16.dp)) {
        Row(verticalAlignment = Alignment.Top) {
            JourneyEnd(
                caption = stringResource(R.string.steering_source),
                title = sourceTitle(state.source),
                modifier = Modifier.weight(1f),
            ) { SourceArtwork(state.source, Modifier.fillMaxWidth().aspectRatio(1f)) }
            Icon(
                imageVector = Icons.AutoMirrored.Filled.ArrowForward,
                contentDescription = null,
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(horizontal = 12.dp).padding(top = 56.dp),
            )
            JourneyEnd(
                caption = stringResource(R.string.steering_destination),
                title = destinationTitle(destination),
                modifier = Modifier.weight(1f),
            ) {
                if (destination == null) {
                    EmptyDestinationTile(
                        onClick = { actions.openSearch(SteeringSearchTarget.Destination) },
                        modifier = Modifier.fillMaxWidth().aspectRatio(1f),
                    )
                } else {
                    MixArtwork(destination.components.map { it.reference }, Modifier.fillMaxWidth().aspectRatio(1f))
                }
            }
        }
        if (destination == null) {
            Button(
                onClick = { actions.openSearch(SteeringSearchTarget.Destination) },
                shape = CircleShape,
                contentPadding = PaddingValues(vertical = 16.dp),
                modifier = Modifier.fillMaxWidth(),
            ) {
                Text(
                    text = stringResource(R.string.steering_set_destination),
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.Bold,
                )
            }
        } else {
            val progress by animateFloatAsState(destination.progress, label = "steeringProgress")
            Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                LinearProgressIndicator(
                    progress = { progress },
                    color = if (accent == MaterialTheme.colorScheme.surfaceVariant) {
                        MaterialTheme.colorScheme.primary
                    } else {
                        accent
                    },
                    trackColor = MaterialTheme.colorScheme.surfaceVariant,
                    strokeCap = StrokeCap.Round,
                    modifier = Modifier.fillMaxWidth().height(6.dp),
                )
                Row {
                    MutedText(
                        text = stringResource(R.string.steering_progress, state.stepsDone, state.stepsTotal),
                        modifier = Modifier.weight(1f),
                    )
                    destination.queryToDestination?.let {
                        MutedText(stringResource(R.string.steering_component_match, percent(it)))
                    }
                }
            }
        }
    }
}

@Composable
private fun JourneyEnd(
    caption: String,
    title: String,
    modifier: Modifier = Modifier,
    artwork: @Composable () -> Unit,
) {
    Column(modifier = modifier, verticalArrangement = Arrangement.spacedBy(8.dp)) {
        artwork()
        Column {
            Text(
                text = caption.uppercase(),
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            Text(
                text = title,
                style = MaterialTheme.typography.titleMedium,
                fontWeight = FontWeight.Bold,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
        }
    }
}

@Composable
private fun sourceTitle(source: SteeringSource): String = when (source) {
    is SteeringSource.Queue -> stringResource(R.string.steering_your_queue)
    is SteeringSource.References -> mixTitle(source.references)
}

@Composable
private fun destinationTitle(destination: SteeringDestination?): String =
    if (destination == null) "—" else mixTitle(destination.components.map { it.reference })

@Composable
private fun mixTitle(references: List<SteeringReference>): String = when (references.size) {
    0 -> "—"
    1 -> references.first().label
    else -> stringResource(R.string.steering_mix_title, references.size)
}

@Composable
private fun SourceArtwork(source: SteeringSource, modifier: Modifier) {
    when (source) {
        is SteeringSource.Queue -> QueueCollage(source.previewTrackIds, modifier)
        is SteeringSource.References -> MixArtwork(source.references, modifier)
    }
}

/** One reference shows its own artwork; several are a 2x2 grid of the first four. */
@Composable
private fun MixArtwork(references: List<SteeringReference>, modifier: Modifier) {
    if (references.size <= 1) {
        references.firstOrNull()?.let { ReferenceArtwork(it, modifier, corner = 8.dp, showText = true) }
            ?: ArtworkPlaceholder(modifier)
        return
    }
    Grid2x2(modifier) { index ->
        references.getOrNull(index)?.let { ReferenceArtwork(it, Modifier.fillMaxSize(), corner = 0.dp) }
            ?: Box(Modifier.fillMaxSize().background(MaterialTheme.colorScheme.surfaceVariant))
    }
}

/** Album art of the queue's first chosen tracks: a 2x2 collage when there are four albums. */
@Composable
private fun QueueCollage(trackIds: List<String>, modifier: Modifier) {
    val provider = LocalSteeringArtwork.current
    val urls = trackIds.take(8).map { trackId ->
        key(trackId) {
            val flow = remember(trackId) { provider(SteeringReference("track", trackId, "")) }
            flow.collectAsState(initial = null).value
        }
    }.filterNotNull().distinct()
    when {
        urls.size >= 4 -> Grid2x2(modifier) { index ->
            NullablePezzottifyImage(url = urls[index], shape = PezzottifyImageShape.FullSize)
        }
        urls.isNotEmpty() -> Box(modifier.clip(RoundedCornerShape(8.dp))) {
            NullablePezzottifyImage(url = urls.first(), shape = PezzottifyImageShape.FullSize)
        }
        else -> ArtworkPlaceholder(modifier)
    }
}

@Composable
private fun Grid2x2(modifier: Modifier, cell: @Composable (Int) -> Unit) {
    Column(modifier.clip(RoundedCornerShape(8.dp))) {
        repeat(2) { row ->
            Row(Modifier.weight(1f)) {
                repeat(2) { column ->
                    Box(Modifier.weight(1f).fillMaxSize()) { cell(row * 2 + column) }
                }
            }
        }
    }
}

@Composable
private fun ArtworkPlaceholder(modifier: Modifier) {
    Box(
        modifier = modifier
            .clip(RoundedCornerShape(8.dp))
            .background(MaterialTheme.colorScheme.surfaceVariant),
        contentAlignment = Alignment.Center,
    ) {
        Icon(
            painter = painterResource(R.drawable.baseline_music_note_24),
            contentDescription = null,
            tint = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.size(40.dp),
        )
    }
}

@Composable
private fun EmptyDestinationTile(onClick: () -> Unit, modifier: Modifier) {
    Box(
        modifier = modifier
            .clip(RoundedCornerShape(8.dp))
            .border(2.dp, MaterialTheme.colorScheme.outlineVariant, RoundedCornerShape(8.dp))
            .clickable(onClick = onClick),
        contentAlignment = Alignment.Center,
    ) {
        Icon(
            imageVector = Icons.Default.Add,
            contentDescription = stringResource(R.string.steering_set_destination),
            tint = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.size(40.dp),
        )
    }
}

// ---------------------------------------------------------------------------------------------
// Sections
// ---------------------------------------------------------------------------------------------

@Composable
private fun SectionHeader(title: String, help: String) {
    Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
        Text(text = title, style = MaterialTheme.typography.titleLarge, fontWeight = FontWeight.Bold)
        MutedText(help)
    }
}

@Composable
private fun MutedText(text: String, modifier: Modifier = Modifier) {
    Text(
        text = text,
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = modifier,
    )
}

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun StartingPointSection(source: SteeringSource, actions: SteeringScreenActions) {
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        SectionHeader(stringResource(R.string.steering_source), stringResource(R.string.steering_source_help))
        when (source) {
            is SteeringSource.Queue -> Row(verticalAlignment = Alignment.CenterVertically) {
                QueueCollage(source.previewTrackIds, Modifier.size(56.dp))
                Spacer(Modifier.width(12.dp))
                Column {
                    Text(
                        text = stringResource(R.string.steering_your_queue),
                        style = MaterialTheme.typography.titleSmall,
                        fontWeight = FontWeight.Bold,
                    )
                    MutedText(
                        stringResource(R.string.steering_source_queue, source.userChosenCount, source.suggestedCount),
                    )
                }
            }

            is SteeringSource.References -> {
                MutedText(stringResource(R.string.steering_source_references))
                FlowRow(
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    source.references.forEach { reference ->
                        ReferencePill(reference, onRemove = { actions.removeSourceReference(reference) })
                    }
                }
            }
        }
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            FilledTonalButton(onClick = { actions.openSearch(SteeringSearchTarget.Source) }, shape = CircleShape) {
                Text(stringResource(R.string.steering_add_anchor))
            }
            if (source is SteeringSource.References) {
                TextButton(onClick = actions::resetSourceToQueue, shape = CircleShape) {
                    Text(stringResource(R.string.steering_reset_to_queue))
                }
            }
        }
    }
}

@Composable
private fun HeadingToSection(state: SteeringScreenState, actions: SteeringScreenActions) {
    val destination = state.destination
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        SectionHeader(stringResource(R.string.steering_destination), stringResource(R.string.steering_destination_help))
        if (destination == null) {
            MutedText(stringResource(R.string.steering_no_destination))
            return@Column
        }
        if (destination.components.size > 1) {
            MutedText(stringResource(R.string.steering_destination_mix))
        }
        val totalWeight = destination.components.sumOf { it.weight.toDouble() }.toFloat().coerceAtLeast(0.0001f)
        destination.components.forEach { component ->
            key(component.reference.entityType, component.reference.entityId) {
                DestinationComponentChip(
                    component = component,
                    share = component.weight / totalWeight,
                    showWeight = destination.components.size > 1,
                    actions = actions,
                )
            }
        }
        Row(verticalAlignment = Alignment.CenterVertically) {
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = stringResource(R.string.steering_steps_remaining, state.stepsRemaining),
                    style = MaterialTheme.typography.titleSmall,
                    fontWeight = FontWeight.Bold,
                )
                MutedText(stringResource(R.string.steering_steps_help))
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
            FilledTonalButton(
                onClick = { actions.openSearch(SteeringSearchTarget.Destination) },
                enabled = !destination.isFull,
                shape = CircleShape,
            ) {
                Icon(Icons.Default.Add, contentDescription = null, modifier = Modifier.size(18.dp))
                Spacer(Modifier.width(6.dp))
                Text(stringResource(R.string.steering_add_to_mix))
            }
            TextButton(onClick = actions::clearDestination, shape = CircleShape) {
                Text(stringResource(R.string.steering_clear_destination))
            }
        }
    }
}

/** A mix component: artwork, name, its share of the mix; tap to adjust how much it counts. */
@Composable
private fun DestinationComponentChip(
    component: SteeringDestinationComponent,
    share: Float,
    showWeight: Boolean,
    actions: SteeringScreenActions,
) {
    val reference = component.reference
    var expanded by rememberSaveable(reference.entityType, reference.entityId) { mutableStateOf(false) }
    var dragWeight by remember(component.weight) { mutableFloatStateOf(component.weight) }
    Surface(
        shape = RoundedCornerShape(16.dp),
        color = MaterialTheme.colorScheme.surfaceContainerHigh,
        onClick = { if (showWeight) expanded = !expanded },
    ) {
        Column(modifier = Modifier.padding(10.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                ReferenceArtwork(reference, Modifier.size(48.dp), corner = 6.dp)
                Spacer(Modifier.width(12.dp))
                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        text = reference.label,
                        style = MaterialTheme.typography.titleSmall,
                        fontWeight = FontWeight.Bold,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                    val detail = buildString {
                        append(referenceKindLabel(reference))
                        component.similarity?.let {
                            append(" · ").append(stringResourceMatch(it))
                        }
                    }
                    MutedText(detail)
                }
                IconButton(onClick = { actions.removeDestinationComponent(reference) }) {
                    Icon(Icons.Default.Close, contentDescription = stringResource(R.string.steering_remove))
                }
            }
            if (showWeight) {
                LinearProgressIndicator(
                    progress = { share.coerceIn(0f, 1f) },
                    color = accentFor(reference),
                    trackColor = MaterialTheme.colorScheme.surfaceVariant,
                    strokeCap = StrokeCap.Round,
                    modifier = Modifier.fillMaxWidth().height(3.dp),
                )
            }
            AnimatedVisibility(visible = expanded && showWeight) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    MutedText(stringResource(R.string.steering_weight))
                    Slider(
                        value = dragWeight,
                        onValueChange = { dragWeight = it },
                        onValueChangeFinished = { actions.setDestinationComponentWeight(reference, dragWeight) },
                        valueRange = MIN_COMPONENT_WEIGHT..MAX_COMPONENT_WEIGHT,
                        colors = accentSliderColors(),
                        modifier = Modifier.weight(1f).padding(start = 12.dp),
                    )
                }
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

@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun AlongTheWaySection(state: SteeringScreenState, actions: SteeringScreenActions) {
    val knobs = state.knobs
    var showMore by rememberSaveable { mutableStateOf(false) }
    Column(verticalArrangement = Arrangement.spacedBy(20.dp)) {
        SectionHeader(stringResource(R.string.steering_knobs), stringResource(R.string.steering_knobs_help))

        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(
                text = stringResource(R.string.steering_away),
                style = MaterialTheme.typography.titleSmall,
                fontWeight = FontWeight.Bold,
            )
            MutedText(stringResource(R.string.steering_away_help))
            FlowRow(
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                knobs.away.forEach { reference ->
                    ReferencePill(reference, onRemove = { actions.removeAway(reference) })
                }
                Surface(
                    shape = CircleShape,
                    color = Color.Transparent,
                    modifier = Modifier.border(1.dp, MaterialTheme.colorScheme.outlineVariant, CircleShape),
                    onClick = { actions.openSearch(SteeringSearchTarget.Away) },
                ) {
                    Row(
                        modifier = Modifier.padding(horizontal = 14.dp, vertical = 8.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Icon(Icons.Default.Add, contentDescription = null, modifier = Modifier.size(16.dp))
                        Spacer(Modifier.width(6.dp))
                        Text(
                            text = stringResource(R.string.steering_add_away),
                            style = MaterialTheme.typography.labelLarge,
                        )
                    }
                }
            }
        }

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
            Column {
                val rotation by animateFloatAsState(if (showMore) 180f else 0f, label = "moreOptions")
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .clip(RoundedCornerShape(12.dp))
                        .clickable { showMore = !showMore }
                        .padding(vertical = 8.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Text(
                        text = stringResource(
                            if (showMore) R.string.steering_fewer_options else R.string.steering_more_options,
                        ),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.weight(1f),
                    )
                    Icon(
                        imageVector = Icons.Default.KeyboardArrowDown,
                        contentDescription = null,
                        tint = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.rotate(rotation),
                    )
                }
                AnimatedVisibility(visible = showMore) {
                    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                        Text(
                            text = stringResource(R.string.steering_listen_for),
                            style = MaterialTheme.typography.titleSmall,
                            fontWeight = FontWeight.Bold,
                        )
                        MutedText(stringResource(R.string.steering_listen_for_help))
                        knobs.criteria.forEach { criterion ->
                            CriterionSlider(
                                label = criterionLabel(criterion),
                                value = criterion.weight,
                                onCommit = { actions.setCriterionWeight(criterion.namespace, it) },
                            )
                        }
                    }
                }
            }
        }

        if (!knobs.isDefault) {
            TextButton(onClick = actions::resetKnobs, shape = CircleShape) {
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

@Composable
private fun accentSliderColors() = SliderDefaults.colors(
    thumbColor = MaterialTheme.colorScheme.primary,
    activeTrackColor = MaterialTheme.colorScheme.primary,
    inactiveTrackColor = MaterialTheme.colorScheme.surfaceVariant,
)

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
        Text(text = title, style = MaterialTheme.typography.titleSmall, fontWeight = FontWeight.Bold)
        MutedText(help)
        Slider(
            value = dragValue,
            onValueChange = { dragValue = it },
            onValueChangeFinished = { onCommit(dragValue) },
            valueRange = 0f..1f,
            colors = accentSliderColors(),
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
            style = MaterialTheme.typography.bodyMedium,
            modifier = Modifier.width(112.dp),
        )
        Slider(
            value = dragValue,
            onValueChange = { dragValue = it },
            onValueChangeFinished = { onCommit(dragValue) },
            valueRange = 0f..1f,
            colors = accentSliderColors(),
            modifier = Modifier.weight(1f),
        )
    }
}

/** A removable pill with the reference's artwork or colour swatch. */
@Composable
private fun ReferencePill(reference: SteeringReference, onRemove: () -> Unit) {
    Surface(
        shape = CircleShape,
        color = MaterialTheme.colorScheme.surfaceContainerHigh,
        onClick = onRemove,
    ) {
        Row(
            modifier = Modifier.padding(start = 4.dp, end = 10.dp, top = 4.dp, bottom = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            ReferenceArtwork(reference, Modifier.size(28.dp), shape = CircleShape)
            Spacer(Modifier.width(8.dp))
            Text(
                text = reference.label,
                style = MaterialTheme.typography.labelLarge,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.widthIn(max = MaxPillLabelWidth),
            )
            Spacer(Modifier.width(4.dp))
            Icon(
                Icons.Default.Close,
                contentDescription = stringResource(R.string.steering_remove),
                modifier = Modifier.size(16.dp),
            )
        }
    }
}

// Pills cap their label so a long album title cannot take a whole row.
private val MaxPillLabelWidth = 160.dp

// ---------------------------------------------------------------------------------------------
// Artwork and concept colours
// ---------------------------------------------------------------------------------------------

/**
 * Artwork for a reference: album/artist/track art (artists are round), or a coloured concept
 * tile. [showText] puts the concept's family and label on the tile, as in a genre grid.
 */
@Composable
private fun ReferenceArtwork(
    reference: SteeringReference,
    modifier: Modifier,
    corner: Dp = 8.dp,
    shape: Shape? = null,
    showText: Boolean = false,
) {
    val tileShape = shape ?: if (reference.entityType == "artist") CircleShape else RoundedCornerShape(corner)
    if (reference.entityType == CONCEPT) {
        ConceptTile(reference, modifier, tileShape, showText)
        return
    }
    val provider = LocalSteeringArtwork.current
    val flow = remember(reference.entityType, reference.entityId) { provider(reference) }
    val url by flow.collectAsState(initial = null)
    Box(modifier.clip(tileShape).background(MaterialTheme.colorScheme.surfaceVariant)) {
        NullablePezzottifyImage(
            url = url,
            shape = PezzottifyImageShape.FullSize,
            placeholder = if (reference.entityType == "artist") {
                PezzottifyImagePlaceholder.Head
            } else {
                PezzottifyImagePlaceholder.GenericImage
            },
        )
    }
}

@Composable
private fun ConceptTile(reference: SteeringReference, modifier: Modifier, shape: Shape, showText: Boolean) {
    Box(
        modifier = modifier.clip(shape).background(conceptColor(reference.entityId)),
        contentAlignment = if (showText) Alignment.BottomStart else Alignment.Center,
    ) {
        if (showText) {
            Column(modifier = Modifier.padding(12.dp)) {
                reference.family?.let { family ->
                    Text(
                        text = conceptFamilyLabel(family).uppercase(),
                        style = MaterialTheme.typography.labelSmall,
                        color = Color.White.copy(alpha = 0.8f),
                    )
                }
                Text(
                    text = reference.label,
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.Bold,
                    color = Color.White,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        } else {
            Text(
                text = reference.label.take(1).uppercase(),
                style = MaterialTheme.typography.titleMedium,
                fontWeight = FontWeight.Bold,
                color = Color.White,
            )
        }
    }
}

/** Saturated tile colours, picked deterministically per concept like a genre grid. */
private val ConceptPalette = listOf(
    Color(0xFFE13300), Color(0xFF1E3264), Color(0xFF8D67AB), Color(0xFF148A08),
    Color(0xFFBA5D07), Color(0xFFE8115B), Color(0xFF27856A), Color(0xFF503750),
    Color(0xFF477D95), Color(0xFF0D73EC), Color(0xFFAF2896), Color(0xFF7D4B32),
)

private fun conceptColor(conceptId: String): Color =
    ConceptPalette[conceptId.hashCode().absoluteValue % ConceptPalette.size]

@Composable
private fun accentFor(reference: SteeringReference): Color =
    if (reference.entityType == CONCEPT) conceptColor(reference.entityId) else MaterialTheme.colorScheme.primary

// ---------------------------------------------------------------------------------------------
// Search sheet
// ---------------------------------------------------------------------------------------------

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun ReferenceSearchSheet(search: SteeringSearch, actions: SteeringScreenActions) {
    val sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true)
    ModalBottomSheet(onDismissRequest = actions::closeSearch, sheetState = sheetState) {
        SearchSheetContent(search, actions)
    }
}

@Composable
private fun SearchSheetContent(search: SteeringSearch, actions: SteeringScreenActions) {
    Column(modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp)) {
        TextField(
            value = search.query,
            onValueChange = actions::updateSearchQuery,
            placeholder = { Text(stringResource(R.string.steering_search_hint)) },
            leadingIcon = { Icon(Icons.Default.Search, contentDescription = null) },
            singleLine = true,
            shape = CircleShape,
            colors = TextFieldDefaults.colors(
                focusedIndicatorColor = Color.Transparent,
                unfocusedIndicatorColor = Color.Transparent,
                disabledIndicatorColor = Color.Transparent,
            ),
            modifier = Modifier.fillMaxWidth(),
        )
        Spacer(Modifier.height(12.dp))
        when {
            search.isError -> SheetMessage(stringResource(R.string.steering_search_error))
            !search.isSearching && !search.isQueryTooShort && search.query.isNotBlank() &&
                search.results.isEmpty() && search.concepts.isEmpty() && !search.isLoadingConcepts ->
                SheetMessage(stringResource(R.string.steering_search_empty))

            else -> LazyColumn(
                modifier = Modifier.height(480.dp),
                verticalArrangement = Arrangement.spacedBy(4.dp),
            ) {
                if (search.isQueryTooShort) {
                    item(key = "min-chars") { SheetMessage(stringResource(R.string.steering_search_min_chars)) }
                }
                if (search.results.isNotEmpty() || search.isSearching) {
                    item(key = "catalog-header") { SheetHeading(stringResource(R.string.steering_search_catalog)) }
                }
                // Results stream in (best matches first) while the search is still running.
                items(search.results, key = { "${it.entityType}:${it.entityId}" }) { reference ->
                    SearchResultRow(reference, actions)
                }
                if (search.isSearching) {
                    item(key = "searching") {
                        Box(Modifier.fillMaxWidth().padding(16.dp), contentAlignment = Alignment.Center) {
                            CircularProgressIndicator()
                        }
                    }
                }
                if (search.isLoadingConcepts) {
                    item(key = "concepts-loading") { SheetMessage(stringResource(R.string.steering_concepts_loading)) }
                }
                if (search.concepts.isNotEmpty()) {
                    item(key = "concepts-header") { SheetHeading(stringResource(R.string.steering_search_concepts)) }
                }
                search.concepts.forEach { group ->
                    item(key = "family-${group.family}") {
                        Text(
                            text = conceptFamilyLabel(group.family),
                            style = MaterialTheme.typography.titleSmall,
                            fontWeight = FontWeight.Bold,
                            modifier = Modifier.padding(top = 12.dp, bottom = 4.dp),
                        )
                    }
                    items(group.concepts.chunked(2), key = { row -> "concepts:" + row.joinToString { it.entityId } }) { row ->
                        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                            row.forEach { reference ->
                                ConceptTile(
                                    reference = reference,
                                    modifier = Modifier
                                        .weight(1f)
                                        .aspectRatio(1.8f)
                                        .clickable { actions.pickSearchResult(reference) },
                                    shape = RoundedCornerShape(8.dp),
                                    showText = true,
                                )
                            }
                            if (row.size == 1) Spacer(Modifier.weight(1f))
                        }
                    }
                }
            }
        }
        Spacer(Modifier.height(24.dp))
    }
}

@Composable
private fun SheetHeading(text: String) {
    Text(
        text = text,
        style = MaterialTheme.typography.titleLarge,
        fontWeight = FontWeight.Bold,
        modifier = Modifier.padding(top = 8.dp, bottom = 4.dp),
    )
}

@Composable
private fun SearchResultRow(reference: SteeringReference, actions: SteeringScreenActions) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(8.dp))
            .clickable { actions.pickSearchResult(reference) }
            .padding(vertical = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        ReferenceArtwork(reference, Modifier.size(52.dp), corner = 4.dp)
        Spacer(Modifier.width(12.dp))
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = reference.label,
                style = MaterialTheme.typography.bodyLarge,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            MutedText(listOfNotNull(referenceKindLabel(reference), reference.detail).joinToString(" · "))
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

// ---------------------------------------------------------------------------------------------
// Previews
// ---------------------------------------------------------------------------------------------

private object PreviewActions : SteeringScreenActions {
    override fun setSmartContinuationEnabled(enabled: Boolean) = Unit
    override fun setStepsRemaining(remaining: Int) = Unit
    override fun clearDestination() = Unit
    override fun setDestinationComponentWeight(reference: SteeringReference, weight: Float) = Unit
    override fun removeDestinationComponent(reference: SteeringReference) = Unit
    override fun resetSourceToQueue() = Unit
    override fun removeSourceReference(reference: SteeringReference) = Unit
    override fun setRecencyWeight(value: Float) = Unit
    override fun setVariety(value: Float) = Unit
    override fun setCriterionWeight(namespace: String, weight: Float) = Unit
    override fun removeAway(reference: SteeringReference) = Unit
    override fun resetKnobs() = Unit
    override fun openSearch(target: SteeringSearchTarget) = Unit
    override fun updateSearchQuery(query: String) = Unit
    override fun pickSearchResult(reference: SteeringReference) = Unit
    override fun closeSearch() = Unit
}

private val PreviewCriteria = listOf(
    SteeringCriterion("musicfm.mean.v1", "Sound profile", SteeringCriterionKind.OverallSound, 1f),
    SteeringCriterion("ast.audioset.v2", "Audio scene", SteeringCriterionKind.AudioScene, 0f),
)

private val PreviewNotSteering = SteeringScreenState(
    availability = SteeringAvailability.Ready,
    smartContinuationEnabled = true,
    source = SteeringSource.Queue(userChosenCount = 12, suggestedCount = 4),
    knobs = SteeringKnobs(criteria = PreviewCriteria),
)

private val PreviewMix = PreviewNotSteering.copy(
    destination = SteeringDestination(
        components = listOf(
            SteeringDestinationComponent(
                SteeringReference("concept", "audioset:Jazz", "Jazz", "sound_genre"),
                weight = 1f,
                similarity = 0.62f,
            ),
            SteeringDestinationComponent(
                SteeringReference("concept", "recorded:1960s", "Recorded in the 1960s", "recorded"),
                weight = 0.6f,
                similarity = 0.41f,
            ),
            SteeringDestinationComponent(
                SteeringReference("artist", "artist-1", "Miles Davis"),
                weight = 0.4f,
                similarity = null,
            ),
        ),
        progress = 0.4f,
        queryToDestination = 0.55f,
        sourceToDestination = 0.2f,
    ),
    stepsTotal = 20,
    stepsDone = 8,
    knobs = SteeringKnobs(
        criteria = PreviewCriteria,
        away = listOf(SteeringReference("concept", "audioset:Heavy metal", "Heavy metal", "sound_genre")),
        isDefault = false,
    ),
)

private val PreviewConceptSearch = SteeringSearch(
    target = SteeringSearchTarget.Destination,
    concepts = listOf(
        SteeringConceptGroup(
            "sound_genre",
            listOf("Jazz", "Reggae", "Classical music", "Techno", "Blues").map {
                SteeringReference("concept", "audioset:$it", it, "sound_genre")
            },
        ),
        SteeringConceptGroup(
            "instrument",
            listOf("Piano", "Saxophone", "Violin, fiddle").map {
                SteeringReference("concept", "audioset:$it", it, "instrument")
            },
        ),
    ),
)

@Preview(showBackground = true, heightDp = 1400)
@Composable
private fun SteeringNotSteeringPreview() {
    MaterialTheme(colorScheme = darkColorScheme()) {
        SteeringScreenContent(state = PreviewNotSteering, actions = PreviewActions, onBack = {})
    }
}

@Preview(showBackground = true, heightDp = 1800)
@Composable
private fun SteeringMixPreview() {
    MaterialTheme(colorScheme = darkColorScheme()) {
        SteeringScreenContent(state = PreviewMix, actions = PreviewActions, onBack = {})
    }
}

@Preview(showBackground = true, heightDp = 640)
@Composable
private fun SteeringConceptSearchPreview() {
    MaterialTheme(colorScheme = darkColorScheme()) {
        Surface(color = MaterialTheme.colorScheme.surface) {
            SearchSheetContent(search = PreviewConceptSearch, actions = PreviewActions)
        }
    }
}
