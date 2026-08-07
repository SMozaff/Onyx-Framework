# ONYX v1.0.0 Go-Live Checklist

| Item | Status | Required sign-off |
|---|---|---|
| All CI gates green for v1.0.0 | ☐ | SRE Lead |
| Backend E2E Journeys 1–4 pass | ☐ | QA Lead |
| Client Journeys 5–7 disposition recorded | ☐ | Team 5 Lead |
| Five chaos scenarios pass; staging scenarios ran 15 minutes | ☐ | Architect |
| 100-user/60-second load test meets p95 <500 ms | ☐ | Performance Engineer |
| DR drill achieves RTO <1 hour and RPO <5 minutes | ☐ | SRE Lead |
| Security audit and Ed25519 key ceremony complete | ☐ | Security Lead |
| Audit hash chain verifies | ☐ | Compliance Lead |
| PostgreSQL backup/restore and migration up/down tested | ☐ | DBA |
| Helm lint/render and Terraform validation pass | ☐ | Platform Lead |
| SPDX 2.3 SBOM and provenance attached | ☐ | Release Engineer |
| Containers cosign-verified and binaries GPG-verified | ☐ | Security Lead |
| 10→50→100 canary and >1% rollback verified | ☐ | Product Manager |
| Runbooks reviewed by active on-call rotation | ☐ | On-Call Team |
| All ADRs and DECISIONS entries resolved or accepted | ☐ | Architect |

Release is **NO-GO** until every required sign-off is present.
