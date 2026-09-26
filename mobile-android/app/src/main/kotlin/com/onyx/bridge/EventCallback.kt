package com.onyx.bridge

/**
 * Receives committed domain events routed out of the local event bus by
 * `mobile-android-jni` (see that crate's `JavaEventForwarder` doc
 * comment for the full delivery design).
 *
 * Implementations are invoked from a native (tokio worker) thread, not
 * the main thread: the JNI forwarder attaches to the JVM for the
 * duration of each delivery, so receivers must hop back onto their own
 * dispatcher before touching UI state (see `OnyxController`'s use of
 * this interface).
 */
fun interface EventCallback {
    /**
     * Delivers one event: the JSON-serialized `DomainEventEnvelope` —
     * with `event_type` (e.g. `"mission.event.0"`),
     * `aggregate_ref.id/type`, and `audit_metadata.tenant_isolation_key`
     * among its fields, the same shape the outbox pump publishes.
     */
    fun onEvent(json: String)
}