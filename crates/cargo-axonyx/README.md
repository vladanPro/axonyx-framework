# cargo-axonyx

Cargo helper commands for Axonyx apps.

This package installs the `cargo ax` and `cargo axonyx` subcommands used for the Axonyx local development loop.

## Install

```bash
cargo install cargo-axonyx
```

## Commands

```bash
cargo ax check
cargo ax content
cargo ax doctor
cargo ax build --clean
cargo ax run dev
```

## Typical Flow

```bash
create-axonyx my-site --yes --template site
cd my-site
cargo ax doctor
cargo ax run dev
```

`cargo ax doctor` verifies the app structure, runtime dependency, Axonyx UI package resolution, stylesheet wiring, and `.ax` source diagnostics.

`cargo ax content` reads `[content.collections]` from `Axonyx.toml` and prints the current Melt-time content manifest.
`cargo ax build` writes the same manifest to `dist/_ax/content/manifest.json` when collections are configured.

## Package Model

Generated apps depend on published Cargo packages by default:

```toml
axonyx-runtime = "0.1.5"
axonyx-ui = "0.0.33"
```

Local path and package override flows are still supported for framework development and UI dogfooding.

## Architecture Direction

Axonyx is not intended to be another React wrapper. It is aiming to become a Rust-first framework for sites, docs, CMS products, and full-stack apps where HTML-first output, explicit compilation, low JavaScript, and packageable themes/templates matter more than React compatibility.

Read the architecture comparison:

https://github.com/vladanPro/axonyx-framework/wiki/Next.js-vs-Axonyx
