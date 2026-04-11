# Axonix Framework Monorepo

Axonix is a Rust-first framework focused on:

- Single-binary architecture
- Algebraic UI pipelines (`|>`)
- Fast data-to-UI execution flow

This repository includes:

- `axonix-core`: pipeline parser and core types
- `axonix-adapter-blokbite`: adapter contract for BlokBite-style block systems
- `create-axonix`: project scaffolding CLI (similar to `create-next-app`)

## Quick Start

### 1) Create a new Axonix app locally

```bash
cargo run -p create-axonix -- my-app --yes
```

### 2) Run the generated app

```bash
cd my-app
cargo run
```

## Planned global CLI flow

Once published, we target:

```bash
cargo install create-axonix
create-axonix my-app --yes
```

## Repo Layout

```text
crates/
  axonix-core/
  axonix-adapter-blokbite/
  create-axonix/
```

