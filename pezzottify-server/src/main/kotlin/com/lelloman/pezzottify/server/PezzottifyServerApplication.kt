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


@SpringBootApplication
class PezzottifyServerApplication

fun main(args: Array<String>) {
    runApplication<PezzottifyServerApplication>(*args)
}
