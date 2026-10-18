# Application Storage Startup V0 Candidate

Only `cargo-axonyx` moves from 0.6.14 to 0.6.15. Core 0.6.9, runtime 0.6.10,
scaffolder, LSP and UI remain unchanged and are not republished.

Compiled apps can provide `src/native_storage.rs` to configure a trusted native
storage adapter once, before listening. Startup failure prevents readiness.
Configured fallback capabilities and read-only diagnostics are preserved;
unknown names delegated to application Rust must still be denied by the adapter.
Noncompiled preview refuses the hook rather than silently ignoring it.

The generated Axum entry also honors configured `max_body_bytes` instead of a
hardcoded 1 MiB. Current oversized multipart conversion returns 400, not a
promised 413. This release does not provide upload streaming, image validation,
authorization policy, a plugin sandbox or automatic CMS activation.

See [the native storage contract](../native-storage-startup-v0.md). The isolated
native-startup smoke covers custom writes, fallback continuity, guards, limits
and startup refusal. Legura additionally passed its seven CI gates with product
configuration and generated private recovery settings (Legura PRs 55 and 56),
including session/CSRF, validated media, persistence and source rollback.

## Release Gate

This file records a candidate, not a publication claim. Merge the preparation PR
to dev only after its five required checks pass; then promote dev to main through
a release PR and wait for green main. Verify the registry-only package build and
publish only the CLI. Verify a fresh registry installation before creating the
package-scoped tag and release. Consumer Git pins are replaced separately after
that verification; operator `.env`, DBs and running services are not changed.
