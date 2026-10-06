# ONYX Production / Release Go-Live Checklist

**Current deployment:** Cloudflare Edge → Render Axum API → Render PostgreSQL + Hugging Face object storage.

**Rule:** Release is **NO-GO** until every applicable blocking item has evidence and the required owner sign-off.

## 1. Application / runtime

| Gate | Status | Evidence / owner |
|---|---|---|
| Render ONYX API deployment is live | ☑ | Render deployment evidence |
| `/ready` succeeds against production PostgreSQL | ☑ | Render health/log evidence |
| Production uses PostgreSQL, not SQLite fallback | ☑ | Runtime/config audit |
| Production signing key configured | ☑ | Render secret/config audit |
| Governance database configured | ☑ | Render secret/config audit |
| Production CORS allow-list is non-empty | ☑ | Render configuration audit |
| Required backend E2E Journeys 1–4 pass on release candidate | ☐ | QA Lead |
| Client Journeys 5–7 disposition recorded | ☐ | Client Lead |
| Unsupported/non-implemented client capabilities are accurately documented | ☐ | Product Lead |
| Real production usage / customer evidence is available before claiming “production proven” | ☐ | Product/Commercial Lead |

## 2. Security / secrets

| Gate | Status | Evidence / owner |
|---|---|---|
| No production credentials committed to repository | ☑ | Repository audit |
| GitHub Actions references are immutable SHA-pinned | ☑ | Security workflow |
| RustSec/cargo-deny checks pass | ☐ | Security Lead |
| Container HIGH/CRITICAL scan passes | ☐ | Security Lead |
| Dependency/SBOM checks pass | ☐ | Security Lead |
| Signing-key ceremony and rotation procedure complete | ☐ | Security Lead |
| Security contact / vulnerability reporting process published | ☐ | Security Lead |
| Production secret rotation procedure documented | ☐ | SRE/Security Lead |

## 3. Database / migrations

| Gate | Status | Evidence / owner |
|---|---|---|
| Render PostgreSQL 16 is available | ☑ | Render resource audit |
| Production API connects to PostgreSQL | ☑ | `/ready` + startup evidence |
| Migration startup succeeds | ☑ | Render startup evidence |
| Migration up/idempotency/down/up CI checks pass | ☐ | DBA/CI |
| Production backup/PITR recovery tested | ☐ | DBA/SRE |
| Logical backup/export retained according to policy | ☐ | DBA/SRE |
| Recovery RTO <1h demonstrated | ☐ | SRE |
| Recovery RPO <5m demonstrated | ☐ | SRE |
| Current database HA requirement explicitly accepted or enabled | ☐ | Architect/SRE |

**Current limitation:** production has no HA standby or read replica. Do not claim HA or measured RTO/RPO until a current-topology drill proves it.

## 4. Object storage

| Gate | Status | Evidence / owner |
|---|---|---|
| Production backend selects Hugging Face | ☑ | Runtime configuration |
| HF endpoint/bucket configuration verified | ☑ | Configuration audit |
| SigV4/path-style adapter source-audited | ☑ | Source audit |
| Production authenticated PUT → GET → DELETE cycle | ☐ | Release Engineer |
| Object integrity/content-addressing verified | ☐ | Release Engineer |
| Credential rotation procedure tested | ☐ | Security/SRE |
| Recovery/export procedure documented | ☐ | SRE |

**Current blocker:** actual production HF object operations are not yet independently proven.

## 5. Cloudflare edge

| Gate | Status | Evidence / owner |
|---|---|---|
| Worker exists and is live | ☑ | Cloudflare deployment audit |
| Worker origin points to Render API | ☑ | Cloudflare settings audit |
| HTTPS origin enforced | ☑ | Worker source audit |
| Authorization remains in ONYX API | ☑ | Architecture/source audit |
| WebSocket 101 pass-through preserved | ☑ | Worker source audit |
| Public Cloudflare → Render readiness probe previously returned HTTP 200 | ☑ | External GitHub-runner evidence |
| Current production edge E2E probe repeated after final release candidate | ☐ | Release Engineer |
| Custom production domain configured and verified | ☐ | Platform/Product |

## 6. CI/CD / release controls

