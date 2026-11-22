# R-62 — governing BOUND plan path — PROPOSED (2026-11-23)

**Status: PROPOSED path placement; needs owner approval.** DOC-BOUND-01 stays OPEN.

## Evidence

- `docs/migrations/slint/decision-log.md` cites `common/ONYX_Slint_Migration_BOUND_Plan_Execution_Ready.md`
  and records that the path returned 404 on `main` (also master record DOC-BOUND-01, line 242).
- The execution-ready revision that actually exists is
  `/home/all-father/Documents/ONYX_Slint_Migration_Planning_Package/ONYX_Slint_Migration_BOUND_Plan_Execution_Ready.md`
  (680 lines, header: "ONYX Slint Migration: BOUND Execution Plan (Execution-Ready Revision)").
- The register calls the revised plan `ONYX_Slint_Migration_BOUND_Plan_Revised.md`; that literal
  filename does not exist in the repository or the planning package.

## PROPOSED action

1. Copy the execution-ready revision verbatim into the repository at the cited path:
   `common/ONYX_Slint_Migration_BOUND_Plan_Execution_Ready.md`.
2. Record in the decision log that this file is the governing source, with its version/hash and the
   date of placement; keep the external planning-package copy as provenance, not authority.
3. Update the register's R-62 name to the actual filename so future agents do not search for a
   non-existent `*_Revised.md`.
4. Re-check Gate 0b requirements against the restored in-repository text.

This draft does not place the file or change the citation. No owner approval is implied, and
DOC-BOUND-01 is not closed by this document.
