package com.lelloman.pezzottify.android.ui.screen.main.profile

import com.lelloman.pezzottify.android.ui.model.Permission

interface ProfileScreenActions {

    fun clickOnLogout()

    fun confirmLogout()

    fun dismissLogoutConfirmation()

    fun onPermissionClicked(permission: Permission)

    fun onPermissionDialogDismissed()
}
