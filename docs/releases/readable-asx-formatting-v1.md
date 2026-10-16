# Readable ASX Formatting V1 Candidate

| Package | Candidate |
| --- | --- |
| axonyx-core | 0.6.9 |
| axonyx-runtime | 0.6.10 |
| axonyx-lsp | 0.6.2 |
| create-axonyx | 0.6.10 |
| cargo-axonyx | 0.6.14 |

CLI and LSP share the compiler-owned formatter: expanded nested markup, wrapped
long attribute lists, preserved mixed prose/raw blocks and stable repeated
formatting. Legacy indentation-sensitive source is preserved rather than silently
migrated. Project formatting excludes generated DB types, and DB contract checks
accept LF/CRLF equivalence without accepting changed types.

Scaffold source inputs now use the same readable layout. Template-only formatting
and Rust regression checks cover all four starters and add-docs inputs, without
rewriting parser fixtures. Runtime defaults in doctor/upgrade and create-axonyx
are aligned at 0.6.10; the older scaffold default is corrected.

UI is released independently. Tooling still recommends the currently published
UI 0.0.86; UI 0.0.87 and replacement of catalog Git dependencies have their own
publication/adoption gates. No editor generation or CMS operator upgrade is
performed by this release. No new VSIX is required to implement the formatter,
but users must update installed CLI/LSP binaries and reload the editor.

## Promotion And Publication

Prepare versions through a feature -> dev PR, then a dev -> main release PR.
Wait for required CI before merging and for green main before publication.
Publish in order: core, runtime, LSP, scaffolder, CLI. Resolve each new registry
dependency before publishing the next package. Unchanged macros/state-wasm are
not republished. Verify registry-only install and scaffold/check/build/fmt before
creating package-scoped tags and GitHub releases.

This is a candidate, not a publication claim. Git-pinned formatter CI jobs remain
valid until the registry CLI is verified; replacing those pins is a separate
post-publication change. Docker publication and production site deployments are
not triggered by this candidate.
