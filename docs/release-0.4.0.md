# Axonyx 0.4.0

Axonyx 0.4.0 is the first coordinated release of the language tooling, runtime,
and application CLIs under one minor version.

## Highlights

- Pages and ASX language services, formatter support, local symbols, and the
  first public `axonyx-lsp` package.
- Typed backend contracts, response validation, API hashes, database schema
  workflows, migrations, joins, and SQLite/Postgres execution paths.
- Axum/Tokio production serving with streaming, middleware, security headers,
  compression, health/readiness probes, and graceful shutdown.
- Typed state and WASM bridge work, reactive browser updates, actions,
  invalidation, and data refresh without a virtual DOM.
- File Upload V1 with multipart progress, capability-based storage, limits,
  lifecycle events, and real Chromium coverage.
- Updated scaffolds, Docker defaults, diagnostics, Doctor checks, and registry
  upgrade paths pinned to the 0.4.0 runtime.

## Packages

- `axonyx-core 0.4.0`
- `axonyx-runtime 0.4.0`
- `axonyx-lsp 0.4.0`
- `create-axonyx 0.4.0`
- `cargo-axonyx 0.4.0`

`axonyx-macros` remains at `0.1.0`, and `axonyx-ui` keeps its independent
release cadence.

## Upgrade

```bash
cargo install cargo-axonyx --version 0.4.0 --force
cargo ax upgrade
cargo update
cargo ax check
cargo ax build --clean
```

For a new project:

```bash
cargo install create-axonyx --version 0.4.0 --force
create-axonyx my-app --template site --runtime-source registry
```
