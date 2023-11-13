package com.lelloman.pezzottify.server.controller

import com.lelloman.pezzottify.server.service.ProfileService
import org.springframework.beans.factory.annotation.Autowired
import org.springframework.stereotype.Controller
import org.springframework.web.bind.annotation.RequestMapping
import org.springframework.web.bind.annotation.RequestMethod
import org.springframework.web.bind.annotation.ResponseBody


@Controller
class HomeController(@Autowired private val profileService: ProfileService) {

    @ResponseBody
    @RequestMapping("/")
    fun home(): String {
        return "HOME ${profileService.name}"
    }
}