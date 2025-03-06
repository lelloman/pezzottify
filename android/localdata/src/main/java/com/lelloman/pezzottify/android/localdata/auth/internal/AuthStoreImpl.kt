package com.lelloman.pezzottify.android.localdata.auth.internal

import android.content.Context
import androidx.security.crypto.EncryptedSharedPreferences
import com.lelloman.pezzottify.android.localdata.auth.AuthState
import com.lelloman.pezzottify.android.localdata.auth.AuthStore
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.withContext
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json

internal class AuthStoreImpl(
    context: Context,
    private val dispatcher: CoroutineDispatcher = Dispatchers.IO,
) : AuthStore {

    private var initialized = false

    private val sharedPrefs = EncryptedSharedPreferences.create(
        SHARED_PREF_FILE_NAME,
        "AuthStoreMasterKeyAlias",
        context,
        EncryptedSharedPreferences.PrefKeyEncryptionScheme.AES256_SIV,
        EncryptedSharedPreferences.PrefValueEncryptionScheme.AES256_GCM,
    )

    private val mutableAuthStateFlow = MutableStateFlow<AuthState>(AuthState.Loading)

    override fun getAuthState(): StateFlow<AuthState> = mutableAuthStateFlow.asStateFlow()

    override suspend fun storeAuthState(newAuthState: AuthState): Result<Unit> =
        withContext(dispatcher) {
            try {
                mutableAuthStateFlow.value = newAuthState
                when (newAuthState) {
                    is AuthState.LoggedIn -> sharedPrefs.edit()
                        .putString(KEY_AUTH_STATE, Json.encodeToString(newAuthState))
                        .apply()

                    is AuthState.LoggedOut -> sharedPrefs.edit().putString(KEY_AUTH_STATE, null)
                        .apply()

                    is AuthState.Loading -> Unit
                }
                Result.success(Unit)
            } catch (throwable: Throwable) {
                Result.failure(throwable)
            }
        }

    suspend fun initialize() {
        withContext(dispatcher) {
            if (!initialized) {
                val state = try {
                    val jsonState = sharedPrefs.getString(KEY_AUTH_STATE, null)
                    if (jsonState != null) {
                        Json.decodeFromString<AuthState.LoggedIn>(jsonState)
                    } else {
                        AuthState.LoggedOut
                    }
                } catch (e: Exception) {
                    e.printStackTrace()
                    AuthState.LoggedOut
                } finally {
                    initialized = true
                }
                mutableAuthStateFlow.value = state
            }
        }
    }

    internal companion object {
        const val SHARED_PREF_FILE_NAME = "AuthStore"
        private const val KEY_AUTH_STATE = "AuthState"
    }
}