| Gate | Status | Evidence / owner |
|---|---|---|
| All required CI gates green for release candidate | ☐ | Release Engineer |
| Frontend audits green | ☑* | Current candidate checks |
| i18n gate green | ☑* | Current candidate checks |
| SBOM generation green | ☑* | Current candidate checks |
| Dependency-manifest gate green | ☑* | Current candidate checks |
| Container security gates green | ☐ | Current run still in progress/audit |
| Rust check/build/test gates green | ☐ | Current run still in progress/audit |
| Release workflow tag path reviewed | ☑ | Release workflow audit |
| Container image signing configured | ☐ | Release Engineer |
| Binary signing configured for every shipped platform | ☐ | Release Engineer |
| Provenance attestations attached to release artifacts | ☐ | Release Engineer |
| Render deployment is reproducible from repository state | ☑ | Render/Blueprint audit |
| Cloudflare deployment path is reproducible and its automated build check is green | ☐ | Platform Lead |

* Green on the current candidate where the corresponding check has completed; final release still requires all gates green.

## 7. Domain / frontend integration

| Gate | Status | Evidence / owner |
|---|---|---|
| Actual production frontend origin selected | ☐ | Product/Platform |
| Production CORS allow-list updated to the actual frontend origin | ☐ | Platform |
| Web UI deployed to a production domain | ☐ | Frontend Lead |
| Admin UI deployed to a production domain | ☐ | Admin Lead |
| Client API/WS production base URLs configured | ☐ | Frontend Lead |
| Cloudflare edge + frontend + API E2E verified | ☐ | QA |
| Auth/session flow verified from real production client | ☐ | QA/Security |
| Domain/TLS/DNS ownership verified | ☐ | Platform |

The repository currently contains development Vite origins and example `api.onyx.example.com` values, not a verified production frontend deployment.

## 8. Licensing / IP / legal

| Gate | Status | Evidence / owner |
|---|---|---|
| Proprietary ONYX ownership position documented | ☑ | `LICENSE.md`, `LEGAL/PROPRIETARY_NOTICE.md` |
| Current owner recorded | ☑ | IP ownership register |
| Chain-of-title evidence collected and verified | ☐ | Legal |
| Founder/contractor/employee IP assignments verified | ☐ | Legal |
| Third-party dependency license policy completed | ☐ | Legal/Engineering |
| Final distributable license/notice inventory generated | ☐ | Release/Legal |
| Trademark clearance completed | ☐ | Legal |
| Trademark registration strategy decided | ☐ | Legal |
| SaaS Terms / MSA completed | ☐ | Legal |
| EULA / self-hosted license completed | ☐ | Legal |
| Custom enterprise IP terms completed | ☐ | Legal |
| Privacy Policy completed | ☐ | Legal |
| DPA completed | ☐ | Legal |
| Subprocessor register completed | ☐ | Legal/Security |
| Security/technical-measures schedule completed | ☐ | Legal/Security |

## 9. Commercial launch

| Gate | Status | Evidence / owner |
|---|---|---|
| First ICP selected | ☐ | Commercial Lead |
| Initial commercial use case validated | ☐ | Commercial Lead |
| Pricing/packaging defined | ☐ | Commercial Lead |
| SaaS/self-hosted/custom enterprise entitlement mechanics defined | ☐ | Product/Commercial |
| Sales/demo workflow prepared | ☐ | Commercial |
| Pilot/customer evidence available | ☐ | Commercial |
| Support and escalation model defined | ☐ | Operations |
| Customer data/export/termination process defined | ☐ | Legal/Product |
| Claims reviewed against actual product evidence | ☐ | Marketing/Product |

Do not market ONYX as “production proven,” “enterprise scale,” “fully offline,” “AI-powered,” or regulatory-certified without the corresponding evidence.

## 10. Final go / no-go

### Current decision: **NO-GO**

Blocking reasons presently supported by evidence:

1. **HF production object PUT/GET/DELETE is unverified.**
2. **Current CI/release candidate is not fully green.**
3. **Production frontend/domain integration is not yet established.**
4. **Current Render PostgreSQL has no HA/read replica, and the documented RTO/RPO has not been demonstrated.**
5. **Chain-of-title and customer-facing legal contract/privacy package remain incomplete.**
6. **Trademark clearance/protection remains incomplete.**
7. **Commercial ICP, packaging/pricing, and customer proof remain incomplete.**

A future release candidate can move to **GO** only when the applicable blockers above have evidence, owner sign-off, and a release artifact tied to the verified commit/tag.
