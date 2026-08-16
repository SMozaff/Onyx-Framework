# Addendum: why the previous handoff failed, and how to fix it

## What went wrong

The previous build-and-test report (`ONYX_Build_and_Test_Report.md`)
correctly found that `crates/domains/todo-domain/`,
`staff_loan_scheduler.rs`, and everything else described in
`MANUS_HANDOFF_Build_and_Test.md` **does not exist** in
`So-Muzaff/Onyx-Framwork` at commit `35b5d34`. That report is accurate
— the code genuinely was never pushed to that repository. It was built
in a separate, disconnected sandbox and only ever existed as a tarball
handed to the person, who then connected you to GitHub without
realizing the tarball's contents had never been committed there.

**This is now fixed.** `onyx-todo-domain.bundle` is a git bundle
containing a single commit, built on top of the same lineage as
`35b5d34`, with the full todo-domain feature applied as one commit on
`master`.

## How to apply it

```bash
# From a fresh clone of So-Muzaff/Onyx-Framwork at (or near) 35b5d34:
git bundle verify /path/to/onyx-todo-domain.bundle
git fetch /path/to/onyx-todo-domain.bundle master:todo-domain-feature
git checkout todo-domain-feature

# Then either:
#   - fast-forward/merge this branch into your working branch, or
#   - rebase it onto current master if 35b5d34 has since moved, or
#   - push it directly as a new branch for review:
git push origin todo-domain-feature
```

If `git fetch <bundle> master:<local-branch>` complains about the
bundle's base not being reachable from your current history (e.g. if
your clone has diverged further from `35b5d34` than expected), the
bundle contains complete history (`git bundle verify` confirms this) —
a plain `git clone /path/to/onyx-todo-domain.bundle` into a fresh
directory will always work as a standalone repo you can then push from
or cherry-pick out of.

A plain tarball (`Onyx-Framwork-updated.tar.gz`, no `.git` included) is
also provided as a fallback if the bundle doesn't apply cleanly for any
reason — but the bundle is the preferred path since it preserves this
as a real, signed-off commit rather than a pile of untracked files.

## Everything else in the original handoff still applies

Once the code is actually present in your working tree, follow
`MANUS_HANDOFF_Build_and_Test.md` exactly as written — the Postgres
provisioning work you already did (PostgreSQL 16.14, migrations
applied through `20260107000000_add_user_class_hierarchy`) is still
valid and reusable; you'll just now have the staff-loan migrations,
worker code, and `todo-domain` crate to actually test against it.

One correction to the original handoff: it referenced running
migrations via a "migration-tool" — your report shows you found and
used the real mechanism successfully, so no correction needed there,
just confirming your approach was right.

The mobile-core fix you made (`AppState::new` becoming async,
`blob_store_root` field) is unrelated to this feature and orthogonal —
keep it; there's no reason to revert it, and it was a real, legitimate
bug you found independently.
