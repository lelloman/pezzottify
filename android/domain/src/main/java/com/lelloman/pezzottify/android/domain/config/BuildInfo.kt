package com.lelloman.pezzottify.android.domain.config

interface BuildInfo {
    val buildVariant: String
    val versionName: String
    val gitCommit: String
}
