package com.lelloman.pezzottify.android.ui.screen.main.settings

import android.Manifest
import android.os.Build
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.*
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.unit.dp
import androidx.hilt.navigation.compose.hiltViewModel
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.ViewModel
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.navigation.NavController
import com.lelloman.pezzottify.android.domain.equalizer.*
import com.lelloman.pezzottify.android.ui.R
import com.lelloman.pezzottify.android.ui.screen.main.MainScreenScaffold
import dagger.hilt.android.lifecycle.HiltViewModel
import javax.inject.Inject

@HiltViewModel
class EqualizerProfilesViewModel @Inject constructor(val store: EqualizerStore,
    val outputs: EqualizerOutputController) : ViewModel()

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun EqualizerProfilesScreen(navController: NavController) {
    val vm = hiltViewModel<EqualizerProfilesViewModel>()
    val state by vm.store.state.collectAsStateWithLifecycle()
    val output by vm.outputs.output.collectAsStateWithLifecycle()
    val lifecycle = LocalLifecycleOwner.current.lifecycle
    val permission = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { vm.outputs.refresh() }
    DisposableEffect(lifecycle, vm) {
        val observer = LifecycleEventObserver { _, event -> if (event == Lifecycle.Event.ON_RESUME) vm.outputs.refresh() }
        lifecycle.addObserver(observer)
        vm.outputs.refresh()
        onDispose { lifecycle.removeObserver(observer) }
    }
    MainScreenScaffold(topBar = {
        TopAppBar(title = { Text(stringResource(R.string.equalizer_profiles)) }, navigationIcon = {
            IconButton(onClick = { navController.popBackStack() }) {
                Icon(Icons.AutoMirrored.Filled.ArrowBack, stringResource(R.string.back))
            }
        })
    }) { padding ->
        EqualizerProfilesContent(state, vm.store, output, vm.outputs::associateCurrentOutput,
            onRequestPermission = { if (Build.VERSION.SDK_INT >= 31) permission.launch(Manifest.permission.BLUETOOTH_CONNECT) },
            modifier = Modifier.padding(padding))
    }
}

@Composable
internal fun EqualizerProfilesContent(state: EqualizerSettings, actions: EqualizerStore, output: EqualizerOutput?,
    onAssociate: (String, String) -> Boolean, onRequestPermission: () -> Unit = {}, modifier: Modifier = Modifier) {
    var routeChanged by remember(output?.key) { mutableStateOf(false) }
    EqualizerContent(state, actions, modifier, profilesOnly = true, profileHeader = {
        Card {
            Column(Modifier.fillMaxWidth().padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(stringResource(R.string.equalizer_current_output), style = MaterialTheme.typography.titleMedium)
                Text(output?.name ?: stringResource(R.string.equalizer_start_playback))
                Text(stringResource(R.string.equalizer_auto_description), style = MaterialTheme.typography.bodySmall)
                if (output?.needsBluetoothPermission == true) {
                    Text(stringResource(R.string.equalizer_bluetooth_permission), style = MaterialTheme.typography.bodySmall)
                    OutlinedButton(onClick = onRequestPermission) { Text(stringResource(R.string.equalizer_allow_bluetooth)) }
                } else if (output != null && output.key == null) {
                    Text(stringResource(R.string.equalizer_unidentified_output), style = MaterialTheme.typography.bodySmall)
                }
                if (routeChanged) Text(stringResource(R.string.equalizer_output_changed), color = MaterialTheme.colorScheme.error)
            }
        }
    }, profileFooter = { profile ->
        val alreadyAssigned = state.outputAssociations.any { it.outputKey == output?.key && it.profileId == profile.id }
        OutlinedButton(enabled = output?.key != null && !alreadyAssigned, modifier = Modifier.fillMaxWidth(), onClick = {
            val key = output?.key
            if (key != null) routeChanged = !onAssociate(profile.id, key)
        }) {
            Text(stringResource(if (alreadyAssigned) R.string.equalizer_associated else R.string.equalizer_associate,
                output?.name ?: stringResource(R.string.equalizer_current_output)))
        }
        state.outputAssociations.filter { it.profileId == profile.id }.forEach { association ->
            TextButton(onClick = { actions.removeOutputAssociation(association.outputKey) }) {
                Text(stringResource(R.string.equalizer_remove_association, association.outputName))
            }
        }
    })
}
