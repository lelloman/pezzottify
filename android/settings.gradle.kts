pluginManagement {
    repositories {
        google {
            content {
                includeGroupByRegex("com\\.android.*")
                includeGroupByRegex("com\\.google.*")
                includeGroupByRegex("androidx.*")
            }
        }
        mavenCentral()
        gradlePluginPortal()
    }
}
dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        mavenCentral()
        maven { url = uri("https://jitpack.io") }
        mavenLocal()  // For local DuckMapper development
    }
}

rootProject.name = "Pezzottify"

// The Rust-backed 0.2 API requires the matching source checkout until release.
// CI provisions the revision in simple-android-assistant.rev; developers can
// run scripts/checkout-simple-android-assistant.sh or override assistantCheckout.
val assistantCheckout = providers.gradleProperty("assistantCheckout").orNull
    ?: file("../../simple-android-assistant").takeIf { it.isDirectory }?.absolutePath
    ?: error("Run bash scripts/checkout-simple-android-assistant.sh from the repository root or set -PassistantCheckout=/path/to/checkout")
assistantCheckout.let { checkout ->
    includeBuild(checkout) {
        dependencySubstitution {
            listOf("assistant-core", "assistant-compose", "provider-ollama", "provider-simpleai").forEach { artifact ->
                substitute(module("com.github.lelloman.simple-android-assistant:$artifact"))
                    .using(project(":$artifact"))
            }
        }
    }
}
include(":app")
include(":ui")
include(":remoteapi")
include(":localdata")
include(":debuginterface")
include(":player")
include(":logger")
include(":domain")
