package com.lelloman.pezzottify.server

import com.lelloman.pezzottify.server.model.User
import org.assertj.core.api.Assertions.assertThat
import org.junit.jupiter.api.Test
import org.springframework.beans.factory.annotation.Autowired
import org.springframework.boot.test.context.SpringBootTest
import org.springframework.test.annotation.DirtiesContext
import org.springframework.test.context.ActiveProfiles
import kotlin.jvm.optionals.getOrNull

@SpringBootTest(webEnvironment = SpringBootTest.WebEnvironment.RANDOM_PORT)
@ActiveProfiles("test")
@DirtiesContext(classMode = DirtiesContext.ClassMode.BEFORE_EACH_TEST_METHOD)
class UsersRepositoryTest {

    @Autowired
    private lateinit var tested: UsersRepository

    @Test
    fun `gets user by name`() {
        val user1 = User("user1", "123", emptyList())
        val savedUser1 = tested.save(user1)
        assertThat(user1).isEqualTo(savedUser1)

        assertThat(tested.getByName("user1").getOrNull()?.name).isEqualTo("user1")
        assertThat(tested.getByName("user2").getOrNull()).isNull()

        tested.save(User("user2", "123", emptyList()))
        assertThat(tested.getByName("user2").getOrNull()?.name).isEqualTo("user2")
    }
}