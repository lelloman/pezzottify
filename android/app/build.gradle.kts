import java.net.URI
import java.util.Properties

plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.android)
    alias(libs.plugins.kotlin.compose)
    alias(libs.plugins.hilt)
    alias(libs.plugins.kotlin.kapt)
    alias(libs.plugins.google.ksp)
    id("com.lelloman.paravoid")
    id("com.lelloman.paravoid.hilt")
}

// Load local.properties for OIDC, server and update settings
val localProperties = Properties().apply {
    val localPropsFile = rootProject.file("local.properties")
    if (localPropsFile.exists()) {
        localPropsFile.inputStream().use { load(it) }
    }
}

// Optional external server/OIDC settings for isolated worktrees; never copied into Git.
val connectionProperties = Properties().apply {
    providers.gradleProperty("connectionPropertiesFile").orNull?.let { path ->
        rootProject.file(path).inputStream().use { load(it) }
    }
}

// Deployment settings: -PconnectionPropertiesFile wins over local.properties.
fun deploymentProperty(name: String, default: String = ""): String =
    connectionProperties.getProperty(name, localProperties.getProperty(name, default))

fun isHttpsUrl(value: String): Boolean = runCatching {
    val uri = URI(value)
    uri.scheme == "https" && !uri.host.isNullOrBlank() && uri.userInfo == null
}.getOrDefault(false)

// Complete packaging and its disposable delivery acceptance are explicit opt-ins.
val paravoidComplete = providers.gradleProperty("paravoidComplete").map(String::toBooleanStrict).orElse(false).get()
val paravoidAcceptance = providers.gradleProperty("paravoidAcceptance").map(String::toBooleanStrict).orElse(false).get()
val paravoidProduction = providers.gradleProperty("paravoidProduction").map(String::toBooleanStrict).orElse(false).get()
require(!paravoidProduction || (paravoidComplete && !paravoidAcceptance)) {
    "paravoidProduction requires complete packaging without acceptance mode"
}
require(!paravoidAcceptance || paravoidComplete) { "paravoidAcceptance requires paravoidComplete=true" }
require(!providers.gradleProperty("paravoidAcceptanceGeneration").isPresent || paravoidAcceptance) {
    "paravoidAcceptanceGeneration requires paravoidAcceptance=true"
}
val paravoidAcceptanceGeneration = providers.gradleProperty("paravoidAcceptanceGeneration").orElse("none").get()

// Production updates come from the deployment's update server, never from a built-in host.
// An optional trigger app (package plus signing-certificate SHA-256) may hint the shell to
// check for updates.
val paravoidUpdateBaseUrl = deploymentProperty("paravoid.updateBaseUrl")
val paravoidTriggerPackage = deploymentProperty("paravoid.updateTriggerPackage")
val paravoidTriggerCertificate = deploymentProperty("paravoid.updateTriggerCertificate")
if (paravoidProduction) {
    require(isHttpsUrl(paravoidUpdateBaseUrl) && paravoidUpdateBaseUrl.endsWith("/")) {
        "Paravoid production requires an HTTPS paravoid.updateBaseUrl ending in / in local.properties or -PconnectionPropertiesFile"
    }
    require(
        (paravoidTriggerPackage.isBlank() && paravoidTriggerCertificate.isBlank()) || (
            paravoidTriggerPackage.matches(Regex("[A-Za-z][A-Za-z0-9_]*(\\.[A-Za-z][A-Za-z0-9_]*)+")) &&
                paravoidTriggerCertificate.matches(Regex("[a-fA-F0-9]{64}"))
            )
    ) {
        "paravoid.updateTriggerPackage and a 64-hex paravoid.updateTriggerCertificate must be set together"
    }
}

