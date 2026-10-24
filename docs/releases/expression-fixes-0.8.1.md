# Expression and Action Diagnostics - Patch Candidate

Preparation only, not a published Cargo release. This candidate pairs
`axonyx-core` and `axonyx-runtime` 0.8.1 with `axonyx-lsp` and
`create-axonyx` 0.8.1 and `cargo-axonyx` 0.8.3. Macros, UI, npm, Docker and
deployed applications are unchanged.

The runtime/core patch contains the grouped-coalescing `require` parser fix
(#377), an early diagnostic for unsupported `params.*` and `query.*` references
inside compiled actions (#374), and null-safe direct optional member access
on raw DB query records (#375). The latter does not change typed Rust record
lowering. The action diagnostic does not implement route-parameter binding.

The scaffolder's registry default and the CLI's upgrade target both move to
runtime 0.8.1 so fresh and upgraded apps consume the same fix. Framework
compiled HTTP smoke covers existing and absent JSON records, missing fields,
and an untouched string literal.

Promotion gate: green exact-head runtime/framework CI; separate dev-to-main
PRs; package verification with published dependencies; publish core, then
runtime, then LSP/scaffolder/CLI; fresh registry-only compiled consumer smoke;
package-scoped tags only after publication. Do not upgrade operator tools or
deploy Legura as part of this candidate.
