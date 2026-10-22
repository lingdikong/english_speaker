import com.android.build.api.variant.ApplicationVariant

plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.android)
    alias(libs.plugins.kotlin.compose)
}

android {
    namespace = "app.englishspeaker"
    compileSdk = 36

    defaultConfig {
        applicationId = "app.englishspeaker"
        minSdk = 26
        targetSdk = 36
        versionCode = 1
        versionName = "0.1.0"
        ndk { abiFilters += listOf("arm64-v8a", "x86_64") }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            signingConfig = signingConfigs.getByName("debug")
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    buildFeatures { compose = true }

    // 模型直接以未压缩形式打包，首次启动复制到 filesDir（ONNX Runtime 需要真实文件路径）
    androidResources { noCompress += listOf("onnx", "bin") }

    packaging {
        jniLibs { useLegacyPackaging = false }
    }
}

kotlin {
    compilerOptions {
        jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17)
        optIn.addAll(
            "androidx.compose.animation.ExperimentalSharedTransitionApi",
            "androidx.compose.foundation.ExperimentalFoundationApi",
            "androidx.compose.material3.ExperimentalMaterial3Api",
            "androidx.media3.common.util.UnstableApi",
        )
    }
}

// 开发期：把仓库根目录的 models/ 同步到生成的 assets/models/（没有模型时为空目录 → 核心退回 Mock 引擎）
val modelsSrc = rootProject.layout.projectDirectory.dir("../../models")
abstract class SyncModels : DefaultTask() {
    @get:InputFiles @get:Optional @get:PathSensitive(PathSensitivity.RELATIVE)
    abstract val source: ConfigurableFileCollection
    @get:Internal abstract val sourceRoot: DirectoryProperty
    @get:OutputDirectory abstract val outputDir: DirectoryProperty

    @TaskAction
    fun run() {
        val out = outputDir.get().asFile.resolve("models")
        out.deleteRecursively()
        val src = sourceRoot.get().asFile
        if (src.isDirectory) src.copyRecursively(out, overwrite = true) { _, _ -> OnErrorAction.SKIP }
        // 写入版本标记：内容变化时 App 会重新复制
        val stamp = source.files.filter { it.isFile }.sortedBy { it.path }
            .joinToString("|") { "${it.name}:${it.length()}:${it.lastModified()}" }.hashCode()
        out.mkdirs()
        out.resolve("VERSION_STAMP").writeText(stamp.toString())
    }
}
val syncModels = tasks.register<SyncModels>("syncModels") {
    sourceRoot.set(modelsSrc)
    source.from(fileTree(modelsSrc) { exclude("**/.DS_Store") })
    outputDir.set(layout.buildDirectory.dir("generated/modelAssets"))
}
androidComponents {
    onVariants { variant: ApplicationVariant ->
        variant.sources.assets?.addGeneratedSourceDirectory(syncModels, SyncModels::outputDir)
    }
}

dependencies {
    implementation(project(":core"))
    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.activity.compose)
    implementation(libs.androidx.lifecycle.runtime.compose)
    implementation(libs.androidx.lifecycle.viewmodel.compose)
    implementation(platform(libs.androidx.compose.bom))
    implementation(libs.androidx.compose.ui)
    implementation(libs.androidx.compose.ui.graphics)
    implementation(libs.androidx.compose.foundation)
    implementation(libs.androidx.compose.animation)
    implementation(libs.androidx.compose.material3)
    implementation(libs.androidx.compose.ui.tooling.preview)
    debugImplementation(libs.androidx.compose.ui.tooling)
    implementation(libs.androidx.media3.session)
    implementation(libs.androidx.media3.common)
    implementation(libs.kotlinx.coroutines.android)
    implementation(libs.haze)
}
