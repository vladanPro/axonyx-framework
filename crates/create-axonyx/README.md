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

## Architecture Direction

Axonyx is not intended to be another React wrapper. It is aiming to become a Rust-first framework for sites, docs, CMS products, and full-stack apps where HTML-first output, explicit compilation, low JavaScript, and packageable themes/templates matter more than React compatibility.

Read the architecture comparison:

https://github.com/vladanPro/axonyx-framework/wiki/Next.js-vs-Axonyx

## Runtime Source

The default scaffold uses the published crates.io runtime:

```toml
axonyx-runtime = "0.1.3"
```

For framework development, use:

```bash
create-axonyx my-site --yes --runtime-source path
```

For testing an unreleased runtime branch, use:

```bash
create-axonyx my-site --yes --runtime-source git
```
