package com.lelloman.pezzottify.android.oidc

import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.content.pm.verify.domain.DomainVerificationManager
import android.content.pm.verify.domain.DomainVerificationUserState
import android.os.Build
import net.openid.appauth.AuthorizationManagementActivity
import net.openid.appauth.AuthorizationRequest
import java.security.MessageDigest

/** A broker is an authorization UI. AppAuth still owns the request, callback and token exchange. */
internal class AuthenticatorLauncher(
    private val context: Context,
    private val enabled: Boolean,
    private val packageName: String,
    private val certificate: String,
) {
    @Suppress("DEPRECATION")
    fun launch(request: AuthorizationRequest, issuer: String): Intent? = runCatching {
        if (!enabled || Build.VERSION.SDK_INT < 30 || !certificate.matches(Regex("[a-fA-F0-9]{64}"))) return null
        val endpoint = request.configuration.authorizationEndpoint
        val origin = android.net.Uri.parse(issuer)
        require(origin.scheme == "https" && origin.host != null && origin.userInfo == null && origin.query == null && origin.fragment == null && origin.path.isNullOrEmpty())
        require(endpoint.scheme == origin.scheme && endpoint.encodedAuthority == origin.encodedAuthority && endpoint.path == "/authorize" && endpoint.query == null && endpoint.fragment == null)
        val packages = context.packageManager
        val signing = checkNotNull(packages.getPackageInfo(packageName, PackageManager.GET_SIGNING_CERTIFICATES).signingInfo)
        fun trusted(value: android.content.pm.Signature) = MessageDigest.getInstance("SHA-256").digest(value.toByteArray())
            .joinToString("") { "%02x".format(it) }.equals(certificate, true)
        require(if (signing.hasMultipleSigners()) signing.apkContentsSigners.all(::trusted) else signing.signingCertificateHistory.any(::trusted))
        val view = Intent(Intent.ACTION_VIEW, request.toUri()).addCategory(Intent.CATEGORY_BROWSABLE)
        if (Build.VERSION.SDK_INT >= 31) {
            val state = checkNotNull(context.getSystemService(DomainVerificationManager::class.java).getDomainVerificationUserState(packageName))
            require(state.isLinkHandlingAllowed && state.hostToStateMap[origin.host] == DomainVerificationUserState.DOMAIN_STATE_VERIFIED)
        } else {
            // Android 11 exposes no public domain-state API; retain the user's selected handler and pin its certificate.
            require(packages.resolveActivity(view, PackageManager.MATCH_DEFAULT_ONLY)?.activityInfo?.packageName == packageName)
        }
        view.setPackage(packageName)
        val activity = checkNotNull(packages.resolveActivity(view, PackageManager.MATCH_DEFAULT_ONLY)?.activityInfo)
        require(activity.packageName == packageName && activity.enabled && activity.exported && activity.applicationInfo.enabled)
        view.setClassName(packageName, activity.name)
        AuthorizationManagementActivity.createStartForResultIntent(context, request, view)
    }.getOrNull()
}
