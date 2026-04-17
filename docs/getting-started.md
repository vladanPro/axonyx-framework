# Getting Started

This is the practical starting point for Axonyx development today.

## Local Scaffold

From the framework repo:

```bash
cargo run -p create-axonyx -- my-app --yes
```

Then:

```bash
cd my-app
cargo run
```

## Recommended Early-Adopter Flow

If you want your generated app to track the standalone runtime repository:

```bash
cargo run -p create-axonyx -- my-app --yes --runtime-source git
```

## First Useful Variants

Minimal starter:

```bash
cargo run -p create-axonyx -- my-app --yes --template minimal
```

Site starter:

```bash
cargo run -p create-axonyx -- my-site --yes --template site --runtime-source git
```

## What You Get

Generated apps currently include:

- `app/` for `.ax` UI authoring
- `routes/` for route-style backend authoring
- `jobs/` for scheduled or background-style backend authoring
- `src/generated/` for generated backend Rust output
- `src/db/` and `src/domain/` as early integration seams
