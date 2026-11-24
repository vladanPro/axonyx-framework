# Scoped page metadata tooling candidate

Prepare `create-axonyx 0.8.8` and `cargo-axonyx 0.8.12` against the published
Core and Runtime 0.8.6. No LSP, UI, or template layout changes are needed.

Promote through `dev` and `main` and wait for the main workflow. Verify both
package archives against crates.io dependencies before publishing the tools.
Once both appear in the registry, run:

```powershell
pwsh -NoProfile -File scripts/smoke-registry-beta.ps1 -Profile metadata
```

The smoke checks site, blog, and docs outside the checkout. It adds a metadata
test route without modifying the starter layout, and verifies the route in
preview and compiled production. Keep the earlier registry baseline as the
default and keep runtime issue #304 open until this published-tool smoke passes.
