plugins {
    id("java-library")
    id("org.jetbrains.kotlin.jvm")
}

dependencies {
    // gson
    implementation("com.google.code.gson:gson:2.10.1")
    // okhttp
    implementation(platform("com.squareup.okhttp3:okhttp-bom:4.11.0"))
    implementation("com.squareup.okhttp3:okhttp")
    implementation("com.squareup.okhttp3:logging-interceptor")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-core:1.7.3")

    testImplementation("junit:junit:4.13.2")
    testImplementation("com.google.truth:truth:1.1.4")
    // keep v4 because from v5 it requires java 11
    testImplementation("org.mockito.kotlin:mockito-kotlin:4.1.0")
}

tasks.withType<org.jetbrains.kotlin.gradle.tasks.KotlinCompile> { kotlinOptions.jvmTarget = "11" }

java {
    sourceCompatibility = JavaVersion.VERSION_11
    targetCompatibility = JavaVersion.VERSION_11
}