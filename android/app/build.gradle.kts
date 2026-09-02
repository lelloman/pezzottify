import java.util.Properties

plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.android)
    alias(libs.plugins.kotlin.compose)
    alias(libs.plugins.hilt)
    alias(libs.plugins.kotlin.kapt)
    alias(libs.plugins.google.ksp)
}

// Load local.properties for OIDC config
val localProperties = Properties().apply {
    val localPropsFile = rootProject.file("local.properties")
    if (localPropsFile.exists()) {
        localPropsFile.inputStream().use { load(it) }
    }
}

// Load signing.properties for release signing config
val signingProperties = Properties().apply {
    val signingPropsFile = rootProject.file("signing.properties")
    if (signingPropsFile.exists()) {
        signingPropsFile.inputStream().use { load(it) }
    }
}

// Compute full version: MAJOR.MINOR.COMMIT-COUNT
val baseVersion: String by lazy {
    try {
        rootProject.file("../VERSION").readText().trim()
    } catch (e: Exception) {
        "0.0"
    }
}

val commitCount: Int by lazy {
    try {
        val process = ProcessBuilder("git", "rev-list", "--count", "HEAD")
            .redirectErrorStream(true)
            .start()
        process.inputStream.bufferedReader().readText().trim().toInt()
    } catch (e: Exception) {
        0
    }
}

val appVersion = "$baseVersion.$commitCount"

fun buildProperty(name: String, default: String): String =
    providers.gradleProperty(name).orNull ?: localProperties.getProperty(name, default)

fun String.asBuildConfigString(): String =
    "\"${replace("\\", "\\\\").replace("\"", "\\\"")}\""

val assistantProvider = buildProperty("assistant.provider", "simpleai").lowercase()
val supportedAssistantProviders = setOf("user", "simpleai", "ollama")
require(assistantProvider in supportedAssistantProviders) {
    "assistant.provider must be one of: ${supportedAssistantProviders.joinToString()}"
}

val assistantOllamaBaseUrl = buildProperty("assistant.ollama.baseUrl", "http://10.0.2.2:11434")
val assistantOllamaModel = buildProperty("assistant.ollama.model", "gpt-oss:20b")
val assistantOllamaTimeoutSeconds =
    buildProperty("assistant.ollama.timeoutSeconds", "120").toLongOrNull()
        ?: error("assistant.ollama.timeoutSeconds must be an integer")
require(assistantOllamaTimeoutSeconds > 0) {
    "assistant.ollama.timeoutSeconds must be positive"
}

android {
    namespace = "com.lelloman.pezzottify.android"
    compileSdk = 36

    defaultConfig {
        applicationId = "com.lelloman.pezzottify.android"
        minSdk = 24
        targetSdk = 36
        versionCode = commitCount
        versionName = appVersion

        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"

        // AppAuth redirect scheme for OIDC callback
        manifestPlaceholders["appAuthRedirectScheme"] = "com.lelloman.pezzottify.android"
    }

    flavorDimensions += "formFactor"
    productFlavors {
        create("phone") {
            dimension = "formFactor"
            buildConfigField("boolean", "IS_TV", "false")
            buildConfigField("String", "DEVICE_TYPE", "\"android\"")
        }
        create("tv") {
            dimension = "formFactor"
            applicationIdSuffix = ".tv"
            versionNameSuffix = "-tv"
            buildConfigField("boolean", "IS_TV", "true")
            buildConfigField("String", "DEVICE_TYPE", "\"android_tv\"")
        }
    }

    signingConfigs {
        getByName("debug") {
            // Uses default debug keystore
        }
        if (signingProperties.containsKey("storeFile")) {
            create("release") {
                storeFile = file(signingProperties.getProperty("storeFile"))
                storePassword = signingProperties.getProperty("storePassword")
                keyAlias = signingProperties.getProperty("keyAlias")
                keyPassword = signingProperties.getProperty("keyPassword")
            }
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = true
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro"
            )
            if (signingConfigs.findByName("release") != null) {
                signingConfig = signingConfigs.getByName("release")
            }
        }
        create("releaseDebugSigned") {
            initWith(getByName("release"))
            signingConfig = signingConfigs.getByName("debug")
            matchingFallbacks += listOf("release")
        }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_11
        targetCompatibility = JavaVersion.VERSION_11
    }
    kotlinOptions {
        jvmTarget = "11"
    }
    buildFeatures {
        compose = true
        buildConfig = true
    }
}

