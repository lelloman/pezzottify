package com.lelloman.pezzottify.server.security

import org.springframework.beans.factory.annotation.Autowired
import org.springframework.context.annotation.Bean
import org.springframework.context.annotation.Configuration
import org.springframework.security.authentication.AuthenticationManager
import org.springframework.security.config.annotation.authentication.builders.AuthenticationManagerBuilder
import org.springframework.security.config.annotation.method.configuration.EnableMethodSecurity
import org.springframework.security.config.annotation.web.builders.HttpSecurity
import org.springframework.security.config.annotation.web.configuration.EnableWebSecurity
import org.springframework.security.config.http.SessionCreationPolicy
import org.springframework.security.core.userdetails.UserDetailsService
import org.springframework.security.crypto.password.PasswordEncoder
import org.springframework.security.web.SecurityFilterChain
import org.springframework.security.web.servlet.util.matcher.MvcRequestMatcher
import org.springframework.security.web.util.matcher.AntPathRequestMatcher
import org.springframework.web.servlet.handler.HandlerMappingIntrospector


@Configuration
@EnableWebSecurity
@EnableMethodSecurity(securedEnabled = true)
class SecurityConfig(@Autowired private val userDetailsService: UserDetailsService) {

    @Bean
    fun authenticationManager(
        http: HttpSecurity,
        passwordEncoder: PasswordEncoder,
    ): AuthenticationManager {
        val authenticationManagerBuilder = http.getSharedObject(
            AuthenticationManagerBuilder::class.java
        )
        authenticationManagerBuilder.userDetailsService(userDetailsService)
            .passwordEncoder(passwordEncoder)
        return authenticationManagerBuilder.build()
    }

    @Bean
    fun filterChain(
        http: HttpSecurity,
        introspector: HandlerMappingIntrospector,
        authenticationManager: AuthenticationManager
    ): SecurityFilterChain {
        http.addFilter(JwtAuthenticationFilter(authenticationManager))
        http.addFilter(JwtAuthorizationFilter(authenticationManager, userDetailsService))
        val mvcMatcherBuilder = MvcRequestMatcher.Builder(introspector)
        http.authorizeHttpRequests {
            it.requestMatchers(mvcMatcherBuilder.pattern("/")).permitAll()
            it.requestMatchers(AntPathRequestMatcher("/api/auth")).permitAll()
            it.requestMatchers(mvcMatcherBuilder.pattern("/me")).permitAll()
            it.requestMatchers(AntPathRequestMatcher("/h2/**")).permitAll()
//            it.requestMatchers(mvcMatcherBuilder.pattern("/api/track/**")).permitAll()
            it.anyRequest().authenticated()
        }
        http.csrf {
            it.disable()
            it.ignoringRequestMatchers(AntPathRequestMatcher("/h2/**"))
            it.ignoringRequestMatchers(AntPathRequestMatcher("/api/**"))
        }
        http.sessionManagement { it.sessionCreationPolicy(SessionCreationPolicy.STATELESS) }
        return http.build();
    }
}