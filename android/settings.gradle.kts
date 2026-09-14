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

// Opt-in local verification of unpublished assistant changes; normal builds use
// the immutable JitPack revision in libs.versions.toml.
providers.gradleProperty("assistantCheckout").orNull?.let { checkout ->
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
