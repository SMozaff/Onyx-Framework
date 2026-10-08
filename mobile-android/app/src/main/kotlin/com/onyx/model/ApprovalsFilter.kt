package com.onyx.model

/**
 * Kotlin port of `ui/screens/approvals.dart`'s real, current filtering
 * logic, re-verified fresh against that file for A5.1 (confirmed
 * exactly: `pendingTasks = controller.tasks.where(status == 'Submitted')`,
 * `pendingMissions = controller.missions.where(status ==
 * 'AwaitingApproval')`, tasks listed before missions). A pure,
 * dependency-free function over already-loaded
 * [OnyxController][com.onyx.controller.OnyxController] state -- no
 * separate query, no `Approval` aggregate involved (see that class's
 * own doc comment on why `listAggregates('approval')`'s result is
 * fetched but never stored: this filter is the real reason it never
 * needs to be).
 */
object ApprovalsFilter {
    fun pendingTasks(tasks: List<LoadedAggregate>): List<LoadedAggregate> =
        tasks.filter { it.status == "Submitted" }

    fun pendingMissions(missions: List<LoadedAggregate>): List<LoadedAggregate> =
        missions.filter { it.status == "AwaitingApproval" }

    /** Tasks first, then missions -- matches Dart's real, current display order exactly. */
    fun pending(tasks: List<LoadedAggregate>, missions: List<LoadedAggregate>): List<LoadedAggregate> =
        pendingTasks(tasks) + pendingMissions(missions)
}
