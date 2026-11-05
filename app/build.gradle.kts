plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose")
}

android {
    namespace = "app.noted"
    compileSdk = 35
    buildToolsVersion = "35.0.0"

    defaultConfig {
        applicationId = "app.noted"
        minSdk = 30
        targetSdk = 35
        versionCode = 3
        versionName = "0.2.1"
    }

    signingConfigs {
        // The release key is never committed. CI decodes it from the ANDROID_KEYSTORE_BASE64
        // secret; locally, put keystore.properties (and the .jks it names) in the repo root.
        val props = java.util.Properties().apply {
            rootProject.file("keystore.properties").takeIf { it.exists() }?.inputStream()?.use { load(it) }
        }
        val storePath = System.getenv("NOTED_KEYSTORE") ?: props.getProperty("storeFile")
        if (storePath != null) {
            create("release") {
                storeFile = rootProject.file(storePath)
                storePassword = System.getenv("NOTED_KEYSTORE_PASSWORD") ?: props.getProperty("storePassword")
                keyAlias = System.getenv("NOTED_KEY_ALIAS") ?: props.getProperty("keyAlias") ?: "noted"
                keyPassword = System.getenv("NOTED_KEY_PASSWORD") ?: props.getProperty("keyPassword")
            }
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
            signingConfig = signingConfigs.findByName("release") ?: signingConfigs.getByName("debug")
        }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions { jvmTarget = "17" }
    buildFeatures { compose = true }
}

dependencies {
    val bom = platform("androidx.compose:compose-bom:2024.12.01")
    implementation(bom)
    implementation("androidx.core:core-ktx:1.15.0")
    implementation("androidx.activity:activity-compose:1.9.3")
    implementation("androidx.lifecycle:lifecycle-viewmodel-compose:2.8.7")
    implementation("androidx.lifecycle:lifecycle-runtime-compose:2.8.7")
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.foundation:foundation")
    implementation("androidx.compose.material3:material3")
}