fun getGitCommit(): String {
    return try {
        val commitProcess = ProcessBuilder("git", "rev-parse", "--short", "HEAD")
            .redirectErrorStream(true)
            .start()
        val commit = commitProcess.inputStream.bufferedReader().readText().trim()

        val statusProcess = ProcessBuilder("git", "status", "--porcelain")
            .redirectErrorStream(true)
            .start()
        val isDirty = statusProcess.inputStream.bufferedReader().readText().isNotBlank()

        if (isDirty) "$commit-dirty" else commit
    } catch (e: Exception) {
        "unknown"
    }
}

android.defaultConfig {
    buildConfigField("String", "GIT_COMMIT", "\"${getGitCommit()}\"")

    // OIDC config from local.properties
    val oidcIssuerUrl = localProperties.getProperty("oidc.issuerUrl", "")
    val oidcClientId = localProperties.getProperty("oidc.clientId", "")
    buildConfigField("String", "OIDC_ISSUER_URL", "\"$oidcIssuerUrl\"")
    buildConfigField("String", "OIDC_CLIENT_ID", "\"$oidcClientId\"")

    // Server config from local.properties
    val defaultBaseUrl = localProperties.getProperty("server.baseUrl", "http://10.0.2.2:3001")
    buildConfigField("String", "DEFAULT_BASE_URL", "\"$defaultBaseUrl\"")

    // Assistant provider policy. "user" keeps provider selection in runtime settings;
    // a provider ID locks both the provider and its configuration at build time.
    buildConfigField("boolean", "ASSISTANT_PROVIDER_CONFIGURABLE", (assistantProvider == "user").toString())
    buildConfigField(
        "String",
        "ASSISTANT_PROVIDER_ID",
        (if (assistantProvider == "user") "simpleai" else assistantProvider).asBuildConfigString()
    )
    buildConfigField("String", "ASSISTANT_OLLAMA_BASE_URL", assistantOllamaBaseUrl.asBuildConfigString())
    buildConfigField("String", "ASSISTANT_OLLAMA_MODEL", assistantOllamaModel.asBuildConfigString())
    buildConfigField("long", "ASSISTANT_OLLAMA_TIMEOUT_SECONDS", "${assistantOllamaTimeoutSeconds}L")
}

dependencies {
    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.lifecycle.runtime.ktx)
    implementation(libs.androidx.lifecycle.process)
    implementation(libs.androidx.activity.compose)
    implementation(platform(libs.androidx.compose.bom))
    implementation(libs.androidx.ui)
    implementation(libs.androidx.ui.graphics)
    implementation(libs.androidx.ui.tooling.preview)
    implementation(libs.androidx.material3)
    implementation(libs.androidx.compose.material.icons.extended)

    implementation(libs.hilt)
    implementation(libs.hilt.navigation.compose)
    kapt(libs.hilt.compiler)

    implementation(libs.androidx.work.runtime)

    implementation(libs.coil)
    implementation(libs.coil.network)
    implementation(platform(libs.okhttp.bom))
    implementation(libs.okhttp)
    implementation(libs.appauth)
    implementation(libs.room.runtime)
    implementation(libs.exoplayer)
    implementation(libs.androidx.media3.session)
    implementation(libs.kotlinx.serialization.json)

    implementation(project(":ui"))
    implementation(project(":domain"))
    implementation(project(":remoteapi"))
    implementation(project(":localdata"))
    implementation(project(":player"))
    implementation(project(":logger"))
    implementation(libs.simple.android.assistant.core)
    implementation(libs.simple.android.assistant.provider.ollama)
    implementation(libs.simple.android.assistant.provider.simpleai)
    debugImplementation(project(":debuginterface"))

    // DuckMapper for automatic mapping code generation
    implementation("com.github.lelloman.duckmapper:annotations:0.3.0")
    ksp("com.github.lelloman.duckmapper:ksp:0.3.0")

    testImplementation(libs.junit)
    testImplementation(libs.truth)
    testImplementation(libs.mockk)
    testImplementation(libs.kotlinx.coroutines.test)

    androidTestImplementation(libs.androidx.junit)
    androidTestImplementation(libs.androidx.espresso.core)
    androidTestImplementation(platform(libs.androidx.compose.bom))
    androidTestImplementation(libs.androidx.ui.test.junit4)

    debugImplementation(libs.androidx.ui.tooling)
    debugImplementation(libs.androidx.ui.test.manifest)
}
