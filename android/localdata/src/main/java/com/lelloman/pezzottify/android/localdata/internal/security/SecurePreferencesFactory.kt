package com.lelloman.pezzottify.android.localdata.internal.security

import android.content.Context
import android.content.SharedPreferences
import androidx.security.crypto.EncryptedSharedPreferences
import androidx.security.crypto.MasterKey
import com.lelloman.pezzottify.android.logger.Logger

/**
 * Creates the client's secret [SharedPreferences]: [EncryptedSharedPreferences] with a Keystore
 * master key, or, on a device whose Keystore is unusable (see [KeystoreFallback] for when and for
 * the security posture), a plain private preferences file named `<name>_plain`.
 *
 * - Keystore works: the encrypted `<name>` file is used and its data kept. When it cannot be opened
 *   although the Keystore works (its keyset and the master key are out of sync, e.g. after a
 *   reinstall restored the file), [recoverEncrypted] clears it and it is recreated, as before.
 * - Keystore unusable: `<name>_plain` is used from then on, at every start, without touching the
 *   Keystore again. The old encrypted file, unreadable anyway, is left alone.
 */
internal class SecurePreferencesFactory(
    private val context: Context,
    private val keystore: KeystoreFallback,
    private val logger: Logger,
    private val createEncrypted: (String) -> SharedPreferences = { name ->
        encryptedPreferences(context, name, MasterKey.DEFAULT_MASTER_KEY_ALIAS)
    },
    private val recoverEncrypted: (String) -> Unit = { name -> context.deleteSharedPreferences(name) },
) {

    fun create(name: String): SharedPreferences {
        if (keystore.usePlain()) return plain(name)
        return try {
            createEncrypted(name)
        } catch (first: Exception) {
            if (keystore.confirmUnusable(first)) return plain(name)
            logger.warn("Encrypted preferences $name are unreadable; recreating them", first)
            recoverEncrypted(name)
            try {
                createEncrypted(name)
            } catch (second: Exception) {
                // The probe passed but the encrypted store still cannot be created: never crash.
                if (second !== first) second.addSuppressed(first)
                keystore.markUnusable(second)
                plain(name)
            }
        }
    }

    private fun plain(name: String): SharedPreferences =
        context.getSharedPreferences(plainName(name), Context.MODE_PRIVATE)

    companion object {
        fun plainName(name: String) = "${name}_plain"

        fun encryptedPreferences(context: Context, name: String, masterKeyAlias: String): SharedPreferences {
            val masterKey = MasterKey.Builder(context, masterKeyAlias)
                .setKeyScheme(MasterKey.KeyScheme.AES256_GCM)
                .build()
            return EncryptedSharedPreferences.create(
                context,
                name,
                masterKey,
                EncryptedSharedPreferences.PrefKeyEncryptionScheme.AES256_SIV,
                EncryptedSharedPreferences.PrefValueEncryptionScheme.AES256_GCM,
            )
        }
    }
}
