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
| `Java_com_onyx_bridge_MobileCoreBridge_nativeSubscribeEvents` | `nativeSubscribeEvents(handle: Long, filterJson: String, callback: EventCallback): Long` | `mobile_core_subscribe_events` | Implemented (real subscription; returns the `*mut EventSubscription` as the `Long` handle, `0` on failure) — but unwired beyond `OnyxController` |
| `Java_com_onyx_bridge_MobileCoreBridge_nativeUnsubscribe` | `nativeUnsubscribe(subscription: Long)` | `mobile_core_unsubscribe` | Implemented (aborts the forwarding task and reclaims the forwarder's `GlobalRef`) |
| ~~`Java_com_onyx_bridge_MobileCoreBridge_nativeSecureStorage`~~ | ~~`nativeSecureStorage(handle, action, key, value): String?`~~ | None | **Removed** (2026-09-24): `ffi_secure_storage.rs` exports no function and the C headers contain no secure-storage symbol; Android Keystore in `SecureTokenStore.kt` is the sanctioned secret store, per `DECISIONS M11-D12.4`. A JNI function will be added only alongside a real `mobile_core_*` export |

Outside the adapter crate, `WorkManagerService.nativeAndroidDoWork(): Int` calls `mobile-core` directly rather than through `mobile-android-jni`.

## P2P transport codec (Phase 4.1, `DECISIONS P2P-1/P2P-11`)

`P2pCodec.kt` ↔ `crates/mobile-android-jni/src/p2p.rs`. This is a second JNI
surface, and unlike the `MobileCoreBridge` table it is a **real**
implementation (session handle registry, handshake state machine, framing,
AES-GCM encryption) — `Implemented` for all rows, not a stub. It replaces the
deleted placeholder C-ABI transports (`sync-transport-mobile`'s
`android_wifi_direct`/`android_ble`), as recorded in those crates' docs.

`J` = JNI adapter in `crates/mobile-android-jni/src/p2p.rs`.
`K` = Kotlin declaration in `com.onyx.p2p.P2pCodec.kt`.

| JNI native symbol | Kotlin `external fun` | Rust core | Status |
|---|---|---|---|
| `Java_com_onyx_p2p_P2pCodec_nativeSessionStart` | `nativeSessionStart(isServer: Boolean): Long` | New session, own ephemeral key | Implemented |
| `Java_com_onyx_p2p_P2pCodec_nativeSessionClientMessage` | `nativeSessionClientMessage(handle: Long): ByteArray` | client hello (65-byte pubkey) | Implemented |
| `Java_com_onyx_p2p_P2pCodec_nativeSessionAccept` | `nativeSessionAccept(clientMessage: ByteArray): Long` | server session from client hello | Implemented |
| `Java_com_onyx_p2p_P2pCodec_nativeSessionServerMessage` | `nativeSessionServerMessage(handle: Long): ByteArray` | server hello (65-byte pubkey) | Implemented |
| `Java_com_onyx_p2p_P2pCodec_nativeSessionComplete` | `nativeSessionComplete(handle: Long, serverMessage: ByteArray): Unit` | client derives keys | Implemented |
| `Java_com_onyx_p2p_P2pCodec_nativeSessionEncode` | `nativeSessionEncode(handle: Long, plaintext: ByteArray): ByteArray` | header + ciphertext + tag | Implemented |
| `Java_com_onyx_p2p_P2pCodec_nativeSessionDecode` | `nativeSessionDecode(handle: Long, frame: ByteArray): ByteArray` | strict-counter decrypt | Implemented |
| `Java_com_onyx_p2p_P2pCodec_nativeSessionClose` | `nativeSessionClose(handle: Long): Unit` | drop session/keys | Implemented |

Security notes applying to the P2P surface: session plaintext only ever
exists inside Rust (`encode`/`decode` take and return byte arrays; Kotlin
never holds key material); a failed `decode` permanently invalidates the
session (desync, not sliding-window); sessions are single-direction
stream-oriented transports scoped to the handshake's role/counter pair.

## Event subscription (2026-09-24, `DECISIONS M11-D10/M11-D12`)

`nativeSubscribeEvents` no longer has a stub-shaped signature. The design:

- The C ABI callback is the single static `extern "C" fn
  deliver_to_kotlin(context, json)`. All per-subscription state travels in
  `context`, which is a `Box<JavaEventForwarder>` (`Arc<JavaVM>` +
  `GlobalRef` to the Kotlin `EventCallback` instance). Since
  `mobile_core_subscribe_events` gained an explicit `*mut c_void`
  userdata parameter (M11-D12), the forwarder is reclaimed by
  `nativeUnsubscribe` (which aborts the delivery task first); a null
  subscription result reclaims it in `nativeSubscribeEvents` itself.
- `json` is an owned, NUL-terminated envelope buffer. The trampoline
  reclaims it up front via `CString::from_raw` so every path frees it
  exactly once — same ownership convention as Dart's cross-thread
  listener (M11-D10).
- Delivery attaches the (tokio) worker thread to the JVM per event:
  `JavaVM::attach_current_thread`, which is re-entrancy-guarded by an
  `AtomicBool`, invokes `EventCallback.onEvent(String)` (resolved by
  name/signature once at subscribe time so a typo fails fast), then the
  `AttachGuard` detaches on drop. A failing delivery is skipped, never
  fatal. The crate has no logger wired for this path (JNI entry points
  use jni's `LogErrorAndDefault`); logcat plumbing is future work.
- Kotlin side: `EventCallback` is a `fun interface { onEvent(json: String) }`
  held by the `GlobalRef`. `OnyxController` owns one org-wide
  subscription and unsubscribes from `onCleared()`. `proguard-rules.pro`
  keeps `EventCallback` so R8 cannot rename the class/method the native
  forwarder resolves by name.

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

1. ~~Decide the JNI event-callback design.~~ Resolved 2026-09-24 (M11-D12): context-carrying C ABI, static trampoline, per-event JVM attach, `EventCallback` fun interface.
2. ~~Decide whether `nativeSubscribeEvents` should return an opaque subscription token or a polling/event-bus bridge.~~ Resolved: opaque `*mut EventSubscription` as `Long`; `nativeUnsubscribe(subscription)` frees it. No Kotlin-side registry needed.
3. Wire `nativeExecuteQuery` to a real Kotlin query path and add its JSON schema/test coverage (still the one unwired "implemented" row).
4. Wire the remaining `MobileCoreBridge` declarations to real runtime call sites (import, sync status, conflicts, upload/download) beyond what `OnyxController` already drives.
5. Logcat plumbing for the async delivery path (see the event-subscription section).
6. Device-gated: real JNI round-trip through a running `EventCallback` (currently proven only at the C-ABI boundary via `ffi_integration.rs`).
