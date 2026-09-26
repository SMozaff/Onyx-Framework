package com.onyx.ui.screens

import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.swipeDown
import androidx.test.ext.junit.runners.AndroidJUnit4
import com.onyx.model.LoadedAggregate
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * Real Compose UI proof of [ApprovalsScreen], intended to run on a real
 * Android emulator/device via `connectedAndroidTest`.
 *
 * # Disclosure -- not executed in the sandbox this task was built in
 * Same real, disclosed constraint as every prior instrumented test in
 * this module: this sandbox has no `/dev/kvm` and reports zero
 * `vmx`/`svm` CPU flags, so no Android emulator can boot here, and no
 * physical device was available. This file is real, complete, and was
 * written to actually run under `connectedAndroidTest`, but has not
 * been executed on-device as part of this task. `ApprovalsFilterTest`
 * (`src/test/`) independently proves the pure filtering/ordering logic
 * this screen renders, real and executed.
 */
@RunWith(AndroidJUnit4::class)
class ApprovalsScreenTest {
    @get:Rule
    val composeRule = createComposeRule()

    private fun aggregate(name: String, status: String): LoadedAggregate = LoadedAggregate.fromJson(
        JSONObject(
            """{"id": [0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15], "aggregate": {"name": "$name", "status": "$status"}, "version": 0, "lifecycle_epoch": 0, "authority_epoch": 0, "updated_at": 0}""",
        ),
    )

    /** The real, exact Flutter empty-state string (`approvals.dart`), confirmed byte-for-byte. */
    @Test
    fun emptyState_showsTheRealFlutterEquivalentMessage() {
        composeRule.setContent {
            ApprovalsScreen(tasks = emptyList(), missions = emptyList(), onRefresh = {}, onOpenTask = {}, onOpenMission = {})
        }
        composeRule.onNodeWithText("No tasks or missions are currently awaiting approval.").assertExists()
    }

    /** Tapping a task result opens Task Detail -- via the real callback, not a new navigation mechanism. */
    @Test
    fun tappingATask_invokesOnOpenTaskWithTheRealAggregate() {
        val task = aggregate("Recon Task", "Submitted")
        var opened: LoadedAggregate? = null
        composeRule.setContent {
            ApprovalsScreen(tasks = listOf(task), missions = emptyList(), onRefresh = {}, onOpenTask = { opened = it }, onOpenMission = {})
        }
        composeRule.onNodeWithText("Recon Task").performClick()
        assertEquals(task.id, opened?.id)
    }

    /** Tapping a mission result opens Mission Detail -- via the real callback, not a new navigation mechanism. */
    @Test
    fun tappingAMission_invokesOnOpenMissionWithTheRealAggregate() {
        val mission = aggregate("Host Mission", "AwaitingApproval")
        var opened: LoadedAggregate? = null
        composeRule.setContent {
            ApprovalsScreen(tasks = emptyList(), missions = listOf(mission), onRefresh = {}, onOpenTask = {}, onOpenMission = { opened = it })
        }
        composeRule.onNodeWithText("Host Mission").performClick()
        assertEquals(mission.id, opened?.id)
    }

    /** No Approve/Reject affordance exists on this screen -- a queue/discovery surface only, matching Dart exactly. */
    @Test
    fun noApproveOrRejectActionExistsOnThisScreen() {
        composeRule.setContent {
            ApprovalsScreen(
                tasks = listOf(aggregate("Recon Task", "Submitted")),
                missions = listOf(aggregate("Host Mission", "AwaitingApproval")),
                onRefresh = {},
                onOpenTask = {},
                onOpenMission = {},
            )
        }
        composeRule.onNodeWithText("Approve", substring = true, ignoreCase = true).assertDoesNotExist()
        composeRule.onNodeWithText("Reject", substring = true, ignoreCase = true).assertDoesNotExist()
    }

    /** The pull-to-refresh gesture calls the existing OnyxController.refresh() passed in as [onRefresh] -- no separate mechanism. */
    @Test
    fun pullToRefresh_invokesTheExistingOnRefreshCallback() {
        var refreshed = false
        composeRule.setContent {
            ApprovalsScreen(tasks = emptyList(), missions = emptyList(), onRefresh = { refreshed = true }, onOpenTask = {}, onOpenMission = {})
        }
        composeRule.onRoot().performTouchInput { swipeDown() }
        composeRule.waitForIdle()
        assertTrue("pull-to-refresh gesture must call the real onRefresh callback", refreshed)
    }
}
