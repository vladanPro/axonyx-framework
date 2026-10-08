# Native Validation Release

Packages: axonyx-runtime 0.6.1, cargo-axonyx 0.6.5, create-axonyx 0.6.3.
Core, LSP, UI and WASM packages have no changes in this release.

## Changes

- Preview validation supports lowered grouped comparisons and short-circuit
  logical operators, with Rust string literal decoding.
- Native validation responses can retain explicitly allowlisted public form
  values through `data-ax-retain-fields="title,summary"`.
- Password, hidden/file and framework transport controls do not replay.
  URL-encoded forms only; duplicates and oversized values disable replay.
- Both preview and compiled servers use the same retention boundary. Loader
  rerenders receive read requests, not the submitted mutation body.
- New projects and `cargo ax upgrade` target runtime 0.6.1.

## Verification

Implementation PRs passed 262 runtime and 354 CLI library tests, compiled HTTP
smoke on Windows and Linux, MSRV 1.82, package rehearsal and Postgres checks.
Release PRs must pass CI before merging to main. Publish runtime first, then
both CLI packages. Tags/releases follow successful registry verification.

This is not multipart retention, general preview/compiled expression parity,
or a CMS/authentication product release.
