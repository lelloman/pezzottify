package com.lelloman.pezzottify.android.ui.screen.main.settings

import android.content.ActivityNotFoundException
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager

/** Open our shell's private controls without exposing another launcher entry. */
internal object AppUpdates {
    private const val ACTIVITY = "com.lelloman.paravoidandroid.delivery.ShellUpdatesActivity"

    private fun intent(context: Context): Intent? {
        val component = ComponentName(context.packageName, ACTIVITY)
        @Suppress("DEPRECATION")
        val activity = try {
            context.packageManager.getActivityInfo(component, 0)
        } catch (_: PackageManager.NameNotFoundException) {
            return null
        }
        if (!activity.enabled || !activity.applicationInfo.enabled) return null
        return Intent().setComponent(component)
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP)
    }

    fun available(context: Context): Boolean = intent(context) != null

    fun open(context: Context): Boolean {
        val intent = intent(context) ?: return false
        return try {
            context.startActivity(intent)
            true
        } catch (_: ActivityNotFoundException) {
            false
        } catch (_: SecurityException) {
            false
        }
    }
}
