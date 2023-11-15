package com.lelloman.pezzottify.server.controller

import com.lelloman.pezzottify.server.service.ImageUploader
import org.springframework.http.HttpStatus
import org.springframework.http.ResponseEntity
import org.springframework.web.bind.annotation.ControllerAdvice
import org.springframework.web.bind.annotation.ExceptionHandler
import org.springframework.web.servlet.mvc.method.annotation.ResponseEntityExceptionHandler

sealed class CustomControllerException(message: String) : Throwable(message) {
    class BadRequest(message: String) : CustomControllerException(message)
    class NotFound(message: String) : CustomControllerException(message)
}

fun badRequest(message: String) {
    throw CustomControllerException.BadRequest(message)
}

fun notFound(message: String) {
    throw CustomControllerException.NotFound(message)
}

@ControllerAdvice
class CustomExceptionHandler : ResponseEntityExceptionHandler() {

    @ExceptionHandler(CustomControllerException::class)
    fun handleException(e: CustomControllerException) = ResponseEntity.status(HttpStatus.BAD_REQUEST).body(e.message)

    @ExceptionHandler(ImageUploader.DecodeException::class)
    fun handleException(e: ImageUploader.DecodeException) =
        ResponseEntity.status(HttpStatus.BAD_REQUEST).body("Could not decode image")
}