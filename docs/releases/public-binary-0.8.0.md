# Public Binary Responses - 0.8.0 Candidate

Preparation is not publication confirmation. Coordinated packages: axonyx-core,
axonyx-runtime, cargo-axonyx, create-axonyx and axonyx-lsp 0.8.0. Macros, internal
state WASM, UI/editor and the browser/WASM ABI are independent and unchanged.

The new PublicResponse compiler-plan enum variant requires a minor boundary;
old CLI 0.7.x dependency ranges must not resolve this incompatible core. Workspace
dependency pins, recommended upgrade/doctor runtime, scaffold default, all three
tool versions and current package examples move together to 0.8.0.

## Scope And Proof

Runtime PRs 273/274 and framework PR 378 passed their exact mandatory CI gates
before dev merge. The source compiled HTTP proof covers public anonymous exact
bytes, current publication grant revocation, public/private registry isolation,
safe errors and unchanged private anonymous/logout/CSRF admission. API/OpenAPI/
Melt contracts and session/env discovery recognize both binary channels.

Native Rust callbacks remain explicitly compiled-only. Interpretive preview
rejects registration with a compiled-mode hint; it does not emulate native Rust.
The fixture still emits pre-existing unused generated-host helper warnings.
Independent grouped-coalescing guard issue 377 is not claimed fixed. Legura public
images and actual CMS browser acceptance are not delivered merely by a CLI bump.

## Required Promotion

Candidate PRs target dev, with all required exact-head checks. Release uses
separate dev-to-main PRs and green exact main CI before any upload. Publish core,
then runtime after core is registry-visible, then build-verify and publish tooling
against actual registry 0.8.0 dependencies. Never use --no-verify to disguise an
unpublished dependency or claim package --list proves registry execution.

Fresh isolated registry installs must verify all templates, Rust 1.82 consumers,
LSP startup, and compiled public/private binary HTTP before tags/releases and
product adoption. Do not mutate the operator CMS, its DB/.env/process, global
CLI, deployed sites or Docker image as a side effect of publication.
