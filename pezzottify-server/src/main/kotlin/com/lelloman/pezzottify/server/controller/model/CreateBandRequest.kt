package com.lelloman.pezzottify.server.controller.model

data class CreateBandRequest(
    val displayName: String,
    val membersIds: List<String>,
)

data class UpdateBandRequest(
    val id: String,
    val displayName: String,
    val membersIds: List<String>,
    val imageId: String?,
)