package com.lelloman.pezzottify.android.oidc

import android.net.Uri
import net.openid.appauth.AuthorizationRequest
import net.openid.appauth.AuthorizationResponse

internal object AuthorizationCallbacks {
    fun validate(uri: Uri, request: AuthorizationRequest) {
        require(uri.toString().length <= 8192 && uri.isHierarchical && uri.fragment == null)
        val expected = request.redirectUri
        require(uri.buildUpon().clearQuery().build() == expected.buildUpon().clearQuery().build())
        val fields = uri.queryParameterNames
        fields.forEach { require(uri.getQueryParameters(it).size == 1) }
        expected.queryParameterNames.forEach { require(uri.getQueryParameter(it) == expected.getQueryParameter(it)) }
        require(!request.state.isNullOrEmpty() && uri.getQueryParameter("state") == request.state)
        val code = uri.getQueryParameter("code")
        val error = uri.getQueryParameter("error")
        require((!code.isNullOrEmpty()) != (!error.isNullOrEmpty()))
        uri.getQueryParameter("iss")?.let { require(it == request.configuration.discoveryDoc?.issuer?.toString()) }
    }
    fun validate(response: AuthorizationResponse, request: AuthorizationRequest) {
        require(response.request.jsonSerializeString() == request.jsonSerializeString())
        require(!request.state.isNullOrEmpty() && response.state == request.state && !response.authorizationCode.isNullOrEmpty())
    }
}
