# CLI Safety Patches

Published packages: `create-axonyx 0.8.4` and `cargo-axonyx 0.8.5`.
The scaffold still pins `axonyx-runtime 0.8.2` and `axonyx-ui 0.0.86`.
Core, runtime, LSP, UI and npm packages are unchanged by this release.

- `create-axonyx --force` asks before replacing an existing directory. It
  builds the new project in a sibling staging directory, including optional
  Git initialization, before moving the old directory to a backup and
  promoting the new one. A scaffold failure leaves the old directory in place.
- `cargo ax build` validates its output path before writing, with or without
  `--clean`. It rejects app-root and source directories, parent traversal,
  out-of-root paths, and source aliases through symlinks. Cleaning a custom
  output requires a marker from a previous successful Axonyx build.
- The framework root `Cargo.lock` is tracked so the declared Rust 1.82 CI
  check uses a reproducible dependency set.

The full framework CI passed on `main` commit `607104e`. Both packages passed
`cargo package --locked` verification and were published to crates.io. The
registry-only smoke test installs those exact versions outside the checkout.
