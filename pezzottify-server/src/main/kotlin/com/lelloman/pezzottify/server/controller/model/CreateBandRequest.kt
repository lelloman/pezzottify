package com.lelloman.pezzottify.server.controller.model

data class CreateBandRequest(
    val displayName: String,
    val membersIds: List<String>,
)