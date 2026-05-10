# Axonyx Release Runbook

This is the framework-level release map for the first public Cargo release path.

It does not mean "publish now". Use it to decide when the system is ready and to avoid publishing crates in the wrong order.

## Release Goals

- Let new apps use a published `axonyx-runtime` dependency.
- Let Axonyx apps consume `axonyx-ui` as a Cargo package with `.ax` components and package CSS.
- Let developers install the public CLIs without cloning the framework repo.
- Keep the first release small, honest, and easy to explain.

## Package Order

Publish in dependency order:

1. `axonyx-macros`
2. `axonyx-core`
3. `axonyx-runtime`
4. `axonyx-ui`
5. `create-axonyx`
6. `cargo-axonyx`

Why this order:

- `axonyx-core` depends on `axonyx-macros`.
- `axonyx-runtime` depends on `axonyx-core`.
- generated apps depend on `axonyx-runtime`.
- generated UI-enabled apps can depend on `axonyx-ui`.
- the CLIs should be published only after the packages they scaffold are available.

## Current CI Gate

Before publishing, `main` should be green in GitHub Actions.

The framework CI currently checks:

- formatting
- full framework workspace tests
- core smoke loop
- `cargo ax check`
- `cargo ax doctor --deny-warnings`
- `cargo ax build --clean`
- runtime workspace tests
- package-content rehearsal for runtime and CLI crates

The local equivalent from the framework repo is:

```bash
cargo fmt --all -- --check
cargo test
powershell -ExecutionPolicy Bypass -File scripts/smoke-core-loop.ps1 -Template site
```

## Pre-Publish Checks

Run these before the first publish attempt:

```bash
git status --short
git submodule status
cargo fmt --all -- --check
cargo test
powershell -ExecutionPolicy Bypass -File scripts/smoke-core-loop.ps1 -Template site
```

Then verify `axonyx-ui` from its repo:

```bash
cd H:/CODE/axonyx/axonyx-ui
cargo test
cargo package --list
npm run build
npm pack --dry-run
```

## Runtime Publish

Runtime has its own detailed runbook:

- `vendor/axonyx-runtime/docs/release-0.1.0.md`
- `vendor/axonyx-runtime/docs/publish-0.1.0.md`

The important rule is to wait for crates.io index propagation between crates:

```bash
cd H:/CODE/axonyx/axonyx-framework/vendor/axonyx-runtime
cargo publish -p axonyx-macros
cargo publish -p axonyx-core
cargo publish -p axonyx-runtime
```

Do not continue to the next crate until Cargo can resolve the previous crate from crates.io.

## UI Publish

After runtime is available, publish `axonyx-ui` from the UI repo:

```bash
cd H:/CODE/axonyx/axonyx-ui
cargo test
cargo package
cargo publish
```

`axonyx-ui` public API includes:

- `Axonyx.package.toml`
- `@axonyx/ui` namespace metadata
- `src/foundry/*.ax`
- `src/css/index.css`
- package-served URL shape: `/_ax/pkg/axonyx-ui/index.css`

## CLI Publish

Publish CLIs only after runtime packages are available.

From the framework repo:

```bash
cd H:/CODE/axonyx/axonyx-framework
cargo package -p create-axonyx
cargo package -p cargo-axonyx
cargo publish -p create-axonyx
cargo publish -p cargo-axonyx
```

After this, the desired install story is:

```bash
cargo install create-axonyx
cargo install cargo-axonyx
```

## Post-Publish Verification

Create one app in registry mode:

```bash
create-axonyx registry-site --yes --template site --runtime-source registry
cd registry-site
cargo ax check
cargo ax doctor --deny-warnings
cargo ax build --clean
cargo ax run dev
```

Expected result:

- app resolves `axonyx-runtime` from crates.io
- UI imports resolve through the package model
- package CSS is available through `/_ax/pkg/axonyx-ui/index.css`
- `dist/index.html` is generated

## Stop Conditions

Stop the release if:

- CI is red
- `cargo ax doctor --deny-warnings` fails on a fresh app
- `cargo package` changes the package contents unexpectedly
- crates.io cannot resolve an upstream crate after normal propagation time
- generated docs still describe only local path or Git-only flows

If a partial publish happens, do not rush the next crate. Update release notes honestly and continue only when the next package is ready.
