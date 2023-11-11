package com.lelloman.pezzottify.server

import org.springframework.context.annotation.Bean
import org.springframework.context.annotation.Configuration
import org.springframework.security.config.annotation.web.builders.HttpSecurity
import org.springframework.security.config.annotation.web.builders.WebSecurity
import org.springframework.security.config.annotation.web.configuration.EnableWebSecurity
import org.springframework.security.config.annotation.web.configuration.WebSecurityCustomizer
import org.springframework.security.core.userdetails.User
import org.springframework.security.core.userdetails.UserDetails
import org.springframework.security.core.userdetails.UserDetailsService
import org.springframework.security.provisioning.InMemoryUserDetailsManager
import org.springframework.security.web.SecurityFilterChain
import org.springframework.security.web.servlet.util.matcher.MvcRequestMatcher
import org.springframework.security.web.util.matcher.AntPathRequestMatcher
import org.springframework.web.servlet.handler.HandlerMappingIntrospector


@Configuration
@EnableWebSecurity
class SecurityConfig {

    @Bean
    fun filterChain(http: HttpSecurity, introspector: HandlerMappingIntrospector): SecurityFilterChain {
        val mvcMatcherBuilder = MvcRequestMatcher.Builder(introspector)
        http.authorizeHttpRequests {
            it.requestMatchers(mvcMatcherBuilder.pattern("/")).permitAll()
            it.requestMatchers(AntPathRequestMatcher("/h2/**")).permitAll()
            it.anyRequest().authenticated()
        }
        http.csrf { it.ignoringRequestMatchers(AntPathRequestMatcher("/h2/**")) }
        http.headers { it.frameOptions {
            it.disable()
        } }
        http.formLogin { it.permitAll() }
        return http.build();
    }

    @Bean
    fun userDetailsService(): UserDetailsService {
        val user: UserDetails = User.withDefaultPasswordEncoder()
            .username("admin")
            .password("admin")
            .roles("ADMIN")
            .build()
        return InMemoryUserDetailsManager(user)
    }
}