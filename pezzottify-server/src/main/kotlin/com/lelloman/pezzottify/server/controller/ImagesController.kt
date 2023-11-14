package com.lelloman.pezzottify.server.controller

import com.lelloman.pezzottify.server.ImagesRepository
import com.lelloman.pezzottify.server.service.FileStorageService
import org.springframework.beans.factory.annotation.Autowired
import org.springframework.web.bind.annotation.RequestMapping
import org.springframework.web.bind.annotation.RestController

@RestController
@RequestMapping("/image")
class ImagesController(
    @Autowired private val repo: ImagesRepository,
    @Autowired private val storage: FileStorageService,
) {


}