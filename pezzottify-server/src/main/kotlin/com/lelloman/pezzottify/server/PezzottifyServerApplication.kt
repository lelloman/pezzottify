package com.lelloman.pezzottify.server

import org.springframework.boot.autoconfigure.SpringBootApplication
import org.springframework.boot.runApplication

@SpringBootApplication
class PezzottifyServerApplication

fun main(args: Array<String>) {
	runApplication<PezzottifyServerApplication>(*args)
}
