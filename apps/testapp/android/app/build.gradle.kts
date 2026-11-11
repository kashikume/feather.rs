import javax.inject.Inject

plugins {
    alias(libs.plugins.android.application)
}

android {
    namespace = "com.kashikume.feather.testapp"
    compileSdk = 37

    defaultConfig {
        applicationId = "com.kashikume.feather.testapp"
        minSdk = 30
        targetSdk = 37
        versionCode = 1
        versionName = "0.0.1"

        ndk {
            abiFilters += "arm64-v8a"
        }
    }
}

dependencies {
    implementation(libs.games.activity)
    implementation(libs.appcompat)
    implementation(libs.core.ktx)
}

/**
 * Builds the app's Rust library for Android with `cargo ndk`, and copies the assets `build.rs`
 * made. Its two output directories become the APK's native libraries and assets (see
 * `androidComponents` below), so nothing is written into `src/`.
 */
abstract class CargoNdkTask : DefaultTask() {
    @get:Inject
    abstract val exec: ExecOperations

    @get:Inject
    abstract val files: FileSystemOperations

    /** The Cargo workspace's root, where `cargo` runs. */
    @get:Internal
    abstract val workspaceDir: DirectoryProperty

    /** Whether Rust is built optimized (`--release`). */
    @get:Input
    abstract val release: Property<Boolean>

    /** `<abi>/libfeather_testapp.so`, laid out as the APK's `lib/` directory. */
    @get:OutputDirectory
    abstract val jniLibsDir: DirectoryProperty

    /** A fresh copy of the assets, so none left over from older builds go into the APK. */
    @get:OutputDirectory
    abstract val assetsDir: DirectoryProperty

    @TaskAction
    fun build() {
        val workspace = workspaceDir.get().asFile
        val profile = if (release.get()) "release" else "debug"
        exec.exec {
            workingDir = workspace
            commandLine(
                buildList {
                    addAll(listOf("cargo", "ndk", "-t", "arm64-v8a", "-P", "30"))
                    addAll(listOf("-o", jniLibsDir.get().asFile.absolutePath))
                    addAll(listOf("build", "-p", "testapp", "--lib"))
                    if (release.get()) add("--release")
                },
            )
        }
        files.sync {
            from(workspace.resolve("target/aarch64-linux-android/$profile/assets"))
            into(assetsDir)
        }
    }
}

androidComponents {
    onVariants { variant ->
        val name = variant.name.replaceFirstChar { it.uppercase() }
        val cargo = tasks.register<CargoNdkTask>("cargoNdk$name") {
            workspaceDir.set(rootProject.layout.projectDirectory.dir("../../.."))
            // Rust is built optimized unless -Pfeather.rustProfile=debug: unoptimized Rust is
            // too slow for most demos on a phone.
            release.set(
                providers.gradleProperty("feather.rustProfile").map { it != "debug" }.orElse(true),
            )
            // Cargo decides what to rebuild; Gradle runs it every time.
            outputs.upToDateWhen { false }
        }
        variant.sources.jniLibs?.addGeneratedSourceDirectory(cargo, CargoNdkTask::jniLibsDir)
        variant.sources.assets?.addGeneratedSourceDirectory(cargo, CargoNdkTask::assetsDir)
    }
}
