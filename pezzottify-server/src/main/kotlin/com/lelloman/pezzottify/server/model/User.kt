package com.lelloman.pezzottify.server.model

import jakarta.persistence.*
import org.springframework.security.core.GrantedAuthority
import org.springframework.security.core.authority.SimpleGrantedAuthority
import org.springframework.security.core.userdetails.UserDetails

@Entity
@Table(name = "UserEntity")
data class User(
    @Id
    val name: String,

    val pw: String,

    val roles: List<Role>,

    @ManyToMany(fetch = FetchType.LAZY)
    val bookmarkedAlbums: List<Album> = emptyList(),
) : UserDetails {

    override fun getAuthorities(): MutableCollection<out GrantedAuthority> =
        mutableListOf(SimpleGrantedAuthority("USER"))

    override fun getPassword() = pw

    override fun getUsername() = name

    override fun isAccountNonExpired() = true

    override fun isAccountNonLocked() = true

    override fun isCredentialsNonExpired() = true

    override fun isEnabled() = true

    enum class Role {
        ADMIN, USER,
    }
}