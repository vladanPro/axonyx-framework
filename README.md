# Axonyx Framework

Axonyx is a Rust-first web framework and language layer for building low-JavaScript sites, docs, and future CMS-style applications with `.ax` files, Cargo-native tooling, and Foundry UI.

Status: public beta loop.

Axonyx can already scaffold apps, render `.ax` pages, build static output, serve route-aware local previews, import Foundry UI from `axonyx-ui`, and publish deployable sites. It is not yet a full replacement for React, Next.js, or mature CMS platforms. The next runtime work is tracked in the GitHub issues and Wiki.

## What Works Today

- JSX-like `.ax` authoring in `app/**/page.ax` and `app/**/layout.ax`
- nested app routes
- dynamic route params and query context
- route-local `loader.ax` and `actions.ax` draft support
- backend-oriented `.ax` files for loaders, actions, routes, and jobs
- static builds through `cargo ax build`
- route-aware dev/start server through `cargo ax run dev` and `cargo ax run start`
- strict project diagnostics through `cargo ax doctor --deny-warnings`
- early typed data checks for `type Post`, `List<Post>`, and `<Each>` field access
- reusable Foundry UI imports through `@axonyx/ui/...`
- generated apps consuming published crates from crates.io

## Packages

This repository contains the public CLI packages:

- `create-axonyx` - project scaffolding CLI, similar in spirit to `create-next-app`
- `cargo-axonyx` - Cargo helper CLI exposed as `cargo ax ...`

Generated apps consume the runtime and UI packages through crates.io by default:

```toml
[dependencies]
axonyx-runtime = "0.1.0"
axonyx-ui = "0.0.33"
```

## Quick Start

Install the public CLI tools:

```bash
cargo install create-axonyx
cargo install cargo-axonyx
```

Create and run a site:

```bash
create-axonyx my-site --yes --template site
cd my-site
cargo ax run dev
```

Check and build:

```bash
cargo ax doctor --deny-warnings
cargo ax build --clean
```

Available templates today:

- `minimal`
- `site`
- `docs`

From this repository, contributors can also run the scaffold locally:

```bash
cargo run -p create-axonyx -- my-site --yes --template site
```

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

Typed data is available in the JSX-like path:

```ax
page Blog

type Post {
  title: String
  slug: String
  summary?: String
}

let posts: List<Post> = load PostsList

<Each items={posts} as="post">
  <Card title={post.title} />
</Each>
```

`cargo ax check` reports `axonyx-type` diagnostics when a typed page accesses a missing field such as `post.summary`.
Use `post?.summary` when a missing field is intentional and should render as an empty string.
Use `summary?: String` in the type when the field is part of the schema but optional.

## Common Commands

From an app root:

```bash
cargo ax doctor
cargo ax check
cargo ax content
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

Inspect content collections:

```bash
cargo ax content
cargo ax content --format json
```

Configure early Melt-time content indexing in `Axonyx.toml`:

```toml
[content.collections.docs]
path = "content/docs"
extensions = ["md", "mdx"]
```

This indexes matching files into a content manifest today. Later runtime work can use the same manifest for docs, blog, and CMS routing.
During `cargo ax build`, configured collections are written to:

```text
dist/_ax/content/manifest.json
```

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

The server/runtime can be async internally, but Axonyx authoring should stay structured and declarative. Developers should place work into `loader`, `action`, `signal`, `<Await>`, and `job` instead of hand-orchestrating promise timing.

See [Structured Async In Axonyx](./docs/architecture/structured-async.md).

## Roadmap

The next framework spine is tracked as `Axonyx Runtime Core / The Melt`.

Primary GitHub issues:

- [#8 Epic: Axonyx Runtime Core / The Melt](https://github.com/vladanPro/axonyx-framework/issues/8)
- [#9 axonyx-server-net: migrate server to Hyper/Tokio](https://github.com/vladanPro/axonyx-framework/issues/9)
- [#10 axonyx-std-fs: capability FS and content collections](https://github.com/vladanPro/axonyx-framework/issues/10)
- [#11 axonyx-std-state: SignalId bridge and typed patches](https://github.com/vladanPro/axonyx-framework/issues/11)
- [#12 axonyx-std-auth: sessions, crypto, and policies](https://github.com/vladanPro/axonyx-framework/issues/12)
- [#13 axonyx-std-process: jobs, workers, and child processes](https://github.com/vladanPro/axonyx-framework/issues/13)
- [#14 Framework finishing layer after runtime core](https://github.com/vladanPro/axonyx-framework/issues/14)

Architecture references:

- [Next.js vs Axonyx](https://github.com/vladanPro/axonyx-framework/wiki/Next.js-vs-Axonyx)
- [The Melt](https://github.com/vladanPro/axonyx-framework/wiki/The-Melt)
- [Structured Async In Axonyx](https://github.com/vladanPro/axonyx-framework/wiki/Structured-Async-In-Axonyx)
- [Axonyx Modules](https://github.com/vladanPro/axonyx-framework/wiki/Axonyx-Modules)
- [Axonyx CLI](https://github.com/vladanPro/axonyx-framework/wiki/Axonyx-CLI)

## Docs

The structured docs index lives in:

```text
docs/README.md
```

Recommended reading order:

- `docs/overview.md`
- `docs/ax-v2-authoring.md`
- `docs/architecture/structured-async.md`
- `docs/templates.md`
- `docs/backend-authoring.md`
- `docs/release-runbook.md`

Drafts and lower-level architecture notes should live in `docs/`, not in the top-level README.

## Links

- Main site: https://axonyx.dev
- React adapter site: https://react.axonyx.dev
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
