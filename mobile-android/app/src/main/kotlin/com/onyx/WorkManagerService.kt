package com.onyx

import android.content.Context
import androidx.work.CoroutineWorker
import androidx.work.WorkerParameters

/**
 * Background sync entry point for A5, calling straight into
 * `mobile-core`'s existing, already Android-specific plumbing rather
 * than inventing a different background-task mechanism -- per A5's own
 * instruction. This is a near-verbatim port of the *already real*
 * Kotlin file this project ships for the frozen Flutter app's own
 * Android embedding (`mobile/android/app/src/main/kotlin/com/onyx/
 * WorkManagerService.kt`), same package (`com.onyx`, matching the
 * fixed JNI symbol name below) and same body -- confirmed by reading
 * that file directly, not written independently.
 *
 * [nativeAndroidDoWork] is `mobile-core`'s
 * `Java_com_onyx_WorkManagerService_nativeAndroidDoWork`, which calls
 * `mobile_core_background_sync_registered` against whatever `MobileApp`
 * handle is currently registered as this *process*'s active instance
 * (`mobile_core_new` registers it, `mobile_core_free` clears it) --
 * see `crates/mobile-core/src/lib.rs`'s `REGISTERED_BACKGROUND_APP`
 * doc comment. Concretely: this worker only does real work while the
 * app's own session (`OnyxSessionViewModel`) is holding an open handle
 * in this same process; if Android has killed the process since the
 * last open session, there is no registered handle and this correctly
 * no-ops (`Result.retry()`), the same honest "nothing real to sync
 * without a live identity" limitation Dart's own background dispatcher
 * documents for its own, differently-shaped reason (no saved,
 * logged-in session) -- see this class's own module-level doc comment
 * in `DECISIONS.md`'s A5 entry for the real, current scope comparison.
 */
class WorkManagerService(context: Context, params: WorkerParameters) :
    CoroutineWorker(context, params) {

    companion object {
        init { System.loadLibrary("mobile_core") }
        @JvmStatic external fun nativeAndroidDoWork(): Int
    }

    override suspend fun doWork(): Result = when (nativeAndroidDoWork()) {
        1 -> Result.success()
        else -> Result.retry()
    }
}
