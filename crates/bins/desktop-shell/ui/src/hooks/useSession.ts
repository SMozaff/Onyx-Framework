import { useMemo } from "react";
import type { Id16 } from "@/types/onyx";
import { newId16 } from "@/types/onyx";

/**
 * Placeholder session identity, standing in for a real authenticated
 * session until one exists (Increment 7 scope — see
 * `desktop-shell/src/lib.rs`'s own `TODO(auth/org resolution)` note,
 * which flags that `AppState`'s `organization_id` is currently
 * random-per-launch on the Rust side too, not just here).
 *
 * Generated ONCE per app session (`useMemo` with no deps, not
 * `useState`+`useEffect`, since there is nothing to react to — this
 * value never legitimately changes during a session) and held for the
 * page's lifetime, rather than randomized on every command (which would
 * make every command in one session appear to come from a different
 * user/device — clearly wrong even as a placeholder).
 *
 * Real risk this masks: the ids generated here are NOT guaranteed to
 * match the `organization_id` `AppState::new` randomly generated on the
 * Rust side at launch (see the same TODO there) — so a command issued
 * through this session may target a different `organization_id` than
 * what `AppState`'s aggregates are actually scoped under, and fail or
 * silently create data under an org nothing else queries. This is a
 * real, load-bearing gap in the current placeholder auth story, not
 * papered over by this hook — flagged here explicitly rather than
 * implied to work.
 */
export function useSession() {
  return useMemo(
    () => ({
      organizationId: newId16() as Id16,
      userId: newId16() as Id16,
      deviceId: newId16() as Id16,
    }),
    [],
  );
}
