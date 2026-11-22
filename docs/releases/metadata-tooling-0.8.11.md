# Server-rendered metadata tooling release

This change prepares `create-axonyx 0.8.7` and `cargo-axonyx 0.8.11`.
Core and Runtime 0.8.5 are already published; no new LSP or UI package is
needed. New scaffolds use Runtime 0.8.5 with the existing UI baseline.

## Before publishing

1. Promote the checked release commit from `dev` to `main` and wait for the
   main workflow, including the source preview/compiled parity smoke.
2. Verify both locked package archives can be built from published dependencies.
3. Publish `create-axonyx 0.8.7` and `cargo-axonyx 0.8.11` after approval.

## Independent user acceptance

After both packages can be resolved from crates.io:

```powershell
pwsh -NoProfile -File scripts/smoke-registry-beta.ps1 -Profile metadata
```

The script installs the exact tool versions into a new temporary root,
scaffolds site, blog and docs outside the checkout, and builds and serves the
site. The two server modes must render the same request-specific title,
description and canonical link from a loader. Missing link values are omitted.
The earlier registry-only baseline stays the default until this profile passes
with the published tools. Do not treat local packages as registry acceptance.
