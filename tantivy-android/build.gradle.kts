plugins {
    id("com.android.library")
    id("org.jetbrains.kotlin.android")
    id("maven-publish")
    signing
}

group = "com.noexcs"
version = "0.1.0"

android {
    namespace = "com.noexcs.tantivy"
    compileSdk = 36

    defaultConfig {
        minSdk = 33
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    }

    publishing {
        singleVariant("release") {
            withSourcesJar()
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
        }
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    kotlin {
        compilerOptions {
            jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17)
        }
    }

    sourceSets {
        getByName("main") {
            jniLibs.srcDirs("src/main/jniLibs")
        }
    }
}

dependencies {
    testImplementation("junit:junit:4.13.2")
    androidTestImplementation("androidx.test.ext:junit:1.2.1")
    androidTestImplementation("androidx.test:runner:1.6.2")
}

afterEvaluate {
    publishing {
        publications {
            create<MavenPublication>("release") {
                from(components["release"])
                groupId = "com.noexcs"
                artifactId = "tantivy-android"
                version = project.version.toString()

                pom {
                    name.set("tantivy-android")
                    description.set("Android BM25 full-text search library backed by Tantivy (Rust) via JNI")
                    url.set("https://github.com/noexcs/tantivy-android")
                    licenses {
                        license {
                            name.set("MIT")
                            url.set("https://opensource.org/licenses/MIT")
                        }
                    }
                    developers {
                        developer {
                            id.set("noexcs")
                            name.set("noexcs")
                        }
                    }
                    scm {
                        connection.set("scm:git:git://github.com/noexcs/tantivy-android.git")
                        developerConnection.set("scm:git:ssh://github.com:noexcs/tantivy-android.git")
                        url.set("https://github.com/noexcs/tantivy-android")
                    }
                }
            }
        }
        repositories {
            maven {
                name = "sonatype"
                url = uri(
                    (findProperty("sonatypeUrl") as String?)
                        ?: "https://s01.oss.sonatype.org/service/local/staging/deploy/maven2/"
                )
                credentials {
                    username = findProperty("ossrhUsername") as String? ?: ""
                    password = findProperty("ossrhPassword") as String? ?: ""
                }
            }
        }
    }

    signing {
        sign(publishing.publications["release"])
    }
}
