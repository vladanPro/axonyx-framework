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
powershell -ExecutionPolicy Bypass -File scripts/smoke-production-server.ps1 -Template site
powershell -ExecutionPolicy Bypass -File scripts/smoke-server-transports.ps1 -Template minimal
```

## Pre-Publish Checks

Run these before the first publish attempt:

```bash
git status --short
git submodule status
cargo fmt --all -- --check
cargo test
powershell -ExecutionPolicy Bypass -File scripts/smoke-core-loop.ps1 -Template site
powershell -ExecutionPolicy Bypass -File scripts/smoke-production-server.ps1 -Template site
powershell -ExecutionPolicy Bypass -File scripts/smoke-server-transports.ps1 -Template minimal
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
cargo ax doctor --deploy render
cargo ax build --clean
cargo ax run dev
cargo ax run start --host 0.0.0.0 --port 3000
```

Expected result:

- app resolves `axonyx-runtime` from crates.io
- UI imports resolve through the package model
- package CSS is available through `/_ax/pkg/axonyx-ui/index.css`
- health probe returns JSON from `GET /__axonyx/health`
- Render deploy doctor reports `/__axonyx/health` as the health-check path
- server request timeout resolves through `[server].request_timeout_seconds`
- server shutdown grace resolves through `[server].shutdown_grace_seconds`
- server max connections resolves through `[server].max_connections`
- Tokio production server logs graceful shutdown support
- Tokio production server logs the shutdown grace period
- `dist/index.html` is generated

## Git Tags And GitHub Releases

Create tags only after the matching crates.io publish and smoke verification pass.

Use package-scoped tags because the framework repository can publish multiple packages with different versions:

```bash
git tag create-axonyx-v0.1.33
git push origin create-axonyx-v0.1.33

git tag cargo-axonyx-v0.1.62
git push origin cargo-axonyx-v0.1.62
```

For runtime releases, tag from the standalone runtime repository:

```bash
cd H:/CODE/axonyx/axonyx-framework/vendor/axonyx-runtime
git tag axonyx-runtime-v0.1.28
git push origin axonyx-runtime-v0.1.28
```

For UI releases, tag from the UI repository:

```bash
cd H:/CODE/axonyx/axonyx-ui
git tag axonyx-ui-v0.0.0
git push origin axonyx-ui-v0.0.0
```

Then create a GitHub release for the tag with:

- the crates.io package name and version
- the Docker image tag, when relevant
- the most important user-facing changes
- upgrade notes such as `cargo ax upgrade && cargo ax build --clean`

Do not move published release tags. If a publish is wrong, ship a new patch version and tag the new version.

## Stop Conditions

Stop the release if:

- CI is red
- `cargo ax doctor --deny-warnings` fails on a fresh app
- `cargo package` changes the package contents unexpectedly
- crates.io cannot resolve an upstream crate after normal propagation time
- generated docs still describe only local path or Git-only flows

If a partial publish happens, do not rush the next crate. Update release notes honestly and continue only when the next package is ready.
