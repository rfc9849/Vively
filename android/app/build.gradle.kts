import org.gradle.process.ExecOperations
import java.util.Properties
import javax.inject.Inject

plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.plugin.compose")
}

val repoRootDir: File = rootDir.parentFile
val rustAbis: List<String> =
    (findProperty("vively.abis") as String? ?: "arm64-v8a").split(',').map { it.trim() }.filter { it.isNotEmpty() }
val cargoBin: String =
    findProperty("vively.cargo") as String? ?: "${System.getProperty("user.home")}/.cargo/bin/cargo"

/** Builds `vively-ffi` for Android with cargo-ndk into a jniLibs-shaped dir. */
abstract class CargoNdkTask : DefaultTask() {
    @get:Inject abstract val execOps: ExecOperations
    @get:Input abstract val cargo: Property<String>
    @get:Input abstract val abis: ListProperty<String>
    @get:Input abstract val release: Property<Boolean>
    @get:Internal abstract val repoRoot: DirectoryProperty
    @get:OutputDirectory abstract val outDir: DirectoryProperty

    init {
        // cargo does its own incremental tracking.
        outputs.upToDateWhen { false }
    }

    @TaskAction
    fun build() {
        val cmd = mutableListOf(cargo.get(), "ndk", "-P", "26", "-o", outDir.get().asFile.absolutePath)
        abis.get().forEach { cmd += listOf("-t", it) }
        cmd += listOf("build", "-p", "vively-ffi", "--lib")
        if (release.get()) cmd += "--release"
        execOps.exec {
            workingDir = repoRoot.get().asFile
            commandLine(cmd)
        }
    }
}

/** Generates Kotlin bindings from a host build of `vively-ffi`. */
abstract class UniffiBindingsTask : DefaultTask() {
    @get:Inject abstract val execOps: ExecOperations
    @get:Input abstract val cargo: Property<String>
    @get:Internal abstract val repoRoot: DirectoryProperty
    @get:OutputDirectory abstract val outDir: DirectoryProperty

    init {
        outputs.upToDateWhen { false }
    }

    @TaskAction
    fun generate() {
        val root = repoRoot.get().asFile
        val out = outDir.get().asFile
        out.deleteRecursively()
        out.mkdirs()
        execOps.exec {
            workingDir = root
            commandLine(cargo.get(), "build", "-p", "vively-ffi", "--lib")
        }
        val os = System.getProperty("os.name").lowercase()
        val lib = when {
            os.contains("mac") -> "libvively_ffi.dylib"
            os.contains("win") -> "vively_ffi.dll"
            else -> "libvively_ffi.so"
        }
        execOps.exec {
            workingDir = root
            commandLine(
                cargo.get(), "run", "-q", "-p", "vively-ffi", "--features", "bindgen",
                "--bin", "uniffi-bindgen", "--",
                "generate", File(root, "target/debug/$lib").absolutePath,
                "--language", "kotlin", "--no-format", "--out-dir", out.absolutePath,
            )
        }
    }
}

val uniffiBindings = tasks.register<UniffiBindingsTask>("uniffiBindings") {
    cargo.set(cargoBin)
    repoRoot.set(repoRootDir)
    outDir.set(layout.buildDirectory.dir("generated/uniffi/kotlin"))
}

android {
    namespace = "eu.r3pr3ss10n.vively"
    compileSdk = 37
    ndkVersion = "28.2.13676358"

    val cargoVersion = providers.fileContents(rootProject.layout.projectDirectory.file("../crates/vively/Cargo.toml")).asText.get()
        .lineSequence().first { it.startsWith("version = ") }.substringAfter('"').substringBefore('"')
    val (major, minor, patch) = cargoVersion.substringBefore('-').split('.').map(String::toInt)

    defaultConfig {
        applicationId = "eu.r3pr3ss10n.vively"
        minSdk = 26
        targetSdk = 36
        versionCode = major * 1_000_000 + minor * 1_000 + patch
        versionName = cargoVersion
        ndk { abiFilters += rustAbis }
    }

    val keystorePropsFile = rootProject.file("keystore.properties")
    val keystoreProps = Properties().apply {
        if (keystorePropsFile.isFile) {
            keystorePropsFile.inputStream().use { load(it) }
        }
    }

    signingConfigs {
        create("release") {
            val path = keystoreProps.getProperty("storeFile") ?: System.getenv("ANDROID_KEYSTORE_PATH")
            if (!path.isNullOrBlank()) {
                storeFile = rootProject.file(path)
                storePassword = keystoreProps.getProperty("storePassword") ?: System.getenv("ANDROID_KEYSTORE_PASSWORD")
                keyAlias = keystoreProps.getProperty("keyAlias") ?: System.getenv("ANDROID_KEY_ALIAS")
                keyPassword = keystoreProps.getProperty("keyPassword") ?: System.getenv("ANDROID_KEY_PASSWORD")
            }
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            val releaseSigning = signingConfigs.getByName("release")
            if (releaseSigning.storeFile != null) {
                signingConfig = releaseSigning
            }
        }
    }

    buildFeatures {
        compose = true
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    packaging {
        jniLibs { useLegacyPackaging = false }
    }
}

androidComponents {
    onVariants { variant ->
        val cap = variant.name.replaceFirstChar { it.uppercase() }
        val rust = tasks.register<CargoNdkTask>("cargoNdk$cap") {
            cargo.set(cargoBin)
            abis.set(rustAbis)
            release.set(variant.buildType == "release")
            repoRoot.set(repoRootDir)
            outDir.set(layout.buildDirectory.dir("rustJniLibs/${variant.name}"))
        }
        variant.sources.jniLibs?.addGeneratedSourceDirectory(rust, CargoNdkTask::outDir)
        (variant.sources.kotlin ?: variant.sources.java)
            ?.addGeneratedSourceDirectory(uniffiBindings, UniffiBindingsTask::outDir)
    }
}

dependencies {
    val composeBom = platform("androidx.compose:compose-bom:2026.09.00")
    implementation(composeBom)
    implementation("androidx.compose.runtime:runtime")
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.foundation:foundation")
    implementation("androidx.compose.ui:ui-tooling-preview")
    debugImplementation("androidx.compose.ui:ui-tooling")
    implementation("androidx.activity:activity-compose:1.13.0")

    implementation("net.java.dev.jna:jna:5.19.1@aar")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.11.0")

    implementation("io.coil-kt.coil3:coil-compose:3.6.3")
    implementation("io.coil-kt.coil3:coil-network-okhttp:3.6.3")

    implementation("androidx.media3:media3-session:1.11.1")
}
