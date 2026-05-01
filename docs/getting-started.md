# Getting Started

This is the practical starting point for Axonyx development today.

## Local Scaffold

From the framework repo:

```bash
git submodule update --init --recursive
cargo run -p create-axonyx -- my-app --yes
```

By default the generated app uses the shared `axonyx-runtime` Git repository, so regular users do not need the framework submodule setup after scaffolding.

Then:

```bash
cd my-app
cargo run
```

If `cargo-axonyx` is installed, the first framework-shaped local loop is:

```bash
cargo ax build
cargo ax run dev
```

`cargo ax build` regenerates `src/generated/backend.rs` from:

- `app/**/loader.ax`
- `app/**/actions.ax`
- `routes/**/*.ax`
- `jobs/**/*.ax`

`cargo ax run dev` now runs that backend sync once before starting the local route-aware dev server with live reload polling.

For a production-style local run, use:

```bash
cargo ax build
cargo ax run start --host 0.0.0.0 --port 3000
```

`cargo ax run start` serves the same Axonyx app routes and public assets without injecting the dev live-reload client. On a host such as Render, use the platform `PORT` value in the start command.

## Runtime Source Defaults

The default scaffold flow now uses `--runtime-source git`.

- `git`
  - best default for current public use
- `path`
  - best for Axonyx contributors working inside the framework repo
- `registry`
  - best once the runtime crates are published

```bash
cargo run -p create-axonyx -- my-app --yes
```

## First Useful Variants

Minimal starter:

```bash
cargo run -p create-axonyx -- my-app --yes --template minimal
```

Site starter:

```bash
cargo run -p create-axonyx -- my-site --yes --template site
```

Docs starter:

```bash
cargo run -p create-axonyx -- my-docs --yes --template docs
```

## What You Get

Generated apps currently include:

- `app/` for `.ax` UI authoring
- `routes/` for route-style backend authoring
- `jobs/` for scheduled or background-style backend authoring
- `src/generated/` for generated backend Rust output
- `src/db/` and `src/domain/` as early integration seams

## Next Step To Close Core

After the first run loop works, use the proof checklist to verify the full framework story:

- [Proof App Checklist](./proof-app-checklist.md)

That checklist is the fastest path to confirm that your project is not only scaffolded, but
also aligned with the current AX v2 route, import, loader/action, and dev-server flow.
