package com.lelloman.pezzottify.android.ui

import kotlinx.serialization.Serializable

sealed interface Screen {

    @Serializable
    data object Splash : Screen

    @Serializable
    data object Login : Screen

    @Serializable
    data object About : Screen

    @Serializable
    data object Main : Screen
}