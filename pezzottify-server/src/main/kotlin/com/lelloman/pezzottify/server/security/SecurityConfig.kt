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
class SecurityConfig {

    @Bean
    fun authenticationManager(
        http: HttpSecurity,
        passwordEncoder: PasswordEncoder,
        userDetailsService: UserDetailsService
    ): AuthenticationManager {
        val authenticationManagerBuilder = http.getSharedObject(
            AuthenticationManagerBuilder::class.java
        )
        authenticationManagerBuilder.userDetailsService(userDetailsService)
            .passwordEncoder(passwordEncoder)
        return authenticationManagerBuilder.build()
    }

    @Bean
    fun filterChain(http: HttpSecurity, introspector: HandlerMappingIntrospector, authenticationManager: AuthenticationManager): SecurityFilterChain {
        val mvcMatcherBuilder = MvcRequestMatcher.Builder(introspector)
//        http.headers { it.frameOptions { it.disable() } }
        http.authorizeHttpRequests {
            it.requestMatchers(mvcMatcherBuilder.pattern("/")).permitAll()
//            it.requestMatchers(mvcMatcherBuilder.pattern("/authenticate/**")).permitAll()
            it.requestMatchers(AntPathRequestMatcher("/api/auth")).permitAll()
            it.requestMatchers(mvcMatcherBuilder.pattern("/me/**")).permitAll()
            it.requestMatchers(AntPathRequestMatcher("/h2/**")).permitAll()
            it.anyRequest().authenticated()
        }
        http.addFilter(JwtAuthenticationFilter(authenticationManager))
        http.addFilter(JwtAuthorizationFilter(authenticationManager))
        http.csrf {
            it.ignoringRequestMatchers(AntPathRequestMatcher("/h2/**"))
            it.ignoringRequestMatchers(AntPathRequestMatcher("/api/**"))
            it.ignoringRequestMatchers(AntPathRequestMatcher("/authenticate/**"))
        }
        http.sessionManagement { it.sessionCreationPolicy(SessionCreationPolicy.STATELESS) }
        //http.httpBasic { it}
        //http.formLogin { it.disable() }
        return http.build();
    }
}