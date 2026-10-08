package com.onyx.background

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import androidx.work.NetworkType
import androidx.work.WorkInfo
import androidx.work.WorkManager
import androidx.work.testing.WorkManagerTestInitHelper
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Real instrumented proof of the Phase 4.2 scheduling contract
 * (`scheduleBackgroundSync` → WorkManager): the work is alone under its
 * unique name (KEEP is idempotent), is a periodic `[WorkManagerService]
 *`-typed request, and carries the `NetworkType.CONNECTED` constraint —
 * the same guarantees `registerAndroidBackgroundSync()` had in the frozen
 * Flutter app. Intended to run under `connectedAndroidTest`.
 *
 * # Disclosure -- not executed in the sandbox this phase was built in
 * Same environment limitation as `MobileCoreRoundTripTest` (no `/dev/kvm`,
 * no device): this test is written to actually run on a real Android
 * emulator/device but has not been executed on-device yet. On-device
 * execution (including Doze/airplane-mode *behavior*, which no instrumented
 * test can prove) remains MIGRATION_PLAN Phase 4.2's hardware gate.
 */
@RunWith(AndroidJUnit4::class)
class BackgroundSyncInstrumentedTest {

    @Before
    fun initWorkManagerForTest() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        WorkManagerTestInitHelper.initializeTestWorkManager(context)
    }

    @Test
    fun scheduleBackgroundSync_enqueuesUniquePeriodicWorkWithConnectedConstraint() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext

        // Called twice to prove ExistingPeriodicWorkPolicy.KEEP is
        // idempotent: the second enqueue must not create a second work item.
        scheduleBackgroundSync(context)
        scheduleBackgroundSync(context)

        val infos = WorkManager.getInstance(context)
            .getWorkInfosForUniqueWork(UNIQUE_WORK_NAME)
            .get()
        assertEquals("KEEP must not duplicate the unique periodic work", 1, infos.size)

        val info = infos[0]
        assertNotNull("work must have an id", info.id)
        assertEquals(WorkInfo.State.ENQUEUED, info.state)
        assertEquals(
            "periodic sync must require a connected network",
            NetworkType.CONNECTED,
            info.constraints.requiredNetworkType,
        )

        // The work must be the real periodic sync task, not a ghost entry.
        assertEquals(
            "the scheduled work must be periodic",
            WorkInfo.PeriodicityInfo.PERIODIC,
            info.periodicityInfo,
        )
    }
}