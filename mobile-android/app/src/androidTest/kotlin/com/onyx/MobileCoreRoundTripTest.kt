package com.onyx

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import com.onyx.bridge.MobileCoreBridge
import org.junit.Assert.assertNotEquals
import org.junit.Test
import org.junit.runner.RunWith
import java.io.File

/**
 * Real instrumented proof of the Kotlin -> JNI -> Rust round trip
 * (ONYX-MOB-00 §25 step 5's actual gate for A1), intended to run on a
 * real Android emulator or device.
 *
 * # Disclosure -- not executed in the sandbox this task was built in
 * This sandbox has no `/dev/kvm` and reports zero `vmx`/`svm` CPU flags
 * (checked directly, not assumed) -- there is no way to boot an Android
 * emulator here, and no physical device is attached. This test file is
 * real and was written to actually run under `connectedAndroidTest`,
 * but it has not been executed on-device as part of this task.
 *
 * What *was* actually run and verified in this task, as the best
 * available substitute (see DECISIONS.md's A1 entry for the full
 * transcript): the identical `mobile_android_jni` native entry points
 * this test calls, loaded and invoked from a plain host JVM (OpenJDK 21,
 * linux-x86_64 build of the same crate, not cross-compiled for Android)
 * via a small Java harness -- proving the JNI marshalling and Rust glue
 * are correct, but not proving the Android-ABI cross-compiled `.so`
 * (confirmed real and built for arm64-v8a via `cargo ndk` in this same
 * task) actually loads and runs under ART on a real device.
 */
@RunWith(AndroidJUnit4::class)
class MobileCoreRoundTripTest {
    @Test
    fun nativeNewAndFree_roundTripsThroughRustWithoutCrashing() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val dbPath = File(context.cacheDir, "onyx_mobile_roundtrip_test.db").absolutePath
        // organization_id is a raw 16-byte array under this project's
        // real ObjectId serde shape (a plain derive on `struct
        // ObjectId([u8; 16])`, confirmed by reading
        // platform-kernel::identifiers directly -- not a UUID string).
        val configJson = """{"organization_id":[0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15],"cloud_relay_endpoint":"http://localhost:0"}"""

        val handle = MobileCoreBridge.nativeNew(dbPath, configJson)
        assertNotEquals("nativeNew must return a real, non-null handle", 0L, handle)

        // A malformed envelope (not a valid CommandEnvelope<Value>) is
        // documented to return null, not a JSON error string -- see
        // mobile_core_execute_command's own doc comment and
        // mobile-android-jni's identical pass-through behavior.
        val result = MobileCoreBridge.nativeExecuteCommand(handle, "{}")
        org.junit.Assert.assertNull(result)

        MobileCoreBridge.nativeFree(handle)
    }
}
