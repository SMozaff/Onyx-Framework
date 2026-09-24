# Android JNI Contract

**Document:** `docs/mobile-migration/android-jni-contract.md`
**Date:** 2026-09-23
**Status:** PHASE 0 CONTRACT — DECLARATIONS PLUS IMPLEMENTATION STATUS

The JNI layer must contain no business logic. Its only responsibilities are native-library loading, handle management, value marshalling, ownership transfer, callback translation, and error translation.

## Native libraries

- `OnyxApplication` loads `mobile_android_jni` at application initialization.
  - Source: `mobile-android/app/src/main/kotlin/com/onyx/OnyxApplication.kt`
- `WorkManagerService` separately loads `mobile_core` for background execution.
  - Source: `mobile-android/app/src/main/kotlin/com/onyx/WorkManagerService.kt`

## Function inventory

`J` means a JNI adapter function in `crates/mobile-android-jni/src/lib.rs`.  
`K` means a matching Kotlin declaration in `MobileCoreBridge.kt`.  
`C` means the underlying `mobile-core` C ABI function.

| Native entry point | Kotlin declaration | Underlying mobile-core function | Status |
|---|---|---|---|
| `Java_com_onyx_bridge_MobileCoreBridge_nativeNew` | `nativeNew(dbPath: String, configJson: String): Long` | `mobile_core_new` | Implemented |
| `Java_com_onyx_bridge_MobileCoreBridge_nativeFree` | `nativeFree(handle: Long)` | `mobile_core_free` | Implemented |
| `Java_com_onyx_bridge_MobileCoreBridge_nativeSetHierarchy` | `nativeSetHierarchy(handle: Long, hierarchyJson: String): Int` | `mobile_core_set_hierarchy` | Implemented |
| `Java_com_onyx_bridge_MobileCoreBridge_nativeExecuteCommand` | `nativeExecuteCommand(handle: Long, commandJson: String): String?` | `mobile_core_execute_command` | Implemented |
| `Java_com_onyx_bridge_MobileCoreBridge_nativeListAggregates` | `nativeListAggregates(handle: Long, aggregateType: String): String?` | `mobile_core_list_aggregates` | Implemented |
| `Java_com_onyx_bridge_MobileCoreBridge_nativeGetSyncStatus` | `nativeGetSyncStatus(handle: Long): String?` | `mobile_core_get_sync_status` | Implemented |
| `Java_com_onyx_bridge_MobileCoreBridge_nativeListConflicts` | `nativeListConflicts(handle: Long): String?` | `mobile_core_list_conflicts` | Implemented |
| `Java_com_onyx_bridge_MobileCoreBridge_nativeUploadFile` | `nativeUploadFile(handle: Long, path: String, organizationId: String, userId: String, deviceId: String): String?` | `mobile_core_upload_file` | Implemented |
| `Java_com_onyx_bridge_MobileCoreBridge_nativeDownloadFile` | `nativeDownloadFile(handle: Long, contentHash: String, destinationPath: String): Long` | `mobile_core_download_file` | Implemented |
| `Java_com_onyx_bridge_MobileCoreBridge_nativeTriggerSync` | `nativeTriggerSync(handle: Long): Int` | `mobile_core_trigger_sync` | Implemented |
| `Java_com_onyx_bridge_MobileCoreBridge_nativeResolveConflict` | `nativeResolveConflict(handle: Long, conflictJson: String, resolution: String): Int` | `mobile_core_resolve_conflict` | Implemented |
| `Java_com_onyx_bridge_MobileCoreBridge_nativeExecuteQuery` | `nativeExecuteQuery(handle: Long, queryJson: String): String?` | `mobile_core_execute_query` | Implemented pass-through, but unwired: no Kotlin runtime call site outside the bridge declaration |
| `Java_com_onyx_bridge_MobileCoreBridge_nativeSubscribeEvents` | `nativeSubscribeEvents(handle: Long, filterJson: String): Long` | Intended: `mobile_core_subscribe_events` | **Stub:** ignores the filter and returns placeholder `0`; does not establish a subscription |
| `Java_com_onyx_bridge_MobileCoreBridge_nativeUnsubscribe` | `nativeUnsubscribe(handle: Long)` | Intended: `mobile_core_unsubscribe` | **Stub:** currently a no-op and does not free a subscription |
| `Java_com_onyx_bridge_MobileCoreBridge_nativeSecureStorage` | `nativeSecureStorage(handle: Long, action: String, key: String, value: String): String?` | None | **Stub:** `mobile-core/src/ffi_secure_storage.rs` exports no secure-storage function and explicitly remains unimplemented |

Outside the adapter crate, `WorkManagerService.nativeAndroidDoWork(): Int` calls `mobile-core` directly rather than through `mobile-android-jni`.

## Marshalling conventions

- An opaque `*mut MobileApp` is represented in Kotlin as `Long`.
- JNI strings are converted to NUL-terminated Rust `CString` values before calling `mobile-core`.
- Rust-owned `*mut c_char` results are copied into JVM-owned strings before `mobile_core_free_string` is called.
- Null results remain `null` in Kotlin. A null result means a malformed FFI call, not a domain-level rejection.
- Integer/status conventions are passed through unchanged:
  - `0` means success where the underlying Rust function uses that convention.
  - `-1` means failure where the underlying Rust function uses that convention.
  - Download returns bytes written or `-1`.

## Open items before this contract is complete

1. Decide the JNI event-callback design. The underlying C function uses an `extern "C" fn(*const c_char)` callback. The stub has no callback parameter and cannot be treated as implemented.
2. Decide whether `nativeSubscribeEvents` should return an opaque subscription token or the Kotlin side should use a polling/event-bus bridge instead.
3. Add a handle-to-subscription registry if opaque subscription handles are retained.
4. Decide secure storage direction: keep Android Keystore handling in `SecureTokenStore.kt` and remove/deprecate `nativeSecureStorage`, or define a real Rust-backed secure-storage interface first.
5. Wire `nativeExecuteQuery` to a real Kotlin query path and add its JSON schema/test coverage.
