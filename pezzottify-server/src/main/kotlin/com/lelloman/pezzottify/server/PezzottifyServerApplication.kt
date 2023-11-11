package com.lelloman.pezzottify.server

import org.jetbrains.annotations.PropertyKey
import org.springframework.beans.factory.annotation.Autowired
import org.springframework.boot.autoconfigure.SpringBootApplication
import org.springframework.boot.runApplication
import org.springframework.context.annotation.Bean
import org.springframework.context.annotation.Profile
import org.springframework.stereotype.Controller
import org.springframework.stereotype.Service
import org.springframework.web.bind.annotation.GetMapping
import org.springframework.web.bind.annotation.RequestMapping
import org.springframework.web.bind.annotation.RequestMethod
import org.springframework.web.bind.annotation.ResponseBody

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

@Controller
class HomeController(@Autowired private val profileService: ProfileService) {

    @RequestMapping(value = ["/"], method = [RequestMethod.GET])
    @ResponseBody
    fun home(): String {
        return "HOME ${profileService.name}"
    }
}

@SpringBootApplication
class PezzottifyServerApplication

fun main(args: Array<String>) {
    runApplication<PezzottifyServerApplication>(*args)
}
