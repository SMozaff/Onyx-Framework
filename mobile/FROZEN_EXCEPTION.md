# Frozen Reference Exception Log

`mobile/lib/` is a Frozen Reference Implementation (ONYX-MOB-00 §8 / M0,
`DECISIONS.md`). Ordinary new product development there is no longer
permitted. Only two kinds of change may still land:

- **Security fixes**
- **Critical defects** (a real, user-facing break of existing behavior --
  not a missing feature, not a UX improvement)

`scripts/verify/verify_mobile_freeze.sh` (wired into CI as the
`mobile-freeze-guard` job) fails any change that touches `mobile/lib/`
unless this file is edited in the same commit/PR. To make a legitimate
exception, add an entry below explaining what was fixed and why it
qualifies, then push.

This file has no entries yet -- it exists so the very first real
exception has somewhere to go, and so the guard itself is provably real
(see the M0 DECISIONS.md entry for the test-then-revert proof that a
change without an entry here is actually blocked).

## Log

_No exceptions recorded yet._
