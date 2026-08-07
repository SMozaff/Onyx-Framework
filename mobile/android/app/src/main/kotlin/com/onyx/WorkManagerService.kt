package com.onyx

import android.content.Context
import androidx.work.CoroutineWorker
import androidx.work.WorkerParameters

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
