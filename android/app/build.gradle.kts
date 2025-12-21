import java.util.Properties

plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.android)
    alias(libs.plugins.kotlin.compose)
    alias(libs.plugins.hilt)
    alias(libs.plugins.kotlin.kapt)
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
            isMinifyEnabled = false
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
        sourceCompatibility = JavaVersion.VERSION_1_8
        targetCompatibility = JavaVersion.VERSION_1_8
    }
    kotlinOptions {
        jvmTarget = "1.8"
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

    implementation(libs.hilt)
    kapt(libs.hilt.compiler)

    implementation(libs.coil)
    implementation(libs.coil.network)
    implementation(platform(libs.okhttp.bom))
    implementation(libs.okhttp)
    implementation(libs.appauth)

    implementation(project(":ui"))
    implementation(project(":domain"))
    implementation(project(":remoteapi"))
    implementation(project(":localdata"))
    implementation(project(":player"))
    implementation(project(":logger"))
    debugImplementation(project(":debuginterface"))

    testImplementation(libs.junit)
    testImplementation(libs.truth)

    androidTestImplementation(libs.androidx.junit)
    androidTestImplementation(libs.androidx.espresso.core)
    androidTestImplementation(platform(libs.androidx.compose.bom))
    androidTestImplementation(libs.androidx.ui.test.junit4)

    debugImplementation(libs.androidx.ui.tooling)
    debugImplementation(libs.androidx.ui.test.manifest)
}
