package com.lelloman.pezzottify.android.ui.screen.main.settings

import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.ViewModel
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.navigation.NavController
import com.lelloman.pezzottify.android.domain.equalizer.*
import com.lelloman.pezzottify.android.ui.R
import dagger.hilt.android.lifecycle.HiltViewModel
import javax.inject.Inject
import kotlin.math.round

@HiltViewModel
class EqualizerViewModel @Inject constructor(val store: EqualizerStore) : ViewModel()

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun EqualizerScreen(navController: NavController) {
    val viewModel = hiltViewModel<EqualizerViewModel>()
    val state by viewModel.store.state.collectAsStateWithLifecycle()
    Scaffold(topBar = {
        TopAppBar(title = { Text(stringResource(R.string.equalizer_title)) }, navigationIcon = {
            IconButton(onClick = { navController.popBackStack() }) {
                Icon(Icons.AutoMirrored.Filled.ArrowBack, stringResource(R.string.back))
            }
        })
    }) { padding ->
        EqualizerContent(state, viewModel.store, Modifier.padding(padding))
    }
}

@Composable
internal fun EqualizerContent(state: EqualizerSettings, actions: EqualizerStore, modifier: Modifier = Modifier) {
    var dialog by rememberSaveable { mutableStateOf<String?>(null) }
    var profileId by rememberSaveable { mutableStateOf<String?>(null) }
    var name by rememberSaveable { mutableStateOf("") }
    val selected = state.selectedProfile

    Column(modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(20.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp)) {
        Card {
            Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Column(Modifier.weight(1f)) {
                        Text(stringResource(R.string.equalizer_title), style = MaterialTheme.typography.titleMedium)
                        Text(stringResource(if (state.enabled) R.string.equalizer_on else R.string.equalizer_off),
                            style = MaterialTheme.typography.bodySmall)
                    }
                    val label = stringResource(R.string.equalizer_title)
                    Switch(state.enabled, actions::setEnabled, Modifier.semantics { contentDescription = label })
                }
                Text(stringResource(R.string.equalizer_description), style = MaterialTheme.typography.bodySmall)
            }
        }

        Text(selected?.let { if (state.isModified) stringResource(R.string.equalizer_modified, it.name) else it.name }
            ?: stringResource(R.string.equalizer_custom), style = MaterialTheme.typography.titleLarge)
        Text(stringResource(R.string.equalizer_headroom), style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant)
        Card {
            Column(Modifier.padding(16.dp)) {
                EqualizerBands.frequencies.forEachIndexed { index, hz ->
                    val frequency = if (hz < 1000) "$hz Hz" else "${hz / 1000f} kHz"
                    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                        Text(frequency)
                        Text(stringResource(R.string.equalizer_gain, state.gains[index]))
                    }
                    Slider(value = state.gains[index], onValueChange = { actions.setBand(index, round(it * 2) / 2) },
                        valueRange = EqualizerBands.MIN_DB..EqualizerBands.MAX_DB, steps = 47,
                        modifier = Modifier.semantics { contentDescription = frequency })
                }
                TextButton(onClick = actions::reset) { Text(stringResource(R.string.equalizer_flat)) }
            }
        }

        if (selected != null) {
            Button(onClick = actions::updateProfile, enabled = state.isModified, modifier = Modifier.fillMaxWidth()) {
                Text(stringResource(R.string.equalizer_update, selected.name))
            }
        }
        OutlinedButton(onClick = { name = ""; profileId = null; dialog = "save" },
            enabled = state.profiles.size < 50, modifier = Modifier.fillMaxWidth()) {
            Text(stringResource(R.string.equalizer_save_as))
        }
        Text(stringResource(R.string.equalizer_profiles), style = MaterialTheme.typography.titleMedium)
        if (state.profiles.isEmpty()) Text(stringResource(R.string.equalizer_no_profiles),
            color = MaterialTheme.colorScheme.onSurfaceVariant)
        state.profiles.forEach { profile ->
            Card {
                Column(Modifier.fillMaxWidth().padding(12.dp)) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        RadioButton(selected = profile.id == state.selectedProfileId,
                            onClick = { actions.selectProfile(profile.id) },
                            modifier = Modifier.semantics { contentDescription = profile.name })
                        TextButton(onClick = { actions.selectProfile(profile.id) }, modifier = Modifier.weight(1f)) {
                            Text(profile.name)
                        }
                    }
                    Row {
                        TextButton(onClick = { profileId = profile.id; name = profile.name; dialog = "rename" }) {
                            Text(stringResource(R.string.equalizer_rename))
                        }
                        TextButton(onClick = { profileId = profile.id; name = profile.name; dialog = "delete" }) {
                            Text(stringResource(R.string.equalizer_delete))
                        }
                    }
                }
            }
        }
        Text(stringResource(R.string.equalizer_local_profiles), style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant)
    }

    if (dialog == "save" || dialog == "rename") {
        val duplicate = state.profiles.any { it.id != profileId && it.name.equals(name.trim(), true) }
        val valid = name.trim().isNotEmpty() && name.trim().length <= 40 && !duplicate
        AlertDialog(onDismissRequest = { dialog = null },
            title = { Text(stringResource(if (dialog == "save") R.string.equalizer_save_as else R.string.equalizer_rename)) },
            text = {
                OutlinedTextField(value = name, onValueChange = { name = it.take(40) }, singleLine = true,
                    label = { Text(stringResource(R.string.equalizer_profile_name)) },
                    placeholder = { Text(stringResource(R.string.equalizer_profile_example)) },
                    isError = duplicate,
                    supportingText = { if (duplicate) Text(stringResource(R.string.equalizer_duplicate)) })
            },
            confirmButton = { TextButton(enabled = valid, onClick = {
                if (dialog == "save") actions.saveProfile(name) else profileId?.let { actions.renameProfile(it, name) }
                dialog = null
            }) { Text(stringResource(R.string.equalizer_save)) } },
            dismissButton = { TextButton(onClick = { dialog = null }) { Text(stringResource(R.string.cancel)) } })
    }
    if (dialog == "delete") {
        AlertDialog(onDismissRequest = { dialog = null },
            title = { Text(stringResource(R.string.equalizer_delete)) },
            text = { Text(stringResource(R.string.equalizer_delete_confirmation, name)) },
            confirmButton = { TextButton(onClick = { profileId?.let(actions::deleteProfile); dialog = null }) {
                Text(stringResource(R.string.equalizer_delete))
            } },
            dismissButton = { TextButton(onClick = { dialog = null }) { Text(stringResource(R.string.cancel)) } })
    }
}
