package com.lelloman.pezzottify.android.localdata.internal.auth

import android.content.Context
import android.content.SharedPreferences
import android.util.Log
import androidx.security.crypto.EncryptedSharedPreferences
import androidx.security.crypto.MasterKey
import com.lelloman.pezzottify.android.domain.auth.AuthState
import com.lelloman.pezzottify.android.domain.auth.AuthStore
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import androidx.core.content.edit
import java.io.File
import java.security.KeyStore

internal class AuthStoreImpl(
    context: Context,
    private val coroutineScope: CoroutineScope,
    private val dispatcher: CoroutineDispatcher = Dispatchers.IO,
) : AuthStore {

    private var initialized = false

    private val sharedPrefs: SharedPreferences = createEncryptedSharedPreferences(context)

    private val mutableAuthStateFlow =
        MutableStateFlow<AuthState>(
            AuthState.Loading
        )

    override fun getAuthState(): StateFlow<AuthState> =
        mutableAuthStateFlow.asStateFlow()

    override suspend fun storeAuthState(newAuthState: AuthState): Result<Unit> =
        withContext(dispatcher) {
            try {
                mutableAuthStateFlow.value = newAuthState
                when (newAuthState) {
                    is AuthState.LoggedIn -> sharedPrefs.edit()
                        .putString(KEY_AUTH_STATE, Json.encodeToString(newAuthState))
                        .apply()

                    is AuthState.LoggedOut -> sharedPrefs.edit()
                        .putString(
                            KEY_AUTH_STATE, null
                        )
                        .apply()

                    is AuthState.Loading -> Unit
                }
                Result.success(Unit)
            } catch (throwable: Throwable) {
                Log.e("AuthStore", "Error storing auth state", throwable)
                Result.failure(throwable)
            }
        }

    override fun initialize() {
        coroutineScope.launch {
            withContext(dispatcher) {
                if (!initialized) {
                    val state = try {
                        val jsonState = sharedPrefs.getString(KEY_AUTH_STATE, null)
                        if (jsonState != null) {
                            Json.decodeFromString<AuthState.LoggedIn>(
                                jsonState
                            )
                        } else {
                            AuthState.LoggedOut
                        }
                    } catch (e: Exception) {
                        Log.w("AuthStore", "Error reading auth state, defaulting to LoggedOut", e)
                        AuthState.LoggedOut
                    } finally {
                        initialized = true
                    }
                    mutableAuthStateFlow.value = state
                }
            }
        }
    }

    override fun getLastUsedHandle(): String? =
        sharedPrefs.getString(KEY_LAST_USED_HANDLE, null)

    override suspend fun storeLastUsedHandle(handle: String) {
        withContext(dispatcher) {
            sharedPrefs.edit {
                putString(KEY_LAST_USED_HANDLE, handle)
            }
        }
    }

    internal companion object {
        const val SHARED_PREF_FILE_NAME = "AuthStore"
        private const val KEY_AUTH_STATE = "AuthState"
        private const val KEY_LAST_USED_HANDLE = "LastUsedHandle"
        private const val MASTER_KEY_ALIAS = "AuthStoreMasterKeyAlias"

        private fun createEncryptedSharedPreferences(context: Context): SharedPreferences {
            return try {
                createEncryptedPrefsInternal(context)
            } catch (e: Exception) {
                Log.w("AuthStore", "Failed to create encrypted prefs, clearing corrupted key", e)
                clearCorruptedKeyAndPrefs(context)
                createEncryptedPrefsInternal(context)
            }
        }

        private fun createEncryptedPrefsInternal(context: Context): SharedPreferences {
            val masterKey = MasterKey.Builder(context, MASTER_KEY_ALIAS)
                .setKeyScheme(MasterKey.KeyScheme.AES256_GCM)
                .build()

            return EncryptedSharedPreferences.create(
                context,
                SHARED_PREF_FILE_NAME,
                masterKey,
                EncryptedSharedPreferences.PrefKeyEncryptionScheme.AES256_SIV,
                EncryptedSharedPreferences.PrefValueEncryptionScheme.AES256_GCM,
            )
        }

        private fun clearCorruptedKeyAndPrefs(context: Context) {
            try {
                val keyStore = KeyStore.getInstance("AndroidKeyStore")
                keyStore.load(null)
                if (keyStore.containsAlias(MASTER_KEY_ALIAS)) {
                    keyStore.deleteEntry(MASTER_KEY_ALIAS)
                    Log.i("AuthStore", "Deleted corrupted keystore entry")
                }
            } catch (e: Exception) {
                Log.e("AuthStore", "Failed to delete keystore entry", e)
            }

            try {
                val prefsFile = File(context.filesDir.parent, "shared_prefs/$SHARED_PREF_FILE_NAME.xml")
                if (prefsFile.exists()) {
                    prefsFile.delete()
                    Log.i("AuthStore", "Deleted corrupted shared prefs file")
                }
            } catch (e: Exception) {
                Log.e("AuthStore", "Failed to delete prefs file", e)
            }
        }
    }
}