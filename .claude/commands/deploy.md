---
name: deploy
argument-hint: [version: X.Y.Z]
---

Release Symphony v$ARGUMENTS. This app has no staging/production environments; a release is a `vX.Y.Z` git tag that triggers `.github/workflows/release.yml`.
1. Run `cargo test` — block if any fail
2. Run `cargo build --release` — block if build fails
3. Run security-auditor agent — block on CRITICAL findings
4. Confirm `Cargo.toml` version and `CHANGELOG.md` match $ARGUMENTS
5. Ask the user to confirm, then: `git tag v$ARGUMENTS && git push origin v$ARGUMENTS` (publishes a release — never without explicit yes)
6. Check the Release workflow run: `gh run list --workflow=release.yml --limit 1`
