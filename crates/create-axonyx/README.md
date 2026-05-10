# create-axonyx

Project scaffolding CLI for Axonyx apps.

Use it to create a new Axonyx project with working `.ax` pages, runtime wiring, optional Foundry UI setup, and the `cargo ax` developer loop.

## Install

```bash
cargo install create-axonyx
```

## Create A Project

```bash
create-axonyx my-site --yes --template site
cd my-site
cargo ax doctor
cargo ax run dev
```

Available templates:

- `minimal`
- `site`
- `docs`

## Runtime Source

The default scaffold uses the published crates.io runtime:

```toml
axonyx-runtime = "0.1.0"
```

For framework development, use:

```bash
create-axonyx my-site --yes --runtime-source path
```

For testing an unreleased runtime branch, use:

```bash
create-axonyx my-site --yes --runtime-source git
```

