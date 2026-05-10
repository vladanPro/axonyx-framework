# Axonyx Framework Monorepo

Axonyx is a Rust-first framework for building Axonyx apps with `.ax` routes, Foundry UI packages, and a small Cargo-based authoring workflow.

Current focus:

- Rust-first app/runtime architecture
- JSX-like `.ax` authoring in `app/**/page.ax` and `app/**/layout.ax`
- route-aware static builds and local dev serving
- backend-oriented `.ax` files for loaders, actions, routes, and jobs
- reusable Foundry UI imports through `@axonyx/ui/...`

## Packages

This repository contains the public CLI packages:

- `create-axonyx` - project scaffolding CLI, similar in spirit to `create-next-app`
- `cargo-axonyx` - Cargo helper CLI exposed as `cargo ax ...`

Runtime crates live in the standalone runtime workspace and are consumed by generated apps through crates.io by default:

```toml
[dependencies]
axonyx-runtime = "0.1.0"
```

Axonyx UI is also available as both npm and Cargo package:

```toml
[dependencies]
axonyx-ui = "0.1.0"
```

## Quick Start

From this repository, create a new app:

```bash
cargo run -p create-axonyx -- my-app --yes
```

Create a site or docs app:

```bash
cargo run -p create-axonyx -- my-site --yes --template site
cargo run -p create-axonyx -- my-docs --yes --template docs
```

Generated apps use the published crates.io runtime source by default, so they do not need this monorepo or its submodule layout.

Available templates today:

- `minimal`
- `site`
- `docs`

## App Authoring Model

Recommended authoring path today:

- JSX-like `.ax` files in `app/**/page.ax` and `app/**/layout.ax`
- nested app routes with route-local `loader.ax` and `actions.ax` when needed
- imports from local app components via `@/components/...`
- imports from Axonyx UI packages via `@axonyx/ui/...`

Example route tree:

```text
app/
  layout.ax
  page.ax
  docs/
    page.ax
  components/
    page.ax
  blog/
    [slug]/
      page.ax
      loader.ax
```

Legacy indentation-first `.ax` syntax still exists for compatibility and reference work, but new examples and new framework authoring should prefer the JSX-like `.ax` direction.

## Common Commands

From an app root:

```bash
cargo ax doctor
cargo ax check
cargo ax build
cargo ax run dev
```

Use strict doctor mode in CI:

```bash
cargo ax doctor --deny-warnings
```

Use JSON output for editor tooling:

```bash
cargo ax doctor --format json
cargo ax routes --format json
```

## Build

`cargo ax build` scans backend-oriented `.ax` sources:

- `app/**/loader.ax`
- `app/**/actions.ax`
- `routes/**/*.ax`
- `jobs/**/*.ax`

and regenerates:

```text
src/generated/backend.rs
```

It also renders static page routes from `app/**/page.ax` into:

```text
dist/
  index.html
  docs/index.html
  components/index.html
  ...
```

Use a clean static output build when preparing deploy artifacts:

```bash
cargo ax build --clean
```

To choose another output directory:

```bash
cargo ax build --out-dir public-build --clean
```

Dynamic page routes are skipped unless they are listed in `Axonyx.toml`:

```toml
[prerender]
routes = [
  { route = "/blog/:slug", params = [{ slug = "hello-axonyx" }, { slug = "foundry-ui" }] },
]
```

That renders:

```text
dist/blog/hello-axonyx/index.html
dist/blog/foundry-ui/index.html
```

## Local Dev Server

Run the route-aware server:

```bash
cargo ax run dev
```

For a production-style process without dev live reload:

```bash
cargo ax run start --host 0.0.0.0 --port 3000
```

Inspect the route tree:

```bash
cargo ax routes
```

This lists `app/**/page.ax` page routes, `routes/**/*.ax` backend routes, dynamic params, nested layout count, and route-local `loader.ax` / `actions.ax` files.

## Adding Modules

Add a docs module into an existing app:

```bash
cargo ax add docs
```

Add the Foundry UI package when needed:

```bash
cargo ax add ui
```

Today, `cargo ax add ui` and the `site` / `docs` templates use the published `axonyx-ui` Cargo package by default.

## Runtime Source Options

Generated apps can target:

- the published crates.io package, `axonyx-runtime = "0.1.0"`
- a local Cargo `path` dependency into a checked-out runtime workspace
- the standalone Git repo at `https://github.com/vladanPro/axonyx-runtime`

Use `--runtime-source path` only when contributing to Axonyx itself from the framework workspace.

Use `--runtime-source git` when testing an unreleased runtime branch:

```bash
cargo run -p create-axonyx -- my-app --yes --runtime-source git
```

## Framework Development

When working on this monorepo itself:

```bash
git submodule update --init --recursive
cargo test
```

Run the core loop smoke test from the framework repo root:

```powershell
powershell -ExecutionPolicy Bypass -File scripts/smoke-core-loop.ps1 -Template site
```

The smoke test creates a temporary app, uses the local framework and local `axonyx-ui` when available, then runs:

```bash
cargo ax check
cargo ax doctor --deny-warnings
cargo ax build --clean
```

It passes only if the app has no strict doctor warnings/errors and `dist/index.html` is generated.

## Design Direction

Axonyx should stay Rust-first and compiler-assisted, not a React clone.

The preferred runtime direction is:

```txt
compile .ax
  -> static HTML with stable node ids
  -> dependency graph
  -> small runtime patcher
```

State and binding are separate concepts:

```txt
global/state = storage model
hard/soft = binding model
```

Preferred mental model:

```txt
Soft = snapshot
Hard = live handle
```

## Docs

The structured docs index lives in:

```text
docs/README.md
```

Recommended reading order:

- `docs/overview.md`
- `docs/ax-v2-authoring.md`
- `docs/templates.md`
- `docs/backend-authoring.md`
- `docs/release-runbook.md`

Drafts and lower-level architecture notes should live in `docs/`, not in the top-level README.

## Links

- Runtime repo: https://github.com/vladanPro/axonyx-runtime
- UI package: https://github.com/vladanPro/axonyx-ui
- React adapter: https://github.com/vladanPro/axonyx-react
- crates.io user: https://crates.io/users/vladanPro

## Repo Layout

```text
crates/
  cargo-axonyx/
  create-axonyx/
vendor/
  axonyx-runtime/
docs/
```
