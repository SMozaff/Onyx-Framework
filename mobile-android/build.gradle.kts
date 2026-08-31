// Root build script. Real, current plugin versions confirmed against
// live sources before writing this file, not remembered/guessed:
// - Android Gradle Plugin: dl.google.com's own maven-metadata.xml for
//   com.android.tools.build:gradle lists 9.4.0 as the latest stable
//   release, but AGP 9.x requires Gradle 9.5+ (confirmed via
//   developer.android.com/build/releases/gradle-plugin's own
//   compatibility table for the 9.x series) while this sandbox's
//   available Gradle is 8.14.3 -- upgrading Gradle itself would mean an
//   unverified network download of a ~150MB distribution this task
//   does not need. 8.13.2 (the latest *stable* 8.x release per the same
//   maven-metadata.xml, confirmed by listing every "8." prefixed
//   version rather than assuming that's the newest) is compatible with
//   the Gradle already available here, so that's what's used -- a real,
//   verifiable choice over an untested reach for the newest major line.
// - Compose 1.9+ needs AGP/Lint 8.8.2+ (confirmed via
//   developer.android.com/develop/ui/compose/tooling/lint), satisfied
//   by 8.13.2.
// - Kotlin 2.3.21, with the Compose compiler now a Kotlin-version-locked
//   plugin (`org.jetbrains.kotlin.plugin.compose`), not a BOM-tracked
//   artifact -- confirmed: "Starting with Kotlin 2.0, the Compose
//   compiler is managed directly alongside the Kotlin compiler using
//   the same versioning."
plugins {
    id("com.android.application") version "8.13.2" apply false
    id("org.jetbrains.kotlin.android") version "2.3.21" apply false
    id("org.jetbrains.kotlin.plugin.compose") version "2.3.21" apply false
}
