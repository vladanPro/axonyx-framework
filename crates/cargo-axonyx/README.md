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

## Package Model

Generated apps depend on published Cargo packages by default:

```toml
axonyx-runtime = "0.1.0"
axonyx-ui = "0.0.32"
```

Local path and package override flows are still supported for framework development and UI dogfooding.

