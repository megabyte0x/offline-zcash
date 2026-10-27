plugins { kotlin("jvm"); application }
kotlin { jvmToolchain(17) }
sourceSets.main {
    kotlin.srcDir("../generated")
    kotlin.srcDir("../app/src/main/java")
    kotlin.include("uniffi/**", "org/zcash/offline/ChunkWriter.kt", "Contract.kt")
}
dependencies {
    implementation("net.java.dev.jna:jna:5.17.0")
    implementation("org.json:json:20250107")
}
application { mainClass.set("ContractKt") }
tasks.named<JavaExec>("run") {
    systemProperty("jna.library.path", rootProject.file("../target/release").absolutePath)
}