if (paravoidComplete) {
    paravoid {
        packaging.set("complete")
        // Only disposable acceptance fixtures expose the harness launcher.
        controlsLauncher.set(paravoidAcceptance)
        bootstrap.set("embedded")
        crashRecovery { enabled.set(paravoidProduction); updater.set("default") }
        payloadVersion.set(providers.gradleProperty("paravoidPayloadVersion").map(String::toLong).orElse(1L))
        // Shrink the payload with Paravoid's payload-only R8 (AGP minify stays off for
        // Paravoid variants). -PparavoidMinifyPayload=false builds an unshrunk payload.
        minifyPayload.set(
            providers.gradleProperty("paravoidMinifyPayload").map(String::toBoolean).orElse(true)
        )
        // Same baseline as the normal release (enum values()/valueOf(), Parcelable creators,
        // native methods, ...): without it R8 stripped an enum's values() used by reflection
        // and the payload failed to start.
        payloadProguardFiles.from(
            android.getDefaultProguardFile("proguard-android-optimize.txt"),
            "proguard-rules.pro",
            "paravoid-payload-rules.pro",
        )
        updates {
            enabled.set(paravoidAcceptance || paravoidProduction)
            baseUrl.set(if (paravoidProduction) paravoidUpdateBaseUrl else "http://127.0.0.1:19165/")
            authentication.set(if (paravoidProduction) "apkKey" else "public")
            if (paravoidProduction) {
                // Trigger hints drive prompt updates; polling covers missed hints.
                schedule {
                    checks.set(true)
                    downloads.set(true)
                    intervalSeconds.set(48L * 3600)
                    flexSeconds.set(6L * 3600)
                }
                if (paravoidTriggerPackage.isNotBlank()) {
                    localTriggers {
                        trustedCallers.put(paravoidTriggerPackage, listOf(paravoidTriggerCertificate.lowercase()))
                    }
                }
            }
            debugHttpAllowed.set(paravoidAcceptance)
            trustPolicyFile.set(rootProject.file(providers.gradleProperty("paravoidTrustPolicy").get()))
        }
        signing {
            keyId.set("release")
            privateKeyFile.set(rootProject.file(providers.gradleProperty("paravoidReleaseKey").get()))
        }
        providers.gradleProperty("paravoidBaseline").orNull?.let { baselineDirectory.set(rootProject.file(it)) }
    }
}

// The payload R8 rules include AGP's extracted proguard-android-optimize.txt.
tasks.matching { it.name.startsWith("package") && it.name.endsWith("ParavoidApplication") }
    .configureEach { dependsOn("extractProguardFiles") }

