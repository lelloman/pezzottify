package com.lelloman.pezzottify.android.remoteapi.internal.requests

import com.lelloman.pezzottify.android.domain.sync.UserSetting
import kotlinx.serialization.Serializable

@Serializable
internal data class UpdateUserSettingsRequest(
    val settings: List<UserSetting>,
)
