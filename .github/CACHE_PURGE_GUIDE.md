# GitHub Actions Cache Purge Guide

## Stale Caches Identified (as of 2026-10-24)

The following GitHub Actions caches are on stale PR merge refs and should be purged:

| Cache ID | Key | Ref | Size | Last Accessed | Status |
|----------|-----|-----|------|---------------|--------|
| 9428871016 | cache-trivy-2026-10-24 | refs/pull/295/merge | 96 MB | 2026-10-24T00:06:44Z | STALE - PR #295 merged |
| 9423807241 | v0-rust-release-build-Linux-x64-d0ea4c79-ad978de8 | refs/pull/296/merge | 1.7 GB | 2026-10-23T21:27:38Z | STALE - PR #296 merged |
| 9423691740 | v0-rust-release-build-Linux-x64-d0ea4c79-e4efd79b | refs/pull/294/merge | 1.5 GB | 2026-10-23T21:24:44Z | STALE - PR #294 merged |
| 9423056670 | node-cache-Linux-x64-npm-b228befa4d705a134182afce3e6f7b1c1695aee695386d1d113525bc8cfba1d6 | refs/pull/293/merge | 66 MB | 2026-10-23T21:08:57Z | STALE - PR #293 merged |

**Total stale cache size: ~3.4 GB**

## Caches to Keep

The following caches on `refs/heads/main` are active and should be kept:

| Cache ID | Key | Size | Last Accessed |
|----------|-----|------|---------------|
| 9428913415 | cache-trivy-2026-10-24 | 96 MB | 2026-10-24T03:51:47Z |
| 9423033080 | trivy-binary-v0.70.0-Linux-X64 | 44 MB | 2026-10-24T03:51:46Z |
| 9424812565 | v0-rust-release-build-Linux-x64-d0ea4c79-ad978de8 | 1.7 GB | 2026-10-24T03:48:28Z |
| 9423035771 | node-cache-Linux-x64-npm-b228befa4d705a134182afce3e6f7b1c1695aee695386d1d113525bc8cfba1d6 | 66 MB | 2026-10-24T03:47:21Z |
| 9423070248 | node-cache-Linux-x64-npm-e225fded197cc6f861faae0a074bc758916e96062764c76c71bae59a147250ba | 76 MB | 2026-10-24T03:47:20Z |
| 9423190817 | node-cache-Linux-x64-npm-90d34f189dad8fb4b98c1d43ae4e5c6f54c784d0fdba79244a4b5eab85ee9580 | 78 MB | 2026-10-24T03:47:18Z |
| 9423001659 | node-cache-Linux-x64-npm-005023d1cac87fb00a5c42871dd1c38935287a933ea8e9d285b5f8497c08f389 | 109 MB | 2026-10-24T03:47:18Z |
| 9423054316 | node-cache-Linux-x64-npm-69c001106c04a669e6a41d67ee2cc151d9a68b754cba4fa44df1fab740fca786 | 38 MB | 2026-10-24T03:47:18Z |
| 9430010863 | node-cache-Linux-x64-npm-19649b16d97056c33d867616a1a7021a0f645663eb74ad0e70d3c84f671dd3e4 | 74 MB | 2026-10-24T03:14:38Z |
| 9423045603 | cache-trivy-2026-10-23 | 96 MB | 2026-10-24T00:07:13Z |
| 9423233517 | v0-rust-release-build-Linux-x64-d0ea4c79-e4efd79b | 419 MB | 2026-10-23T21:33:35Z |

**Total active cache size: ~2.5 GB**

## How to Purge Caches

### Using GitHub CLI

```bash
# Delete individual stale caches
gh api repos/SMozaff/Onyx-Framework/actions/caches/9428871016 -X DELETE
gh api repos/SMozaff/Onyx-Framework/actions/caches/9423807241 -X DELETE
gh api repos/SMozaff/Onyx-Framework/actions/caches/9423691740 -X DELETE
gh api repos/SMozaff/Onyx-Framework/actions/caches/9423056670 -X DELETE

# Verify deletion
gh api repos/SMozaff/Onyx-Framework/actions/caches -q '.actions_caches | length'
```

### Using REST API

```bash
# Delete a cache by ID
curl -X DELETE \
  -H "Authorization: token YOUR_GITHUB_TOKEN" \
  -H "Accept: application/vnd.github+json" \
  https://api.github.com/repos/SMozaff/Onyx-Framework/actions/caches/9428871016
```

## Pre-Purge Inventory

Run before deletion to record current state:

```bash
gh api repos/SMozaff/Onyx-Framework/actions/caches -q '.actions_caches[] | {id, key, ref, last_accessed_at, size_in_bytes}' > cache_inventory_before.json
```

## Post-Purge Verification

Run after deletion to verify:

```bash
# Count remaining caches
gh api repos/SMozaff/Onyx-Framework/actions/caches -q '.actions_caches | length'

# List remaining caches
gh api repos/SMozaff/Onyx-Framework/actions/caches -q '.actions_caches[] | {id, key, ref}'

# Save post-purge inventory
gh api repos/SMozaff/Onyx-Framework/actions/caches -q '.actions_caches[] | {id, key, ref, last_accessed_at, size_in_bytes}' > cache_inventory_after.json
```

## Expected Results After Purge

- **Before**: 15 caches total
- **After**: 11 caches (4 stale PR caches removed)
- **Space reclaimed**: ~3.4 GB

## Notes

- Caches on `refs/heads/main` are actively used and should NOT be deleted
- Caches on merged PR refs (`refs/pull/*/merge`) are stale because those refs no longer exist
- GitHub automatically cleans up caches after 7 days of inactivity, but manual purge is recommended for immediate cleanup
- Deleting caches does NOT affect running workflows; they will create new caches as needed
