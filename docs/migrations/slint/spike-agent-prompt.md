# Agent prompt: Slint feasibility spikes

Use this prompt to hand the spikes to an agent. It depends on the documents listed at the end.

---

You are working in the git repo at /home/claude/onyx-framework (GitHub SMozaff/Onyx-Framework, branch main is current).

Your task: run the Slint feasibility spikes described in docs/migrations/slint/spike-handover.md. Read that file first and follow its rules exactly. Also read the plan section for brief B-13 in the Onyx project doc ONYX_Slint_Migration_BOUND_Plan_Revised.md if you can access it through the Projects tool; otherwise rely on the handover file.

Hard rules:
- Work only in spikes/slint/ (create it) and docs/migrations/slint/spike-results.md. Do not edit crates/bins/admin-shell, crates/bins/desktop-shell, deploy/, the API, or the database schema.
- Never store, log, prefill, or request passwords.
- Do not make product decisions. Record them as open questions.
- Do not mark manual checks as passed. Use pending-manual when a person must run them. You run in a Linux cloud container with no display or screen reader, so you can only do automated or headless checks; mark anything needing a display, IME, screen reader, or Windows as pending-manual.
- Do not push to main. Commit on a branch named spike/slint-feasibility, push it, and open a PR with gh api (POST repos/SMozaff/Onyx-Framework/pulls with a JSON file). Do not merge it.
- Use /home/claude/scratch-spike for temporary files.

Deliverables: spikes/slint/ code (small, runnable where possible), docs/migrations/slint/spike-results.md with one row per spike item (spike, item, OS, result, evidence, notes) and a summary at the top stating what passed, failed, and needs manual testing. Finish with a short report: the PR number, what passed, what failed, what is pending-manual.

---

## Documents this prompt depends on

- docs/migrations/slint/spike-handover.md: the spike definitions, rules, and deliverables.
- ONYX_Slint_Migration_BOUND_Plan_Revised.md (Onyx project): brief B-13 and section 1.2.
- docs/migrations/slint/decision-log.md: the font and license decision, and U-10 fallback (2026-11-01).
- docs/migrations/slint/spike-results.md: the output file the agent writes (created by the run).

## Notes from the first run (2026-11-01)

- The pinned Rust toolchain 1.97.1 was not available in that environment; the agent used 1.97.0. Rebuild with the pinned toolchain before relying on results.
- The first run's results are in PR #391 (merged).
