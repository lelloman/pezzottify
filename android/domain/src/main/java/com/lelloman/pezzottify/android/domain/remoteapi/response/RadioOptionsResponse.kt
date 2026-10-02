package com.lelloman.pezzottify.android.domain.remoteapi.response

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

/** `GET /v1/content/radio/options`: tunable radio and smart-continuation knobs. */
@Serializable
data class RadioOptions(
    val criteria: List<RadioCriterionOption> = emptyList(),
    @SerialName("default_recipe_id") val defaultRecipeId: String? = null,
    val modes: List<String> = listOf("similar", "explore"),
    val diversity: RadioRange = RadioRange(0.0, 1.0, 0.3),
    val randomness: RadioRange = RadioRange(0.0, 1.0, 0.3),
)

@Serializable
data class RadioCriterionOption(
    val namespace: String,
    val label: String,
)

@Serializable
data class RadioRange(
    val min: Double,
    val max: Double,
    val default: Double,
)
