package com.onyx.controller

import android.content.Context
import android.net.ConnectivityManager
import android.net.NetworkCapabilities
import android.util.Log
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import com.onyx.bridge.MobileCoreBridge
import com.onyx.model.CommandEnvelopeFactory
import com.onyx.model.ConflictChoice
import com.onyx.model.LoadedAggregate
import com.onyx.model.SyncConflict
import com.onyx.model.SyncSnapshot
import com.onyx.util.UuidCodec
import kotlinx.coroutines.Deferred
import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.coroutines.Dispatchers
import org.json.JSONArray
import org.json.JSONObject

private const val TAG = "OnyxController"

/**
 * The single source of truth for all cross-screen state, Kotlin's port
 * of `ui/app.dart`'s `OnyxController` (a `ChangeNotifier` there; a
 * `ViewModel` exposing `StateFlow`s here, per the current, real Android
 * pattern confirmed via Context7 -- `viewModel()` returns the same
 * instance to every composable scoped to the same owner, and
 * `StateFlow`/`collectAsStateWithLifecycle()` is the documented current
 * replacement for `ChangeNotifier`/`context.watch`).
 *
 * # The one architectural property every screen depends on
 * [refresh] is the *only* place that fans out real FFI calls for list
 * data, firing exactly the same six calls Dart's `refresh()` does, in
 * parallel (`async`/`awaitAll`, mirroring `Future.wait`): mission,
 * task, approval, notification, sync status, conflicts. Every real
 * screen (A4: Dashboard, Missions, Tasks, Notifications) reads
 * [missions]/[tasks]/[notifications]/[sync]/[conflicts] directly from
 * this already-loaded state -- **no screen may independently call
 * [MobileCoreBridge.nativeListAggregates] itself.** This is not a
 * style preference: Dart's own architecture treats a partial-success
 * fan-out as impossible by design (all six lists are only assigned
 * after every call succeeds), and duplicating list-fetching per screen
 * would both violate that invariant and multiply real FFI/JNI calls
 * per refresh cycle -- the exact regression A4's own instructions
 * warn against.
 *
 * `approvals` is fetched (mirroring Dart's `listAggregates('approval')`
 * call) but deliberately exposed by no field a screen reads, matching
 * the parity matrix's own finding (§6): no real screen, including
 * Dart's own `ApprovalsScreen`, ever reads it -- approvals are computed
 * as an in-memory filter over [missions]/[tasks] instead (see
 * `ApprovalsFilter` if/when that screen is built). The call itself is
 * still made every cycle, since Dart's `Future.wait` treats it as part
 * of the same atomic fan-out (a failure there fails the whole refresh),
 * and this class must reproduce that failure behavior exactly, not
 * just the fields a screen happens to read today.
 */
