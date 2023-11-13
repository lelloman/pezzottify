package com.lelloman.pezzottify.server.service

import org.springframework.context.annotation.Profile
import org.springframework.stereotype.Service


@Service
interface ProfileService {
    val name: String
}

@Profile("prod")
@Service
class ProdProfileService : ProfileService {
    override val name = "PROD"
}

@Profile("dev")
@Service
class DevProfileService : ProfileService {
    override val name = "DEV"
}

@Profile("test")
@Service
class TestProfileService : ProfileService {
    override val name = "TEST"
}
