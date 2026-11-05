package com.lelloman.pezzottify.android.localdata.internal.security

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import com.lelloman.pezzottify.android.logger.Logger
import com.lelloman.pezzottify.android.logger.LoggerFactory
import java.io.File
import java.io.FileOutputStream
import java.io.IOException
import java.security.KeyStore
import java.util.concurrent.ConcurrentHashMap
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/**
 * Lets Pezzottify run on devices whose Android Keystore cannot be used.
 *
 * Some cheap Android TV boxes (seen on a Tanix X4: Android 11, Amlogic S905X4, 32-bit userspace)
 * ship a keymaster HAL that cannot generate or use AES keys: `KeyGenerator.generateKey()` throws
 * `ProviderException: Keystore operation failed`, and a key left by an earlier run is reported as
 * "exists but is unusable". Reinstalling does not help. The Keystore-backed store in the client
 * (the auth session in `EncryptedSharedPreferences`, see `AuthStoreImpl`) asks this class whether
 * to use Keystore encryption or plain app-private storage.
 *
 * The decision is made only after a Keystore-backed store failed: a [probe] then generates and uses
 * a dedicated AES-GCM key (a few attempts). When the probe fails too, the Keystore is recorded as
 * unusable in [markerFile] (in `noBackupFilesDir`, so it is per install and is not carried to
 * another device by a backup). From then on the stores never touch the Keystore again, so a key
 * that later becomes "unusable" cannot crash the app, and no key generation is retried at each
 * start. On a device whose Keystore works nothing changes: the probe passes, nothing is marked and
 * the encrypted stores keep their data.
 *
 * Security posture: on a device marked here, the secrets these stores hold (the auth token and
 * session) rest in app-private storage protected only by the Android application sandbox and the
 * device's own storage encryption, as most apps keep their tokens. Root, or anyone who can read the
 * app's data directory, can read them. This is accepted for these boxes; the alternative is an app
 * that cannot start at all.
 */
internal class KeystoreFallback(
    private val markerFile: File,
    private val logger: Logger,
    private val probe: () -> Unit = ::probeAndroidKeystore,
) {
    @Volatile private var markedInProcess = false

    /** True when the Keystore was found unusable on this install; stores then use plain storage. */
    val isUnusable: Boolean get() = markedInProcess || markerFile.exists()

    /**
     * Whether a store should use plain storage. Logs the fallback warning once per process the first
     * time the answer is yes.
     */
    fun usePlain(): Boolean {
        if (!isUnusable) return false
        warnOnce(null)
        return true
    }

    /**
     * Called by a store whose Keystore-backed operation failed with [cause]. Probes the Keystore;
     * when it is unusable, records that for good and returns true (the caller then uses plain
     * storage). Returns false when the Keystore works, so the failure has another reason.
     */
    fun confirmUnusable(cause: Throwable): Boolean {
        if (usePlain()) return true
        val probeFailure = runProbe() ?: return false
        if (probeFailure !== cause) cause.addSuppressed(probeFailure)
        markUnusable(cause)
        return true
    }

    /** Records that the Keystore is unusable on this install, whatever a probe would say. */
    fun markUnusable(cause: Throwable) {
        markedInProcess = true
        try {
            markerFile.absoluteFile.parentFile?.mkdirs()
            FileOutputStream(markerFile).use { output ->
                output.write("${System.currentTimeMillis()} ${cause.javaClass.name}: ${cause.message}\n".toByteArray())
                output.fd.sync()
            }
        } catch (e: IOException) {
            // Still falls back for this process; the next start probes again.
            logger.warn("Could not record that the Android Keystore is unusable", e)
        }
        warnOnce(cause)
    }

    private fun runProbe(): Throwable? {
        var failure: Throwable? = null
        repeat(PROBE_ATTEMPTS) { attempt ->
            try {
                probe()
                return null
            } catch (e: Exception) {
                failure = e
                if (attempt < PROBE_ATTEMPTS - 1) Thread.sleep(PROBE_RETRY_DELAY_MS)
            }
        }
        return failure
    }

    private fun warnOnce(cause: Throwable?) {
        if (!warnedMarkers.add(markerFile.absolutePath)) return
        logger.warn(
            "Android Keystore is unusable on this device; secrets are kept in app-private storage " +
                "without Keystore encryption (protected only by the app sandbox)",
            cause,
        )
    }

    companion object {
        const val MARKER_FILE_NAME = "keystore-unusable"
        const val LOG_TAG = "KeystoreFallback"
        private const val PROBE_ATTEMPTS = 3
        private const val PROBE_RETRY_DELAY_MS = 100L

        // One warning per process for each install marker, however many instances ask.
        private val warnedMarkers: MutableSet<String> = ConcurrentHashMap.newKeySet()

        /** The instance for this app install, recording its decision in `noBackupFilesDir`. */
        fun forApp(context: Context, loggerFactory: LoggerFactory): KeystoreFallback =
            KeystoreFallback(
                File(context.noBackupFilesDir, MARKER_FILE_NAME),
                loggerFactory.getLogger(LOG_TAG),
            )
    }
}

private const val ANDROID_KEY_STORE = "AndroidKeyStore"
private const val PROBE_ALIAS = "pezzottify.keystore_probe.v1"

/** Generates (or reuses) an AES-GCM Keystore key and round-trips data with it; throws when it cannot. */
internal fun probeAndroidKeystore() {
    val keyStore = KeyStore.getInstance(ANDROID_KEY_STORE).apply { load(null) }
    try {
        val key = (keyStore.getKey(PROBE_ALIAS, null) as? SecretKey)
            ?: KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, ANDROID_KEY_STORE).run {
                init(
                    KeyGenParameterSpec.Builder(PROBE_ALIAS, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
                        .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                        .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                        .setKeySize(256)
                        .build(),
                )
                generateKey()
            }
        val plaintext = "pezzottify keystore probe".toByteArray()
        val encrypt = Cipher.getInstance("AES/GCM/NoPadding").apply { init(Cipher.ENCRYPT_MODE, key) }
        val sealed = encrypt.doFinal(plaintext)
        val decrypt = Cipher.getInstance("AES/GCM/NoPadding").apply {
            init(Cipher.DECRYPT_MODE, key, GCMParameterSpec(128, encrypt.iv))
        }
        check(decrypt.doFinal(sealed).contentEquals(plaintext)) { "Keystore probe round trip mismatch" }
    } catch (e: Exception) {
        // A stale probe key can be the problem; the next attempt starts from a fresh one.
        runCatching { keyStore.deleteEntry(PROBE_ALIAS) }
        throw e
    }
}
