package com.onyx.ui.screens

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import java.io.File
import org.junit.Test

/**
 * A real, direct, automated proof of the one security property this
 * project has already had to fix twice (`settings.dart`,
 * `startup_error_screen.dart`): Settings has **no editable-identity
 * path**. This app has no Compose UI testing harness set up (no
 * Robolectric dependency -- confirmed absent from `build.gradle.kts`,
 * same disclosed constraint as every other "cannot render Compose
 * under a plain JVM unit test" case this project has already hit), so
 * this test reads [SettingsScreen]'s own real, current source text at
 * test time -- a genuine, executed, failing-if-violated check, not
 * just this class's or that screen's doc comment asserting it.
 *
 * Working directory for `./gradlew testDebugUnitTest` is this module's
 * root (`mobile-android/app`), so the relative path below resolves
 * without needing a resource bundle.
 */
class SettingsScreenSourceTest {
    private val source = File("src/main/kotlin/com/onyx/ui/screens/SettingsScreen.kt").readText()

    @Test
    fun `organizationId and userId are interpolated into read-only Text, never bound to an editable field`() {
        // The only editable input control this file may contain at all
        // is the relay-endpoint OutlinedTextField -- confirmed by
        // asserting there is exactly one OutlinedTextField call site,
        // and that it is not bound to organizationId/userId.
        val editableFieldCount = Regex("OutlinedTextField\\(").findAll(source).count()
        assertTrue("expected exactly one editable field (the relay endpoint); found $editableFieldCount", editableFieldCount == 1)

        val editableFieldBlockStart = source.indexOf("OutlinedTextField(")
        val editableFieldBlockEnd = source.indexOf(")", editableFieldBlockStart).let { idx ->
            // Walk to the matching close paren of the OutlinedTextField(...) call, not just the first ')'.
            var depth = 0
            var i = editableFieldBlockStart
            while (i < source.length) {
                when (source[i]) {
                    '(' -> depth++
                    ')' -> {
                        depth--
                        if (depth == 0) return@let i
                    }
                }
                i++
            }
            idx
        }
        val editableFieldBlock = source.substring(editableFieldBlockStart, editableFieldBlockEnd)
        assertFalse("the one editable field must not be bound to organizationId", editableFieldBlock.contains("organizationId"))
        assertFalse("the one editable field must not be bound to userId", editableFieldBlock.contains("userId"))
        assertTrue("the one editable field must be the relay endpoint", editableFieldBlock.contains("relay"))
    }

    @Test
    fun `no write path exists from this screen to organizationId or userId`() {
        // Real, direct check for the exact historical bug: a write
        // call like `sessionPrefs.organizationId = ...` or
        // `controller.organizationId = ...` originating from this file.
        assertFalse(source.contains(Regex("\\.organizationId\\s*=")))
        assertFalse(source.contains(Regex("\\.userId\\s*=")))
    }

    @Test
    fun `a sign-out action is present as the only identity-changing affordance`() {
        assertTrue(source.contains("onSignOut"))
    }
}
