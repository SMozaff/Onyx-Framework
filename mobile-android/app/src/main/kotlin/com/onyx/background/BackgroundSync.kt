package com.onyx.background

import android.content.Context
import androidx.work.Constraints
import androidx.work.ExistingPeriodicWorkPolicy
import androidx.work.NetworkType
import androidx.work.PeriodicWorkRequestBuilder
import androidx.work.WorkManager
import com.onyx.WorkManagerService
import java.util.concurrent.TimeUnit

private const val UNIQUE_WORK_NAME = "onyx-periodic-sync"

/**
 * Schedules [WorkManagerService] to run periodically, Kotlin's
 * equivalent of `background/android/workmanager_service.dart`'s
 * `registerAndroidBackgroundSync()` -- same 15-minute period and
 * network-required constraint, confirmed directly against that
 * function rather than assumed. [ExistingPeriodicWorkPolicy.KEEP] means
 * calling this on every app start (see `MainActivity`) is a safe no-op
 * once already scheduled, matching `Workmanager().registerPeriodicTask`'s
 * own idempotent registration.
 */
fun scheduleBackgroundSync(context: Context) {
    val request = PeriodicWorkRequestBuilder<WorkManagerService>(15, TimeUnit.MINUTES)
        .setConstraints(Constraints.Builder().setRequiredNetworkType(NetworkType.CONNECTED).build())
        .build()
    WorkManager.getInstance(context)
        .enqueueUniquePeriodicWork(UNIQUE_WORK_NAME, ExistingPeriodicWorkPolicy.KEEP, request)
}
