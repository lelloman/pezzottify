package com.lelloman.pezzottify.android.ui.screen.about

data class AboutScreenState(
    val appName: String = "Pezzottify",
    val versionName: String = "",
    val gitCommit: String = "",
    val serverUrl: String = "",
    val artistCount: Int = 0,
    val albumCount: Int = 0,
    val trackCount: Int = 0,
)
