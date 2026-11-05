package com.lelloman.pezzottify.android.localdata.internal.auth

import android.content.Context
import android.content.SharedPreferences
import com.lelloman.pezzottify.android.domain.auth.AuthState
import com.lelloman.pezzottify.android.domain.auth.AuthStore
import com.lelloman.pezzottify.android.localdata.internal.security.KeystoreFallback
import com.lelloman.pezzottify.android.localdata.internal.security.SecurePreferencesFactory
import com.lelloman.pezzottify.android.logger.Logger
import com.lelloman.pezzottify.android.logger.LoggerFactory
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
    loggerFactory: LoggerFactory,
    private val dispatcher: CoroutineDispatcher = Dispatchers.IO,
    keystoreFallback: KeystoreFallback = KeystoreFallback.forApp(context, loggerFactory),
) : AuthStore {

    private val logger: Logger = loggerFactory.getLogger(AuthStoreImpl::class)

    private var initialized = false

    // Keystore-encrypted, or plain app-private where the Keystore is unusable (see KeystoreFallback).
    private val sharedPrefs: SharedPreferences = SecurePreferencesFactory(
        context = context,
        keystore = keystoreFallback,
        logger = logger,
        createEncrypted = { name ->
            SecurePreferencesFactory.encryptedPreferences(context, name, MASTER_KEY_ALIAS)
        },
        recoverEncrypted = { name -> clearCorruptedKeyAndPrefs(context, name, logger) },
    ).create(SHARED_PREF_FILE_NAME)

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
                logger.error("Error storing auth state", throwable)
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
                        logger.warn("Error reading auth state, defaulting to LoggedOut", e)
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

        // Only called once the Keystore probe passed: the encrypted file and its master key are out
        // of sync (e.g. after a reinstall restored the file), so both are dropped and recreated.
        private fun clearCorruptedKeyAndPrefs(context: Context, name: String, logger: Logger) {
            try {
                val keyStore = KeyStore.getInstance("AndroidKeyStore")
                keyStore.load(null)
                if (keyStore.containsAlias(MASTER_KEY_ALIAS)) {
                    keyStore.deleteEntry(MASTER_KEY_ALIAS)
                    logger.info("Deleted corrupted keystore entry")
                }
            } catch (e: Exception) {
                logger.error("Failed to delete keystore entry", e)
            }

            val sharedPrefsDir = File(context.filesDir.parent, "shared_prefs")
            val filesToDelete = listOf(
                "$name.xml",
                "__androidx_security_crypto_encrypted_prefs__.xml"
            )

            for (fileName in filesToDelete) {
                try {
                    val file = File(sharedPrefsDir, fileName)
                    if (file.exists()) {
                        file.delete()
                        logger.info("Deleted corrupted file: $fileName")
                    }
                } catch (e: Exception) {
                    logger.error("Failed to delete file: $fileName", e)
                }
            }
        }
    }
}