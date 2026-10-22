// 核心层：UniFFI 生成的 Kotlin 绑定 + libes_ffi.so（由 scripts/build-android.sh 生成）
plugins {
    alias(libs.plugins.android.library)
    alias(libs.plugins.kotlin.android)
}

android {
    namespace = "app.englishspeaker.core"
    compileSdk = 36
    defaultConfig {
        minSdk = 26
        consumerProguardFiles("consumer-rules.pro")
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
}

kotlin {
    compilerOptions {
        jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17)
        // 生成代码里有大量 unchecked/unused 警告，不影响使用
        suppressWarnings.set(true)
    }
}

dependencies {
    // JNA 必须用 aar（自带 libjnidispatch.so）
    api("${libs.jna.get()}@aar")
}
