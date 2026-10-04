package com.lelloman.pezzottify.android.push

import android.content.Context
import android.content.pm.PackageManager
import dagger.hilt.android.qualifiers.ApplicationContext
import org.unifiedpush.android.connector.UnifiedPush
import org.unifiedpush.android.connector.data.ResolvedDistributor
import javax.inject.Inject

/** Thin wrapper over the UnifiedPush connector's static API, so the logic is testable. */
interface UnifiedPushClient {
    fun distributors(): List<String>
    fun label(packageName: String): String
    fun savedDistributor(): String?

    /** The system's default distributor, when it can be resolved without user interaction. */
    fun defaultDistributor(): String?
    fun saveDistributor(packageName: String)
    fun register(vapid: String)
    fun unregister()
}

class AndroidUnifiedPushClient @Inject constructor(
    @ApplicationContext private val context: Context,
) : UnifiedPushClient {

    override fun distributors(): List<String> = UnifiedPush.getDistributors(context)

    override fun label(packageName: String): String = try {
        val pm = context.packageManager
        pm.getApplicationLabel(pm.getApplicationInfo(packageName, 0)).toString()
    } catch (_: PackageManager.NameNotFoundException) {
        packageName
    }

    override fun savedDistributor(): String? = UnifiedPush.getSavedDistributor(context)

    override fun defaultDistributor(): String? =
        when (val resolved = UnifiedPush.resolveDefaultDistributor(context)) {
            is ResolvedDistributor.Found -> resolved.packageName
            else -> null
        }

    override fun saveDistributor(packageName: String) =
        UnifiedPush.saveDistributor(context, packageName)

    override fun register(vapid: String) =
        UnifiedPush.register(context, INSTANCE, MESSAGE_FOR_DISTRIBUTOR, vapid)

    override fun unregister() = UnifiedPush.unregister(context, INSTANCE)

    companion object {
        const val INSTANCE = "default"
        private const val MESSAGE_FOR_DISTRIBUTOR = "Pezzottify"
    }
}
