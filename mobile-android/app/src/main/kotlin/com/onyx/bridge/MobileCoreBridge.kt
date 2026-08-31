package com.onyx.bridge

/**
 * Kotlin-side declarations for the `mobile-android-jni` native entry
 * points (`crates/mobile-android-jni/src/lib.rs`), binding by JNI's
 * standard name-mangling convention (`Java_com_onyx_bridge_MobileCoreBridge_native*`).
 *
 * Handle lifecycle and one representative string round-trip
 * (`executeCommand`) only, per Work Package A1's own scope -- the
 * remaining `mobile-core` FFI surface follows this identical pattern
 * and is deliberately left for the task that actually needs each
 * function (A3 for auth/session, A4 for the core screens, etc.), not
 * built speculatively here. See `mobile-android-jni`'s own module doc
 * comment for why a dedicated JNI adapter crate exists at all instead
 * of binding directly to `mobile-core`'s plain C ABI.
 */
object MobileCoreBridge {
    external fun nativeNew(dbPath: String, configJson: String): Long
    external fun nativeFree(handle: Long)
    external fun nativeExecuteCommand(handle: Long, commandJson: String): String?
    external fun nativeSetHierarchy(handle: Long, hierarchyJson: String): Int

    // Added for A4 (core screens) -- the shared-refresh cycle's data calls.
    external fun nativeListAggregates(handle: Long, aggregateType: String): String?
    external fun nativeGetSyncStatus(handle: Long): String?
    external fun nativeListConflicts(handle: Long): String?

    // Added for A5 (Files, sync status, conflict resolution).
    external fun nativeUploadFile(handle: Long, path: String, organizationId: String, userId: String, deviceId: String): String?
    external fun nativeDownloadFile(handle: Long, contentHash: String, destinationPath: String): Long
    external fun nativeTriggerSync(handle: Long): Int
    external fun nativeResolveConflict(handle: Long, conflictJson: String, resolution: String): Int
}
