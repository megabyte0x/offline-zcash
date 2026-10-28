plugins { id("com.android.application"); kotlin("android") }

android {
    namespace = "org.zcash.offline"
    compileSdk = 35
    defaultConfig {
        applicationId = "org.zcash.offline.test"
        minSdk = 29
        targetSdk = 35
        versionCode = 11
        versionName = "0.7.2"
        ndk { abiFilters += "arm64-v8a" }
    }
    sourceSets["main"].java.srcDir("../generated")
    sourceSets["main"].jniLibs.srcDir("../jniLibs")
    signingConfigs {
        create("regtest") {
            storeFile = file("../test-signing.p12")
            storePassword = "android"
            keyAlias = "androiddebugkey"
            keyPassword = "android"
        }
    }
    buildTypes {
        // Public test key: stable across CI runs so upgrades preserve wallets. Never use for real funds.
        getByName("release") { signingConfig = signingConfigs.getByName("regtest"); isMinifyEnabled = false }
        getByName("debug") { signingConfig = signingConfigs.getByName("regtest") }
    }
    compileOptions { sourceCompatibility = JavaVersion.VERSION_17; targetCompatibility = JavaVersion.VERSION_17 }
    kotlinOptions { jvmTarget = "17" }
    packaging { resources.excludes += "META-INF/AL2.0"; resources.excludes += "META-INF/LGPL2.1" }
}
dependencies { implementation("net.java.dev.jna:jna:5.17.0@aar") }
