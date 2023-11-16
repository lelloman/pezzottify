package com.lelloman.pezzottify.server.controller.model

sealed class MeResponse(val username: String, val description: String) {
    object Nobody : MeResponse("", "You're nobody")
    class AdminAndUser(username: String) : MeResponse(username, "You're the boss and a regular user.")
    class Admin(username: String) : MeResponse(username, "You're the boss.")
    class User(username: String) : MeResponse(username, "You're a regular user.")
    class NoRolesUser(username: String) : MeResponse(username, "You have no role...?")
}
