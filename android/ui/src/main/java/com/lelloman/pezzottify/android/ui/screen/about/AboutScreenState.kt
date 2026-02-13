package com.lelloman.pezzottify.android.ui.screen.about

data class CatalogStatItem(
    val available: Int,
    val unavailable: Int,
)

data class CatalogStats(
    val artists: CatalogStatItem,
    val albums: CatalogStatItem,
    val tracks: CatalogStatItem,
)

data class AboutScreenState(
    val appName: String = "Pezzottify",
    val versionName: String = "",
    val gitCommit: String = "",
    val serverUrl: String = "",
    val serverVersion: String = "disconnected",
    val catalogStats: CatalogStats? = null,
    val catalogStatsLoading: Boolean = true,
)