// Load signing.properties for release signing config
val signingProperties = Properties().apply {
    val signingPropsFile = rootProject.file("signing.properties")
    if (signingPropsFile.exists()) {
        signingPropsFile.inputStream().use { load(it) }
    }
}
if (paravoidProduction) {
    require(listOf("storeFile", "storePassword", "keyAlias", "keyPassword").all {
        !signingProperties.getProperty(it).isNullOrBlank()
    }) { "Paravoid production requires complete release signing.properties" }
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

// A Paravoid payload (VPK) update must be built for the already installed shell, whose
// manifest includes its version. -PparavoidShellVersionCode pins the shell version to
// that release while the payload version still advances.
val shellVersionCode: Int =
    providers.gradleProperty("paravoidShellVersionCode").orNull?.toInt() ?: commitCount

val appVersion = "$baseVersion.$shellVersionCode"

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
        versionCode = shellVersionCode
        versionName = appVersion
        buildConfigField("boolean", "PARAVOID_ACCEPTANCE_ENABLED", "false")
        buildConfigField("String", "PARAVOID_ACCEPTANCE_GENERATION", "\"none\"")

        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"

        // AppAuth redirect scheme for OIDC callback
        manifestPlaceholders["appAuthRedirectScheme"] = "com.lelloman.pezzottify.android"
        buildConfigField("String", "OIDC_REDIRECT_SCHEME", "\"com.lelloman.pezzottify.android\"")
    }

    flavorDimensions += "formFactor"
    productFlavors {
        getByName("paravoidAndroid") {
            // Production is an in-place update of Pezzottify. Only disposable
            // test builds use an isolated package and OAuth callback.
            applicationIdSuffix = if (paravoidProduction) "" else ".paravoid"
            buildConfigField("boolean", "PARAVOID_ACCEPTANCE_ENABLED", paravoidAcceptance.toString())
            buildConfigField("String", "PARAVOID_ACCEPTANCE_GENERATION", paravoidAcceptanceGeneration.asBuildConfigString())
            minSdk = 30
            val redirectScheme = if (paravoidProduction) "com.lelloman.pezzottify.android" else "com.lelloman.pezzottify.android.paravoid"
            manifestPlaceholders["appAuthRedirectScheme"] = redirectScheme
            buildConfigField("String", "OIDC_REDIRECT_SCHEME", redirectScheme.asBuildConfigString())
        }
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
        debug {
        }
        release {
            buildConfigField("boolean", "PARAVOID_ACCEPTANCE_ENABLED", "false")
            buildConfigField("String", "PARAVOID_ACCEPTANCE_GENERATION", "\"none\"")
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
        create("paravoidTestRelease") {
            initWith(getByName("release"))
            isMinifyEnabled = false
            isDebuggable = false
            signingConfig = signingConfigs.getByName("debug")
            matchingFallbacks += listOf("release")
        }
        create("paravoidRelease") {
            initWith(getByName("release"))
            isMinifyEnabled = false
            isDebuggable = false
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

// Shells use a separate unshrunk release type; normal releases retain R8.
androidComponents.beforeVariants {
    val shell = it.productFlavors.any { flavor -> flavor.second == "paravoidAndroid" }
    if (shell) {
        val allowedBuildTypes = when {
            paravoidProduction -> listOf("paravoidRelease")
            paravoidAcceptance -> listOf("debug")
            else -> listOf("debug", "paravoidTestRelease")
        }
        it.enable = it.buildType in allowedBuildTypes && it.productFlavors.any { flavor -> flavor.second == "phone" }
    } else if (it.buildType in listOf("paravoidTestRelease", "paravoidRelease")) {
        it.enable = false
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
    val oidcIssuerUrl = connectionProperties.getProperty("oidc.issuerUrl", localProperties.getProperty("oidc.issuerUrl", ""))
    val oidcClientId = connectionProperties.getProperty("oidc.clientId", localProperties.getProperty("oidc.clientId", ""))
    if (providers.gradleProperty("connectionPropertiesFile").isPresent) {
        require(oidcIssuerUrl.isNotBlank() && oidcClientId.isNotBlank()) { "External connection configuration must provide OIDC issuer and client ID" }
    }
    buildConfigField("String", "OIDC_ISSUER_URL", "\"$oidcIssuerUrl\"")
    buildConfigField("String", "OIDC_CLIENT_ID", "\"$oidcClientId\"")

    // Trusted authorization broker: an optional installed app that is the verified App Link
    // handler for the issuer host and may show the authorization request instead of a browser.
    // Deployment-specific and off unless local.properties (or -PconnectionPropertiesFile)
    // provides it; see README "Trusted authorization broker".
    fun brokerProperty(name: String, default: String): String =
        connectionProperties.getProperty(name, localProperties.getProperty(name, default))
    val brokerEnabled = brokerProperty("oidc.brokerEnabled", "false").toBooleanStrict()
    val brokerPackage = brokerProperty("oidc.brokerPackage", "")
    val brokerCertificate = brokerProperty("oidc.brokerCertificate", "")
    val brokerIssuerHost = runCatching { URI(oidcIssuerUrl) }.getOrNull()
        ?.takeIf { it.scheme == "https" && it.userInfo == null && !it.host.isNullOrBlank() }?.host
    require(!brokerEnabled || (
        brokerPackage.matches(Regex("[A-Za-z][A-Za-z0-9_]*(\\.[A-Za-z][A-Za-z0-9_]*)+")) &&
            brokerCertificate.matches(Regex("[a-fA-F0-9]{64}")) && brokerIssuerHost != null
    )) {
        "oidc.brokerEnabled requires oidc.brokerPackage, a 64-hex oidc.brokerCertificate and an HTTPS oidc.issuerUrl"
    }
    buildConfigField("boolean", "OIDC_BROKER_ENABLED", brokerEnabled.toString())
    buildConfigField("String", "OIDC_BROKER_PACKAGE", brokerPackage.asBuildConfigString())
    buildConfigField("String", "OIDC_BROKER_CERTIFICATE", brokerCertificate.asBuildConfigString())
    // Package visibility for the broker is granted by intent, never by name; unconfigured builds
    // query an unresolvable host and therefore see no broker.
    manifestPlaceholders["oidcBrokerQueryHost"] = if (brokerEnabled) checkNotNull(brokerIssuerHost) else "unconfigured.invalid"

    // Server config from local.properties
    val defaultBaseUrl = connectionProperties.getProperty("server.baseUrl", localProperties.getProperty("server.baseUrl", "http://10.0.2.2:3001"))
    if (paravoidProduction) {
        require(isHttpsUrl(oidcIssuerUrl) && oidcClientId.isNotBlank()) {
            "Paravoid production requires an HTTPS oidc.issuerUrl and non-empty oidc.clientId in local.properties or -PconnectionPropertiesFile"
        }
        require(isHttpsUrl(defaultBaseUrl)) {
            "Paravoid production requires an HTTPS server.baseUrl in local.properties or -PconnectionPropertiesFile; the emulator default cannot be published"
        }
    }
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

// One contract fixture is shared with the server and web tests.
android.sourceSets.getByName("test").resources.srcDir(rootProject.file("../docs/fixtures"))

dependencies {
    implementation(project(":paravoid-api"))
    "paravoidAndroidImplementation"(project(":paravoid-runtime"))
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
    // The connector depends on the JVM `tink` artifact, which duplicates tink-android
    // (already pulled in by encrypted preferences). Use tink-android at the version the
    // connector needs instead.
    implementation(libs.unifiedpush.connector) {
        exclude(group = "com.google.crypto.tink", module = "tink")
    }
    implementation(libs.tink.android)

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
    implementation(project(":debuginterface"))

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
