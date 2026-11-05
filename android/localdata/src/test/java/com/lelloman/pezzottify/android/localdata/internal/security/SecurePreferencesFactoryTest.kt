package com.lelloman.pezzottify.android.localdata.internal.security

import android.content.Context
import android.content.SharedPreferences
import androidx.core.content.edit
import org.robolectric.RuntimeEnvironment
import com.google.common.truth.Truth.assertThat
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config
import java.security.KeyStoreException
import java.security.ProviderException

// The Android Keystore does not exist under Robolectric: the encrypted store and the probe are fakes.
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class SecurePreferencesFactoryTest {

    @get:Rule val folder = TemporaryFolder()

    private val context: Context = RuntimeEnvironment.getApplication()
    private val logger = WarningLogger()
    private val marker get() = folder.root.resolve(KeystoreFallback.MARKER_FILE_NAME)
    private var probeFailure: Exception? = null
    private var probes = 0
    private val encryptedCreations = mutableListOf<String>()
    private val recoveries = mutableListOf<String>()

    /** Stands in for EncryptedSharedPreferences: a distinct file, or the given failure. */
    private fun factory(failure: () -> Exception? = { null }) = SecurePreferencesFactory(
        context,
        KeystoreFallback(marker, logger, probe = { probes++; probeFailure?.let { throw it } }),
        logger,
        createEncrypted = { name ->
            encryptedCreations += name
            failure()?.let { throw it }
            context.getSharedPreferences("encrypted:$name", Context.MODE_PRIVATE)
        },
        recoverEncrypted = { name -> recoveries += name },
    )

    private fun SharedPreferences.isFile(name: String) = this === context.getSharedPreferences(name, Context.MODE_PRIVATE)

    @Test
    fun `a working encrypted store is used and keeps its data`() {
        factory().create(NAME).edit(commit = true) { putString("auth_state", "session") }

        val prefs = factory().create(NAME)

        assertThat(prefs.isFile("encrypted:$NAME")).isTrue()
        assertThat(prefs.getString("auth_state", null)).isEqualTo("session")
        assertThat(probes).isEqualTo(0)
        assertThat(marker.exists()).isFalse()
        assertThat(logger.warnings).isEmpty()
    }

    @Test
    fun `a broken keystore falls back to plain preferences and warns`() {
        probeFailure = ProviderException("Keystore operation failed")

        val prefs = factory { ProviderException("Keystore operation failed") }.create(NAME)

        assertThat(prefs.isFile("${NAME}_plain")).isTrue()
        assertThat(marker.exists()).isTrue()
        assertThat(logger.warnings).hasSize(1)
        assertThat(logger.warnings.single()).contains("Android Keystore is unusable")
    }

    @Test
    fun `the fallback is sticky and does not touch the keystore again`() {
        probeFailure = ProviderException("Keystore operation failed")
        factory { ProviderException("Keystore operation failed") }.create(NAME)
        encryptedCreations.clear()
        probes = 0

        // Even a keystore that would now work is not used again on this install.
        probeFailure = null
        val prefs = factory().create(NAME)

        assertThat(prefs.isFile("${NAME}_plain")).isTrue()
        assertThat(encryptedCreations).isEmpty()
        assertThat(probes).isEqualTo(0)
        assertThat(logger.warnings).hasSize(1)
    }

    @Test
    fun `an existing master key that is unusable falls back too`() {
        probeFailure = ProviderException("Keystore operation failed")
        val unusable = KeyStoreException("the master key android-keystore://_androidx_security_master_key_ exists but is unusable")

        val prefs = factory { unusable }.create(NAME)

        assertThat(prefs.isFile("${NAME}_plain")).isTrue()
        assertThat(marker.exists()).isTrue()
        // The broken key is not deleted: recovery is only for a working Keystore.
        assertThat(recoveries).isEmpty()
    }

    @Test
    fun `plain data survives later starts`() {
        probeFailure = ProviderException("Keystore operation failed")
        factory { ProviderException("Keystore operation failed") }.create(NAME)
            .edit(commit = true) { putString("auth_state", "session") }

        val prefs = factory { ProviderException("Keystore operation failed") }.create(NAME)

        assertThat(prefs.getString("auth_state", null)).isEqualTo("session")
    }

    @Test
    fun `an unreadable encrypted file on a working keystore is recreated, not marked`() {
        var failures = 1
        val prefs = factory { if (failures-- > 0) java.security.GeneralSecurityException("keyset mismatch") else null }.create(NAME)

        assertThat(prefs.isFile("encrypted:$NAME")).isTrue()
        assertThat(encryptedCreations).containsExactly(NAME, NAME)
        assertThat(recoveries).containsExactly(NAME)
        assertThat(probes).isEqualTo(1)
        assertThat(marker.exists()).isFalse()
    }

    @Test
    fun `a store that keeps failing on a working keystore still does not crash`() {
        val prefs = factory { java.io.IOException("cannot read keyset") }.create(NAME)

        assertThat(prefs.isFile("${NAME}_plain")).isTrue()
        assertThat(marker.exists()).isTrue()
    }

    private companion object {
        const val NAME = "AuthStore"
    }
}
