# Scoped Upload Budgets V0 Candidate

| Package | Candidate |
| --- | --- |
| axonyx-runtime | 0.6.11 |
| cargo-axonyx | 0.6.16 |
| create-axonyx | 0.6.11 |

Core 0.6.9, macros, LSP, state WASM, UI and editor are unchanged. These versions
are release candidates, not confirmation of crates.io publication.

## Changes

Explicit `[[server.action_body_limits]]` entries grant a chosen request budget
only to known page/action pairs in compiled Axum hosts. The global configured
limit is retained for ordinary, unknown and ambiguous requests. Static pages,
including pages without actions, shadow dynamic allowances. Check and Doctor
validate unknown/duplicate/malformed entries before compilation.

Strict action selectors share one parser between preview and compiled dispatch.
Handwritten action URLs that omitted `path` must now send `path=%2F` for root
actions. Duplicate selectors, encoded reserved-key aliases and malformed percent
or UTF-8 values are rejected rather than accepting the first value silently.
Generated forms already use the required literal keys.

The opt-in runtime bounds the complete request before dispatch and distinguishes
safe 413 size errors from safe 400 malformed bodies. Legacy hosts retain their
old path when no rules are configured. Preview/std/noncompiled hosts explicitly
refuse scoped rules for now, not silently ignore them. This is bounded buffering,
not upload streaming, auth policy or a total memory bound.

Runtime selection is synchronized across workspace dependencies, CLI upgrade/
Doctor and create-axonyx defaults. Generated scoped hosts omit the unused legacy
host import; unrelated fixture-only warnings are not hidden.

See ../scoped-action-body-limits-v0.md and ../action-selector-v0.md for the exact
contract. Framework source gates and Legura PR #59 prove known-length/chunked
requests, small ordinary-form budgets, guarded canonical image writes, auth,
archive/recovery and session revocation. They are not registry publication proof.

## Promotion And Adoption

Prepare runtime and tooling through separate feature -> dev PRs, then dev -> main
release PRs. Require green release/main CI. Publish runtime first and verify its
registry package against unchanged published core 0.6.9. After index propagation,
verify and publish scaffolder, then CLI. Do not republish unchanged packages.

Verify fresh isolated registry installations before creating package-scoped tags
and GitHub releases. Then update Legura dependency/CI pins and rerun Media with
`ScopedMedia`, no RuntimeSource/ToolManifest overrides. Its source acceptance job
is separate from the existing registry gate until that adoption is complete.

No Docker image publication, site deployment, operator database migration or
automatic product Media activation is part of this release preparation.
