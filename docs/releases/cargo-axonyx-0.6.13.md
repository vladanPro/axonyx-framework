# cargo-axonyx 0.6.13

Release candidate: align the CLI-owned preview with published runtime 0.6.9.
The core stays at 0.6.8; this release does not republish the UI, scaffolder or LSP.
The framework runtime gitlink selects the published release commit b6979a1.

An app lock update affects its compiled server, not an existing installed CLI's
preview runtime. This CLI patch embeds runtime 0.6.9 and recommends that version
through cargo ax upgrade/doctor. Existing app locks still need a dependency update
and compiled apps must be rebuilt before using the patch there.

The focused HTTP proof runs the same fixture through `run dev` and
`run start --compiled`. Both must return HTTP 422 with the opted-in disabled
textarea's exact escaped submitted value and recovery marker. Disabled state
is preserved, unmarked disabled fields are untouched and JSON does not expose
the retained document. The fixture has no database, login or application Rust
callbacks. It uses a disposable directory and isolated localhost port.

```powershell
pwsh -File scripts/smoke-native-form-retry.ps1
```

Once the candidate passes CI and is promoted/published:

```sh
cargo install cargo-axonyx --version 0.6.13 --locked --force
```

This document is not evidence of registry availability. Source CI, packaging
against registry runtime 0.6.9 and release promotion are publication gates.
The prior installed CLI is not changed by local proof runs.

This does not let preview execute application Rust/native callbacks. Such apps,
including Legura's current rich-content pilot, must continue to build and run
their app-specific compiled server. It is not a CMS updater or automatic editor
activation, and it does not change no-JS rich editing into writable content.

Local acceptance: 359 CLI library tests, the complete Rust 1.82 workspace
all-target/all-feature check, formatting and the HTTP proof in both server modes
passed. The minimal generated server still emits existing unused-helper/mutability
warnings; those are not a claim of a warning-free generated application. The
package archive built and verified against registry runtime 0.6.9. CI and release
promotion remain publication gates.
