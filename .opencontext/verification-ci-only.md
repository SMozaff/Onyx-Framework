# Verification and push rules (owner instruction, 2026-11-23)

- **No local builds or tests** for this project. Verification happens only through GitHub Actions / workflows. Read the actual run logs with `gh` (use `env -u GH_TOKEN` because the stored GH_TOKEN is invalid).
- Admin Slint CI workflow: `.github/workflows/admin-slint.yml` — `cargo fmt -p admin-slint -- --check`, `cargo clippy -p admin-slint --all-targets -- -D warnings`, `cargo test -p admin-slint` on Ubuntu and Windows.
- CI formatting diffs must be applied verbatim: a 2026-11-23 push failed at the Format step for a chained `let name = selected...` in `main.rs`; the fix was pushed as `9bafe451`.
- Known pre-existing failure: `rust-advisories` (D-11/R-02) and downstream `merge-gate`. Do not weaken security policy to make it pass.
- GitHub push: SSH auth works via keyring account SMozaff; the repo has a local git config rewrite `url."git@github.com:".insteadOf "https://github.com/"`. GH_TOKEN is invalid, so `gh` commands need `env -u GH_TOKEN`.
- Never deploy, run D1 migrations, touch production data, or merge outside a reviewed PR unless the stage explicitly says the owner approved it.
