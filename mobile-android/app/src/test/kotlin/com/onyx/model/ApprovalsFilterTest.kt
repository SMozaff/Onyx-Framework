package com.onyx.model

import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Real tests against `ApprovalsFilter`'s exact port of
 * `ui/screens/approvals.dart`'s real, current filtering logic
 * (`pendingTasks = tasks.where(status == 'Submitted')`,
 * `pendingMissions = missions.where(status == 'AwaitingApproval')`,
 * tasks before missions), confirmed by reading that file directly for
 * A5.1.
 */
class ApprovalsFilterTest {
    @Test
    fun `a Submitted task appears in the filtered result, a task in any other status does not`() {
        val submitted = aggregate("Recon Task", "Submitted")
        val draft = aggregate("Other Task", "Draft")
        val approved = aggregate("Approved Task", "Approved")

        val result = ApprovalsFilter.pendingTasks(listOf(submitted, draft, approved))

        assertEquals(1, result.size)
        assertEquals("Recon Task", result.first().title)
    }

    @Test
    fun `an AwaitingApproval mission appears, a mission in any other status does not`() {
        val awaiting = aggregate("Host Mission", "AwaitingApproval")
        val active = aggregate("Other Mission", "Active")
        val draft = aggregate("Draft Mission", "Draft")

        val result = ApprovalsFilter.pendingMissions(listOf(awaiting, active, draft))

        assertEquals(1, result.size)
        assertEquals("Host Mission", result.first().title)
    }

    @Test
    fun `tasks precede missions in the combined, ordered result`() {
        val task = aggregate("Recon Task", "Submitted")
        val mission = aggregate("Host Mission", "AwaitingApproval")

        val combined = ApprovalsFilter.pending(tasks = listOf(task), missions = listOf(mission))

        assertEquals(listOf("Recon Task", "Host Mission"), combined.map { it.title })
    }

    @Test
    fun `zero matches produces an empty combined result`() {
        val task = aggregate("Other Task", "Draft")
        val mission = aggregate("Other Mission", "Active")

        val combined = ApprovalsFilter.pending(tasks = listOf(task), missions = listOf(mission))

        assertTrue(combined.isEmpty())
    }

    @Test
    fun `multiple pending tasks and missions are all included, tasks still ordered before missions`() {
        val taskA = aggregate("Task A", "Submitted")
        val taskB = aggregate("Task B", "Submitted")
        val missionA = aggregate("Mission A", "AwaitingApproval")
        val missionB = aggregate("Mission B", "AwaitingApproval")

        val combined = ApprovalsFilter.pending(tasks = listOf(taskA, taskB), missions = listOf(missionA, missionB))

        assertEquals(listOf("Task A", "Task B", "Mission A", "Mission B"), combined.map { it.title })
    }

    private fun aggregate(name: String, status: String): LoadedAggregate = LoadedAggregate.fromJson(
        JSONObject(
            """{"id": [0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15], "aggregate": {"name": "$name", "status": "$status"}, "version": 0, "lifecycle_epoch": 0, "authority_epoch": 0, "updated_at": 0}""",
        ),
    )
}