class OnyxController(
    private val handle: Long,
    private val envelopeFactory: CommandEnvelopeFactory,
    private val applicationContext: Context,
) : ViewModel() {
    /** Read-only for Settings' "Signed in" display -- see that screen's own doc comment on why this is never editable. */
    val organizationId: String get() = envelopeFactory.organizationId
    val userId: String get() = envelopeFactory.userId

    private val _missions = MutableStateFlow<List<LoadedAggregate>>(emptyList())
    val missions: StateFlow<List<LoadedAggregate>> = _missions.asStateFlow()

    private val _tasks = MutableStateFlow<List<LoadedAggregate>>(emptyList())
    val tasks: StateFlow<List<LoadedAggregate>> = _tasks.asStateFlow()

    private val _notifications = MutableStateFlow<List<LoadedAggregate>>(emptyList())
    val notifications: StateFlow<List<LoadedAggregate>> = _notifications.asStateFlow()

    private val _sync = MutableStateFlow(SyncSnapshot.EMPTY)
    val sync: StateFlow<SyncSnapshot> = _sync.asStateFlow()

    private val _conflictCount = MutableStateFlow(0)
    val conflictCount: StateFlow<Int> = _conflictCount.asStateFlow()

    /** Full conflict list -- added for A5's conflict resolution dialog (A4 only needed the count). */
    private val _conflicts = MutableStateFlow<List<SyncConflict>>(emptyList())
    val conflicts: StateFlow<List<SyncConflict>> = _conflicts.asStateFlow()

    /** Mirrors `ui/app.dart`'s `OnyxController.isSyncing`: true only while a manual [triggerSync] call is in flight. */
    private val _isSyncing = MutableStateFlow(false)
    val isSyncing: StateFlow<Boolean> = _isSyncing.asStateFlow()

    /**
     * Mirrors `ui/app.dart`'s `hasNetwork`, computed the same way each
     * [refresh] cycle: real device-level connectivity (any active
     * network with internet capability), not `sync.online` (which
     * reflects whether `mobile-core`'s own sync agent found a reachable
     * peer/relay -- a materially different, narrower question). Default
     * `true`, exactly matching Dart's own initial value.
     */
    private val _hasNetwork = MutableStateFlow(true)
    val hasNetwork: StateFlow<Boolean> = _hasNetwork.asStateFlow()

    private val _isLoading = MutableStateFlow(true)
    val isLoading: StateFlow<Boolean> = _isLoading.asStateFlow()

    private val _error = MutableStateFlow<String?>(null)
    val error: StateFlow<String?> = _error.asStateFlow()

    /** Tracks how many real refresh cycles have completed -- used only by
     * the instrumented test proving the single-refresh-per-cycle
     * property (A4's own verification requirement), not by any screen. */
    private val _refreshCount = MutableStateFlow(0)
    val refreshCount: StateFlow<Int> = _refreshCount.asStateFlow()

    init {
        refresh()
    }

    /**
     * Fans out the same six calls Dart's `refresh()` does, in parallel,
     * all-or-nothing: if any call throws, [error] is set and none of
     * the six fields below are updated for this cycle (mirrors Dart's
     * `try` block assigning all six only after every call resolves).
     * [isLoading] is only ever set to `false` after the very first
     * successful-or-failed cycle completes -- it starts `true` and is
     * never reset to `true` again, exactly matching Dart's own
     * documented behavior that a later `refresh()` (from a mutation or
     * pull-to-refresh) never re-triggers a full-screen loading spinner.
     */
    fun refresh() {
        viewModelScope.launch {
            try {
                val results = withContext(Dispatchers.Default) {
                    val missionsDeferred: Deferred<List<LoadedAggregate>> = async { listAggregates("mission") }
                    val tasksDeferred: Deferred<List<LoadedAggregate>> = async { listAggregates("task") }
                    val approvalsDeferred: Deferred<List<LoadedAggregate>> = async { listAggregates("approval") }
                    val notificationsDeferred: Deferred<List<LoadedAggregate>> = async { listAggregates("notification") }
                    val syncDeferred: Deferred<SyncSnapshot> = async { getSyncStatus() }
                    val conflictsDeferred: Deferred<List<SyncConflict>> = async { listConflicts() }
                    listOf(missionsDeferred, tasksDeferred, approvalsDeferred, notificationsDeferred, syncDeferred, conflictsDeferred)
                        .awaitAll()
                }
                @Suppress("UNCHECKED_CAST")
                _missions.value = results[0] as List<LoadedAggregate>
                @Suppress("UNCHECKED_CAST")
                _tasks.value = results[1] as List<LoadedAggregate>
                // results[2] (approvals) intentionally not stored -- see class doc comment.
                @Suppress("UNCHECKED_CAST")
                _notifications.value = results[3] as List<LoadedAggregate>
                _sync.value = results[4] as SyncSnapshot
                @Suppress("UNCHECKED_CAST")
                val conflicts = results[5] as List<SyncConflict>
                _conflicts.value = conflicts
                _conflictCount.value = conflicts.size
                _hasNetwork.value = deviceHasNetwork()
                _error.value = null
            } catch (e: Exception) {
                Log.w(TAG, "refresh() failed", e)
                _error.value = e.message ?: e.toString()
            } finally {
                _isLoading.value = false
                _refreshCount.value += 1
            }
        }
    }

    private fun listAggregates(aggregateType: String): List<LoadedAggregate> {
        val json = MobileCoreBridge.nativeListAggregates(handle, aggregateType)
            ?: throw IllegalStateException("nativeListAggregates($aggregateType) returned null")
        val array = JSONArray(json)
        return (0 until array.length()).map { LoadedAggregate.fromJson(array.getJSONObject(it)) }
    }

    private fun getSyncStatus(): SyncSnapshot {
        val json = MobileCoreBridge.nativeGetSyncStatus(handle)
            ?: throw IllegalStateException("nativeGetSyncStatus returned null")
        return SyncSnapshot.fromJson(JSONObject(json))
    }

    private fun listConflicts(): List<SyncConflict> {
        val json = MobileCoreBridge.nativeListConflicts(handle)
            ?: throw IllegalStateException("nativeListConflicts returned null")
        val array = JSONArray(json)
        return (0 until array.length()).map { SyncConflict.fromJson(array.getJSONObject(it)) }
    }

    /**
     * Real device-level connectivity, mirroring `ui/app.dart`'s own
     * `connectivity_plus`-based check (`results.any((r) => r !=
     * ConnectivityResult.none)`) via the platform API it wraps directly
     * -- no extra dependency needed for this one query.
     */
    private fun deviceHasNetwork(): Boolean {
        val manager = applicationContext.getSystemService(Context.CONNECTIVITY_SERVICE) as? ConnectivityManager
            ?: return true
        val network = manager.activeNetwork ?: return false
        val capabilities = manager.getNetworkCapabilities(network) ?: return false
        return capabilities.hasCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET)
    }

    /** Mirrors `missions.dart`'s `createMission`: execute, then a full [refresh]. */
    fun createMission(name: String, description: String?) {
        viewModelScope.launch {
            val payload = JSONObject().put(
                "CreateMission",
                JSONObject()
                    .put("name", name)
                    .put("description", description ?: JSONObject.NULL)
                    .put("owner_id", JSONArray(UuidCodec.uuidToBytes(envelopeFactory.userId))),
            )
            executeAndRefresh(commandType = "CreateMission", targetType = "mission", targetId = UuidCodec.randomUuid(), payload = payload)
        }
    }

    /** Mirrors `tasks.dart`'s `createTask`: execute, then a full [refresh]. */
    fun createTask(missionId: String, title: String, description: String?) {
        viewModelScope.launch {
            val payload = JSONObject().put(
                "CreateTask",
                JSONObject()
                    .put("mission_id", JSONArray(UuidCodec.uuidToBytes(missionId)))
                    .put("title", title)
                    .put("description", description ?: JSONObject.NULL)
                    .put("owner_id", JSONArray(UuidCodec.uuidToBytes(envelopeFactory.userId))),
            )
            executeAndRefresh(commandType = "CreateTask", targetType = "task", targetId = UuidCodec.randomUuid(), payload = payload)
        }
    }

    /**
     * Mirrors Mission Detail's/Task Detail's `controller.decide()`:
     * one generic method parameterized by `commandType`/`targetType`,
     * matching Dart's own real design (both aggregates share the
     * identical `{reason}` payload shape and owner-authority gate, even
     * though their command *names* differ -- `ActivateMission`/
     * `RejectApproval` for Mission, `ApproveTask`/`RejectTask` for
     * Task, confirmed in `mission_detail.dart`/`task_detail.dart`
     * directly). Returns the raw success/failure JSON so the caller
     * (a screen's own local state) can show the same inline
     * pop-on-success / red-text-on-failure behavior Dart's screens do.
     */
    suspend fun decide(target: LoadedAggregate, targetType: String, commandType: String, reason: String): JSONObject {
        val payload = JSONObject().put(commandType, JSONObject().put("reason", reason))
        val envelope = envelopeFactory.create(
            commandType = commandType,
            targetType = targetType,
            targetId = target.id,
            payload = payload,
            expectedVersion = target.version,
            lifecycleEpoch = target.lifecycleEpoch,
            authorityEpoch = target.authorityEpoch,
        )
        val resultJson = withContext(Dispatchers.Default) {
            MobileCoreBridge.nativeExecuteCommand(handle, envelope.toString())
        } ?: throw IllegalStateException("nativeExecuteCommand returned null (malformed command envelope)")
        val result = JSONObject(resultJson)
        refresh()
        return result
    }

    private suspend fun executeAndRefresh(commandType: String, targetType: String, targetId: String, payload: JSONObject) {
        val envelope = envelopeFactory.create(commandType = commandType, targetType = targetType, targetId = targetId, payload = payload)
        withContext(Dispatchers.Default) {
            MobileCoreBridge.nativeExecuteCommand(handle, envelope.toString())
        }
        refresh()
    }

    /**
     * Mirrors `files.dart`'s `_upload`: uploads the file at [path],
     * returning the real `UploadOutcome` JSON (`content_hash`,
     * `size_bytes`, ...) on success. Throws on any failure -- including
     * the file exceeding `file_domain::value::MAX_FILE_SIZE_BYTES` (100
     * MiB) -- with the same real, current *generic* signal Dart's own
     * screen gets: `mobile_core_upload_file` collapses every failure
     * mode (I/O error, oversized file, coordinator error) into a null
     * return with no further detail (confirmed by reading
     * `ffi_files.rs` directly), so this is honest parity with Dart's
     * actual behavior, not a regression from some richer error Dart
     * secretly has and this task forgot to wire up.
     */
    suspend fun uploadFile(path: String): JSONObject {
        val json = withContext(Dispatchers.IO) {
            MobileCoreBridge.nativeUploadFile(handle, path, envelopeFactory.organizationId, envelopeFactory.userId, envelopeFactory.deviceId)
        } ?: throw IllegalStateException("Upload failed (unreadable file, over the 100 MiB size limit, or a storage error)")
        return JSONObject(json)
    }

    /** Mirrors `files.dart`'s `_download`: same generic failure signal as [uploadFile] -- see its doc comment. */
    suspend fun downloadFile(contentHash: String, destinationPath: String): Long {
        val bytesWritten = withContext(Dispatchers.IO) {
            MobileCoreBridge.nativeDownloadFile(handle, contentHash, destinationPath)
        }
        if (bytesWritten < 0) throw IllegalStateException("Download failed (no stored content for that hash, or a write error)")
        return bytesWritten
    }

    /** Mirrors `background/sync_service.dart`'s `startSync`/the sync status widget's manual "tap to synchronize now" action. */
    fun triggerSync() {
        if (_isSyncing.value) return
        viewModelScope.launch {
            _isSyncing.value = true
            try {
                val code = withContext(Dispatchers.Default) { MobileCoreBridge.nativeTriggerSync(handle) }
                if (code != 0) _error.value = "Sync failed with code $code"
                refresh()
            } finally {
                _isSyncing.value = false
            }
        }
    }

    /**
     * Mirrors `widgets/conflict_dialog.dart`'s `resolve`: sends
     * [choice] for [conflict] and refreshes. All three real resolution
     * choices (accept local, accept remote, escalate) go through this
     * one path, matching `mobile_core_resolve_conflict`'s own
     * `"local"`/`"remote"`/`"escalate"` string match.
     */
    suspend fun resolveConflict(conflict: SyncConflict, choice: ConflictChoice) {
        val code = withContext(Dispatchers.Default) {
            MobileCoreBridge.nativeResolveConflict(handle, conflict.raw.toString(), choice.wireValue)
        }
        if (code != 0) throw IllegalStateException("Conflict resolution failed with code $code")
        refresh()
    }

    class Factory(
        private val handle: Long,
        private val organizationId: String,
        private val userId: String,
        private val applicationContext: Context,
    ) : ViewModelProvider.Factory {
        @Suppress("UNCHECKED_CAST")
        override fun <T : ViewModel> create(modelClass: Class<T>): T {
            return OnyxController(handle, CommandEnvelopeFactory(organizationId, userId), applicationContext) as T
        }
    }
}
