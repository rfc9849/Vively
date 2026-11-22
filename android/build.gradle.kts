plugins {
    id("com.android.application") version "9.5.1" apply false
    // Not applied (AGP 9 has built-in Kotlin); pins the Kotlin version the
    // Compose compiler plugin must match.
    id("org.jetbrains.kotlin.android") version "2.4.0" apply false
    id("org.jetbrains.kotlin.plugin.compose") version "2.4.0" apply false
}
