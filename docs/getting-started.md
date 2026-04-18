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

## What You Get

Generated apps currently include:

- `app/` for `.ax` UI authoring
- `routes/` for route-style backend authoring
- `jobs/` for scheduled or background-style backend authoring
- `src/generated/` for generated backend Rust output
- `src/db/` and `src/domain/` as early integration seams
