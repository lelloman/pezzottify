package com.lelloman.pezzottify.android.ui.screen.main.profile

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.lelloman.pezzottify.android.ui.model.Permission
import dagger.hilt.android.lifecycle.HiltViewModel
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import javax.inject.Inject

@HiltViewModel
class ProfileScreenViewModel @Inject constructor(
    private val interactor: Interactor,
) : ViewModel(), ProfileScreenActions {

    private val mutableState = MutableStateFlow(ProfileScreenState())
    val state: StateFlow<ProfileScreenState> = mutableState.asStateFlow()

    private val mutableEvents = MutableSharedFlow<ProfileScreenEvents>()
    val events: SharedFlow<ProfileScreenEvents> = mutableEvents.asSharedFlow()

    init {
        viewModelScope.launch {
            val initialState = ProfileScreenState(
                userName = interactor.getUserName(),
                baseUrl = interactor.getBaseUrl(),
                buildVariant = interactor.getBuildVariant(),
                versionName = interactor.getVersionName(),
                gitCommit = interactor.getGitCommit(),
            )
            mutableState.value = initialState
        }
        viewModelScope.launch {
            interactor.observeServerVersion().collect { serverVersion ->
                mutableState.update { it.copy(serverVersion = serverVersion) }
            }
        }
        viewModelScope.launch {
            interactor.observePermissions().collect { permissions ->
                mutableState.update { it.copy(permissions = permissions) }
            }
        }
    }

    override fun clickOnLogout() {
        if (!mutableState.value.isLoggingOut) {
            mutableState.update { it.copy(showLogoutConfirmation = true) }
        }
    }

    override fun confirmLogout() {
        if (!mutableState.value.isLoggingOut) {
            mutableState.update { it.copy(showLogoutConfirmation = false, isLoggingOut = true) }
            viewModelScope.launch {
                interactor.logout()
                mutableState.update { it.copy(isLoggingOut = false) }
                mutableEvents.emit(ProfileScreenEvents.NavigateToLoginScreen)
            }
        }
    }

    override fun dismissLogoutConfirmation() {
        mutableState.update { it.copy(showLogoutConfirmation = false) }
    }

    override fun onPermissionClicked(permission: Permission) {
        mutableState.update { it.copy(selectedPermission = permission) }
    }

    override fun onPermissionDialogDismissed() {
        mutableState.update { it.copy(selectedPermission = null) }
    }

    interface Interactor {
        suspend fun logout()
        fun getUserName(): String
        fun getBaseUrl(): String
        fun getBuildVariant(): String
        fun getVersionName(): String
        fun getGitCommit(): String
        fun observeServerVersion(): Flow<String>
        fun observePermissions(): Flow<Set<Permission>>
    }
}
